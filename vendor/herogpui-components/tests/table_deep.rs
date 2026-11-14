//! Deeper Table behaviour not covered by the sorting, selection, resize,
//! virtualisation, footer and load-more suites.

use crate::harness;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{prelude::*, px, SharedString, TestAppContext, VisualTestContext};
use herogpui_components::{SelectionBehavior, SelectionMode, Table, TableColumn, TableRow};

use harness::{click, events, open_host, press};

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

#[gpui::test]
fn table_body_rows_share_intrinsic_column_tracks_with_the_header(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(420.))
            .child(
                Table::new(vec!["Name".into(), "Notes".into()])
                    .id("table-track")
                    .row(vec![
                        gpui::div()
                            .w(px(240.))
                            .child("A long first-column value")
                            .into_any_element(),
                        gpui::div().child("first").into_any_element(),
                    ])
                    .row(vec![
                        gpui::div().child("Short").into_any_element(),
                        gpui::div().child("second").into_any_element(),
                    ]),
            )
            .into_any_element()
    });
    // The first pass records the long cell's intrinsic minimum; the following
    // pass applies that same minimum to the header and every body row.
    for _ in 0..3 {
        flush_frame(cx);
    }
    let header = cx
        .debug_bounds("table-header-track-1")
        .expect("the second header track paints");
    let first_row = cx
        .debug_bounds("table-row-track-0-1")
        .expect("the first row's second track paints");
    let second_row = cx
        .debug_bounds("table-row-track-1-1")
        .expect("the second row's second track paints");
    assert_eq!(header.origin.x, first_row.origin.x);
    assert_eq!(first_row.origin.x, second_row.origin.x);
}

/// Upstream draws one `border-separate` table, so the header's `<th>` cells
/// define tracks that every body row shares. A table narrower than its
/// columns must therefore keep one track set — the grid overflows the
/// viewport and the scroll container slides the whole grid — instead of
/// letting each row resolve its own tracks and stagger against the header.
#[gpui::test]
fn a_table_narrower_than_its_columns_keeps_one_track_set_across_header_and_rows(
    cx: &mut TestAppContext,
) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(320.))
            .child(
                Table::new(vec!["First".into(), "Second".into(), "Third".into()])
                    .id("table-narrow")
                    .row(vec![
                        gpui::div().w(px(200.)).child("wide-one").into_any_element(),
                        gpui::div().w(px(150.)).child("wide-two").into_any_element(),
                        gpui::div()
                            .w(px(120.))
                            .child("wide-three")
                            .into_any_element(),
                    ])
                    .row(vec![
                        gpui::div().child("narrow").into_any_element(),
                        gpui::div().child("cells").into_any_element(),
                        gpui::div().child("here").into_any_element(),
                    ]),
            )
            .into_any_element()
    });
    // The first pass records the widest cell per column; the next passes lay
    // every row out against those shared minima.
    for _ in 0..3 {
        flush_frame(cx);
    }
    let mut header_tracks = Vec::new();
    let mut row_tracks = Vec::new();
    for column in 0..3 {
        let header_selector: &'static str =
            Box::leak(format!("table-header-track-{column}").into_boxed_str());
        let first_selector: &'static str =
            Box::leak(format!("table-row-track-0-{column}").into_boxed_str());
        let second_selector: &'static str =
            Box::leak(format!("table-row-track-1-{column}").into_boxed_str());
        let header = cx
            .debug_bounds(header_selector)
            .unwrap_or_else(|| panic!("header track {column} paints"));
        let first = cx
            .debug_bounds(first_selector)
            .unwrap_or_else(|| panic!("first row's track {column} paints"));
        let second = cx
            .debug_bounds(second_selector)
            .unwrap_or_else(|| panic!("second row's track {column} paints"));
        assert_eq!(
            header.origin.x, first.origin.x,
            "column {column}: header and first body row must share a track origin"
        );
        assert_eq!(
            first.origin.x, second.origin.x,
            "column {column}: body rows must share a track origin"
        );
        assert_eq!(
            header.size.width, first.size.width,
            "column {column}: header and body must share a track width"
        );
        assert_eq!(
            first.size.width, second.size.width,
            "column {column}: body rows must share a track width"
        );
        header_tracks.push(header);
        row_tracks.push(first);
    }
    // The columns' natural widths (232 + 182 + 152 with padding) exceed the
    // 320px viewport, so the whole grid must overflow it and scroll together
    // rather than squeeze into it.
    let grid_right = header_tracks[2].origin.x + header_tracks[2].size.width;
    assert!(
        grid_right > px(320.),
        "a narrow table must overflow its viewport so the scroller moves the \
         whole grid; last track ends at {grid_right}"
    );
    assert_eq!(
        row_tracks[1].origin.x - row_tracks[0].origin.x,
        header_tracks[1].origin.x - header_tracks[0].origin.x,
        "the first track's width must agree between header and body"
    );
}

/// The shared-track contract must also hold on a sortable narrow table, where
/// each header cell is `w_full` inside a flexible wrapper: the wrapper, not
/// the per-row flex resolution, has to carry the column's measured minimum so
/// the header still lands on the body's tracks.
#[gpui::test]
fn a_narrow_sortable_table_keeps_one_track_set_across_header_and_rows(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(320.))
            .child(
                Table::new(vec![])
                    .id("table-narrow-sortable")
                    .column(TableColumn::new("First").allows_sorting(true))
                    .column(TableColumn::new("Second").allows_sorting(true))
                    .column(TableColumn::new("Third").allows_sorting(true))
                    .on_sort_change(|_, _, _| {})
                    .row(vec![
                        gpui::div().w(px(220.)).child("wide-one").into_any_element(),
                        gpui::div().w(px(140.)).child("wide-two").into_any_element(),
                        gpui::div()
                            .w(px(100.))
                            .child("wide-three")
                            .into_any_element(),
                    ])
                    .row(vec![
                        gpui::div().child("narrow").into_any_element(),
                        gpui::div().child("cells").into_any_element(),
                        gpui::div().child("here").into_any_element(),
                    ]),
            )
            .into_any_element()
    });
    for _ in 0..3 {
        flush_frame(cx);
    }
    for column in 0..3 {
        let header_selector: &'static str =
            Box::leak(format!("table-header-track-{column}").into_boxed_str());
        let body_selector: &'static str =
            Box::leak(format!("table-row-track-0-{column}").into_boxed_str());
        let header = cx
            .debug_bounds(header_selector)
            .unwrap_or_else(|| panic!("header track {column} paints"));
        let body = cx
            .debug_bounds(body_selector)
            .unwrap_or_else(|| panic!("body track {column} paints"));
        assert_eq!(
            header.origin.x, body.origin.x,
            "sortable column {column}: header and body must share a track origin"
        );
        assert_eq!(
            header.size.width, body.size.width,
            "sortable column {column}: header and body must share a track width"
        );
    }
}

