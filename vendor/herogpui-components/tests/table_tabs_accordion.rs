//! Behaviour tests for the three surfaces the earlier suite left half driven:
//! the **Table**'s keyboard and its composed parts (sortable headers,
//! horizontal scroll, footer), the **Tabs** overflow scroller and its
//! separator, and the **Accordion**'s modes (multiple-expand, disabled keys,
//! and `DisclosureGroup`'s bodies).
//!
//! `tests/table_and_drag.rs` already covers Table sorting by click, row
//! selection, select-all, resize, load-more and virtualization; `tests/
//! collections.rs` covers ListBox/TagGroup/Tabs/Accordion basics. Nothing here
//! duplicates those — every test drives a path no earlier test did, and every
//! assertion is behavioural (recorded callbacks, or a probe click that must
//! record nothing), never appearance.
//!
//! Geometry is derived from the components' own constants, not guessed, and
//! every number carries its arithmetic in a comment:
//!
//! - Table rows are made exactly 105px tall (`tall_cell`: an `h(80)` filler
//!   plus `py-3` twice and a 1px border), the header spans 30..46px (12px
//!   text in a `py-2.5` cell), and the 44px checkbox column puts row *i*'s
//!   checkbox centre at y = 52 + 105i below the header.
//! - A sortable header is `py-2.5` around a 12px line; the 320px wrapper in
//!   the sort test pins the two flexing headers to 156px each.
//! - Tab labels are *measured* with the window's own text system, exactly as
//!   `tests/collections.rs` does; a tab is `px-4` (16px each side) around its
//!   14px label and the list carries `p-1` (4px).
//! - The Tabs scroller (`overflow-x-auto` in v3's sheet) only overflows when
//!   its list is `w-max` inside a bounded box; the chevrons are 16px circles
//!   at the box's edges (`start-1`/`end-1`, vertically centred), and the
//!   canvas that measures `max_offset` needs a few frames before they appear.
//! - An accordion trigger is `px-4 py-4` (16px all round) around one 20px
//!   line: 52px, with a 1px separator between items. An open body is
//!   `pt-2 pb-4` + its content, so a fixed 40px content makes everything
//!   below shift by an exact 58px.
//! - A `DisclosureGroup` trigger is a md `Button` (`h-9`, 36px) stretched to
//!   the group width, and an open body is `p-2` (8px all round) around its
//!   content.
//!
//! Two regressions pin defects the suite found: the Table's horizontal content
//! must exceed its viewport, and the Tabs chevrons must occlude the tabs below.

use crate::harness;
use crate::source_scan;

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::{rc::Rc, time::Duration};

use gpui::{
    point, prelude::*, px, Font, FontFeatures, FontStyle, FontWeight, ScrollDelta, ScrollEvent,
    SharedString, TestAppContext, VisualTestContext,
};
use herogpui_components::{
    Accordion, AccordionItem, DisclosureGroup, Orientation, Pagination, SelectionMode,
    SortDescriptor, SortDirection, TabItem, Table, TableColumn, Tabs,
};

use harness::{click, events, open_host, press, Events};

/// A single 80px-tall table cell, so rows have an exact height: 80 + `py-3`
/// (12px each side) + a 1px row border = 105px, which is what every row-based
/// y in this file derives from.
fn tall_cell(text: impl Into<SharedString>) -> gpui::AnyElement {
    gpui::div()
        .h(px(80.))
        .flex()
        .items_center()
        .child(text.into())
        .into_any_element()
}

/// A full-row-clickable cell that records `label`, used as the column probe:
/// the click lands inside whichever column owns that x at that moment, so the
/// recorded label names the column behaviourally.
fn probe_cell(id: &'static str, label: &'static str, recorded: Events) -> gpui::AnyElement {
    gpui::div()
        .id(id)
        .w_full()
        .h(px(80.))
        .flex()
        .items_center()
        .cursor_pointer()
        .on_click(move |_, _, _| recorded.borrow_mut().push(label.to_owned()))
        .child(label)
        .into_any_element()
}

/// A fixed-size clickable box recording `label`, for a click target whose
/// geometry is its own rather than a cell's (the footer probe).
fn box_probe(
    id: &'static str,
    label: &'static str,
    w: f32,
    h: f32,
    recorded: Events,
) -> gpui::AnyElement {
    gpui::div()
        .id(id)
        .w(px(w))
        .h(px(h))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_click(move |_, _, _| recorded.borrow_mut().push(label.to_owned()))
        .child(label)
        .into_any_element()
}

/// The advance width of `text` shaped the way the components shape it: gpui's
/// default `.SystemUIFont` stack at `size` px and `weight` (copied from
/// `tests/collections.rs` — the labels are laid out by the window's own
/// `WindowTextSystem`, so this measurement is the render's measurement).
fn text_width(system: &gpui::WindowTextSystem, text: &str, size: f32, weight: FontWeight) -> f32 {
    let run = gpui::TextRun {
        len: text.len(),
        font: Font {
            family: ".SystemUIFont".into(),
            features: FontFeatures::default(),
            weight,
            style: FontStyle::default(),
            fallbacks: None,
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = system.shape_line(text.to_owned().into(), px(size), &[run], None);
    f32::from(line.width)
}

/// The keys of an expanded set joined in a stable order.
///
/// A `HashSet` iterates in no particular order, so asserting on a raw join
/// would be flaky; sorting makes the recorded report deterministic.
fn sorted_join(keys: &HashSet<SharedString>) -> String {
    let mut keys: Vec<String> = keys.iter().map(ToString::to_string).collect();
    keys.sort();
    keys.join(",")
}

/// Pushes the pending frame through. Mouse and wheel events hit-test the
/// *last rendered frame*, so anything that changes state — a scroll, an
/// accordion header opening — needs a redraw before the next event, or the
/// next event lands on the stale layout.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// Drives the shared 200ms measured panel transition to its endpoint. Since
/// gpui-pre 0.3.11 GPUI's animation phase uses the deterministic executor
/// clock, as the retained exit timer does, so it is advanced here before a
/// geometry assertion or a click below a panel.
fn settle_collapsible(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(220));
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    cx.executor().advance_clock(Duration::from_millis(210));
    cx.run_until_parked();
    flush_frame(cx);
}

/// One horizontal wheel at window coordinates (`x`, `y`), scrolling `dx`
/// pixels: **negative moves the content left** (later columns into view),
/// matching the scrollable element's `scroll_offset.x += delta.x` with
/// negative offsets meaning "scrolled right". The delta is `Pixels`, not
/// `Lines`, so no line height enters the arithmetic. Followed by a redraw so
/// the next event sees the frame the scroll produced.
fn wheel_h(cx: &mut VisualTestContext, x: f32, y: f32, dx: f32) {
    cx.simulate_event(ScrollEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Pixels(point(px(dx), px(0.))),
        ..Default::default()
    });
    flush_frame(cx);
}

