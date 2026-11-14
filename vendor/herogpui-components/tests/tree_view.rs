//! `TreeView` (HeroGPUI extension): expandable rows with chevron and
//! Right/Left expansion, Up/Down/Home/End over the visible rows, Right to the
//! first child and Left to the parent, typeahead, single and multiple
//! selection, and disabled rows that the cursor skips.

use crate::harness;

use std::collections::HashSet;

use gpui::{
    prelude::*, px, Context, Modifiers, Render, SharedString, TestAppContext, VisualTestContext,
    Window,
};
use harness::{events, open_host, press, Events};
use herogpui_components::{SelectionMode, TreeItem, TreeView};

#[derive(Clone, Copy)]
struct Config {
    mode: SelectionMode,
    open_src: bool,
    controlled_expansion: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: SelectionMode::Single,
            open_src: false,
            controlled_expansion: false,
        }
    }
}

fn items() -> Vec<TreeItem> {
    vec![
        TreeItem::new("docs", "Docs").children(vec![
            TreeItem::new("guide", "Guide"),
            TreeItem::new("api", "API").child(TreeItem::new("v1", "Version 1")),
        ]),
        TreeItem::new("src", "Sources").children(vec![
            TreeItem::new("lib", "lib.rs"),
            TreeItem::new("main", "main.rs").is_disabled(true),
        ]),
        TreeItem::new("readme", "Readme"),
        TreeItem::new("secret", "Secret"),
    ]
}

fn sorted(keys: &HashSet<SharedString>) -> String {
    let mut keys: Vec<&str> = keys.iter().map(|k| k.as_ref()).collect();
    keys.sort_unstable();
    keys.join(",")
}

fn host(cx: &mut TestAppContext, config: Config) -> (Events, &mut VisualTestContext) {
    let seen = events();
    let s = seen.clone();
    let cx = open_host(cx, move || {
        let (sel, exp) = (s.clone(), s.clone());
        let mut tree = TreeView::new("tree", items())
            .selection_mode(config.mode)
            .disabled_keys(["secret".into()])
            .on_selection_change(move |keys, _, _| {
                sel.borrow_mut().push(format!("sel:{}", sorted(keys)));
            })
            .on_expanded_change(move |keys, _, _| {
                exp.borrow_mut().push(format!("exp:{}", sorted(keys)));
            });
        if config.open_src {
            tree = tree.default_expanded_keys(["src".into()]);
        }
        if config.controlled_expansion {
            tree = tree.expanded_keys(Vec::<SharedString>::new());
        }
        gpui::div().w(px(300.)).child(tree).into_any_element()
    });
    (seen, cx)
}

fn frame(cx: &mut VisualTestContext) {
    for _ in 0..2 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn row(cx: &mut VisualTestContext, key: &str) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.debug_bounds(Box::leak(format!("tree-row-{key}").into_boxed_str()))
}

fn click(cx: &mut VisualTestContext, name: &str) {
    let at = cx
        .debug_bounds(Box::leak(name.to_owned().into_boxed_str()))
        .unwrap_or_else(|| panic!("{name} laid out"))
        .center();
    cx.simulate_click(at, Modifiers::none());
    frame(cx);
}

fn keys(cx: &mut VisualTestContext, list: &[&str]) {
    for key in list {
        press(cx, key);
        frame(cx);
    }
}

fn log(seen: &Events) -> Vec<String> {
    seen.borrow().clone()
}

#[gpui::test]
fn the_chevron_expands_and_collapses_without_selecting(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    assert!(row(cx, "guide").is_none(), "children start hidden");
    assert!(
        cx.debug_bounds("tree-toggle-readme").is_none(),
        "a leaf has no chevron"
    );
    click(cx, "tree-toggle-docs");
    let guide = row(cx, "guide").expect("expanded");
    let docs = row(cx, "docs").unwrap();
    assert!(guide.origin.y > docs.origin.y, "the child renders below");
    assert!(
        row(cx, "src").unwrap().origin.y > row(cx, "api").unwrap().origin.y,
        "siblings move down"
    );
    click(cx, "tree-toggle-docs");
    assert!(row(cx, "guide").is_none());
    assert_eq!(log(&seen), ["exp:docs", "exp:"]);
}

#[gpui::test]
fn right_and_left_walk_the_hierarchy(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    // Tab enters on the first row; Right expands Docs, then moves to Guide;
    // Down reaches API, Right expands it and moves to Version 1; Left goes
    // back to API, collapses it, then goes up to Docs, which Enter selects.
    keys(
        cx,
        &[
            "tab", "right", "right", "down", "right", "right", "left", "left", "left", "enter",
        ],
    );
    assert_eq!(
        log(&seen),
        ["exp:docs", "exp:api,docs", "exp:docs", "sel:docs"]
    );
    // Left on the expanded Docs collapses it, a second Left on the collapsed
    // root does nothing, and Space toggles the selection off.
    keys(cx, &["left", "left", "space"]);
    assert_eq!(log(&seen)[4..], ["exp:", "sel:"]);
}

