//! What wave 3 of the accessibility contract changed, and what can be checked.
//!
//! Read `a11y_deep.rs` first: it pins the fact that the AccessKit tree itself
//! is *not* observable from the headless platform, and that still holds here.
//! `a11y_overlays_deep.rs` is the sibling for wave 2. Wave 5 — calendars and
//! overlay-backed fields — is `a11y_pickers_deep.rs`. So these tests assert
//! the other half — the half a role change can actually break.
//!
//! # Why a collection is the sharpest case yet
//!
//! An AccessKit node id is a hash of the element's `GlobalElementId`, and that
//! id path is *also* gpui's key for per-element state: the hover slot, the
//! press latch, the scroll offset, the focus handle. Wave 3 gave a long list
//! of containers an element id where they had none, because an element with no
//! id produces no node and a role set on it is silently dropped:
//!
//! - `Tabs`' tab list (`{id}-tablist`) and its open panel (`{id}-tabpanel`),
//! - `Toolbar`'s root, when the caller named it,
//! - `Breadcrumbs`' bar and each crumb's row (`{id}-item-{i}`),
//! - `Pagination`'s bar,
//! - `TagGroup`'s tag list (`{id}-list`),
//! - `Table`'s grid, header row, body and *every body cell*
//!   (`{id}-row-{i}-cell-{c}`),
//! - `Autocomplete`'s virtual list wrapper (`{id}-list`),
//! - `TagGroup`'s remove buttons and `Table`'s select-all cell, which gained
//!   both an id and a role.
//!
//! Every one of those nests its existing descendants one segment deeper. A
//! collection multiplies the damage a collision does, because the thing that
//! collides is not one widget but every row, cell and button inside it: two
//! tables that folded together would share one press latch *per cell* while
//! drawing perfectly.
//!
//! Each test therefore drives *two* independent instances and asserts they
//! answer separately, per row where the component has rows. Keyboard wherever
//! the component has a tab stop, so no assertion depends on a measured
//! coordinate.
//!
//! # The one component that reports nothing, on purpose
//!
//! [`an_unnamed_toolbar_still_presses_its_children`] pins the deliberate gap:
//! `Toolbar::new()` takes no id, so it states no role, because the constant
//! its keyed focus scope falls back to would fold every unnamed toolbar in the
//! window into one AccessKit node. It must still work.

use crate::harness;

use std::collections::HashSet;

use gpui::{prelude::*, px, SharedString, TestAppContext};
use herogpui_components::{
    extend::FIELD_HEIGHT, Autocomplete, Breadcrumbs, Button, Crumb, InputState, ListBox,
    ListBoxItem, Pagination, PickerItem, TabItem, Table, TableColumn, Tabs, Tag, TagGroup, Toolbar,
};
use herogpui_core::SelectionMode;

use harness::{click, events, open_host, press};

/// A pair of collections side by side, each in its own fixed-width column so a
/// click coordinate is arithmetic rather than a guess.
fn side_by_side(left: gpui::AnyElement, right: gpui::AnyElement) -> gpui::AnyElement {
    gpui::div()
        .flex()
        .flex_row()
        .child(gpui::div().w(px(COLUMN)).child(left))
        .child(gpui::div().w(px(COLUMN)).child(right))
        .into_any_element()
}

const COLUMN: f32 = 320.;

/// The keys of a selection joined in a stable order.
fn sorted_join(keys: &HashSet<SharedString>) -> String {
    let mut keys: Vec<String> = keys.iter().map(ToString::to_string).collect();
    keys.sort();
    keys.join(",")
}

// ---------------------------------------------------------------------------
// ListBox — every option row took a role, a name and a selected flag
// ---------------------------------------------------------------------------

