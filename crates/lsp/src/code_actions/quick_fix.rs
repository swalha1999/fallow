#[expect(clippy::disallowed_types)]
use std::collections::HashMap;
use std::path::Path;

use tower_lsp::lsp_types::*;

use fallow_core::results::AnalysisResults;

use crate::diagnostics::ZERO_RANGE;

/// Build quick-fix code actions for unused exports (remove the `export` keyword).
#[expect(clippy::disallowed_types)]
pub fn build_remove_export_actions(
    results: &AnalysisResults,
    file_path: &Path,
    uri: &Url,
    cursor_range: &Range,
    file_lines: &[&str],
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    for (exports, msg_prefix) in [
        (&results.unused_exports, "Export"),
        (&results.unused_types, "Type export"),
    ] {
        for export in exports {
            if export.path != file_path {
                continue;
            }

            // export.line is a 1-based line number; convert to 0-based for LSP
            let export_line = export.line.saturating_sub(1);

            // Check if this diagnostic is in the requested range
            if export_line < cursor_range.start.line || export_line > cursor_range.end.line {
                continue;
            }

            // Determine the export prefix to remove by inspecting the line content
            let line_content = file_lines.get(export_line as usize).copied().unwrap_or("");
            let trimmed = line_content.trim_start();
            let indent_len = line_content.len() - trimmed.len();

            let prefix_to_remove = if trimmed.starts_with("export default ") {
                Some("export default ")
            } else if trimmed.starts_with("export ") {
                // Handles: export const, export function, export class, export type,
                // export interface, export enum, export abstract, export async,
                // export let, export var, etc.
                Some("export ")
            } else {
                None
            };

            let Some(prefix) = prefix_to_remove else {
                continue;
            };

            let title = format!("Remove unused export `{}`", export.export_name);
            let mut changes = HashMap::new();

            // Create a text edit that removes the export keyword prefix
            let edit = TextEdit {
                range: Range {
                    start: Position {
                        line: export_line,
                        character: indent_len as u32,
                    },
                    end: Position {
                        line: export_line,
                        character: (indent_len + prefix.len()) as u32,
                    },
                },
                new_text: String::new(),
            };

            changes.insert(uri.clone(), vec![edit]);

            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                title,
                kind: Some(CodeActionKind::QUICKFIX),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    ..Default::default()
                }),
                diagnostics: Some(vec![Diagnostic {
                    range: Range {
                        start: Position {
                            line: export_line,
                            character: export.col,
                        },
                        end: Position {
                            line: export_line,
                            character: export.col + export.export_name.len() as u32,
                        },
                    },
                    severity: Some(DiagnosticSeverity::HINT),
                    source: Some("fallow".to_string()),
                    message: format!("{msg_prefix} '{}' is unused", export.export_name),
                    tags: Some(vec![DiagnosticTag::UNNECESSARY]),
                    ..Default::default()
                }]),
                ..Default::default()
            }));
        }
    }

    actions
}

/// Build quick-fix code actions for unused files (delete the file).
pub fn build_delete_file_actions(
    results: &AnalysisResults,
    file_path: &Path,
    uri: &Url,
    cursor_range: &Range,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    for file in &results.unused_files {
        if file.path != file_path {
            continue;
        }

        // The diagnostic is at line 0, col 0 — check if the request range overlaps
        if cursor_range.start.line > 0 {
            continue;
        }

        let title = "Delete this unused file".to_string();

        let delete_file_op = DocumentChangeOperation::Op(ResourceOp::Delete(DeleteFile {
            uri: uri.clone(),
            options: Some(DeleteFileOptions {
                recursive: Some(false),
                ignore_if_not_exists: Some(true),
                annotation_id: None,
            }),
        }));

        actions.push(CodeActionOrCommand::CodeAction(CodeAction {
            title,
            kind: Some(CodeActionKind::QUICKFIX),
            edit: Some(WorkspaceEdit {
                document_changes: Some(DocumentChanges::Operations(vec![delete_file_op])),
                ..Default::default()
            }),
            diagnostics: Some(vec![Diagnostic {
                range: ZERO_RANGE,
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("fallow".to_string()),
                code: Some(NumberOrString::String("unused-file".to_string())),
                message: "File is not reachable from any entry point".to_string(),
                tags: Some(vec![DiagnosticTag::UNNECESSARY]),
                ..Default::default()
            }]),
            ..Default::default()
        }));
    }

    actions
}
