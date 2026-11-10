//! Explicit overlay-stack contracts for DatePicker and DateRangePicker.

use crate::harness;

use std::{cell::Cell, rc::Rc};

use gpui::{point, prelude::*, px, Modifiers, MouseButton, TestAppContext};
use harness::{click, events, open_host, press};
use herogpui_components::{
    Button, CalendarState, Date, DatePicker, DateRangePicker, DateRangeState, Popover,
};

fn reduce_motion() {
    harness::still();
}

#[gpui::test]
fn nested_date_picker_escape_closes_only_the_picker_then_parent(cx: &mut TestAppContext) {
    reduce_motion();
    let changes = events();
    let date_open = Rc::new(Cell::new(true));
    let parent_open = Rc::new(Cell::new(true));
    let date_state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 6, 15)));

    let date_open_for_view = date_open;
    let parent_open_for_view = parent_open;
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes_for_view.clone();
        let date_open = date_open_for_view.clone();
        let parent_open = parent_open_for_view.clone();
        let parent_for_callback = parent_open.clone();
        let date_for_callback = date_open.clone();
        let parent_changes = changes.clone();
        let date_changes = changes;
        Popover::new(Button::new("date-parent-trigger").label("Parent"))
            .id("date-parent")
            .is_open(parent_open.get())
            .on_open_change(move |open, window, _| {
                parent_for_callback.set(*open);
                parent_changes.borrow_mut().push(format!("parent:{open}"));
                window.refresh();
            })
            .child(
                DatePicker::new(date_state.clone())
                    .is_open(date_open.get())
                    .on_open_change(move |open, window, _| {
                        date_for_callback.set(*open);
                        date_changes.borrow_mut().push(format!("date:{open}"));
                        window.refresh();
                    }),
            )
            .into_any_element()
    });

    press(cx, "escape");
    assert_eq!(changes.borrow().as_slice(), ["date:false"]);

    press(cx, "escape");
    assert_eq!(changes.borrow().as_slice(), ["date:false", "parent:false"]);
}

#[gpui::test]
fn content_only_date_picker_does_not_register_an_invisible_overlay(cx: &mut TestAppContext) {
    reduce_motion();
    let changes = events();
    let parent_open = Rc::new(Cell::new(true));
    let date_state = cx.new(|cx| CalendarState::new(cx));

    let parent_open_for_view = parent_open;
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let parent_for_callback = parent_open_for_view.clone();
        let changes = changes_for_view.clone();
        Popover::new(Button::new("content-date-parent-trigger").label("Parent"))
            .id("content-date-parent")
            .is_open(parent_open_for_view.get())
            .on_open_change(move |open, window, _| {
                parent_for_callback.set(*open);
                changes.borrow_mut().push(format!("parent:{open}"));
                window.refresh();
            })
            .child(
                DatePicker::new(date_state.clone())
                    .is_open(true)
                    .content(|_| gpui::div().child("Custom date content").into_any_element()),
            )
            .into_any_element()
    });

    press(cx, "escape");
    assert_eq!(changes.borrow().as_slice(), ["parent:false"]);
}

#[gpui::test]
fn content_only_date_range_picker_does_not_register_an_invisible_overlay(cx: &mut TestAppContext) {
    reduce_motion();
    let changes = events();
    let parent_open = Rc::new(Cell::new(true));
    let range_state = cx.new(|cx| DateRangeState::new(cx));

    let parent_open_for_view = parent_open;
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let parent_for_callback = parent_open_for_view.clone();
        let changes = changes_for_view.clone();
        Popover::new(Button::new("content-range-parent-trigger").label("Parent"))
            .id("content-range-parent")
            .is_open(parent_open_for_view.get())
            .on_open_change(move |open, window, _| {
                parent_for_callback.set(*open);
                changes.borrow_mut().push(format!("parent:{open}"));
                window.refresh();
            })
            .child(
                DateRangePicker::new(range_state.clone())
                    .is_open(true)
                    .content(|_| gpui::div().child("Custom range content").into_any_element()),
            )
            .into_any_element()
    });

    press(cx, "escape");
    assert_eq!(changes.borrow().as_slice(), ["parent:false"]);
}

