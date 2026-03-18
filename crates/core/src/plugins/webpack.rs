//! Webpack bundler plugin.
//!
//! Detects Webpack projects and marks conventional entry points and config files.

use std::path::Path;

use super::config_parser;
use super::{Plugin, PluginResult};

pub struct WebpackPlugin;

const ENABLERS: &[&str] = &["webpack"];

const ENTRY_PATTERNS: &[&str] = &["src/index.{ts,tsx,js,jsx}"];

const CONFIG_PATTERNS: &[&str] = &[
    "webpack.config.{ts,js,mjs,cjs}",
    "webpack.*.config.{ts,js,mjs,cjs}",
];

const ALWAYS_USED: &[&str] = &[
    "webpack.config.{ts,js,mjs,cjs}",
    "webpack.*.config.{ts,js,mjs,cjs}",
];

const TOOLING_DEPENDENCIES: &[&str] = &[
    "webpack",
    "webpack-cli",
    "webpack-dev-server",
    "html-webpack-plugin",
];

impl Plugin for WebpackPlugin {
    fn name(&self) -> &'static str {
        "webpack"
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

        // entry → entry points (string, array, or object with string values)
        // e.g. entry: "./src/index.js"
        // e.g. entry: { main: "./src/main.js", vendor: "./src/vendor.js" }
        let entries =
            config_parser::extract_config_string_or_array(source, config_path, &["entry"]);
        result.entry_patterns.extend(entries);

        // require() calls for loaders/plugins in CJS configs
        let require_deps =
            config_parser::extract_config_require_strings(source, config_path, "plugins");
        for dep in &require_deps {
            result
                .referenced_dependencies
                .push(crate::resolve::extract_package_name(dep));
        }

        result
    }
}
