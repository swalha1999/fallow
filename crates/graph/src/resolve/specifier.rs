//! Main resolution engine: creates the oxc_resolver instance and resolves individual specifiers.

use std::path::{Path, PathBuf};

use oxc_resolver::{ResolveOptions, Resolver};

use super::fallbacks::{
    extract_package_name_from_node_modules_path, try_path_alias_fallback,
    try_pnpm_workspace_fallback, try_source_fallback,
};
use super::path_info::{extract_package_name, is_bare_specifier, is_path_alias};
use super::react_native::{build_condition_names, build_extensions};
use super::types::{ResolveContext, ResolveResult};

/// Create an `oxc_resolver` instance with standard configuration.
///
/// When React Native or Expo plugins are active, platform-specific extensions
/// (e.g., `.web.tsx`, `.ios.ts`) are prepended to the extension list so that
/// Metro-style platform resolution works correctly.
pub(super) fn create_resolver(active_plugins: &[String]) -> Resolver {
    let mut options = ResolveOptions {
        extensions: build_extensions(active_plugins),
        // Support TypeScript's node16/nodenext module resolution where .ts files
        // are imported with .js extensions (e.g., `import './api.js'` for `api.ts`).
        extension_alias: vec![
            (
                ".js".into(),
                vec![".ts".into(), ".tsx".into(), ".js".into()],
            ),
            (".jsx".into(), vec![".tsx".into(), ".jsx".into()]),
            (".mjs".into(), vec![".mts".into(), ".mjs".into()]),
            (".cjs".into(), vec![".cts".into(), ".cjs".into()]),
        ],
        condition_names: build_condition_names(active_plugins),
        main_fields: vec!["module".into(), "main".into()],
        ..Default::default()
    };

    // Always use auto-discovery mode so oxc_resolver finds the nearest tsconfig.json
    // for each file. This is critical for monorepos where workspace packages have
    // their own tsconfig with path aliases (e.g., `~/*` → `./src/*`). Manual mode
    // with a root tsconfig only uses that single tsconfig's paths for ALL files,
    // missing workspace-specific aliases. Auto mode walks up from each file to find
    // the nearest tsconfig.json and follows `extends` chains, so workspace tsconfigs
    // that extend a root tsconfig still inherit root-level paths.
    options.tsconfig = Some(oxc_resolver::TsconfigDiscovery::Auto);

    Resolver::new(options)
}

/// Resolve a single import specifier to a target.
pub(super) fn resolve_specifier(
    ctx: &ResolveContext<'_>,
    from_file: &Path,
    specifier: &str,
) -> ResolveResult {
    // URL imports (https://, http://, data:) are valid but can't be resolved locally
    if specifier.contains("://") || specifier.starts_with("data:") {
        return ResolveResult::ExternalFile(PathBuf::from(specifier));
    }

    // Fast path for bare specifiers: check cache first to avoid repeated resolver work.
    // Path aliases (e.g., `@/components`, `~/lib`) are excluded from caching because
    // they may resolve differently depending on the importing file's tsconfig context.
    let is_bare = is_bare_specifier(specifier);
    let is_alias = is_path_alias(specifier);
    if is_bare
        && !is_alias
        && let Some(cached) = ctx.bare_cache.get(specifier)
    {
        return cached;
    }

    // Use resolve_file instead of resolve so that TsconfigDiscovery::Auto works.
    // oxc_resolver's resolve() ignores Auto tsconfig discovery — only resolve_file()
    // walks up from the importing file to find the nearest tsconfig.json and apply
    // its path aliases (e.g., @/ → src/).
    //
    // Track whether the resolver succeeded to decide caching strategy. When resolution
    // fails, the specifier might be a tsconfig path alias (e.g., `@bazam/shared-types`)
    // that `is_path_alias` didn't detect (lowercase scoped package). Caching the
    // `NpmPackage` fallback would poison the cache — all subsequent files would skip
    // resolution and use the wrong result, even if their tsconfig context can resolve it.
    let (result, resolver_succeeded) = match ctx.resolver.resolve_file(from_file, specifier) {
        Ok(resolved) => {
            let resolved_path = resolved.path();
            // Try raw path lookup first (avoids canonicalize syscall in most cases)
            if let Some(&file_id) = ctx.raw_path_to_id.get(resolved_path) {
                let result = ResolveResult::InternalModule(file_id);
                // Cache successful resolution for reuse by other files
                if is_bare && !is_alias {
                    ctx.bare_cache.insert(specifier.to_string(), result.clone());
                }
                return result;
            }
            // Fall back to canonical path lookup
            let result = match resolved_path.canonicalize() {
                Ok(canonical) => {
                    if let Some(&file_id) = ctx.path_to_id.get(canonical.as_path()) {
                        ResolveResult::InternalModule(file_id)
                    } else if let Some(file_id) = try_source_fallback(&canonical, ctx.path_to_id) {
                        // Exports map resolved to a built output (e.g., dist/utils.js)
                        // but the source file (e.g., src/utils.ts) is what we track.
                        ResolveResult::InternalModule(file_id)
                    } else if let Some(file_id) =
                        try_pnpm_workspace_fallback(&canonical, ctx.path_to_id, ctx.workspace_roots)
                    {
                        ResolveResult::InternalModule(file_id)
                    } else if let Some(pkg_name) =
                        extract_package_name_from_node_modules_path(&canonical)
                    {
                        ResolveResult::NpmPackage(pkg_name)
                    } else {
                        ResolveResult::ExternalFile(canonical)
                    }
                }
                Err(_) => {
                    // Path doesn't exist on disk — try source fallback on the raw path
                    if let Some(file_id) = try_source_fallback(resolved_path, ctx.path_to_id) {
                        ResolveResult::InternalModule(file_id)
                    } else if let Some(file_id) = try_pnpm_workspace_fallback(
                        resolved_path,
                        ctx.path_to_id,
                        ctx.workspace_roots,
                    ) {
                        ResolveResult::InternalModule(file_id)
                    } else if let Some(pkg_name) =
                        extract_package_name_from_node_modules_path(resolved_path)
                    {
                        ResolveResult::NpmPackage(pkg_name)
                    } else {
                        ResolveResult::ExternalFile(resolved_path.to_path_buf())
                    }
                }
            };
            (result, true)
        }
        Err(_) => {
            let result = if is_alias {
                // Try plugin-provided path aliases before giving up.
                // These substitute import prefixes (e.g., `~/` → `app/`) and re-resolve
                // as relative imports from the project root.
                if let Some(resolved) = try_path_alias_fallback(ctx, specifier) {
                    resolved
                } else {
                    // Path aliases that fail resolution are unresolvable, not npm packages.
                    // Classifying them as NpmPackage would cause false "unlisted dependency" reports.
                    ResolveResult::Unresolvable(specifier.to_string())
                }
            } else if is_bare {
                let pkg_name = extract_package_name(specifier);
                ResolveResult::NpmPackage(pkg_name)
            } else {
                ResolveResult::Unresolvable(specifier.to_string())
            };
            (result, false)
        }
    };

    // Cache bare specifier results only when the resolver succeeded.
    // When resolver.resolve_file returned Ok, the result is authoritative — it either
    // found the file in node_modules (NpmPackage) or via tsconfig paths (InternalModule).
    // When it returned Err, the NpmPackage fallback is a guess — another file's tsconfig
    // context might resolve the same specifier to an internal module. Caching that guess
    // would prevent correct resolution for all subsequent files.
    if is_bare && !is_alias && resolver_succeeded {
        ctx.bare_cache.insert(specifier.to_string(), result.clone());
    }

    result
}
