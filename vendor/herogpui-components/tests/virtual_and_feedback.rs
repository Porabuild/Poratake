//! Behaviour tests for the paths a screenshot cannot see: VIRTUALIZED
//! collections (`ListBox` and `Table` with a fixed or an estimated row
//! height), the toast queue's lifecycle (cap, action, pause/resume,
//! placement) and `ScrollShadow`'s derived visibility.
//!
//! A virtual list builds only the rows the viewport shows, so a row far down
//! a 1000-item list does not exist until a scroll brings it in. That is what
//! makes these tests worth having: a plain list cannot tell you whether the
//! click to index mapping survives the virtual path, because it never had to
//! build the row for index 300.
//!
//! The scroll mechanic, learned from gpui 0.2.2's own sources:
//!
//! - A scroll is `cx.simulate_event(ScrollEvent { position, delta, .. })`.
//!   The event hit-tests the **last rendered frame** (`dispatch_mouse_event`
//!   reads `rendered_frame`), so every state change must be followed by a
//!   redraw (`window.refresh()` inside an update) before the next event.
//! - `delta: ScrollDelta::Pixels(point(px(0.), px(-N)))` scrolls **down** by
//!   exactly N pixels: the scrollable element's listener does
//!   `scroll_offset.y += delta.y` and offsets are negative when scrolled down.
//!   `Lines` deltas are scaled by `window.line_height()`, so pixels are used
//!   throughout to keep the arithmetic exact.
//! - The wheel must land *inside* the scrollable element's hitbox
//!   (`hitbox.should_handle_scroll`), which is why every wheel event below
//!   names a point the component's geometry puts inside the scroller.
//!
//! Index arithmetic for the virtual lists (ListBox and Table alike):
//! `uniform_list` lays row i at `i * row_height + scroll_offset.y` inside its
//! own box, with `scroll_offset.y` clamped to `-(content - viewport)..0`, so a
//! wheel of `-12000px` over 40px rows brings item 300 to the top of the
//! viewport. The ListBox adds `p-1` (4px) of padding, so item 300 spans
//! window y 4..44 and a click at y = 24 is its centre.
//!
//! The Table's body sits below a header whose 12px text makes its height
//! land in [30, 46]px (the bound the sibling suite relies on), and the
//! header-drift cancels when a click is aimed at the middle of a row band:
//! a click at y hits row `floor((y - H) / h)` for the true H, so choosing y
//! strictly between `H_max + row_top` and `H_min + row_bottom` pins the row
//! for *every* H in the bound.
//!
//! The toast store (`ToastStore`, v3's `ToastQueue`) is app-global, so a
//! toast's lifecycle is asserted on the store entity, never on pixels; when
//! a card must be *pressed* it is rendered by `ToastViewport` and driven by
//! the keyboard or by probing the smallest clickable surface — the 20px
//! close button at a placement-derived coordinate.

use crate::harness;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    point, prelude::*, px, ScrollDelta, ScrollEvent, SharedString, TestAppContext,
    VisualTestContext,
};

use harness::{click, events, open_host, press, Events};
use herogpui_components::{
    pause_toasts, toast_store, ListBox, ListBoxItem, ScrollShadow, ScrollShadowVisibility,
    SelectionMode, SortDescriptor, SortDirection, Table, TableColumn, TableRow, Toast,
    ToastPlacement, ToastViewport, VirtualTreeMetadata,
};

/// Pins the toast card layout by enabling reduced motion **before** the first
/// frame. A toast wraps itself in `entering_zoom`, whose animation runs on
/// the executor clock this suite does not advance, so without this the card would sit
/// at its t=0 pose for the whole test. The harness applies the preference
/// before opening the host window — exactly the rule the overlay suite learned
/// the hard way.
fn still() {
    harness::still();
}

/// Pushes the pending frame through. Events are dispatched against the last
/// rendered frame, so anything that changes state — a scroll, a press, a
/// dismissal, a pushed toast — needs a redraw before the next event, or the
/// next event hits the stale frame.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn virtual_tree_key(index: usize) -> SharedString {
    ["parent", "child", "sibling"][index].into()
}

fn virtual_tree_metadata(index: usize) -> VirtualTreeMetadata {
    match index {
        0 => VirtualTreeMetadata {
            depth: 0,
            parent_key: None,
            has_children: true,
        },
        1 => VirtualTreeMetadata {
            depth: 1,
            parent_key: Some("parent".into()),
            has_children: false,
        },
        _ => VirtualTreeMetadata {
            depth: 0,
            parent_key: None,
            has_children: false,
        },
    }
}

fn virtual_tree_row(index: usize) -> TableRow {
    TableRow::new(vec![gpui::div()
        .child(["Parent", "Child", "Sibling"][index])
        .into_any_element()])
}

fn virtual_tree_table(
    id: &'static str,
    variable_height: bool,
    expanded: Rc<RefCell<Vec<SharedString>>>,
    recorded: Events,
) -> gpui::AnyElement {
    let expanded_now = expanded.borrow().clone();
    let expansion_events = recorded.clone();
    let action_events = recorded;
    let table = Table::new(vec![])
        .id(id)
        .column(TableColumn::new("Name").default_width(px(240.)))
        .tree_column(0)
        .expanded_keys(expanded_now)
        .virtual_rows(3, id, virtual_tree_key, virtual_tree_row)
        .virtual_tree_metadata(virtual_tree_metadata)
        .max_h(px(160.))
        .on_expanded_change(move |keys, window, _| {
            *expanded.borrow_mut() = keys.to_vec();
            expansion_events.borrow_mut().push(format!(
                "expanded:{}",
                keys.iter()
                    .map(AsRef::<str>::as_ref)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
            window.refresh();
        })
        .on_row_click(move |index, _, _, _| {
            action_events.borrow_mut().push(format!("action:{index}"));
        });
    if variable_height {
        table.estimated_row_height(px(40.)).into_any_element()
    } else {
        table.row_height(px(40.)).into_any_element()
    }
}

fn drive_virtual_tree_keyboard(cx: &mut VisualTestContext, recorded: &Events) {
    press(cx, "tab");
    press(cx, "down");
    press(cx, "right");
    flush_frame(cx);
    press(cx, "down");
    press(cx, "enter");
    press(cx, "left");
    press(cx, "enter");
    press(cx, "left");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["expanded:parent", "action:1", "action:0", "expanded:"],
        "a virtual tree must expand, enter its child, return to its parent, and collapse"
    );
}

#[gpui::test]
fn fixed_height_virtual_table_carries_tree_metadata(cx: &mut TestAppContext) {
    let expanded = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let recorded = events();
    let expanded_for_view = expanded;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        virtual_tree_table(
            "virtual-tree-fixed",
            false,
            expanded_for_view.clone(),
            for_view.clone(),
        )
    });

    drive_virtual_tree_keyboard(cx, &recorded);
}