/// A sortable wrapper must carry the column's explicit width: upstream's
/// `<th>` honors `width` on a sortable column, so the header track cannot
/// become an equal share of the leftover space while the body keeps the named
/// width.
#[gpui::test]
fn a_sortable_explicit_width_column_gives_the_header_the_body_track(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(800.))
            .child(
                Table::new(vec![])
                    .id("table-sortable-widths")
                    .column(
                        TableColumn::new("First")
                            .allows_sorting(true)
                            .default_width(px(232.)),
                    )
                    .column(
                        TableColumn::new("Second")
                            .allows_sorting(true)
                            .default_width(px(182.)),
                    )
                    .on_sort_change(|_, _, _| {})
                    .row(vec![
                        gpui::div().child("a").into_any_element(),
                        gpui::div().child("b").into_any_element(),
                    ]),
            )
            .into_any_element()
    });
    for _ in 0..3 {
        flush_frame(cx);
    }
    let mut body_origins = Vec::new();
    for column in 0..2 {
        let header_selector: &'static str =
            Box::leak(format!("table-header-track-{column}").into_boxed_str());
        let body_selector: &'static str =
            Box::leak(format!("table-row-track-0-{column}").into_boxed_str());
        let header = cx
            .debug_bounds(header_selector)
            .unwrap_or_else(|| panic!("header track {column} paints"));
        let body = cx
            .debug_bounds(body_selector)
            .unwrap_or_else(|| panic!("body track {column} paints"));
        assert_eq!(
            header.size.width, body.size.width,
            "sortable column {column}: the header must keep the column width"
        );
        assert_eq!(
            header.origin.x, body.origin.x,
            "sortable column {column}: header and body must share a track origin"
        );
        body_origins.push(body.origin.x);
    }
    assert_eq!(body_origins[1] - body_origins[0], px(232.));
}

/// The windowed body draws the same `RowCtx` rows a short table does, so a
/// narrow virtual table must share the header's track set exactly like the
/// plain path.
#[gpui::test]
fn a_narrow_virtual_table_keeps_one_track_set_across_header_and_rows(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(320.))
            .child(
                Table::new(vec!["First".into(), "Second".into()])
                    .id("table-narrow-virtual")
                    .row_height(px(44.))
                    .max_h(px(120.))
                    .virtual_rows(
                        20,
                        "narrow-virtual-rows",
                        |i| SharedString::from(i.to_string()),
                        |_| {
                            TableRow::new(vec![
                                gpui::div().w(px(300.)).child("wide-one").into_any_element(),
                                gpui::div().w(px(200.)).child("wide-two").into_any_element(),
                            ])
                        },
                    ),
            )
            .into_any_element()
    });
    for _ in 0..4 {
        flush_frame(cx);
    }
    for column in 0..2 {
        let header_selector: &'static str =
            Box::leak(format!("table-header-track-{column}").into_boxed_str());
        let header = cx
            .debug_bounds(header_selector)
            .unwrap_or_else(|| panic!("header track {column} paints"));
        let body_widths = [0, 1]
            .iter()
            .map(|row| {
                let selector: &'static str =
                    Box::leak(format!("table-row-track-{row}-{column}").into_boxed_str());
                cx.debug_bounds(selector)
                    .unwrap_or_else(|| panic!("virtual row {row} track {column} paints"))
            })
            .collect::<Vec<_>>();
        for (row, body) in body_widths.iter().enumerate() {
            assert_eq!(
                header.origin.x, body.origin.x,
                "virtual row {row} column {column}: header and body must share a track origin"
            );
            assert_eq!(
                header.size.width, body.size.width,
                "virtual row {row} column {column}: header and body must share a track width"
            );
        }
    }
}

/// A table narrower than its columns must not squeeze into the viewport: the
/// scroll container keeps the viewport width while the shared grid extends
/// past it, which is the overflow `overflow-x-auto` scrolls together.
#[gpui::test]
fn a_narrow_table_overflows_its_scroll_viewport_instead_of_squeezing(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(320.))
            .child(
                Table::new(vec!["First".into(), "Second".into()])
                    .id("table-narrow-scroll")
                    .row(vec![
                        gpui::div().w(px(280.)).child("wide-one").into_any_element(),
                        gpui::div().w(px(240.)).child("wide-two").into_any_element(),
                    ]),
            )
            .into_any_element()
    });
    for _ in 0..3 {
        flush_frame(cx);
    }
    let viewport = cx
        .debug_bounds("table-narrow-scroll-scroll-x")
        .expect("the scroll viewport paints");
    assert!(
        viewport.size.width <= px(321.),
        "the scroll viewport must stay at the table box width, not follow the \
         grid; got {:?}",
        viewport.size.width
    );
    let last = cx
        .debug_bounds("table-header-track-1")
        .expect("the last header track paints");
    let grid_right = last.origin.x + last.size.width;
    assert!(
        grid_right > viewport.right(),
        "the shared grid must extend past the scroll viewport so the whole \
         grid scrolls; grid ends at {grid_right}, viewport at {:?}",
        viewport.right()
    );
}

