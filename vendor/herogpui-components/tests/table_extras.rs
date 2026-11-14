//! Data-table extras on `Table` (HeroGPUI extension, after gpui-kit's
//! table): column reordering by header drag and Alt+Left/Alt+Right,
//! controlled and uncontrolled order, and single-cell selection with a
//! press, Left/Right/Home/End and the row keys, reported by the columns'
//! given indices; and frozen leading columns (`TableColumn::frozen`) that
//! stay put while the rest of the body and the header scroll horizontally.

use crate::harness;

use std::{cell::RefCell, rc::Rc};

use gpui::{
    point, prelude::*, px, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    TestAppContext, VisualTestContext,
};
use harness::{events, open_host, press, still, Events};
use herogpui_components::{SelectionMode, SortDescriptor, Table, TableCell, TableColumn, TableRow};

const COLUMNS: [&str; 3] = ["Name", "Role", "Status"];

#[derive(Clone, Default)]
struct Config {
    reorder: bool,
    order: Option<Vec<usize>>,
    default_order: Option<Vec<usize>>,
    cells: bool,
    selected_cell: Option<Option<TableCell>>,
}

fn row(r: usize) -> TableRow {
    TableRow::new(
        (0..COLUMNS.len())
            .map(|c| {
                gpui::div()
                    .debug_selector(move || format!("c-{r}-{c}"))
                    .child(format!("r{r}c{c}"))
                    .into_any_element()
            })
            .collect(),
    )
}

fn host(cx: &mut TestAppContext, config: Config) -> (Events, &mut VisualTestContext) {
    still();
    let seen = events();
    let s = seen.clone();
    let sort = Rc::new(RefCell::new(None::<SortDescriptor>));
    let cx = open_host(cx, move || {
        let (moves, cells, sorts, sort_now) = (s.clone(), s.clone(), s.clone(), sort.clone());
        let mut table = Table::new(Vec::new())
            .id("people")
            .columns(
                COLUMNS
                    .iter()
                    .map(|label| TableColumn::new(*label).allows_sorting(true))
                    .collect(),
            )
            .on_sort_change(move |descriptor, _, _| {
                sorts
                    .borrow_mut()
                    .push(format!("sort:{}", descriptor.column));
                *sort_now.borrow_mut() = Some(descriptor.clone());
            })
            .on_column_move(move |m, _, _| {
                moves
                    .borrow_mut()
                    .push(format!("move:{}->{}:{:?}", m.from, m.to, m.order));
            })
            .on_cell_select(move |cell, _, _| {
                cells
                    .borrow_mut()
                    .push(format!("cell:{}/{}", cell.row, cell.column));
            });
        if let Some(descriptor) = sort.borrow().clone() {
            table = table.sort_descriptor(descriptor);
        }
        table = table.allows_column_reorder(config.reorder);
        if let Some(order) = config.order.clone() {
            table = table.column_order(order);
        }
        if let Some(order) = config.default_order.clone() {
            table = table.default_column_order(order);
        }
        if config.cells {
            table = table.cell_selectable(true);
        }
        if let Some(cell) = config.selected_cell.clone() {
            table = table.selected_cell(cell);
        }
        for r in 0..3 {
            table = table.tree_row(row(r));
        }
        gpui::div().w(px(720.)).child(table).into_any_element()
    });
    (seen, cx)
}