/// Each option row already had `{id}-item-{i}`, and wave 3 hung
/// `role="option"`, `aria-selected` and (on the virtual paths) the set
/// position off it. Two lists holding the *same item keys* must still report
/// their own rows: the only thing keeping them apart is the list id at the
/// head of every row's path.
#[gpui::test]
fn two_list_boxes_choose_from_their_own_rows(cx: &mut TestAppContext) {
    harness::still();
    let chosen = events();
    let recorded = chosen.clone();
    let cx = open_host(cx, move || {
        let left = chosen.clone();
        let right = chosen.clone();
        let items = || {
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
            ]
        };
        side_by_side(
            ListBox::new("left-list", items())
                .selection_mode(SelectionMode::Single)
                .on_selection_change(move |keys, _, _| {
                    left.borrow_mut()
                        .push(format!("left:{}", sorted_join(keys)));
                })
                .into_any_element(),
            ListBox::new("right-list", items())
                .selection_mode(SelectionMode::Single)
                .on_selection_change(move |keys, _, _| {
                    right
                        .borrow_mut()
                        .push(format!("right:{}", sorted_join(keys)));
                })
                .into_any_element(),
        )
    });

    // `.list-box` is `p-1` (4px) around rows of `min-h(FIELD_HEIGHT)` with
    // `mt-1` (4px) between them, so row *i*'s centre is 4 + i*(h+4) + h/2.
    let row = f32::from(FIELD_HEIGHT);
    let centre = |i: usize| 4. + i as f32 * (row + 4.) + row / 2.;

    click(cx, 60., centre(0));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:alpha"],
        "the left list's first row must select in the left list alone"
    );

    // The *same key* in the other list. A shared row path would report
    // `left:beta` here, or nothing at all.
    click(cx, COLUMN + 60., centre(1));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:alpha", "right:beta"],
        "the right list's second row must answer as the right list's own"
    );
}

// ---------------------------------------------------------------------------
// Tabs — the list and the panel both took ids
// ---------------------------------------------------------------------------

/// `Tabs` gained `{id}-tablist` around the tabs and `{id}-tabpanel` around the
/// open panel, so every tab and everything the caller composed into a panel
/// now sits one segment deeper. The tab list is a single roving tab stop, so
/// two tab bars are two stops, and an arrow must only move the bar it is in.
#[gpui::test]
fn two_tab_bars_switch_independently(cx: &mut TestAppContext) {
    harness::still();
    let switched = events();
    let recorded = switched.clone();
    let cx = open_host(cx, move || {
        let left = switched.clone();
        let right = switched.clone();
        let items = || vec![TabItem::new("one", "One"), TabItem::new("two", "Two")];
        side_by_side(
            Tabs::new("left-tabs", items(), "one")
                .on_selection_change(move |key: &SharedString, _, _| {
                    left.borrow_mut().push(format!("left:{key}"));
                })
                .into_any_element(),
            Tabs::new("right-tabs", items(), "one")
                .on_selection_change(move |key: &SharedString, _, _| {
                    right.borrow_mut().push(format!("right:{key}"));
                })
                .into_any_element(),
        )
    });

    // No panel content is composed, so the only stops on the page are the two
    // tab lists. The default `KeyboardActivation::Automatic` selects as the
    // arrow moves.
    press(cx, "tab");
    press(cx, "right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:two"],
        "the first stop is the left tab list, and Right must move only it"
    );

    press(cx, "tab");
    press(cx, "right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:two", "right:two"],
        "the second stop must be the right tab list: two bars, two roving \
         stops, or the two share one selection"
    );
}

// ---------------------------------------------------------------------------
// Toolbar — a root that reports a node only when it was named
// ---------------------------------------------------------------------------

