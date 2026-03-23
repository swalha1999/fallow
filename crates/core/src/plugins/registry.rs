//! Plugin registry: discovers active plugins, collects patterns, parses configs.

use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

use fallow_config::{ExternalPluginDef, PackageJson, PluginDetection};

use super::{Plugin, PluginResult};

// Import all plugin structs
use super::angular::AngularPlugin;
use super::astro::AstroPlugin;
use super::ava::AvaPlugin;
use super::babel::BabelPlugin;
use super::biome::BiomePlugin;
use super::bun::BunPlugin;
use super::c8::C8Plugin;
use super::capacitor::CapacitorPlugin;
use super::changesets::ChangesetsPlugin;
use super::commitizen::CommitizenPlugin;
use super::commitlint::CommitlintPlugin;
use super::cspell::CspellPlugin;
use super::cucumber::CucumberPlugin;
use super::cypress::CypressPlugin;
use super::dependency_cruiser::DependencyCruiserPlugin;
use super::docusaurus::DocusaurusPlugin;
use super::drizzle::DrizzlePlugin;
use super::electron::ElectronPlugin;
use super::eslint::EslintPlugin;
use super::expo::ExpoPlugin;
use super::gatsby::GatsbyPlugin;
use super::graphql_codegen::GraphqlCodegenPlugin;
use super::husky::HuskyPlugin;
use super::i18next::I18nextPlugin;
use super::jest::JestPlugin;
use super::karma::KarmaPlugin;
use super::knex::KnexPlugin;
use super::kysely::KyselyPlugin;
use super::lefthook::LefthookPlugin;
use super::lint_staged::LintStagedPlugin;
use super::markdownlint::MarkdownlintPlugin;
use super::mocha::MochaPlugin;
use super::msw::MswPlugin;
use super::nestjs::NestJsPlugin;
use super::next_intl::NextIntlPlugin;
use super::nextjs::NextJsPlugin;
use super::nitro::NitroPlugin;
use super::nodemon::NodemonPlugin;
use super::nuxt::NuxtPlugin;
use super::nx::NxPlugin;
use super::nyc::NycPlugin;
use super::openapi_ts::OpenapiTsPlugin;
use super::oxlint::OxlintPlugin;
use super::parcel::ParcelPlugin;
use super::playwright::PlaywrightPlugin;
use super::plop::PlopPlugin;
use super::pm2::Pm2Plugin;
use super::postcss::PostCssPlugin;
use super::prettier::PrettierPlugin;
use super::prisma::PrismaPlugin;
use super::react_native::ReactNativePlugin;
use super::react_router::ReactRouterPlugin;
use super::relay::RelayPlugin;
use super::remark::RemarkPlugin;
use super::remix::RemixPlugin;
use super::rolldown::RolldownPlugin;
use super::rollup::RollupPlugin;
use super::rsbuild::RsbuildPlugin;
use super::rspack::RspackPlugin;
use super::sanity::SanityPlugin;
use super::semantic_release::SemanticReleasePlugin;
use super::sentry::SentryPlugin;
use super::simple_git_hooks::SimpleGitHooksPlugin;
use super::storybook::StorybookPlugin;
use super::stylelint::StylelintPlugin;
use super::sveltekit::SvelteKitPlugin;
use super::svgo::SvgoPlugin;
use super::svgr::SvgrPlugin;
use super::swc::SwcPlugin;
use super::syncpack::SyncpackPlugin;
use super::tailwind::TailwindPlugin;
use super::tanstack_router::TanstackRouterPlugin;
use super::tsdown::TsdownPlugin;
use super::tsup::TsupPlugin;
use super::turborepo::TurborepoPlugin;
use super::typedoc::TypedocPlugin;
use super::typeorm::TypeormPlugin;
use super::typescript::TypeScriptPlugin;
use super::vite::VitePlugin;
use super::vitepress::VitePressPlugin;
use super::vitest::VitestPlugin;
use super::webdriverio::WebdriverioPlugin;
use super::webpack::WebpackPlugin;
use super::wrangler::WranglerPlugin;

/// Registry of all available plugins (built-in + external).
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
    external_plugins: Vec<ExternalPluginDef>,
}

/// Aggregated results from all active plugins for a project.
#[derive(Debug, Default)]
pub struct AggregatedPluginResult {
    /// All entry point patterns from active plugins: (pattern, plugin_name).
    pub entry_patterns: Vec<(String, String)>,
    /// All config file patterns from active plugins.
    pub config_patterns: Vec<String>,
    /// All always-used file patterns from active plugins: (pattern, plugin_name).
    pub always_used: Vec<(String, String)>,
    /// All used export rules from active plugins.
    pub used_exports: Vec<(String, Vec<String>)>,
    /// Dependencies referenced in config files (should not be flagged unused).
    pub referenced_dependencies: Vec<String>,
    /// Additional always-used files discovered from config parsing: (pattern, plugin_name).
    pub discovered_always_used: Vec<(String, String)>,
    /// Setup files discovered from config parsing: (path, plugin_name).
    pub setup_files: Vec<(PathBuf, String)>,
    /// Tooling dependencies (should not be flagged as unused devDeps).
    pub tooling_dependencies: Vec<String>,
    /// Package names discovered as used in package.json scripts (binary invocations).
    pub script_used_packages: FxHashSet<String>,
    /// Import prefixes for virtual modules provided by active frameworks.
    /// Imports matching these prefixes should not be flagged as unlisted dependencies.
    pub virtual_module_prefixes: Vec<String>,
    /// Path alias mappings from active plugins (prefix → replacement directory).
    /// Used by the resolver to substitute import prefixes before re-resolving.
    pub path_aliases: Vec<(String, String)>,
    /// Names of active plugins.
    pub active_plugins: Vec<String>,
}