/// Selection must survive the narrow layout: the keyboard cursor still walks
/// the rows and Enter still selects through the scroller's overflow.
#[gpui::test]
fn a_narrow_table_still_walks_and_selects_rows_with_the_keyboard(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec!["First".into(), "Second".into()])
            .id("table-narrow-select")
            .selection_mode(SelectionMode::Single)
            .keyed_row(
                "alpha",
                vec![
                    gpui::div().w(px(280.)).child("wide-one").into_any_element(),
                    gpui::div().w(px(240.)).child("wide-two").into_any_element(),
                ],
            )
            .keyed_row(
                "beta",
                vec![
                    gpui::div().child("narrow").into_any_element(),
                    gpui::div().child("cells").into_any_element(),
                ],
            )
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
    for _ in 0..3 {
        flush_frame(cx);
    }
    // The tracks must still be shared while the keyboard work happens.
    let header = cx
        .debug_bounds("table-header-track-1")
        .expect("the header track paints");
    let body = cx
        .debug_bounds("table-row-track-0-1")
        .expect("the body track paints");
    assert_eq!(header.origin.x, body.origin.x);

    press(cx, "tab");
    press(cx, "down");
    press(cx, "enter");
    flush_frame(cx);
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "beta"],
        "the narrow table must walk to and select the second row"
    );
}

/// HeroUI's tree-column rule replaces the normal start padding with one rem
/// for each 1-based row level. A deep tree must therefore advance by one 16px
/// step per level; the old `12 + 20 * depth` approximation drifted four pixels
/// too far by the grandchild and made nested labels visibly misalign.
#[gpui::test]
fn table_tree_column_indent_advances_one_rem_per_level(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        Table::new(vec![])
            .id("table-tree-indent")
            .column(TableColumn::new("Name").default_width(px(320.)))
            .tree_column(0)
            .expanded_keys([SharedString::from("root"), SharedString::from("child")])
            .tree_row(
                TableRow::new(vec![gpui::div()
                    .id("tree-indent-root-label")
                    .debug_selector(|| "tree-indent-root-label".to_owned())
                    .child("Root")
                    .into_any_element()])
                .key("root")
                .children(vec![TableRow::new(vec![gpui::div()
                    .id("tree-indent-child-label")
                    .debug_selector(|| "tree-indent-child-label".to_owned())
                    .child("Child")
                    .into_any_element()])
                .key("child")
                .children(vec![TableRow::new(vec![gpui::div()
                    .id("tree-indent-leaf-label")
                    .debug_selector(|| "tree-indent-leaf-label".to_owned())
                    .child("Leaf")
                    .into_any_element()])
                .key("leaf")])]),
            )
            .into_any_element()
    });

    for _ in 0..3 {
        flush_frame(cx);
    }
    let root = cx
        .debug_bounds("tree-indent-root-label")
        .expect("the root tree label paints");
    let child = cx
        .debug_bounds("tree-indent-child-label")
        .expect("the child tree label paints");
    let leaf = cx
        .debug_bounds("tree-indent-leaf-label")
        .expect("the grandchild tree label paints");
    assert_eq!(child.origin.x - root.origin.x, px(16.));
    assert_eq!(leaf.origin.x - child.origin.x, px(16.));
}

fn press_mod_a(cx: &mut VisualTestContext) {
    if cfg!(target_os = "macos") {
        press(cx, "cmd-a");
    } else {
        press(cx, "ctrl-a");
    }
}

/// v3's tree-table example controls `expandedKeys` through
/// `onExpandedChange`. The first data row starts after the ~37px header; its
/// chevron is 18px square after the tree cell's 16px left padding, so (29, 58)
/// is its centre inside the primary table's 4px tray.
#[gpui::test]
fn table_tree_chevron_reports_expand_then_collapse(cx: &mut TestAppContext) {
    let expanded = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let expanded_for_view = expanded;
    let recorded = events();
    let recorded_for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let expanded = expanded_for_view.clone();
        let expanded_now = expanded.borrow().clone();
        let recorded = recorded_for_view.clone();
        gpui::div()
            .w(px(320.))
            .child(
                Table::new(vec![])
                    .id("table-tree-deep")
                    .column(TableColumn::new("Name").default_width(px(320.)))
                    .tree_column(0)
                    .expanded_keys(expanded_now)
                    .tree_row(
                        TableRow::new(vec![gpui::div().child("Parent").into_any_element()])
                            .key("parent")
                            .children(vec![TableRow::new(vec![gpui::div()
                                .child("Child")
                                .into_any_element()])
                            .key("child")]),
                    )
                    .on_expanded_change(move |keys, window, _| {
                        *expanded.borrow_mut() = keys.to_vec();
                        recorded.borrow_mut().push(
                            keys.iter()
                                .map(AsRef::<str>::as_ref)
                                .collect::<Vec<_>>()
                                .join(","),
                        );
                        window.refresh();
                    }),
            )
            .into_any_element()
    });

    click(cx, 29., 58.);
    flush_frame(cx);
    click(cx, 29., 58.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["parent", ""],
        "the same tree chevron must report the controlled expanded set on open and close"
    );
}