#[gpui::test]
fn variable_height_virtual_table_carries_tree_metadata(cx: &mut TestAppContext) {
    let expanded = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let recorded = events();
    let expanded_for_view = expanded;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        virtual_tree_table(
            "virtual-tree-variable",
            true,
            expanded_for_view.clone(),
            for_view.clone(),
        )
    });

    drive_virtual_tree_keyboard(cx, &recorded);
}

/// One simulated wheel event at window coordinates (`x`, `y`), scrolling
/// `dy` pixels: **negative moves down** (later rows into view), matching the
/// scrollable element's `scroll_offset.y += delta.y` with negative offsets
/// meaning "scrolled down". The delta is `Pixels`, not `Lines`, so no line
/// height enters the arithmetic. Followed by a redraw so the next event sees
/// the frame the scroll produced.
fn wheel(cx: &mut VisualTestContext, x: f32, y: f32, dy: f32) {
    cx.simulate_event(ScrollEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
        ..Default::default()
    });
    flush_frame(cx);
}

/// A full-height clickable table cell that records `vrow-{index:04}` — the
/// row's *identity*, which is the claim a virtual table makes: the factory
/// must build the cell for exactly the index the viewport asked for.
fn probe_cell(index: usize, recorded: Events) -> gpui::AnyElement {
    let label = format!("vrow-{index:04}");
    let shown = label.clone();
    gpui::div()
        .id(gpui::ElementId::Name(format!("vt-probe-{index}").into()))
        .w_full()
        .h_full()
        .flex()
        .items_center()
        .cursor_pointer()
        .on_click(move |_, _, _| recorded.borrow_mut().push(label.clone()))
        .child(shown)
        .into_any_element()
}

/// A table cell whose content decides the row's height: `py-3` (12px each
/// side, applied by `RowCtx`) plus the 1px row border make a `h(20)` cell's
/// row 45px tall and a `h(60)` cell's row 85px tall — two real heights the
/// `gpui::list` measurement must keep apart.
fn var_cell(index: usize) -> gpui::AnyElement {
    let content_h = if index.is_multiple_of(2) {
        px(20.)
    } else {
        px(60.)
    };
    gpui::div()
        .h(content_h)
        .w_full()
        .child(format!("row {index}"))
        .into_any_element()
}

/// The name a resolved `ScrollShadowVisibility` is recorded under.
fn shadow_label(v: ScrollShadowVisibility) -> &'static str {
    match v {
        ScrollShadowVisibility::Auto => "auto",
        ScrollShadowVisibility::Both => "both",
        ScrollShadowVisibility::Top => "top",
        ScrollShadowVisibility::Bottom => "bottom",
        ScrollShadowVisibility::Left => "left",
        ScrollShadowVisibility::Right => "right",
        ScrollShadowVisibility::None => "none",
    }
}

// ---------------------------------------------------------------------------
// Virtualized collections
// ---------------------------------------------------------------------------

/// A virtual list builds only the rows in view, so item 300 is not in the
/// first paint at all. Scrolling -12000px over a 40px row grid brings item
/// 300 to the top of the 160px viewport; the click then lands on it and the
/// reported key must be the one its index names. A plain list cannot make
/// this claim — it never had to answer for row 300's identity.
#[gpui::test]
fn virtual_list_box_selects_a_row_after_scrolling(cx: &mut TestAppContext) {
    let recorded = events();
    let items: Vec<ListBoxItem> = (0..1000)
        .map(|i| ListBoxItem::new(format!("key-{i:04}"), format!("Item {i}")))
        .collect();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // `row_height` is the option that virtualizes the list (v3's
        // `layoutOptions.rowHeight`); `max_h` sizes the viewport, here 4 rows.
        ListBox::new("vlb-click", items.clone())
            .row_height(px(40.))
            .max_h(px(160.))
            .on_action(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    // Index arithmetic: the list's `p-1` padding puts the rows' box at
    // window y 4..164; the uniform_list lays row i at `i * 40 +
    // scroll_offset.y`. A wheel of -12000 (300 * 40) sets first_visible to
    // -(-12000)/40 = 300, so item 300 occupies window y 4..44. The wheel
    // position (20, 80) is inside the rows' box on the *initial* frame, which
    // is the only frame it hit-tests. The click at (20, 24) is item 300's
    // centre — a row that was not built (or was built for another index)
    // cannot answer it.
    wheel(cx, 20., 80., -12000.);
    click(cx, 20., 24.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-0300"],
        "scrolling 300 rows down and clicking must report the key of index 300"
    );
}

/// The arrows move a cursor the list has *not* built yet, and tell the list's
/// own scroll handle to bring the new row into view. Tab focuses index 0, so
/// after 31 Downs the cursor is on index 31 — not anything clamped to 4 visible
/// rows — and the deferred center-scroll places it at viewport centre, where
/// a click can prove the row now exists at that spot.
#[gpui::test]
fn virtual_list_box_arrows_scroll_the_focused_row_into_view(cx: &mut TestAppContext) {
    let recorded = events();
    let items: Vec<ListBoxItem> = (0..1000)
        .map(|i| ListBoxItem::new(format!("key-{i:04}"), format!("Item {i}")))
        .collect();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new("vlb-keys", items.clone())
            .row_height(px(40.))
            .max_h(px(160.))
            .on_action(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    // The list is the only tab stop, so Tab focuses item 0 and the arrows belong
    // to it. Thirty-one Downs walk to item 31, and each press defers a
    // center-scroll for the new cursor, applied at the next draw. The draw the
    // Enter dispatch triggers first is the one that places item 31.
    press(cx, "tab");
    for _ in 0..31 {
        press(cx, "down");
    }
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-0031"],
        "31 Downs after focus entry must activate index 31"
    );

    // The deferred center-scroll from the preceding key frame leaves item 30
    // centred at y 84 and the focused item 31 immediately below it, spanning
    // y 104..144. If the arrows had not scrolled the list, only rows 0..3
    // would exist and this click could not record index 31.
    flush_frame(cx);
    click(cx, 20., 124.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-0031", "key-0031"],
        "the centered row must be built and clickable at the position the \
         scroll arithmetic says"
    );
}

/// Pinned `ListKeyboardDelegate` pages by one visible rectangle. Four 40px
/// rows fit this 160px viewport, so the current row plus three more landings
/// moves row 2 toward disabled row 5, skips it to row 6, then lands on row 9;
/// PageUp returns to row 6.
#[gpui::test]
fn fixed_height_virtual_list_box_page_keys_move_one_viewport(cx: &mut TestAppContext) {
    let recorded = events();
    let items: Vec<ListBoxItem> = (0..20)
        .map(|i| ListBoxItem::new(format!("key-{i:02}"), format!("Item {i}")).is_disabled(i == 5))
        .collect();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new("vlb-fixed-page", items.clone())
            .row_height(px(40.))
            .max_h(px(160.))
            .on_action(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "pagedown");
    press(cx, "pageup");
    press(cx, "enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-06"],
        "fixed ListBox paging must skip disabled rows and keep the target actionable"
    );
}

