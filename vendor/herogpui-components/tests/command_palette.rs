//! `CommandPalette` (HeroGPUI extension): the search field takes the focus,
//! typing filters and re-highlights the first match, Up/Down wrap over the
//! enabled commands, Enter and a press run one, Escape and an outside press
//! close and return the focus, and a query with no match shows the empty
//! state.

use crate::harness;

use std::{cell::Cell, rc::Rc};

use gpui::{prelude::*, px, Modifiers, TestAppContext, VisualTestContext};
use harness::{events, open_host, press, still, Events};
use herogpui_components::{Button, CommandItem, CommandPalette};

fn items() -> Vec<CommandItem> {
    vec![
        CommandItem::new("new", "New file")
            .group("File")
            .shortcut(["⌘", "N"]),
        CommandItem::new("open", "Open file").group("File"),
        CommandItem::new("save", "Save")
            .group("File")
            .is_disabled(true),
        CommandItem::new("close", "Close window").group("Window"),
        CommandItem::new("theme", "Toggle theme").keywords(["dark", "light"]),
    ]
}

struct Host {
    seen: Events,
    open: Rc<Cell<bool>>,
}

fn host(cx: &mut TestAppContext) -> (Host, &mut VisualTestContext) {
    still();
    let seen = events();
    let open = Rc::new(Cell::new(false));
    let (s, o) = (seen.clone(), open.clone());
    let cx = open_host(cx, move || {
        let (sel, change, flag) = (s.clone(), s.clone(), o.clone());
        let press_flag = o.clone();
        gpui::div()
            .size_full()
            .child(
                gpui::div().debug_selector(|| "opener".into()).child(
                    Button::new("opener")
                        .label("Open palette")
                        .on_press(move |_, _, _| press_flag.set(true)),
                ),
            )
            .child(
                CommandPalette::new("palette", items())
                    .is_open(o.get())
                    .on_select(move |key, _, _| sel.borrow_mut().push(format!("select:{key}")))
                    .on_open_change(move |open, _, _| {
                        flag.set(*open);
                        change.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .into_any_element()
    });
    (Host { seen, open }, cx)
}

fn frame(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn open_palette(host: &Host, cx: &mut VisualTestContext) {
    frame(cx);
    // Focus the opener first so the palette has a focus to hand back.
    let at = cx.debug_bounds("opener").expect("opener").center();
    cx.simulate_click(at, Modifiers::none());
    frame(cx);
    assert!(host.open.get(), "the opener opened the palette");
}

fn shown(cx: &mut VisualTestContext, key: &str) -> bool {
    cx.debug_bounds(Box::leak(format!("palette-item-{key}").into_boxed_str()))
        .is_some()
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

#[gpui::test]
fn opening_focuses_the_search_and_lists_every_command(cx: &mut TestAppContext) {
    let (host, cx) = host(cx);
    assert!(
        cx.debug_bounds("palette-panel").is_none(),
        "closed at first"
    );
    open_palette(&host, cx);
    assert!(cx.debug_bounds("palette-panel").is_some());
    for key in ["new", "open", "save", "close", "theme"] {
        assert!(shown(cx, key), "{key} is listed");
    }
    // The first enabled command is highlighted: Enter runs it.
    keys(cx, "enter");
    assert_eq!(log(&host.seen), ["open:false", "select:new"]);
}

#[gpui::test]
fn arrows_wrap_over_the_enabled_commands(cx: &mut TestAppContext) {
    let (host, cx) = host(cx);
    open_palette(&host, cx);
    // new → open → (save is disabled) close → theme → wraps to new → up to theme.
    keys(cx, "down down down down up enter");
    assert_eq!(log(&host.seen), ["open:false", "select:theme"]);
}

#[gpui::test]
fn typing_filters_and_highlights_the_first_match(cx: &mut TestAppContext) {
    let (host, cx) = host(cx);
    open_palette(&host, cx);
    keys(cx, "down");
    cx.simulate_input("file");
    frame(cx);
    assert!(shown(cx, "new") && shown(cx, "open"));
    assert!(!shown(cx, "close"), "Close window does not match");
    assert!(!shown(cx, "theme"));
    keys(cx, "enter");
    assert_eq!(
        log(&host.seen),
        ["open:false", "select:new"],
        "a new query re-highlights the first match"
    );
}

#[gpui::test]
fn keywords_and_every_word_of_the_query_are_searched(cx: &mut TestAppContext) {
    let (host, cx) = host(cx);
    open_palette(&host, cx);
    cx.simulate_input("dark tog");
    frame(cx);
    assert!(shown(cx, "theme"));
    assert!(!shown(cx, "new"));
    keys(cx, "enter");
    assert_eq!(log(&host.seen), ["open:false", "select:theme"]);
}

#[gpui::test]
fn no_match_shows_the_empty_state_and_enter_runs_nothing(cx: &mut TestAppContext) {
    let (host, cx) = host(cx);
    open_palette(&host, cx);
    cx.simulate_input("zzz");
    frame(cx);
    assert!(cx.debug_bounds("palette-empty").is_some());
    keys(cx, "enter");
    assert!(log(&host.seen).is_empty());
}

#[gpui::test]
fn a_press_runs_a_command_and_a_disabled_one_does_nothing(cx: &mut TestAppContext) {
    let (host, cx) = host(cx);
    open_palette(&host, cx);
    let save = cx.debug_bounds("palette-item-save").unwrap().center();
    cx.simulate_click(save, Modifiers::none());
    frame(cx);
    assert!(
        log(&host.seen).is_empty(),
        "a disabled command does not run"
    );
    let close = cx.debug_bounds("palette-item-close").unwrap().center();
    cx.simulate_click(close, Modifiers::none());
    frame(cx);
    assert_eq!(log(&host.seen), ["open:false", "select:close"]);
}

#[gpui::test]
fn escape_closes_and_returns_the_focus(cx: &mut TestAppContext) {
    let (host, cx) = host(cx);
    frame(cx);
    // Open from the keyboard, so the opener holds the focus the palette takes.
    keys(cx, "tab enter");
    assert!(host.open.get());
    frame(cx);
    keys(cx, "escape");
    assert_eq!(log(&host.seen), ["open:false"]);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    frame(cx);
    assert!(cx.debug_bounds("palette-panel").is_none());
    // The opener got the focus back: Enter on it opens the palette again.
    keys(cx, "enter");
    frame(cx);
    assert!(host.open.get(), "the focus went back to the opener");
    // A reopened palette starts from an empty search.
    for key in ["new", "open", "close", "theme"] {
        assert!(shown(cx, key));
    }
}

#[gpui::test]
fn a_press_outside_the_panel_closes_it(cx: &mut TestAppContext) {
    let (host, cx) = host(cx);
    open_palette(&host, cx);
    let panel = cx.debug_bounds("palette-panel").unwrap();
    let below = gpui::point(panel.center().x, panel.bottom() + px(100.));
    cx.simulate_click(below, Modifiers::none());
    frame(cx);
    assert_eq!(log(&host.seen), ["open:false"]);
}