#[gpui::test]
fn table_tree_right_expands_and_left_collapses_the_focused_parent(cx: &mut TestAppContext) {
    let expanded = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let expanded_for_view = expanded;
    let recorded = events();
    let recorded_for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let expanded = expanded_for_view.clone();
        let expanded_now = expanded.borrow().clone();
        let recorded = recorded_for_view.clone();
        Table::new(vec![])
            .id("table-tree-keys")
            .column(TableColumn::new("Name").default_width(px(320.)))
            .tree_column(0)
            .expanded_keys(expanded_now)
            .tree_row(
                TableRow::new(vec![gpui::div().child("Parent").into_any_element()])
                    .key("parent")
                    .children(vec![TableRow::new(vec![gpui::div()
                        .child("Child")
                        .into_any_element()])
                    .key("child")]),
            )
            .on_expanded_change(move |keys, window, _| {
                *expanded.borrow_mut() = keys.to_vec();
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(AsRef::<str>::as_ref)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                window.refresh();
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    cx.update(|_, cx| herogpui_components::extend::set_focus_visible(false, cx));
    press(cx, "right");
    assert!(
        cx.update(|_, cx| herogpui_components::extend::focus_visible(cx)),
        "a handled tree key must still record keyboard focus visibility"
    );
    flush_frame(cx);
    press(cx, "left");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["parent", ""],
        "Right must expand the focused parent and Left must collapse it again"
    );
}

#[gpui::test]
fn table_tree_left_on_a_child_moves_the_cursor_to_its_parent(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-tree-parent-key")
            .column(TableColumn::new("Name").default_width(px(320.)))
            .tree_column(0)
            .expanded_keys([SharedString::from("parent")])
            .tree_row(
                TableRow::new(vec![gpui::div().child("Parent").into_any_element()])
                    .key("parent")
                    .children(vec![TableRow::new(vec![gpui::div()
                        .child("Child")
                        .into_any_element()])
                    .key("child")]),
            )
            .on_row_click(move |index, _, _, _| {
                recorded.borrow_mut().push(index.to_string());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "down");
    press(cx, "left");
    press(cx, "enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["0"],
        "Left on a child must move the row cursor to its parent before Enter activates"
    );
}

#[gpui::test]
fn callbackless_table_tree_consumes_an_expand_key(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .on_key_down(move |event, _, _| {
                recorded.borrow_mut().push(event.keystroke.key.clone());
            })
            .child(
                Table::new(vec![])
                    .id("table-tree-controlled-read-only")
                    .column(TableColumn::new("Name").default_width(px(320.)))
                    .tree_column(0)
                    .tree_row(
                        TableRow::new(vec![gpui::div().child("Parent").into_any_element()])
                            .key("parent")
                            .children(vec![TableRow::new(vec![gpui::div()
                                .child("Child")
                                .into_any_element()])
                            .key("child")]),
                    ),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    recorded.borrow_mut().clear();
    cx.update(|_, cx| herogpui_components::extend::set_focus_visible(false, cx));
    press(cx, "right");

    assert!(
        recorded.borrow().is_empty(),
        "Right on a collapsed parent must not escape a callbackless controlled tree"
    );
    assert!(
        cx.update(|_, cx| herogpui_components::extend::focus_visible(cx)),
        "a consumed tree key must still record keyboard focus visibility"
    );
}

/// React Aria's default `disabledBehavior="all"` disables every interaction
/// on a row, including the expansion button it composes in the tree column.
#[gpui::test]
fn disabled_table_tree_chevron_is_inert(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(320.))
            .child(
                Table::new(vec![])
                    .id("table-tree-disabled")
                    .column(TableColumn::new("Name").default_width(px(320.)))
                    .tree_column(0)
                    .disabled_keys(["parent"])
                    .tree_row(
                        TableRow::new(vec![gpui::div().child("Parent").into_any_element()])
                            .key("parent")
                            .children(vec![TableRow::new(vec![gpui::div()
                                .child("Child")
                                .into_any_element()])
                            .key("child")]),
                    )
                    .on_expanded_change(move |keys, _, _| {
                        recorded.borrow_mut().push(
                            keys.iter()
                                .map(AsRef::<str>::as_ref)
                                .collect::<Vec<_>>()
                                .join(","),
                        );
                    }),
            )
            .into_any_element()
    });

    click(cx, 29., 58.);
    assert!(
        recorded.borrow().is_empty(),
        "a disabled expandable row must not report an expanded-key change"
    );
}

/// Pinned `TableKeyboardDelegate.getKeyForSearch` searches row text, wraps,
/// and skips rows excluded from the roving collection. The typeahead only
/// moves focus; Enter proves which row it found.
#[gpui::test]
fn table_typeahead_uses_row_text_and_skips_disabled_matches(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-typeahead-text")
            .column(TableColumn::new("Name").default_width(px(320.)))
            .disabled_keys(["delta"])
            .tree_row(
                TableRow::new(vec![gpui::div().child("Dawn").into_any_element()])
                    .key("dawn")
                    .text_value("Dawn"),
            )
            .tree_row(
                TableRow::new(vec![gpui::div().child("Delta").into_any_element()])
                    .key("delta")
                    .text_value("Delta"),
            )
            .tree_row(
                TableRow::new(vec![gpui::div().child("Denmark").into_any_element()])
                    .key("denmark")
                    .text_value("Denmark"),
            )
            .on_row_click(move |index, _, _, _| recorded.borrow_mut().push(index.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "d e");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2"],
        "the growing 'de' query must skip disabled Delta and focus Denmark"
    );
}

/// Pinned `useTypeSelect` appends repeated letters verbatim. A failed `dd`
/// query clears the buffer but leaves focus on Dawn; it does not turn the
/// second `d` into a request for the next d-row.
#[gpui::test]
fn table_typeahead_does_not_cycle_a_repeated_letter(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-typeahead-repeat")
            .column(TableColumn::new("Name").default_width(px(320.)))
            .tree_row(
                TableRow::new(vec![gpui::div().child("Dawn").into_any_element()])
                    .key("dawn")
                    .text_value("Dawn"),
            )
            .tree_row(
                TableRow::new(vec![gpui::div().child("Denmark").into_any_element()])
                    .key("denmark")
                    .text_value("Denmark"),
            )
            .on_row_click(move |index, _, _, _| recorded.borrow_mut().push(index.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "d d");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["0"],
        "a repeated letter must not cycle to the next matching row"
    );
}

/// Once pinned `useTypeSelect` has a query, Space extends it instead of
/// activating the current row. This distinguishes "New York" from "New".
#[gpui::test]
fn table_typeahead_includes_space_after_the_query_starts(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-typeahead-space")
            .column(TableColumn::new("Name").default_width(px(320.)))
            .tree_row(
                TableRow::new(vec![gpui::div().child("New").into_any_element()])
                    .key("new")
                    .text_value("New"),
            )
            .tree_row(
                TableRow::new(vec![gpui::div().child("New York").into_any_element()])
                    .key("new-york")
                    .text_value("New York"),
            )
            .on_row_click(move |index, _, _, _| recorded.borrow_mut().push(index.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "n e w space");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1"],
        "Space inside a live query must distinguish the longer prefix"
    );
}