impl PluginRegistry {
    /// Create a registry with all built-in plugins and optional external plugins.
    pub fn new(external: Vec<ExternalPluginDef>) -> Self {
        let plugins: Vec<Box<dyn Plugin>> = vec![
            // Frameworks
            Box::new(NextJsPlugin),
            Box::new(NuxtPlugin),
            Box::new(RemixPlugin),
            Box::new(AstroPlugin),
            Box::new(AngularPlugin),
            Box::new(ReactRouterPlugin),
            Box::new(TanstackRouterPlugin),
            Box::new(ReactNativePlugin),
            Box::new(ExpoPlugin),
            Box::new(NestJsPlugin),
            Box::new(DocusaurusPlugin),
            Box::new(GatsbyPlugin),
            Box::new(SvelteKitPlugin),
            Box::new(NitroPlugin),
            Box::new(CapacitorPlugin),
            Box::new(SanityPlugin),
            Box::new(VitePressPlugin),
            Box::new(NextIntlPlugin),
            Box::new(RelayPlugin),
            Box::new(ElectronPlugin),
            Box::new(I18nextPlugin),
            // Bundlers
            Box::new(VitePlugin),
            Box::new(WebpackPlugin),
            Box::new(RollupPlugin),
            Box::new(RolldownPlugin),
            Box::new(RspackPlugin),
            Box::new(RsbuildPlugin),
            Box::new(TsupPlugin),
            Box::new(TsdownPlugin),
            Box::new(ParcelPlugin),
            // Testing
            Box::new(VitestPlugin),
            Box::new(JestPlugin),
            Box::new(PlaywrightPlugin),
            Box::new(CypressPlugin),
            Box::new(MochaPlugin),
            Box::new(AvaPlugin),
            Box::new(StorybookPlugin),
            Box::new(KarmaPlugin),
            Box::new(CucumberPlugin),
            Box::new(WebdriverioPlugin),
            // Linting & formatting
            Box::new(EslintPlugin),
            Box::new(BiomePlugin),
            Box::new(StylelintPlugin),
            Box::new(PrettierPlugin),
            Box::new(OxlintPlugin),
            Box::new(MarkdownlintPlugin),
            Box::new(CspellPlugin),
            Box::new(RemarkPlugin),
            // Transpilation & language
            Box::new(TypeScriptPlugin),
            Box::new(BabelPlugin),
            Box::new(SwcPlugin),
            // CSS
            Box::new(TailwindPlugin),
            Box::new(PostCssPlugin),
            // Database & ORM
            Box::new(PrismaPlugin),
            Box::new(DrizzlePlugin),
            Box::new(KnexPlugin),
            Box::new(TypeormPlugin),
            Box::new(KyselyPlugin),
            // Monorepo
            Box::new(TurborepoPlugin),
            Box::new(NxPlugin),
            Box::new(ChangesetsPlugin),
            Box::new(SyncpackPlugin),
            // CI/CD & release
            Box::new(CommitlintPlugin),
            Box::new(CommitizenPlugin),
            Box::new(SemanticReleasePlugin),
            // Deployment
            Box::new(WranglerPlugin),
            Box::new(SentryPlugin),
            // Git hooks
            Box::new(HuskyPlugin),
            Box::new(LintStagedPlugin),
            Box::new(LefthookPlugin),
            Box::new(SimpleGitHooksPlugin),
            // Media & assets
            Box::new(SvgoPlugin),
            Box::new(SvgrPlugin),
            // Code generation & docs
            Box::new(GraphqlCodegenPlugin),
            Box::new(TypedocPlugin),
            Box::new(OpenapiTsPlugin),
            Box::new(PlopPlugin),
            // Coverage
            Box::new(C8Plugin),
            Box::new(NycPlugin),
            // Other tools
            Box::new(MswPlugin),
            Box::new(NodemonPlugin),
            Box::new(Pm2Plugin),
            Box::new(DependencyCruiserPlugin),
            // Runtime
            Box::new(BunPlugin),
        ];
        Self {
            plugins,
            external_plugins: external,
        }
    }

