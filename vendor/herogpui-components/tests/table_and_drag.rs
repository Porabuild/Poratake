//! Behaviour tests for the Table and the drag-driven controls: Slider,
//! ColorSlider, ColorArea and ColorSwatchPicker.
//!
//! Everything static about them is measured by the `.shots/*.py` audits; these
//! tests drive the controls -- a pointer drag, a click, a keyboard step -- and
//! assert on recorded callbacks and behavioural probes only, never on
//! appearance.
//!
//! Geometry is derived from the components' own constants and from fixed
//! wrapper widths, not guessed:
//!
//! - Every *sorting* table sits inside an explicit-width wrapper (`w(320)`):
//!   a table is `w_full` and a sortable header flexes, so without the wrapper
//!   the columns stretch to the host window. Resizable columns do not flex --
//!   `default_width(px(160.))` pins each column at exactly 160px -- and the
//!   selection tables get `w(204)` (44px checkbox column + 160px data) so the
//!   checkbox geometry can never stretch either.
//! - A table header cell is `py-2.5` (10px) around a 12px text line: about
//!   37px tall. Every click at `y = 18` targets the header; the exact line
//!   height only matters within 0..36, so the margin is a dozen pixels each
//!   way.
//! - Column resize handles sit at a column's trailing edge: the handle is
//!   `absolute right(-8) w(17)` inside the column's wrapper, so column 0's
//!   handle spans x 151..168 with its press at x = 160. The resize probe is
//!   behavioural: cells carry clickable probes ("cell-a" in column 0,
//!   "cell-b" in column 1), and a click at x = 180 names the column that owns
//!   that x -- column 1 before the drag (content starts at 176, after the
//!   cell's `px-4`), column 0 after a 40px drag (content ends at 184).
//! - Selection rows are made exactly 105px tall by putting an `h(80)` filler
//!   in every data cell (80 + `py-3` 12px each side + 1px row border). The
//!   44px checkbox column is `py-2.5` around a 16px (`size-4`) checkbox, so
//!   each row's checkbox centre sits 52px below the row top: row i's centre
//!   is y = 52 + 105i below the header, which the fixed click points 90/195/
//!   300 hit for any header height in 30..46px.
//! - The load-more sentinel is driven inside a fixed 160px parent scroller.
//!   Four 105px rows keep it outside the first content mask; a 400px wheel
//!   brings it into view without relying on a clickable replacement row.
//! - Sliders are wrapped in `w(600)`, which fixes the track at 600px because
//!   a horizontal slider is `w_full`; the track is 20px high at the
//!   window origin, so a drag at y = 10 travels along its centre. A drop at x maps to
//!   `x / 600 * (max - min)` (the track's own `set_from_x` arithmetic).
//! - The ColorSlider track is 240px wide (`length` default) and 16px tall;
//!   it is driven by keyboard here, so only its focus is needed, not its
//!   pixels.
//! - The swatch picker lays cells 32px (`size-8`) apart with an 8px gap:
//!   cell i's centre is x = 16 + 40i, y = 16.
//!
//! Drags are synthesised as the platform would deliver them, and the test
//! platform only redraws dirty windows at the next `App::update`, so every
//! mutating mouse event is followed by `flush_frame`:
//!
//! 1. `MouseDownEvent` with `button: Left, click_count: 1, first_mouse: false`;
//! 2. one or more `MouseMoveEvent`s with `pressed_button: Some(Left)` -- a
//!    single jump with no move is a click, and a component that tracks motion
//!    sees nothing;
//! 3. `MouseUpEvent` with the same `button`/`click_count` at the drop point.
//!
//! Reduce motion is deliberately **not** set for this process: none of these
//! components play an exit animation, so no animation phase can swallow a
//! probe.

use crate::harness;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    point, prelude::*, px, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ScrollDelta, ScrollEvent, SharedString, TestAppContext, VisualTestContext,
};
use herogpui_components::{
    ColorArea, ColorChannel, ColorSlider, ColorSwatchPicker, PickerColor, SelectionMode, Slider,
    SortDescriptor, SortDirection, Table, TableColumn, TableRow,
};

use harness::{click, events, open_host, press};

/// A single 80px-tall table cell, so rows have an exact height (see the file
/// doc comment) rather than a text-metric one.
fn tall_cell(text: impl Into<SharedString>) -> gpui::AnyElement {
    gpui::div()
        .h(px(80.))
        .flex()
        .items_center()
        .child(text.into())
        .into_any_element()
}

/// A full-row-clickable cell that records `label`, used as the column probe
/// in the resize test: the click lands inside whichever column owns the x at
/// that moment, so the recorded label names the column behaviourally.
fn probe_cell(
    id: &'static str,
    label: &'static str,
    recorded: harness::Events,
) -> gpui::AnyElement {
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

fn resize_event(stage: &str, widths: &[(SharedString, gpui::Pixels)]) -> String {
    let widths = widths
        .iter()
        .map(|(column, width)| format!("{column}={:.0}", f32::from(*width)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{stage}:{widths}")
}

/// Pushes the pending frame through. Mouse events are dispatched outside an
/// `App::update`, so a `cx.notify()` from a handler only marks the window
/// dirty; the test platform draws it at the end of the next update. Call
/// after every press/move/release whose effect the next event must see.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn press_mod_a(cx: &mut VisualTestContext) {
    if cfg!(target_os = "macos") {
        press(cx, "cmd-a");
    } else {
        press(cx, "ctrl-a");
    }
}

/// A real pointer drag: down at `from`, one move to `to` with the left button
/// held, up at `to`. See the file doc comment for the event shapes -- this is
/// the sequence every drag-driven control needs, and a plain `simulate_click`
/// would look like a press with no motion at all.
fn drag(cx: &mut VisualTestContext, from: (f32, f32), to: (f32, f32)) {
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(from.0), px(from.1)),
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    flush_frame(cx);
    cx.simulate_event(MouseMoveEvent {
        position: point(px(to.0), px(to.1)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    });
    flush_frame(cx);
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: point(px(to.0), px(to.1)),
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    flush_frame(cx);
}

fn wheel_v(cx: &mut VisualTestContext, x: f32, y: f32, dy: f32) {
    cx.simulate_event(ScrollEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
        ..Default::default()
    });
    flush_frame(cx);
}

// ---------------------------------------------------------------------------
// Table
// ---------------------------------------------------------------------------

#[gpui::test]
fn table_sortable_header_toggles_direction(cx: &mut TestAppContext) {
    let recorded = events();
    // Sorting is controlled in v3: a click reports the new descriptor and the
    // caller feeds it back, so the second click on the same column flips. The
    // test holds the descriptor in its own Rc and hands it back through
    // `sort_descriptor`; without the feedback every click would report the
    // same first click.
    let held: Rc<RefCell<Option<SortDescriptor>>> = Rc::new(RefCell::new(None));
    let held_for_view = held;

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let held_view = held_for_view.clone();
        // Two columns at 160px inside a 320px wrapper: column 0 spans
        // x 0..160 (centre 80), column 1 x 160..320 (centre 240). The header
        // row is ~37px tall, so y = 18 is inside it.
        let mut table = Table::new(vec![])
            .id("tbl-sort")
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
        // The 320px wrapper is what fixes the column geometry: without it the
        // table's `w_full` stretches to the host window (~2000px here) and the
        // flex-1 headers split that instead of 320.
        gpui::div()
            .w(px(320.))
            .child(
                table
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

    // First click starts ascending (React Aria's `SortDescriptor::next`).
    click(cx, 80., 18.);
    flush_frame(cx);
    // The same column again flips to descending.
    click(cx, 80., 18.);
    flush_frame(cx);
    // A different column resets to ascending.
    click(cx, 240., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Name:asc", "Name:desc", "Size:asc"],
        "sorting must toggle direction on repeat clicks and start ascending \
         on a new column"
    );
}

#[gpui::test]
fn table_row_selection_reports_keys(cx: &mut TestAppContext) {
    let recorded = events();
    // Selection is controlled in v3 too: rows report the new whole set and the
    // caller feeds it back, which is also how a repeat click on a picked row
    // can toggle it back off (the row's closure captured the set at render).
    let held: Rc<RefCell<Vec<SharedString>>> = Rc::new(RefCell::new(Vec::new()));
    let held_for_view = held;

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let held_view = held_for_view.clone();
        // Selection column is 44px wide (`size-8` checkbox + `py-2.5`), data
        // column 160px: the whole table is 204px. The 80px filler makes each
        // row 105px tall, so row i's checkbox centre is y = 52 + 105i below
        // the ~37px header: 90, 195, 300.
        let mut table = Table::new(vec![])
            .id("tbl-sel")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
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
        // The 204px wrapper fixes the table at selection column (44) + data
        // column (160), so the checkbox column can never flex to the host
        // window width.
        gpui::div()
            .w(px(204.))
            .child(table.into_any_element())
            .into_any_element()
    });

    // Row alpha at (22, 90) -- the 16px checkbox sits centred in the 44px
    // column.
    click(cx, 22., 90.);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha"],
        "clicking the first row's checkbox must select it"
    );

    // Row beta joins it, appended in click order (`selection::next_selection`).
    click(cx, 22., 195.);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta"],
        "a second row must extend the reported selection"
    );

    // Re-clicking alpha toggles it off -- only reachable if the reported set
    // was fed back and the row knew it was already selected.
    click(cx, 22., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta", "beta"],
        "re-clicking a picked row must deselect it"
    );
}