fn frame(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn cell(cx: &mut VisualTestContext, r: usize, c: usize) -> gpui::Bounds<gpui::Pixels> {
    cx.debug_bounds(Box::leak(format!("c-{r}-{c}").into_boxed_str()))
        .unwrap_or_else(|| panic!("cell {r},{c} laid out"))
}

/// The given columns, left to right, read off the first row's cells.
fn displayed(cx: &mut VisualTestContext) -> Vec<usize> {
    let mut columns: Vec<(f32, usize)> = (0..COLUMNS.len())
        .map(|c| (f32::from(cell(cx, 0, c).left()), c))
        .collect();
    columns.sort_by(|a, b| a.0.total_cmp(&b.0));
    columns.into_iter().map(|(_, c)| c).collect()
}

/// A header's centre, found above its column's first cell.
fn header(cx: &mut VisualTestContext, given: usize) -> gpui::Point<gpui::Pixels> {
    let first = cell(cx, 0, given);
    let at = displayed(cx).iter().position(|c| *c == given).unwrap();
    let track = cx
        .debug_bounds(Box::leak(
            format!("table-header-track-{at}").into_boxed_str(),
        ))
        .expect("header track");
    point(first.left() + px(4.), track.center().y)
}

fn keys(cx: &mut VisualTestContext, list: &str) {
    for key in list.split_whitespace() {
        press(cx, key);
        frame(cx);
    }
}

fn log(seen: &Events) -> Vec<String> {
    seen.borrow().clone()
}

fn mouse(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, kind: &str) {
    match kind {
        "down" => cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        }),
        "move" => cx.simulate_event(MouseMoveEvent {
            position: at,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        }),
        _ => cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: 1,
        }),
    }
    frame(cx);
}

#[gpui::test]
fn a_default_order_puts_columns_and_cells_in_display_order(cx: &mut TestAppContext) {
    let (_seen, cx) = host(
        cx,
        Config {
            default_order: Some(vec![2, 0, 1]),
            ..Config::default()
        },
    );
    frame(cx);
    assert_eq!(displayed(cx), [2, 0, 1]);
    // Every row follows the header.
    assert!(cell(cx, 2, 2).left() < cell(cx, 2, 0).left());
}

#[gpui::test]
fn alt_arrows_move_the_focused_header_and_the_focus_follows(cx: &mut TestAppContext) {
    let (seen, cx) = host(
        cx,
        Config {
            reorder: true,
            ..Config::default()
        },
    );
    frame(cx);
    // The body is the table's first tab stop; the next is the first
    // sortable header, Name.
    keys(cx, "tab tab alt-right");
    assert_eq!(log(&seen), ["move:0->1:[1, 0, 2]"]);
    assert_eq!(displayed(cx), [1, 0, 2]);
    // The focus followed Name: the next move takes it further right.
    keys(cx, "alt-right");
    assert_eq!(displayed(cx), [1, 2, 0]);
    // At the end nothing moves; Alt+Left brings it back one.
    keys(cx, "alt-right alt-left");
    assert_eq!(
        log(&seen),
        [
            "move:0->1:[1, 0, 2]",
            "move:1->2:[1, 2, 0]",
            "move:2->1:[1, 0, 2]"
        ]
    );
    // Enter still sorts the focused header — it is Name, where it moved.
    keys(cx, "enter");
    assert_eq!(log(&seen).last().unwrap(), "sort:Name");
}

#[gpui::test]
fn dragging_a_header_onto_another_column_moves_it(cx: &mut TestAppContext) {
    let (seen, cx) = host(
        cx,
        Config {
            reorder: true,
            ..Config::default()
        },
    );
    frame(cx);
    let from = header(cx, 0);
    let to = header(cx, 2);
    mouse(cx, from, "down");
    mouse(cx, point(from.x + px(20.), from.y), "move");
    mouse(cx, to, "move");
    assert!(
        cx.debug_bounds("table-column-drop-indicator").is_some(),
        "the drop position is drawn"
    );
    mouse(cx, to, "up");
    assert_eq!(log(&seen), ["move:0->2:[1, 2, 0]"]);
    assert_eq!(displayed(cx), [1, 2, 0]);
    assert!(cx.debug_bounds("table-column-drop-indicator").is_none());
    // A plain click on a header sorts it and moves nothing.
    let status = header(cx, 2);
    cx.simulate_click(status, Modifiers::none());
    frame(cx);
    assert_eq!(log(&seen), ["move:0->2:[1, 2, 0]", "sort:Status"]);
}