// ---------------------------------------------------------------------------
// Table
// ---------------------------------------------------------------------------

/// The table body is one tab stop with a cursor inside it: v3's grid roves a
/// single stop over the rows, and this port's `list_nav` resolver walks the
/// same four keys every list-shaped control does. The cursor itself is
/// internal, so the proof is behavioural: Tab reaches the table, arrows move
/// the cursor without reporting, Enter performs the row action, and Space is
/// reserved for selection (Home/End jump the cursor).
#[gpui::test]
fn table_keyboard_rows_rove_and_activate(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // No sortable columns, so the wrapper (which holds the body's one tab
        // stop) is the only stop on the page and a single Tab lands on it.
        Table::new(vec![])
            .id("tbl-keys")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .keyed_row("alpha", vec![tall_cell("Alpha")])
            .keyed_row("beta", vec![tall_cell("Beta")])
            .keyed_row("gamma", vec![tall_cell("Gamma")])
            .on_row_click(move |i, _, _, _| recorded.borrow_mut().push(format!("row:{i}")))
            .into_any_element()
    });

    // Tab: root -> wrapper (the only stop). Down moves the cursor to row 0;
    // Enter activates it. Down again to row 1; Space is inert because this
    // action-only table has no selection. Home jumps the cursor back to row 0;
    // End jumps it to the last row.
    press(cx, "tab");
    press(cx, "down");
    press(cx, "enter");
    press(cx, "down");
    press(cx, "space");
    press(cx, "home");
    press(cx, "enter");
    press(cx, "end");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row:0", "row:0", "row:2"],
        "Enter must activate the row action, Space must remain reserved for \
         selection, and Home and End must jump the cursor"
    );
}

/// Pinned React Aria 3.51's Table delegate sends PageDown to the last enabled
/// row, and PageUp out of the body into the first column header.
///
/// The header is focusable whether or not it sorts, so PageUp leaves the body
/// rather than stopping at its first row: the cursor stays where PageDown put
/// it, and Enter no longer reaches the body at all.
#[gpui::test]
fn table_page_keys_rove_a_plain_body_to_its_ends(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("tbl-plain-page")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .keyed_row("alpha", vec![tall_cell("Alpha")])
            .keyed_row("beta", vec![tall_cell("Beta")])
            .keyed_row("gamma", vec![tall_cell("Gamma")])
            .keyed_row("delta", vec![tall_cell("Delta")])
            .disabled_keys(["alpha", "delta"])
            .on_row_click(move |i, _, _, _| recorded.borrow_mut().push(format!("row:{i}")))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row:2"],
        "PageDown must reach the last enabled row of a plain Table body"
    );

    // PageUp leaves the body. The proof is that Enter no longer activates a
    // row: the focus is on the header, not on the cursor it left behind.
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row:2"],
        "PageUp must move the focus into the first column header, so Enter no          longer reaches the row the cursor was left on"
    );
    press(cx, "pagedown");
    press(cx, "enter");
    press(cx, "pageup");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row:2", "row:2", "row:1"],
        "PageDown and Down from a header must reach the last and first enabled body rows"
    );
}

#[gpui::test]
fn table_focusable_header_preserves_fixed_column_width(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(800.))
            .child(
                Table::new(vec![])
                    .id("tbl-header-width")
                    .column(TableColumn::new("Name").default_width(px(100.)))
                    .column(
                        TableColumn::new("Role")
                            .default_width(px(300.))
                            .allows_sorting(true),
                    )
                    .row(vec![tall_cell("Alpha"), tall_cell("Developer")])
                    .on_sort_change(move |sort, _, _| {
                        recorded.borrow_mut().push(sort.column.to_string());
                    }),
            )
            .into_any_element()
    });
    // The first fixed column ends at x=100 even when the host has spare width.
    // Its focus target must not move the second header away from its body cell.
    click(cx, 140., 18.);
    assert_eq!(recorded.borrow().as_slice(), ["Role"]);
}