/// Pinned `getStringForKey` accepts any single printable character, not only
/// letters and digits.
#[gpui::test]
fn table_typeahead_accepts_punctuation(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-typeahead-punctuation")
            .column(TableColumn::new("Name").default_width(px(320.)))
            .tree_row(
                TableRow::new(vec![gpui::div().child("-Dash").into_any_element()])
                    .key("dash")
                    .text_value("-Dash"),
            )
            .on_row_click(move |index, _, _, _| recorded.borrow_mut().push(index.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "-");
    press(cx, "enter");
    assert_eq!(recorded.borrow().as_slice(), ["0"]);
}

/// `useTypeSelect` clears its query after one second. A later Space activates
/// selection again, even when no other key arrived to clear our lazy buffer.
#[gpui::test]
fn table_typeahead_timeout_restores_space_activation(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-typeahead-timeout")
            .column(TableColumn::new("Name").default_width(px(320.)))
            .selection_mode(SelectionMode::Multiple)
            .tree_row(
                TableRow::new(vec![gpui::div().child("North").into_any_element()])
                    .key("north")
                    .text_value("North"),
            )
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
    press(cx, "n");
    std::thread::sleep(std::time::Duration::from_millis(1020));
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["north"],
        "Space after the query timeout must select the focused row"
    );
}

/// Pinned `useTypeSelect` intercepts Space during capture while a query is
/// live, before a focused row checkbox can arm its own keyboard activation.
#[gpui::test]
fn table_typeahead_space_precedes_a_focused_row_checkbox(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-typeahead-checkbox")
            .column(TableColumn::new("Name").default_width(px(320.)))
            .selection_mode(SelectionMode::Multiple)
            .tree_row(
                TableRow::new(vec![gpui::div().child("North").into_any_element()])
                    .key("north")
                    .text_value("North"),
            )
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
    press(cx, "n");
    click(cx, 22., 58.);
    recorded.borrow_mut().clear();
    press(cx, "space");
    assert!(
        recorded.borrow().is_empty(),
        "Space extending a live query must not toggle the focused row checkbox"
    );
}

/// A virtual table's collection still owns every row's text value even though
/// the viewport builds only nearby elements. Typeahead must find an offscreen
/// enabled match without eagerly constructing the full row set.
#[gpui::test]
fn virtual_table_typeahead_searches_unbuilt_row_text(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let builds = Rc::new(Cell::new(0usize));
    let builds_for_view = builds.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let builds = builds_for_view.clone();
        Table::new(vec![])
            .id("table-typeahead-virtual")
            .column(TableColumn::new("Name").default_width(px(320.)))
            .row_height(px(40.))
            .max_h(px(160.))
            .disabled_keys(["25"])
            .virtual_rows(
                50,
                "typeahead-rows",
                |index| SharedString::from(index.to_string()),
                move |index| {
                    builds.set(builds.get() + 1);
                    TableRow::new(vec![gpui::div()
                        .child(format!("Row {index}"))
                        .into_any_element()])
                    .key(index.to_string())
                },
            )
            .virtual_text_value(|index| match index {
                25 => SharedString::from("Zeta"),
                40 => SharedString::from("Zulu"),
                _ => SharedString::from(format!("Row {index}")),
            })
            .on_row_click(move |index, _, _, _| recorded.borrow_mut().push(index.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "z");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["40"],
        "typeahead must skip disabled offscreen Zeta and focus offscreen Zulu"
    );
    assert!(
        builds.get() < 50,
        "projecting typeahead text must not eagerly construct every virtual row"
    );
}

/// `Table.Body.renderEmptyState` is interactive content, not a painted label.
/// With a 320px table its full-width 40px probe is centred below the ~37px
/// header and the empty wrapper's 28px top padding, so (160, 85) is inside it.
#[gpui::test]
fn table_empty_state_keeps_its_content_interactive(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(320.))
            .child(
                Table::new(vec!["Name".into()])
                    .id("table-empty-deep")
                    .empty_state(
                        gpui::div()
                            .id("table-empty-probe")
                            .w_full()
                            .h(px(40.))
                            .on_click(move |_, _, _| recorded.borrow_mut().push("empty".into())),
                    ),
            )
            .into_any_element()
    });

    click(cx, 160., 85.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["empty"],
        "an empty table must preserve the behavior of its renderEmptyState content"
    );
}

/// `selectionMode="none"` is action-only. Enter must still activate the row,
/// but `onSelectionChange` has no selection to report and must remain silent.
#[gpui::test]
fn table_none_mode_keyboard_fires_only_the_row_action(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let actions = for_view.clone();
        let selections = for_view.clone();
        Table::new(vec![])
            .id("table-none-deep")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::None)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .on_row_click(move |index, _, _, _| {
                actions.borrow_mut().push(format!("action:{index}"));
            })
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

    press(cx, "tab");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["action:0"],
        "none mode must preserve the row action without reporting selection"
    );
}

/// Single selection replaces the current row and clears when the selected row
/// is activated again. The caller feeds each controlled set back before the
/// next key, so the test exercises the table's real per-frame selection input.
#[gpui::test]
fn table_single_mode_keyboard_replaces_then_clears(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let held_for_view = held;
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let held = held_for_view.clone();
        let selected = held.borrow().clone();
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-single-deep")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Single)
            .selected_keys(selected)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .on_selection_change(move |keys, window, _| {
                *held.borrow_mut() = keys.to_vec();
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                window.refresh();
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "enter");
    flush_frame(cx);
    press(cx, "down");
    press(cx, "enter");
    flush_frame(cx);
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "beta", ""],
        "single mode must replace the selected key and permit clearing it"
    );
}

/// React Aria's `selectionBehavior="replace"` selects the focused row while
/// navigating, and a plain activation replaces a multi-selection instead of
/// toggling membership.
#[gpui::test]
fn table_replace_behavior_selects_on_focus_and_collapses_selection(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(vec![
        SharedString::from("alpha"),
        SharedString::from("beta"),
    ]));
    let held_for_view = held;
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let held = held_for_view.clone();
        let selected = held.borrow().clone();
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-replace-behavior")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .selection_behavior(SelectionBehavior::Replace)
            .selected_keys(selected)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .keyed_row("gamma", vec![gpui::div().child("Gamma").into_any_element()])
            .on_selection_change(move |keys, window, _| {
                *held.borrow_mut() = keys.to_vec();
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                window.refresh();
            })
            .into_any_element()
    });

    press(cx, "tab down");
    flush_frame(cx);
    press(cx, "down");
    flush_frame(cx);
    press(cx, "space");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "beta"],
        "replace mode selects on focus and keeps a re-activated row selected"
    );
}

