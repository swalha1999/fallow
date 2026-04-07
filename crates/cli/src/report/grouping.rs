//! Grouping infrastructure for `--group-by owner|directory|package`.
//!
//! Partitions `AnalysisResults` into labeled groups by ownership (CODEOWNERS),
//! by first directory component, or by workspace package.

use std::path::{Path, PathBuf};

use fallow_config::WorkspaceInfo;
use fallow_core::results::AnalysisResults;
use rustc_hash::FxHashMap;

use super::relative_path;
use crate::codeowners::{self, CodeOwners, UNOWNED_LABEL};

/// Ownership resolver for `--group-by`.
///
/// Owns the `CodeOwners` data when grouping by owner, avoiding lifetime
/// complexity in the report context.
pub enum OwnershipResolver {
    /// Group by CODEOWNERS file (first owner, last matching rule).
    Owner(CodeOwners),
    /// Group by first directory component.
    Directory,
    /// Group by workspace package (monorepo).
    Package(PackageResolver),
}

/// Resolves file paths to workspace package names via longest-prefix matching.
///
/// Stores workspace roots as paths relative to the project root so that
/// resolution works with the relative paths passed to `OwnershipResolver::resolve`.
pub struct PackageResolver {
    /// `(relative_root, package_name)` sorted by path length descending.
    workspaces: Vec<(PathBuf, String)>,
}

const ROOT_PACKAGE_LABEL: &str = "(root)";

impl PackageResolver {
    /// Build a resolver from discovered workspace info.
    ///
    /// Workspace roots are stored relative to `project_root` and sorted by path
    /// length descending so the first match is always the most specific prefix.
    pub fn new(project_root: &Path, workspaces: &[WorkspaceInfo]) -> Self {
        let mut ws: Vec<(PathBuf, String)> = workspaces
            .iter()
            .map(|w| {
                let rel = w.root.strip_prefix(project_root).unwrap_or(&w.root);
                (rel.to_path_buf(), w.name.clone())
            })
            .collect();
        ws.sort_by(|a, b| b.0.as_os_str().len().cmp(&a.0.as_os_str().len()));
        Self { workspaces: ws }
    }

    /// Find the workspace package that owns `rel_path`, or `"(root)"` if none match.
    fn resolve(&self, rel_path: &Path) -> &str {
        self.workspaces
            .iter()
            .find(|(root, _)| rel_path.starts_with(root))
            .map_or(ROOT_PACKAGE_LABEL, |(_, name)| name.as_str())
    }
}

impl OwnershipResolver {
    /// Resolve the group key for a file path (relative to project root).
    pub fn resolve(&self, rel_path: &Path) -> String {
        match self {
            Self::Owner(co) => co.owner_of(rel_path).unwrap_or(UNOWNED_LABEL).to_string(),
            Self::Directory => codeowners::directory_group(rel_path).to_string(),
            Self::Package(pr) => pr.resolve(rel_path).to_string(),
        }
    }

    /// Resolve the group key and matching rule for a path.
    ///
    /// Returns `(owner, Some(pattern))` for Owner mode,
    /// `(directory, None)` for Directory/Package mode.
    pub fn resolve_with_rule(&self, rel_path: &Path) -> (String, Option<String>) {
        match self {
            Self::Owner(co) => {
                if let Some((owner, rule)) = co.owner_and_rule_of(rel_path) {
                    (owner.to_string(), Some(rule.to_string()))
                } else {
                    (UNOWNED_LABEL.to_string(), None)
                }
            }
            Self::Directory => (codeowners::directory_group(rel_path).to_string(), None),
            Self::Package(pr) => (pr.resolve(rel_path).to_string(), None),
        }
    }

    /// Label for the grouping mode (used in JSON `grouped_by` field).
    pub fn mode_label(&self) -> &'static str {
        match self {
            Self::Owner(_) => "owner",
            Self::Directory => "directory",
            Self::Package(_) => "package",
        }
    }
}

/// A single group: a label and its subset of results.
pub struct ResultGroup {
    /// Group label (owner name or directory).
    pub key: String,
    /// Issues belonging to this group.
    pub results: AnalysisResults,
}

