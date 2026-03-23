use super::common::{create_config, fixture_path};

#[test]
fn duplicate_code_detects_exact_clones() {
    let root = fixture_path("duplicate-code");
    let config = create_config(root.clone());
    let files = fallow_core::discover::discover_files(&config);

    let dupes_config = fallow_core::duplicates::DuplicatesConfig {
        min_tokens: 20,
        min_lines: 3,
        ..fallow_core::duplicates::DuplicatesConfig::default()
    };

    let report = fallow_core::duplicates::find_duplicates(&root, &files, &dupes_config);

    assert!(
        !report.clone_groups.is_empty(),
        "Should detect clones in duplicate-code fixture"
    );
    assert!(
        report.stats.files_with_clones >= 2,
        "At least 2 files should have clones"
    );
    assert!(
        report.stats.duplication_percentage > 0.0,
        "Duplication percentage should be > 0"
    );
}

#[test]
fn duplicate_code_semantic_mode_detects_type2_clones() {
    let root = fixture_path("duplicate-code");
    let config = create_config(root.clone());
    let files = fallow_core::discover::discover_files(&config);

    let dupes_config = fallow_core::duplicates::DuplicatesConfig {
        min_tokens: 20,
        min_lines: 3,
        mode: fallow_core::duplicates::DetectionMode::Semantic,
        ..fallow_core::duplicates::DuplicatesConfig::default()
    };

    let report = fallow_core::duplicates::find_duplicates(&root, &files, &dupes_config);

    // In semantic mode, copy2.ts (renamed variables) should also match
    let files_with_clones: rustc_hash::FxHashSet<_> = report
        .clone_groups
        .iter()
        .flat_map(|g| g.instances.iter())
        .map(|inst| inst.file.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(
        files_with_clones.contains("copy2.ts"),
        "Semantic mode should detect copy2.ts with renamed variables, files found: {files_with_clones:?}"
    );
}

#[test]
fn duplicate_code_unique_file_has_no_clones() {
    let root = fixture_path("duplicate-code");
    let config = create_config(root.clone());
    let files = fallow_core::discover::discover_files(&config);

    let dupes_config = fallow_core::duplicates::DuplicatesConfig {
        min_tokens: 20,
        min_lines: 3,
        ..fallow_core::duplicates::DuplicatesConfig::default()
    };

    let report = fallow_core::duplicates::find_duplicates(&root, &files, &dupes_config);

    // unique.ts should not appear in any clone group (its code is distinct)
    let all_clone_files: Vec<String> = report
        .clone_groups
        .iter()
        .flat_map(|g| g.instances.iter())
        .map(|inst| inst.file.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(
        !all_clone_files.contains(&"unique.ts".to_string()),
        "unique.ts should not appear in any clone group, found in: {all_clone_files:?}"
    );
}

#[test]
fn duplicate_code_json_output_serializable() {
    let root = fixture_path("duplicate-code");
    let config = create_config(root.clone());
    let files = fallow_core::discover::discover_files(&config);

    let dupes_config = fallow_core::duplicates::DuplicatesConfig {
        min_tokens: 20,
        min_lines: 3,
        ..fallow_core::duplicates::DuplicatesConfig::default()
    };

    let report = fallow_core::duplicates::find_duplicates(&root, &files, &dupes_config);

    // Should be serializable to JSON
    let json = serde_json::to_string_pretty(&report).expect("report should serialize to JSON");
    let reparsed: serde_json::Value = serde_json::from_str(&json).expect("JSON should be valid");
    assert!(reparsed["clone_groups"].is_array());
    assert!(reparsed["stats"]["total_files"].is_number());
}

#[test]
fn duplicate_code_skip_local_filters_same_directory() {
    let root = fixture_path("duplicate-code");
    let config = create_config(root.clone());
    let files = fallow_core::discover::discover_files(&config);

    let dupes_config = fallow_core::duplicates::DuplicatesConfig {
        min_tokens: 20,
        min_lines: 3,
        skip_local: true,
        ..fallow_core::duplicates::DuplicatesConfig::default()
    };

    let report = fallow_core::duplicates::find_duplicates(&root, &files, &dupes_config);

    // All fixture files are in the same directory (src/), so skip_local should filter them all
    assert!(
        report.clone_groups.is_empty(),
        "skip_local should filter same-directory clones"
    );
}

#[test]
fn duplicate_code_min_tokens_threshold_filters() {
    let root = fixture_path("duplicate-code");
    let config = create_config(root.clone());
    let files = fallow_core::discover::discover_files(&config);

    // Use very high min_tokens — should find no clones
    let dupes_config = fallow_core::duplicates::DuplicatesConfig {
        min_tokens: 10000,
        min_lines: 1,
        ..fallow_core::duplicates::DuplicatesConfig::default()
    };

    let report = fallow_core::duplicates::find_duplicates(&root, &files, &dupes_config);

    assert!(
        report.clone_groups.is_empty(),
        "Very high min_tokens should find no clones"
    );
}

#[test]
fn duplicate_code_find_duplicates_in_project_convenience() {
    let root = fixture_path("duplicate-code");

    let dupes_config = fallow_core::duplicates::DuplicatesConfig {
        min_tokens: 20,
        min_lines: 3,
        ..fallow_core::duplicates::DuplicatesConfig::default()
    };

    let report = fallow_core::duplicates::find_duplicates_in_project(&root, &dupes_config);

    assert!(
        !report.clone_groups.is_empty(),
        "Convenience function should detect clones"
    );
}