#[gpui::test]
fn a_table_without_reordering_ignores_the_drag_and_the_keys(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    let from = header(cx, 0);
    let to = header(cx, 2);
    mouse(cx, from, "down");
    mouse(cx, point(from.x + px(20.), from.y), "move");
    mouse(cx, to, "move");
    mouse(cx, to, "up");
    keys(cx, "tab tab alt-right");
    assert!(log(&seen).iter().all(|e| !e.starts_with("move")));
    assert_eq!(displayed(cx), [0, 1, 2]);
}

#[gpui::test]
fn a_controlled_order_reports_and_waits_for_its_prop(cx: &mut TestAppContext) {
    let (seen, cx) = host(
        cx,
        Config {
            reorder: true,
            order: Some(vec![2, 1, 0]),
            ..Config::default()
        },
    );
    frame(cx);
    assert_eq!(displayed(cx), [2, 1, 0]);
    keys(cx, "tab tab alt-right");
    assert_eq!(log(&seen), ["move:0->1:[1, 2, 0]"]);
    assert_eq!(displayed(cx), [2, 1, 0], "the prop still decides");
}

#[gpui::test]
fn a_press_selects_a_cell_and_the_arrows_move_it(cx: &mut TestAppContext) {
    let (seen, cx) = host(
        cx,
        Config {
            cells: true,
            ..Config::default()
        },
    );
    frame(cx);
    let at = cell(cx, 1, 1).center();
    cx.simulate_click(at, Modifiers::none());
    frame(cx);
    assert_eq!(log(&seen), ["cell:1/1"]);
    let ring = cx
        .debug_bounds("table-selected-cell")
        .expect("the cell is ringed");
    assert!(ring.contains(&at), "the ring is on the pressed cell");
    keys(cx, "right");
    keys(cx, "right");
    keys(cx, "down");
    keys(cx, "home");
    keys(cx, "up");
    keys(cx, "end");
    assert_eq!(
        log(&seen),
        ["cell:1/1", "cell:1/2", "cell:2/2", "cell:2/0", "cell:1/0", "cell:1/2"],
        "Right clamps at the last column; Up/Down keep the column"
    );
}

#[gpui::test]
fn cells_are_reported_by_their_given_column_after_a_reorder(cx: &mut TestAppContext) {
    let (seen, cx) = host(
        cx,
        Config {
            cells: true,
            default_order: Some(vec![2, 0, 1]),
            ..Config::default()
        },
    );
    frame(cx);
    // The leftmost displayed cell is Status (given index 2).
    let at = cell(cx, 0, 2).center();
    cx.simulate_click(at, Modifiers::none());
    frame(cx);
    keys(cx, "right");
    assert_eq!(log(&seen), ["cell:0/2", "cell:0/0"]);
}

#[gpui::test]
fn a_controlled_cell_reports_and_keeps_its_prop(cx: &mut TestAppContext) {
    let (seen, cx) = host(
        cx,
        Config {
            cells: true,
            selected_cell: Some(Some(TableCell::new("0", 0))),
            ..Config::default()
        },
    );
    frame(cx);
    let ring = cx
        .debug_bounds("table-selected-cell")
        .expect("the prop is ringed");
    assert!(ring.contains(&cell(cx, 0, 0).center()));
    // The body is the first tab stop.
    keys(cx, "tab right");
    let reported = log(&seen);
    assert!(
        reported.iter().any(|e| e == "cell:0/1"),
        "the request is reported: {reported:?}"
    );
    let ring = cx.debug_bounds("table-selected-cell").unwrap();
    assert!(
        ring.contains(&cell(cx, 0, 0).center()),
        "the prop still decides"
    );
}

#[gpui::test]
fn virtual_rows_follow_the_column_order(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, || {
        Table::new(COLUMNS.iter().map(|c| (*c).into()).collect())
            .id("virtual-people")
            .default_column_order([1, 2, 0])
            .row_height(px(40.))
            .max_h(px(200.))
            .virtual_rows(20, "people", |ix| ix.to_string().into(), row)
            .into_any_element()
    });
    frame(cx);
    assert_eq!(displayed(cx), [1, 2, 0]);
}

