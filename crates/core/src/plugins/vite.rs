//! Vite bundler plugin.
//!
//! Detects Vite projects and marks conventional entry points and config files.

use std::path::Path;

use super::config_parser;
use super::{Plugin, PluginResult};

pub struct VitePlugin;

const ENABLERS: &[&str] = &["vite"];

const ENTRY_PATTERNS: &[&str] = &[
    "src/main.{ts,tsx,js,jsx}",
    "src/index.{ts,tsx,js,jsx}",
    "index.html",
];

const CONFIG_PATTERNS: &[&str] = &["vite.config.{ts,js,mts,mjs}"];

const ALWAYS_USED: &[&str] = &["vite.config.{ts,js,mts,mjs}"];

const TOOLING_DEPENDENCIES: &[&str] = &["vite", "@vitejs/plugin-react", "@vitejs/plugin-vue"];

impl Plugin for VitePlugin {
    fn name(&self) -> &'static str {
        "vite"
    }

    fn enablers(&self) -> &'static [&'static str] {
        ENABLERS
    }

    fn entry_patterns(&self) -> &'static [&'static str] {
        ENTRY_PATTERNS
    }

    fn config_patterns(&self) -> &'static [&'static str] {
        CONFIG_PATTERNS
    }

    fn always_used(&self) -> &'static [&'static str] {
        ALWAYS_USED
    }

    fn tooling_dependencies(&self) -> &'static [&'static str] {
        TOOLING_DEPENDENCIES
    }

    fn resolve_config(&self, config_path: &Path, source: &str, _root: &Path) -> PluginResult {
        let mut result = PluginResult::default();

        let imports = config_parser::extract_imports(source, config_path);
        for imp in &imports {
            let dep = crate::resolve::extract_package_name(imp);
            result.referenced_dependencies.push(dep);
        }

        // build.rollupOptions.input → entry points (string, array, or object)
        let rollup_input = config_parser::extract_config_string_or_array(
            source,
            config_path,
            &["build", "rollupOptions", "input"],
        );
        result.entry_patterns.extend(rollup_input);

        // build.lib.entry → entry points (string or array)
        let lib_entry = config_parser::extract_config_string_or_array(
            source,
            config_path,
            &["build", "lib", "entry"],
        );
        result.entry_patterns.extend(lib_entry);

        // optimizeDeps.include → referenced dependencies
        let optimize_include = config_parser::extract_config_string_array(
            source,
            config_path,
            &["optimizeDeps", "include"],
        );
        for dep in &optimize_include {
            result
                .referenced_dependencies
                .push(crate::resolve::extract_package_name(dep));
        }

        result
    }
}
