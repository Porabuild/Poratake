//! Explicit overlay-stack coverage for the three picker surfaces.
//!
//! Each picker is mounted inside an open Popover.  Escape must dismiss the
//! focused picker exactly once; Select additionally verifies that a later
//! outside press reaches the parent. The callbacks are recorded rather than
//! inferred from pixels so a duplicate close report cannot hide behind an
//! unchanged frame.

use crate::harness;

use gpui::{prelude::*, TestAppContext, VisualTestContext};
use harness::{click, events, open_host, press};
use herogpui_components::{
    Autocomplete, Button, ComboBox, InputState, PickerItem, Popover, Select,
};

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn child_point(cx: &mut VisualTestContext) {
    // Parent trigger: 36px tall. Its panel starts 8px below it, has 16px top
    // padding, and the child trigger is the first panel child.
    click(cx, 80., 78.);
}

fn autocomplete_search_point(cx: &mut VisualTestContext) {
    // Autocomplete focuses its SearchField inside the child popover, below
    // the trigger rather than on the trigger itself.
    click(cx, 80., 132.);
}

fn outside_parent(cx: &mut VisualTestContext) {
    click(cx, 300., 78.);
}

/// Items whose labels are unique, so the key can be the label itself. Tests
/// that need duplicate labels build explicit keys instead.
fn keyed(labels: &[&str]) -> Vec<PickerItem> {
    labels
        .iter()
        .map(|l| PickerItem::new(l.to_string(), l.to_string()))
        .collect()
}

#[gpui::test]
fn nested_select_escape_closes_child_then_parent_outside(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let state = events.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        let state = state.clone();
        Popover::new(Button::new("picker-parent-trigger").label("Parent"))
            .id("picker-parent-select")
            .default_open(true)
            .show_close_button(false)
            .on_open_change(move |open, _, _| {
                recorded.borrow_mut().push(format!("parent:{open}"));
            })
            .child(
                Select::new(
                    "nested-select",
                    vec![
                        PickerItem::new("alpha", "Alpha"),
                        PickerItem::new("beta", "Beta"),
                    ],
                )
                .default_open(true)
                .on_open_change(move |open, _, _| {
                    state.borrow_mut().push(format!("select:{open}"));
                }),
            )
            .into_any_element()
    });

    press(cx, "tab");
    flush_frame(cx);
    press(cx, "escape");
    assert_eq!(
        events.borrow().as_slice(),
        ["select:false"],
        "the focused Select must consume the first Escape"
    );

    flush_frame(cx);
    outside_parent(cx);
    assert_eq!(
        events.borrow().as_slice(),
        ["select:false", "parent:false"],
        "after the child closes, an outside press must reach the parent"
    );
}

#[gpui::test]
fn nested_combo_box_escape_closes_child_once_inside_parent(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let parent_events = recorded.clone();
        let child_events = recorded.clone();
        Popover::new(Button::new("picker-parent-trigger").label("Parent"))
            .id("picker-parent-combo")
            .default_open(true)
            .show_close_button(false)
            .on_open_change(move |open, _, _| {
                parent_events.borrow_mut().push(format!("parent:{open}"));
            })
            .child(
                ComboBox::new(state.clone(), keyed(&["Alpha", "Beta"]))
                    .default_open(true)
                    .on_open_change(move |open, _, _| {
                        child_events.borrow_mut().push(format!("combo:{open}"));
                    }),
            )
            .into_any_element()
    });

    child_point(cx);
    flush_frame(cx);
    assert!(
        events.borrow().is_empty(),
        "pressing the ComboBox input must not count as an outside dismissal"
    );
    press(cx, "escape");
    assert_eq!(
        events.borrow().as_slice(),
        ["combo:false"],
        "the focused ComboBox must consume the first Escape"
    );

    assert_eq!(
        events.borrow().as_slice(),
        ["combo:false"],
        "the nested ComboBox must report its close exactly once"
    );
}

#[gpui::test]
fn nested_autocomplete_escape_closes_child_once_inside_parent(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let child_events = recorded.clone();
        Popover::new(Button::new("picker-parent-trigger").label("Parent"))
            .id("picker-parent-autocomplete")
            .default_open(true)
            .show_close_button(false)
            .child(
                Autocomplete::new(state.clone(), keyed(&["Alpha", "Beta"]))
                    .default_open(true)
                    .on_open_change(move |open, _, _| {
                        child_events
                            .borrow_mut()
                            .push(format!("autocomplete:{open}"));
                    }),
            )
            .into_any_element()
    });

    autocomplete_search_point(cx);
    flush_frame(cx);
    press(cx, "escape");
    assert_eq!(
        events.borrow().as_slice(),
        ["autocomplete:false"],
        "the focused Autocomplete must consume the first Escape"
    );

    assert_eq!(
        events.borrow().as_slice(),
        ["autocomplete:false"],
        "the nested Autocomplete must report its close exactly once"
    );
}