#[gpui::test]
fn table_select_all_checkbox_toggles_every_row(cx: &mut TestAppContext) {
    let recorded = events();
    let held: Rc<RefCell<Vec<SharedString>>> = Rc::new(RefCell::new(Vec::new()));
    let held_for_view = held;

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let held_view = held_for_view.clone();
        let mut table = Table::new(vec![])
            .id("tbl-all")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
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
        // The 204px wrapper fixes the table at selection column (44) + data
        // column (160), so the checkbox column can never flex to the host
        // window width.
        gpui::div()
            .w(px(204.))
            .child(table.into_any_element())
            .into_any_element()
    });

    // The header checkbox is a 16px box centred in a `py-2.5` cell at the top
    // of the 44px selection column: x 22, y 18. This was a real bug once -- a
    // dead "select all" that never reported.
    click(cx, 22., 18.);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,beta,gamma"],
        "the header checkbox must select every row"
    );

    // Second click: with every row selected the same checkbox must clear all.
    click(cx, 22., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,beta,gamma", ""],
        "the header checkbox must clear every row again"
    );
}

/// The pinned React Aria 3.51 `useSelectableCollection` binds `Mod+A` -- the
/// platform Mod, Control off macOS and Command on macOS -- to `selectAll`, which only runs
/// in multiple-selection mode. The set it reports is the header checkbox's
/// select-all target: every selectable, non-disabled key, in row order. The
/// Table page's own prose never mentions the shortcut, which is exactly the
/// derived-claim case the sortable-header and tree-row keys are.
#[gpui::test]
fn table_ctrl_a_selects_every_enabled_row(cx: &mut TestAppContext) {
    let recorded = events();
    let held: Rc<RefCell<Vec<SharedString>>> = Rc::new(RefCell::new(Vec::new()));
    let held_for_view = held;

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let held_view = held_for_view.clone();
        let mut table = Table::new(vec![])
            .id("tbl-ctrl-a")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .keyed_row("alpha", vec![tall_cell("Alpha")])
            .keyed_row("beta", vec![tall_cell("Beta")])
            .keyed_row("gamma", vec![tall_cell("Gamma")])
            .disabled_keys(["beta"])
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

    // Tab lands on the table body's stop, the first one on the page: the
    // sortable-header test proves the wrapper's own stop precedes the stops
    // of the elements inside it (the header's select-all checkbox included).
    press(cx, "tab");
    press(cx, "ctrl-cmd-a");
    assert!(
        recorded.borrow().is_empty(),
        "Mod+A must reject an extra non-platform modifier"
    );
    press_mod_a(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,gamma"],
        "Ctrl+A on a focused multiple-selection table must report every \
         selectable non-disabled row and no disabled one"
    );

    // React Stately's `selectAll` is idempotent once the whole selectable set
    // is selected. The flush feeds the controlled selection back into the
    // next render before the second shortcut.
    flush_frame(cx);
    press_mod_a(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,gamma"],
        "a second Ctrl+A must not clear or re-report an already complete selection"
    );
}

