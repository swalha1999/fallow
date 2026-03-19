use std::collections::{HashMap, HashSet};

use fallow_config::ResolvedConfig;

use crate::discover::FileId;
use crate::extract::MemberKind;
use crate::graph::ModuleGraph;
use crate::resolve::ResolvedModule;
use crate::results::*;
use crate::suppress::{self, IssueKind, Suppression};

use super::predicates::{is_angular_lifecycle_method, is_react_lifecycle_method};
use super::{byte_offset_to_line_col, read_source};

/// Find unused enum and class members in exported symbols.
///
/// Collects all `Identifier.member` static member accesses from all modules,
/// maps them to their imported names, and filters out members that are accessed.
pub(crate) fn find_unused_members(
    graph: &ModuleGraph,
    _config: &ResolvedConfig,
    resolved_modules: &[ResolvedModule],
    suppressions_by_file: &HashMap<FileId, &[Suppression]>,
) -> (Vec<UnusedMember>, Vec<UnusedMember>) {
    let mut unused_enum_members = Vec::new();
    let mut unused_class_members = Vec::new();

    // Map export_name -> set of member_names that are accessed across all modules.
    // We map local import names back to the original imported names.
    let mut accessed_members: HashMap<String, HashSet<String>> = HashMap::new();

    // Also build a per-file set of `this.member` accesses. These indicate internal usage
    // within a class body — class members accessed via `this.foo` are used internally
    // even if no external code accesses them via `ClassName.foo`.
    let mut self_accessed_members: HashMap<crate::discover::FileId, HashSet<String>> =
        HashMap::new();

    // Build a set of export names that are used as whole objects (Object.values, for..in, etc.).
    // All members of these exports should be considered used.
    let mut whole_object_used_exports: HashSet<String> = HashSet::new();

    for resolved in resolved_modules {
        // Build a map from local name -> imported name for this module's imports
        let local_to_imported: HashMap<&str, &str> = resolved
            .resolved_imports
            .iter()
            .filter_map(|imp| match &imp.info.imported_name {
                crate::extract::ImportedName::Named(name) => {
                    Some((imp.info.local_name.as_str(), name.as_str()))
                }
                crate::extract::ImportedName::Default => {
                    Some((imp.info.local_name.as_str(), "default"))
                }
                _ => None,
            })
            .collect();

        for access in &resolved.member_accesses {
            // Track `this.member` accesses per-file for internal class usage
            if access.object == "this" {
                self_accessed_members
                    .entry(resolved.file_id)
                    .or_default()
                    .insert(access.member.clone());
                continue;
            }
            // If the object is a local name for an import, map it to the original export name
            let export_name = local_to_imported
                .get(access.object.as_str())
                .copied()
                .unwrap_or(access.object.as_str());
            accessed_members
                .entry(export_name.to_string())
                .or_default()
                .insert(access.member.clone());
        }

        // Map whole-object uses from local names to imported names
        for local_name in &resolved.whole_object_uses {
            let export_name = local_to_imported
                .get(local_name.as_str())
                .copied()
                .unwrap_or(local_name.as_str());
            whole_object_used_exports.insert(export_name.to_string());
        }
    }

    for module in &graph.modules {
        if !module.is_reachable || module.is_entry_point {
            continue;
        }

        // Lazily load source content for line/col computation
        let mut source_content: Option<String> = None;

        for export in &module.exports {
            if export.members.is_empty() {
                continue;
            }

            // If the export itself is unused, skip member analysis (whole export is dead)
            if export.references.is_empty() && !graph.has_namespace_import(module.file_id) {
                continue;
            }

            let export_name = export.name.to_string();

            // If this export is used as a whole object (Object.values, for..in, etc.),
            // all members are considered used — skip individual member analysis.
            if whole_object_used_exports.contains(&export_name) {
                continue;
            }

            // Get `this.member` accesses from this file (internal class usage)
            let file_self_accesses = self_accessed_members.get(&module.file_id);

            for member in &export.members {
                // Check if this member is accessed anywhere via external import
                if accessed_members
                    .get(&export_name)
                    .is_some_and(|s| s.contains(&member.name))
                {
                    continue;
                }

                // Check if this member is accessed via `this.member` within the same file
                // (internal class usage — e.g., constructor sets this.label, methods use this.label)
                if matches!(
                    member.kind,
                    MemberKind::ClassMethod | MemberKind::ClassProperty
                ) && file_self_accesses.is_some_and(|accesses| accesses.contains(&member.name))
                {
                    continue;
                }

                // Skip decorated class members — decorators like @Column(), @ApiProperty(),
                // @Inject() etc. indicate runtime usage by frameworks (NestJS, TypeORM,
                // class-validator, class-transformer). These members are accessed
                // reflectively and should never be flagged as unused.
                if member.has_decorator {
                    continue;
                }

                // Skip React class component lifecycle methods — they are called by the
                // React runtime, not user code, so they should never be flagged as unused.
                // Also skip Angular lifecycle hooks (OnInit, OnDestroy, etc.).
                if matches!(
                    member.kind,
                    MemberKind::ClassMethod | MemberKind::ClassProperty
                ) && (is_react_lifecycle_method(&member.name)
                    || is_angular_lifecycle_method(&member.name))
                {
                    continue;
                }

                let source = source_content.get_or_insert_with(|| read_source(&module.path));
                let (line, col) = byte_offset_to_line_col(source, member.span.start);

                // Check inline suppression
                let issue_kind = match member.kind {
                    MemberKind::EnumMember => IssueKind::UnusedEnumMember,
                    MemberKind::ClassMethod | MemberKind::ClassProperty => {
                        IssueKind::UnusedClassMember
                    }
                };
                if let Some(supps) = suppressions_by_file.get(&module.file_id)
                    && suppress::is_suppressed(supps, line, issue_kind)
                {
                    continue;
                }

                let unused = UnusedMember {
                    path: module.path.clone(),
                    parent_name: export_name.clone(),
                    member_name: member.name.clone(),
                    kind: member.kind.clone(),
                    line,
                    col,
                };

                match member.kind {
                    MemberKind::EnumMember => unused_enum_members.push(unused),
                    MemberKind::ClassMethod | MemberKind::ClassProperty => {
                        unused_class_members.push(unused);
                    }
                }
            }
        }
    }

    (unused_enum_members, unused_class_members)
}
