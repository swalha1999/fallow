use std::collections::{HashMap, HashSet};

use fallow_config::ResolvedConfig;

use crate::discover::FileId;
use crate::graph::ModuleGraph;
use crate::results::*;
use crate::suppress::{self, IssueKind, Suppression};

use super::{byte_offset_to_line_col, read_source};

/// Find exports that are never imported by other files.
pub(crate) fn find_unused_exports(
    graph: &ModuleGraph,
    config: &ResolvedConfig,
    plugin_result: Option<&crate::plugins::AggregatedPluginResult>,
    suppressions_by_file: &HashMap<FileId, &[Suppression]>,
) -> (Vec<UnusedExport>, Vec<UnusedExport>) {
    let mut unused_exports = Vec::new();
    let mut unused_types = Vec::new();

    // Pre-compile glob matchers for ignore rules
    let ignore_matchers: Vec<(globset::GlobMatcher, &[String])> = config
        .ignore_export_rules
        .iter()
        .filter_map(|rule| {
            globset::Glob::new(&rule.file)
                .ok()
                .map(|g| (g.compile_matcher(), rule.exports.as_slice()))
        })
        .collect();

    // Compile plugin-discovered used_exports rules (includes framework preset rules)
    let plugin_matchers: Vec<(globset::GlobMatcher, Vec<&str>)> = plugin_result
        .map(|pr| {
            pr.used_exports
                .iter()
                .filter_map(|(file_pat, exports)| {
                    globset::Glob::new(file_pat).ok().map(|g| {
                        (
                            g.compile_matcher(),
                            exports.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    for module in &graph.modules {
        // Skip unreachable modules (already reported as unused files)
        if !module.is_reachable {
            continue;
        }

        // Skip entry points (their exports are consumed externally)
        if module.is_entry_point {
            continue;
        }

        // Skip CJS modules with module.exports (hard to track individual exports)
        if module.has_cjs_exports && module.exports.is_empty() {
            continue;
        }

        // Namespace imports are now handled with member-access narrowing in graph.rs:
        // only specific accessed members get references populated. No blanket skip needed.

        // Svelte files use `export let`/`export const` for component props, which are
        // consumed by the Svelte runtime rather than imported by other modules. Since we
        // can't distinguish props from utility exports in the `<script>` block without
        // Svelte compiler semantics, we skip export analysis entirely for reachable
        // .svelte files. Unreachable Svelte files are still caught by `find_unused_files`.
        if module.path.extension().is_some_and(|ext| ext == "svelte") {
            continue;
        }

        // Check ignore rules — compute relative path and string once per module
        let relative_path = module
            .path
            .strip_prefix(&config.root)
            .unwrap_or(&module.path);
        let file_str = relative_path.to_string_lossy();

        // Pre-check which ignore/plugin matchers match this file
        let matching_ignore: Vec<&[String]> = ignore_matchers
            .iter()
            .filter(|(m, _)| m.is_match(file_str.as_ref()))
            .map(|(_, exports)| *exports)
            .collect();

        // Check plugin-discovered used_exports rules (includes framework preset rules)
        let matching_plugin: Vec<&Vec<&str>> = plugin_matchers
            .iter()
            .filter(|(m, _)| m.is_match(file_str.as_ref()))
            .map(|(_, exports)| exports)
            .collect();

        // Lazily load source content for line/col computation
        let mut source_content: Option<String> = None;

        for export in &module.exports {
            if export.references.is_empty() {
                let export_str = export.name.to_string();

                // Check if this export is ignored by config
                if matching_ignore
                    .iter()
                    .any(|exports| exports.iter().any(|e| e == "*" || e == &export_str))
                {
                    continue;
                }

                // Check if this export is considered "used" by a plugin/framework rule
                if matching_plugin
                    .iter()
                    .any(|exports| exports.iter().any(|e| *e == export_str))
                {
                    continue;
                }

                let source = source_content.get_or_insert_with(|| read_source(&module.path));
                let (line, col) = byte_offset_to_line_col(source, export.span.start);

                // Barrel re-exports are synthesized in graph.rs with Span::new(0, 0) as a sentinel.
                let is_re_export = export.span.start == 0 && export.span.end == 0;

                // Check inline suppression
                let issue_kind = if export.is_type_only {
                    IssueKind::UnusedType
                } else {
                    IssueKind::UnusedExport
                };
                if let Some(supps) = suppressions_by_file.get(&module.file_id)
                    && suppress::is_suppressed(supps, line, issue_kind)
                {
                    continue;
                }

                let unused = UnusedExport {
                    path: module.path.clone(),
                    export_name: export_str,
                    is_type_only: export.is_type_only,
                    line,
                    col,
                    span_start: export.span.start,
                    is_re_export,
                };

                if export.is_type_only {
                    unused_types.push(unused);
                } else {
                    unused_exports.push(unused);
                }
            }
        }
    }

    (unused_exports, unused_types)
}

/// Find exports that appear with the same name in multiple files (potential duplicates).
///
/// Barrel re-exports (files that only re-export from other modules via `export { X } from './source'`)
/// are excluded — having an index.ts re-export the same name as the source module is the normal
/// barrel file pattern, not a true duplicate.
pub(crate) fn find_duplicate_exports(
    graph: &ModuleGraph,
    config: &ResolvedConfig,
    suppressions_by_file: &HashMap<FileId, &[Suppression]>,
) -> Vec<DuplicateExport> {
    // Build a set of re-export relationships: (re-exporting module idx) -> set of (source module idx)
    let mut re_export_sources: HashMap<usize, HashSet<usize>> = HashMap::new();
    for (idx, module) in graph.modules.iter().enumerate() {
        for re in &module.re_exports {
            re_export_sources
                .entry(idx)
                .or_default()
                .insert(re.source_file.0 as usize);
        }
    }

    let mut export_locations: HashMap<String, Vec<(usize, std::path::PathBuf)>> = HashMap::new();

    for (idx, module) in graph.modules.iter().enumerate() {
        if !module.is_reachable || module.is_entry_point {
            continue;
        }

        // Skip files with file-wide duplicate-export suppression
        if suppressions_by_file
            .get(&module.file_id)
            .is_some_and(|supps| suppress::is_file_suppressed(supps, IssueKind::DuplicateExport))
        {
            continue;
        }

        for export in &module.exports {
            if matches!(export.name, crate::extract::ExportName::Default) {
                continue; // Skip default exports
            }
            // Skip synthetic re-export entries (span 0..0) — these are generated by
            // graph construction for re-exports, not real local declarations
            if export.span.start == 0 && export.span.end == 0 {
                continue;
            }
            let name = export.name.to_string();
            export_locations
                .entry(name)
                .or_default()
                .push((idx, module.path.clone()));
        }
    }

    // Filter: only keep truly independent duplicates (not re-export chains)
    let _ = config; // used for consistency
    export_locations
        .into_iter()
        .filter_map(|(name, locations)| {
            if locations.len() <= 1 {
                return None;
            }
            // Remove entries where one module re-exports from another in the set.
            // For each pair (A, B), if A re-exports from B or B re-exports from A,
            // they are part of the same export chain, not true duplicates.
            let module_indices: HashSet<usize> = locations.iter().map(|(idx, _)| *idx).collect();
            let independent: Vec<std::path::PathBuf> = locations
                .into_iter()
                .filter(|(idx, _)| {
                    // Keep this module only if it doesn't re-export from another module in the set
                    // AND no other module in the set re-exports from it (unless both are sources)
                    let sources = re_export_sources.get(idx);
                    let has_source_in_set = sources
                        .map(|s| s.iter().any(|src| module_indices.contains(src)))
                        .unwrap_or(false);
                    !has_source_in_set
                })
                .map(|(_, path)| path)
                .collect();

            if independent.len() > 1 {
                Some(DuplicateExport {
                    export_name: name,
                    locations: independent,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Collect usage counts for all exports in the module graph.
///
/// Iterates every module and every export, producing an `ExportUsage` entry with the
/// reference count and reference locations. This data is used by the LSP server to show
/// Code Lens annotations (e.g., "3 references") above export declarations, with
/// click-to-navigate support via `editor.action.showReferences`.
pub(crate) fn collect_export_usages(graph: &ModuleGraph) -> Vec<ExportUsage> {
    let mut usages = Vec::new();

    // Build FileId -> path index for resolving reference locations
    let file_paths: HashMap<FileId, &std::path::Path> = graph
        .modules
        .iter()
        .map(|m| (m.file_id, m.path.as_path()))
        .collect();

    // Cache source content per file for byte offset -> line/col conversion
    let mut source_cache: HashMap<FileId, String> = HashMap::new();

    for module in &graph.modules {
        // Skip unreachable modules — no point showing Code Lens for files
        // that aren't reachable from any entry point
        if !module.is_reachable {
            continue;
        }

        // Lazily load source content for byte offset -> line/col conversion
        let mut source_content: Option<String> = None;

        for export in &module.exports {
            // Skip synthetic re-export entries (span 0..0) — these are generated
            // by graph construction, not real local declarations in the source
            if export.span.start == 0 && export.span.end == 0 {
                continue;
            }

            let source = source_content.get_or_insert_with(|| read_source(&module.path));
            let (line, col) = byte_offset_to_line_col(source, export.span.start);

            // Resolve reference locations for Code Lens navigation
            let reference_locations: Vec<ReferenceLocation> = export
                .references
                .iter()
                .filter_map(|r| {
                    // Skip references with no span (e.g. from dynamic import patterns)
                    if r.import_span.start == 0 && r.import_span.end == 0 {
                        return None;
                    }
                    let ref_path = file_paths.get(&r.from_file)?;
                    let ref_source = source_cache
                        .entry(r.from_file)
                        .or_insert_with(|| read_source(ref_path));
                    let (ref_line, ref_col) =
                        byte_offset_to_line_col(ref_source, r.import_span.start);
                    Some(ReferenceLocation {
                        path: ref_path.to_path_buf(),
                        line: ref_line,
                        col: ref_col,
                    })
                })
                .collect();

            usages.push(ExportUsage {
                path: module.path.clone(),
                export_name: export.name.to_string(),
                line,
                col,
                reference_count: export.references.len(),
                reference_locations,
            });
        }
    }

    usages
}
