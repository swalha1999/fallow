//! Bare specifier caching for import resolution.

use dashmap::DashMap;

use super::types::ResolveResult;

/// Thread-safe cache for bare specifier resolutions using lock-free concurrent reads.
/// Bare specifiers (like `react`, `lodash/merge`) resolve to the same target
/// regardless of which file imports them (modulo nested `node_modules`, which is rare).
/// Uses `DashMap` (sharded read-write locks) instead of `Mutex<FxHashMap>` to eliminate
/// contention under rayon's work-stealing on large projects.
pub(super) struct BareSpecifierCache {
    cache: DashMap<String, ResolveResult>,
}

impl BareSpecifierCache {
    pub(super) fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    pub(super) fn get(&self, specifier: &str) -> Option<ResolveResult> {
        self.cache.get(specifier).map(|entry| entry.clone())
    }

    pub(super) fn insert(&self, specifier: String, result: ResolveResult) {
        self.cache.insert(specifier, result);
    }
}