#[gpui::test]
fn table_selection_rerender_preserves_all_column_tracks(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(600.))
            .child(
                Table::new(vec!["Name".into(), "Role".into(), "Status".into()])
                    .id("table-selection-rerender-tracks")
                    .selection_mode(SelectionMode::Multiple)
                    .selection_behavior(SelectionBehavior::Replace)
                    .default_selected_keys(["0"])
                    .keyed_row(
                        "0",
                        vec![
                            gpui::div().child("Tony Reichert").into_any_element(),
                            gpui::div().child("CEO").into_any_element(),
                            gpui::div().child("Active").into_any_element(),
                        ],
                    )
                    .keyed_row(
                        "1",
                        vec![
                            gpui::div().child("Zoey Lang").into_any_element(),
                            gpui::div().child("Tech Lead").into_any_element(),
                            gpui::div().child("Paused").into_any_element(),
                        ],
                    )
                    .into_any_element(),
            )
            .into_any_element()
    });

    for _ in 0..3 {
        flush_frame(cx);
    }
    press(cx, "tab down down");
    for _ in 0..3 {
        flush_frame(cx);
    }

    for (column, selector) in [
        "table-header-track-0",
        "table-header-track-1",
        "table-header-track-2",
    ]
    .into_iter()
    .enumerate()
    {
        let bounds = cx
            .debug_bounds(selector)
            .expect("every table header track remains painted after selection");
        assert!(
            bounds.size.width > px(40.),
            "selection rerender collapsed table header track {column}: {bounds:?}"
        );
    }
}

/// `disallowEmptySelection` applies to both Escape and the explicit row
/// selection toggle, matching React Stately's `clearSelection` and
/// `toggleSelection` guards.
#[gpui::test]
fn table_disallow_empty_selection_keeps_the_last_row(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-disallow-empty")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .disallow_empty_selection(true)
            .selected_keys([SharedString::from("alpha")])
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
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

    press(cx, "tab escape");
    assert!(
        recorded.borrow().is_empty(),
        "Escape must not clear a non-empty selection when empty selection is disallowed"
    );
}

/// React Aria's inherited `useGrid` contract defaults Escape to clearing a
/// non-empty selection. The table body owns the roving stop, so the clear is
/// reported through the same controlled selection callback as a row press.
#[gpui::test]
fn table_escape_clears_a_non_empty_selection(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-escape-deep")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .selected_keys([SharedString::from("alpha")])
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
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

    press(cx, "tab escape");
    assert_eq!(
        recorded.borrow().as_slice(),
        [""],
        "Escape must clear the table's non-empty selection"
    );
}

/// Pinned React Aria extends a multiple selection from the last toggled row
/// when Shift is held during arrow navigation.
#[gpui::test]
fn table_shift_arrows_extend_and_reverse_from_selection_anchor(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let held_for_view = held;
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = held_for_view.borrow().clone();
        let held = held_for_view.clone();
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-shift-extend")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(selected)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .keyed_row("gamma", vec![gpui::div().child("Gamma").into_any_element()])
            .on_selection_change(move |keys, window, _| {
                *held.borrow_mut() = keys.to_vec();
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                window.refresh();
            })
            .into_any_element()
    });

    press(cx, "tab down space");
    flush_frame(cx);
    press(cx, "down shift-space");
    flush_frame(cx);
    press(cx, "shift-space");
    flush_frame(cx);
    press(cx, "up shift-up down");
    flush_frame(cx);
    press(cx, "shift-down");
    flush_frame(cx);
    press(cx, "shift-up");
    flush_frame(cx);
    press(cx, "down shift-enter");
    flush_frame(cx);
    press_mod_a(cx);
    flush_frame(cx);
    press(cx, "shift-up");
    flush_frame(cx);
    press_mod_a(cx);
    flush_frame(cx);
    press(cx, "shift-down");
    assert_eq!(
        recorded.borrow().as_slice(),
        [
            "alpha",
            "alpha,beta",
            "alpha,beta,gamma",
            "alpha,beta",
            "alpha,beta,gamma",
            "alpha,beta",
            "alpha,beta,gamma",
            "gamma",
        ],
        "Shift+Arrow must rebuild the range from the last toggled row"
    );
}

/// Range extension uses the Table's own uncontrolled selection state and skips
/// disabled rows inside the anchor-to-target span.
#[gpui::test]
fn table_uncontrolled_shift_range_skips_disabled_rows(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-shift-uncontrolled")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .disabled_keys([SharedString::from("beta")])
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .keyed_row("gamma", vec![gpui::div().child("Gamma").into_any_element()])
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

    press(cx, "tab down space");
    flush_frame(cx);
    press(cx, "shift-down");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,gamma"],
        "an uncontrolled Shift range must exclude disabled collection rows"
    );
}

/// The pointer select-all control enters the same pinned `all` selection state
/// as Mod+A, so the next Shift move collapses to its target row.
#[gpui::test]
fn table_header_select_all_resets_the_shift_range(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let held_for_view = held;
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = held_for_view.borrow().clone();
        let held = held_for_view.clone();
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-header-all-range")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(selected)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .keyed_row("gamma", vec![gpui::div().child("Gamma").into_any_element()])
            .on_selection_change(move |keys, window, _| {
                *held.borrow_mut() = keys.to_vec();
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                window.refresh();
            })
            .into_any_element()
    });

    press(cx, "tab down space");
    flush_frame(cx);
    click(cx, 22., 18.);
    flush_frame(cx);
    press(cx, "shift-tab shift-down");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta,gamma", "beta"],
        "header select-all must not leave the previous row anchor active"
    );
}