// ---- frozen columns (`TableColumn::frozen`) --------------------------------

/// Five 200px columns, the first frozen, in a 600px box: the frozen part is
/// the 44px selection column and the first column, and the other four scroll
/// through the 348px left of the box.
const FROZEN: [&str; 5] = ["Name", "A", "B", "C", "D"];

#[derive(Clone, Copy, Default, PartialEq)]
enum Body {
    #[default]
    Plain,
    /// `row_height` over `virtual_rows`: the uniform `VirtualList` path.
    Fixed,
    /// `estimated_row_height` over `virtual_rows`: the measured path.
    Estimated,
}

#[derive(Clone, Default)]
struct Frozen {
    body: Body,
    selectable: bool,
    cells: bool,
    reorder: bool,
    order: Option<Vec<usize>>,
    /// Which given columns are frozen; `[0]` when empty.
    frozen: Vec<usize>,
    resizable: bool,
}

/// The fill each column's content paints, so the painted scene can tell the
/// frozen cells from the scrolling ones.
fn swatch(c: usize) -> gpui::Hsla {
    gpui::hsla(c as f32 / 5., 0.8, 0.5, 1.)
}

fn frozen_row(r: usize, seen: Events) -> TableRow {
    TableRow::new(
        (0..FROZEN.len())
            .map(|c| {
                let seen = seen.clone();
                gpui::div()
                    .id(("frozen-content", r * 10 + c))
                    .debug_selector(move || format!("c-{r}-{c}"))
                    .w(px(120.))
                    .h(px(20.))
                    .bg(swatch(c))
                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                        seen.borrow_mut().push(format!("content:{r}/{c}"));
                    })
                    .into_any_element()
            })
            .collect(),
    )
}

fn frozen_host(cx: &mut TestAppContext, config: Frozen) -> (Events, &mut VisualTestContext) {
    still();
    let seen = events();
    let s = seen.clone();
    let cx = open_host(cx, move || {
        let frozen = if config.frozen.is_empty() {
            vec![0]
        } else {
            config.frozen.clone()
        };
        let (selection, moves, cells, rows) = (s.clone(), s.clone(), s.clone(), s.clone());
        let mut table = Table::new(Vec::new())
            .id("frozen")
            .columns(
                FROZEN
                    .iter()
                    .enumerate()
                    .map(|(c, label)| {
                        let column = TableColumn::new(*label)
                            .allows_sorting(true)
                            .frozen(frozen.contains(&c));
                        if config.resizable && c == 0 {
                            column.allows_resizing(true).default_width(px(200.))
                        } else {
                            column.min_width(px(200.))
                        }
                    })
                    .collect(),
            )
            .on_sort_change(|_, _, _| {})
            .on_column_move(move |m, _, _| {
                moves
                    .borrow_mut()
                    .push(format!("move:{}->{}:{:?}", m.from, m.to, m.order));
            })
            .allows_column_reorder(config.reorder)
            .on_cell_select(move |cell, _, _| {
                cells
                    .borrow_mut()
                    .push(format!("cell:{}/{}", cell.row, cell.column));
            })
            .cell_selectable(config.cells)
            .on_selection_change(move |keys, _, _| {
                selection.borrow_mut().push(format!("select:{keys:?}"));
            });
        if config.selectable {
            table = table.selection_mode(SelectionMode::Multiple);
        } else {
            table = table.selection_mode(SelectionMode::Single);
        }
        if let Some(order) = config.order.clone() {
            table = table.column_order(order);
        }
        table = match config.body {
            Body::Plain => {
                for r in 0..3 {
                    table = table.tree_row(frozen_row(r, rows.clone()));
                }
                table
            }
            Body::Fixed => table.row_height(px(44.)).max_h(px(200.)).virtual_rows(
                30,
                "frozen",
                |ix| ix.to_string().into(),
                move |ix| frozen_row(ix, rows.clone()),
            ),
            Body::Estimated => table
                .estimated_row_height(px(44.))
                .max_h(px(200.))
                .virtual_rows(
                    30,
                    "frozen",
                    |ix| ix.to_string().into(),
                    move |ix| frozen_row(ix, rows.clone()),
                ),
        };
        gpui::div().w(px(600.)).child(table).into_any_element()
    });
    (seen, cx)
}