    /// Run all plugins against a project, returning aggregated results.
    ///
    /// This discovers which plugins are active, collects their static patterns,
    /// then parses any config files to extract dynamic information.
    pub fn run(
        &self,
        pkg: &PackageJson,
        root: &Path,
        discovered_files: &[PathBuf],
    ) -> AggregatedPluginResult {
        let _span = tracing::info_span!("run_plugins").entered();
        let mut result = AggregatedPluginResult::default();

        // Phase 1: Determine which plugins are active
        // Compute deps once to avoid repeated Vec<String> allocation per plugin
        let all_deps = pkg.all_dependency_names();
        let active: Vec<&dyn Plugin> = self
            .plugins
            .iter()
            .filter(|p| p.is_enabled_with_deps(&all_deps, root))
            .map(|p| p.as_ref())
            .collect();

        tracing::info!(
            plugins = active
                .iter()
                .map(|p| p.name())
                .collect::<Vec<_>>()
                .join(", "),
            "active plugins"
        );

        // Phase 2: Collect static patterns from active plugins
        for plugin in &active {
            process_static_patterns(*plugin, root, &mut result);
        }

        // Phase 2b: Process external plugins (includes inline framework definitions)
        process_external_plugins(
            &self.external_plugins,
            &all_deps,
            root,
            discovered_files,
            &mut result,
        );

        // Phase 3: Find and parse config files for dynamic resolution
        // Pre-compile all config patterns
        let config_matchers: Vec<(&dyn Plugin, Vec<globset::GlobMatcher>)> = active
            .iter()
            .filter(|p| !p.config_patterns().is_empty())
            .map(|p| {
                let matchers: Vec<globset::GlobMatcher> = p
                    .config_patterns()
                    .iter()
                    .filter_map(|pat| globset::Glob::new(pat).ok().map(|g| g.compile_matcher()))
                    .collect();
                (*p, matchers)
            })
            .collect();

        // Build relative paths for matching (used by Phase 3 and 4)
        let relative_files: Vec<(&PathBuf, String)> = discovered_files
            .iter()
            .map(|f| {
                let rel = f
                    .strip_prefix(root)
                    .unwrap_or(f)
                    .to_string_lossy()
                    .into_owned();
                (f, rel)
            })
            .collect();

        if !config_matchers.is_empty() {
            // Phase 3a: Match config files from discovered source files
            let mut resolved_plugins: FxHashSet<&str> = FxHashSet::default();

            for (plugin, matchers) in &config_matchers {
                for (abs_path, rel_path) in &relative_files {
                    if matchers.iter().any(|m| m.is_match(rel_path.as_str())) {
                        // Mark as resolved regardless of result to prevent Phase 3b
                        // from re-parsing a JSON config for the same plugin.
                        resolved_plugins.insert(plugin.name());
                        if let Ok(source) = std::fs::read_to_string(abs_path) {
                            let plugin_result = plugin.resolve_config(abs_path, &source, root);
                            if !plugin_result.is_empty() {
                                tracing::debug!(
                                    plugin = plugin.name(),
                                    config = rel_path.as_str(),
                                    entries = plugin_result.entry_patterns.len(),
                                    deps = plugin_result.referenced_dependencies.len(),
                                    "resolved config"
                                );
                                process_config_result(plugin.name(), plugin_result, &mut result);
                            }
                        }
                    }
                }
            }

            // Phase 3b: Filesystem fallback for JSON config files.
            // JSON files (angular.json, project.json) are not in the discovered file set
            // because fallow only discovers JS/TS/CSS/Vue/etc. files.
            let json_configs = discover_json_config_files(
                &config_matchers,
                &resolved_plugins,
                &relative_files,
                root,
            );
            for (abs_path, plugin) in &json_configs {
                if let Ok(source) = std::fs::read_to_string(abs_path) {
                    let plugin_result = plugin.resolve_config(abs_path, &source, root);
                    if !plugin_result.is_empty() {
                        let rel = abs_path
                            .strip_prefix(root)
                            .map(|p| p.to_string_lossy())
                            .unwrap_or_default();
                        tracing::debug!(
                            plugin = plugin.name(),
                            config = %rel,
                            entries = plugin_result.entry_patterns.len(),
                            deps = plugin_result.referenced_dependencies.len(),
                            "resolved config (filesystem fallback)"
                        );
                        process_config_result(plugin.name(), plugin_result, &mut result);
                    }
                }
            }
        }

        // Phase 4: Package.json inline config fallback
        // For plugins that define `package_json_config_key()`, check if the root
        // package.json contains that key and no standalone config file was found.
        for plugin in &active {
            if let Some(key) = plugin.package_json_config_key()
                && !check_has_config_file(*plugin, &config_matchers, &relative_files)
            {
                // Try to extract the key from package.json
                let pkg_path = root.join("package.json");
                if let Ok(content) = std::fs::read_to_string(&pkg_path)
                    && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
                    && let Some(config_value) = json.get(key)
                {
                    let config_json = serde_json::to_string(config_value).unwrap_or_default();
                    let fake_path = root.join(format!("{key}.config.json"));
                    let plugin_result = plugin.resolve_config(&fake_path, &config_json, root);
                    if !plugin_result.is_empty() {
                        tracing::debug!(
                            plugin = plugin.name(),
                            key = key,
                            "resolved inline package.json config"
                        );
                        process_config_result(plugin.name(), plugin_result, &mut result);
                    }
                }
            }
        }

        result
    }

