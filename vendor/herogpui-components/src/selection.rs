//! The shared `selectionMode` semantics.
//!
//! Toggle groups, dropdown menus, tables and pickers all answer the same
//! question when an item is activated: what is the selection now? Keeping one
//! implementation means they cannot disagree.

use gpui::SharedString;
use herogpui_core::{SelectionBehavior, SelectionMode};

/// What an unmodified Escape press does in a selectable collection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EscapeKeyBehavior {
    /// Clear a nonempty selection and consume the key.
    #[default]
    ClearSelection,
    /// Leave selection unchanged and let the key bubble.
    None,
}

/// Whether activating an item in `mode` has a selection change to report.
pub(crate) fn reports_changes(mode: SelectionMode) -> bool {
    mode != SelectionMode::None
}

/// Normalizes a keyed selection to the shape of pinned react-stately 3.49.0's
/// `selectedKeys`: a JS `Set`, which collapses duplicates to the first
/// insertion and iterates in insertion order. A single-mode selection holds at
/// most one key — the first the owner (or default) listed.
pub(crate) fn normalize_selection(keys: Vec<SharedString>, multiple: bool) -> Vec<SharedString> {
    let mut out: Vec<SharedString> = Vec::with_capacity(keys.len());
    for key in keys {
        if out.contains(&key) {
            continue;
        }
        out.push(key);
        if !multiple {
            break;
        }
    }
    out
}

/// Toggles `key` in an ordered selection: removing takes it out in place so
/// the remaining keys keep their insertion order; adding appends — the
/// mutation a JS `Set` performs.
pub(crate) fn toggle_key(selection: &mut Vec<SharedString>, key: &SharedString) {
    if let Some(at) = selection.iter().position(|k| k == key) {
        selection.remove(at);
    } else {
        selection.push(key.clone());
    }
}

/// The selection after activating `key`.
///
/// `Single` collapses to just `key` (or clears it, unless
/// `disallow_empty_selection`); `Multiple` toggles membership; `None` is inert.
pub fn next_selection(
    current: &[SharedString],
    key: &SharedString,
    mode: SelectionMode,
    disallow_empty: bool,
) -> Vec<SharedString> {
    let is_selected = current.iter().any(|k| k == key);
    match mode {
        SelectionMode::None => current.to_vec(),
        SelectionMode::Single => {
            if is_selected && !disallow_empty {
                Vec::new()
            } else {
                vec![key.clone()]
            }
        }
        SelectionMode::Multiple => {
            if is_selected {
                if disallow_empty && current.len() == 1 {
                    return current.to_vec();
                }
                current.iter().filter(|k| *k != key).cloned().collect()
            } else {
                let mut next = current.to_vec();
                next.push(key.clone());
                next
            }
        }
    }
}

/// The selection after an activation with React Stately's
/// `selectionBehavior` applied.
///
/// A single-select collection always uses its single-item toggle rule. In a
/// multi-select collection, `Toggle` preserves the existing set while
/// `Replace` selects only the activated key. Callers that received Shift or a
/// platform non-contiguous modifier should use their range/toggle path before
/// calling this helper, just as React Aria does.
pub(crate) fn next_selection_with_behavior(
    current: &[SharedString],
    key: &SharedString,
    mode: SelectionMode,
    behavior: SelectionBehavior,
    disallow_empty: bool,
) -> Vec<SharedString> {
    match (mode, behavior) {
        (SelectionMode::Multiple, SelectionBehavior::Replace) => vec![key.clone()],
        _ => next_selection(current, key, mode, disallow_empty),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(items: &[&str]) -> Vec<SharedString> {
        items
            .iter()
            .map(|s| SharedString::from(s.to_string()))
            .collect()
    }

    #[test]
    fn normalize_keeps_first_insertion_order_and_collapses_duplicates() {
        assert_eq!(
            normalize_selection(keys(&["b", "a", "b", "c"]), true),
            keys(&["b", "a", "c"]),
            "duplicates collapse to their first insertion; the rest keep \
             insertion order"
        );
        assert_eq!(
            normalize_selection(keys(&["a", "b"]), false),
            keys(&["a"]),
            "single mode keeps only the first listed key"
        );
    }

    #[test]
    fn toggle_removes_in_place_and_appends() {
        let mut selection = keys(&["a", "b", "c"]);
        toggle_key(&mut selection, &SharedString::from("b"));
        assert_eq!(selection, keys(&["a", "c"]));
        toggle_key(&mut selection, &SharedString::from("d"));
        assert_eq!(selection, keys(&["a", "c", "d"]));
    }

    #[test]
    fn only_selection_modes_report_changes() {
        assert!(!reports_changes(SelectionMode::None));
        assert!(reports_changes(SelectionMode::Single));
        assert!(reports_changes(SelectionMode::Multiple));
    }

    #[test]
    fn single_replaces() {
        let next = next_selection(
            &keys(&["a"]),
            &SharedString::from("b"),
            SelectionMode::Single,
            false,
        );
        assert_eq!(next, keys(&["b"]));
    }

    #[test]
    fn single_clears_on_reselect() {
        let next = next_selection(
            &keys(&["a"]),
            &SharedString::from("a"),
            SelectionMode::Single,
            false,
        );
        assert!(next.is_empty());
    }

    #[test]
    fn single_keeps_when_empty_disallowed() {
        let next = next_selection(
            &keys(&["a"]),
            &SharedString::from("a"),
            SelectionMode::Single,
            true,
        );
        assert_eq!(next, keys(&["a"]));
    }

    #[test]
    fn multiple_toggles() {
        let added = next_selection(
            &keys(&["a"]),
            &SharedString::from("b"),
            SelectionMode::Multiple,
            false,
        );
        assert_eq!(added, keys(&["a", "b"]));
        let removed = next_selection(
            &keys(&["a", "b"]),
            &SharedString::from("a"),
            SelectionMode::Multiple,
            false,
        );
        assert_eq!(removed, keys(&["b"]));
    }

    #[test]
    fn multiple_keeps_last_when_empty_disallowed() {
        let next = next_selection(
            &keys(&["a"]),
            &SharedString::from("a"),
            SelectionMode::Multiple,
            true,
        );
        assert_eq!(next, keys(&["a"]));
        let next = next_selection(
            &keys(&["a", "b"]),
            &SharedString::from("a"),
            SelectionMode::Multiple,
            true,
        );
        assert_eq!(next, keys(&["b"]));
    }

    #[test]
    fn none_is_inert() {
        let next = next_selection(
            &keys(&["a"]),
            &SharedString::from("b"),
            SelectionMode::None,
            false,
        );
        assert_eq!(next, keys(&["a"]));
    }

    #[test]
    fn replace_behavior_collapses_a_multiple_selection() {
        let next = next_selection_with_behavior(
            &keys(&["a", "b"]),
            &SharedString::from("c"),
            SelectionMode::Multiple,
            SelectionBehavior::Replace,
            false,
        );
        assert_eq!(next, keys(&["c"]));
    }

    #[test]
    fn replace_behavior_keeps_the_activated_key_selected() {
        let next = next_selection_with_behavior(
            &keys(&["a", "b"]),
            &SharedString::from("b"),
            SelectionMode::Multiple,
            SelectionBehavior::Replace,
            false,
        );
        assert_eq!(next, keys(&["b"]));
    }
}