/// A named toolbar's root took an id for `role="toolbar"`, which nests every
/// control the caller composed inside it. Two named toolbars must keep their
/// own children: Tab leaves a toolbar entirely, so one press per toolbar walks
/// from the first to the second.
#[gpui::test]
fn two_named_toolbars_press_their_own_children(cx: &mut TestAppContext) {
    harness::still();
    let pressed = events();
    let recorded = pressed.clone();
    let cx = open_host(cx, move || {
        let left = pressed.clone();
        let right = pressed.clone();
        side_by_side(
            Toolbar::new()
                .id("left-toolbar")
                .child(
                    Button::new("bold")
                        .label("Bold")
                        .on_press(move |_, _, _| left.borrow_mut().push("left".to_owned()))
                        .into_any_element(),
                )
                .into_any_element(),
            Toolbar::new()
                .id("right-toolbar")
                .child(
                    Button::new("bold")
                        .label("Bold")
                        .on_press(move |_, _, _| right.borrow_mut().push("right".to_owned()))
                        .into_any_element(),
                )
                .into_any_element(),
        )
    });

    // Both buttons carry the *same* caller id. Only the toolbar id above them
    // keeps their element paths — and therefore their press latches and their
    // a11y nodes — apart.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left"],
        "the first stop is the left toolbar's button"
    );

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left", "right"],
        "two buttons with one id must stay two buttons under two toolbars"
    );
}

/// The recorded gap: `Toolbar::new()` takes no id, so it states no role and
/// contributes no node. That is deliberate — a constant id would fold every
/// unnamed toolbar into one — and it must not have cost the toolbar anything
/// it had before.
#[gpui::test]
fn an_unnamed_toolbar_still_presses_its_children(cx: &mut TestAppContext) {
    harness::still();
    let pressed = events();
    let recorded = pressed.clone();
    let cx = open_host(cx, move || {
        let seen = pressed.clone();
        Toolbar::new()
            .child(
                Button::new("unnamed-bold")
                    .label("Bold")
                    .on_press(move |_, _, _| seen.borrow_mut().push("pressed".to_owned()))
                    .into_any_element(),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["pressed"],
        "an id-less toolbar reports no accessibility node, but its children \
         must still take the focus and the press"
    );
}

// ---------------------------------------------------------------------------
// Breadcrumbs — the bar and each crumb's row took ids
// ---------------------------------------------------------------------------

/// `Breadcrumbs` gained an id on the bar (`role="list"`) and on each crumb's
/// row (`role="listitem"`), so every crumb link now hangs two segments below
/// the bar instead of one. Two bars carrying the *same labels* must still
/// navigate their own crumbs.
#[gpui::test]
fn two_breadcrumb_bars_navigate_their_own_crumbs(cx: &mut TestAppContext) {
    harness::still();
    let navigated = events();
    let recorded = navigated.clone();
    let cx = open_host(cx, move || {
        let left = navigated.clone();
        let right = navigated.clone();
        let items = || {
            vec![
                Crumb::new("Build"),
                Crumb::new("Deploy"),
                Crumb::new("Live"),
            ]
        };
        side_by_side(
            Breadcrumbs::new(items())
                .id("left-crumbs")
                .on_navigate(move |index, crumb, _, _, _| {
                    left.borrow_mut()
                        .push(format!("left:{index}:{}", crumb.label));
                })
                .into_any_element(),
            Breadcrumbs::new(items())
                .id("right-crumbs")
                .on_navigate(move |index, crumb, _, _, _| {
                    right
                        .borrow_mut()
                        .push(format!("right:{index}:{}", crumb.label));
                })
                .into_any_element(),
        )
    });

    // The last crumb is the current page and is inert upstream and here, so
    // each bar contributes two tab stops.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(recorded.borrow().as_slice(), ["left:0:Build"]);

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:0:Build", "left:1:Deploy"],
        "the second stop is still inside the left bar"
    );

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:0:Build", "left:1:Deploy", "right:0:Build"],
        "the third stop must be the right bar's first crumb, reporting its \
         own index against its own bar"
    );
}

// ---------------------------------------------------------------------------
// Pagination — the bar took an id for role="navigation"
// ---------------------------------------------------------------------------