/// A sortable header is its own tab stop (the port's reading of "one stop per
/// sortable column"), and gpui fires a *focused* element's click listeners on
/// Enter and Space. The descriptor reported by the keys must be exactly the
/// one a click reports, and the custom indicator must receive that current
/// direction: same column flips, feeding the result back continues the cycle.
#[gpui::test]
fn table_sortable_header_answers_enter_space_then_click(cx: &mut TestAppContext) {
    let recorded = events();
    let indicator_seen: Rc<RefCell<Option<SortDirection>>> = Rc::new(RefCell::new(None));
    let indicator_for_view = indicator_seen.clone();
    let held: Rc<RefCell<Option<SortDescriptor>>> = Rc::new(RefCell::new(None));
    let held_for_view = held;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let indicator_seen = indicator_for_view.clone();
        let held_view = held_for_view.clone();
        // Two sortable columns at 160px inside a 320px wrapper: each flexing
        // header cell is ~156px wide, so the click at x = 80 lands in column
        // 0. Sorting is controlled: the caller feeds the descriptor back,
        // which is what makes the second activation on the same column flip.
        let mut table = Table::new(vec![])
            .id("tbl-sort-keys")
            .columns(vec![
                TableColumn::new("Name")
                    .allows_sorting(true)
                    .default_width(px(160.)),
                TableColumn::new("Size")
                    .allows_sorting(true)
                    .default_width(px(160.)),
            ])
            .row(vec![gpui::div().child("A").into_any_element()])
            .row(vec![gpui::div().child("B").into_any_element()]);
        if let Some(d) = held_view.borrow().clone() {
            table = table.sort_descriptor(d);
        }
        gpui::div()
            .w(px(320.))
            .child(
                table
                    .indicator(move |direction| {
                        *indicator_seen.borrow_mut() = Some(direction);
                        gpui::div().into_any_element()
                    })
                    .on_sort_change(move |d, _, _| {
                        let dir = if d.direction == SortDirection::Ascending {
                            "asc"
                        } else {
                            "desc"
                        };
                        recorded.borrow_mut().push(format!("{}:{dir}", d.column));
                        *held_view.borrow_mut() = Some(d.clone());
                    })
                    .into_any_element(),
            )
            .into_any_element()
    });

    // Tab 1 lands on the wrapper (the body stop), Tab 2 on the first
    // sortable header's own stop. Enter, Enter, Space all activate the
    // focused header; the header row is `py-2.5` around a 12px line
    // (~37px), so y = 18 is inside it for the click.
    // A keyboard activation fires the header's click listener without marking
    // the window dirty, so the next key dispatch would hit the *last rendered
    // frame* — whose captured `next` descriptor is still the pre-activation
    // one. The harness flushes a frame after each activation (the feed-back
    // lives in test state; a real caller re-renders through its own state).
    press(cx, "tab tab");
    press(cx, "enter");
    flush_frame(cx);
    assert_eq!(
        *indicator_seen.borrow(),
        Some(SortDirection::Ascending),
        "the custom indicator must receive the first controlled sort direction"
    );
    press(cx, "enter");
    flush_frame(cx);
    assert_eq!(
        *indicator_seen.borrow(),
        Some(SortDirection::Descending),
        "the custom indicator must receive the flipped controlled sort direction"
    );
    press(cx, "space");
    flush_frame(cx);
    click(cx, 80., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Name:asc", "Name:desc", "Name:asc", "Name:desc"],
        "a focused sortable header must sort on Enter and Space exactly like \
         a click: same column flips, feeding the descriptor back continues \
         the cycle"
    );
}

/// The Table's `{id}-scroll-x` container is `overflow-x-auto` — v3's
/// `.table__scroll-container`, which is what lets a table wider than its box
/// scroll instead of being clipped. No prop pins a column (v3's Table page
/// documents no pinned/sticky column and ships no example of one; the only
/// `sticky` in its sheet is the *header* row), so the honest contract is:
/// the whole body slides under a fixed pointer, and a column that was
/// entirely off-screen comes into reach.
///
/// The assertion below pins that contract with fixed-pointer probes before and
/// after the wheel.
#[gpui::test]
fn table_horizontal_scroll_moves_all_columns(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Three 160px columns with a 160px floor inside a 240px wrapper: the
        // folded content is 480px wide against a ~232px viewport (the
        // wrapper's `px-1` tray), and `min_width` forbids flex-shrink from
        // compressing the columns — so a working scroller must slide the
        // body under the pointer.
        gpui::div()
            .w(px(240.))
            .child(
                Table::new(vec![])
                    .id("tbl-hscroll")
                    .columns(vec![
                        TableColumn::new("A")
                            .default_width(px(160.))
                            .min_width(px(160.)),
                        TableColumn::new("B")
                            .default_width(px(160.))
                            .min_width(px(160.)),
                        TableColumn::new("C")
                            .default_width(px(160.))
                            .min_width(px(160.)),
                    ])
                    .row(vec![
                        probe_cell("hs-a", "col-a", recorded.clone()),
                        probe_cell("hs-b", "col-b", recorded.clone()),
                        probe_cell("hs-c", "col-c", recorded),
                    ])
                    .into_any_element(),
            )
            .into_any_element()
    });

    // Column 0 spans x 0..160 (its probe, inset by the cell's `px-4`, spans
    // 16..144), column 1 x 160..320, column 2 x 320..480. The viewport clips
    // at x = 232, so before any scroll only columns 0 and part of 1 are
    // reachable. The row band: header [30,46] + 80px filler + `py-3` twice +
    // a 1px border, so y = 90 is inside row 0 for every header in the bound.
    click(cx, 100., 90.);
    click(cx, 225., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["col-a", "col-b"],
        "before the scroll, x=100 names the first column and x=225 the second"
    );

    // A 120px wheel to the left (negative horizontal delta) must slide the
    // body: column 1 moves to 40..200 (probe 56..184) and column 2 to
    // 200..360 (probe 216..344), so the same fixed x names the next column
    // over and the column that was clipped past x=232 comes under the
    // pointer. The wheel lands inside the scroller at (100, 90).
    wheel_h(cx, 100., 90., -120.);
    click(cx, 100., 90.);
    click(cx, 225., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["col-a", "col-b", "col-b", "col-c"],
        "a horizontal wheel must move the body under a fixed pointer: the x \
         that named the first column must name the second, and a column that \
         was clipped off-screen must come into reach"
    );
}

