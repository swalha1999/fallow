use fallow_config::ResolvedConfig;

use crate::report;

// ── SARIF output ─────────────────────────────────────────────────

/// Write SARIF output to a file if `--sarif-file` was specified.
pub fn write_sarif_file(
    results: &fallow_core::results::AnalysisResults,
    config: &ResolvedConfig,
    sarif_path: &std::path::Path,
    quiet: bool,
) {
    let sarif = report::build_sarif(results, &config.root, &config.rules);
    match serde_json::to_string_pretty(&sarif) {
        Ok(json) => {
            // Ensure parent directories exist
            if let Some(parent) = sarif_path.parent()
                && !parent.as_os_str().is_empty()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                eprintln!(
                    "Warning: failed to create directory for SARIF file '{}': {e}",
                    sarif_path.display()
                );
            }
            if let Err(e) = std::fs::write(sarif_path, json) {
                eprintln!(
                    "Warning: failed to write SARIF file '{}': {e}",
                    sarif_path.display()
                );
            } else if !quiet {
                eprintln!("SARIF output written to {}", sarif_path.display());
            }
        }
        Err(e) => {
            eprintln!("Warning: failed to serialize SARIF output: {e}");
        }
    }
}

// ── Cross-reference output ───────────────────────────────────────

/// Run duplication cross-reference and print combined findings.
pub fn run_cross_reference(
    config: &ResolvedConfig,
    unfiltered_results: &fallow_core::results::AnalysisResults,
    quiet: bool,
) {
    let files = fallow_core::discover::discover_files(config);
    let dupe_report =
        fallow_core::duplicates::find_duplicates(&config.root, &files, &config.duplicates);
    let cross_ref = fallow_core::cross_reference::cross_reference(&dupe_report, unfiltered_results);

    if cross_ref.has_findings() {
        report::print_cross_reference_findings(&cross_ref, &config.root, quiet, &config.output);
    }
}