    /// Fast variant of `run()` for workspace packages.
    ///
    /// Reuses pre-compiled config matchers and pre-computed relative files from the root
    /// project run, avoiding repeated glob compilation and path computation per workspace.
    /// Skips external plugins (they only activate at root level) and package.json inline
    /// config (workspace packages rarely have inline configs).
    pub fn run_workspace_fast(
        &self,
        pkg: &PackageJson,
        root: &Path,
        project_root: &Path,
        precompiled_config_matchers: &[(&dyn Plugin, Vec<globset::GlobMatcher>)],
        relative_files: &[(&PathBuf, String)],
    ) -> AggregatedPluginResult {
        let _span = tracing::info_span!("run_plugins").entered();
        let mut result = AggregatedPluginResult::default();

        // Phase 1: Determine which plugins are active (with pre-computed deps)
        let all_deps = pkg.all_dependency_names();
        let active: Vec<&dyn Plugin> = self
            .plugins
            .iter()
            .filter(|p| p.is_enabled_with_deps(&all_deps, root))
            .map(|p| p.as_ref())
            .collect();

        tracing::info!(
            plugins = active
                .iter()
                .map(|p| p.name())
                .collect::<Vec<_>>()
                .join(", "),
            "active plugins"
        );

        // Early exit if no plugins are active (common for leaf workspace packages)
        if active.is_empty() {
            return result;
        }

        // Phase 2: Collect static patterns from active plugins
        for plugin in &active {
            process_static_patterns(*plugin, root, &mut result);
        }

        // Phase 3: Find and parse config files using pre-compiled matchers
        // Only check matchers for plugins that are active in this workspace
        let active_names: FxHashSet<&str> = active.iter().map(|p| p.name()).collect();
        let workspace_matchers: Vec<_> = precompiled_config_matchers
            .iter()
            .filter(|(p, _)| active_names.contains(p.name()))
            .collect();

        let mut resolved_ws_plugins: FxHashSet<&str> = FxHashSet::default();
        if !workspace_matchers.is_empty() {
            for (plugin, matchers) in &workspace_matchers {
                for (abs_path, rel_path) in relative_files {
                    if matchers.iter().any(|m| m.is_match(rel_path.as_str()))
                        && let Ok(source) = std::fs::read_to_string(abs_path)
                    {
                        // Mark resolved regardless of result to prevent Phase 3b
                        // from re-parsing a JSON config for the same plugin.
                        resolved_ws_plugins.insert(plugin.name());
                        let plugin_result = plugin.resolve_config(abs_path, &source, root);
                        if !plugin_result.is_empty() {
                            tracing::debug!(
                                plugin = plugin.name(),
                                config = rel_path.as_str(),
                                entries = plugin_result.entry_patterns.len(),
                                deps = plugin_result.referenced_dependencies.len(),
                                "resolved config"
                            );
                            process_config_result(plugin.name(), plugin_result, &mut result);
                        }
                    }
                }
            }
        }

        // Phase 3b: Filesystem fallback for JSON config files at the project root.
        // Config files like angular.json live at the monorepo root, but Angular is
        // only active in workspace packages. Check the project root for unresolved
        // config patterns.
        let mut ws_json_configs: Vec<(PathBuf, &dyn Plugin)> = Vec::new();
        let mut ws_seen_paths: FxHashSet<PathBuf> = FxHashSet::default();
        for plugin in &active {
            if resolved_ws_plugins.contains(plugin.name()) || plugin.config_patterns().is_empty() {
                continue;
            }
            for pat in plugin.config_patterns() {
                let has_glob = pat.contains("**") || pat.contains('*') || pat.contains('?');
                if !has_glob {
                    // Check both workspace root and project root (deduplicate when equal)
                    let check_roots: Vec<&Path> = if root == project_root {
                        vec![root]
                    } else {
                        vec![root, project_root]
                    };
                    for check_root in check_roots {
                        let abs_path = check_root.join(pat);
                        if abs_path.is_file() && ws_seen_paths.insert(abs_path.clone()) {
                            ws_json_configs.push((abs_path, *plugin));
                            break; // Found it — don't check other roots for this pattern
                        }
                    }
                } else {
                    // Glob pattern (e.g., "**/project.json") — check directories
                    // that contain discovered source files
                    let filename = std::path::Path::new(pat)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(pat);
                    let matcher = globset::Glob::new(pat).ok().map(|g| g.compile_matcher());
                    if let Some(matcher) = matcher {
                        let mut checked_dirs: FxHashSet<&Path> = FxHashSet::default();
                        checked_dirs.insert(root);
                        if root != project_root {
                            checked_dirs.insert(project_root);
                        }
                        for (abs_path, _) in relative_files {
                            if let Some(parent) = abs_path.parent() {
                                checked_dirs.insert(parent);
                            }
                        }
                        for dir in checked_dirs {
                            let candidate = dir.join(filename);
                            if candidate.is_file() && ws_seen_paths.insert(candidate.clone()) {
                                let rel = candidate
                                    .strip_prefix(project_root)
                                    .map(|p| p.to_string_lossy())
                                    .unwrap_or_default();
                                if matcher.is_match(rel.as_ref()) {
                                    ws_json_configs.push((candidate, *plugin));
                                }
                            }
                        }
                    }
                }
            }
        }
        // Parse discovered JSON config files
        for (abs_path, plugin) in &ws_json_configs {
            if let Ok(source) = std::fs::read_to_string(abs_path) {
                let plugin_result = plugin.resolve_config(abs_path, &source, root);
                if !plugin_result.is_empty() {
                    let rel = abs_path
                        .strip_prefix(project_root)
                        .map(|p| p.to_string_lossy())
                        .unwrap_or_default();
                    tracing::debug!(
                        plugin = plugin.name(),
                        config = %rel,
                        entries = plugin_result.entry_patterns.len(),
                        deps = plugin_result.referenced_dependencies.len(),
                        "resolved config (workspace filesystem fallback)"
                    );
                    process_config_result(plugin.name(), plugin_result, &mut result);
                }
            }
        }

        result
    }

