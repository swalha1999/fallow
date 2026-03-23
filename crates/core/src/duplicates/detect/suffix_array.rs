//! Step 3: Suffix array construction using O(N log N) prefix-doubling with radix sort.

/// Build a suffix array using the O(N log N) prefix-doubling algorithm with
/// radix sort.
///
/// Returns `sa` where `sa[i]` is the starting position of the i-th
/// lexicographically smallest suffix in `text`.
pub(super) fn build_suffix_array(text: &[i64]) -> Vec<usize> {
    let n = text.len();
    if n == 0 {
        return vec![];
    }

    // Initial ranks based on raw values. Shift so sentinels (negative) sort
    // before all real tokens.
    let min_val = text.iter().copied().min().unwrap_or(0);
    let mut rank: Vec<i64> = text.iter().map(|&v| v - min_val).collect();
    let mut sa: Vec<usize> = (0..n).collect();
    let mut tmp: Vec<i64> = vec![0; n];
    let mut k: usize = 1;
    let mut iterations = 0u32;

    // Scratch buffers for radix sort (reused across iterations).
    let mut sa_tmp: Vec<usize> = vec![0; n];

    while k < n {
        iterations += 1;
        let max_rank = rank.iter().copied().max().unwrap_or(0) as usize;

        // Two-pass radix sort: sort by secondary key (rank[i+k]) first,
        // then by primary key (rank[i]). Each pass is O(N + K) where
        // K = max_rank + 2 (including the -1 sentinel rank).
        let bucket_count = max_rank + 2; // ranks 0..=max_rank plus -1 mapped to 0

        // Pass 1: sort by secondary key (rank at offset k).
        let mut counts = vec![0usize; bucket_count + 1];
        for &i in &sa {
            let r2 = if i + k < n {
                rank[i + k] as usize + 1
            } else {
                0
            };
            counts[r2] += 1;
        }
        // Prefix sum.
        let mut sum = 0;
        for c in &mut counts {
            let v = *c;
            *c = sum;
            sum += v;
        }
        for &i in &sa {
            let r2 = if i + k < n {
                rank[i + k] as usize + 1
            } else {
                0
            };
            sa_tmp[counts[r2]] = i;
            counts[r2] += 1;
        }

        // Pass 2: sort by primary key (rank[i]), stable.
        // No +1 offset needed here: rank[i] is always >= 0 because the
        // initial ranks are shifted by min_val, and subsequent iterations
        // assign ranks starting from 0.
        counts.fill(0);
        counts.resize(bucket_count + 1, 0);
        for &i in &sa_tmp {
            let r1 = rank[i] as usize;
            counts[r1] += 1;
        }
        sum = 0;
        for c in &mut counts {
            let v = *c;
            *c = sum;
            sum += v;
        }
        for &i in &sa_tmp {
            let r1 = rank[i] as usize;
            sa[counts[r1]] = i;
            counts[r1] += 1;
        }

        // Compute new ranks.
        tmp[sa[0]] = 0;
        for i in 1..n {
            let prev = sa[i - 1];
            let curr = sa[i];
            let same = rank[prev] == rank[curr] && {
                let rp2 = if prev + k < n { rank[prev + k] } else { -1 };
                let rc2 = if curr + k < n { rank[curr + k] } else { -1 };
                rp2 == rc2
            };
            tmp[curr] = tmp[prev] + i64::from(!same);
        }

        // Early exit when all ranks are unique.
        let new_max_rank = tmp[sa[n - 1]];
        std::mem::swap(&mut rank, &mut tmp);

        if new_max_rank as usize == n - 1 {
            break;
        }

        k *= 2;
    }

    tracing::trace!(n, iterations, "suffix array constructed");
    sa
}