/// The Mod+A chord follows the runtime platform identity, not the
/// compile-time host: `os: Mac` makes Cmd+A the chord even for a Web
/// target, and `os: Windows` makes Ctrl+A the chord even on a Mac host --
/// while the other convention's chord stays inert in each.
#[gpui::test]
fn table_mod_a_follows_the_runtime_platform(cx: &mut TestAppContext) {
    for (os, target, select_chord, other_chord) in [
        (
            gpui::OperatingSystem::Mac,
            gpui::PlatformTarget::Web,
            "cmd-a",
            "ctrl-a",
        ),
        (
            gpui::OperatingSystem::Windows,
            gpui::PlatformTarget::Native,
            "ctrl-a",
            "cmd-a",
        ),
    ] {
        cx.set_platform_info(gpui::PlatformInfo { os, target });
        let recorded = events();
        let for_view = recorded.clone();
        let vcx = open_host(cx, move || {
            let recorded = for_view.clone();
            Table::new(vec![])
                .id("tbl-mod-a-runtime")
                .columns(vec![TableColumn::new("Name").default_width(px(160.))])
                .selection_mode(SelectionMode::Multiple)
                .keyed_row("alpha", vec![tall_cell("Alpha")])
                .keyed_row("beta", vec![tall_cell("Beta")])
                .keyed_row("gamma", vec![tall_cell("Gamma")])
                .disabled_keys(["beta"])
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

        press(vcx, "tab");
        press(vcx, other_chord);
        assert!(
            recorded.borrow().is_empty(),
            "{os:?}/{target:?}: {other_chord} must not select all"
        );
        press(vcx, select_chord);
        assert_eq!(
            recorded.borrow().as_slice(),
            ["alpha,gamma"],
            "{os:?}/{target:?}: {select_chord} must select every enabled row"
        );
    }
}

#[gpui::test]
fn table_mod_a_bubbles_from_the_header_checkbox(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("tbl-header-mod-a")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .keyed_row("alpha", vec![tall_cell("Alpha")])
            .keyed_row("beta", vec![tall_cell("Beta")])
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

    press(cx, "tab");
    press(cx, "tab");
    press_mod_a(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,beta"],
        "Mod+A from the header checkbox must bubble to the Table root"
    );
}

#[gpui::test]
fn table_mod_a_reports_an_all_disabled_collection(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("tbl-disabled-mod-a")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .keyed_row("alpha", vec![tall_cell("Alpha")])
            .keyed_row("beta", vec![tall_cell("Beta")])
            .disabled_keys(["alpha", "beta"])
            .on_selection_change(move |keys, _, _| {
                recorded.borrow_mut().push(keys.len().to_string());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press_mod_a(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["0"],
        "a non-empty all-disabled collection must report its empty selectable set"
    );
}

#[gpui::test]
fn table_column_resize_drag_changes_width(cx: &mut TestAppContext) {
    let recorded = events();

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Three resizable columns at 160px inside a 480px wrapper. The probe
        // cells are clickable ROW cells: column 0 rows record "cell-a",
        // column 1 rows "cell-b", so a click names the column that owns that
        // x by behaviour. (The first version probed the sortable headers, and
        // the drag's press also hovered the header's click target, so the
        // drop synthesised a stray column click.)
        // Three resizable columns at `default_width(160)`. Unlike the sort
        // table's headers, resizable columns do NOT flex to fill slack, so
        // the table stays three 160px columns with no wrapper: the probes and
        // the resize handle land exactly. (A width wrapper was tried and it
        // shifts the post-drag geometry ~8px off the arithmetic.)
        Table::new(vec![])
            .id("tbl-resize")
            .columns(vec![
                TableColumn::new("Name")
                    .allows_resizing(true)
                    .default_width(px(160.)),
                TableColumn::new("Size")
                    .allows_resizing(true)
                    .default_width(px(160.)),
                TableColumn::new("Weight")
                    .allows_resizing(true)
                    .default_width(px(160.)),
            ])
            .row(vec![
                probe_cell("resize-probe-a0", "cell-a", recorded.clone()),
                probe_cell("resize-probe-b0", "cell-b", recorded),
                gpui::div().child("1").into_any_element(),
            ])
            .into_any_element()
    });

    // Probe before: column 1 spans x 160..320 and its cell content (the cell
    // is `px-4`, 16px each side) starts at x 176; x = 180 is inside it. The
    // single 105px row (80px filler + padding) puts y = 90 inside row 0 for
    // any header height in 30..46px.
    click(cx, 180., 90.);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-b"],
        "before the drag, x=180 must be column two's cell"
    );

    // Drag column 0's trailing-edge handle from x 160 to x 200. The handle is
    // `absolute right(-8) w(17)`, so it spans x 151..168; the table's move
    // handler adds the 40px delta to the column's 160px width. The probe after
    // is behavioural: the same click that landed in column two must now land
    // in column one (0..200), whose cell content ends at x 184. The lifecycle
    // callback path is covered separately below; this proves committed geometry.
    drag(cx, (160., 18.), (200., 18.));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-b"],
        "the drag itself must not fire any cell probe"
    );

    click(cx, 180., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-b", "cell-a"],
        "after dragging the boundary 40px right, a click at x=180 that used \
         to land in column two must land in column one"
    );
}

#[gpui::test]
fn table_column_resize_reports_pointer_lifecycle_outside_bounds(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let on_start = for_view.clone();
        let on_resize = for_view.clone();
        let on_end = for_view.clone();
        Table::new(vec![])
            .id("tbl-resize-lifecycle")
            .columns(vec![
                TableColumn::new("Name")
                    .allows_resizing(true)
                    .default_width(px(160.)),
                TableColumn::new("Size")
                    .allows_resizing(true)
                    .default_width(px(160.)),
            ])
            .row(vec![tall_cell("A"), tall_cell("B")])
            .on_resize_start(move |widths, _, _| {
                on_start.borrow_mut().push(resize_event("start", widths));
            })
            .on_resize(move |widths, _, _| {
                on_resize.borrow_mut().push(resize_event("resize", widths));
            })
            .on_resize_end(move |widths, _, _| {
                on_end.borrow_mut().push(resize_event("end", widths));
            })
            .into_any_element()
    });

    // y=500 is below the table, so only the paint-time window listeners can
    // observe the move and release and pair the lifecycle callbacks.
    drag(cx, (160., 18.), (200., 500.));
    assert_eq!(
        recorded.borrow().as_slice(),
        [
            "start:Name=160,Size=160",
            "resize:Name=200,Size=160",
            "end:Name=200,Size=160",
        ]
    );
}

#[gpui::test]
fn table_column_controlled_width_reports_without_mutating_layout(cx: &mut TestAppContext) {
    let probes = events();
    let resize_events = events();
    let probes_for_view = probes.clone();
    let resize_for_view = resize_events.clone();
    let cx = open_host(cx, move || {
        let probes = probes_for_view.clone();
        let on_start = resize_for_view.clone();
        let on_resize = resize_for_view.clone();
        let on_end = resize_for_view.clone();
        Table::new(vec![])
            .id("tbl-resize-controlled")
            .columns(vec![
                TableColumn::new("Name")
                    .allows_resizing(true)
                    .width(px(160.)),
                TableColumn::new("Size")
                    .allows_resizing(true)
                    .width(px(160.)),
            ])
            .row(vec![
                probe_cell("resize-controlled-a0", "cell-a", probes.clone()),
                probe_cell("resize-controlled-b0", "cell-b", probes),
            ])
            .on_resize_start(move |widths, _, _| {
                on_start.borrow_mut().push(resize_event("start", widths));
            })
            .on_resize(move |widths, _, _| {
                on_resize.borrow_mut().push(resize_event("resize", widths));
            })
            .on_resize_end(move |widths, _, _| {
                on_end.borrow_mut().push(resize_event("end", widths));
            })
            .into_any_element()
    });

    click(cx, 190., 90.);
    assert_eq!(probes.borrow().as_slice(), ["cell-b"]);
    press(cx, "tab enter right enter");
    flush_frame(cx);
    assert_eq!(
        resize_events.borrow().as_slice(),
        [
            "start:Name=160,Size=160",
            "resize:Name=170,Size=160",
            "end:Name=170,Size=160",
        ]
    );
    resize_events.borrow_mut().clear();

    // A second session with no arrow must report the caller-owned 160px, not
    // the prior session's rejected 170px proposal.
    press(cx, "enter enter");
    assert_eq!(
        resize_events.borrow().as_slice(),
        ["start:Name=160,Size=160", "end:Name=160,Size=160"]
    );

    click(cx, 190., 90.);
    assert_eq!(
        probes.borrow().as_slice(),
        ["cell-b", "cell-b"],
        "without controlled feedback, the requested width must not change layout"
    );
}