    /// Pre-compile config pattern glob matchers for all plugins that have config patterns.
    /// Returns a vec of (plugin, matchers) pairs that can be reused across multiple `run_workspace_fast` calls.
    pub fn precompile_config_matchers(&self) -> Vec<(&dyn Plugin, Vec<globset::GlobMatcher>)> {
        self.plugins
            .iter()
            .filter(|p| !p.config_patterns().is_empty())
            .map(|p| {
                let matchers: Vec<globset::GlobMatcher> = p
                    .config_patterns()
                    .iter()
                    .filter_map(|pat| globset::Glob::new(pat).ok().map(|g| g.compile_matcher()))
                    .collect();
                (p.as_ref(), matchers)
            })
            .collect()
    }
}

/// Collect static patterns from a single plugin into the aggregated result.
fn process_static_patterns(plugin: &dyn Plugin, root: &Path, result: &mut AggregatedPluginResult) {
    result.active_plugins.push(plugin.name().to_string());

    let pname = plugin.name().to_string();
    for pat in plugin.entry_patterns() {
        result
            .entry_patterns
            .push(((*pat).to_string(), pname.clone()));
    }
    for pat in plugin.config_patterns() {
        result.config_patterns.push((*pat).to_string());
    }
    for pat in plugin.always_used() {
        result.always_used.push(((*pat).to_string(), pname.clone()));
    }
    for (file_pat, exports) in plugin.used_exports() {
        result.used_exports.push((
            file_pat.to_string(),
            exports.iter().map(|s| s.to_string()).collect(),
        ));
    }
    for dep in plugin.tooling_dependencies() {
        result.tooling_dependencies.push((*dep).to_string());
    }
    for prefix in plugin.virtual_module_prefixes() {
        result.virtual_module_prefixes.push((*prefix).to_string());
    }
    for (prefix, replacement) in plugin.path_aliases(root) {
        result.path_aliases.push((prefix.to_string(), replacement));
    }
}

/// Process external plugin definitions, checking activation and aggregating patterns.
fn process_external_plugins(
    external_plugins: &[ExternalPluginDef],
    all_deps: &[String],
    root: &Path,
    discovered_files: &[PathBuf],
    result: &mut AggregatedPluginResult,
) {
    let all_dep_refs: Vec<&str> = all_deps.iter().map(|s| s.as_str()).collect();
    for ext in external_plugins {
        let is_active = if let Some(detection) = &ext.detection {
            check_plugin_detection(detection, &all_dep_refs, root, discovered_files)
        } else if !ext.enablers.is_empty() {
            ext.enablers.iter().any(|enabler| {
                if enabler.ends_with('/') {
                    all_deps.iter().any(|d| d.starts_with(enabler))
                } else {
                    all_deps.iter().any(|d| d == enabler)
                }
            })
        } else {
            false
        };
        if is_active {
            result.active_plugins.push(ext.name.clone());
            result.entry_patterns.extend(
                ext.entry_points
                    .iter()
                    .map(|p| (p.clone(), ext.name.clone())),
            );
            // Track config patterns for introspection (not used for AST parsing —
            // external plugins cannot do resolve_config())
            result.config_patterns.extend(ext.config_patterns.clone());
            result.always_used.extend(
                ext.config_patterns
                    .iter()
                    .chain(ext.always_used.iter())
                    .map(|p| (p.clone(), ext.name.clone())),
            );
            result
                .tooling_dependencies
                .extend(ext.tooling_dependencies.clone());
            for ue in &ext.used_exports {
                result
                    .used_exports
                    .push((ue.pattern.clone(), ue.exports.clone()));
            }
        }
    }
}