#[gpui::test]
fn table_header_select_all_respects_disallow_empty_selection(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(vec![
        SharedString::from("alpha"),
        SharedString::from("beta"),
    ]));
    let held_for_view = held;
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = held_for_view.borrow().clone();
        let held = held_for_view.clone();
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-header-all-disallow-empty")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(selected)
            .disallow_empty_selection(true)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .on_selection_change(move |keys, window, _| {
                *held.borrow_mut() = keys.to_vec();
                recorded.borrow_mut().push(keys.len().to_string());
                window.refresh();
            })
            .into_any_element()
    });

    flush_frame(cx);
    click(cx, 22., 18.);
    assert!(
        recorded.borrow().is_empty(),
        "select-all must not clear every key when empty selection is disallowed"
    );
}

/// Pinned `useSelectableCollection` registers Home and End per platform:
/// Windows and Linux install none, Shift, Control, and Control+Shift, and
/// only Control+Shift extends; macOS installs none, Shift, Alt, and
/// Alt+Shift only, so Shift and Alt+Shift move the focus alone and every
/// Control- or Meta-bearing chord is entirely inert. Each branch drives its
/// own host's real chords; the cfg-free unit truth tables in the components
/// prove both maps everywhere.
#[gpui::test]
fn table_home_end_extend_only_from_the_registered_chord(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-shift-home-end")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .keyed_row("gamma", vec![gpui::div().child("Gamma").into_any_element()])
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

    press(cx, "tab down space");
    flush_frame(cx);
    press(cx, "shift-end");
    assert_eq!(recorded.borrow().as_slice(), ["alpha"]);
    press(cx, "space");
    flush_frame(cx);
    if cfg!(target_os = "macos") {
        // Control-bearing chords sit outside the macOS registration: the
        // event is entirely inert, not even the focus moves.
        press(cx, "ctrl-shift-home");
        flush_frame(cx);
        assert_eq!(
            recorded.borrow().as_slice(),
            ["alpha", "alpha,gamma"],
            "macOS must leave Control-bearing Home/End entirely inert"
        );
        // Alt+Shift *is* registered on macOS: the focus walks, extends nothing.
        press(cx, "alt-shift-home");
        flush_frame(cx);
        assert_eq!(
            recorded.borrow().as_slice(),
            ["alpha", "alpha,gamma"],
            "macOS Alt+Shift+Home must move the focus without extending"
        );
    } else {
        press(cx, "ctrl-shift-home");
        assert_eq!(
            recorded.borrow().as_slice(),
            ["alpha", "alpha,gamma", "alpha,beta,gamma"],
            "plain Shift+End must only move focus, while Control+Shift+Home extends"
        );
        // The reverse chord extends the same way, and `extendSelection`
        // replaces the anchor..target range, so extending back to the
        // anchor shrinks the selection to it again.
        press(cx, "ctrl-shift-end");
        assert_eq!(
            recorded.borrow().as_slice(),
            ["alpha", "alpha,gamma", "alpha,beta,gamma", "gamma"],
            "Control+Shift+End must extend back across the replaced range"
        );
        // An Alt-bearing chord is outside the Windows/Linux registration,
        // so it cannot move the focus or extend again.
        press(cx, "alt-shift-end");
        flush_frame(cx);
        assert_eq!(
            recorded.borrow().as_slice(),
            ["alpha", "alpha,gamma", "alpha,beta,gamma", "gamma"],
            "an unregistered Alt-bearing chord must leave Home and End inert"
        );
    }
}

/// The whole-event registration guard keeps an unregistered Home/End chord
/// from moving the keyboard cursor, not only from reporting a selection:
/// Enter after the chord must still activate the row the cursor held, not
/// the row Home would have walked to.
#[gpui::test]
fn table_unregistered_home_end_leave_the_cursor_in_place(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-inert-home-end")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::None)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .keyed_row("gamma", vec![gpui::div().child("Gamma").into_any_element()])
            .on_row_click(move |index, _, _, _| {
                recorded.borrow_mut().push(format!("row:{index}"));
            })
            .into_any_element()
    });

    // Focus entry then two Downs seat the cursor on the second row; the
    // arrow keys report nothing in this mode.
    press(cx, "tab down down");
    flush_frame(cx);

    // Each host's own unregistered chord: Control-bearing on macOS, Alt-
    // bearing on Windows and Linux. Removing the whole-event guard would
    // walk Home to the first row, which is exactly what the Enter target
    // below exposes.
    if cfg!(target_os = "macos") {
        press(cx, "ctrl-shift-home");
    } else {
        press(cx, "alt-shift-home");
    }
    flush_frame(cx);
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["row:1"],
        "an unregistered Home/End chord must not move the keyboard cursor"
    );
}

/// Pinned row presses route Shift+Click through `extendSelection`, preserving
/// the anchor established by the prior toggle.
#[gpui::test]
fn table_shift_click_extends_from_the_selection_anchor(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let held_for_view = held;
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = held_for_view.borrow().clone();
        let held = held_for_view.clone();
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-shift-click")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(selected)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .keyed_row("gamma", vec![gpui::div().child("Gamma").into_any_element()])
            .on_selection_change(move |keys, window, _| {
                *held.borrow_mut() = keys.to_vec();
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                window.refresh();
            })
            .into_any_element()
    });

    press(cx, "tab down space");
    flush_frame(cx);
    let mut modifiers = gpui::Modifiers::none();
    modifiers.shift = true;
    cx.simulate_click(gpui::point(px(100.), px(100.)), modifiers);
    flush_frame(cx);
    cx.simulate_click(gpui::point(px(100.), px(100.)), modifiers);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta"],
        "Shift+Click must extend rather than re-anchor and toggle"
    );
}

/// Focus entry in pinned `useSelectableCollection` seats the cursor on the
/// first row before the first arrow, so Shift+Down targets the second row.
#[gpui::test]
fn table_first_shift_down_starts_after_the_focused_first_row(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-first-shift-down")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .keyed_row("gamma", vec![gpui::div().child("Gamma").into_any_element()])
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

    press(cx, "tab shift-down");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["beta"],
        "the first Shift+Down must move below the row focused on entry"
    );
}

/// React Stately's raw `all` selection is idempotent even when no enabled key
/// materializes into the port's selection slice.
#[gpui::test]
fn table_mod_a_is_idempotent_when_every_row_is_disabled(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-all-disabled")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .disabled_keys([SharedString::from("alpha"), SharedString::from("beta")])
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
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
    press_mod_a(cx);
    flush_frame(cx);
    press_mod_a(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        [""],
        "repeated Mod+A over an empty selectable collection must report once"
    );
}