#[gpui::test]
fn table_pointer_resize_ends_active_keyboard_session(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let on_start = for_view.clone();
        let on_resize = for_view.clone();
        let on_end = for_view.clone();
        Table::new(vec![])
            .id("tbl-resize-modality")
            .columns(vec![
                TableColumn::new("Name")
                    .allows_resizing(true)
                    .default_width(px(160.)),
                TableColumn::new("Size")
                    .allows_resizing(true)
                    .default_width(px(160.)),
            ])
            .row(vec![tall_cell("A"), tall_cell("B")])
            .on_resize_start(move |widths, _, _| {
                on_start.borrow_mut().push(resize_event("start", widths));
            })
            .on_resize(move |widths, _, _| {
                on_resize.borrow_mut().push(resize_event("resize", widths));
            })
            .on_resize_end(move |widths, _, _| {
                on_end.borrow_mut().push(resize_event("end", widths));
            })
            .into_any_element()
    });

    click(cx, 200., 90.);
    press(cx, "tab enter");
    drag(cx, (160., 18.), (200., 18.));
    press(cx, "enter enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        [
            "start:Name=160,Size=160",
            "end:Name=160,Size=160",
            "start:Name=160,Size=160",
            "resize:Name=200,Size=160",
            "end:Name=200,Size=160",
            "start:Name=200,Size=160",
            "end:Name=200,Size=160",
        ],
        "pointer resizing must finish keyboard edit mode before starting its own session"
    );
}

/// React Stately starts a resize from the column's current laid-out width, not
/// from a constant fallback. In a 600px primary table, the 4px tray padding
/// leaves 592px and two un-sized columns measure 296px each.
#[gpui::test]
fn table_column_resize_starts_from_the_measured_flex_width(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Table::new(vec![])
                    .id("tbl-resize-measured")
                    .columns(vec![
                        TableColumn::new("Name").allows_resizing(true),
                        TableColumn::new("Size").allows_resizing(true),
                    ])
                    .row(vec![
                        probe_cell("resize-measured-a0", "cell-a", recorded.clone()),
                        probe_cell("resize-measured-b0", "cell-b", recorded),
                    ]),
            )
            .into_any_element()
    });

    click(cx, 320., 90.);
    flush_frame(cx);
    assert_eq!(recorded.borrow().as_slice(), ["cell-b"]);

    // The first boundary is at screen x=300: 4px tray padding plus the 296px
    // column. Moving it 40px right must leave x=320 in column 0. A 160px or
    // 240px constant anchor leaves the same point in column 1.
    drag(cx, (300., 18.), (340., 18.));
    click(cx, 320., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-b", "cell-a"],
        "resizing an un-sized flex column must start from its measured width"
    );
}

#[gpui::test]
fn table_column_keyboard_resize_starts_from_the_measured_flex_width(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Table::new(vec![])
                    .id("tbl-resize-measured-keys")
                    .columns(vec![
                        TableColumn::new("Name").allows_resizing(true),
                        TableColumn::new("Size").allows_resizing(true),
                    ])
                    .row(vec![
                        probe_cell("resize-measured-keys-a0", "cell-a", recorded.clone()),
                        probe_cell("resize-measured-keys-b0", "cell-b", recorded),
                    ]),
            )
            .into_any_element()
    });

    // The column-1 probe focuses the table body. Tab reaches resizer 0;
    // Enter+Right grows the measured 296px width to 306px, which moves x=290
    // from the padding gap into column 0's probe content.
    click(cx, 320., 90.);
    assert_eq!(recorded.borrow().as_slice(), ["cell-b"]);
    press(cx, "tab enter right");
    flush_frame(cx);
    click(cx, 290., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-b", "cell-a"],
        "keyboard resizing an un-sized column must start from its measured width"
    );
}

/// Pinned React Aria exposes a column resizer as a keyboard-editable range:
/// Enter starts editing and each arrow step moves the boundary by 10px.
#[gpui::test]
fn table_column_resizer_answers_keyboard_editing(cx: &mut TestAppContext) {
    let recorded = events();
    let bubbled = events();
    let bubbled_after = bubbled.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let bubbled = bubbled.clone();
        gpui::div()
            .on_key_down(move |event, _, _| {
                bubbled.borrow_mut().push(event.keystroke.key.clone());
            })
            .child(
                Table::new(vec![])
                    .id("tbl-resize-keys")
                    .columns(vec![
                        TableColumn::new("Name")
                            .allows_resizing(true)
                            .default_width(px(160.)),
                        TableColumn::new("Size")
                            .allows_resizing(true)
                            .default_width(px(160.)),
                    ])
                    .row(vec![
                        probe_cell("resize-keys-a0", "cell-a", recorded.clone()),
                        probe_cell("resize-keys-b0", "cell-b", recorded),
                    ]),
            )
            .into_any_element()
    });

    // Column 0's padded content ends before x=190 at its 160px default, so
    // this point belongs to column 1. Five pinned 10px steps move the first
    // boundary to 210px and put x=190 inside column 0's content.
    click(cx, 190., 90.);
    assert_eq!(recorded.borrow().as_slice(), ["cell-b"]);

    // The probe click focuses the table body stop, so Tab reaches the first
    // resizer; Enter then Right commits one step.
    press(cx, "tab");
    bubbled_after.borrow_mut().clear();
    // The first Tab ends edit mode without moving focus. Enter therefore
    // starts the same resizer again, and the final Right is its fifth step.
    press(cx, "enter right right right right tab enter right");
    assert!(
        bubbled_after.borrow().is_empty(),
        "resizer edit keys must be consumed by the focused handle; bubbled: {:?}",
        bubbled_after.borrow().as_slice()
    );
    flush_frame(cx);
    click(cx, 190., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-b", "cell-a"],
        "five Right presses across a Tab-ended edit must widen the column by 50px"
    );
}

/// A conditionally rendered resizer gets a new GPUI element and focus handle
/// when it returns. Its outside-press exit must belong to that live element.
#[gpui::test]
fn table_recreated_column_resizer_ends_editing_on_outside_press(cx: &mut TestAppContext) {
    let first_resizable = Rc::new(Cell::new(true));
    let resizable_for_view = first_resizable.clone();
    let bubbled = events();
    let bubbled_after = bubbled.clone();
    let cx = open_host(cx, move || {
        let bubbled = bubbled.clone();
        gpui::div()
            .on_key_down(move |event, _, _| {
                bubbled.borrow_mut().push(event.keystroke.key.clone());
            })
            .child(
                Table::new(vec![])
                    .id("tbl-resize-recreated")
                    .columns(vec![
                        TableColumn::new("Name")
                            .allows_resizing(resizable_for_view.get())
                            .default_width(px(160.)),
                        TableColumn::new("Size")
                            .allows_resizing(true)
                            .default_width(px(160.)),
                    ])
                    .row(vec![
                        gpui::div().child("A").into_any_element(),
                        gpui::div().child("B").into_any_element(),
                    ]),
            )
            .into_any_element()
    });

    first_resizable.set(false);
    flush_frame(cx);
    flush_frame(cx);
    first_resizable.set(true);
    flush_frame(cx);

    // Clicking the recreated column-0 boundary focuses it, and Enter starts
    // edit mode. Clicking column 1's boundary must blur/end column 0; clicking
    // column 0 again only refocuses it and does not start a new keyboard edit.
    click(cx, 160., 18.);
    press(cx, "enter");
    click(cx, 320., 18.);
    flush_frame(cx);
    click(cx, 160., 18.);
    bubbled_after.borrow_mut().clear();
    press(cx, "right");
    assert_eq!(
        bubbled_after.borrow().as_slice(),
        ["right"],
        "a recreated resizer must not retain edit mode after an outside press"
    );
}