/// `estimatedRowHeight` uses measured row heights for rows already built and
/// the estimate only for unseen rows. The 120px section plus the current row
/// fills this 160px viewport, so PageDown from row 1 lands on row 3. Treating
/// the section as the 40px estimate would incorrectly skip onward to row 5.
#[gpui::test]
fn estimated_height_virtual_list_box_page_keys_use_measured_sections(cx: &mut TestAppContext) {
    let recorded = events();
    let items = vec![
        ListBoxItem::new("key-00", "Item 0"),
        ListBoxItem::new("key-01", "Item 1"),
        ListBoxItem::section("Measured section"),
        ListBoxItem::new("key-03", "Item 3"),
        ListBoxItem::new("key-04", "Item 4"),
        ListBoxItem::new("key-05", "Item 5"),
    ];
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new("vlb-estimated-page", items.clone())
            .estimated_row_height(px(40.))
            .heading_height(px(120.))
            .max_h(px(160.))
            .on_action(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-03"],
        "estimated ListBox paging must account for a measured section row"
    );
}

/// ListBox's pinned delegate compares candidate tops, unlike Table's Grid
/// delegate which compares candidate bottoms. With an 80px content block in
/// the focused row, the next row's top already crosses the 160px page boundary;
/// a Grid-style accumulated-bottom walk would skip onward.
#[gpui::test]
fn estimated_height_list_box_page_down_uses_the_focused_rows_top_boundary(cx: &mut TestAppContext) {
    let recorded = events();
    let items: Vec<ListBoxItem> = (0..6)
        .map(|i| ListBoxItem::new(format!("key-{i:02}"), format!("Item {i}")))
        .collect();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new("vlb-tall-focused-page", items.clone())
            .estimated_row_height(px(40.))
            .max_h(px(160.))
            .item_content(|key, _| {
                gpui::div()
                    .h(if key.as_ref() == "key-00" {
                        px(80.)
                    } else {
                        px(20.)
                    })
                    .child(key.to_string())
                    .into_any_element()
            })
            .on_action(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "pagedown");
    press(cx, "enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-01"],
        "variable ListBox PageDown must use the focused row's top-based boundary"
    );
}

/// An estimated list's adjacent arrow target is already measured in the
/// viewport overdraw. Moving from row 0 to visible row 1 must not pin row 1 to
/// the top; the original row remains clickable in its first 36px band.
#[gpui::test]
fn estimated_height_list_box_arrow_keeps_a_visible_neighbor_in_place(cx: &mut TestAppContext) {
    let recorded = events();
    let items: Vec<ListBoxItem> = (0..10)
        .map(|i| ListBoxItem::new(format!("key-{i:02}"), format!("Item {i}")))
        .collect();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new("vlb-estimated-arrow-scroll", items.clone())
            .estimated_row_height(px(40.))
            .max_h(px(160.))
            .on_action(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    flush_frame(cx);
    click(cx, 20., 20.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-00"],
        "an adjacent arrow move must reveal without snapping the row to the viewport top"
    );
}

/// A plain scrollable ListBox has every row's real bounds. Its 36px options
/// plus 4px flex gap form 40px starts, so the same 160px viewport pages row 2
/// to row 6 and back. Without a layout-aware PageUp/PageDown path both keys
/// are ignored by the shared one-row resolver.
#[gpui::test]
fn plain_scrollable_list_box_page_keys_use_laid_out_bounds(cx: &mut TestAppContext) {
    let recorded = events();
    let items: Vec<ListBoxItem> = (0..20)
        .map(|i| ListBoxItem::new(format!("key-{i:02}"), format!("Item {i}")))
        .collect();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new("vlb-plain-page", items.clone())
            .max_h(px(160.))
            .on_action(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "pageup");
    press(cx, "enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-02"],
        "plain ListBox paging must use actual row bounds and keep the target actionable"
    );
}

/// Pinned `ListKeyboardDelegate` uses the enabled collection ends when the
/// ListBox is not scrollable, because there is no shorter visible page to
/// preserve. Disabled rows at both ends are skipped by the same delegate.
#[gpui::test]
fn plain_unbounded_list_box_page_keys_reach_enabled_ends(cx: &mut TestAppContext) {
    let recorded = events();
    let items: Vec<ListBoxItem> = (0..10)
        .map(|i| {
            ListBoxItem::new(format!("key-{i:02}"), format!("Item {i}"))
                .is_disabled(i == 0 || i == 9)
        })
        .collect();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new("vlb-unbounded-page", items.clone())
            .on_action(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "enter");
    press(cx, "pageup");
    press(cx, "enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-08", "key-01"],
        "unbounded ListBox paging must reach the enabled ends"
    );
}