#[gpui::test]
fn select_trigger_latch_is_one_mouse_down_without_parent_repaint(cx: &mut TestAppContext) {
    let opens = events();
    let recorded = opens.clone();
    let cx = open_host(cx, move || {
        let opens = recorded.clone();
        Select::new("latch-select", vec![PickerItem::new("alpha", "Alpha")])
            .is_open(true)
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .into_any_element()
    });

    flush_frame(cx);
    click(cx, 60., 18.);
    assert_eq!(opens.borrow().as_slice(), ["open:false"]);

    // The controlled callback intentionally does not update state or refresh the
    // parent. The next press must still reach the open panel's outside listener.
    click(cx, 600., 300.);
    assert_eq!(
        opens.borrow().as_slice(),
        ["open:false", "open:false"],
        "a trigger latch must not suppress a later outside press"
    );
}

#[gpui::test]
fn combo_box_trigger_latch_is_one_mouse_down_without_parent_repaint(cx: &mut TestAppContext) {
    let opens = events();
    let recorded = opens.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let opens = recorded.clone();
        ComboBox::new(state.clone(), keyed(&["Alpha"]))
            .is_open(true)
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .into_any_element()
    });

    flush_frame(cx);
    click(cx, 298., 18.);
    assert_eq!(opens.borrow().as_slice(), ["open:false"]);
    click(cx, 600., 300.);
    assert_eq!(
        opens.borrow().as_slice(),
        ["open:false", "open:false"],
        "a trigger latch must not suppress a later outside press"
    );
}

#[gpui::test]
fn autocomplete_trigger_latch_is_one_mouse_down_without_parent_repaint(cx: &mut TestAppContext) {
    let opens = events();
    let recorded = opens.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let opens = recorded.clone();
        Autocomplete::new(state.clone(), keyed(&["Alpha"]))
            .is_open(true)
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .into_any_element()
    });

    flush_frame(cx);
    click(cx, 60., 18.);
    assert_eq!(opens.borrow().as_slice(), ["open:false"]);
    click(cx, 600., 300.);
    assert_eq!(
        opens.borrow().as_slice(),
        ["open:false", "open:false"],
        "a trigger latch must not suppress a later outside press"
    );
}

#[gpui::test]
fn empty_select_still_reports_an_outside_dismissal(cx: &mut TestAppContext) {
    let opens = events();
    let recorded = opens.clone();
    let cx = open_host(cx, move || {
        let opens = recorded.clone();
        Select::new("empty-select", Vec::new())
            .is_open(true)
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .into_any_element()
    });

    flush_frame(cx);
    click(cx, 600., 300.);
    assert_eq!(
        opens.borrow().as_slice(),
        ["open:false"],
        "an open empty Select still needs a token-gated outside boundary"
    );
}

#[gpui::test]
fn no_match_combo_box_still_reports_an_outside_dismissal(cx: &mut TestAppContext) {
    let opens = events();
    let recorded = opens.clone();
    let state = cx.new(|cx| InputState::new(cx));
    cx.update(|cx| state.update(cx, |state, _| state.set_value("zz")));
    let cx = open_host(cx, move || {
        let opens = recorded.clone();
        ComboBox::new(state.clone(), keyed(&["Alpha"]))
            .is_open(true)
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .into_any_element()
    });

    flush_frame(cx);
    click(cx, 600., 300.);
    assert_eq!(
        opens.borrow().as_slice(),
        ["open:false"],
        "an open no-match ComboBox still needs a token-gated outside boundary"
    );
}

#[gpui::test]
fn empty_autocomplete_still_reports_an_outside_dismissal(cx: &mut TestAppContext) {
    let opens = events();
    let recorded = opens.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let opens = recorded.clone();
        Autocomplete::new(state.clone(), Vec::new())
            .is_open(true)
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .into_any_element()
    });

    flush_frame(cx);
    click(cx, 600., 300.);
    assert_eq!(
        opens.borrow().as_slice(),
        ["open:false"],
        "an open empty Autocomplete still needs a token-gated outside boundary"
    );
}

#[gpui::test]
fn read_only_autocomplete_keeps_query_immutable_while_roving_the_collection(
    cx: &mut TestAppContext,
) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = recorded.clone();
        Autocomplete::new(state_for_view.clone(), keyed(&["Alpha", "Beta"]))
            .default_open(true)
            .is_read_only(true)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    flush_frame(cx);
    press(cx, "x");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "",
        "a read-only Autocomplete must not edit its query"
    );

    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        changes.borrow().as_slice(),
        ["Alpha"],
        "read-only Autocomplete remains navigable and can choose an item"
    );
}