#[gpui::test]
fn table_column_resize_drag_clamps_at_the_default_floor(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("tbl-resize-floor")
            .columns(vec![
                TableColumn::new("Name")
                    .allows_resizing(true)
                    .default_width(px(160.)),
                TableColumn::new("Size")
                    .allows_resizing(true)
                    .default_width(px(160.)),
            ])
            .row(vec![
                probe_cell("resize-floor-a0", "cell-a", recorded.clone()),
                probe_cell("resize-floor-b0", "cell-b", recorded),
            ])
            .into_any_element()
    });

    click(cx, 52., 90.);
    flush_frame(cx);
    assert_eq!(recorded.borrow().as_slice(), ["cell-a"]);

    // Pinned react-stately 3.49.0 defaults a resizable column's minimum to
    // 75px. Dragging from 160px to a raw 30px must therefore leave x=52 in
    // the first column; the old hardcoded 48px floor leaves that point bare.
    drag(cx, (160., 18.), (30., 18.));
    click(cx, 52., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-a", "cell-a"],
        "a resize past the default floor must commit the pinned 75px minimum"
    );
}

#[gpui::test]
fn table_column_resize_drag_clamps_to_explicit_min_and_max(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("tbl-resize-bounds")
            .columns(vec![
                TableColumn::new("Name")
                    .allows_resizing(true)
                    .default_width(px(160.))
                    .min_width(px(120.))
                    .max_width(px(180.)),
                TableColumn::new("Size")
                    .allows_resizing(true)
                    .default_width(px(160.)),
            ])
            .row(vec![
                probe_cell("resize-bounds-a0", "cell-a", recorded.clone()),
                probe_cell("resize-bounds-b0", "cell-b", recorded),
            ])
            .into_any_element()
    });

    drag(cx, (160., 18.), (30., 18.));
    click(cx, 100., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-a"],
        "the explicit 120px minimum must own x=100 after a far-left drag"
    );

    drag(cx, (120., 18.), (300., 18.));
    click(cx, 150., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-a", "cell-a"],
        "the explicit maximum must leave x=150 in the widened first column"
    );
    click(cx, 200., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-a", "cell-a", "cell-b"],
        "the explicit 180px maximum must leave x=200 in the second column"
    );
}

#[gpui::test]
fn table_column_resize_starts_from_the_constrained_default_width(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("tbl-resize-constrained-default")
            .columns(vec![
                TableColumn::new("Name")
                    .allows_resizing(true)
                    .default_width(px(40.))
                    .min_width(px(120.)),
                TableColumn::new("Size")
                    .allows_resizing(true)
                    .default_width(px(160.)),
            ])
            .row(vec![
                probe_cell("resize-default-a0", "cell-a", recorded.clone()),
                probe_cell("resize-default-b0", "cell-b", recorded),
            ])
            .into_any_element()
    });

    // The explicit minimum renders the first boundary at x=120. A 50px drag
    // must start from that constrained width and end at 170px, so x=150 is
    // still in column one. Starting from the raw 40px default would clamp the
    // result back to 120px and leave x=150 in column two.
    drag(cx, (120., 18.), (170., 18.));
    click(cx, 150., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-a"],
        "resizing must start from the rendered constrained default width"
    );
}

#[gpui::test]
fn table_column_resize_default_min_wins_over_a_smaller_max(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("tbl-resize-min-wins")
            .columns(vec![
                TableColumn::new("Name")
                    .allows_resizing(true)
                    .default_width(px(160.))
                    .max_width(px(40.)),
                TableColumn::new("Size")
                    .allows_resizing(true)
                    .default_width(px(160.)),
            ])
            .row(vec![
                probe_cell("resize-min-wins-a0", "cell-a", recorded.clone()),
                probe_cell("resize-min-wins-b0", "cell-b", recorded),
            ])
            .into_any_element()
    });

    click(cx, 52., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["cell-a"],
        "the 75px default minimum must beat a smaller maximum in the body"
    );
}

#[gpui::test]
fn table_load_more_fires_when_the_sentinel_enters_view(cx: &mut TestAppContext) {
    let recorded = events();

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // The four 105px rows put the sentinel below this 160px scroller on
        // the first frame. v3's Table.LoadMore is an intersection sentinel:
        // it reports when scrolling brings that row into the content mask.
        let table = Table::new(vec![])
            .id("tbl-more")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .row(vec![tall_cell("A")])
            .row(vec![tall_cell("B")])
            .row(vec![tall_cell("C")])
            .row(vec![tall_cell("D")])
            .scroll_offset(0.)
            .on_load_more(move |_, _| recorded.borrow_mut().push("load-more".to_owned()));
        gpui::div()
            .id("tbl-more-scroll")
            .w(px(200.))
            .h(px(160.))
            .overflow_y_scroll()
            .child(table)
            .into_any_element()
    });
    flush_frame(cx);
    assert!(
        recorded.borrow().is_empty(),
        "an offscreen sentinel must not ask for more rows"
    );

    wheel_v(cx, 100., 80., -400.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more"],
        "entering the scroll viewport must report exactly once"
    );

    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more"],
        "remaining visible across redraws must not repeat the request"
    );

    wheel_v(cx, 100., 80., 400.);
    wheel_v(cx, 100., 80., -400.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more", "load-more"],
        "leaving and re-entering the viewport must make a new request"
    );
}

/// React Aria's inherited `scrollOffset` defaults to one viewport, so a
/// sentinel may prefetch before it is physically visible. Two 105px rows put
/// this sentinel below a 160px mask but less than another 160px beyond it.
#[gpui::test]
fn table_load_more_defaults_to_one_viewport_of_prefetch(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let table = Table::new(vec![])
            .id("tbl-more-default-offset")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .row(vec![tall_cell("A")])
            .row(vec![tall_cell("B")])
            .on_load_more(move |_, _| recorded.borrow_mut().push("load-more".to_owned()));
        gpui::div()
            .id("tbl-more-default-scroll")
            .w(px(200.))
            .h(px(160.))
            .overflow_y_scroll()
            .child(table)
            .into_any_element()
    });
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more"],
        "the default one-viewport offset must prefetch before intersection"
    );

    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more"],
        "prefetch must remain one request while the sentinel stays in range"
    );
}

