use super::common::{create_config, fixture_path};

#[test]
fn barrel_exports_resolves_through_barrel() {
    let root = fixture_path("barrel-exports");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    let unused_export_names: Vec<&str> = results
        .unused_exports
        .iter()
        .map(|e| e.export_name.as_str())
        .collect();

    // fooUnused should be detected as unused (it's not re-exported from barrel)
    assert!(
        unused_export_names.contains(&"fooUnused"),
        "fooUnused should be unused, found: {unused_export_names:?}"
    );
}

// ── Barrel re-export unused detection ──────────────────────────

#[test]
fn barrel_unused_re_exports_detected() {
    let root = fixture_path("barrel-unused-reexports");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    let unused_export_names: Vec<&str> = results
        .unused_exports
        .iter()
        .map(|e| e.export_name.as_str())
        .collect();

    // UnusedComponent is re-exported from barrel but never imported by anyone
    assert!(
        unused_export_names.contains(&"UnusedComponent"),
        "UnusedComponent should be detected as unused re-export on barrel, found: {unused_export_names:?}"
    );

    // UsedComponent IS imported via barrel, so it should NOT be unused
    assert!(
        !unused_export_names.contains(&"UsedComponent"),
        "UsedComponent should NOT be detected as unused"
    );
}

#[test]
fn barrel_unused_type_re_exports_detected() {
    let root = fixture_path("barrel-unused-reexports");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    let unused_type_names: Vec<&str> = results
        .unused_types
        .iter()
        .map(|e| e.export_name.as_str())
        .collect();

    // UnusedType is re-exported as type from barrel but never imported
    assert!(
        unused_type_names.contains(&"UnusedType"),
        "UnusedType should be detected as unused type re-export on barrel, found: {unused_type_names:?}"
    );

    // UsedType IS imported via barrel, so it should NOT be unused
    assert!(
        !unused_type_names.contains(&"UsedType"),
        "UsedType should NOT be detected as unused type"
    );
}

#[test]
fn barrel_re_export_propagates_to_source_module() {
    // When a re-export on a barrel is unused, the source module's export
    // should also be flagged if only consumed through the (unused) barrel re-export.
    // Conversely, if the barrel re-export IS used, the source should NOT be flagged.
    let root = fixture_path("barrel-unused-reexports");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    // UsedComponent on the source module should NOT be flagged
    // (it's referenced through the barrel which is consumed)
    assert!(
        !results
            .unused_exports
            .iter()
            .any(|e| e.export_name == "UsedComponent"),
        "source UsedComponent should not be unused since barrel re-export is consumed"
    );
}

#[test]
fn barrel_exports_detects_unused_re_export_bar() {
    // In the existing barrel-exports fixture, `bar` is re-exported from barrel
    // but nobody imports `bar` from the barrel.
    let root = fixture_path("barrel-exports");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    let unused_export_names: Vec<&str> = results
        .unused_exports
        .iter()
        .map(|e| e.export_name.as_str())
        .collect();

    assert!(
        unused_export_names.contains(&"bar"),
        "bar should be detected as unused re-export on barrel (nobody imports it), found: {unused_export_names:?}"
    );

    // foo should not be flagged (it IS imported from barrel by index.ts)
    assert!(
        !unused_export_names.contains(&"foo"),
        "foo should NOT be unused since index.ts imports it from barrel"
    );
}

// ── Multi-hop barrel chains ────────────────────────────────────

#[test]
fn multi_hop_barrel_used_propagates() {
    let root = fixture_path("multi-hop-barrel");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    // `used` is imported through barrel1 -> barrel2 -> source, so it should NOT be flagged
    assert!(
        !results
            .unused_exports
            .iter()
            .any(|e| e.export_name == "used"),
        "used should propagate through barrel chain and NOT be flagged"
    );
}

#[test]
fn multi_hop_barrel_unused_detected() {
    let root = fixture_path("multi-hop-barrel");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    let unused_export_names: Vec<&str> = results
        .unused_exports
        .iter()
        .map(|e| e.export_name.as_str())
        .collect();

    // unused2 is only exported from source.ts and re-exported from barrel2
    // but NOT re-exported from barrel1, so it should be flagged
    assert!(
        unused_export_names.contains(&"unused2"),
        "unused2 should be detected as unused export, found: {unused_export_names:?}"
    );
}

// ── Star re-export chains ──────────────────────────────────────

#[test]
fn star_re_export_chain_used_propagates() {
    let root = fixture_path("star-re-export-chain");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    let unused_export_names: Vec<&str> = results
        .unused_exports
        .iter()
        .map(|e| e.export_name.as_str())
        .collect();

    // `used` is imported through barrel1 (export *) -> barrel2 (export *) -> source
    assert!(
        !unused_export_names.contains(&"used"),
        "used should propagate through star re-export chain and NOT be flagged, found: {unused_export_names:?}"
    );
}

#[test]
fn star_re_export_chain_unused_detected() {
    let root = fixture_path("star-re-export-chain");
    let config = create_config(root);
    let results = fallow_core::analyze(&config).expect("analysis should succeed");

    let unused_export_names: Vec<&str> = results
        .unused_exports
        .iter()
        .map(|e| e.export_name.as_str())
        .collect();

    // `unused` is exported from source.ts but never imported
    assert!(
        unused_export_names.contains(&"unused"),
        "unused should be detected as unused export, found: {unused_export_names:?}"
    );
}