/// `Table.Footer` is the row under the body where v3 puts a table's
/// pagination. The port exposes it as `Table::footer(content)` — a plain
/// child row — so the proof is that content *inside* it answers inputs: a
/// probe and a `Pagination` both report from within the footer.
#[gpui::test]
fn table_footer_hosts_pagination_and_reports(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // One 105px row under the ~37px header, then the footer
        // (`px-4 py-2.5`, 16px side padding, 10px vertical) holding a fixed
        // 60x32 probe and a size-md pagination 16px apart.
        Table::new(vec![])
            .id("tbl-footer")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .row(vec![tall_cell("A")])
            .footer(
                gpui::div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .child(box_probe(
                        "ft-probe",
                        "footer-probe",
                        60.,
                        32.,
                        recorded.clone(),
                    ))
                    .child(
                        Pagination::new("ft-page", 1, 3).on_change(move |page, _, _| {
                            recorded.borrow_mut().push(format!("page:{page}"));
                        }),
                    ),
            )
            .into_any_element()
    });

    // Footer top = header (30..46) + 105; its content band is that plus 10
    // (py-2.5 top) and 32 tall, so y = 168 sits inside it for every header in
    // the bound. The probe spans x 16..76 (footer px-4), so x = 46 is its
    // centre.
    click(cx, 46., 168.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["footer-probe"],
        "the footer must render its content and answer a click inside it"
    );

    // The pagination starts at 16 (footer padding) + 60 (probe) + 16 (gap) =
    // 92. Its size-md row: prev pill `px-2.5` around a 14px glyph (34px),
    // `gap-1` (4px), three 32px page cells, then the next pill — 180px of
    // content, so the next button spans x 92+146..92+180, centre 92+163.
    click(cx, 255., 168.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["footer-probe", "page:2"],
        "a Table.Footer must host a live pagination: pressing its next arrow \
         reports page 2"
    );
}

/// HeroUI's `Table.Content` forwards React Aria's inherited `disabledKeys`.
/// The default `disabledBehavior="all"` removes that row from every
/// interaction: the roving cursor skips it and its pointer checkbox is inert,
/// while enabled siblings keep reporting the controlled selection.
#[gpui::test]
fn table_disabled_keys_skip_keyboard_and_pointer_selection(cx: &mut TestAppContext) {
    let recorded = events();
    let held: Rc<RefCell<Vec<SharedString>>> = Rc::new(RefCell::new(Vec::new()));
    let held_for_view = held;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let held_view = held_for_view.clone();
        // Selection is controlled; rows report the whole next set and the
        // caller feeds it back. The 204px wrapper fixes the table at the 44px
        // selection column + a 160px data column.
        let mut table = Table::new(vec![])
            .id("tbl-disabled-keys")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .disabled_keys(["beta"])
            .keyed_row("alpha", vec![tall_cell("Alpha")])
            .keyed_row("beta", vec![tall_cell("Beta")])
            .keyed_row("gamma", vec![tall_cell("Gamma")])
            .selected_keys(held_view.borrow().iter().cloned());
        table = table.on_selection_change(move |keys, _, _| {
            let joined = keys
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            recorded.borrow_mut().push(joined);
            *held_view.borrow_mut() = keys.to_vec();
        });
        gpui::div()
            .w(px(204.))
            .child(table.into_any_element())
            .into_any_element()
    });

    // The entry Down lands on Alpha. The next Down skips disabled Beta and
    // lands on Gamma, so the two Enters select only those enabled rows.
    press(cx, "tab");
    press(cx, "down");
    press(cx, "enter");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,gamma"],
        "the roving cursor must skip the disabled row"
    );

    // The 44px selection column centres its checkbox at x = 22; Beta is the
    // second 105px row below the ~37px header, centred near y = 195. A click
    // on that disabled checkbox must not report or mutate the controlled set.
    click(cx, 22., 195.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,gamma"],
        "a disabled row's checkbox must be inert"
    );
}