/// A virtual table's rows come from a *factory* — `AnyElement` is built once
/// and consumed once, so the table asks for the rows the viewport shows. The
/// probe cell records the index the factory was called with, so the click
/// after scrolling proves the visible-range math handed out row 300, not a
/// neighbour; the sort header then proves the virtual body did not disturb
/// the controlled sort loop.
#[gpui::test]
fn virtual_table_rows_click_and_sort(cx: &mut TestAppContext) {
    let probes = events();
    let sorts = events();
    let held: Rc<RefCell<Option<SortDescriptor>>> = Rc::new(RefCell::new(None));
    let probes_for_view = probes.clone();
    let sorts_for_view = sorts.clone();
    let held_for_view = held;
    let cx = open_host(cx, move || {
        let probes = probes_for_view.clone();
        let sorts = sorts_for_view.clone();
        let held = held_for_view.clone();
        let mut table = Table::new(vec![])
            .id("vt-sort")
            .columns(vec![
                TableColumn::new("Name")
                    .allows_sorting(true)
                    .default_width(px(160.)),
            ])
            // 1000 rows built on demand: the factory must be re-invokable on
            // every scroll, which is exactly what `virtual_rows` is for.
            .virtual_rows(
                1000,
                "virtual-sort-users",
                |i| i.to_string().into(),
                move |i| TableRow::new(vec![probe_cell(i, probes.clone())]),
            )
            .row_height(px(40.))
            .max_h(px(160.));
        // Sorting is controlled: the caller feeds the reported descriptor
        // back, which is also what lets a second click on the same column
        // flip.
        if let Some(d) = held.borrow().clone() {
            table = table.sort_descriptor(d);
        }
        table
            .on_sort_change(move |d, _, _| {
                let dir = if d.direction == SortDirection::Ascending {
                    "asc"
                } else {
                    "desc"
                };
                sorts.borrow_mut().push(format!("{}:{dir}", d.column));
                *held.borrow_mut() = Some(d.clone());
            })
            .into_any_element()
    });

    // Header height is ~37px (12px text in `py-2.5`), the bound the sibling
    // suite established as [30, 46]. Scrolling -12000 (300 * 40) puts item
    // 300 at the body top, so its 40px band spans y H..H+40 for the true H;
    // y = 58 is inside that band for every H in [30, 46] (58 > 46, 58 < 70).
    // The wheel at (100, 100) is inside the virtual body (H..H+160) for the
    // same bound.
    wheel(cx, 100., 100., -12000.);
    click(cx, 30., 58.);
    assert_eq!(
        probes.borrow().as_slice(),
        ["vrow-0300"],
        "after scrolling 300 rows, the click must land on the row built for \
         index 300"
    );

    // The header cell is ~37px tall, so y = 18 is inside it; the column is
    // 160px wide and clickable. Two clicks report asc then desc — the
    // controlled loop, driven through the virtual body's untouched header.
    click(cx, 80., 18.);
    click(cx, 80., 18.);
    assert_eq!(
        sorts.borrow().as_slice(),
        ["Name:asc", "Name:desc"],
        "the virtual table's sortable header must toggle like a plain one"
    );
}

/// The virtual projection — every collection key plus the rows visible under
/// `expanded_keys` — is cached against the collection's identity, its count and
/// the expanded set. An unchanged frame must not re-run `key_for_row` (a fresh
/// build every frame is exactly the cost the cache exists to avoid), and each
/// of those three inputs changing must re-run it.
#[gpui::test]
fn virtual_table_projection_skips_key_calls_on_unchanged_frames(cx: &mut TestAppContext) {
    let key_calls = Rc::new(Cell::new(0usize));
    let count = Rc::new(Cell::new(400usize));
    let identity = Rc::new(RefCell::new(String::from("cache-a")));
    let expanded = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let cx = open_host(cx, {
        let key_calls = key_calls.clone();
        let count = count.clone();
        let identity = identity.clone();
        let expanded = expanded.clone();
        move || {
            let key_calls = key_calls.clone();
            Table::new(vec![])
                    .id("vt-projection-cache")
                    .column(TableColumn::new("Name").default_width(px(240.)))
                    .tree_column(0)
                    .expanded_keys(expanded.borrow().clone())
                    .virtual_rows(
                        count.get(),
                        identity.borrow().clone(),
                        move |i| {
                            key_calls.set(key_calls.get() + 1);
                            SharedString::from(format!("row-{i}"))
                        },
                        |i| {
                            TableRow::new(vec![gpui::div()
                                .child(format!("Row {i}"))
                                .into_any_element()])
                        },
                    )
                    // Ten-item groups: a group head is visible collapsed, and
                    // expanding it brings its nine children into the
                    // projection, so the expansion invalidation below is a
                    // real change of the visible set, not just the fingerprint.
                    .virtual_tree_metadata(|i| VirtualTreeMetadata {
                        depth: usize::from(i % 10 != 0),
                        parent_key: (i % 10 != 0)
                            .then(|| SharedString::from(format!("row-{}", i - i % 10))),
                        has_children: i % 10 == 0,
                    })
                    .row_height(px(40.))
                    .max_h(px(400.))
                    .into_any_element()
        }
    });

    let after_first = key_calls.get();
    assert!(
        after_first >= 400,
        "the first frame must project a key for every row in the collection"
    );

    for _ in 0..3 {
        flush_frame(cx);
    }
    assert_eq!(
        key_calls.get(),
        after_first,
        "unchanged frames must reuse the cached projection"
    );

    expanded.borrow_mut().push(SharedString::from("row-0"));
    flush_frame(cx);
    let after_expansion = key_calls.get();
    assert!(
        after_expansion > after_first,
        "a changed expanded set must invalidate the projection"
    );
    for _ in 0..2 {
        flush_frame(cx);
    }
    assert_eq!(key_calls.get(), after_expansion);

    count.set(401);
    flush_frame(cx);
    let after_count = key_calls.get();
    assert!(
        after_count > after_expansion,
        "a changed collection count must invalidate the projection"
    );
    for _ in 0..2 {
        flush_frame(cx);
    }
    assert_eq!(key_calls.get(), after_count);

    *identity.borrow_mut() = String::from("cache-b");
    flush_frame(cx);
    let after_identity = key_calls.get();
    assert!(
        after_identity > after_count,
        "a changed collection identity must invalidate the projection"
    );
    for _ in 0..2 {
        flush_frame(cx);
    }
    assert_eq!(key_calls.get(), after_identity);
}

/// The full selectable set is cached with its disabled-key input. Changing
/// `disabled_keys` must refresh that filtered set without rebuilding the
/// collection projection, so Mod+A never reports a newly disabled row.
#[gpui::test]
fn virtual_table_mod_a_refreshes_cached_disabled_keys(cx: &mut TestAppContext) {
    let disabled = Rc::new(Cell::new(false));
    let recorded = events();
    let disabled_for_view = disabled.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let disabled = disabled_for_view.clone();
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("vt-selectable-cache")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .disabled_keys(disabled.get().then(|| SharedString::from("key-1")))
            .virtual_rows(
                3,
                "virtual-selectable-cache",
                |i| format!("key-{i}").into(),
                |i| {
                    TableRow::new(vec![gpui::div()
                        .child(format!("Row {i}"))
                        .into_any_element()])
                },
            )
            .row_height(px(40.))
            .max_h(px(120.))
            .on_selection_change(move |keys, _, _| {
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    flush_frame(cx);
    press(cx, "tab");
    disabled.set(true);
    flush_frame(cx);
    press(
        cx,
        if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        },
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-0,key-2"],
        "Mod+A must use the disabled-key-filtered virtual collection after an update"
    );
}

/// React Aria's pinned `useTable` builds its keyboard delegate from the full
/// collection and passes the virtual layout delegate alongside it, so rows do
/// not stop being keyboard destinations merely because they are not painted.
/// This port's table wrapper takes the tab stop, then the first Down enters at
/// row 0. Another 31 Downs must move through the collection and scroll row 31
/// into view, where Enter activates it through the same row action as a pointer
/// press.
#[gpui::test]
fn virtual_table_arrows_scroll_the_focused_row_into_view(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("vt-keys")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .virtual_rows(
                1000,
                "virtual-key-users",
                |i| format!("key-{i:04}").into(),
                |i| {
                    TableRow::new(vec![gpui::div()
                        .child(format!("Row {i}"))
                        .into_any_element()])
                },
            )
            .row_height(px(40.))
            // A concrete rowHeight wins when both TableLayout hints exist.
            .estimated_row_height(px(24.))
            .max_h(px(160.))
            .on_row_click(move |i, _, _, _| recorded.borrow_mut().push(format!("row-{i}")))
            .into_any_element()
    });

    press(cx, "tab");
    for _ in 0..32 {
        press(cx, "down");
    }
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-31"],
        "the entry Down plus 31 moves must activate virtual row 31"
    );

    // The deferred centre-scroll puts row 31 around the middle of the 160px
    // body. The preceding row is centred at the body's midpoint, leaving row
    // 31 in the 40px band immediately below it; with the ~37px header, y = 157
    // is inside that band. A click there proves the keyboard move caused the
    // row factory to build row 31 rather than merely letting Enter address an
    // off-screen index.
    flush_frame(cx);
    click(cx, 30., 157.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-31", "row-31"],
        "the keyboard-focused virtual row must be built at the scrolled position"
    );
}