#[gpui::test]
fn nested_date_range_picker_outside_press_closes_only_the_picker_then_parent(
    cx: &mut TestAppContext,
) {
    reduce_motion();
    let changes = events();
    let range_open = Rc::new(Cell::new(true));
    let parent_open = Rc::new(Cell::new(true));
    let range_state = cx.new(|cx| DateRangeState::new(cx));

    let range_open_for_view = range_open;
    let parent_open_for_view = parent_open;
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes_for_view.clone();
        let range_open = range_open_for_view.clone();
        let parent_open = parent_open_for_view.clone();
        let parent_for_callback = parent_open.clone();
        let range_for_callback = range_open.clone();
        let parent_changes = changes.clone();
        let range_changes = changes;
        Popover::new(Button::new("range-parent-trigger").label("Parent"))
            .id("range-parent")
            .is_open(parent_open.get())
            .on_open_change(move |open, window, _| {
                parent_for_callback.set(*open);
                parent_changes.borrow_mut().push(format!("parent:{open}"));
                window.refresh();
            })
            .child(
                DateRangePicker::new(range_state.clone())
                    .is_open(range_open.get())
                    .on_open_change(move |open, window, _| {
                        range_for_callback.set(*open);
                        range_changes.borrow_mut().push(format!("range:{open}"));
                        window.refresh();
                    }),
            )
            .into_any_element()
    });

    click(cx, 600., 500.);
    assert_eq!(changes.borrow().as_slice(), ["range:false"]);

    click(cx, 600., 500.);
    assert_eq!(changes.borrow().as_slice(), ["range:false", "parent:false"]);
}

#[gpui::test]
fn date_picker_trigger_outside_guard_does_not_report_duplicate_close(cx: &mut TestAppContext) {
    reduce_motion();
    let changes = events();
    let open = Rc::new(Cell::new(false));
    let state = cx.new(|cx| CalendarState::new(cx));
    let open_for_view = open;
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes_for_view.clone();
        let open_for_callback = open_for_view.clone();
        DatePicker::new(state.clone())
            .is_open(open_for_view.get())
            .on_open_change(move |value, window, _| {
                open_for_callback.set(*value);
                changes.borrow_mut().push(value.to_string());
                window.refresh();
            })
            .into_any_element()
    });

    click(cx, 295., 18.);
    click(cx, 295., 18.);
    assert_eq!(changes.borrow().as_slice(), ["true", "false"]);
}

#[gpui::test]
fn date_range_picker_escape_restores_the_actual_start_field_once(cx: &mut TestAppContext) {
    reduce_motion();
    let changes = events();
    let open = Rc::new(Cell::new(false));
    let state = cx.new(|cx| DateRangeState::new(cx));
    let open_for_view = open;
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes_for_view.clone();
        let open_for_callback = open_for_view.clone();
        DateRangePicker::new(state.clone())
            .is_open(open_for_view.get())
            .on_open_change(move |value, window, _| {
                open_for_callback.set(*value);
                changes.borrow_mut().push(value.to_string());
                window.refresh();
            })
            .into_any_element()
    });

    press(cx, "tab alt-up escape space");
    assert_eq!(changes.borrow().as_slice(), ["true", "false", "true"]);
}

#[gpui::test]
fn date_picker_cancelled_trigger_press_does_not_block_later_outside_dismissal(
    cx: &mut TestAppContext,
) {
    reduce_motion();
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let cx = open_host(cx, move || {
        let changes = recorded.clone();
        DatePicker::new(state.clone())
            .is_open(true)
            .on_open_change(move |open, _, _| changes.borrow_mut().push(open.to_string()))
            .into_any_element()
    });

    cx.simulate_mouse_down(
        point(px(295.), px(18.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    click(cx, 600., 300.);
    assert_eq!(
        changes.borrow().as_slice(),
        ["false"],
        "a cancelled trigger press must clear its one-dispatch outside guard"
    );
}

#[gpui::test]
fn date_range_picker_cancelled_trigger_press_does_not_block_later_outside_dismissal(
    cx: &mut TestAppContext,
) {
    reduce_motion();
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let cx = open_host(cx, move || {
        let changes = recorded.clone();
        DateRangePicker::new(state.clone())
            .is_open(true)
            .on_open_change(move |open, _, _| changes.borrow_mut().push(open.to_string()))
            .into_any_element()
    });

    cx.simulate_mouse_down(
        point(px(300.), px(18.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    click(cx, 600., 300.);
    assert_eq!(
        changes.borrow().as_slice(),
        ["false"],
        "a cancelled range trigger press must clear its one-dispatch outside guard"
    );
}
