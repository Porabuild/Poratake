//! Source-shape scanning shared by the customisation wiring tests.
//!
//! These helpers prove *wiring*, not drawn pixels: for a named consumer they
//! find the enclosing top-level function and check a reader appears inside it.
//! Scoping to the enclosing function is the point — a sibling's call must not
//! satisfy an entry. The fixtures in `tests/sx_ownership.rs` pin that behavior
//! with positive and negative snippets.

#![allow(dead_code)]

use std::path::Path;

/// Reads one file from the component crate's `src/` tree.
pub fn component_src(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", path.display()))
}

/// Asserts `source` has exactly one shared field-chrome call and that it sits
/// inside `guard` (the complete `if ... {` line). Every focus-ring variant --
/// the shadow one, the overlay one, and the ringless one a clipping shell
/// takes while its carrier paints the ring -- is counted alongside the stock
/// helper because they all share the same chrome call site.
///
/// Counting appearances alone would pass if the call moved outside the guard;
/// scoping to the guarded block binds the call to the condition the builder
/// actually controls. Pass the full condition so compound guards work.
pub fn assert_chrome_call_is_under(source: &str, guard: &str) {
    let calls = source.matches("apply_field_chrome(").count()
        + source
            .matches("apply_field_chrome_with_focus_ring(")
            .count()
        + source.matches("apply_field_chrome_overlay(").count()
        + source.matches("apply_field_chrome_ringless(").count();
    assert_eq!(
        calls, 1,
        "the component must have exactly one chrome call site, found {calls}"
    );
    let guard_at = source
        .find(guard)
        .unwrap_or_else(|| panic!("the chrome call must be under `{guard}`"));
    let body_at = guard_at + guard.len();
    let body_len = guard_body_len(&source[body_at..]);
    assert!(
        source[body_at..body_at + body_len].contains("apply_field_chrome(")
            || source[body_at..body_at + body_len].contains("apply_field_chrome_with_focus_ring(")
            || source[body_at..body_at + body_len].contains("apply_field_chrome_overlay(")
            || source[body_at..body_at + body_len].contains("apply_field_chrome_ringless("),
        "the only chrome call must sit inside the `{guard}` guard"
    );
}

fn guard_body_len(after_open: &str) -> usize {
    let mut depth = 1usize;
    for (index, ch) in after_open.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return index;
                }
            }
            _ => {}
        }
    }
    panic!("the guard block must close");
}

/// The top-level function containing the first occurrence of `marker`.
///
/// The crate is rustfmt-formatted, so a function's body ends at the first
/// following line that is exactly its own indentation plus `}`.
pub fn enclosing_function<'a>(source: &'a str, marker: &str) -> Option<&'a str> {
    let marker_index = source.find(marker)?;
    let line_start = source[..marker_index].rfind('\n').map_or(0, |i| i + 1);
    let mut start = None;
    if starts_function(&source[line_start..]) {
        start = Some(line_start);
    } else {
        let mut offset = 0;
        for line in source[..line_start].split_inclusive('\n') {
            if starts_function(line) {
                start = Some(offset);
            }
            offset += line.len();
        }
    }
    let start = start?;
    let indent_len = source[start..].len() - source[start..].trim_start().len();
    let indent = &source[start..start + indent_len];
    let rest = &source[start..];
    let close = format!("\n{indent}}}");
    let end = rest.find(&close).map(|index| start + index + 1)?;
    Some(&source[start..end])
}

fn starts_function(line: &str) -> bool {
    let line = line.trim_start();
    let line = line
        .strip_prefix("pub(crate) ")
        .or_else(|| line.strip_prefix("pub(super) "))
        .or_else(|| line.strip_prefix("pub "))
        .or_else(|| line.strip_prefix("async "))
        .unwrap_or(line);
    line.starts_with("fn ")
}

/// `Ok` when the function that owns `marker` also reads `needle`.
pub fn scope_contains(source: &str, marker: &str, needle: &str) -> Result<(), String> {
    let scope = enclosing_function(source, marker)
        .ok_or_else(|| format!("no function contains marker {marker:?}"))?;
    if scope.contains(needle) {
        Ok(())
    } else {
        Err(format!(
            "the function owning {marker:?} does not read {needle:?}"
        ))
    }
}