/// Pinned React Aria's `GridKeyboardDelegate` moves PageUp/PageDown by one
/// visible rectangle. With 40px rows in a 160px viewport, row 2 pages to row
/// 5, then row 8, and PageUp returns to row 5.
#[gpui::test]
fn fixed_height_virtual_table_page_keys_move_one_viewport(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("vt-page-keys")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .virtual_rows(
                20,
                "virtual-page-key-users",
                |i| format!("key-{i:02}").into(),
                |i| {
                    TableRow::new(vec![gpui::div()
                        .child(format!("Row {i}"))
                        .into_any_element()])
                },
            )
            .row_height(px(40.))
            .max_h(px(160.))
            .on_row_click(move |i, _, _, _| recorded.borrow_mut().push(format!("row-{i}")))
            .into_any_element()
    });

    press(cx, "tab");
    for _ in 0..3 {
        press(cx, "down");
    }
    press(cx, "pagedown");
    press(cx, "enter");
    press(cx, "pagedown");
    press(cx, "enter");
    press(cx, "pageup");
    press(cx, "enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-5", "row-8", "row-5"],
        "PageDown and PageUp must move by one virtual viewport"
    );
}

/// `estimatedRowHeight` virtualizes through the same pinned TableLayout, so
/// PageUp/PageDown move by one visible rectangle here too. Normal rows measure
/// 30px, disabled row 5 measures 85px, and the estimate is 40px. The disabled
/// row still occupies layout space but cannot become a target, so the sequence
/// starts 2 → 6; later unseen rows use the estimate, producing 10 → 4. Dropping
/// the disabled height or ignoring measurements lands on different rows.
#[gpui::test]
fn variable_height_virtual_table_page_keys_move_one_viewport(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("vt-var-page-keys")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .virtual_rows(
                20,
                "virtual-variable-page-key-users",
                |i| format!("key-{i:02}").into(),
                |i| {
                    let height = if i == 5 { 60. } else { 5. };
                    TableRow::new(vec![gpui::div()
                        .h(px(height))
                        .w_full()
                        .child(format!("Row {i}"))
                        .into_any_element()])
                },
            )
            .estimated_row_height(px(40.))
            .max_h(px(160.))
            .disabled_keys(["key-05"])
            .on_row_click(move |i, _, _, _| recorded.borrow_mut().push(format!("row-{i}")))
            .into_any_element()
    });

    press(cx, "tab");
    for _ in 0..3 {
        press(cx, "down");
    }
    press(cx, "pagedown");
    press(cx, "enter");
    press(cx, "pagedown");
    press(cx, "enter");
    press(cx, "pageup");
    press(cx, "enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-6", "row-10", "row-4"],
        "page keys must move a variable-height virtual Table by one viewport"
    );
}

/// Pinned paging stops as soon as the current row reaches the page boundary.
/// A row taller than the viewport therefore keeps focus on itself rather than
/// skipping content that has not fit on screen yet.
#[gpui::test]
fn variable_height_table_page_key_stays_on_a_viewport_tall_row(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("vt-var-tall-page")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .virtual_rows(
                2,
                "virtual-variable-tall-page-users",
                |i| format!("key-{i}").into(),
                |i| {
                    let height = if i == 0 { 200. } else { 20. };
                    TableRow::new(vec![gpui::div()
                        .h(px(height))
                        .w_full()
                        .child(format!("Row {i}"))
                        .into_any_element()])
                },
            )
            .estimated_row_height(px(40.))
            .max_h(px(160.))
            .on_row_click(move |i, _, _, _| recorded.borrow_mut().push(format!("row-{i}")))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-0"],
        "a row that already spans a viewport must not be paged past"
    );
}

/// Pinned React Aria's `TableKeyboardDelegate` runs one focus machine over
/// {columns..., rows...}: Down/PageDown cross header->body and PageUp from the
/// top of the body enters the header. The virtual body still pages by viewport
/// mid-list, so only a PageUp with nowhere to go (the cursor already on the
/// first enabled stop) leaves for the header; the cursor stays seated, so Down
/// returns to the first enabled row and PageDown from the header reaches the
/// last enabled row.
#[gpui::test]
fn virtual_table_page_keys_cross_between_header_and_body(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("vt-header-crossing")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .virtual_rows(
                20,
                "virtual-header-crossing-users",
                |i| format!("key-{i:02}").into(),
                |i| {
                    TableRow::new(vec![gpui::div()
                        .child(format!("Row {i}"))
                        .into_any_element()])
                },
            )
            .row_height(px(40.))
            .max_h(px(160.))
            .disabled_keys(["key-00", "key-19"])
            .on_row_click(move |i, _, _, _| recorded.borrow_mut().push(format!("row-{i}")))
            .into_any_element()
    });

    // Tab lands on the wrapper; Down enters the body at the first enabled row.
    press(cx, "tab");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-1"],
        "Down must enter a virtual body at its first enabled row"
    );

    // PageUp from the top of the body leaves for the header. The proof is that
    // Enter no longer activates a row: the non-sortable header answers nothing.
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-1"],
        "PageUp from the first enabled row must enter the header, so Enter no longer reaches the body"
    );

    // Down from the header returns to the first enabled row.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-1", "row-1"],
        "Down from a virtual header must return to the first enabled row"
    );

    // Back to the header, then PageDown reaches the last enabled row.
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-1", "row-1"],
        "PageUp from the first enabled row must re-enter the header"
    );
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-1", "row-1", "row-18"],
        "PageDown from a virtual header must reach the last enabled row"
    );
}

