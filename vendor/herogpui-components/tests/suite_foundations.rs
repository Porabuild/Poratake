//! Integration tests grouped by feature area to keep link time bounded.

#[path = "harness/mod.rs"]
mod harness;
#[path = "source_scan/mod.rs"]
mod source_scan;

#[path = "a11y_collections_deep.rs"]
mod a11y_collections_deep;
#[path = "a11y_deep.rs"]
mod a11y_deep;
#[path = "a11y_overlays_deep.rs"]
mod a11y_overlays_deep;
#[path = "a11y_pickers_deep.rs"]
mod a11y_pickers_deep;
#[path = "capability_hooks_deep.rs"]
mod capability_hooks_deep;
#[path = "cursor_token.rs"]
mod cursor_token;
#[path = "focus_ring_overlay.rs"]
mod focus_ring_overlay;
#[path = "focus_root.rs"]
mod focus_root;
#[path = "focus_visible_deep.rs"]
mod focus_visible_deep;
#[path = "i18n.rs"]
mod i18n;
#[path = "migration_extensions.rs"]
mod migration_extensions;
#[path = "theme_platform.rs"]
mod theme_platform;
#[path = "theme_repaint.rs"]
mod theme_repaint;
#[path = "theme_tokens.rs"]
mod theme_tokens;
#[path = "wasm_portability.rs"]
mod wasm_portability;

#[test]
fn every_integration_source_belongs_to_exactly_one_suite() {
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let suites = [
        "collections",
        "controls",
        "feedback",
        "fields",
        "foundations",
        "overlays",
        "pickers",
        "styling",
        "tables_layout",
    ];
    let mut owners = HashMap::<String, String>::new();
    for suite in suites {
        let source = fs::read_to_string(tests.join(format!("suite_{suite}.rs"))).unwrap();
        for line in source.lines() {
            let Some(name) = line
                .strip_prefix("#[path = \"")
                .and_then(|line| line.strip_suffix("\"]"))
            else {
                continue;
            };
            assert!(tests.join(name).is_file(), "{suite} names missing {name}");
            if matches!(name, "harness/mod.rs" | "source_scan/mod.rs") {
                continue;
            }
            let previous = owners.insert(name.to_owned(), suite.to_owned());
            assert!(
                previous.is_none(),
                "{name} appears in both {previous:?} and {suite}"
            );
        }
    }
    for entry in fs::read_dir(&tests).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".rs") || name.starts_with("suite_") {
            continue;
        }
        assert!(
            owners.contains_key(&name),
            "{name} has no integration suite"
        );
    }
}

#[path = "control_slots.rs"]
mod control_slots;