#[gpui::test]
fn up_down_home_end_skip_disabled_rows(cx: &mut TestAppContext) {
    let config = Config {
        open_src: true,
        ..Config::default()
    };
    let (seen, cx) = host(cx, config);
    frame(cx);
    // Rows: docs, src, lib, main (disabled), readme, secret (disabled).
    keys(cx, &["tab", "end", "enter"]);
    keys(cx, &["home", "down", "down", "enter"]);
    keys(cx, &["down", "enter"]);
    keys(cx, &["up", "up", "enter"]);
    // Down at the last stop holds rather than wrapping.
    keys(cx, &["end", "down", "enter"]);
    assert_eq!(
        log(&seen),
        [
            "sel:readme",
            "sel:lib",
            "sel:readme",
            "sel:src",
            "sel:readme"
        ]
    );
}

#[gpui::test]
fn a_press_selects_and_escape_clears(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    click(cx, "tree-row-readme");
    click(cx, "tree-row-src");
    click(cx, "tree-row-secret");
    keys(cx, &["escape"]);
    assert_eq!(log(&seen), ["sel:readme", "sel:src", "sel:"]);
}

#[gpui::test]
fn multiple_selection_toggles_each_row(cx: &mut TestAppContext) {
    let config = Config {
        mode: SelectionMode::Multiple,
        ..Config::default()
    };
    let (seen, cx) = host(cx, config);
    frame(cx);
    click(cx, "tree-row-readme");
    click(cx, "tree-row-docs");
    click(cx, "tree-row-readme");
    assert_eq!(log(&seen), ["sel:readme", "sel:docs,readme", "sel:docs"]);
}

#[gpui::test]
fn without_selection_a_press_or_enter_toggles_a_parent(cx: &mut TestAppContext) {
    let config = Config {
        mode: SelectionMode::None,
        ..Config::default()
    };
    let (seen, cx) = host(cx, config);
    frame(cx);
    click(cx, "tree-row-docs");
    assert!(row(cx, "guide").is_some());
    keys(cx, &["enter"]);
    assert!(row(cx, "guide").is_none());
    click(cx, "tree-row-readme");
    assert_eq!(log(&seen), ["exp:docs", "exp:"]);
}

#[gpui::test]
fn typeahead_moves_to_the_matching_row(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    keys(cx, &["tab", "r", "enter"]);
    assert_eq!(log(&seen), ["sel:readme"]);
}

#[gpui::test]
fn typeahead_extends_its_buffer_and_skips_disabled_rows(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    // "s" reaches Sources; "se" matches only the disabled Secret, so the
    // cursor stays where it is.
    keys(cx, &["tab", "s", "e", "enter"]);
    assert_eq!(log(&seen), ["sel:src"]);
}

#[gpui::test]
fn controlled_expansion_reports_without_expanding(cx: &mut TestAppContext) {
    let config = Config {
        controlled_expansion: true,
        ..Config::default()
    };
    let (seen, cx) = host(cx, config);
    frame(cx);
    click(cx, "tree-toggle-docs");
    keys(cx, &["right"]);
    assert!(row(cx, "guide").is_none());
    assert_eq!(log(&seen), ["exp:docs", "exp:docs"]);
}

#[gpui::test]
fn collapsing_an_ancestor_moves_the_cursor_to_the_nearest_visible_one(cx: &mut TestAppContext) {
    let open = std::rc::Rc::new(std::cell::RefCell::new(vec![
        SharedString::from("docs"),
        SharedString::from("api"),
    ]));
    let o = open.clone();
    let seen = events();
    let s = seen.clone();
    let cx = open_host(cx, move || {
        let sel = s.clone();
        gpui::div()
            .w(px(300.))
            .child(
                TreeView::new("tree", items())
                    .selection_mode(SelectionMode::Single)
                    .expanded_keys(o.borrow().clone())
                    .on_selection_change(move |keys, _, _| {
                        sel.borrow_mut().push(format!("sel:{}", sorted(keys)));
                    }),
            )
            .into_any_element()
    });
    frame(cx);
    // Rows: docs, guide, api, v1, ... -- the cursor walks down to Version 1.
    keys(cx, &["tab", "down", "down", "down"]);
    // The caller collapses API (its parent), not Docs.
    open.borrow_mut().retain(|key| key.as_ref() != "api");
    frame(cx);
    keys(cx, &["enter"]);
    assert_eq!(log(&seen), ["sel:api"]);
}

/// A bare tree with no `app_focus_root` around it: that root clears the
/// ring on every pointer press, which would hide whether the row does.
struct BareTree;

impl Render for BareTree {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        gpui::div()
            .w(px(300.))
            .child(TreeView::new("tree", items()).selection_mode(SelectionMode::Single))
    }
}