/// A horizontal wheel at `at`; negative `dx` scrolls the content left.
fn wheel_x(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, dx: f32) {
    cx.simulate_event(gpui::ScrollEvent {
        position: at,
        delta: gpui::ScrollDelta::Pixels(point(px(dx), px(0.))),
        ..Default::default()
    });
    frame(cx);
}

/// The painted quad of column `c`'s content in row `r`, found by its fill
/// and its row's vertical band.
fn painted_swatch(
    cx: &mut VisualTestContext,
    r: usize,
    c: usize,
) -> (gpui::Bounds<gpui::Pixels>, gpui::Bounds<gpui::Pixels>) {
    let band = cell(cx, r, 0);
    let scene = harness::painted(cx);
    let quad = scene
        .filled(swatch(c))
        .into_iter()
        .find(|q| (f32::from(scene.bounds(q).center().y) - f32::from(band.center().y)).abs() < 1.)
        .unwrap_or_else(|| panic!("row {r} column {c} painted"));
    (scene.bounds(quad), scene.mask(quad))
}

#[gpui::test]
fn frozen_cells_keep_their_x_while_the_body_and_header_scroll(cx: &mut TestAppContext) {
    let (_seen, cx) = frozen_host(cx, Frozen::default());
    frame(cx);
    let (frozen_before, _) = painted_swatch(cx, 1, 0);
    let (scrolled_before, _) = painted_swatch(cx, 1, 2);
    let header_before = cx.debug_bounds("table-header-track-2").unwrap();
    assert!(
        (f32::from(header_before.left()) - f32::from(scrolled_before.left())).abs() < 17.,
        "the header track sits over its column"
    );
    // Over the scrolling part, then over the frozen part: both scroll.
    wheel_x(cx, scrolled_before.center(), -90.);
    wheel_x(cx, frozen_before.center(), -60.);
    let (frozen_after, frozen_mask) = painted_swatch(cx, 1, 0);
    let (scrolled_after, scrolled_mask) = painted_swatch(cx, 1, 2);
    let header_after = cx.debug_bounds("table-header-track-2").unwrap();
    assert_eq!(frozen_after, frozen_before, "the frozen cell does not move");
    assert_eq!(
        scrolled_after.left(),
        scrolled_before.left() - px(150.),
        "the scrolling cell moves with the wheel"
    );
    assert_eq!(
        header_after.left(),
        header_before.left() - px(150.),
        "the header scrolls with the body"
    );
    // The scrolling part is clipped at the frozen part's edge; the frozen
    // cell is not clipped at all.
    assert!(scrolled_mask.left() >= frozen_after.right());
    assert!(frozen_mask.contains(&frozen_after.origin));
    // The scroll stops at the end of the content.
    wheel_x(cx, scrolled_after.center(), -5000.);
    let (last, last_mask) = painted_swatch(cx, 1, 4);
    assert!(last.right() <= last_mask.right() + px(1.));
    assert!(
        last.right() + px(100.) >= last_mask.right(),
        "scrolled to the end"
    );
}