/// `estimatedRowHeight` takes the `gpui::list` path, which measures *each*
/// built row instead of multiplying one. Rows of two real heights — 45px and
/// 85px — must lay out without overlapping: a click aimed mid-way down a tall
/// row names that row's index, and a mis-measured list (uniform 45 or
/// uniform 85) puts the click in a neighbour's band instead.
#[gpui::test]
fn variable_height_rows_do_not_overlap(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Six rows alternating h(20) and h(60) cells -> 45/85px rows; the
        // whole 390px fits the default 400px body, so every row is built and
        // measured, and only the measurement — never the viewport — decides
        // where each row's click band is.
        Table::new(vec![])
            .id("vt-var")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .virtual_rows(
                6,
                "virtual-variable-users",
                |i| i.to_string().into(),
                |i| TableRow::new(vec![var_cell(i)]),
            )
            .estimated_row_height(px(24.))
            .on_row_click(move |i, _, _, _| recorded.borrow_mut().push(format!("row-{i}")))
            .into_any_element()
    });
    flush_frame(cx);

    // Row bands (body-relative), from the two measured heights and the 1px
    // row border:
    //   row 0:  0..45,  row 1: 45..130, row 2: 130..175,
    //   row 3: 175..260, row 4: 260..305, row 5: 305..390.
    // The body sits below a header of height H in [30, 46], so a window y
    // lands inside row r for every possible H when it lies between
    // H_max + row_top and H_min + row_bottom. For row 1 that interval is
    // (91, 160) — y = 120; for row 3 it is (221, 290) — y = 250.
    // A uniform-45 list puts row 3 at body 135..180 (window ~177) and a
    // uniform-85 one at 255..340 (~window 301), so either mis-measurement
    // turns one of these clicks into a neighbour's index.
    click(cx, 30., 120.);
    click(cx, 30., 250.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-1", "row-3"],
        "rows of two heights must measure and lay out without overlap"
    );
}

/// A variable-height virtual table must navigate beyond the rows it has
/// measured. Logical index scrolling brings the distant row into the viewport
/// first; the list then measures its real height and the row becomes clickable.
#[gpui::test]
fn variable_height_table_keyboard_scrolls_to_an_unmeasured_row(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("vt-var-keys")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .virtual_rows(
                1000,
                "virtual-variable-key-users",
                |i| format!("key-{i:04}").into(),
                |i| TableRow::new(vec![var_cell(i)]),
            )
            .estimated_row_height(px(40.))
            .max_h(px(160.))
            .on_row_click(move |i, _, _, _| recorded.borrow_mut().push(format!("row-{i}")))
            .into_any_element()
    });

    press(cx, "tab");
    for _ in 0..32 {
        press(cx, "down");
    }
    press(cx, "enter");
    assert_eq!(recorded.borrow().as_slice(), ["row-31"]);

    flush_frame(cx);
    click(cx, 30., 60.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-31", "row-31"],
        "the unmeasured keyboard target must be built at the viewport"
    );

    // Row 31 is 85px tall and the preceding rows alternate 45/85px. One
    // 160px PageUp therefore crosses row 30 and lands on row 29; the 40px
    // estimate alone would incorrectly move three indices to row 28.
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-31", "row-31", "row-29"],
        "PageUp must use measured variable row heights when they are available"
    );
}

/// Replacing a variable-height collection with the same count but a different
/// identity must discard its measured heights and logical scroll position.
/// Otherwise the replacement opens around the old row 31 instead of row 0.
#[gpui::test]
fn variable_height_same_count_identity_resets_measurements_and_scroll(cx: &mut TestAppContext) {
    let recorded = events();
    let second_page = Rc::new(Cell::new(false));
    let page_for_view = second_page.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let key_page = page_for_view.clone();
        let row_page = page_for_view.clone();
        let identity = if page_for_view.get() {
            "variable-beta"
        } else {
            "variable-alpha"
        };
        Table::new(vec![])
            .id("vt-var-replace")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .virtual_rows(
                100,
                identity,
                move |i| {
                    if key_page.get() {
                        format!("beta-{i}").into()
                    } else {
                        format!("alpha-{i}").into()
                    }
                },
                move |i| {
                    let height = if row_page.get() { 60. } else { 20. };
                    TableRow::new(vec![gpui::div()
                        .h(px(height))
                        .w_full()
                        .child(format!("Row {i}"))
                        .into_any_element()])
                },
            )
            .estimated_row_height(px(40.))
            .max_h(px(160.))
            .on_row_click(move |i, _, _, _| recorded.borrow_mut().push(format!("row-{i}")))
            .into_any_element()
    });

    press(cx, "tab");
    for _ in 0..32 {
        press(cx, "down");
    }
    flush_frame(cx);
    click(cx, 30., 60.);
    assert_eq!(recorded.borrow().as_slice(), ["row-31"]);

    second_page.set(true);
    flush_frame(cx);
    click(cx, 30., 60.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-31", "row-0"],
        "a new collection identity must start with fresh measurements at row 0"
    );
}

// ---------------------------------------------------------------------------
// Toast queue lifecycle
// ---------------------------------------------------------------------------

/// Pinned v3's `maxVisibleToasts` is visual-only: overflow remains queued and
/// React Stately unshifts each new toast, so five pushes under a three-toast
/// viewport must retain all five, draw the newest three, then reveal the next
/// waiting toast when the frontmost one closes.
#[gpui::test]
fn toast_queue_respects_its_limit(cx: &mut TestAppContext) {
    let ids: Vec<u64> = cx.update(|cx| {
        (1..=5)
            .map(|i| {
                Toast::new(format!("T{i}"))
                    .timeout(Duration::ZERO)
                    .push(None, cx)
            })
            .collect()
    });
    assert_eq!(ids.len(), 5, "five pushes must issue five distinct ids");

    cx.update(|cx| {
        let store = toast_store(cx);
        let store = store.read(cx);
        let toasts = store.toasts();
        assert_eq!(
            toasts.len(),
            5,
            "maxVisibleToasts is visual-only, so overflow must remain queued"
        );
        for (n, (kept, title)) in toasts
            .iter()
            .zip(["T5", "T4", "T3", "T2", "T1"])
            .enumerate()
        {
            assert_eq!(
                kept.id,
                ids[4 - n],
                "every queued toast keeps its id in newest-first order"
            );
            assert_eq!(kept.title.as_ref(), title, "the title matches the id");
        }
        assert_eq!(
            store
                .visible_toasts(3)
                .iter()
                .map(|toast| toast.id)
                .collect::<Vec<_>>(),
            [ids[4], ids[3], ids[2]],
            "the newest three queued toasts must be visible"
        );
    });

    cx.update(|cx| herogpui_components::dismiss_toast(ids[4], cx));
    cx.update(|cx| {
        let store = toast_store(cx);
        let toasts = store.read(cx);
        assert_eq!(
            toasts
                .visible_toasts(3)
                .iter()
                .map(|toast| toast.id)
                .collect::<Vec<_>>(),
            [ids[3], ids[2], ids[1]],
            "closing the frontmost toast must reveal the next queued toast"
        );
    });
}