/// Partition analysis results into groups by ownership or directory.
///
/// Each issue is assigned to a group by extracting its primary file path
/// and resolving the group key via the `OwnershipResolver`.
/// Returns groups sorted alphabetically by key, with `(unowned)` last.
pub fn group_analysis_results(
    results: &AnalysisResults,
    root: &Path,
    resolver: &OwnershipResolver,
) -> Vec<ResultGroup> {
    let mut groups: FxHashMap<String, AnalysisResults> = FxHashMap::default();

    let key_for = |path: &Path| -> String { resolver.resolve(relative_path(path, root)) };

    // ── File-scoped issue types ─────────────────────────────────
    for item in &results.unused_files {
        groups
            .entry(key_for(&item.path))
            .or_default()
            .unused_files
            .push(item.clone());
    }
    for item in &results.unused_exports {
        groups
            .entry(key_for(&item.path))
            .or_default()
            .unused_exports
            .push(item.clone());
    }
    for item in &results.unused_types {
        groups
            .entry(key_for(&item.path))
            .or_default()
            .unused_types
            .push(item.clone());
    }
    for item in &results.unused_enum_members {
        groups
            .entry(key_for(&item.path))
            .or_default()
            .unused_enum_members
            .push(item.clone());
    }
    for item in &results.unused_class_members {
        groups
            .entry(key_for(&item.path))
            .or_default()
            .unused_class_members
            .push(item.clone());
    }
    for item in &results.unresolved_imports {
        groups
            .entry(key_for(&item.path))
            .or_default()
            .unresolved_imports
            .push(item.clone());
    }

    // ── Dependency-scoped (use package.json path) ───────────────
    for item in &results.unused_dependencies {
        groups
            .entry(key_for(&item.path))
            .or_default()
            .unused_dependencies
            .push(item.clone());
    }
    for item in &results.unused_dev_dependencies {
        groups
            .entry(key_for(&item.path))
            .or_default()
            .unused_dev_dependencies
            .push(item.clone());
    }
    for item in &results.unused_optional_dependencies {
        groups
            .entry(key_for(&item.path))
            .or_default()
            .unused_optional_dependencies
            .push(item.clone());
    }
    for item in &results.type_only_dependencies {
        groups
            .entry(key_for(&item.path))
            .or_default()
            .type_only_dependencies
            .push(item.clone());
    }
    for item in &results.test_only_dependencies {
        groups
            .entry(key_for(&item.path))
            .or_default()
            .test_only_dependencies
            .push(item.clone());
    }

    // ── Multi-location types (use first location) ───────────────
    for item in &results.unlisted_dependencies {
        let key = item
            .imported_from
            .first()
            .map_or_else(|| UNOWNED_LABEL.to_string(), |site| key_for(&site.path));
        groups
            .entry(key)
            .or_default()
            .unlisted_dependencies
            .push(item.clone());
    }
    for item in &results.duplicate_exports {
        let key = item
            .locations
            .first()
            .map_or_else(|| UNOWNED_LABEL.to_string(), |loc| key_for(&loc.path));
        groups
            .entry(key)
            .or_default()
            .duplicate_exports
            .push(item.clone());
    }
    for item in &results.circular_dependencies {
        let key = item
            .files
            .first()
            .map_or_else(|| UNOWNED_LABEL.to_string(), |f| key_for(f));
        groups
            .entry(key)
            .or_default()
            .circular_dependencies
            .push(item.clone());
    }
    for item in &results.boundary_violations {
        groups
            .entry(key_for(&item.from_path))
            .or_default()
            .boundary_violations
            .push(item.clone());
    }

    // ── Sort: most issues first, alphabetical tiebreaker, (unowned) last
    let mut sorted: Vec<_> = groups
        .into_iter()
        .map(|(key, results)| ResultGroup { key, results })
        .collect();
    sorted.sort_by(|a, b| {
        let a_unowned = a.key == UNOWNED_LABEL;
        let b_unowned = b.key == UNOWNED_LABEL;
        match (a_unowned, b_unowned) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => b
                .results
                .total_issues()
                .cmp(&a.results.total_issues())
                .then_with(|| a.key.cmp(&b.key)),
        }
    });
    sorted
}

