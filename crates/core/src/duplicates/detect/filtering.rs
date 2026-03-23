//! Step 6: Result filtering and deduplication.
//!
//! Converts raw clone groups into `CloneGroup` structs with line info,
//! applies token-level and line-level subset removal, min_lines filter,
//! and skip_local filter.

use std::path::PathBuf;

use rustc_hash::{FxHashMap, FxHashSet};

use super::FileData;
use super::extraction::RawGroup;
use super::utils::build_clone_instance_fast;
use crate::duplicates::types::{CloneGroup, CloneInstance};

/// Convert raw groups into `CloneGroup` structs, applying `min_lines` and
/// `skip_local` filters, deduplication, and subset removal.
pub(super) fn build_groups(
    mut raw_groups: Vec<RawGroup>,
    files: &[FileData],
    min_lines: usize,
    skip_local: bool,
) -> Vec<CloneGroup> {
    if raw_groups.is_empty() {
        return Vec::new();
    }

    // ── Token-level subset removal (cheap) ────────────────
    //
    // Sort raw groups by length desc. For each instance (file_id, offset),
    // track non-overlapping covered intervals per file. A smaller group is
    // skipped if all its instances fall within already-covered intervals.
    // This eliminates the vast majority of raw groups before the expensive
    // line-calculation step.
    let raw_count = raw_groups.len();
    raw_groups.sort_by(|a, b| b.length.cmp(&a.length));

    // covered[file_id] is a sorted vec of non-overlapping (start, end)
    // intervals. Kept sorted by start for binary search.
    let mut covered: Vec<Vec<(usize, usize)>> = vec![Vec::new(); files.len()];
    let mut surviving_groups: Vec<RawGroup> = Vec::new();

    for rg in raw_groups {
        let len = rg.length;
        // Check if all instances are fully covered by existing intervals.
        let all_covered = rg.instances.iter().all(|&(fid, offset)| {
            let intervals = &covered[fid];
            // Binary search for the interval that could contain [offset, offset+len).
            let idx = intervals.partition_point(|&(s, _)| s <= offset);
            if idx > 0 {
                let (s, e) = intervals[idx - 1];
                offset >= s && offset + len <= e
            } else {
                false
            }
        });

        if !all_covered {
            // Insert covered intervals for this group's instances.
            for &(fid, offset) in &rg.instances {
                let end = offset + len;
                let intervals = &mut covered[fid];
                let idx = intervals.partition_point(|&(s, _)| s < offset);
                // Check if the new interval merges with an existing one.
                if idx > 0 {
                    let prev = &mut intervals[idx - 1];
                    if prev.1 >= offset {
                        // Extend previous interval if needed.
                        if end > prev.1 {
                            prev.1 = end;
                        }
                        continue;
                    }
                }
                intervals.insert(idx, (offset, end));
            }
            surviving_groups.push(rg);
        }
    }

    tracing::trace!(
        raw = raw_count,
        surviving = surviving_groups.len(),
        "token-level subset removal"
    );

    // ── Pre-compute line offset tables ────────────────────
    //
    // For each file, build a sorted vec of newline byte positions so that
    // byte_offset_to_line_col can use binary search (O(log L)) instead of
    // linear scan (O(L)).
    let line_tables: Vec<Vec<usize>> = files
        .iter()
        .map(|f| {
            f.file_tokens
                .source
                .bytes()
                .enumerate()
                .filter_map(|(i, b)| if b == b'\n' { Some(i) } else { None })
                .collect()
        })
        .collect();

    // ── Build CloneGroups for survivors ────────────────────
    let mut clone_groups: Vec<CloneGroup> = Vec::new();

    for rg in surviving_groups {
        // Build instances, deduplicating by (file_id, offset).
        let mut seen: FxHashSet<(usize, usize)> = FxHashSet::default();
        let mut group_instances: Vec<CloneInstance> = Vec::new();

        for &(file_id, offset) in &rg.instances {
            if !seen.insert((file_id, offset)) {
                continue;
            }

            let file = &files[file_id];
            if let Some(inst) =
                build_clone_instance_fast(file, offset, rg.length, &line_tables[file_id])
            {
                group_instances.push(inst);
            }
        }

        // Apply skip_local: only keep cross-directory clones.
        if skip_local && group_instances.len() >= 2 {
            let dirs: FxHashSet<_> = group_instances
                .iter()
                .filter_map(|inst| inst.file.parent().map(|p| p.to_path_buf()))
                .collect();
            if dirs.len() < 2 {
                continue;
            }
        }

        if group_instances.len() < 2 {
            continue;
        }

        // Calculate line count from the instances.
        let line_count = group_instances
            .iter()
            .map(|inst| inst.end_line.saturating_sub(inst.start_line) + 1)
            .max()
            .unwrap_or(0);

        // Apply minimum line filter.
        if line_count < min_lines {
            continue;
        }

        // Sort instances by file path then start line for stable output.
        group_instances.sort_by(|a, b| a.file.cmp(&b.file).then(a.start_line.cmp(&b.start_line)));

        // Deduplicate instances that map to overlapping line ranges within
        // the same file (different token offsets can resolve to overlapping
        // source spans). When two instances overlap, keep the wider one.
        group_instances.dedup_by(|b, a| {
            if a.file != b.file {
                return false;
            }
            // Instances are sorted by start_line. `b` starts at or after `a`.
            // If b's start overlaps with a's range, merge by extending a.
            if b.start_line <= a.end_line {
                // Keep the wider range in `a`.
                if b.end_line > a.end_line {
                    a.end_line = b.end_line;
                    a.end_col = b.end_col;
                }
                true
            } else {
                false
            }
        });

        if group_instances.len() < 2 {
            continue;
        }

        clone_groups.push(CloneGroup {
            instances: group_instances,
            token_count: rg.length,
            line_count,
        });
    }

    // Sort groups by token count (largest first), breaking ties by instance
    // count (most instances first). This ensures that for equal token counts,
    // N-way groups come before M-way (M<N) subsets, so subset removal works
    // correctly regardless of the suffix array's extraction order.
    clone_groups.sort_by(|a, b| {
        b.token_count
            .cmp(&a.token_count)
            .then(b.instances.len().cmp(&a.instances.len()))
    });

    // Remove groups whose line ranges are fully contained within another
    // group's line ranges. Uses a per-file interval index to avoid O(g²×m×n).
    //
    // Strategy: iterate groups from largest to smallest. For each kept group,
    // register its (file, start_line, end_line) spans into a spatial index.
    // Smaller groups are checked against this index in O(instances × log(intervals)).

    // Build file path → index mapping for interval tracking
    let mut path_to_idx: FxHashMap<PathBuf, usize> = FxHashMap::default();
    let mut next_idx = 0usize;
    for group in &clone_groups {
        for inst in &group.instances {
            path_to_idx.entry(inst.file.clone()).or_insert_with(|| {
                let idx = next_idx;
                next_idx += 1;
                idx
            });
        }
    }
    let mut file_intervals: Vec<Vec<(usize, usize)>> = vec![Vec::new(); next_idx];
    let mut kept_groups: Vec<CloneGroup> = Vec::new();

    for group in clone_groups {
        // Check if ALL instances of this group are contained within existing intervals
        let all_contained = group.instances.iter().all(|inst| {
            let fidx = path_to_idx[&inst.file];
            let intervals = &file_intervals[fidx];
            let idx = intervals.partition_point(|&(s, _)| s <= inst.start_line);
            idx > 0 && {
                let (s, e) = intervals[idx - 1];
                inst.start_line >= s && inst.end_line <= e
            }
        });

        if !all_contained {
            // Register this group's instances into the spatial index
            for inst in &group.instances {
                let fidx = path_to_idx[&inst.file];
                let intervals = &mut file_intervals[fidx];
                let idx = intervals.partition_point(|&(s, _)| s < inst.start_line);
                if idx > 0 && intervals[idx - 1].1 >= inst.start_line {
                    if inst.end_line > intervals[idx - 1].1 {
                        intervals[idx - 1].1 = inst.end_line;
                    }
                } else {
                    intervals.insert(idx, (inst.start_line, inst.end_line));
                }
            }
            kept_groups.push(group);
        }
    }

    kept_groups
}