/// If a focused row becomes disabled between frames, React Aria's default
/// `disabledBehavior="all"` removes it from activation immediately. The stale
/// cursor must not let Enter invoke that row; the next Down re-enters at the
/// first enabled row.
#[gpui::test]
fn table_newly_disabled_cursor_cannot_activate(cx: &mut TestAppContext) {
    let recorded = events();
    let disable_beta = Rc::new(Cell::new(false));
    let disabled_for_view = disable_beta.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let mut table = Table::new(vec![])
            .id("tbl-dynamic-disabled")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .keyed_row("alpha", vec![tall_cell("Alpha")])
            .keyed_row("beta", vec![tall_cell("Beta")])
            .keyed_row("gamma", vec![tall_cell("Gamma")]);
        if disabled_for_view.get() {
            table = table.disabled_keys(["beta"]);
        }
        table
            .on_row_click(move |index, _, _, _| {
                recorded.borrow_mut().push(format!("row:{index}"));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "down");
    disable_beta.set(true);
    cx.update(|window, _| window.refresh());
    press(cx, "enter");
    assert!(
        recorded.borrow().is_empty(),
        "a row disabled while focused must not answer Enter"
    );

    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row:0"],
        "after the stale cursor clears, Down must re-enter at the first enabled row"
    );
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

/// Remaining source-text check. A constrained tab keeps its box whether its
/// label wraps or overflows (`tabs_full_width_does_not_overflow_the_scroller`
/// and `tabs_vertical_labels_wrap_inside_the_pinned_fixed_height` measure the
/// boxes, and both still pass with a `nowrap` label), the label slot carries
/// no debug selector, and glyph runs are not exposed, so the wrap itself is
/// not observable headlessly.
#[test]
fn tabs_use_a_constrained_normal_whitespace_label_slot() {
    let source = source_scan::component_src("tabs.rs");
    assert_eq!(
        source.matches(".whitespace_normal()").count(),
        1,
        "both variants must share the same normal-whitespace label slot"
    );
    assert!(
        source.contains(".min_w(px(0.))") && source.contains(".when(label_constrained"),
        "the label slot must release its min-content width when constrained"
    );
    assert!(
        !source.contains(".whitespace_nowrap()"),
        "Tabs must not force labels into an intrinsic-width text node"
    );
}

/// The overflow scroller: `.tabs__list` is `w-max`, so inside a bounded box
/// the row grows past the box and `.tabs__list-container__scroller` scrolls
/// it; the measuring canvas (which reads `ScrollHandle::max_offset` — written
/// during prepaint, so it takes a frame or two to appear) turns the chevrons
/// on only when there is something that way to scroll.
///
/// Geometry, all derived (labels measured with the window's text system):
///
/// - list padding `p-1` (4px), tab = `px-4` (32px) + measured label; tab *i*
///   starts at `4 + sum_{j<i}(w_j + 32)`.
/// - the scroller's visible width is the 240px box; `max` = list width - 240.
/// - the next chevron is a 16px circle at `right-1` (x 220..236) vertically
///   centred on the 40px list (y 12..28); the prev chevron mirrors it at
///   `left-1` (x 4..20).
/// - one chevron click scrolls by 80% of the 240px viewport: 192px.
#[gpui::test]
fn tabs_overflow_scroller_chevrons_scroll_the_list(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(240.))
            .child(
                Tabs::new(
                    "tb-ol",
                    vec![
                        TabItem::new("one", "One"),
                        TabItem::new("two", "Two"),
                        TabItem::new("three", "Three"),
                        TabItem::new("four", "Four"),
                        TabItem::new("five", "Five"),
                        TabItem::new("six", "Six"),
                        TabItem::new("seven", "Seven"),
                        TabItem::new("eight", "Eight"),
                        TabItem::new("nine", "Nine"),
                        TabItem::new("ten", "Ten"),
                    ],
                    "one",
                )
                .on_selection_change(move |key, _, _| {
                    recorded.borrow_mut().push(key.to_string());
                })
                .into_any_element(),
            )
            .into_any_element()
    });

    let labels = [
        "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
    ];
    let mut starts = [0f32; 10];
    let mut centres = [0f32; 10];
    let mut x = 4.;
    for (i, label) in labels.iter().enumerate() {
        // The shaped advance is rounded *up* to whole pixels by gpui's own
        // text measurement, so the layout widths here mirror the box sizes.
        let w = cx
            .update(|window, _| text_width(window.text_system(), label, 14.0, FontWeight::MEDIUM))
            .ceil();
        starts[i] = x;
        centres[i] = x + (w + 32.) / 2.;
        x += w + 32.;
    }
    let scrolled = 240. * 0.8;

    // The measuring canvas needs a few frames: `max_offset` is written during
    // the scroller's prepaint and the chevron renders one frame later.
    flush_frame(cx);
    flush_frame(cx);
    flush_frame(cx);

    // "Five" sits entirely past the 240px viewport (its centre is about 273),
    // so a click at its would-be centre records nothing.
    click(cx, centres[4], 20.);
    assert!(
        recorded.borrow().is_empty(),
        "the last tab starts beyond the viewport: a click where it would be \
         (x = {}) must record nothing",
        centres[4]
    );

    // The next chevron at (228, 20) must scroll the list left by 192px,
    // sliding "Five" to `centres[4] - scrolled`. The old fixed 120px step
    // leaves "Four" at that coordinate, so this distinguishes the contract.
    click(cx, 228., 20.);
    flush_frame(cx);
    click(cx, centres[4] - scrolled, 20.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["five"],
        "a chevron click must move by 80% of the viewport and must not select anything itself"
    );

    // The previous chevron must move the same viewport-relative step back.
    flush_frame(cx);
    flush_frame(cx);
    flush_frame(cx);
    click(cx, 12., 20.);
    flush_frame(cx);
    click(cx, starts[3] + 8., 20.);
    assert_eq!(recorded.borrow().as_slice(), ["five", "four"]);
}

/// Vertical overflow uses the same 80%-of-viewport contract on the y axis.
/// Ten 32px tabs, nine 4px gaps and 8px list padding make a 364px column in a
/// 160px viewport; the next chevron scrolls it by 128px, moving tab five's top
/// from y=148 to y=20. The y=24 probe would miss after the old fixed 120px step.
#[gpui::test]
fn tabs_vertical_overflow_chevrons_scroll_the_list(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .h(px(160.))
            .child(
                Tabs::new(
                    "tb-vertical-overflow",
                    vec![
                        TabItem::new("one", "One"),
                        TabItem::new("two", "Two"),
                        TabItem::new("three", "Three"),
                        TabItem::new("four", "Four"),
                        TabItem::new("five", "Five"),
                        TabItem::new("six", "Six"),
                        TabItem::new("seven", "Seven"),
                        TabItem::new("eight", "Eight"),
                        TabItem::new("nine", "Nine"),
                        TabItem::new("ten", "Ten"),
                    ],
                    "one",
                )
                .orientation(Orientation::Vertical)
                .on_selection_change(move |key, _, _| {
                    recorded.borrow_mut().push(key.to_string());
                })
                .into_any_element(),
            )
            .into_any_element()
    });

    flush_frame(cx);
    flush_frame(cx);
    flush_frame(cx);

    let indicator = cx
        .debug_bounds(r#"Name("tb-vertical-overflow")-indicator"#)
        .expect("indicator paints");
    wheel_h(cx, 40., 80., -40.);
    assert_eq!(
        cx.debug_bounds(r#"Name("tb-vertical-overflow")-indicator"#),
        Some(indicator),
        "horizontal wheel must not scroll vertical tabs"
    );

    click(cx, 40., 164.);
    assert!(
        recorded.borrow().is_empty(),
        "tab five must start clipped below the vertical viewport"
    );

    click(cx, 40., 148.);
    flush_frame(cx);
    click(cx, 40., 24.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["five"],
        "the vertical chevron must move the list by 80% of its viewport"
    );

    flush_frame(cx);
    flush_frame(cx);
    flush_frame(cx);
    click(cx, 40., 12.);
    flush_frame(cx);
    click(cx, 40., 32.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["five", "one"],
        "the previous vertical chevron must restore the same viewport-relative step"
    );
}

/// Pinned React Aria hands the tab-list scroller to its selectable collection,
/// which scrolls the newly keyboard-focused tab into view. The click after the
/// arrow sequence proves that the fifth tab moved under its nearest visible
/// coordinate rather than remaining clipped beyond the 240px viewport.
#[gpui::test]
fn tabs_keyboard_navigation_scrolls_focused_tab_into_view(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(240.))
            .child(
                Tabs::new(
                    "tb-keyboard-overflow",
                    vec![
                        TabItem::new("one", "One"),
                        TabItem::new("two", "Two"),
                        TabItem::new("three", "Three"),
                        TabItem::new("four", "Four"),
                        TabItem::new("five", "Five"),
                        TabItem::new("six", "Six"),
                        TabItem::new("seven", "Seven"),
                        TabItem::new("eight", "Eight"),
                        TabItem::new("nine", "Nine"),
                        TabItem::new("ten", "Ten"),
                    ],
                    "one",
                )
                .on_selection_change(move |key, _, _| {
                    recorded.borrow_mut().push(key.to_string());
                })
                .into_any_element(),
            )
            .into_any_element()
    });

    let labels = ["One", "Two", "Three", "Four", "Five"];
    let mut starts = [0f32; 5];
    let mut centres = [0f32; 5];
    let mut widths = [0f32; 5];
    let mut x = 4.;
    for (i, label) in labels.iter().enumerate() {
        let width = cx
            .update(|window, _| text_width(window.text_system(), label, 14.0, FontWeight::MEDIUM))
            .ceil()
            + 32.;
        starts[i] = x;
        centres[i] = x + width / 2.;
        widths[i] = width;
        x += width;
    }

    flush_frame(cx);
    flush_frame(cx);
    flush_frame(cx);
    press(cx, "tab right right right right");
    flush_frame(cx);
    recorded.borrow_mut().clear();

    let nearest_offset = starts[4] + widths[4] - 240.;
    click(cx, centres[4] - nearest_offset, 20.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["five"],
        "keyboard navigation must scroll the focused tab into view"
    );
}