/// Resolve the group key for a single path (for per-result tagging in SARIF/CodeClimate).
pub fn resolve_owner(path: &Path, root: &Path, resolver: &OwnershipResolver) -> String {
    resolver.resolve(relative_path(path, root))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use fallow_core::results::*;

    use super::*;
    use crate::codeowners::CodeOwners;

    // ── Helpers ────────────────────────────────────────────────────

    fn root() -> PathBuf {
        PathBuf::from("/root")
    }

    fn unused_file(path: &str) -> UnusedFile {
        UnusedFile {
            path: PathBuf::from(path),
        }
    }

    fn unused_export(path: &str, name: &str) -> UnusedExport {
        UnusedExport {
            path: PathBuf::from(path),
            export_name: name.to_string(),
            is_type_only: false,
            line: 1,
            col: 0,
            span_start: 0,
            is_re_export: false,
        }
    }

    fn unlisted_dep(name: &str, sites: Vec<ImportSite>) -> UnlistedDependency {
        UnlistedDependency {
            package_name: name.to_string(),
            imported_from: sites,
        }
    }

    fn import_site(path: &str) -> ImportSite {
        ImportSite {
            path: PathBuf::from(path),
            line: 1,
            col: 0,
        }
    }

    // ── 1. Empty results ──────────────────────────────────────────

    #[test]
    fn empty_results_returns_empty_vec() {
        let results = AnalysisResults::default();
        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);
        assert!(groups.is_empty());
    }

    // ── 2. Single group ──────────────────────────────────────────

    #[test]
    fn single_group_all_same_directory() {
        let mut results = AnalysisResults::default();
        results.unused_files.push(unused_file("/root/src/a.ts"));
        results.unused_files.push(unused_file("/root/src/b.ts"));
        results
            .unused_exports
            .push(unused_export("/root/src/c.ts", "foo"));

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "src");
        assert_eq!(groups[0].results.unused_files.len(), 2);
        assert_eq!(groups[0].results.unused_exports.len(), 1);
        assert_eq!(groups[0].results.total_issues(), 3);
    }

    // ── 3. Multiple groups ───────────────────────────────────────

    #[test]
    fn multiple_groups_split_by_directory() {
        let mut results = AnalysisResults::default();
        results.unused_files.push(unused_file("/root/src/a.ts"));
        results.unused_files.push(unused_file("/root/lib/b.ts"));
        results
            .unused_exports
            .push(unused_export("/root/src/c.ts", "bar"));

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 2);

        let src_group = groups.iter().find(|g| g.key == "src").unwrap();
        let lib_group = groups.iter().find(|g| g.key == "lib").unwrap();

        assert_eq!(src_group.results.total_issues(), 2);
        assert_eq!(lib_group.results.total_issues(), 1);
    }

    // ── 4. Sort order: most issues first ─────────────────────────

    #[test]
    fn sort_order_descending_by_total_issues() {
        let mut results = AnalysisResults::default();
        // lib: 1 issue
        results.unused_files.push(unused_file("/root/lib/a.ts"));
        // src: 3 issues
        results.unused_files.push(unused_file("/root/src/a.ts"));
        results.unused_files.push(unused_file("/root/src/b.ts"));
        results
            .unused_exports
            .push(unused_export("/root/src/c.ts", "x"));
        // test: 2 issues
        results.unused_files.push(unused_file("/root/test/a.ts"));
        results.unused_files.push(unused_file("/root/test/b.ts"));

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].key, "src");
        assert_eq!(groups[0].results.total_issues(), 3);
        assert_eq!(groups[1].key, "test");
        assert_eq!(groups[1].results.total_issues(), 2);
        assert_eq!(groups[2].key, "lib");
        assert_eq!(groups[2].results.total_issues(), 1);
    }

    #[test]
    fn sort_order_alphabetical_tiebreaker() {
        let mut results = AnalysisResults::default();
        results.unused_files.push(unused_file("/root/beta/a.ts"));
        results.unused_files.push(unused_file("/root/alpha/a.ts"));

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 2);
        // Same issue count (1 each) -> alphabetical
        assert_eq!(groups[0].key, "alpha");
        assert_eq!(groups[1].key, "beta");
    }

    // ── 5. Unowned always last ───────────────────────────────────

    #[test]
    fn unowned_sorts_last_regardless_of_count() {
        let mut results = AnalysisResults::default();
        // src: 1 issue
        results.unused_files.push(unused_file("/root/src/a.ts"));
        // unlisted dep with empty imported_from -> goes to (unowned)
        results
            .unlisted_dependencies
            .push(unlisted_dep("pkg-a", vec![]));
        results
            .unlisted_dependencies
            .push(unlisted_dep("pkg-b", vec![]));
        results
            .unlisted_dependencies
            .push(unlisted_dep("pkg-c", vec![]));

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 2);
        // (unowned) has 3 issues vs src's 1, but must still be last
        assert_eq!(groups[0].key, "src");
        assert_eq!(groups[1].key, UNOWNED_LABEL);
        assert_eq!(groups[1].results.total_issues(), 3);
    }

    // ── 6. Multi-location fallback ───────────────────────────────

    #[test]
    fn unlisted_dep_empty_imported_from_goes_to_unowned() {
        let mut results = AnalysisResults::default();
        results
            .unlisted_dependencies
            .push(unlisted_dep("missing-pkg", vec![]));

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, UNOWNED_LABEL);
        assert_eq!(groups[0].results.unlisted_dependencies.len(), 1);
    }

    #[test]
    fn unlisted_dep_with_import_site_goes_to_directory() {
        let mut results = AnalysisResults::default();
        results.unlisted_dependencies.push(unlisted_dep(
            "lodash",
            vec![import_site("/root/src/util.ts")],
        ));

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "src");
        assert_eq!(groups[0].results.unlisted_dependencies.len(), 1);
    }

    // ── 7. Directory mode ────────────────────────────────────────

    #[test]
    fn directory_mode_groups_by_first_path_component() {
        let mut results = AnalysisResults::default();
        results
            .unused_files
            .push(unused_file("/root/packages/ui/Button.ts"));
        results
            .unused_files
            .push(unused_file("/root/packages/auth/login.ts"));
        results
            .unused_exports
            .push(unused_export("/root/apps/web/index.ts", "main"));

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 2);

        let pkgs = groups.iter().find(|g| g.key == "packages").unwrap();
        let apps = groups.iter().find(|g| g.key == "apps").unwrap();

        assert_eq!(pkgs.results.total_issues(), 2);
        assert_eq!(apps.results.total_issues(), 1);
    }

    // ── 8. Owner mode ────────────────────────────────────────────

    #[test]
    fn owner_mode_groups_by_codeowners_owner() {
        let co = CodeOwners::parse("* @default\n/src/ @frontend\n").unwrap();
        let resolver = OwnershipResolver::Owner(co);

        let mut results = AnalysisResults::default();
        results.unused_files.push(unused_file("/root/src/app.ts"));
        results.unused_files.push(unused_file("/root/README.md"));

        let groups = group_analysis_results(&results, &root(), &resolver);

        assert_eq!(groups.len(), 2);

        let frontend = groups.iter().find(|g| g.key == "@frontend").unwrap();
        let default = groups.iter().find(|g| g.key == "@default").unwrap();

        assert_eq!(frontend.results.unused_files.len(), 1);
        assert_eq!(default.results.unused_files.len(), 1);
    }

    #[test]
    fn owner_mode_unmatched_goes_to_unowned() {
        // No catch-all rule -- files outside /src/ have no owner
        let co = CodeOwners::parse("/src/ @frontend\n").unwrap();
        let resolver = OwnershipResolver::Owner(co);

        let mut results = AnalysisResults::default();
        results.unused_files.push(unused_file("/root/README.md"));

        let groups = group_analysis_results(&results, &root(), &resolver);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, UNOWNED_LABEL);
    }

    // ── Boundary violations ──────────────────────────────────────

    #[test]
    fn boundary_violations_grouped_by_from_path() {
        let mut results = AnalysisResults::default();
        results.boundary_violations.push(BoundaryViolation {
            from_path: PathBuf::from("/root/src/bad.ts"),
            to_path: PathBuf::from("/root/lib/secret.ts"),
            from_zone: "src".to_string(),
            to_zone: "lib".to_string(),
            import_specifier: "../lib/secret".to_string(),
            line: 1,
            col: 0,
        });

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "src");
        assert_eq!(groups[0].results.boundary_violations.len(), 1);
    }

    // ── Circular dependencies ────────────────────────────────────

    #[test]
    fn circular_dep_empty_files_goes_to_unowned() {
        let mut results = AnalysisResults::default();
        results.circular_dependencies.push(CircularDependency {
            files: vec![],
            length: 0,
            line: 0,
            col: 0,
            is_cross_package: false,
        });

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, UNOWNED_LABEL);
    }

    #[test]
    fn circular_dep_uses_first_file() {
        let mut results = AnalysisResults::default();
        results.circular_dependencies.push(CircularDependency {
            files: vec![
                PathBuf::from("/root/src/a.ts"),
                PathBuf::from("/root/lib/b.ts"),
            ],
            length: 2,
            line: 1,
            col: 0,
            is_cross_package: false,
        });

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "src");
    }

    // ── Duplicate exports ────────────────────────────────────────

    #[test]
    fn duplicate_exports_empty_locations_goes_to_unowned() {
        let mut results = AnalysisResults::default();
        results.duplicate_exports.push(DuplicateExport {
            export_name: "dup".to_string(),
            locations: vec![],
        });

        let groups = group_analysis_results(&results, &root(), &OwnershipResolver::Directory);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, UNOWNED_LABEL);
    }

    // ── resolve_owner ────────────────────────────────────────────

    #[test]
    fn resolve_owner_returns_directory() {
        let owner = resolve_owner(
            Path::new("/root/src/file.ts"),
            &root(),
            &OwnershipResolver::Directory,
        );
        assert_eq!(owner, "src");
    }

    #[test]
    fn resolve_owner_returns_codeowner() {
        let co = CodeOwners::parse("/src/ @team\n").unwrap();
        let resolver = OwnershipResolver::Owner(co);
        let owner = resolve_owner(Path::new("/root/src/file.ts"), &root(), &resolver);
        assert_eq!(owner, "@team");
    }

    // ── mode_label ───────────────────────────────────────────────

    #[test]
    fn mode_label_owner() {
        let co = CodeOwners::parse("").unwrap();
        let resolver = OwnershipResolver::Owner(co);
        assert_eq!(resolver.mode_label(), "owner");
    }

    #[test]
    fn mode_label_directory() {
        assert_eq!(OwnershipResolver::Directory.mode_label(), "directory");
    }
}