/// A toast's `action` is a button inside the card: pressing it reports *and*
/// dismisses its own toast, leaving the siblings. The card is rendered by
/// `ToastViewport`, the button is the first tab stop in it, and Enter
/// activates a focused element on key-up — which the harness `press` helper
/// supplies explicitly.
#[gpui::test]
fn toast_action_button_reports_and_dismisses(cx: &mut TestAppContext) {
    still();
    let rec = events();
    let action_rec = rec.clone();
    let (id_a, id_b) = cx.update(|cx| {
        // The action toast must be newest/frontmost: pinned v3 makes every
        // stacked toast behind it pointer- and keyboard-inert.
        let b = Toast::new("Sibling").timeout(Duration::ZERO).push(None, cx);
        let a = Toast::new("Has an action")
            .timeout(Duration::ZERO)
            .action("Undo", move |_| {
                action_rec.borrow_mut().push("undo".to_owned());
            })
            .push(None, cx);
        (a, b)
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());

    // The card's children order is [indicator, title, action, close], so the
    // action button is the first tab stop; the close button is second.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        rec.borrow().as_slice(),
        ["undo"],
        "Enter on the action button must report the action exactly once"
    );

    cx.update(|_window, cx| {
        let toasts = toast_store(cx).read(cx).toasts();
        assert_eq!(toasts.len(), 1, "the pressed action must dismiss its toast");
        assert_eq!(
            toasts[0].id, id_b,
            "the dismissed toast must be the acted-on one, not its sibling"
        );
        assert!(
            toasts.iter().all(|t| t.id != id_a),
            "the acted-on toast's id must be gone from the store"
        );
    });
}

/// `pauseAll` / `resumeAll` stop and restart every dismissal clock. While
/// paused, ticking the test clock far past a toast's timeout must not dismiss
/// it; after `resumeAll` the same clock dismisses it.
#[gpui::test]
fn toast_pause_and_resume_stop_the_clock(cx: &mut TestAppContext) {
    still();
    let timed = cx.update(|cx| {
        // Pause first, then push: the toast's timer ticks are what `paused`
        // gates, and the first tick is only 100ms away.
        pause_toasts(true, cx);
        Toast::new("Times out")
            .timeout(Duration::from_millis(300))
            .push(None, cx)
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());

    // 1500ms is five times the timeout; every 100ms tick has seen `paused`.
    cx.executor().advance_clock(Duration::from_millis(1500));
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx)
                .read(cx)
                .toasts()
                .iter()
                .any(|t| t.id == timed),
            "a paused queue must not dismiss a toast whose timeout has passed"
        );
    });

    // Resume: three 100ms ticks subtract the 300ms, and it goes.
    cx.update(|_window, cx| pause_toasts(false, cx));
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx)
                .read(cx)
                .toasts()
                .iter()
                .all(|t| t.id != timed),
            "after resume, the timeout must dismiss the toast"
        );
    });
}

/// Placement is a layout choice, so it is asserted behaviourally: the toast's
/// close button is a real 20px click target, and where it sits in the window
/// is exactly what `placement` decides. At `Bottom` the card hugs the bottom
/// inset and the button answers a click near the window's bottom edge; after
/// switching to `Top` the same spot must hit nothing (the card moved away)
/// and the button answers at the top edge instead. No appearance is asserted.
#[gpui::test]
fn toast_placement_moves_the_viewport(cx: &mut TestAppContext) {
    still();
    // Pushed before the viewport exists: the store is app-global and the
    // viewport reads it at render, so ordering only affects the frame shown.
    cx.update(|cx| {
        Toast::new("A").timeout(Duration::ZERO).push(None, cx);
    });
    let placement = Rc::new(RefCell::new(ToastPlacement::Bottom));
    let holder = placement.clone();
    let cx = open_host(cx, move || {
        ToastViewport::new()
            .placement(*holder.borrow())
            .into_any_element()
    });

    // Card geometry (no action button, so the children are a 20px title and
    // the 20px close button): `p(16)` + width 460 centers the card at
    // x 730..1190; `py-3`-equivalent padding (12px each side) and the 20px
    // line make the card 44px tall. Bottom placement parks its bottom edge
    // 16px off the window's bottom (1080), so the card spans y 1020..1064
    // The pinned close control is absolute `-end-1 -top-1`, so it spans
    // x 1174..1194 and y 1016..1036 — centre (1184, 1026).
    // The top placement's page geometry: the card spans y 16..60 and the
    // close button y 12..32 — centre (1184, 22).
    click(cx, 1184., 1026.);
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "at Bottom, the close button must live near the bottom edge: the \
             click must dismiss the toast"
        );
    });

    // Move the viewport to Top and push a second toast; the same bottom-edge
    // point must now be empty background — nothing can dismiss the toast.
    *placement.borrow_mut() = ToastPlacement::Top;
    cx.update(|_window, cx| {
        Toast::new("B").timeout(Duration::ZERO).push(None, cx);
    });
    flush_frame(cx);
    click(cx, 1184., 1026.);
    cx.update(|_window, cx| {
        assert_eq!(
            toast_store(cx).read(cx).toasts().len(),
            1,
            "at Top, the same bottom-edge point must hit nothing: the toast \
             must survive"
        );
    });

    // And the button answers where Top actually put it.
    click(cx, 1184., 22.);
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "at Top, the close button must live near the top edge: the click \
             must dismiss the toast"
        );
    });
}

// ---------------------------------------------------------------------------
// ScrollShadow
// ---------------------------------------------------------------------------