#[gpui::test]
fn a_short_last_row_does_not_shrink_the_frozen_scroll_extent(cx: &mut TestAppContext) {
    still();
    let seen = events();
    let cx = open_host(cx, move || {
        let table = Table::new(Vec::new())
            .id("short-frozen")
            .columns(
                FROZEN
                    .iter()
                    .enumerate()
                    .map(|(c, label)| TableColumn::new(*label).min_width(px(200.)).frozen(c < 2))
                    .collect(),
            )
            .tree_row(frozen_row(0, seen.clone()))
            .tree_row(TableRow::new(vec![gpui::div()
                .w(px(120.))
                .h(px(20.))
                .bg(swatch(0))
                .into_any_element()]));
        gpui::div().w(px(600.)).child(table).into_any_element()
    });
    frame(cx);
    let frozen_before = cell(cx, 0, 0);
    let scrolling_before = cell(cx, 0, 2);
    let header_before = cx.debug_bounds("table-header-track-2").unwrap();
    wheel_x(cx, scrolling_before.center(), -80.);
    assert_eq!(cell(cx, 0, 0).left(), frozen_before.left());
    assert_eq!(cell(cx, 0, 2).left(), scrolling_before.left() - px(80.));
    assert_eq!(
        cx.debug_bounds("table-header-track-2").unwrap().left(),
        header_before.left() - px(80.),
    );
}

#[gpui::test]
fn a_press_on_a_frozen_cell_selects_the_row_and_misses_the_hidden_cells(cx: &mut TestAppContext) {
    let (seen, cx) = frozen_host(cx, Frozen::default());
    frame(cx);
    let frozen = cell(cx, 1, 0);
    let at = cell(cx, 1, 1).center();
    wheel_x(cx, at, -200.);
    // Column A has scrolled out under the frozen column's x range.
    let hidden = cell(cx, 1, 1);
    let at = point(frozen.center().x, frozen.center().y);
    assert!(
        hidden.contains(&at),
        "precondition: the hidden cell's box lies under the press ({hidden:?} vs {at:?})"
    );
    seen.borrow_mut().clear();
    cx.simulate_click(at, Modifiers::none());
    frame(cx);
    let log = log(&seen);
    assert!(log.contains(&"content:1/0".to_owned()), "{log:?}");
    assert!(
        !log.iter().any(|e| e == "content:1/1"),
        "the hidden cell is not reachable: {log:?}"
    );
    assert!(log.contains(&"select:[\"1\"]".to_owned()), "{log:?}");
}

#[gpui::test]
fn the_row_hover_spans_both_parts(cx: &mut TestAppContext) {
    let (_seen, cx) = frozen_host(cx, Frozen::default());
    frame(cx);
    let frozen = cell(cx, 1, 0);
    let scrolled = cell(cx, 1, 1);
    let before = harness::painted(cx);
    let quads_before = before.quads.len();
    cx.simulate_mouse_move(frozen.center(), None, Modifiers::none());
    frame(cx);
    let after = harness::painted(cx);
    // The hover paints a fill behind every cell of the row: one more quad
    // per cell, and one of them behind the scrolling cell.
    let hovered = |cell: gpui::Bounds<gpui::Pixels>| {
        after
            .quads
            .iter()
            .filter(|q| {
                let b = after.bounds(q);
                b.contains(&cell.center()) && b.size.width > cell.size.width
            })
            .count()
            > before
                .quads
                .iter()
                .filter(|q| {
                    let b = before.bounds(q);
                    b.contains(&cell.center()) && b.size.width > cell.size.width
                })
                .count()
    };
    assert!(after.quads.len() > quads_before);
    assert!(hovered(frozen), "the frozen cell takes the row hover");
    assert!(hovered(scrolled), "the scrolling cell takes it too");
}

