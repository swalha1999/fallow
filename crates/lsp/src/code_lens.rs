use std::path::Path;

use tower_lsp::lsp_types::*;

use fallow_core::results::AnalysisResults;

/// Build Code Lens items for a file showing reference counts above each export declaration.
pub(crate) fn build_code_lenses(
    results: &AnalysisResults,
    file_path: &Path,
    document_uri: &Url,
) -> Vec<CodeLens> {
    results
        .export_usages
        .iter()
        .filter(|usage| usage.path == file_path)
        .map(|usage| {
            // usage.line is 1-based; LSP positions are 0-based
            let line = usage.line.saturating_sub(1);
            let title = if usage.reference_count == 1 {
                "1 reference".to_string()
            } else {
                format!("{} references", usage.reference_count)
            };

            let export_position = Position {
                line,
                character: usage.col,
            };

            // Build reference Location objects for editor.action.showReferences
            let ref_locations: Vec<serde_json::Value> = usage
                .reference_locations
                .iter()
                .filter_map(|loc| {
                    let uri = Url::from_file_path(&loc.path).ok()?;
                    let ref_line = loc.line.saturating_sub(1);
                    Some(serde_json::json!({
                        "uri": uri.as_str(),
                        "range": {
                            "start": { "line": ref_line, "character": loc.col },
                            "end": { "line": ref_line, "character": loc.col }
                        }
                    }))
                })
                .collect();

            // Use editor.action.showReferences when we have reference locations,
            // fall back to display-only noop otherwise
            let (command_name, arguments) = if ref_locations.is_empty() {
                ("fallow.noop".to_string(), None)
            } else {
                (
                    "editor.action.showReferences".to_string(),
                    Some(vec![
                        serde_json::json!(document_uri.as_str()),
                        serde_json::json!({
                            "line": export_position.line,
                            "character": export_position.character,
                        }),
                        serde_json::json!(ref_locations),
                    ]),
                )
            };

            CodeLens {
                range: Range {
                    start: export_position,
                    end: export_position,
                },
                command: Some(Command {
                    title,
                    command: command_name,
                    arguments,
                }),
                data: None,
            }
        })
        .collect()
}