/// Discover JSON config files on the filesystem for plugins that weren't matched against
/// discovered source files. Returns `(path, plugin)` pairs.
fn discover_json_config_files<'a>(
    config_matchers: &[(&'a dyn Plugin, Vec<globset::GlobMatcher>)],
    resolved_plugins: &FxHashSet<&str>,
    relative_files: &[(&PathBuf, String)],
    root: &Path,
) -> Vec<(PathBuf, &'a dyn Plugin)> {
    let mut json_configs: Vec<(PathBuf, &'a dyn Plugin)> = Vec::new();
    for (plugin, _) in config_matchers {
        if resolved_plugins.contains(plugin.name()) {
            continue;
        }
        for pat in plugin.config_patterns() {
            let has_glob = pat.contains("**") || pat.contains('*') || pat.contains('?');
            if !has_glob {
                // Simple pattern (e.g., "angular.json") — check at root
                let abs_path = root.join(pat);
                if abs_path.is_file() {
                    json_configs.push((abs_path, *plugin));
                }
            } else {
                // Glob pattern (e.g., "**/project.json") — check directories
                // that contain discovered source files
                let filename = std::path::Path::new(pat)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(pat);
                let matcher = globset::Glob::new(pat).ok().map(|g| g.compile_matcher());
                if let Some(matcher) = matcher {
                    let mut checked_dirs: FxHashSet<&Path> = FxHashSet::default();
                    checked_dirs.insert(root);
                    for (abs_path, _) in relative_files {
                        if let Some(parent) = abs_path.parent() {
                            checked_dirs.insert(parent);
                        }
                    }
                    for dir in checked_dirs {
                        let candidate = dir.join(filename);
                        if candidate.is_file() {
                            let rel = candidate
                                .strip_prefix(root)
                                .map(|p| p.to_string_lossy())
                                .unwrap_or_default();
                            if matcher.is_match(rel.as_ref()) {
                                json_configs.push((candidate, *plugin));
                            }
                        }
                    }
                }
            }
        }
    }
    json_configs
}

/// Merge a `PluginResult` from config parsing into the aggregated result.
fn process_config_result(
    plugin_name: &str,
    plugin_result: PluginResult,
    result: &mut AggregatedPluginResult,
) {
    let pname = plugin_name.to_string();
    result.entry_patterns.extend(
        plugin_result
            .entry_patterns
            .into_iter()
            .map(|p| (p, pname.clone())),
    );
    result
        .referenced_dependencies
        .extend(plugin_result.referenced_dependencies);
    result.discovered_always_used.extend(
        plugin_result
            .always_used_files
            .into_iter()
            .map(|p| (p, pname.clone())),
    );
    result.setup_files.extend(
        plugin_result
            .setup_files
            .into_iter()
            .map(|p| (p, pname.clone())),
    );
}

/// Check if a plugin already has a config file matched against discovered files.
fn check_has_config_file(
    plugin: &dyn Plugin,
    config_matchers: &[(&dyn Plugin, Vec<globset::GlobMatcher>)],
    relative_files: &[(&PathBuf, String)],
) -> bool {
    !plugin.config_patterns().is_empty()
        && config_matchers.iter().any(|(p, matchers)| {
            p.name() == plugin.name()
                && relative_files
                    .iter()
                    .any(|(_, rel)| matchers.iter().any(|m| m.is_match(rel.as_str())))
        })
}

/// Check if a `PluginDetection` condition is satisfied.
fn check_plugin_detection(
    detection: &PluginDetection,
    all_deps: &[&str],
    root: &Path,
    discovered_files: &[PathBuf],
) -> bool {
    match detection {
        PluginDetection::Dependency { package } => all_deps.iter().any(|d| *d == package),
        PluginDetection::FileExists { pattern } => {
            // Check against discovered files first (fast path)
            if let Ok(matcher) = globset::Glob::new(pattern).map(|g| g.compile_matcher()) {
                for file in discovered_files {
                    let relative = file.strip_prefix(root).unwrap_or(file);
                    if matcher.is_match(relative) {
                        return true;
                    }
                }
            }
            // Fall back to glob on disk for non-source files (e.g., config files)
            let full_pattern = root.join(pattern).to_string_lossy().to_string();
            glob::glob(&full_pattern)
                .ok()
                .is_some_and(|mut g| g.next().is_some())
        }
        PluginDetection::All { conditions } => conditions
            .iter()
            .all(|c| check_plugin_detection(c, all_deps, root, discovered_files)),
        PluginDetection::Any { conditions } => conditions
            .iter()
            .any(|c| check_plugin_detection(c, all_deps, root, discovered_files)),
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new(vec![])
    }
}

#[cfg(test)]
#[expect(clippy::disallowed_types)]
mod tests {
    use super::*;
    use fallow_config::{ExternalPluginDef, ExternalUsedExport, PluginDetection};
    use std::collections::HashMap;

    /// Helper: build a PackageJson with given dependency names.
    fn make_pkg(deps: &[&str]) -> PackageJson {
        let map: HashMap<String, String> =
            deps.iter().map(|d| (d.to_string(), "*".into())).collect();
        PackageJson {
            dependencies: Some(map),
            ..Default::default()
        }
    }

    /// Helper: build a PackageJson with dev dependencies.
    fn make_pkg_dev(deps: &[&str]) -> PackageJson {
        let map: HashMap<String, String> =
            deps.iter().map(|d| (d.to_string(), "*".into())).collect();
        PackageJson {
            dev_dependencies: Some(map),
            ..Default::default()
        }
    }

    // ── Plugin detection via enablers ────────────────────────────

    #[test]
    fn nextjs_detected_when_next_in_deps() {
        let registry = PluginRegistry::default();
        let pkg = make_pkg(&["next", "react"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(
            result.active_plugins.contains(&"nextjs".to_string()),
            "nextjs plugin should be active when 'next' is in deps"
        );
    }

    #[test]
    fn nextjs_not_detected_without_next() {
        let registry = PluginRegistry::default();
        let pkg = make_pkg(&["react", "react-dom"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(
            !result.active_plugins.contains(&"nextjs".to_string()),
            "nextjs plugin should not be active without 'next' in deps"
        );
    }

    #[test]
    fn prefix_enabler_matches_scoped_packages() {
        // Storybook uses "@storybook/" prefix matcher
        let registry = PluginRegistry::default();
        let pkg = make_pkg(&["@storybook/react"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(
            result.active_plugins.contains(&"storybook".to_string()),
            "storybook should activate via prefix match on @storybook/react"
        );
    }

    #[test]
    fn prefix_enabler_does_not_match_without_slash() {
        // "storybook" (exact) should match, but "@storybook" (without /) should not match via prefix
        let registry = PluginRegistry::default();
        // This only has a package called "@storybookish" — it should NOT match
        let mut map = HashMap::new();
        map.insert("@storybookish".to_string(), "*".to_string());
        let pkg = PackageJson {
            dependencies: Some(map),
            ..Default::default()
        };
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(
            !result.active_plugins.contains(&"storybook".to_string()),
            "storybook should not activate for '@storybookish' (no slash prefix match)"
        );
    }

    #[test]
    fn multiple_plugins_detected_simultaneously() {
        let registry = PluginRegistry::default();
        let pkg = make_pkg(&["next", "vitest", "typescript"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(result.active_plugins.contains(&"nextjs".to_string()));
        assert!(result.active_plugins.contains(&"vitest".to_string()));
        assert!(result.active_plugins.contains(&"typescript".to_string()));
    }

    #[test]
    fn no_plugins_for_empty_deps() {
        let registry = PluginRegistry::default();
        let pkg = PackageJson::default();
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(
            result.active_plugins.is_empty(),
            "no plugins should activate with empty package.json"
        );
    }

    // ── Aggregation: entry patterns, tooling deps ────────────────

    #[test]
    fn active_plugin_contributes_entry_patterns() {
        let registry = PluginRegistry::default();
        let pkg = make_pkg(&["next"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        // Next.js should contribute App Router entry patterns
        assert!(
            result
                .entry_patterns
                .iter()
                .any(|(p, _)| p.contains("app/**/page")),
            "nextjs plugin should add app/**/page entry pattern"
        );
    }

    #[test]
    fn inactive_plugin_does_not_contribute_entry_patterns() {
        let registry = PluginRegistry::default();
        let pkg = make_pkg(&["react"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        // Next.js patterns should not be present
        assert!(
            !result
                .entry_patterns
                .iter()
                .any(|(p, _)| p.contains("app/**/page")),
            "nextjs patterns should not appear when plugin is inactive"
        );
    }

    #[test]
    fn active_plugin_contributes_tooling_deps() {
        let registry = PluginRegistry::default();
        let pkg = make_pkg(&["next"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(
            result.tooling_dependencies.contains(&"next".to_string()),
            "nextjs plugin should list 'next' as a tooling dependency"
        );
    }

    #[test]
    fn dev_deps_also_trigger_plugins() {
        let registry = PluginRegistry::default();
        let pkg = make_pkg_dev(&["vitest"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(
            result.active_plugins.contains(&"vitest".to_string()),
            "vitest should activate from devDependencies"
        );
    }

    // ── External plugins ─────────────────────────────────────────

    #[test]
    fn external_plugin_detected_by_enablers() {
        let ext = ExternalPluginDef {
            schema: None,
            name: "my-framework".to_string(),
            detection: None,
            enablers: vec!["my-framework".to_string()],
            entry_points: vec!["src/routes/**/*.ts".to_string()],
            config_patterns: vec![],
            always_used: vec!["my.config.ts".to_string()],
            tooling_dependencies: vec!["my-framework-cli".to_string()],
            used_exports: vec![],
        };
        let registry = PluginRegistry::new(vec![ext]);
        let pkg = make_pkg(&["my-framework"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(result.active_plugins.contains(&"my-framework".to_string()));
        assert!(
            result
                .entry_patterns
                .iter()
                .any(|(p, _)| p == "src/routes/**/*.ts")
        );
        assert!(
            result
                .tooling_dependencies
                .contains(&"my-framework-cli".to_string())
        );
    }

    #[test]
    fn external_plugin_not_detected_when_dep_missing() {
        let ext = ExternalPluginDef {
            schema: None,
            name: "my-framework".to_string(),
            detection: None,
            enablers: vec!["my-framework".to_string()],
            entry_points: vec!["src/routes/**/*.ts".to_string()],
            config_patterns: vec![],
            always_used: vec![],
            tooling_dependencies: vec![],
            used_exports: vec![],
        };
        let registry = PluginRegistry::new(vec![ext]);
        let pkg = make_pkg(&["react"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(!result.active_plugins.contains(&"my-framework".to_string()));
        assert!(
            !result
                .entry_patterns
                .iter()
                .any(|(p, _)| p == "src/routes/**/*.ts")
        );
    }

    #[test]
    fn external_plugin_prefix_enabler() {
        let ext = ExternalPluginDef {
            schema: None,
            name: "custom-plugin".to_string(),
            detection: None,
            enablers: vec!["@custom/".to_string()],
            entry_points: vec!["custom/**/*.ts".to_string()],
            config_patterns: vec![],
            always_used: vec![],
            tooling_dependencies: vec![],
            used_exports: vec![],
        };
        let registry = PluginRegistry::new(vec![ext]);
        let pkg = make_pkg(&["@custom/core"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(result.active_plugins.contains(&"custom-plugin".to_string()));
    }

    #[test]
    fn external_plugin_detection_dependency() {
        let ext = ExternalPluginDef {
            schema: None,
            name: "detected-plugin".to_string(),
            detection: Some(PluginDetection::Dependency {
                package: "special-dep".to_string(),
            }),
            enablers: vec![],
            entry_points: vec!["special/**/*.ts".to_string()],
            config_patterns: vec![],
            always_used: vec![],
            tooling_dependencies: vec![],
            used_exports: vec![],
        };
        let registry = PluginRegistry::new(vec![ext]);
        let pkg = make_pkg(&["special-dep"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(
            result
                .active_plugins
                .contains(&"detected-plugin".to_string())
        );
    }

    #[test]
    fn external_plugin_detection_any_combinator() {
        let ext = ExternalPluginDef {
            schema: None,
            name: "any-plugin".to_string(),
            detection: Some(PluginDetection::Any {
                conditions: vec![
                    PluginDetection::Dependency {
                        package: "pkg-a".to_string(),
                    },
                    PluginDetection::Dependency {
                        package: "pkg-b".to_string(),
                    },
                ],
            }),
            enablers: vec![],
            entry_points: vec!["any/**/*.ts".to_string()],
            config_patterns: vec![],
            always_used: vec![],
            tooling_dependencies: vec![],
            used_exports: vec![],
        };
        let registry = PluginRegistry::new(vec![ext]);
        // Only pkg-b present — should still match via Any
        let pkg = make_pkg(&["pkg-b"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(result.active_plugins.contains(&"any-plugin".to_string()));
    }

    #[test]
    fn external_plugin_detection_all_combinator_fails_partial() {
        let ext = ExternalPluginDef {
            schema: None,
            name: "all-plugin".to_string(),
            detection: Some(PluginDetection::All {
                conditions: vec![
                    PluginDetection::Dependency {
                        package: "pkg-a".to_string(),
                    },
                    PluginDetection::Dependency {
                        package: "pkg-b".to_string(),
                    },
                ],
            }),
            enablers: vec![],
            entry_points: vec![],
            config_patterns: vec![],
            always_used: vec![],
            tooling_dependencies: vec![],
            used_exports: vec![],
        };
        let registry = PluginRegistry::new(vec![ext]);
        // Only pkg-a present — All requires both
        let pkg = make_pkg(&["pkg-a"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(!result.active_plugins.contains(&"all-plugin".to_string()));
    }

    #[test]
    fn external_plugin_used_exports_aggregated() {
        let ext = ExternalPluginDef {
            schema: None,
            name: "ue-plugin".to_string(),
            detection: None,
            enablers: vec!["ue-dep".to_string()],
            entry_points: vec![],
            config_patterns: vec![],
            always_used: vec![],
            tooling_dependencies: vec![],
            used_exports: vec![ExternalUsedExport {
                pattern: "pages/**/*.tsx".to_string(),
                exports: vec!["default".to_string(), "getServerSideProps".to_string()],
            }],
        };
        let registry = PluginRegistry::new(vec![ext]);
        let pkg = make_pkg(&["ue-dep"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(result.used_exports.iter().any(|(pat, exports)| {
            pat == "pages/**/*.tsx" && exports.contains(&"default".to_string())
        }));
    }

    #[test]
    fn external_plugin_without_enablers_or_detection_stays_inactive() {
        let ext = ExternalPluginDef {
            schema: None,
            name: "orphan-plugin".to_string(),
            detection: None,
            enablers: vec![],
            entry_points: vec!["orphan/**/*.ts".to_string()],
            config_patterns: vec![],
            always_used: vec![],
            tooling_dependencies: vec![],
            used_exports: vec![],
        };
        let registry = PluginRegistry::new(vec![ext]);
        let pkg = make_pkg(&["anything"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(!result.active_plugins.contains(&"orphan-plugin".to_string()));
    }

    // ── Virtual module prefixes ──────────────────────────────────

    #[test]
    fn nuxt_contributes_virtual_module_prefixes() {
        let registry = PluginRegistry::default();
        let pkg = make_pkg(&["nuxt"]);
        let result = registry.run(&pkg, Path::new("/project"), &[]);
        assert!(
            result.virtual_module_prefixes.contains(&"#".to_string()),
            "nuxt should contribute '#' virtual module prefix"
        );
    }

    // ── Precompile config matchers ───────────────────────────────

    #[test]
    fn precompile_config_matchers_returns_entries() {
        let registry = PluginRegistry::default();
        let matchers = registry.precompile_config_matchers();
        // At minimum, nextjs, vite, jest, typescript, etc. all have config patterns
        assert!(
            !matchers.is_empty(),
            "precompile_config_matchers should return entries for plugins with config patterns"
        );
    }

    #[test]
    fn precompile_config_matchers_only_for_plugins_with_patterns() {
        let registry = PluginRegistry::default();
        let matchers = registry.precompile_config_matchers();
        for (plugin, _) in &matchers {
            assert!(
                !plugin.config_patterns().is_empty(),
                "plugin '{}' in matchers should have config patterns",
                plugin.name()
            );
        }
    }
}