#[gpui::test]
fn cell_selection_crosses_the_frozen_edge_and_reveals_the_cell(cx: &mut TestAppContext) {
    let (seen, cx) = frozen_host(
        cx,
        Frozen {
            cells: true,
            ..Frozen::default()
        },
    );
    frame(cx);
    let frozen = cell(cx, 0, 0);
    let a_before = cell(cx, 0, 1);
    // Column A's content sits 16px into the first scrolling cell.
    let viewport_left = a_before.left() - px(16.);
    // Scroll column A out of view, then select the frozen cell.
    let at = cell(cx, 0, 1).center();
    wheel_x(cx, at, -200.);
    assert!(cell(cx, 0, 1).right() <= viewport_left);
    cx.simulate_click(frozen.center(), Modifiers::none());
    frame(cx);
    keys(cx, "right");
    let revealed = cell(cx, 0, 1);
    assert_eq!(
        revealed.left(),
        a_before.left(),
        "Right across the edge scrolls column A back into view"
    );
    let ring = cx.debug_bounds("table-selected-cell").unwrap();
    assert!(ring.contains(&revealed.center()));
    keys(cx, "end");
    let last = cell(cx, 0, 4);
    let ring = cx.debug_bounds("table-selected-cell").unwrap();
    assert!(ring.contains(&last.center()), "End reaches the far column");
    keys(cx, "home down");
    assert_eq!(
        log(&seen)
            .into_iter()
            .filter(|e| e.starts_with("cell:"))
            .collect::<Vec<_>>(),
        ["cell:0/0", "cell:0/1", "cell:0/4", "cell:0/0", "cell:1/0"]
    );
    // Back on the frozen column, the frozen cell never moved.
    assert_eq!(cell(cx, 1, 0).left(), frozen.left());
}

#[gpui::test]
fn the_selection_column_is_frozen_with_the_frozen_columns(cx: &mut TestAppContext) {
    let (seen, cx) = frozen_host(
        cx,
        Frozen {
            selectable: true,
            ..Frozen::default()
        },
    );
    frame(cx);
    let frozen = cell(cx, 2, 0);
    let at = cell(cx, 2, 2).center();
    wheel_x(cx, at, -120.);
    assert_eq!(cell(cx, 2, 0).left(), frozen.left());
    // The row keys still walk the rows, and Space selects across the parts.
    keys(cx, "tab down space");
    assert!(
        log(&seen).iter().any(|e| e.starts_with("select:")),
        "{:?}",
        log(&seen)
    );
}

#[gpui::test]
fn a_frozen_column_does_not_move_into_the_scrolling_region(cx: &mut TestAppContext) {
    let (seen, cx) = frozen_host(
        cx,
        Frozen {
            reorder: true,
            frozen: vec![0, 1],
            ..Frozen::default()
        },
    );
    frame(cx);
    // Tab stops: the body, then Name, A, B, ... (every header sorts).
    keys(cx, "tab tab alt-right");
    assert_eq!(log(&seen), ["move:0->1:[1, 0, 2, 3, 4]"]);
    // A (now at 0) moved nowhere past the frozen edge; Name, at 1, is the
    // last frozen position and stays there.
    keys(cx, "alt-right");
    assert_eq!(log(&seen).len(), 1, "{:?}", log(&seen));
    // B, the first scrolling column, cannot move into the frozen region,
    // but moves right among the scrolling ones.
    keys(cx, "tab alt-left");
    assert_eq!(log(&seen).len(), 1, "{:?}", log(&seen));
    keys(cx, "alt-right");
    assert_eq!(log(&seen)[1], "move:2->3:[1, 0, 3, 2, 4]");
}

#[gpui::test]
fn a_frozen_column_is_displayed_first_whatever_the_order(cx: &mut TestAppContext) {
    let (_seen, cx) = frozen_host(
        cx,
        Frozen {
            order: Some(vec![2, 3, 0, 1, 4]),
            ..Frozen::default()
        },
    );
    frame(cx);
    let lefts: Vec<f32> = (0..FROZEN.len())
        .map(|c| f32::from(cell(cx, 0, c).left()))
        .collect();
    let mut by_x: Vec<usize> = (0..FROZEN.len()).collect();
    by_x.sort_by(|a, b| lefts[*a].total_cmp(&lefts[*b]));
    assert_eq!(by_x, [0, 2, 3, 1, 4]);
}