/// Keyboard focus entry uses the same pinned scroll contract as an arrow move.
/// Starting on the fifth tab leaves it beyond the bounded scroller; Tab must
/// reveal it before the pointer probe can reach it.
#[gpui::test]
fn tabs_keyboard_entry_scrolls_selected_tab_into_view(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(240.))
            .child(
                Tabs::new(
                    "tb-keyboard-entry",
                    vec![
                        TabItem::new("one", "One"),
                        TabItem::new("two", "Two"),
                        TabItem::new("three", "Three"),
                        TabItem::new("four", "Four"),
                        TabItem::new("five", "Five"),
                    ],
                    "five",
                )
                .on_selection_change(move |key, _, _| {
                    recorded.borrow_mut().push(key.to_string());
                })
                .into_any_element(),
            )
            .into_any_element()
    });

    flush_frame(cx);
    flush_frame(cx);
    cx.update(|_, cx| herogpui_components::extend::set_focus_visible(true, cx));
    press(cx, "tab");
    flush_frame(cx);

    let labels = ["One", "Two", "Three", "Four", "Five"];
    let mut start = 4.;
    let mut width = 0.;
    for label in labels {
        width = cx
            .update(|window, _| text_width(window.text_system(), label, 14.0, FontWeight::MEDIUM))
            .ceil()
            + 32.;
        if label != "Five" {
            start += width;
        }
    }
    let centre = start + width / 2.;
    let nearest_offset = start + width - 240.;
    click(cx, centre - nearest_offset, 20.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["five"],
        "keyboard entry must scroll the selected tab into view"
    );
}

/// `TabItem::separator()` composes v3's `Tabs.Separator` — a 1px hairline
/// positioned inside the tab that carries it. The line ignores pointer events,
/// so its pixel remains part of the following tab's hit area without becoming
/// a separate keyboard stop.
#[gpui::test]
fn tabs_separator_stays_inside_the_following_tab(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // The separator is the leading 1px inside "two", vertically centred.
        Tabs::new(
            "tb-sep",
            vec![
                TabItem::new("one", "One"),
                TabItem::new("two", "Two").separator(),
                TabItem::new("three", "Three"),
            ],
            "one",
        )
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    // The list is `p-1`, so tab "one" spans x 4..(4 + w_one + 32) and tab
    // "two" begins at that boundary with its separator overlaid inside it.
    // gpui's own text measurement rounds the shaped advance *up* to whole
    // pixels (`TextLayout` ceils the line width) — exactly how the tab's box
    // is sized — so the measured width must be rounded the same way before
    // the boundary arithmetic. A click half a pixel into the line must still
    // land on tab "two" because the separator itself is pointer-inert.
    let w_one =
        cx.update(|window, _| text_width(window.text_system(), "One", 14.0, FontWeight::MEDIUM));
    let tab_one_end = 4. + w_one.ceil() + 32.;
    click(cx, tab_one_end + 0.5, 20.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["two"],
        "the separator pixel must remain inside the following tab's hit area"
    );

    // The pointer focused "two", and Right moves directly to "three" — the
    // separator never became a second stop.
    press(cx, "right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["two", "three"],
        "the separator must not become a keyboard stop"
    );
}