/// React Aria tears down and re-observes the sentinel when the collection
/// changes so a short result page can immediately request another page. The
/// sentinel never leaves this tall viewport, so the row count is the only
/// event that may re-arm it.
#[gpui::test]
fn table_load_more_rearms_when_the_row_collection_changes(cx: &mut TestAppContext) {
    let recorded = events();
    let row_count = Rc::new(RefCell::new(1usize));
    let for_view = recorded.clone();
    let count_for_view = row_count.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let mut table = Table::new(vec![])
            .id("tbl-more-collection")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .scroll_offset(0.)
            .on_load_more(move |_, _| recorded.borrow_mut().push("load-more".to_owned()));
        for index in 0..*count_for_view.borrow() {
            table = table.row(vec![tall_cell(format!("row {index}"))]);
        }
        table.into_any_element()
    });
    flush_frame(cx);
    assert_eq!(recorded.borrow().as_slice(), ["load-more"]);

    *row_count.borrow_mut() = 2;
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more", "load-more"],
        "a changed collection must re-observe a still-visible sentinel"
    );

    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more", "load-more"],
        "an unchanged collection must remain silent on later frames"
    );
}

/// Resizes the window and drains the layout queue, so a bounded virtual body
/// re-resolves its viewport before the next probe reads it.
fn settle_window(cx: &mut VisualTestContext, width: f32, height: f32) {
    cx.simulate_resize(gpui::size(px(width), px(height)));
    for _ in 0..4 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

/// `debug_bounds` wants a `&'static str`; the virtual body's probe name is
/// the table's own `{id}-virtual-rows` selector, so one short leak per call.
fn virtual_rows_probe(table_id: &str) -> &'static str {
    let name = format!("{table_id}-virtual-rows");
    let leaked: &'static mut str = Box::leak(name.into_boxed_str());
    &*leaked
}

/// Pinned `TableKeyboardDelegate` pages a virtual body by one *visible*
/// rectangle. A window shorter than this body's 160px cap shows fewer 40px
/// rows, so PageDown must step by the shown viewport -- one row here -- and
/// not by the cap's three-row ruler; the old ruler lands rows too far at
/// every step and skips past the disabled row differently. Growing the window
/// restores the cap step, and a near-zero viewport keeps the cursor in place.
#[gpui::test]
fn fixed_height_virtual_table_pages_by_the_shown_viewport(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                Table::new(vec![])
                    .id("vt-bounded-page")
                    .columns(vec![TableColumn::new("Name").default_width(px(160.))])
                    .virtual_rows(
                        20,
                        "virtual-bounded-page-users",
                        |i| format!("key-{i:02}").into(),
                        |i| {
                            TableRow::new(vec![gpui::div()
                                .child(format!("Row {i}"))
                                .into_any_element()])
                        },
                    )
                    .row_height(px(40.))
                    .max_h(px(160.))
                    .disabled_keys(["key-05"])
                    .on_row_click(move |i, _, _, _| recorded.borrow_mut().push(format!("row-{i}")))
                    .into_any_element(),
            )
            .into_any_element()
    });

    // A 100px window shows fewer than the 160px cap allows, so one page is
    // shorter than the roomy-window three-row step.
    settle_window(cx, 640., 100.);
    let body = cx
        .debug_bounds(virtual_rows_probe("vt-bounded-page"))
        .expect("the virtual body must be laid out");
    let shown = f32::from(body.size.height);
    assert!(
        shown < 160. - 1.5,
        "the bounded virtual body must shrink below its cap, got {body:?}"
    );
    let step = ((shown / 40.).ceil() as usize).saturating_sub(1);
    let capped_step = ((160f32 / 40.).ceil() as usize).saturating_sub(1);
    assert!(
        step < capped_step,
        "a shorter viewport must page shorter than the cap ruler: {step} vs {capped_step}"
    );

    // Three Downs seat row 2. Each page then moves `step` rows: 2 -> 3,
    // 3 -> 4, and 4 -> 6 with the boundary landing on disabled row 5, which
    // the stops skip; PageUp lands on it from above and skips back to 4. The
    // cap ruler would walk 2 -> 6 -> 9 -> 12 -> 9.
    press(cx, "tab");
    for _ in 0..3 {
        press(cx, "down");
    }
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-3"],
        "a bounded PageDown must step by the shown viewport, not the cap"
    );
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-3", "row-4"],
        "a bounded PageDown must advance by the shown viewport"
    );
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-3", "row-4", "row-6"],
        "a bounded PageDown must still skip the disabled row"
    );
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-3", "row-4", "row-6", "row-4"],
        "a bounded PageUp must skip back over the disabled row"
    );

    // Growing the window restores the cap viewport, so the same key pages
    // further: row 4 reaches row 7 with the three-row step.
    settle_window(cx, 640., 800.);
    let grown = cx
        .debug_bounds(virtual_rows_probe("vt-bounded-page"))
        .expect("the virtual body must be laid out");
    assert!(
        (f32::from(grown.size.height) - 160.).abs() < 1.5,
        "the grown virtual body must return to its cap, got {grown:?}"
    );
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-3", "row-4", "row-6", "row-4", "row-7"],
        "PageDown after resize must page by the grown viewport"
    );

    // A near-zero viewport has no page to turn: the cursor stays on row 7,
    // so Enter reports it again instead of a neighbour.
    settle_window(cx, 640., 32.);
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row-3", "row-4", "row-6", "row-4", "row-7", "row-7"],
        "a page key with no viewport to cross must not move the cursor"
    );
}

/// Gallery Virtualization regression: the first table's 1000x40px body caps
/// at `max_h(320)` inside a natural/unbounded page parent. The virtual list's
/// own bounds stay at the cap while the outer table ends at
/// header + 320 + tray padding, so the marker -- and the second table below
/// it -- directly follow instead of sliding past a white body that extends
/// to the rows' full natural height (40_000px).
#[gpui::test]
fn fixed_height_virtual_table_caps_in_an_unbounded_parent(cx: &mut TestAppContext) {
    fn virtual_table(id: &str, identity: &'static str) -> gpui::AnyElement {
        Table::new(vec![])
            .id(id)
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .virtual_rows(
                1000,
                identity,
                |i| format!("key-{i:04}").into(),
                |i| {
                    TableRow::new(vec![gpui::div()
                        .child(format!("Row {i}"))
                        .into_any_element()])
                },
            )
            .row_height(px(40.))
            .max_h(px(320.))
            .into_any_element()
    }

    let cx = open_host(cx, move || {
        gpui::div()
            .size_full()
            .child(
                gpui::div()
                    .id("unbounded-page-scroll")
                    .overflow_y_scroll()
                    .size_full()
                    .child(
                        gpui::div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .child(virtual_table("vt-unbounded", "virtual-unbounded-users"))
                            .child(
                                gpui::div()
                                    .h(px(8.))
                                    .w_full()
                                    .debug_selector(|| "after-first-table".to_owned()),
                            )
                            .child(virtual_table("vt-second", "virtual-second-users")),
                    ),
            )
            .into_any_element()
    });
    cx.run_until_parked();

    let body = cx
        .debug_bounds(virtual_rows_probe("vt-unbounded"))
        .expect("the first virtual body must be laid out");
    assert!(
        (f32::from(body.size.height) - 320.).abs() < 1.5,
        "the unbounded virtual body must cap at 320, got {body:?}"
    );

    let after = cx
        .debug_bounds("after-first-table")
        .expect("the marker below the first table must be laid out");
    let after_top = f32::from(after.origin.y);
    let body_bottom = f32::from(body.origin.y) + f32::from(body.size.height);
    assert!(
        after_top < 800.,
        "the first table must end near header+320+padding, marker at y={after_top} body={body:?}"
    );
    assert!(
        (after_top - body_bottom) < 120.,
        "the marker must directly follow the first body, marker y={after_top} body bottom={body_bottom}"
    );

    let second = cx
        .debug_bounds(virtual_rows_probe("vt-second"))
        .expect("the second virtual body must be laid out");
    let second_top = f32::from(second.origin.y);
    assert!(
        second_top < 1200.,
        "the second table must directly follow the first, second at y={second_top} marker at y={after_top}"
    );
    assert!(
        (f32::from(second.size.height) - 320.).abs() < 1.5,
        "the second virtual body must cap at 320, got {second:?}"
    );
}

