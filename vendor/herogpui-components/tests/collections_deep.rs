//! Deeper collection behaviour: ListBox's `onAction` (activated, not selected)
//! and TagGroup's `disabledKeys`.
//!
//! The existing `collections.rs` drives ListBox single/multiple picks,
//! typeahead and disabled keys, plus TagGroup removal and roving focus. These
//! tests cover the halves v3 documents that the suite did not drive:
//!
//! - `onAction` is "Handler called when an item is activated", and v3's "With
//!   Disabled Items" and "With Sections" examples pair it with
//!   `selectionMode="none"` — a row must answer activation with no selection
//!   to report. The list here records `on_action` only, so a row that fired
//!   the selection path instead would stay silent.
//! - TagGroup's `disabledKeys` ("Keys of disabled tags") must make a tag take
//!   no click *and* drop out of the roving cursor, so the arrows skip it. Its
//!   React Aria keyboard delegate is horizontal, so Up and Down do nothing.
//!
//! Geometry is derived from the components' own constants, exactly as
//! `collections.rs` does:
//!
//! - ListBox row *i* centre: y = 4 + i*(36 + 4) + 18 = 22 + 40i
//!   (`.list-box` p-1, rows `min-h(util::FIELD_HEIGHT)` = 36px, `mt-1` gap).
//! - TagGroup: the `tag_content` slot draws a fixed 40x20 box, so a `--md`
//!   chip (px-2 py-1, no remove button, no icon) is 8+40+8 = 56px wide and
//!   28px tall; chips sit 6px apart, so chip *i* centre x = 28 + 62i, y = 14.

use crate::harness;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gpui::{prelude::*, px, TestAppContext};
use herogpui_components::{
    extend::FIELD_HEIGHT, Button, ListBox, ListBoxItem, Menu, MenuItem, SelectionMode, Tag,
    TagGroup,
};

use harness::{click, events, open_host, press};

/// The y centre of ListBox row `i`; see the module comment for the derivation.
fn list_row_centre(i: usize) -> f32 {
    let row = f32::from(FIELD_HEIGHT);
    4. + i as f32 * (row + 4.) + row / 2.
}

/// The keys of a selection joined in a stable order.
fn sorted_join(keys: &HashSet<gpui::SharedString>) -> String {
    let mut keys: Vec<String> = keys.iter().map(ToString::to_string).collect();
    keys.sort();
    keys.join(",")
}

// ---------------------------------------------------------------------------
// ListBox: onAction, separate from selection
// ---------------------------------------------------------------------------

#[gpui::test]
fn list_box_on_action_fires_when_selection_is_none(cx: &mut TestAppContext) {
    // v3: `onAction` is "Handler called when an item is activated", and the
    // "With Disabled Items" / "With Sections" examples pair it with
    // `selectionMode="none"`. The list here records `on_action` only, so
    // activation is the only thing that can report.
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        let selection_events = events.clone();
        ListBox::new(
            "lb-action",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
                ListBoxItem::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::None)
        .on_action(move |key, _, _| events.borrow_mut().push(key.to_string()))
        .on_selection_change(move |keys, _, _| {
            selection_events
                .borrow_mut()
                .push(format!("selection:{}", sorted_join(keys)));
        })
        .into_any_element()
    });

    // Rows 0 and 1 centre at y 22 and 62; clicking must report the activated
    // row's key with nothing to select.
    click(cx, 60., list_row_centre(0));
    click(cx, 60., list_row_centre(1));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "beta"],
        "clicking rows in selectionMode=none must activate them through onAction"
    );

    // The last click focused Beta, so Down advances to Gamma and the second
    // Down clamps there. Enter activates Gamma through the same callback. Had
    // activation gone to the selection path instead, there is nobody to record
    // it.
    press(cx, "down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "beta", "gamma"],
        "Enter must activate the row the arrows reached through onAction"
    );
}

/// Dropdown.Menu has the same action-only `selectionMode="none"` contract as
/// ListBox. Both pointer and Enter activation report `onAction`, while
/// `onSelectionChange` must remain silent because no selection exists.
#[gpui::test]
fn menu_on_action_fires_without_selection_in_none_mode(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let actions = events.clone();
        let selections = events.clone();
        Menu::new(
            "collections-action-menu",
            vec![MenuItem::new("one", "One"), MenuItem::new("two", "Two")],
        )
        .id("menu-action-none")
        .selection_mode(SelectionMode::None)
        .disabled_keys(["two"])
        .on_action(move |key, _, _| actions.borrow_mut().push(key.to_string()))
        .on_selection_change(move |keys, _, _| {
            selections.borrow_mut().push(format!(
                "selection:{}",
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        })
        .into_any_element()
    });

    click(cx, 60., 58.);
    assert!(
        recorded.borrow().is_empty(),
        "a disabled none-mode item must report neither action nor selection"
    );
    click(cx, 60., 22.);
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["one", "one"],
        "none mode must report only the pointer and keyboard actions"
    );
}

// ---------------------------------------------------------------------------
// TagGroup: disabledKeys
// ---------------------------------------------------------------------------

