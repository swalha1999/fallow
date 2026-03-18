//! Tailwind CSS plugin.
//!
//! Detects Tailwind projects and marks config files as always used.
//! Parses tailwind.config to extract content globs and plugin dependencies.

use std::path::Path;

use super::config_parser;
use super::{Plugin, PluginResult};

pub struct TailwindPlugin;

const ENABLERS: &[&str] = &["tailwindcss", "@tailwindcss/postcss"];

const CONFIG_PATTERNS: &[&str] = &["tailwind.config.{ts,js,cjs,mjs}"];

const ALWAYS_USED: &[&str] = &["tailwind.config.{ts,js,cjs,mjs}"];

const TOOLING_DEPENDENCIES: &[&str] = &[
    "tailwindcss",
    "@tailwindcss/postcss",
    "@tailwindcss/typography",
    "@tailwindcss/forms",
    "@tailwindcss/aspect-ratio",
    "@tailwindcss/container-queries",
    "autoprefixer",
];

impl Plugin for TailwindPlugin {
    fn name(&self) -> &'static str {
        "tailwind"
    }

    fn enablers(&self) -> &'static [&'static str] {
        ENABLERS
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

        // Extract import sources as referenced dependencies
        let imports = config_parser::extract_imports(source, config_path);
        for imp in &imports {
            let dep = crate::resolve::extract_package_name(imp);
            result.referenced_dependencies.push(dep);
        }

        // content → file globs that Tailwind scans for class usage
        // e.g. content: ["./src/**/*.{js,ts,jsx,tsx}", "./index.html"]
        let content = config_parser::extract_config_string_array(source, config_path, &["content"]);
        result.always_used_files.extend(content);

        // plugins as require() calls: plugins: [require("@tailwindcss/typography")]
        let require_deps =
            config_parser::extract_config_require_strings(source, config_path, "plugins");
        for dep in &require_deps {
            result
                .referenced_dependencies
                .push(crate::resolve::extract_package_name(dep));
        }

        // plugins as shallow strings (less common): plugins: ["@tailwindcss/typography"]
        let plugin_strings =
            config_parser::extract_config_shallow_strings(source, config_path, "plugins");
        for plugin in &plugin_strings {
            result
                .referenced_dependencies
                .push(crate::resolve::extract_package_name(plugin));
        }

        // presets → referenced dependencies
        let presets = config_parser::extract_config_shallow_strings(source, config_path, "presets");
        for preset in &presets {
            result
                .referenced_dependencies
                .push(crate::resolve::extract_package_name(preset));
        }

        result
    }
}