/// `Pagination`'s root took an id, which nests every page cell and both nav
/// arrows one segment deeper. Two bars must page independently.
#[gpui::test]
fn two_paginations_page_independently(cx: &mut TestAppContext) {
    harness::still();
    let paged = events();
    let recorded = paged.clone();
    let cx = open_host(cx, move || {
        let left = paged.clone();
        let right = paged.clone();
        side_by_side(
            Pagination::new("left-pager", 1, 5)
                .on_change(move |page, _, _| left.borrow_mut().push(format!("left:{page}")))
                .into_any_element(),
            Pagination::new("right-pager", 1, 5)
                .on_change(move |page, _, _| right.borrow_mut().push(format!("right:{page}")))
                .into_any_element(),
        )
    });

    // Medium metrics: the previous arrow spans x 0..34, then a 4px gap and
    // 32px page cells — page 1 at 38..70, page 2 at 74..106 — on a 32px row,
    // so y 16 is inside every one of them.
    click(cx, 90., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:2"],
        "the left bar's page 2 must report against the left bar"
    );

    click(cx, COLUMN + 90., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:2", "right:2"],
        "the same cell in the right bar must be a different element"
    );
}

// ---------------------------------------------------------------------------
// TagGroup — the tag list took an id for role="grid"
// ---------------------------------------------------------------------------

/// `TagGroup`'s tag list took `{id}-list` for `role="grid"`, so every tag row
/// and every remove button moved one segment deeper. The group is one roving
/// tab stop with its own keyed cursor, so two groups are two stops and Enter
/// must select in the group the cursor is actually in.
///
/// No `on_remove` here on purpose: a removable tag mounts a second focusable
/// element inside the row, and this test is about the group's own stop.
#[gpui::test]
fn two_tag_groups_select_their_own_tags(cx: &mut TestAppContext) {
    harness::still();
    let picked = events();
    let recorded = picked.clone();
    let cx = open_host(cx, move || {
        let left = picked.clone();
        let right = picked.clone();
        let tags = || vec![Tag::new("alpha", "Alpha"), Tag::new("beta", "Beta")];
        side_by_side(
            TagGroup::new("left-tags", tags())
                .selection_mode(SelectionMode::Single)
                .on_selection_change(move |keys, _, _| {
                    left.borrow_mut()
                        .push(format!("left:{}", sorted_join(keys)));
                })
                .into_any_element(),
            TagGroup::new("right-tags", tags())
                .selection_mode(SelectionMode::Single)
                .on_selection_change(move |keys, _, _| {
                    right
                        .borrow_mut()
                        .push(format!("right:{}", sorted_join(keys)));
                })
                .into_any_element(),
        )
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:beta"],
        "the first stop is the left group, and its cursor must walk its own \
         tags"
    );

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:beta", "right:alpha"],
        "the second stop must be the right group, with its own cursor seated \
         on its own first tag: a shared list path would carry the left \
         group's cursor across"
    );
}

// ---------------------------------------------------------------------------
// Table — the grid, the header, the body and every cell took ids
// ---------------------------------------------------------------------------

/// This is the case wave 3 exists for. `Table` gained ids on the grid, the
/// header row, the body and — new — on **every body cell**
/// (`{id}-row-{i}-cell-{c}`), so a collision would not share one node but one
/// node per cell. Two tables must keep a row cursor each.
#[gpui::test]
fn two_tables_activate_their_own_rows(cx: &mut TestAppContext) {
    harness::still();
    let clicked = events();
    let recorded = clicked.clone();
    let cx = open_host(cx, move || {
        let left = clicked.clone();
        let right = clicked.clone();
        let build = |id: &'static str, tag: &'static str, sink: harness::Events| {
            Table::new(vec![])
                .id(id)
                .column(TableColumn::new("Name").default_width(px(200.)))
                .keyed_row("first", vec![gpui::div().child("First").into_any_element()])
                .keyed_row(
                    "second",
                    vec![gpui::div().child("Second").into_any_element()],
                )
                .on_row_click(move |index, _, _, _| {
                    sink.borrow_mut().push(format!("{tag}:{index}"));
                })
                .into_any_element()
        };
        side_by_side(
            build("left-table", "left", left),
            build("right-table", "right", right),
        )
    });

    // A table is one tab stop with a roving row cursor: Down seats it, a
    // second Down moves it, Enter activates the row it is on.
    press(cx, "tab");
    press(cx, "down");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:1"],
        "the first stop is the left table, whose cursor must walk its own rows"
    );

    press(cx, "tab");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:1", "right:0"],
        "the right table must start its own cursor at its own first row, not \
         inherit the left table's"
    );
}

