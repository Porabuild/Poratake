//! ColorPicker overlay-stack behavior against the pinned React Aria contract.
//!
//! React Aria 3.51.0's `useOverlay` keeps visible overlays in order and lets
//! only the topmost overlay close on Escape or an outside interaction. These
//! tests keep a ColorPicker inside a Popover so a second event proves the
//! parent remains available after the child closes.

use crate::harness;

use std::{cell::Cell, rc::Rc};

use gpui::{point, prelude::*, px, Modifiers, MouseButton, TestAppContext};
use harness::{click, events, open_host, press};
use herogpui_components::{Button, ColorPicker, PickerColor, Popover};

fn reduced_motion() {
    harness::still();
}

#[gpui::test]
fn color_picker_flips_and_shifts_inside_the_viewport(cx: &mut TestAppContext) {
    reduced_motion();
    let cx = open_host(cx, || {
        gpui::div()
            .absolute()
            .left(px(500.))
            .top(px(440.))
            .child(
                ColorPicker::new("edge-picker", PickerColor::hsb(210., 0.5, 0.6))
                    .label("Color")
                    .is_open(true)
                    .show_alpha(true),
            )
            .into_any_element()
    });
    cx.simulate_resize(gpui::size(px(640.), px(540.)));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let panel = cx.debug_bounds(r#"Name("edge-picker")-panel"#).unwrap();
    assert!(
        panel.left() >= px(12.) && panel.right() <= px(628.),
        "{panel:?}"
    );
    assert!(
        panel.top() >= px(12.) && panel.bottom() < px(480.),
        "{panel:?}"
    );
    assert!(
        panel.size.height > px(250.),
        "the complete panel should fit above the trigger: {panel:?}"
    );
}

#[gpui::test]
fn color_picker_initial_open_with_motion_mounts_and_settles(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .size_full()
            .child(
                gpui::div().absolute().left(px(500.)).top(px(120.)).child(
                    ColorPicker::new("motion-open-picker", PickerColor::hsb(210., 0.5, 0.6))
                        .is_open(true)
                        .show_alpha(true),
                ),
            )
            .into_any_element()
    });
    cx.simulate_resize(gpui::size(px(640.), px(540.)));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let panel = cx
        .debug_bounds(r#"Name("motion-open-picker")-panel"#)
        .expect("an initially-open picker must render its panel");
    assert!(panel.size.height > px(250.), "{panel:?}");
}

