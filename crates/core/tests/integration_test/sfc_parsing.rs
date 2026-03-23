use super::common::{create_config, fixture_path};

// ── Vue SFC parsing ────────────────────────────────────────────

#[test]
fn vue_project_discovers_vue_files() {
    let root = fixture_path("vue-project");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    let unused_file_names: Vec<String> = results
        .unused_files
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    // App.vue is imported by main.ts, should NOT be unused
    assert!(
        !unused_file_names.contains(&"App.vue".to_string()),
        "App.vue should be reachable via import from main.ts, unused: {unused_file_names:?}"
    );

    // Orphan.vue is not imported by anything, should be unused
    assert!(
        unused_file_names.contains(&"Orphan.vue".to_string()),
        "Orphan.vue should be detected as unused file, found: {unused_file_names:?}"
    );
}

#[test]
fn vue_imports_mark_exports_used() {
    let root = fixture_path("vue-project");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    let unused_export_names: Vec<&str> = results
        .unused_exports
        .iter()
        .map(|e| e.export_name.as_str())
        .collect();

    // formatDate is imported inside App.vue <script>, should be used
    assert!(
        !unused_export_names.contains(&"formatDate"),
        "formatDate should be used (imported in App.vue), found: {unused_export_names:?}"
    );

    // unusedUtil is not imported anywhere, should be unused
    assert!(
        unused_export_names.contains(&"unusedUtil"),
        "unusedUtil should be detected as unused export, found: {unused_export_names:?}"
    );
}

// ── Svelte SFC parsing ─────────────────────────────────────────

#[test]
fn svelte_project_discovers_svelte_files() {
    let root = fixture_path("svelte-project");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    let unused_file_names: Vec<String> = results
        .unused_files
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    // App.svelte is imported by main.ts, should NOT be unused
    assert!(
        !unused_file_names.contains(&"App.svelte".to_string()),
        "App.svelte should be reachable via import from main.ts, unused: {unused_file_names:?}"
    );

    // Orphan.svelte is not imported, should be unused
    assert!(
        unused_file_names.contains(&"Orphan.svelte".to_string()),
        "Orphan.svelte should be detected as unused file, found: {unused_file_names:?}"
    );
}

#[test]
fn svelte_imports_mark_exports_used() {
    let root = fixture_path("svelte-project");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    let unused_export_names: Vec<&str> = results
        .unused_exports
        .iter()
        .map(|e| e.export_name.as_str())
        .collect();

    // formatName is imported inside App.svelte, should be used
    assert!(
        !unused_export_names.contains(&"formatName"),
        "formatName should be used (imported in App.svelte), found: {unused_export_names:?}"
    );

    // unusedUtil is not imported anywhere, should be unused
    assert!(
        unused_export_names.contains(&"unusedUtil"),
        "unusedUtil should be detected as unused export, found: {unused_export_names:?}"
    );
}