// ---------------------------------------------------------------------------
// Slider
// ---------------------------------------------------------------------------

#[gpui::test]
fn slider_keyboard_steps_and_clamps(cx: &mut TestAppContext) {
    let recorded = events();

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // `default_value` hands the slider its own state, so the keyboard
        // advances it; min 0 / max 10 / step 1 keep every reported value an
        // exact integer (formatted as a string, so clippy::float_cmp has
        // nothing to compare). The 600px wrapper fixes the track width -- a
        // horizontal slider is `w_full` and would otherwise stretch to the
        // host window -- which is what the press below maps through: x 300 is
        // half the track, i.e. value 5.
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("sk-slider", 5.)
                    .default_value(5.)
                    .min_value(0.)
                    .max_value(10.)
                    .step(1.)
                    .on_change(move |v, _, _| recorded.borrow_mut().push(format!("{v}")))
                    .into_any_element(),
            )
            .into_any_element()
    });

    // Pressing the track both focuses the handle (its `on_mouse_down` calls
    // `window.focus`) and lands on 5. Since 5 is already effective, the
    // unchanged press is silent.
    click(cx, 300., 10.);
    assert_eq!(
        recorded.borrow().as_slice(),
        &[] as &[String],
        "pressing the track at the current value must suppress onChange"
    );

    // Right/Left step by `step`; Home/End jump to the bounds; the value never
    // leaves [min, max] -- End then Right must still report 10.
    press(cx, "right");
    press(cx, "right");
    press(cx, "left");
    press(cx, "home");
    press(cx, "end");
    press(cx, "right");
    press(cx, "left");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["6", "7", "6", "0", "10", "9"],
        "arrows must step by `step`, Home/End must land exactly on the \
         bounds, and a step past a bound must clamp to it"
    );
}

#[gpui::test]
fn slider_drag_moves_the_thumb(cx: &mut TestAppContext) {
    let recorded = events();

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // The 600px wrapper fixes the track width (a horizontal slider is
        // `w_full`), so a pointer x maps to `x / 600 * max`.
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("dr-slider", 50.)
                    .default_value(50.)
                    .min_value(0.)
                    .max_value(100.)
                    .step(1.)
                    .on_change(move |v, _, _| recorded.borrow_mut().push(format!("{v}")))
                    .into_any_element(),
            )
            .into_any_element()
    });

    // The track is 600px wide, so a pointer x maps to `x / 600 * 100`. Down
    // at x=60 is 10; one move (the drag) to x=150 is 0.25 * 100 = 25. The
    // drop position's value is the one the drag must land on.
    //
    // Defect reproduction (fixed): the drag's flag and the track bounds live
    // in the window's keyed state, so the press's own repaint rebuilds the
    // listeners without losing the drag -- `on_mouse_down` marks the keyed
    // flag, the move sees it and maps x=150 through the recorded bounds, and
    // the drop lands on 25. A per-render `Rc<Cell>` read as a fresh `false`
    // on every rebuilt listener and no move ever arrived.
    drag(cx, (60., 10.), (150., 10.));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["10", "25"],
        "the down reports 60/600*100=10 and the drop at 150/600*100=25"
    );
}

#[gpui::test]
fn two_sliders_do_not_share_a_drag(cx: &mut TestAppContext) {
    let recorded = events();

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Two handles into the recorder, one per slider's closure; `recorded`
        // itself can move into the second, since every handle is the same Rc.
        let a_rec = recorded.clone();
        let b_rec = recorded;
        // Two sliders with different track widths, so a drag state that was
        // keyed by anything but the component's own id would mis-map the
        // moves: the top track is 600px (value = x / 6), the bottom one 300px
        // (value = x / 3), and the lower canvas would overwrite the bounds
        // the upper drag reads. The records are prefixed so a report can
        // always be blamed on one slider.
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                gpui::div().w(px(600.)).child(
                    Slider::new("iso-a", 50.)
                        .default_value(50.)
                        .min_value(0.)
                        .max_value(100.)
                        .step(1.)
                        .on_change(move |v, _, _| a_rec.borrow_mut().push(format!("a:{v}"))),
                ),
            )
            .child(
                gpui::div().w(px(300.)).child(
                    Slider::new("iso-b", 50.)
                        .default_value(50.)
                        .min_value(0.)
                        .max_value(100.)
                        .step(1.)
                        .on_change(move |v, _, _| b_rec.borrow_mut().push(format!("b:{v}"))),
                ),
            )
            .into_any_element()
    });

    // The top slider's track spans y 0..20 (centre 10): down at x=60 is
    // 60/600*100=10, the drop at x=150 is 25. The bottom track sits 20px
    // below (y 40..60, centre 50) and is 300px wide: the same x maps to
    // 60/300*100=20 and 150/300*100=50.
    drag(cx, (60., 10.), (150., 10.));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a:10", "a:25"],
        "dragging the top slider must report only the top slider's values"
    );

    drag(cx, (60., 50.), (150., 50.));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a:10", "a:25", "b:20", "b:50"],
        "dragging the bottom slider must move the bottom slider alone and \
         map its x through its own 300px track, not the top slider's 600px"
    );
}