#[gpui::test]
fn a_row_press_after_keyboard_use_hides_the_focus_ring(cx: &mut TestAppContext) {
    cx.update(herogpui_theme::ThemeProvider::init);
    let (_view, cx) = cx.add_window_view(|_, _| BareTree);
    frame(cx);
    let visible = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| herogpui_components::extend::focus_visible(cx))
    };
    // As the chevron does, a row press leaves keyboard modality.
    cx.update(|_, cx| herogpui_components::extend::set_focus_visible(true, cx));
    click(cx, "tree-toggle-docs");
    assert!(!visible(cx), "the chevron press hides the ring");
    cx.update(|_, cx| herogpui_components::extend::set_focus_visible(true, cx));
    click(cx, "tree-row-readme");
    assert!(!visible(cx), "a row press hides the ring");
}

fn shift_click(cx: &mut VisualTestContext, name: &str) {
    let at = cx
        .debug_bounds(Box::leak(name.to_owned().into_boxed_str()))
        .unwrap_or_else(|| panic!("{name} laid out"))
        .center();
    cx.simulate_click(
        at,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    frame(cx);
}

fn multiple_open_src() -> Config {
    Config {
        mode: SelectionMode::Multiple,
        open_src: true,
        ..Config::default()
    }
}

#[gpui::test]
fn shift_down_extends_a_multiple_selection_from_the_anchor(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, multiple_open_src());
    frame(cx);
    // Rows: docs, src, lib, main (disabled), readme, secret (disabled).
    // Space on Docs seats the anchor; Shift+Down twice grows the range over
    // Sources and lib; Shift+Up shrinks it back to Sources. Shift+Down past
    // the disabled main.rs reaches Readme and skips main.rs in the range.
    keys(
        cx,
        &[
            "tab",
            "space",
            "shift-down",
            "shift-down",
            "shift-up",
            "shift-down",
            "shift-down",
        ],
    );
    assert_eq!(
        log(&seen),
        [
            "sel:docs",
            "sel:docs,src",
            "sel:docs,lib,src",
            "sel:docs,src",
            "sel:docs,lib,src",
            "sel:docs,lib,readme,src",
        ]
    );
    // Shift+Home only moves the cursor; Space then toggles Docs off.
    keys(cx, &["shift-home", "space"]);
    assert_eq!(log(&seen)[6..], ["sel:lib,readme,src"]);
}

#[gpui::test]
fn shift_click_selects_the_visible_range(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, multiple_open_src());
    frame(cx);
    click(cx, "tree-row-src");
    shift_click(cx, "tree-row-readme");
    // A second Shift press replaces the range from the same anchor.
    shift_click(cx, "tree-row-docs");
    assert_eq!(
        log(&seen),
        ["sel:src", "sel:lib,readme,src", "sel:docs,src"]
    );
}

#[gpui::test]
fn shift_does_not_extend_a_single_selection(cx: &mut TestAppContext) {
    let config = Config {
        open_src: true,
        ..Config::default()
    };
    let (seen, cx) = host(cx, config);
    frame(cx);
    click(cx, "tree-row-src");
    shift_click(cx, "tree-row-readme");
    keys(cx, &["shift-up"]);
    assert_eq!(log(&seen), ["sel:src", "sel:readme"]);
}

/// A flat tree of `n` rows, all top-level, so the visible count is `n`.
fn wide_items(n: usize) -> Vec<TreeItem> {
    (0..n)
        .map(|i| TreeItem::new(format!("n{i}"), format!("Node {i}")))
        .collect()
}

#[gpui::test]
fn a_capped_tree_builds_only_the_rows_in_view(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(300.))
            .child(
                TreeView::new("tree", wide_items(5_000))
                    .selection_mode(SelectionMode::Single)
                    .max_h(px(200.)),
            )
            .into_any_element()
    });
    frame(cx);
    let tree = cx.debug_bounds("tree-tree").expect("tree laid out");
    assert_eq!(tree.size.height, px(200.), "capped at max_h");
    assert!(row(cx, "n0").is_some());
    assert!(row(cx, "n5").is_some(), "rows in view are built");
    assert!(row(cx, "n10").is_none(), "rows below the cap are not");
    assert!(row(cx, "n4999").is_none());
    // End moves the cursor to the last row and scrolls it into view.
    keys(cx, &["tab", "end"]);
    let last = row(cx, "n4999").expect("the last row scrolled into view");
    assert!(last.bottom() <= tree.bottom(), "{last:?} inside {tree:?}");
    assert!(row(cx, "n0").is_none(), "the first row scrolled out");
}

#[gpui::test]
fn an_uncapped_tree_keeps_its_flex_column_geometry(cx: &mut TestAppContext) {
    let (_, cx) = host(cx, Config::default());
    frame(cx);
    // Four top-level rows: 4px padding, 32px rows 2px apart, 4px padding.
    let tree = cx.debug_bounds("tree-tree").expect("tree laid out");
    assert_eq!(tree.size.height, px(4. + 4. * 32. + 3. * 2. + 4.));
    let docs = row(cx, "docs").unwrap();
    let src = row(cx, "src").unwrap();
    assert_eq!(docs.origin.y, tree.origin.y + px(4.));
    assert_eq!(src.origin.y, docs.origin.y + px(34.));
    assert_eq!(docs.origin.x, tree.origin.x + px(4.));
}