/// The port's spelling of a disabled tab is `Tabs::is_disabled`, which
/// disables the *whole* list. Per-tab `TabItem::is_disabled` is driven in the
/// deeper Tabs suite. A disabled list must leave the tab order and answer no
/// key and no click.
#[gpui::test]
fn tabs_disabled_list_answers_no_key_or_click(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Tabs::new(
            "tb-dead",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
                TabItem::new("third", "Third"),
            ],
            "first",
        )
        .is_disabled(true)
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    // No tab claims the list's focus handle (`track_focus` is gated on
    // `!is_disabled`) and no tab has a click handler, so Tab reaches nothing
    // and every key the live list would answer falls through to the host
    // root, which has no handlers either.
    press(cx, "tab");
    press(cx, "right");
    press(cx, "left");
    press(cx, "home");
    press(cx, "end");
    press(cx, "enter");
    press(cx, "space");

    // A click on the second tab's measured centre (list p-1 + the first
    // tab's label+32 + half of the second's) must record nothing either.
    let w_first =
        cx.update(|window, _| text_width(window.text_system(), "First", 14.0, FontWeight::MEDIUM));
    let w_second =
        cx.update(|window, _| text_width(window.text_system(), "Second", 14.0, FontWeight::MEDIUM));
    let tab1_centre_x = 4. + w_first + 32. + (w_second + 32.) / 2.;
    click(cx, tab1_centre_x, 20.);
    assert!(
        recorded.borrow().is_empty(),
        "a disabled tab list must not answer Tab, the arrows, Enter, Space or \
         a click"
    );
}

// ---------------------------------------------------------------------------
// Accordion
// ---------------------------------------------------------------------------

/// Accordion and Disclosure share `anim::collapsible_panel`: a toggled panel
/// eases its height on the pinned 200ms `DISCLOSURE` motion (measured on
/// the executor clock gpui animations read) instead of snapping, and
/// the collapsing body stays mounted until the motion ends. A probe below
/// the component reads the height it takes up.
#[gpui::test]
fn accordion_and_disclosure_panels_ease_open_and_closed(cx: &mut TestAppContext) {
    type Build = fn() -> gpui::AnyElement;
    let builds: [(&str, Build); 2] = [
        ("Accordion", || {
            Accordion::new(vec![
                AccordionItem::new("one", "Item one").content(gpui::div().h(px(120.)))
            ])
            .id("acc-ease")
            .into_any_element()
        }),
        ("Disclosure", || {
            herogpui_components::Disclosure::new("dis-ease", "Details")
                .child(gpui::div().h(px(120.)))
                .into_any_element()
        }),
    ];
    for (name, build) in builds {
        let cx = open_host(cx, move || {
            gpui::div()
                .flex()
                .flex_col()
                .w(px(400.))
                .child(build())
                .child(harness::probe("below"))
                .into_any_element()
        });
        harness::settle(cx);
        let below = |cx: &mut VisualTestContext| {
            harness::settle(cx);
            f32::from(
                cx.debug_bounds("below")
                    .expect("the probe must be laid out")
                    .origin
                    .y,
            )
        };
        let closed = below(cx);
        click(cx, 100., closed / 2.);
        let opening = below(cx);
        harness::wait_ms(cx, 400);
        let open = below(cx);
        assert!(
            open >= closed + 119.,
            "{name}: the open panel must take its 120px body ({closed} -> {open})"
        );
        assert!(
            opening < open - 1.,
            "{name}: opening must ease the height, not snap ({closed} -> {opening} -> {open})"
        );

        click(cx, 100., closed / 2.);
        let closing = below(cx);
        assert!(
            closing > closed + 1.,
            "{name}: the collapsing body must stay mounted while it eases shut \
             ({open} -> {closing})"
        );
        harness::wait_ms(cx, 400);
        assert!(
            (below(cx) - closed).abs() < 0.5,
            "{name}: the closed panel must give its height back"
        );
    }
}

/// Remaining source-text check. The chevrons are `svg()` glyphs turned by a
/// paint transformation: the test platform draws no svg and exposes no
/// transforms, so the path and its 250ms rotation leave no trace.
#[test]
fn accordion_and_disclosure_indicators_use_shared_rotating_chevrons() {
    let accordion = source_scan::component_src("accordion.rs");
    let disclosure = source_scan::component_src("disclosure.rs");
    let animation = source_scan::component_src("anim.rs");

    assert!(accordion.contains("crate::anim::rotating_indicator"));
    assert!(accordion.contains(".path(icons::CHEVRON_DOWN)"));
    assert!(disclosure.contains("crate::anim::rotating_indicator"));
    assert!(disclosure.contains(".path(crate::icons::CHEVRON_DOWN)"));
    assert!(animation.contains("INDICATOR_ROTATION_MS: u64 = 250"));
    assert!(animation.contains("with_easing(tailwind_default_ease())"));
    assert!(animation.contains("rotation.target() * angle"));
    assert!(animation.contains("progress * angle"));
}

/// `allowsMultipleExpanded` opted in to true: expanding a second item must
/// not collapse the first, and the reported set must contain both keys at
/// once. (The prop now has to be set explicitly — v3's default is `false`,
/// which `nav_deep.rs` pins. The bodies are a fixed 40px so the second
/// header's seat is exact.)
#[gpui::test]
fn accordion_multiple_expand_keeps_both_open(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Accordion::new(vec![
            AccordionItem::new("one", "Item one").content(gpui::div().h(px(40.))),
            AccordionItem::new("two", "Item two").content(gpui::div().h(px(40.))),
        ])
        .id("acc-multi")
        .allows_multiple_expanded(true)
        .on_expanded_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    // Header 0 centre: y 26. With item one open, item two's header sits below
    // 52 (header) + 2+40+16 (body) + 1 (separator) + 26 (half header) = 137.
    // Both must be reported together — the multiple-expand mode.
    click(cx, 60., 26.);
    flush_frame(cx);
    settle_collapsible(cx);
    click(cx, 60., 137.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["one", "one,two"],
        "expanding the second item must report both keys, never collapse the \
         first"
    );

    // Closing them one at a time leaves the other open, then clears the set.
    // With item one collapsed again, item two's header moves back up to its
    // natural seat — 52 (header) + 1 (separator) + 26 (half header) = 79 —
    // which is where the closing click must land.
    click(cx, 60., 26.);
    flush_frame(cx);
    settle_collapsible(cx);
    click(cx, 60., 79.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["one", "one,two", "two", ""],
        "closing either item must report the set with the other still open, \
         and the last closure must report an empty set"
    );
}