#[gpui::test]
fn color_picker_scrolls_to_alpha_in_a_short_viewport(cx: &mut TestAppContext) {
    reduced_motion();
    let changed = Rc::new(Cell::new(false));
    let page_scroll = gpui::ScrollHandle::new();
    let cx = open_host(cx, {
        let changed = changed.clone();
        let page_scroll = page_scroll.clone();
        move || {
            let changed = changed.clone();
            gpui::div()
                .id("picker-scroll-page")
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&page_scroll)
                .child(
                    gpui::div().relative().h(px(1000.)).child(
                        gpui::div().absolute().left(px(500.)).top(px(120.)).child(
                            ColorPicker::new("short-picker", PickerColor::hsb(210., 0.5, 0.6))
                                .is_open(true)
                                .show_alpha(true)
                                .on_change(move |value, _, _| changed.set(value.alpha < 0.9)),
                        ),
                    ),
                )
                .into_any_element()
        }
    });
    cx.simulate_resize(gpui::size(px(640.), px(300.)));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let panel = cx.debug_bounds(r#"Name("short-picker")-panel"#).unwrap();
    assert_eq!(panel.top(), px(152.));
    assert_eq!(panel.bottom(), px(288.));
    assert!(
        panel.left() >= px(12.) && panel.right() <= px(628.),
        "{panel:?}"
    );
    cx.simulate_click(panel.center(), Modifiers::none());
    for selector in [
        r#"Name("short-picker")-hue"#,
        r#"Name("short-picker")-alpha"#,
    ] {
        press(cx, "tab");
        let focused = cx.debug_bounds(selector).unwrap();
        assert!(
            focused.top() >= panel.top() && focused.bottom() <= panel.bottom(),
            "focused slider {focused:?} outside {panel:?}"
        );
    }
    press(cx, "shift-tab shift-tab");
    let hidden_alpha = cx.debug_bounds(r#"Name("short-picker")-alpha"#).unwrap();
    assert!(
        hidden_alpha.top() >= panel.bottom(),
        "returning to the area must reveal its beginning"
    );
    cx.simulate_event(gpui::ScrollEvent {
        position: panel.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
        modifiers: Modifiers::none(),
        touch_phase: gpui::TouchPhase::Moved,
        ..Default::default()
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let alpha = cx.debug_bounds(r#"Name("short-picker")-alpha"#).unwrap();
    assert!(
        alpha.top() >= panel.top() && alpha.bottom() <= panel.bottom(),
        "{alpha:?} outside {panel:?}"
    );
    cx.simulate_event(gpui::ScrollEvent {
        position: panel.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
        modifiers: Modifiers::none(),
        touch_phase: gpui::TouchPhase::Moved,
        ..Default::default()
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(
        page_scroll.offset().y,
        px(0.),
        "scrolling at the panel boundary must not move the page"
    );
    cx.simulate_click(alpha.center(), Modifiers::none());
    assert!(
        changed.get(),
        "the scrolled alpha slider must remain interactive"
    );
    cx.simulate_resize(gpui::size(px(640.), px(540.)));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let expanded = cx.debug_bounds(r#"Name("short-picker")-panel"#).unwrap();
    assert_eq!(expanded.top(), px(152.));
    assert!(
        expanded.size.height > px(250.),
        "the panel must grow again after resize: {expanded:?}"
    );
}

#[gpui::test]
fn color_picker_reopen_starts_at_area_after_full_close(cx: &mut TestAppContext) {
    reduced_motion();
    let cx = open_host(cx, || {
        gpui::div()
            .relative()
            .size_full()
            .child(
                gpui::div().absolute().left(px(500.)).top(px(120.)).child(
                    ColorPicker::new("reopen-picker", PickerColor::hsb(210., 0.5, 0.6))
                        .show_alpha(true),
                ),
            )
            .into_any_element()
    });
    cx.simulate_resize(gpui::size(px(640.), px(300.)));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();

    // Uncontrolled trigger opens the short panel.
    click(cx, 560., 132.);
    cx.run_until_parked();
    let panel = cx.debug_bounds(r#"Name("reopen-picker")-panel"#).unwrap();
    assert_eq!(panel.top(), px(152.));
    assert_eq!(panel.bottom(), px(288.));

    // Scroll to the bottom so the alpha slider is visible.
    cx.simulate_event(gpui::ScrollEvent {
        position: panel.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
        modifiers: Modifiers::none(),
        touch_phase: gpui::TouchPhase::Moved,
        ..Default::default()
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let panel = cx.debug_bounds(r#"Name("reopen-picker")-panel"#).unwrap();
    let alpha = cx.debug_bounds(r#"Name("reopen-picker")-alpha"#).unwrap();
    assert!(
        alpha.top() >= panel.top() && alpha.bottom() <= panel.bottom(),
        "scrolling down must reveal the alpha slider: {alpha:?} outside {panel:?}"
    );

    // Fully close: Escape starts the exit, then the exit timer unmounts the
    // panel the way RAC 1.20.0 returns null after closed+exit.
    cx.simulate_click(panel.center(), Modifiers::none());
    press(cx, "escape");
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(150));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(r#"Name("reopen-picker")-panel"#).is_none(),
        "the panel must be fully closed before reopening"
    );

    // Reopen from the trigger: the area is back at the beginning and the
    // alpha slider is below the viewport again.
    click(cx, 560., 132.);
    cx.run_until_parked();
    let panel = cx.debug_bounds(r#"Name("reopen-picker")-panel"#).unwrap();
    let alpha = cx.debug_bounds(r#"Name("reopen-picker")-alpha"#).unwrap();
    assert!(
        alpha.top() >= panel.bottom(),
        "reopening must show the area at the beginning, got {alpha:?} inside {panel:?}"
    );
}

#[gpui::test]
fn nested_color_picker_escape_closes_only_the_picker_then_parent(cx: &mut TestAppContext) {
    reduced_motion();
    let picker_open = Rc::new(Cell::new(true));
    let outer_open = Rc::new(Cell::new(true));
    let changes = events();

    let cx = open_host(cx, {
        let changes = changes.clone();
        move || {
            let picker_open = picker_open.clone();
            let outer_open = outer_open.clone();
            let changes = changes.clone();
            Popover::new(Button::new("color-stack-outer-trigger").label("Outer"))
                .id("color-stack-outer")
                .is_open(outer_open.get())
                .on_open_change({
                    let outer_open = outer_open.clone();
                    let changes = changes.clone();
                    move |value, window, _| {
                        outer_open.set(*value);
                        changes.borrow_mut().push(format!("outer:{value}"));
                        window.refresh();
                    }
                })
                .child(
                    ColorPicker::new("color-stack-picker", PickerColor::hsb(210., 0.5, 0.6))
                        .is_open(picker_open.get())
                        .on_open_change({
                            let picker_open = picker_open.clone();
                            move |value, window, _| {
                                picker_open.set(*value);
                                changes.borrow_mut().push(format!("picker:{value}"));
                                window.refresh();
                            }
                        }),
                )
                .into_any_element()
        }
    });

    // The parent focus scope first reaches the ColorPicker trigger and then
    // its ColorArea. Escape bubbles from that area through the picker root.
    press(cx, "tab tab");
    press(cx, "escape");
    assert_eq!(
        changes.borrow().as_slice(),
        ["picker:false"],
        "the first Escape must close only the topmost ColorPicker"
    );

    press(cx, "escape");
    assert_eq!(
        changes.borrow().as_slice(),
        ["picker:false", "outer:false"],
        "the second Escape must reach the still-open parent Popover"
    );
}

#[gpui::test]
fn nested_color_picker_outside_press_closes_only_the_picker_then_parent(cx: &mut TestAppContext) {
    reduced_motion();
    let picker_open = Rc::new(Cell::new(true));
    let outer_open = Rc::new(Cell::new(true));
    let changes = events();

    let cx = open_host(cx, {
        let changes = changes.clone();
        move || {
            let picker_open = picker_open.clone();
            let outer_open = outer_open.clone();
            let changes = changes.clone();
            Popover::new(Button::new("color-outside-outer-trigger").label("Outer"))
                .id("color-outside-outer")
                .is_open(outer_open.get())
                .on_open_change({
                    let outer_open = outer_open.clone();
                    let changes = changes.clone();
                    move |value, window, _| {
                        outer_open.set(*value);
                        changes.borrow_mut().push(format!("outer:{value}"));
                        window.refresh();
                    }
                })
                .child(
                    ColorPicker::new("color-outside-picker", PickerColor::hsb(210., 0.5, 0.6))
                        .is_open(picker_open.get())
                        .on_open_change({
                            let picker_open = picker_open.clone();
                            move |value, window, _| {
                                picker_open.set(*value);
                                changes.borrow_mut().push(format!("picker:{value}"));
                                window.refresh();
                            }
                        }),
                )
                .into_any_element()
        }
    });

    click(cx, 700., 500.);
    assert_eq!(
        changes.borrow().as_slice(),
        ["picker:false"],
        "the first outside press must close only the topmost ColorPicker"
    );

    click(cx, 700., 500.);
    assert_eq!(
        changes.borrow().as_slice(),
        ["picker:false", "outer:false"],
        "the second outside press must reach the remaining Popover"
    );
}

#[gpui::test]
fn color_picker_escape_reports_one_close_and_restores_trigger_activation(cx: &mut TestAppContext) {
    reduced_motion();
    let picker_open = Rc::new(Cell::new(false));
    let changes = events();

    let cx = open_host(cx, {
        let changes = changes.clone();
        move || {
            let picker_open = picker_open.clone();
            let changes = changes.clone();
            ColorPicker::new("color-focus-picker", PickerColor::hsb(210., 0.5, 0.6))
                .is_open(picker_open.get())
                .on_open_change({
                    let picker_open = picker_open.clone();
                    move |value, window, _| {
                        picker_open.set(*value);
                        changes.borrow_mut().push(format!("picker:{value}"));
                        window.refresh();
                    }
                })
                .into_any_element()
        }
    });

    click(cx, 60., 12.);
    press(cx, "tab");
    press(cx, "escape");
    assert_eq!(
        changes.borrow().as_slice(),
        ["picker:true", "picker:false"],
        "Escape must invoke on_open_change exactly once"
    );

    // The closing path returns focus to the trigger, so its click listener is
    // the only activation path when Enter is pressed after Escape.
    press(cx, "enter");
    assert_eq!(
        changes.borrow().as_slice(),
        ["picker:true", "picker:false", "picker:true"],
        "focus restoration must make Enter reopen the picker once"
    );
}

#[gpui::test]
fn clicking_an_open_color_picker_trigger_reports_one_close(cx: &mut TestAppContext) {
    reduced_motion();
    let picker_open = Rc::new(Cell::new(true));
    let changes = events();

    let cx = open_host(cx, {
        let changes = changes.clone();
        move || {
            ColorPicker::new("color-open-trigger-race", PickerColor::hsb(210., 0.5, 0.6))
                .is_open(picker_open.get())
                .on_open_change({
                    let picker_open = picker_open.clone();
                    let changes = changes.clone();
                    move |value, window, _| {
                        picker_open.set(*value);
                        changes.borrow_mut().push(format!("picker:{value}"));
                        window.refresh();
                    }
                })
                .into_any_element()
        }
    });

    click(cx, 60., 12.);

    assert_eq!(
        changes.borrow().as_slice(),
        ["picker:false"],
        "an open trigger owns its close; outside dismissal must not report a second change"
    );
}

#[gpui::test]
fn canceled_color_picker_trigger_press_does_not_block_a_later_outside_close(
    cx: &mut TestAppContext,
) {
    reduced_motion();
    let picker_open = Rc::new(Cell::new(true));
    let changes = events();

    let cx = open_host(cx, {
        let changes = changes.clone();
        move || {
            ColorPicker::new("color-canceled-trigger", PickerColor::hsb(210., 0.5, 0.6))
                .is_open(picker_open.get())
                .on_open_change({
                    let picker_open = picker_open.clone();
                    let changes = changes.clone();
                    move |value, window, _| {
                        picker_open.set(*value);
                        changes.borrow_mut().push(format!("picker:{value}"));
                        window.refresh();
                    }
                })
                .into_any_element()
        }
    });

    let modifiers = Modifiers::none();
    cx.simulate_mouse_down(point(px(60.), px(12.)), MouseButton::Left, modifiers);
    cx.simulate_mouse_up(point(px(700.), px(500.)), MouseButton::Left, modifiers);
    click(cx, 700., 500.);

    assert_eq!(
        changes.borrow().as_slice(),
        ["picker:false"],
        "a canceled trigger press must not leave later outside dismissal declined"
    );
}

#[gpui::test]
fn controlled_color_picker_without_callback_blocks_parent_outside_dismissal(
    cx: &mut TestAppContext,
) {
    reduced_motion();
    let outer_open = Rc::new(Cell::new(true));
    let changes = events();

    let cx = open_host(cx, {
        let changes = changes.clone();
        move || {
            Popover::new(Button::new("color-static-outer-trigger").label("Outer"))
                .id("color-static-outer")
                .is_open(outer_open.get())
                .on_open_change({
                    let outer_open = outer_open.clone();
                    let changes = changes.clone();
                    move |value, window, _| {
                        outer_open.set(*value);
                        changes.borrow_mut().push(format!("outer:{value}"));
                        window.refresh();
                    }
                })
                .child(
                    ColorPicker::new("color-static-picker", PickerColor::hsb(210., 0.5, 0.6))
                        .is_open(true),
                )
                .into_any_element()
        }
    });

    click(cx, 700., 500.);

    assert!(
        changes.borrow().is_empty(),
        "the topmost controlled picker must consume outside dismissal even when it cannot change"
    );
}