#[gpui::test]
fn slider_range_two_thumbs_do_not_cross(cx: &mut TestAppContext) {
    let recorded = events();

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Range support exists (`values` + `on_change_all`). The component is
        // fully controlled: each thumb keeps its identity and clamps against
        // its neighbours rather than crossing and becoming another thumb.
        // The 600px wrapper fixes the track width, so a pointer x maps to
        // `x / 600 * 100`: x 90 is value 15, x 120 is 20, x 540 is 90.
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("rg-slider", 0.)
                    .values([20., 80.])
                    .min_value(0.)
                    .max_value(100.)
                    .step(1.)
                    .on_change_all(move |vs, _, _| {
                        let joined = vs
                            .iter()
                            .map(|v| format!("{v}"))
                            .collect::<Vec<_>>()
                            .join(",");
                        recorded.borrow_mut().push(joined);
                    })
                    .into_any_element(),
            )
            .into_any_element()
    });

    // A press at x=90 (value 15) moves whichever thumb is nearest: the lower
    // one (20), so the set stays [15, 80].
    click(cx, 90., 10.);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["15,80"],
        "a press near the lower thumb must move the lower thumb"
    );

    // A press on the lower thumb's own position (x=120 maps to its value 20)
    // is silent because the effective pair is unchanged; a press far past the
    // upper thumb (x=540 maps to 90) moves the nearest thumb -- the upper --
    // to 90, giving [20, 90]. This is the same "nearest follows the pointer,
    // the set never inverts" arithmetic a drag would run. The range form
    // drives it with presses here; the pointer drag itself is exercised, in
    // the single-thumb form, by `slider_drag_moves_the_thumb` and
    // `two_sliders_do_not_share_a_drag`.
    click(cx, 120., 10.);
    flush_frame(cx);
    click(cx, 540., 10.);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["15,80", "20,90"],
        "a pointer past the upper thumb must move the upper thumb, keeping \
         the set ascending"
    );

    // Re-activate the lower thumb, then press End. It clamps at the upper
    // thumb; it does not cross to 100 and become the upper thumb.
    click(cx, 120., 10.);
    flush_frame(cx);
    press(cx, "end");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["15,80", "20,90", "80,80"],
        "pushing the lower thumb toward the maximum must clamp at its neighbour"
    );

    // Every report, parsed back, is an ascending pair.
    for s in recorded.borrow().iter() {
        let nums: Vec<f32> = s.split(',').map(|p| p.parse::<f32>().unwrap()).collect();
        assert!(
            nums[0] <= nums[1],
            "every multi-thumb report must be ordered, got {s}"
        );
    }
}

// ---------------------------------------------------------------------------
// Colour pickers
// ---------------------------------------------------------------------------

#[gpui::test]
fn color_slider_keyboard_changes_channel(cx: &mut TestAppContext) {
    let recorded = events();

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Hue is a 0..360 channel, so the keyboard step is 1 (`max - min > 2`),
        // and hue round-trips exactly through PickerColor (stored verbatim),
        // so the reported values are exact integers. The colour is compared as
        // a formatted string -- no float equality anywhere.
        ColorSlider::new("cs-hue", PickerColor::hsb(180., 1., 1.), ColorChannel::Hue)
            .default_value(PickerColor::hsb(180., 1., 1.))
            .on_change(move |c, _, _| recorded.borrow_mut().push(format!("{}", c.hue.round())))
            .into_any_element()
    });

    // Tab is the only tab stop on the page, so it focuses the slider's handle
    // (`.tab_stop(true)`); no click lands on the track, so no press value
    // pollutes the sequence.
    press(cx, "tab");
    press(cx, "right");
    press(cx, "right");
    press(cx, "left");
    press(cx, "home");
    press(cx, "end");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["181", "182", "181", "0", "360"],
        "Right/Left must step the hue by one, Home must land on 0, and the \
         pinned hue range's End must preserve its maximum value 360"
    );
}

#[gpui::test]
fn color_area_keyboard_moves_both_axes(cx: &mut TestAppContext) {
    let recorded = events();

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Default axes are Saturation (x, left-right) and Brightness (y,
        // bottom-top). The start colour has both at 0.50, so each recorded
        // string is "sat,brightness" in 2dp.
        ColorArea::new("area-keys", PickerColor::hsb(210., 0.5, 0.5))
            .default_value(PickerColor::hsb(210., 0.5, 0.5))
            .on_change(move |c, _, _| {
                recorded
                    .borrow_mut()
                    .push(format!("{:.2},{:.2}", c.saturation, c.brightness));
            })
            .into_any_element()
    });

    // The area is a tab stop (`tab_stop_handle`), so Tab focuses it.
    press(cx, "tab");
    press(cx, "right");
    press(cx, "up");

    let got = recorded.borrow().clone();
    assert_eq!(
        got.len(),
        2,
        "each arrow key must report the moved colour -- with the defect, \
         nothing is recorded at all"
    );
    let sat = got[0].split(',').next().unwrap();
    assert_ne!(
        sat, "0.50",
        "Right must move the x axis (saturation) off its start value"
    );
    let brightness_before = got[0].rsplit(',').next().unwrap();
    let brightness_after = got[1].rsplit(',').next().unwrap();
    assert_ne!(
        brightness_after, brightness_before,
        "Up must move the y axis (brightness)"
    );
}

#[gpui::test]
fn disabled_color_area_answers_no_key(cx: &mut TestAppContext) {
    let recorded = events();

    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // A disabled area must leave the tab order and answer no key: the
        // `track_focus` is gated on `is_disabled`, so Tab skips it entirely,
        // and the key handler is only wired on the enabled path. Every key a
        // live area would answer is pressed, and none of them may record.
        ColorArea::new("area-dead", PickerColor::hsb(210., 0.5, 0.5))
            .default_value(PickerColor::hsb(210., 0.5, 0.5))
            .is_disabled(true)
            .on_change(move |c, _, _| {
                recorded
                    .borrow_mut()
                    .push(format!("{:.2},{:.2}", c.saturation, c.brightness));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "up");
    press(cx, "home");
    press(cx, "end");
    assert!(
        recorded.borrow().is_empty(),
        "a disabled colour area must not enter the tab order or answer any key"
    );
}

#[gpui::test]
fn color_swatch_picker_click_reports_the_swatch(cx: &mut TestAppContext) {
    let live_hexes = events();
    let live = live_hexes.clone();
    let dead_hexes = events();
    let dead = dead_hexes.clone();
    let swatches = vec![
        PickerColor::from_hex("#E52D2D").unwrap(),
        PickerColor::from_hex("#F5A524").unwrap(),
        PickerColor::from_hex("#006FEE").unwrap(),
    ];
    let expected: Vec<String> = swatches.iter().map(|s| s.to_hex()).collect();
    let living = swatches.clone();
    let dead_swatches = swatches;

    let cx = open_host(cx, move || {
        let live = live.clone();
        let dead = dead.clone();
        let living = living.clone();
        let dead_swatches = dead_swatches.clone();
        // Two pickers stacked 40px apart: the live row's cells sit at
        // y 0..32 (centres y 16), the disabled row's at y 72..104. Cells are
        // `size-8` (32px) with an 8px gap, so cell i's centre is x = 16+40i.
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(40.))
            .child(
                ColorSwatchPicker::new("swp-live", living)
                    .default_value(dead_swatches[0])
                    .on_change(move |c, _, _| live.borrow_mut().push(c.to_hex())),
            )
            .child(
                ColorSwatchPicker::new("swp-dead", dead_swatches)
                    .is_disabled(true)
                    .on_change(move |c, _, _| dead.borrow_mut().push(c.to_hex())),
            )
            .into_any_element()
    });

    click(cx, 16., 16.);
    click(cx, 56., 16.);
    click(cx, 96., 16.);
    assert_eq!(
        live_hexes.borrow().as_slice(),
        [
            expected[0].as_str(),
            expected[1].as_str(),
            expected[2].as_str()
        ],
        "clicking swatch i must report exactly that swatch's hex"
    );

    // The disabled picker reports nothing for the same three spots.
    click(cx, 16., 88.);
    click(cx, 56., 88.);
    click(cx, 96., 88.);
    assert!(
        dead_hexes.borrow().is_empty(),
        "a disabled picker must not report any swatch"
    );
}