/// A `disabledKeys` item cannot be toggled by click — its trigger has no
/// click handler — or by key: `track_focus` is gated on the disabled flag, so
/// the trigger leaves the tab order and no key event can ever reach it.
#[gpui::test]
fn accordion_disabled_item_cannot_be_toggled(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Accordion::new(vec![
            AccordionItem::new("one", "Item one"),
            AccordionItem::new("two", "Item two"),
        ])
        .id("acc-dis")
        .disabled_keys(["two".into()])
        .on_expanded_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    // The second header's centre: 52 + 1 (separator) + 26 = 79. It is dimmed
    // but present; a click must record nothing.
    click(cx, 60., 79.);
    assert!(
        recorded.borrow().is_empty(),
        "clicking a disabledKeys trigger must record nothing"
    );

    // Tab lands on the first (enabled) trigger and Enter fires its click
    // listener (gpui activates a focused element with click listeners on
    // Enter/Space). The disabled trigger is not a tab stop, so no Tab can
    // reach it: a second Tab finds no further stop and a second Enter just
    // toggles the first trigger again.
    press(cx, "tab");
    press(cx, "enter");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["one", ""],
        "the enabled trigger must answer Enter (open then close); the disabled \
         one must never appear in any report"
    );
}

/// `DisclosureGroup` stacks `Disclosure`s; an open body (`p-2` all round)
/// pushes the triggers below it down. The group is controlled, so the first
/// item is seeded open and its 40px body shifts the second and third
/// triggers by an exact 56px — the offset arithmetic is the point.
#[gpui::test]
fn disclosure_group_third_item_reports_with_bodies_pushing(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Each trigger is a md Button (`h-9`, 36px) stretched to the group's
        // full width by the flex column. With the first body open:
        //   trigger 1      y  0..36
        //   body 1         y 36..92   (8 + 40 + 8)
        //   trigger 2      y 92..128  (centre 110)
        //   trigger 3      y 128..164 (centre 146)
        // Without the open body, trigger 2 would sit at y 36..72 and trigger
        // 3 at 72..108 — the pushed-down seats only exist because the body
        // between them rendered.
        let mut expanded = HashSet::new();
        expanded.insert(SharedString::from("first"));
        DisclosureGroup::new("table-tabs-disclosure-group")
            .expanded_keys(expanded)
            .item("first", "Basic settings", gpui::div().h(px(40.)))
            .item("second", "Advanced settings", gpui::div().h(px(40.)))
            .item("third", "Team plan", gpui::div().h(px(40.)))
            .on_expanded_change(move |keys, _, _| {
                recorded.borrow_mut().push(sorted_join(keys));
            })
            .into_any_element()
    });

    // x = 40 is inside every stretched trigger. The third trigger answers at
    // y = 146 — a seat that exists only because the first item's open body
    // pushed it down by 56px; the second answers at 110.
    click(cx, 40., 110.);
    click(cx, 40., 146.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["second", "third"],
        "pressing the pushed-down second and third triggers must report their \
         keys; only the first item's open body can have put them there"
    );
}

#[gpui::test]
fn horizontal_components_keep_vertical_wheels_on_the_page(cx: &mut TestAppContext) {
    for kind in 0..3 {
        harness::still();
        let page_scroll = gpui::ScrollHandle::new();
        let page_for_view = page_scroll.clone();
        let probe_name = if kind == 1 {
            r#"Name("axis-tabs")-indicator"#
        } else {
            "axis-content"
        };
        let cx = open_host(cx, move || {
            let content = match kind {
                0 => Table::new(vec![])
                    .id("axis-table")
                    .columns(vec![TableColumn::new("Column")
                        .default_width(px(480.))
                        .min_width(px(480.))])
                    .row(vec![gpui::div()
                        .h(px(80.))
                        .w_full()
                        .debug_selector(|| "axis-content".to_owned())
                        .into_any_element()])
                    .into_any_element(),
                1 => Tabs::new(
                    "axis-tabs",
                    (0..10)
                        .map(|i| TabItem::new(format!("{i}"), format!("Tab {i}")))
                        .collect(),
                    "0",
                )
                .into_any_element(),
                _ => herogpui_components::ScrollShadow::new("axis-shadow")
                    .orientation(Orientation::Horizontal)
                    .max_w(px(240.))
                    .child(
                        gpui::div()
                            .w(px(480.))
                            .h(px(80.))
                            .flex_shrink_0()
                            .debug_selector(|| "axis-content".to_owned()),
                    )
                    .into_any_element(),
            };
            gpui::div()
                .id("axis-page")
                .w(px(240.))
                .h(px(200.))
                .overflow_y_scroll()
                .restrict_scroll_to_axis()
                .track_scroll(&page_for_view)
                .child(gpui::div().h(px(600.)).child(content))
                .into_any_element()
        });
        flush_frame(cx);
        flush_frame(cx);
        let before = cx.debug_bounds(probe_name).expect("content paints");
        let y = if kind == 0 { 90. } else { 20. };
        cx.simulate_event(ScrollEvent {
            position: point(px(100.), px(y)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-10.))),
            ..Default::default()
        });
        flush_frame(cx);
        assert_eq!(
            page_scroll.offset().y,
            px(-10.),
            "vertical wheel reaches page: kind={kind}"
        );
        let after_vertical = cx.debug_bounds(probe_name).expect("content stays visible");
        assert_eq!(
            after_vertical.origin.x, before.origin.x,
            "vertical wheel must not move content sideways: kind={kind}"
        );
        wheel_h(cx, 100., y - 10., -20.);
        let after_horizontal = cx
            .debug_bounds(probe_name)
            .expect("content stays visible after horizontal wheel");
        assert!(
            after_horizontal.origin.x < before.origin.x,
            "horizontal wheel moves component: kind={kind}"
        );
        assert_eq!(
            page_scroll.offset().y,
            px(-10.),
            "horizontal wheel leaves page in place: kind={kind}"
        );
    }
}