#[gpui::test]
fn tag_group_disabled_key_takes_no_click_and_is_skipped(cx: &mut TestAppContext) {
    // v3: `disabledKeys` is "Keys of disabled tags". A disabled tag must not
    // answer the pointer, and it must drop out of the roving cursor so the
    // arrows step over it. The caller owns the selection, as a controlled
    // TagGroup is used.
    let selection: Rc<RefCell<HashSet<gpui::SharedString>>> = Rc::new(RefCell::new(HashSet::new()));
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let selection = selection.clone();
        let events = events.clone();
        let held = selection.borrow().clone();
        TagGroup::new(
            "tg-disabled",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Single)
        .selected_keys(held)
        .disabled_keys(["beta".into()])
        .tag_content(|_, _| gpui::div().w(px(40.)).h(px(20.)).into_any_element())
        .on_selection_change(move |keys, _, _| {
            events.borrow_mut().push(sorted_join(keys));
            *selection.borrow_mut() = keys.clone();
        })
        .into_any_element()
    });

    // Chips centre at (28, 14) and (90, 14); the second is the disabled Beta.
    // Selecting Alpha and then clicking the disabled tag must report Alpha
    // once and then stop — no callback, no toggle, no replacement.
    click(cx, 28., 14.);
    click(cx, 90., 14.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha"],
        "a disabledKeys tag must take no click"
    );

    // Tab enters the group on Alpha; Right must land the roving cursor on
    // Gamma, skipping the disabled Beta. Enter then selects Gamma, which in
    // single mode *replaces* Alpha — the report "gamma" proves the arrow never
    // stopped at "beta" (a click that landed there would have said so).
    press(cx, "tab");
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "gamma"],
        "the arrow must skip the disabled tag: Right after Alpha lands on Gamma"
    );
}

/// React Aria builds TagGroup's keyboard delegate with horizontal orientation.
/// Down is therefore a cross-axis key: it must not move the roving stop or be
/// consumed as Right. Enter after Down still activates the first tag.
#[gpui::test]
fn tag_group_ignores_perpendicular_arrows(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "tg-horizontal-axis",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Single)
        .on_selection_change(move |keys, _, _| {
            recorded.borrow_mut().push(sorted_join(keys));
        })
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha"],
        "Down must leave a horizontal TagGroup's roving stop on Alpha"
    );
}

/// Deleting the last tag removes every enabled stop. The group must then leave
/// the tab order rather than retaining a focused handle that no tag claims.
#[gpui::test]
fn tag_group_delete_last_tag_leaves_the_tab_order(cx: &mut TestAppContext) {
    let tags = Rc::new(RefCell::new(vec![Tag::new("alpha", "Alpha")]));
    let recorded = events();
    let for_view = recorded.clone();
    let tags_for_view = tags;
    let cx = open_host(cx, move || {
        let tags = tags_for_view.clone();
        let removed = for_view.clone();
        let after = for_view.clone();
        let current = tags.borrow().clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(
                TagGroup::new("tg-delete-last", current).on_remove(move |keys, _, cx| {
                    removed
                        .borrow_mut()
                        .push(format!("remove:{}", sorted_join(keys)));
                    tags.borrow_mut().clear();
                    cx.refresh_windows();
                }),
            )
            .child(
                Button::new("tg-after")
                    .label("After")
                    .on_press(move |_, _, _| after.borrow_mut().push("after".to_owned())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "delete");
    cx.update(|window, _| window.refresh());
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["remove:alpha", "after"],
        "an empty TagGroup must yield Tab to the following control"
    );
}

/// `row_padding_y` grows the plain option row: 20px line + 2 x padding, with
/// the default 6px keeping the 36px `.list-box-item` floor.
#[gpui::test]
fn list_box_row_padding_y_sizes_the_option(cx: &mut TestAppContext) {
    const PAD: f32 = 12.;
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(16.))
            .child(
                ListBox::new(
                    "lb-row-default",
                    vec![
                        ListBoxItem::new("a", "Alpha"),
                        ListBoxItem::new("b", "Beta"),
                    ],
                )
                .into_any_element(),
            )
            .child(
                ListBox::new(
                    "lb-row-pad",
                    vec![
                        ListBoxItem::new("a", "Alpha"),
                        ListBoxItem::new("b", "Beta"),
                    ],
                )
                .row_padding_y(px(PAD))
                .into_any_element(),
            )
            .into_any_element()
    });

    let default = cx
        .debug_bounds("Name(\"lb-row-default\")-item-0")
        .expect("the default row must be laid out");
    let padded = cx
        .debug_bounds("Name(\"lb-row-pad\")-item-0")
        .expect("the padded row must be laid out");
    assert!(
        (f32::from(default.size.height) - 36.).abs() < 0.5,
        "the default row must keep the 36px floor, got {default:?}"
    );
    assert!(
        (f32::from(padded.size.height) - 44.).abs() < 0.5,
        "a 12px row padding must grow the 20px line to 44px, got {padded:?}"
    );
}

/// `row_padding_x` moves the label column inward by the delta over the
/// default 8px row inset (the list's own 4px padding sits outside both).
#[gpui::test]
fn list_box_row_padding_x_moves_the_label(cx: &mut TestAppContext) {
    const PAD: f32 = 24.;
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(16.))
            .child(
                ListBox::new(
                    "lb-row-x-default",
                    vec![
                        ListBoxItem::new("a", "Alpha"),
                        ListBoxItem::new("b", "Beta"),
                    ],
                )
                .into_any_element(),
            )
            .child(
                ListBox::new(
                    "lb-row-x-pad",
                    vec![
                        ListBoxItem::new("a", "Alpha"),
                        ListBoxItem::new("b", "Beta"),
                    ],
                )
                .row_padding_x(px(PAD))
                .into_any_element(),
            )
            .into_any_element()
    });

    let default = cx
        .debug_bounds("Name(\"lb-row-x-default\")-item-0-label")
        .expect("the default label must be laid out");
    let padded = cx
        .debug_bounds("Name(\"lb-row-x-pad\")-item-0-label")
        .expect("the padded label must be laid out");
    let moved = f32::from(padded.origin.x - default.origin.x);
    assert!(
        (moved - (PAD - 8.)).abs() < 0.5,
        "`row_padding_x` must move the label inward by the delta over the \
         default 8px inset, moved {moved}: default={default:?} padded={padded:?}"
    );
}