/// Fresh focus starts on the first row, but End still moves to the last row;
/// Shift alone changes focus without extending selection.
#[gpui::test]
fn table_first_shift_end_settles_on_the_last_row(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-first-shift-end")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .keyed_row("gamma", vec![gpui::div().child("Gamma").into_any_element()])
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

    press(cx, "tab shift-end");
    assert!(recorded.borrow().is_empty());
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["gamma"],
        "Shift+End must focus the last row without selecting on its own"
    );
}

/// A controlled owner can replace a selection after Mod+A; the raw-all latch
/// must then yield to the new prop value rather than swallowing the next Mod+A.
#[gpui::test]
fn table_controlled_replacement_clears_a_stale_mod_a_latch(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let held_for_view = held.clone();
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = held_for_view.borrow().clone();
        let held = held_for_view.clone();
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("table-controlled-all-replacement")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(selected)
            .keyed_row("alpha", vec![gpui::div().child("Alpha").into_any_element()])
            .keyed_row("beta", vec![gpui::div().child("Beta").into_any_element()])
            .on_selection_change(move |keys, window, _| {
                *held.borrow_mut() = keys.to_vec();
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                window.refresh();
            })
            .into_any_element()
    });

    press(cx, "tab");
    press_mod_a(cx);
    flush_frame(cx);
    *held.borrow_mut() = vec![SharedString::from("alpha")];
    flush_frame(cx);
    press_mod_a(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,beta", "alpha,beta"],
        "an owner replacement must make Mod+A selectable again"
    );
}

#[gpui::test]
fn table_header_and_cells_keep_pinned_line_heights(cx: &mut TestAppContext) {
    for leading in [None, Some(48.)] {
        for virtual_rows in [false, true] {
            let cx = open_host(cx, move || {
                let content = || {
                    gpui::div()
                        .debug_selector(|| "table-leading-cell".into())
                        .child("First\nSecond")
                        .into_any_element()
                };
                let table = Table::new(vec![])
                    .id("table-leading")
                    .column(TableColumn::new("First\nSecond").default_width(px(240.)));
                let table = if virtual_rows {
                    table
                        .virtual_rows(
                            1,
                            "leading-rows",
                            |_| "row".into(),
                            move |_| TableRow::new(vec![content()]),
                        )
                        .row_height(px(64.))
                        .max_h(px(128.))
                } else {
                    table.row(vec![content()])
                };
                gpui::div()
                    .w(px(300.))
                    .when_some(leading, |el, leading| el.line_height(px(leading)))
                    .child(table)
                    .into_any_element()
            });
            let bounds = cx
                .debug_bounds("table-leading-cell")
                .expect("cell text paints");
            assert_eq!(
                bounds.size.height,
                px(40.),
                "two 20px cell lines; virtual={virtual_rows}, host={leading:?}"
            );
            assert_eq!(bounds.origin.y, px(65.), "two 16px header lines, 20px header padding, 1px separator and 12px cell padding; virtual={virtual_rows}, host={leading:?}");
        }
    }
}

/// Vanilla GPUI clips `overflow_hidden()` to the rectangle, so the edge rows'
/// cell fills carry the box's rounded corners themselves. The corner radii are
/// not readable from the paint harness, so the decision itself is unit tested
/// next to `edge_corner_rounding` in `table.rs`; these guard the render paths
/// that consume it -- a secondary table with and without a footer, and a
/// primary table with a caller-supplied hover fill -- still stacking their
/// rows flush, with the outer cells keeping the shared column track.
fn rounding_rows(table: Table) -> Table {
    table
        .row(vec![
            gpui::div().child("first").into_any_element(),
            gpui::div().child("one").into_any_element(),
        ])
        .row(vec![
            gpui::div().child("middle").into_any_element(),
            gpui::div().child("two").into_any_element(),
        ])
        .row(vec![
            gpui::div().child("last").into_any_element(),
            gpui::div().child("three").into_any_element(),
        ])
}

fn assert_rounding_rows_stack(cx: &mut VisualTestContext) {
    flush_frame(cx);
    for column in [0, 1] {
        let selectors: [&'static str; 3] = match column {
            0 => [
                "table-row-track-0-0",
                "table-row-track-1-0",
                "table-row-track-2-0",
            ],
            _ => [
                "table-row-track-0-1",
                "table-row-track-1-1",
                "table-row-track-2-1",
            ],
        };
        let [first, middle, last] = selectors.map(|selector| {
            cx.debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} paints"))
        });
        assert_eq!(first.origin.x, last.origin.x);
        assert_eq!(first.size.width, last.size.width);
        assert!(first.bottom() <= middle.top() + px(0.5));
        assert!(middle.bottom() <= last.top() + px(0.5));
    }
}

#[gpui::test]
fn secondary_table_rows_stack_when_the_last_row_rounds_the_wrapper(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(640.))
            .child(rounding_rows(
                Table::new(vec!["Name".into(), "Value".into()])
                    .id("table-round-secondary")
                    .variant(herogpui_components::TableVariant::Secondary),
            ))
            .into_any_element()
    });
    assert_rounding_rows_stack(cx);
}

#[gpui::test]
fn secondary_table_with_a_footer_keeps_its_rows_square(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(640.))
            .child(
                rounding_rows(
                    Table::new(vec!["Name".into(), "Value".into()])
                        .id("table-round-secondary-footer")
                        .variant(herogpui_components::TableVariant::Secondary),
                )
                .footer(gpui::div().child("total")),
            )
            .into_any_element()
    });
    assert_rounding_rows_stack(cx);
}

#[gpui::test]
fn primary_table_rows_stack_when_the_edge_rows_round_the_body(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(640.))
            .child(rounding_rows(
                Table::new(vec!["Name".into(), "Value".into()])
                    .id("table-round-primary")
                    .row_hover_bg(gpui::rgb(0x336699)),
            ))
            .into_any_element()
    });
    assert_rounding_rows_stack(cx);
}
