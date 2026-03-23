mod entry_points;
mod infrastructure;
mod parse_scripts;
mod walk;

// Re-export types from fallow-types
pub use fallow_types::discover::{DiscoveredFile, EntryPoint, EntryPointSource, FileId};

// Re-export public functions — preserves the existing `crate::discover::*` API
pub use entry_points::{
    compile_glob_set, discover_entry_points, discover_plugin_entry_points,
    discover_workspace_entry_points,
};
pub use infrastructure::discover_infrastructure_entry_points;
pub use walk::{SOURCE_EXTENSIONS, discover_files};

/// Hidden (dot-prefixed) directories that should be included in file discovery.
///
/// Most hidden directories (`.git`, `.cache`, etc.) should be skipped, but certain
/// convention directories contain source or config files that fallow needs to see:
/// - `.storybook` — Storybook configuration (the Storybook plugin depends on this)
/// - `.well-known` — Standard web convention directory
/// - `.changeset` — Changesets configuration
/// - `.github` — GitHub workflows and CI scripts
const ALLOWED_HIDDEN_DIRS: &[&str] = &[".storybook", ".well-known", ".changeset", ".github"];
