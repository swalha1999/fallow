use std::process::ExitCode;
use std::time::Instant;

use fallow_config::OutputFormat;

use crate::baseline::{DuplicationBaselineData, filter_new_clone_groups};
use crate::report;
use crate::{emit_error, load_config};

#[derive(Clone, clap::ValueEnum)]
pub(crate) enum DupesMode {
    Strict,
    Mild,
    Weak,
    Semantic,
}

pub(crate) struct DupesOptions<'a> {
    pub(crate) root: &'a std::path::Path,
    pub(crate) config_path: &'a Option<std::path::PathBuf>,
    pub(crate) output: OutputFormat,
    pub(crate) no_cache: bool,
    pub(crate) threads: usize,
    pub(crate) quiet: bool,
    pub(crate) mode: DupesMode,
    pub(crate) min_tokens: usize,
    pub(crate) min_lines: usize,
    pub(crate) threshold: f64,
    pub(crate) skip_local: bool,
    pub(crate) cross_language: bool,
    pub(crate) baseline_path: Option<&'a std::path::Path>,
    pub(crate) save_baseline_path: Option<&'a std::path::Path>,
    pub(crate) production: bool,
    pub(crate) trace: Option<&'a str>,
}

pub(crate) fn run_dupes(opts: &DupesOptions<'_>) -> ExitCode {
    let start = Instant::now();

    let config = match load_config(
        opts.root,
        opts.config_path,
        opts.output.clone(),
        opts.no_cache,
        opts.threads,
        opts.production,
    ) {
        Ok(c) => c,
        Err(code) => return code,
    };

    // Build duplication config: start from fallow.toml, override with CLI args
    let toml_dupes = &config.duplicates;
    let dupes_config = fallow_config::DuplicatesConfig {
        enabled: true,
        mode: match opts.mode {
            DupesMode::Strict => fallow_config::DetectionMode::Strict,
            DupesMode::Mild => fallow_config::DetectionMode::Mild,
            DupesMode::Weak => fallow_config::DetectionMode::Weak,
            DupesMode::Semantic => fallow_config::DetectionMode::Semantic,
        },
        min_tokens: opts.min_tokens,
        min_lines: opts.min_lines,
        threshold: opts.threshold,
        ignore: toml_dupes.ignore.clone(),
        skip_local: opts.skip_local,
        cross_language: opts.cross_language || toml_dupes.cross_language,
        normalization: toml_dupes.normalization.clone(),
    };

    // Discover files
    let files = fallow_core::discover::discover_files(&config);

    // Run duplication detection
    let mut report = fallow_core::duplicates::find_duplicates(&config.root, &files, &dupes_config);

    // Handle trace (diagnostic mode — early return)
    if let Some(trace_spec) = opts.trace {
        let (file_path, line_str) = match trace_spec.rsplit_once(':') {
            Some((f, l)) => (f, l),
            None => {
                return emit_error(
                    "--trace requires FILE:LINE format (e.g., src/utils.ts:42)",
                    2,
                    &opts.output,
                );
            }
        };
        let line: usize = match line_str.parse() {
            Ok(l) if l > 0 => l,
            _ => {
                return emit_error("--trace LINE must be a positive integer", 2, &opts.output);
            }
        };
        let trace_result = fallow_core::trace::trace_clone(&report, &config.root, file_path, line);
        if trace_result.matched_instance.is_none() {
            return emit_error(
                &format!("no clone found at {file_path}:{line}"),
                2,
                &opts.output,
            );
        }
        report::print_clone_trace(&trace_result, &config.root, &opts.output);
        return ExitCode::SUCCESS;
    }

    // Save baseline if requested (before filtering)
    if let Some(path) = opts.save_baseline_path {
        let baseline_data = DuplicationBaselineData::from_report(&report, &config.root);
        match serde_json::to_string_pretty(&baseline_data) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    return emit_error(
                        &format!("failed to write duplication baseline: {e}"),
                        2,
                        &opts.output,
                    );
                }
                if !opts.quiet {
                    eprintln!("Saved duplication baseline to {}", path.display());
                }
            }
            Err(e) => {
                return emit_error(
                    &format!("failed to serialize duplication baseline: {e}"),
                    2,
                    &opts.output,
                );
            }
        }
    }

    // Filter against baseline if provided
    if let Some(path) = opts.baseline_path {
        match std::fs::read_to_string(path) {
            Ok(json) => match serde_json::from_str::<DuplicationBaselineData>(&json) {
                Ok(baseline_data) => {
                    report = filter_new_clone_groups(report, &baseline_data, &config.root);
                }
                Err(e) => {
                    return emit_error(
                        &format!("failed to parse duplication baseline: {e}"),
                        2,
                        &opts.output,
                    );
                }
            },
            Err(e) => {
                return emit_error(
                    &format!("failed to read duplication baseline: {e}"),
                    2,
                    &opts.output,
                );
            }
        }
    }

    let elapsed = start.elapsed();

    // Print results
    let result =
        report::print_duplication_report(&report, &config, elapsed, opts.quiet, &opts.output);
    if result != ExitCode::SUCCESS {
        return result;
    }

    // Check threshold
    if opts.threshold > 0.0 && report.stats.duplication_percentage > opts.threshold {
        eprintln!(
            "Duplication ({:.1}%) exceeds threshold ({:.1}%)",
            report.stats.duplication_percentage, opts.threshold
        );
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}