/// The header row and the body are separate nodes now (`role="row"` and
/// `role="rowgroup"`), and each header cell is a `columnheader` that is also a
/// keyboard stop. Two tables' headers must sort their own table.
#[gpui::test]
fn two_tables_sort_from_their_own_headers(cx: &mut TestAppContext) {
    harness::still();
    let sorted = events();
    let recorded = sorted.clone();
    let cx = open_host(cx, move || {
        let left = sorted.clone();
        let right = sorted.clone();
        let build = |id: &'static str, tag: &'static str, sink: harness::Events| {
            Table::new(vec![])
                .id(id)
                .column(
                    TableColumn::new("Name")
                        .default_width(px(200.))
                        .allows_sorting(true),
                )
                .keyed_row("first", vec![gpui::div().child("First").into_any_element()])
                .on_sort_change(move |descriptor, _, _| {
                    sink.borrow_mut()
                        .push(format!("{tag}:{}", descriptor.column));
                })
                .into_any_element()
        };
        side_by_side(
            build("left-sorted", "left", left),
            build("right-sorted", "right", right),
        )
    });

    // `.table__column` is `px-4 py-2.5` around a 16px line: the header row is
    // 36px tall, so y 18 is inside it, and x 60 is inside the 200px column.
    click(cx, 60., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:Name"],
        "the left header must sort the left table"
    );

    click(cx, COLUMN + 60., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:Name", "right:Name"],
        "two columns with the same label must stay two column headers"
    );
}

// ---------------------------------------------------------------------------
// Autocomplete — the picker whose virtual list gained a whole new element
// ---------------------------------------------------------------------------

/// The pickers mostly already had the ids their new roles needed. One did not:
/// `gpui::uniform_list` returns a `UniformList`, which is not a
/// `StatefulInteractiveElement` and so cannot carry `role="listbox"` however
/// many ids it has, so the virtualized `Autocomplete` list is now wrapped in a
/// `{id}-list` div that holds the role. That wrapper nests every virtual row
/// one segment deeper, which is exactly the shape that folds two instances
/// together.
///
/// Two open autocompletes over the same labels must therefore still pick their
/// own rows. Their base ids come from their own `InputState` entities, so the
/// wrapper is what has to keep the rows apart.
#[gpui::test]
fn two_virtual_autocompletes_pick_their_own_rows(cx: &mut TestAppContext) {
    harness::still();
    let picked = events();
    let recorded = picked.clone();
    let left_state = cx.new(|cx| InputState::new(cx));
    let right_state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let left = picked.clone();
        let right = picked.clone();
        let items = || {
            ["Rust", "Go", "Typst"]
                .iter()
                .map(|label| PickerItem::new((*label).to_owned(), (*label).to_owned()))
                .collect::<Vec<PickerItem>>()
        };
        side_by_side(
            Autocomplete::new(left_state.clone(), items())
                .is_open(true)
                .row_height(px(36.))
                .full_width(true)
                .on_selection_change(move |key: &SharedString, _, _| {
                    left.borrow_mut().push(format!("left:{key}"));
                })
                .into_any_element(),
            Autocomplete::new(right_state.clone(), items())
                .is_open(true)
                .row_height(px(36.))
                .full_width(true)
                .on_selection_change(move |key: &SharedString, _, _| {
                    right.borrow_mut().push(format!("right:{key}"));
                })
                .into_any_element(),
        )
    });

    // The popover hangs below its trigger; the search field is `shrink-0`
    // above the list. Rather than measure that stack, walk it: the open
    // popover's search field holds the focus, and the arrows drive the list.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["right:Rust"],
        "both popovers auto-focus their search field on open, so the one drawn \
         second holds it: exactly one instance must answer, and it must be \
         that one. A shared list path would report the wrong side, or both"
    );
}