#[gpui::test]
fn resizing_a_frozen_column_moves_the_scrolling_region_in_every_row(cx: &mut TestAppContext) {
    let (_seen, cx) = frozen_host(
        cx,
        Frozen {
            resizable: true,
            ..Frozen::default()
        },
    );
    frame(cx);
    let before = cell(cx, 0, 1).left();
    // Body, Name's sort, Name's resizer: Enter starts, Right widens by 10.
    keys(cx, "tab tab tab enter right enter");
    for r in 0..3 {
        assert_eq!(cell(cx, r, 1).left(), before + px(10.), "row {r}");
    }
    let header = cx.debug_bounds("table-header-track-1").unwrap();
    assert!((f32::from(header.left()) - f32::from(before + px(10.))).abs() < 17.);
}

fn frozen_virtual(cx: &mut TestAppContext, body: Body) {
    let (seen, cx) = frozen_host(
        cx,
        Frozen {
            body,
            ..Frozen::default()
        },
    );
    frame(cx);
    let frozen = cell(cx, 1, 0);
    let scrolled = cell(cx, 1, 2);
    wheel_x(cx, scrolled.center(), -100.);
    assert_eq!(cell(cx, 1, 0).left(), frozen.left());
    assert_eq!(cell(cx, 1, 2).left(), scrolled.left() - px(100.));
    // A vertical wheel still scrolls the virtual body, and the rows it
    // builds come in at the same horizontal offset.
    cx.simulate_event(gpui::ScrollEvent {
        position: scrolled.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-300.))),
        ..Default::default()
    });
    frame(cx);
    // A row built inside the viewport, below the header: rows the list
    // builds past either edge are clipped.
    let top = cx.debug_bounds("table-header-track-0").unwrap().bottom();
    let bottom = cx.debug_bounds("frozen-scroll-x").unwrap().bottom();
    let row = (6..30)
        .find(|r| {
            cx.debug_bounds(Box::leak(format!("c-{r}-0").into_boxed_str()))
                .is_some_and(|b| b.top() > top && b.bottom() < bottom)
        })
        .expect("the body scrolled to later rows");
    assert_eq!(cell(cx, row, 0).left(), frozen.left());
    assert_eq!(cell(cx, row, 2).left(), scrolled.left() - px(100.));
    seen.borrow_mut().clear();
    let at = cell(cx, row, 0).center();
    cx.simulate_click(at, Modifiers::none());
    frame(cx);
    assert!(
        log(&seen).contains(&format!("select:[\"{row}\"]")),
        "{:?}",
        log(&seen)
    );
}

#[gpui::test]
fn frozen_columns_work_on_the_uniform_virtual_body(cx: &mut TestAppContext) {
    frozen_virtual(cx, Body::Fixed);
}

#[gpui::test]
fn frozen_columns_work_on_the_measured_virtual_body(cx: &mut TestAppContext) {
    frozen_virtual(cx, Body::Estimated);
}

/// A focused row rings each of its cells, and a selected cell rings itself,
/// with an `inset_0` overlay as wide as the cell. The intrinsic measurement
/// that floors a column's track must not read those overlays as content, or
/// the floor grows by the cell padding on every frame the ring is shown.
#[gpui::test]
fn a_focused_row_or_selected_cell_does_not_widen_its_column(cx: &mut TestAppContext) {
    let (_seen, cx) = host(
        cx,
        Config {
            cells: true,
            ..Config::default()
        },
    );
    frame(cx);
    let widths = |cx: &mut VisualTestContext| {
        (0..COLUMNS.len())
            .map(|c| {
                cx.debug_bounds(Box::leak(format!("table-row-track-0-{c}").into_boxed_str()))
                    .expect("row track")
                    .size
                    .width
            })
            .collect::<Vec<_>>()
    };
    let before = widths(cx);
    // The row cursor's ring, over several frames.
    keys(cx, "tab down");
    frame(cx);
    frame(cx);
    assert_eq!(widths(cx), before, "the focused row's ring is not content");
    // The selected cell's ring.
    let at = cell(cx, 0, 0).center();
    cx.simulate_click(at, Modifiers::none());
    frame(cx);
    frame(cx);
    assert_eq!(
        widths(cx),
        before,
        "the selected cell's ring is not content"
    );
}
