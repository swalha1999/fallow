use std::collections::HashMap;
use std::path::Path;

use tower_lsp::lsp_types::*;

use fallow_core::duplicates::DuplicationReport;
use fallow_core::results::AnalysisResults;

/// LSP range at position (0,0) used for file-level and package.json diagnostics.
pub(crate) const ZERO_RANGE: Range = Range {
    start: Position {
        line: 0,
        character: 0,
    },
    end: Position {
        line: 0,
        character: 0,
    },
};

/// Build all LSP diagnostics from analysis results and duplication report, keyed by file URI.
pub(crate) fn build_diagnostics(
    results: &AnalysisResults,
    duplication: &DuplicationReport,
    root: &Path,
) -> HashMap<Url, Vec<Diagnostic>> {
    let mut diagnostics_by_file: HashMap<Url, Vec<Diagnostic>> = HashMap::new();

    // Helper: get the package.json URI for dependency-related diagnostics
    let package_json_path = root.join("package.json");
    let package_json_uri = Url::from_file_path(&package_json_path).ok();

    // Export-like issues: unused exports and unused types
    for (exports, code, msg_prefix) in [
        (&results.unused_exports, "unused-export", "Export" as &str),
        (&results.unused_types, "unused-type", "Type export"),
    ] {
        for export in exports {
            if let Ok(uri) = Url::from_file_path(&export.path) {
                let line = export.line.saturating_sub(1);
                diagnostics_by_file
                    .entry(uri)
                    .or_default()
                    .push(Diagnostic {
                        range: Range {
                            start: Position {
                                line,
                                character: export.col,
                            },
                            end: Position {
                                line,
                                character: export.col + export.export_name.len() as u32,
                            },
                        },
                        severity: Some(DiagnosticSeverity::HINT),
                        source: Some("fallow".to_string()),
                        code: Some(NumberOrString::String(code.to_string())),
                        message: format!("{msg_prefix} '{}' is unused", export.export_name),
                        tags: Some(vec![DiagnosticTag::UNNECESSARY]),
                        ..Default::default()
                    });
            }
        }
    }

    // Unused files: path-only diagnostic at (0,0)
    for file in &results.unused_files {
        if let Ok(uri) = Url::from_file_path(&file.path) {
            diagnostics_by_file
                .entry(uri)
                .or_default()
                .push(Diagnostic {
                    range: ZERO_RANGE,
                    severity: Some(DiagnosticSeverity::WARNING),
                    source: Some("fallow".to_string()),
                    code: Some(NumberOrString::String("unused-file".to_string())),
                    message: "File is not reachable from any entry point".to_string(),
                    tags: Some(vec![DiagnosticTag::UNNECESSARY]),
                    ..Default::default()
                });
        }
    }

    // Unresolved imports
    for import in &results.unresolved_imports {
        if let Ok(uri) = Url::from_file_path(&import.path) {
            let line = import.line.saturating_sub(1);
            diagnostics_by_file
                .entry(uri)
                .or_default()
                .push(Diagnostic {
                    range: Range {
                        start: Position {
                            line,
                            character: import.col,
                        },
                        end: Position {
                            line,
                            character: import.col,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("fallow".to_string()),
                    code: Some(NumberOrString::String("unresolved-import".to_string())),
                    message: format!("Cannot resolve import '{}'", import.specifier),
                    ..Default::default()
                });
        }
    }

    // Dependency issues: unused deps, unused dev deps (routed to their respective package.json)
    for dep in &results.unused_dependencies {
        if let Ok(dep_uri) = Url::from_file_path(&dep.path) {
            let entry = diagnostics_by_file.entry(dep_uri).or_default();
            entry.push(Diagnostic {
                range: ZERO_RANGE,
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("fallow".to_string()),
                code: Some(NumberOrString::String("unused-dependency".to_string())),
                message: format!("Unused dependency: {}", dep.package_name),
                ..Default::default()
            });
        }
    }
    for dep in &results.unused_dev_dependencies {
        if let Ok(dep_uri) = Url::from_file_path(&dep.path) {
            let entry = diagnostics_by_file.entry(dep_uri).or_default();
            entry.push(Diagnostic {
                range: ZERO_RANGE,
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("fallow".to_string()),
                code: Some(NumberOrString::String("unused-dev-dependency".to_string())),
                message: format!("Unused devDependency: {}", dep.package_name),
                ..Default::default()
            });
        }
    }
    // Unlisted deps still use root package.json
    if let Some(ref uri) = package_json_uri {
        for dep in &results.unlisted_dependencies {
            let entry = diagnostics_by_file.entry(uri.clone()).or_default();
            entry.push(Diagnostic {
                range: ZERO_RANGE,
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("fallow".to_string()),
                code: Some(NumberOrString::String("unlisted-dependency".to_string())),
                message: format!(
                    "Unlisted dependency: {} (used but not in package.json)",
                    dep.package_name
                ),
                ..Default::default()
            });
        }
    }

    // Member issues: unused enum members and unused class members
    for (members, code, kind_label) in [
        (
            &results.unused_enum_members,
            "unused-enum-member",
            "Enum member" as &str,
        ),
        (
            &results.unused_class_members,
            "unused-class-member",
            "Class member",
        ),
    ] {
        for member in members {
            if let Ok(uri) = Url::from_file_path(&member.path) {
                let line = member.line.saturating_sub(1);
                diagnostics_by_file
                    .entry(uri)
                    .or_default()
                    .push(Diagnostic {
                        range: Range {
                            start: Position {
                                line,
                                character: member.col,
                            },
                            end: Position {
                                line,
                                character: member.col + member.member_name.len() as u32,
                            },
                        },
                        severity: Some(DiagnosticSeverity::HINT),
                        source: Some("fallow".to_string()),
                        code: Some(NumberOrString::String(code.to_string())),
                        message: format!(
                            "{kind_label} '{}.{}' is unused",
                            member.parent_name, member.member_name
                        ),
                        tags: Some(vec![DiagnosticTag::UNNECESSARY]),
                        ..Default::default()
                    });
            }
        }
    }

    // Duplicate exports: WARNING on each file that has the duplicate
    for dup in &results.duplicate_exports {
        for location in &dup.locations {
            if let Ok(uri) = Url::from_file_path(location) {
                let other_files: Vec<String> = dup
                    .locations
                    .iter()
                    .filter(|l| *l != location)
                    .map(|l| l.display().to_string())
                    .collect();
                diagnostics_by_file
                    .entry(uri)
                    .or_default()
                    .push(Diagnostic {
                        range: ZERO_RANGE,
                        severity: Some(DiagnosticSeverity::WARNING),
                        source: Some("fallow".to_string()),
                        code: Some(NumberOrString::String("duplicate-export".to_string())),
                        message: format!(
                            "Duplicate export '{}' (also in: {})",
                            dup.export_name,
                            other_files.join(", ")
                        ),
                        ..Default::default()
                    });
            }
        }
    }

    // Code duplication diagnostics
    for group in &duplication.clone_groups {
        for instance in &group.instances {
            let Ok(inst_uri) = Url::from_file_path(&instance.file) else {
                continue;
            };

            let start_line = (instance.start_line as u32).saturating_sub(1);
            let end_line = (instance.end_line as u32).saturating_sub(1);

            // Build related information pointing to other instances in the group
            let related_info: Vec<DiagnosticRelatedInformation> = group
                .instances
                .iter()
                .filter(|other| {
                    !(other.file == instance.file && other.start_line == instance.start_line)
                })
                .filter_map(|other| {
                    let other_uri = Url::from_file_path(&other.file).ok()?;
                    Some(DiagnosticRelatedInformation {
                        location: Location {
                            uri: other_uri,
                            range: Range {
                                start: Position {
                                    line: (other.start_line as u32).saturating_sub(1),
                                    character: other.start_col as u32,
                                },
                                end: Position {
                                    line: (other.end_line as u32).saturating_sub(1),
                                    character: other.end_col as u32,
                                },
                            },
                        },
                        message: "Also duplicated here".to_string(),
                    })
                })
                .collect();

            diagnostics_by_file
                .entry(inst_uri)
                .or_default()
                .push(Diagnostic {
                    range: Range {
                        start: Position {
                            line: start_line,
                            character: instance.start_col as u32,
                        },
                        end: Position {
                            line: end_line,
                            character: instance.end_col as u32,
                        },
                    },
                    severity: Some(DiagnosticSeverity::HINT),
                    source: Some("fallow".to_string()),
                    code: Some(NumberOrString::String("code-duplication".to_string())),
                    message: format!(
                        "Duplicated code block ({} lines, {} instances)",
                        group.line_count,
                        group.instances.len()
                    ),
                    related_information: if related_info.is_empty() {
                        None
                    } else {
                        Some(related_info)
                    },
                    ..Default::default()
                });
        }
    }

    diagnostics_by_file
}