/// `visibility(Auto)` is derived from the live scroll offset, and
/// `onVisibilityChange` reports every edge change: at the top only the
/// bottom edge shades, mid-scroll both do, at the bottom only the top, and
/// back at the top only the bottom again.
#[gpui::test]
fn scroll_shadow_reports_visibility_as_it_scrolls(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Ten 40px blocks with the 8px gap make 472px of content in a 160px
        // box: 312px of scroll (`max_offset`). The wheel positions hit the
        // scroller itself, which spans the full box.
        ScrollShadow::new("ss-auto")
            .max_h(px(160.))
            .visibility(ScrollShadowVisibility::Auto)
            .on_visibility_change(move |v, _, _| {
                recorded.borrow_mut().push(shadow_label(*v).to_owned());
            })
            .children((0..10).map(|_| {
                gpui::div()
                    .h(px(40.))
                    .w_full()
                    .child("block")
                    .into_any_element()
            }))
            .into_any_element()
    });
    flush_frame(cx);

    // The canvas reports from the first frame: offset 0, no leading fade
    // (`past_start` false), plenty of content left (`before_end` true) →
    // Bottom.
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bottom"],
        "at the top of a scrollable box only the bottom edge shades"
    );

    cx.simulate_event(ScrollEvent {
        position: point(px(100.), px(80.)),
        delta: ScrollDelta::Pixels(point(px(-80.), px(0.))),
        ..Default::default()
    });
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bottom"],
        "horizontal wheel must not move a vertical ScrollShadow"
    );

    // Half the max (156 of 312): past the start and short of the end → Both.
    wheel(cx, 100., 80., -156.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bottom", "both"],
        "mid-scroll both edges must shade"
    );

    // The other half reaches the bottom exactly: `-312 > -312` is false, so
    // only the leading (top) edge shades.
    wheel(cx, 100., 80., -156.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bottom", "both", "top"],
        "at the bottom only the top edge shades"
    );

    // All the way back up in one gesture, and the change fires again.
    wheel(cx, 100., 80., 312.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bottom", "both", "top", "bottom"],
        "returning to the top must report the bottom-edge shade again"
    );
}

/// Content shorter than the box has no scroll range, so `Auto` resolves to
/// `None` and `onVisibilityChange` never fires — even under a wheel. The
/// scroller's listener adds the wheel's delta straight into the tracked
/// handle's offset cell during event dispatch and layout only clamps it on
/// the next pass, so `ScrollShadow::render` could read a -40px offset over a
/// zero-range box and resolve `Auto` to a one-frame `Top` shadow. The
/// resolution clamps what it reads into `[-max, 0]`, which pins a zero-range
/// box at 0: `Auto` stays `None` and nothing is ever reported. Regression for
/// the recorder holding `["top", "none"]` before the clamp existed.
#[gpui::test]
fn scroll_shadow_hides_when_content_fits(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Two 40px blocks plus the 8px gap = 88px inside the 160px box: the
        // scroller's range is 0, so the leading/trailing conditions are both
        // false at every offset and Auto stays None forever.
        ScrollShadow::new("ss-fit")
            .max_h(px(160.))
            .visibility(ScrollShadowVisibility::Auto)
            .on_visibility_change(move |v, _, _| {
                recorded.borrow_mut().push(shadow_label(*v).to_owned());
            })
            .children((0..2).map(|_| gpui::div().h(px(40.)).w_full().into_any_element()))
            .into_any_element()
    });
    flush_frame(cx);
    // A wheel at a scroller with zero range: the listener adds the delta but
    // the next layout clamps it back to 0, and resolution never changes.
    wheel(cx, 100., 80., -40.);
    flush_frame(cx);
    assert!(
        recorded.borrow().is_empty(),
        "content that fits must never report a shadow edge, observed {:?}",
        recorded.borrow().as_slice()
    );
}

/// The silent counterpart of the wheeled fits-content case: a box whose
/// content fits and is *never* wheeled reports nothing at all. This is what
/// pins the fix as a rule about scroll *range* — nothing to scroll means no
/// edge, from the first frame on — rather than a clamp that happens to
/// swallow a wheel.
#[gpui::test]
fn scroll_shadow_silent_when_content_fits(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Two 40px blocks plus the 8px gap = 88px inside the 160px box: the
        // scroller's range is 0, so `Auto` resolves `None` on the very first
        // frame and never changes.
        ScrollShadow::new("ss-silent")
            .max_h(px(160.))
            .visibility(ScrollShadowVisibility::Auto)
            .on_visibility_change(move |v, _, _| {
                recorded.borrow_mut().push(shadow_label(*v).to_owned());
            })
            .children((0..2).map(|_| gpui::div().h(px(40.)).w_full().into_any_element()))
            .into_any_element()
    });
    // A second flush gives a mis-resolving render the frame to correct
    // itself in — the shape the wheeled defect used to take.
    flush_frame(cx);
    flush_frame(cx);
    assert!(
        recorded.borrow().is_empty(),
        "a fits-content box that is never wheeled must never report a shadow \
         edge, observed {:?}",
        recorded.borrow().as_slice()
    );
}

/// Content that fits reports nothing; once it grows past the box, the first
/// edge reported is the correct one. The resolution reads the offset and max
/// the last prepaint left in the tracked handle, so the re-measured range
/// (88 → 608px against the 160px box) is seen one frame after the layout
/// that grew it — and it must arrive as `Bottom`, not as a stale `none` or a
/// spurious leading edge.
#[gpui::test]
fn scroll_shadow_reports_first_edge_after_content_grows(cx: &mut TestAppContext) {
    let recorded = events();
    let child_h = Rc::new(RefCell::new(40f32));
    let for_view = recorded.clone();
    let h_for_view = child_h.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let h = *h_for_view.borrow();
        // `flex_shrink_0` keeps the blocks at their laid-out height, so
        // growing them really grows the content past the box instead of
        // letting the flex scroller shrink them back into it.
        ScrollShadow::new("ss-grow")
            .max_h(px(160.))
            .visibility(ScrollShadowVisibility::Auto)
            .on_visibility_change(move |v, _, _| {
                recorded.borrow_mut().push(shadow_label(*v).to_owned());
            })
            .children((0..2).map(|_| {
                gpui::div()
                    .h(px(h))
                    .w_full()
                    .flex_shrink_0()
                    .into_any_element()
            }))
            .into_any_element()
    });
    flush_frame(cx);
    assert!(
        recorded.borrow().is_empty(),
        "while the content fits it must report nothing, observed {:?}",
        recorded.borrow().as_slice()
    );

    // Grow past the box: two 300px blocks plus the 8px gap make 608px of
    // content in the 160px box, so `max_offset` becomes 448. The first flush
    // re-measures during its prepaint but its render still read the old
    // range, so the first edge is reported by the frame after it.
    *child_h.borrow_mut() = 300.;
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bottom"],
        "once the content outgrows the box, the first edge reported must be \
         the trailing one: still at the top, more content below"
    );
}
