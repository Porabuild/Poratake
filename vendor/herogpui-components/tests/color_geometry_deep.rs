//! Deep pointer-geometry tests for ColorArea and ColorSlider.
//!
//! HeroUI v3.2.4 pins react-aria 3.51.0 and react-stately 3.49.0. In those
//! exact releases, `useColorArea` and `useSlider` subtract the target's
//! `getBoundingClientRect()` origin before converting a pointer position,
//! `useMove` applies every drag delta, and state reports `onChangeEnd` only
//! when dragging transitions from true to false. `useColorAreaState` also
//! converts the value to `colorSpace` before reading and replacing channels.
//! These tests drive those contracts through gpui's real headless hit testing.

use crate::harness;
use crate::source_scan;

use std::{cell::RefCell, rc::Rc, sync::Arc};

use gpui::{
    point, prelude::*, px, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext,
};
use herogpui_components::{
    Button, ColorArea, ColorAreaThumbState, ColorChannel, ColorField, ColorSlider,
    ColorSliderThumbState, ColorSpace, Form, FormData, InputState, PickerColor,
};
use herogpui_core::Orientation;

use harness::{click, events, open_host, press};

type Submit = Arc<dyn Fn(&mut gpui::Window, &mut gpui::App)>;

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn drag_through(cx: &mut VisualTestContext, from: (f32, f32), moves: &[(f32, f32)]) {
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(from.0), px(from.1)),
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    flush_frame(cx);

    for &(x, y) in moves {
        cx.simulate_event(MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        });
        flush_frame(cx);
    }

    let &(x, y) = moves.last().expect("a drag needs at least one move");
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: point(px(x), px(y)),
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    flush_frame(cx);
}

fn move_pointer(cx: &mut VisualTestContext, x: f32, y: f32, pressed_button: Option<MouseButton>) {
    cx.simulate_event(MouseMoveEvent {
        position: point(px(x), px(y)),
        pressed_button,
        modifiers: Modifiers::none(),
    });
    flush_frame(cx);
}

fn release_pointer(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: point(px(x), px(y)),
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    flush_frame(cx);
}

fn click_with_modifiers(cx: &mut VisualTestContext, x: f32, y: f32, modifiers: Modifiers) {
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(x), px(y)),
        modifiers,
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: point(px(x), px(y)),
        modifiers,
        click_count: 1,
    });
    flush_frame(cx);
}

fn wheel(cx: &mut VisualTestContext, x: f32, y: f32, dy: f32) {
    cx.simulate_event(ScrollEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
        ..Default::default()
    });
    flush_frame(cx);
}

#[gpui::test]
fn color_area_pointer_uses_area_local_coordinates(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .pl(px(100.))
            .pt(px(50.))
            .child(
                ColorArea::new("offset-area", PickerColor::hsb(210., 0.5, 0.5))
                    .default_value(PickerColor::hsb(210., 0.5, 0.5))
                    .size(px(240.), px(180.))
                    .on_change(move |color, _, _| {
                        recorded
                            .borrow_mut()
                            .push(format!("{:.2},{:.2}", color.saturation, color.brightness));
                    }),
            )
            .into_any_element()
    });

    // The area starts at (100, 50). This is local (60, 45), or 25% across
    // and 25% down. Brightness grows upward, so the resulting value is 75%.
    click(cx, 160., 95.);
    assert_eq!(recorded.borrow().as_slice(), ["0.25,0.75"]);
}

#[gpui::test]
fn color_area_drag_reports_each_move_and_one_change_end(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let change = for_view.clone();
        let end = for_view.clone();
        ColorArea::new("drag-area", PickerColor::hsb(210., 0.5, 0.5))
            .default_value(PickerColor::hsb(210., 0.5, 0.5))
            .size(px(200.), px(100.))
            .on_change(move |color, _, _| {
                change
                    .borrow_mut()
                    .push(format!("change:{:.2}", color.saturation));
            })
            .on_change_end(move |color, _, _| {
                end.borrow_mut()
                    .push(format!("end:{:.2}", color.saturation));
            })
            .into_any_element()
    });

    drag_through(cx, (20., 50.), &[(80., 50.), (150., 50.)]);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:0.10", "change:0.40", "change:0.75", "end:0.75"],
        "the press and both moves report continuously, then release reports one final change"
    );
}

#[gpui::test]
fn color_area_drag_clamps_and_ends_after_release_outside(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let change = for_view.clone();
        let end = for_view.clone();
        ColorArea::new("outside-area", PickerColor::hsb(210., 0.5, 0.5))
            .default_value(PickerColor::hsb(210., 0.5, 0.5))
            .size(px(200.), px(100.))
            .on_change(move |color, _, _| {
                change
                    .borrow_mut()
                    .push(format!("change:{:.2}", color.saturation));
            })
            .on_change_end(move |color, _, _| {
                end.borrow_mut()
                    .push(format!("end:{:.2}", color.saturation));
            })
            .into_any_element()
    });

    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(20.), px(50.)),
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    flush_frame(cx);
    move_pointer(cx, 80., 50., Some(MouseButton::Left));
    move_pointer(cx, 260., 50., Some(MouseButton::Left));
    release_pointer(cx, 260., 50.);

    let after_release = recorded.borrow().len();
    move_pointer(cx, 100., 50., None);
    assert_eq!(recorded.borrow().len(), after_release);
    assert_eq!(
        recorded
            .borrow()
            .iter()
            .filter(|v| v.starts_with("end:"))
            .count(),
        1,
        "an outside release must finish the drag exactly once"
    );
    assert!(
        recorded.borrow().iter().any(|v| v == "change:1.00")
            && recorded.borrow().last().is_some_and(|v| v == "end:1.00"),
        "the outside move and release must clamp to the area's far edge: {:?}",
        recorded.borrow()
    );
}

#[gpui::test]
fn color_slider_pointer_uses_track_local_coordinates(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .pl(px(100.))
            .pt(px(50.))
            .child(
                ColorSlider::new(
                    "offset-slider",
                    PickerColor::hsb(0., 1., 1.),
                    ColorChannel::Hue,
                )
                .default_value(PickerColor::hsb(0., 1., 1.))
                .length(px(240.))
                .show_label(false)
                .on_change(move |color, _, _| {
                    recorded.borrow_mut().push(format!("{:.0}", color.hue));
                }),
            )
            .into_any_element()
    });

    // The track starts at (100, 50). Its 10px edge caps leave 220px of value
    // travel, so local x=65 is one quarter and must be 90 degrees rather than
    // treating the window coordinate as a track coordinate.
    click(cx, 165., 60.);
    assert_eq!(recorded.borrow().as_slice(), ["90"]);
}

#[gpui::test]
fn color_slider_drag_reports_each_move_and_one_change_end(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let change = for_view.clone();
        let end = for_view.clone();
        ColorSlider::new(
            "drag-slider",
            PickerColor::hsb(0., 1., 1.),
            ColorChannel::Hue,
        )
        .default_value(PickerColor::hsb(0., 1., 1.))
        .length(px(240.))
        .show_label(false)
        .on_change(move |color, _, _| {
            change.borrow_mut().push(format!("change:{:.0}", color.hue));
        })
        .on_change_end(move |color, _, _| {
            end.borrow_mut().push(format!("end:{:.0}", color.hue));
        })
        .into_any_element()
    });

    drag_through(cx, (32., 10.), &[(98., 10.), (175., 10.)]);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:36", "change:144", "change:270", "end:270"],
        "the press and both moves report continuously, then release reports one final change"
    );
}

#[gpui::test]
fn color_slider_drag_clamps_and_ends_after_release_outside(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let change = for_view.clone();
        let end = for_view.clone();
        ColorSlider::new(
            "outside-slider",
            PickerColor::hsb(0., 1., 1.),
            ColorChannel::Hue,
        )
        .default_value(PickerColor::hsb(0., 1., 1.))
        .length(px(240.))
        .show_label(false)
        .on_change(move |color, _, _| {
            change.borrow_mut().push(format!("change:{:.0}", color.hue));
        })
        .on_change_end(move |color, _, _| {
            end.borrow_mut().push(format!("end:{:.0}", color.hue));
        })
        .into_any_element()
    });

    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(24.), px(8.)),
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    flush_frame(cx);
    move_pointer(cx, 96., 8., Some(MouseButton::Left));
    move_pointer(cx, 300., 8., Some(MouseButton::Left));
    release_pointer(cx, 300., 8.);

    let after_release = recorded.borrow().len();
    move_pointer(cx, 120., 8., None);
    assert_eq!(recorded.borrow().len(), after_release);
    assert_eq!(
        recorded
            .borrow()
            .iter()
            .filter(|v| v.starts_with("end:"))
            .count(),
        1,
        "an outside release must finish the drag exactly once"
    );
    assert!(
        recorded.borrow().iter().any(|v| v == "change:360")
            && recorded.borrow().last().is_some_and(|v| v == "end:360"),
        "the outside move and release must clamp to the slider's far edge: {:?}",
        recorded.borrow()
    );
}

/// Pinned React Aria 1.20.0 exposes the shared `ColorThumbRenderProps` from a
/// ColorSlider thumb. Pressing the current coordinate is deliberately used:
/// dragging state must rerender even when no color-change callback fires.
#[gpui::test]
fn color_slider_thumb_receives_live_interaction_state(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(ColorSliderThumbState::default()));
    let record = seen.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        ColorSlider::new(
            "slider-thumb-state",
            PickerColor::hsb(180., 1., 1.),
            ColorChannel::Hue,
        )
        .default_value(PickerColor::hsb(180., 1., 1.))
        .show_label(false)
        .thumb(move |state| {
            *record.borrow_mut() = state;
            gpui::div().size_full().into_any_element()
        })
        .into_any_element()
    });

    assert_eq!(seen.borrow().color, PickerColor::hsb(180., 1., 1.));
    assert!(!seen.borrow().is_dragging, "the initial thumb is idle");

    let centre = point(px(120.), px(10.));
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    assert!(seen.borrow().is_hovered, "the thumb must report hover");

    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(
        seen.borrow().is_dragging,
        "pointer down must expose dragging"
    );
    assert!(seen.borrow().is_focused, "pointer down focuses the thumb");
    assert!(!seen.borrow().is_focus_visible);

    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(!seen.borrow().is_dragging, "release must end dragging");

    press(cx, "right");
    flush_frame(cx);
    assert!(seen.borrow().is_focus_visible);
}

#[gpui::test]
fn disabled_color_slider_thumb_masks_interaction_state(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(ColorSliderThumbState::default()));
    let record = seen.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        ColorSlider::new(
            "disabled-slider-thumb",
            PickerColor::hsb(180., 1., 1.),
            ColorChannel::Hue,
        )
        .show_label(false)
        .is_disabled(true)
        .thumb(move |state| {
            *record.borrow_mut() = state;
            gpui::div().size_full().into_any_element()
        })
        .into_any_element()
    });

    let centre = point(px(120.), px(10.));
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);

    let state = *seen.borrow();
    assert!(state.is_disabled);
    assert!(!state.is_hovered);
    assert!(!state.is_dragging);
    assert!(!state.is_focused);
    assert!(!state.is_focus_visible);
}

#[gpui::test]
fn pressing_current_color_ends_without_reporting_a_change(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let area_change = for_view.clone();
        let area_end = for_view.clone();
        let slider_change = for_view.clone();
        let slider_end = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorArea::new("same-area", PickerColor::hsb(180., 0.5, 0.5))
                    .size(px(100.), px(100.))
                    .on_change(move |_, _, _| area_change.borrow_mut().push("area-change".into()))
                    .on_change_end(move |_, _, _| area_end.borrow_mut().push("area-end".into())),
            )
            .child(
                ColorSlider::new(
                    "same-slider",
                    PickerColor::hsb(180., 1., 1.),
                    ColorChannel::Hue,
                )
                .length(px(240.))
                .show_label(false)
                .on_change(move |_, _, _| {
                    slider_change.borrow_mut().push("slider-change".into());
                })
                .on_change_end(move |_, _, _| slider_end.borrow_mut().push("slider-end".into())),
            )
            .into_any_element()
    });

    click(cx, 50., 50.);
    click(cx, 120., 128.);
    assert_eq!(recorded.borrow().as_slice(), ["area-end", "slider-end"]);
}

/// Pinned React Aria 1.20.0 gives `ColorThumb` children its live color,
/// dragging, hover, focus, focus-visible and disabled state. The area remains
/// the pointer target, so pressing the current coordinate must still expose a
/// drag even though it reports no color change.
#[gpui::test]
fn color_area_thumb_receives_live_interaction_state(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(ColorAreaThumbState::default()));
    let record = seen.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        ColorArea::new("thumb-state-area", PickerColor::hsb(180., 0.5, 0.5))
            .default_value(PickerColor::hsb(180., 0.5, 0.5))
            .size(px(100.), px(100.))
            .thumb(move |state| {
                *record.borrow_mut() = state;
                gpui::div().size_full().into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(seen.borrow().color, PickerColor::hsb(180., 0.5, 0.5));
    assert!(!seen.borrow().is_dragging, "the initial thumb is idle");

    let centre = point(px(50.), px(50.));
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    assert!(seen.borrow().is_hovered, "the thumb must report hover");

    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(
        seen.borrow().is_dragging,
        "pressing the current value must still report a live drag"
    );
    assert!(seen.borrow().is_focused, "pointer down focuses the thumb");
    assert!(
        !seen.borrow().is_focus_visible,
        "pointer focus must not invent keyboard-visible focus"
    );

    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(!seen.borrow().is_dragging, "release must end the drag");

    press(cx, "right");
    flush_frame(cx);
    assert!(
        seen.borrow().is_focus_visible,
        "a keyboard event on the focused area must expose focus-visible"
    );
}

#[gpui::test]
fn disabled_color_area_thumb_masks_interaction_state(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(ColorAreaThumbState::default()));
    let record = seen.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        ColorArea::new("disabled-thumb-state", PickerColor::hsb(180., 0.5, 0.5))
            .is_disabled(true)
            .thumb(move |state| {
                *record.borrow_mut() = state;
                gpui::div().size_full().into_any_element()
            })
            .into_any_element()
    });

    let thumb = point(px(112.), px(112.));
    cx.simulate_mouse_move(thumb, None::<MouseButton>, Modifiers::none());
    cx.simulate_mouse_down(thumb, MouseButton::Left, Modifiers::none());
    flush_frame(cx);

    let state = *seen.borrow();
    assert!(state.is_disabled);
    assert!(!state.is_hovered);
    assert!(!state.is_dragging);
    assert!(!state.is_focused);
    assert!(!state.is_focus_visible);
}

#[gpui::test]
fn modified_left_presses_do_not_start_color_drags(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let area_change = for_view.clone();
        let area_end = for_view.clone();
        let slider_change = for_view.clone();
        let slider_end = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorArea::new("modified-area", PickerColor::hsb(180., 0.5, 0.5))
                    .size(px(100.), px(100.))
                    .on_change(move |_, _, _| area_change.borrow_mut().push("area-change".into()))
                    .on_change_end(move |_, _, _| area_end.borrow_mut().push("area-end".into())),
            )
            .child(
                ColorSlider::new(
                    "modified-slider",
                    PickerColor::hsb(180., 1., 1.),
                    ColorChannel::Hue,
                )
                .length(px(240.))
                .show_label(false)
                .on_change(move |_, _, _| {
                    slider_change.borrow_mut().push("slider-change".into());
                })
                .on_change_end(move |_, _, _| slider_end.borrow_mut().push("slider-end".into())),
            )
            .into_any_element()
    });

    for modifiers in [
        Modifiers {
            alt: true,
            ..Modifiers::none()
        },
        Modifiers {
            control: true,
            ..Modifiers::none()
        },
        Modifiers {
            platform: true,
            ..Modifiers::none()
        },
    ] {
        click_with_modifiers(cx, 75., 25., modifiers);
        click_with_modifiers(cx, 180., 128., modifiers);
    }
    assert!(recorded.borrow().is_empty());
}

#[gpui::test]
fn hsl_color_area_mutates_hsl_channels(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ColorArea::new("hsl-area", PickerColor::hsb(0., 0.5, 0.5))
            .default_value(PickerColor::hsb(0., 0.5, 0.5))
            .color_space(ColorSpace::Hsl)
            .size(px(240.), px(180.))
            .on_change(move |color, _, _| {
                recorded.borrow_mut().push(format!(
                    "{:.2},{:.2}",
                    color.channel_in(ColorChannel::Saturation, ColorSpace::Hsl),
                    color.channel(ColorChannel::Lightness)
                ));
            })
            .into_any_element()
    });

    // Local x=60 is HSL saturation 25%; y=36 is HSL lightness 80%.
    // Lightness above 50% distinguishes preserving HSL saturation from
    // accidentally preserving the stored HSB saturation.
    click(cx, 60., 36.);
    assert_eq!(recorded.borrow().as_slice(), ["0.25,0.80"]);
}

#[test]
fn hsl_channels_survive_both_lightness_endpoints() {
    let color = PickerColor::hsb(20., 0.6, 0.8).with_channel_in(
        ColorChannel::Saturation,
        ColorSpace::Hsl,
        0.73,
    );
    for endpoint in [0.0, 1.0] {
        let restored = color
            .with_channel_in(ColorChannel::Lightness, ColorSpace::Hsl, endpoint)
            .with_channel_in(ColorChannel::Lightness, ColorSpace::Hsl, 0.5);
        assert!(
            (restored.channel_in(ColorChannel::Saturation, ColorSpace::Hsl) - 0.73).abs() < 0.001
        );
        assert!(
            (restored.channel_in(ColorChannel::Lightness, ColorSpace::Hsl) - 0.5).abs() < 0.001
        );
    }
}

#[gpui::test]
fn rgb_color_area_mutates_its_rgb_axes(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ColorArea::new("rgb-area", PickerColor::from_rgb(0.2, 0.3, 0.4))
            .color_space(ColorSpace::Rgb)
            .size(px(255.), px(255.))
            .on_change(move |color, _, _| {
                recorded.borrow_mut().push(format!(
                    "{:.0},{:.0},{:.0}",
                    color.channel(ColorChannel::Red),
                    color.channel(ColorChannel::Green),
                    color.channel(ColorChannel::Blue)
                ));
            })
            .into_any_element()
    });

    click(cx, 64., 64.);
    assert_eq!(recorded.borrow().as_slice(), ["64,191,102"]);
}

#[gpui::test]
fn hue_max_stays_at_the_far_edge_in_area_and_slider(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let area = for_view.clone();
        let slider = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorArea::new("hue-max-area", PickerColor::hsb(0., 1., 1.))
                    .default_value(PickerColor::hsb(0., 1., 1.))
                    .x_channel(ColorChannel::Hue)
                    .y_channel(ColorChannel::Brightness)
                    .size(px(240.), px(100.))
                    .on_change(move |color, _, _| {
                        area.borrow_mut().push(format!("area:{:.0}", color.hue));
                    }),
            )
            .child(
                ColorSlider::new(
                    "hue-max-slider",
                    PickerColor::hsb(0., 1., 1.),
                    ColorChannel::Hue,
                )
                .default_value(PickerColor::hsb(0., 1., 1.))
                .length(px(240.))
                .show_label(false)
                .on_change(move |color, _, _| {
                    slider.borrow_mut().push(format!("slider:{:.0}", color.hue));
                }),
            )
            .into_any_element()
    });

    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(120.), px(50.)),
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    flush_frame(cx);
    move_pointer(cx, 300., 50., Some(MouseButton::Left));
    release_pointer(cx, 300., 50.);

    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(120.), px(128.)),
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    flush_frame(cx);
    move_pointer(cx, 300., 128., Some(MouseButton::Left));
    release_pointer(cx, 300., 128.);

    assert!(recorded.borrow().iter().any(|value| value == "area:360"));
    assert!(recorded.borrow().iter().any(|value| value == "slider:360"));
    press(cx, "home");
    press(cx, "end");
    let values = recorded.borrow();
    assert_eq!(&values[values.len() - 2..], ["slider:0", "slider:360"]);
}

#[gpui::test]
fn color_area_keyboard_reports_only_real_changes_at_both_bounds(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ColorArea::new("area-key-boundaries", PickerColor::hsb(0., 1., 1.))
            .default_value(PickerColor::hsb(0., 1., 1.))
            .x_channel(ColorChannel::Hue)
            .y_channel(ColorChannel::Brightness)
            .on_change(move |color, _, _| {
                recorded.borrow_mut().push(format!("{:.0}", color.hue));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "home");
    press(cx, "home");
    assert!(
        recorded.borrow().is_empty(),
        "Home at the minimum must not synthesize a change"
    );

    for _ in 0..10 {
        press(cx, "end");
    }
    press(cx, "end");
    press(cx, "end");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["36", "72", "108", "144", "180", "216", "252", "288", "324", "360"],
        "real page-step changes must report, while repeated End at the maximum stays silent"
    );

    for _ in 0..10 {
        press(cx, "home");
    }
    press(cx, "home");
    press(cx, "home");
    let values = recorded.borrow();
    assert_eq!(values.len(), 20);
    assert_eq!(
        &values[10..],
        ["324", "288", "252", "216", "180", "144", "108", "72", "36", "0"]
    );
}

#[gpui::test]
fn color_slider_keyboard_reports_only_real_changes_at_both_bounds(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ColorSlider::new(
            "slider-key-boundaries",
            PickerColor::hsb(0., 1., 1.),
            ColorChannel::Hue,
        )
        .default_value(PickerColor::hsb(0., 1., 1.))
        .show_label(false)
        .on_change(move |color, _, _| {
            recorded.borrow_mut().push(format!("{:.0}", color.hue));
        })
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "home");
    press(cx, "home");
    press(cx, "end");
    press(cx, "end");
    press(cx, "end");
    press(cx, "home");
    press(cx, "home");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["360", "0"],
        "only the first Home/End transition at each boundary may report"
    );
}

#[gpui::test]
fn vertical_color_slider_increases_toward_the_top(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ColorSlider::new(
            "vertical-hue",
            PickerColor::hsb(0., 1., 1.),
            ColorChannel::Hue,
        )
        .orientation(Orientation::Vertical)
        .length(px(240.))
        .show_label(false)
        .on_change(move |color, _, _| {
            recorded.borrow_mut().push(format!("{:.0}", color.hue));
        })
        .into_any_element()
    });

    click(cx, 10., 65.);
    assert_eq!(recorded.borrow().as_slice(), ["270"]);
}

#[gpui::test]
fn color_area_hue_y_axis_increases_toward_the_top(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ColorArea::new("vertical-hue-area", PickerColor::hsb(0., 1., 0.5))
            .x_channel(ColorChannel::Brightness)
            .y_channel(ColorChannel::Hue)
            .size(px(100.), px(240.))
            .on_change(move |color, _, _| {
                recorded.borrow_mut().push(format!("{:.0}", color.hue));
            })
            .into_any_element()
    });

    click(cx, 50., 60.);
    assert_eq!(recorded.borrow().as_slice(), ["270"]);
}

#[gpui::test]
fn color_area_pointer_snaps_both_channels_to_their_steps(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .pl(px(100.))
            .pt(px(50.))
            .child(
                ColorArea::new("snap-area", PickerColor::hsb(210., 0.5, 0.5))
                    .size(px(240.), px(180.))
                    .on_change(move |color, _, _| {
                        recorded
                            .borrow_mut()
                            .push(format!("{:.2},{:.2}", color.saturation, color.brightness));
                    }),
            )
            .into_any_element()
    });

    // Local (1, 1) maps to raw saturation .0042 and brightness .9944.
    // React Stately snaps both normalized channels to their .01 step.
    click(cx, 101., 51.);
    assert_eq!(recorded.borrow().as_slice(), ["0.00,0.99"]);
}

#[gpui::test]
fn color_slider_pointer_snaps_to_the_channel_step(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .pl(px(100.))
            .child(
                ColorSlider::new(
                    "snap-slider",
                    PickerColor::hsb(0., 1., 1.),
                    ColorChannel::Hue,
                )
                .length(px(240.))
                .show_label(false)
                .on_change(move |color, _, _| {
                    recorded.borrow_mut().push(format!("{:.1}", color.hue));
                }),
            )
            .into_any_element()
    });

    // Local x=11 is one pixel into the 220px value travel after the 10px cap,
    // which maps to 1.64 degrees and snaps to 2.
    click(cx, 111., 10.);
    assert_eq!(recorded.borrow().as_slice(), ["2.0"]);
}

#[gpui::test]
fn channel_color_field_steps_with_up_and_down(cx: &mut TestAppContext) {
    let recorded = events();
    let state = cx.new(|cx| InputState::with_value(cx, "180"));
    let state_for_view = state;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ColorField::new("hue-arrows", PickerColor::hsb(180., 1., 1.))
            .default_value(PickerColor::hsb(180., 1., 1.))
            .state(state_for_view.clone())
            .color_space(ColorSpace::Hsl)
            .channel(ColorChannel::Hue)
            .on_change(move |color, _, _| {
                if let Some(color) = color {
                    recorded.borrow_mut().push(format!("{:.0}", color.hue));
                }
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "up");
    press(cx, "down");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["181", "180"],
        "the pinned ColorChannelField delegates to NumberField's one-step spinbutton keys"
    );
}

#[gpui::test]
fn normalized_channel_field_steps_internally_and_displays_percent(cx: &mut TestAppContext) {
    let recorded = events();
    let state = cx.new(|cx| InputState::with_value(cx, "50%"));
    let state_for_view = state.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ColorField::new("saturation-arrows", PickerColor::hsb(180., 0.5, 1.))
            .default_value(PickerColor::hsb(180., 0.5, 1.))
            .state(state_for_view.clone())
            .channel(ColorChannel::Saturation)
            .on_change(move |color, _, _| {
                if let Some(color) = color {
                    recorded
                        .borrow_mut()
                        .push(format!("{:.2}", color.saturation));
                }
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "up");
    assert_eq!(recorded.borrow().as_slice(), ["0.51"]);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "51%",
        "the input displays a percent without changing the normalized callback value"
    );
}

#[gpui::test]
fn normalized_channel_field_parses_plain_percent_digits(cx: &mut TestAppContext) {
    let recorded = events();
    let state = cx.new(|cx| InputState::with_value(cx, "50%"));
    let state_for_view = state;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ColorField::new("plain-percent", PickerColor::hsb(180., 0.5, 1.))
            .state(state_for_view.clone())
            .channel(ColorChannel::Saturation)
            .on_change(move |color, _, _| {
                recorded.borrow_mut().push(
                    color.map_or_else(|| "none".into(), |color| format!("{:.2}", color.saturation)),
                );
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "ctrl-a");
    cx.simulate_input("51");
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("0.51"),
        "a percent NumberField interprets plain 51 as 51%, not as an out-of-range 51"
    );
}

#[gpui::test]
fn channel_color_field_wheel_honors_is_wheel_disabled(cx: &mut TestAppContext) {
    let recorded = events();
    let enabled_state = cx.new(|cx| InputState::with_value(cx, "180"));
    let disabled_state = cx.new(|cx| InputState::with_value(cx, "180"));
    let enabled_for_view = enabled_state;
    let disabled_for_view = disabled_state;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let enabled = for_view.clone();
        let disabled = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorField::new("hue-wheel-live", PickerColor::hsb(180., 1., 1.))
                    .default_value(PickerColor::hsb(180., 1., 1.))
                    .state(enabled_for_view.clone())
                    .color_space(ColorSpace::Hsl)
                    .channel(ColorChannel::Hue)
                    .on_change(move |color, _, _| {
                        if let Some(color) = color {
                            enabled
                                .borrow_mut()
                                .push(format!("enabled:{:.0}", color.hue));
                        }
                    }),
            )
            .child(
                ColorField::new("hue-wheel-dead", PickerColor::hsb(180., 1., 1.))
                    .default_value(PickerColor::hsb(180., 1., 1.))
                    .state(disabled_for_view.clone())
                    .color_space(ColorSpace::Hsl)
                    .channel(ColorChannel::Hue)
                    .is_wheel_disabled(true)
                    .on_change(move |color, _, _| {
                        if let Some(color) = color {
                            disabled
                                .borrow_mut()
                                .push(format!("disabled:{:.0}", color.hue));
                        }
                    }),
            )
            .into_any_element()
    });

    // A channel field is a 36px input. The second sits at y=56..92 after the
    // explicit 20px gap. Positive vertical wheel delta increments in the
    // pinned `useNumberField`; `isWheelDisabled` suppresses the second one.
    click(cx, 60., 18.);
    wheel(cx, 60., 18., 1.);
    click(cx, 60., 74.);
    wheel(cx, 60., 74., 1.);
    assert_eq!(recorded.borrow().as_slice(), ["enabled:181"]);
}

#[gpui::test]
fn disabled_controls_and_read_only_channel_field_are_inert(cx: &mut TestAppContext) {
    let recorded = events();
    let state = cx.new(|cx| InputState::with_value(cx, "180"));
    let state_for_view = state;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let area = for_view.clone();
        let slider = for_view.clone();
        let field = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorArea::new("disabled-area", PickerColor::hsb(210., 0.5, 0.5))
                    .size(px(100.), px(60.))
                    .is_disabled(true)
                    .on_change(move |_, _, _| area.borrow_mut().push("area".into())),
            )
            .child(
                ColorSlider::new(
                    "disabled-slider",
                    PickerColor::hsb(180., 1., 1.),
                    ColorChannel::Hue,
                )
                .length(px(100.))
                .show_label(false)
                .is_disabled(true)
                .on_change(move |_, _, _| slider.borrow_mut().push("slider".into())),
            )
            .child(
                ColorField::new("readonly-field", PickerColor::hsb(180., 1., 1.))
                    .state(state_for_view.clone())
                    .channel(ColorChannel::Hue)
                    .is_read_only(true)
                    .on_change(move |_, _, _| field.borrow_mut().push("field".into())),
            )
            .into_any_element()
    });

    click(cx, 50., 30.);
    click(cx, 50., 88.);
    click(cx, 50., 134.);
    press(cx, "up");
    wheel(cx, 50., 134., 1.);
    assert!(recorded.borrow().is_empty());
}

fn submit_text(data: &FormData, name: &str) -> String {
    data.get(name)
        .map_or_else(|| "omitted".to_owned(), |value| value.as_text().to_string())
}

#[gpui::test]
fn color_slider_saturation_form_uses_percent_units(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let slider = ColorSlider::new(
            "saturation-form-units",
            PickerColor::hsb(0., 0.25, 1.),
            ColorChannel::Saturation,
        )
        .name("saturation");
        let form = Form::new().field(slider.form_field().expect("named saturation slider"));
        assert_eq!(
            submit_text(&form.data(cx), "saturation"),
            "25",
            "HeroUI's hidden range input submits saturation on its 0..100 scale"
        );
    });
}

fn submit_button(id: &'static str, submit: Submit) -> Button {
    Button::new(id)
        .label("Submit")
        .on_press(move |_, window, cx| submit(window, cx))
}

fn reset_button(id: &'static str, reset: Submit) -> Button {
    Button::new(id)
        .label("Reset")
        .on_press(move |_, window, cx| reset(window, cx))
}

/// React Aria ColorSlider submits the hidden range input's channel number.
/// A disabled input is not successful; reset restores `defaultValue`.
#[gpui::test]
fn uncontrolled_color_slider_form_reads_channel_after_pointer_change(cx: &mut TestAppContext) {
    let submitted = events();
    let for_view = submitted.clone();
    let seed = PickerColor::hsb(0., 1., 1.);
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let slider = ColorSlider::new("hue-live", seed, ColorChannel::Hue)
            .default_value(seed)
            .length(px(240.))
            .show_label(false)
            .name("hue");
        let form = Form::new()
            .field(slider.form_field().expect("named slider field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "hue"));
            });
        let submit = form.submit_handler();
        form.child(slider)
            .child(submit_button("hue-live-submit", submit))
            .into_any_element()
    });

    // The 240px footprint has 10px caps and 220px of value travel, so local
    // x=65 is 90°. The 20px track and 16px gap put submit at y=36..72.
    click(cx, 65., 10.);
    flush_frame(cx);
    click(cx, 60., 50.);
    assert_eq!(submitted.borrow().as_slice(), ["90"]);
}

#[gpui::test]
fn controlled_color_slider_form_waits_for_owner_acceptance(cx: &mut TestAppContext) {
    let submitted = events();
    let current = Rc::new(RefCell::new(PickerColor::hsb(0., 1., 1.)));
    let for_view = current;
    let submitted_for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let current = for_view.clone();
        let submitted = submitted_for_view.clone();
        let value = *current.borrow();
        let slider = ColorSlider::new("hue-owned", value, ColorChannel::Hue)
            .length(px(240.))
            .show_label(false)
            .name("hue")
            .on_change(move |color, _, _| {
                *current.borrow_mut() = *color;
            });
        let form = Form::new()
            .field(slider.form_field().expect("named slider field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "hue"));
            });
        let submit = form.submit_handler();
        form.child(slider)
            .child(submit_button("hue-owned-submit", submit))
            .into_any_element()
    });

    click(cx, 65., 10.);
    flush_frame(cx);
    click(cx, 60., 50.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["90"],
        "a controlled slider submits the channel only after the owner writes it back"
    );
}

#[gpui::test]
fn controlled_color_slider_form_keeps_owner_value_until_accepted(cx: &mut TestAppContext) {
    let submitted = events();
    let current = Rc::new(RefCell::new(PickerColor::hsb(0., 1., 1.)));
    let for_view = current;
    let submitted_for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let current = for_view.clone();
        let submitted = submitted_for_view.clone();
        let value = *current.borrow();
        let slider = ColorSlider::new("hue-ignored", value, ColorChannel::Hue)
            .length(px(240.))
            .show_label(false)
            .name("hue")
            .on_change(move |_, _, _| {});
        let form = Form::new()
            .field(slider.form_field().expect("named slider field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "hue"));
            });
        let submit = form.submit_handler();
        form.child(slider)
            .child(submit_button("hue-ignored-submit", submit))
            .into_any_element()
    });

    click(cx, 65., 10.);
    flush_frame(cx);
    click(cx, 60., 50.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["0"],
        "an owner that ignores onChange keeps the last accepted channel"
    );
}

#[gpui::test]
fn disabled_color_slider_is_not_a_successful_form_control(cx: &mut TestAppContext) {
    let submitted = events();
    let for_view = submitted.clone();
    let seed = PickerColor::hsb(90., 1., 1.);
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let slider = ColorSlider::new("hue-disabled", seed, ColorChannel::Hue)
            .default_value(seed)
            .length(px(240.))
            .show_label(false)
            .name("hue")
            .is_disabled(true);
        let form = Form::new()
            .field(
                slider
                    .form_field()
                    .expect("disabled field remains registered"),
            )
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "hue"));
            });
        let submit = form.submit_handler();
        form.child(slider)
            .child(submit_button("hue-disabled-submit", submit))
            .into_any_element()
    });

    click(cx, 60., 50.);
    assert_eq!(submitted.borrow().as_slice(), ["omitted"]);
}

#[gpui::test]
fn uncontrolled_color_slider_reset_restores_default_before_next_submit(cx: &mut TestAppContext) {
    let submitted = events();
    let for_view = submitted.clone();
    let seed = PickerColor::hsb(0., 1., 1.);
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let slider = ColorSlider::new("hue-reset", seed, ColorChannel::Hue)
            .default_value(seed)
            .length(px(240.))
            .show_label(false)
            .name("hue");
        let form = Form::new()
            .field(slider.form_field().expect("named slider field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "hue"));
            });
        let submit = form.submit_handler();
        let reset = form.reset_handler();
        form.child(slider)
            .child(submit_button("hue-reset-submit", submit))
            .child(reset_button("hue-reset-button", reset))
            .into_any_element()
    });

    click(cx, 65., 10.);
    flush_frame(cx);
    click(cx, 60., 50.);
    flush_frame(cx);
    click(cx, 60., 102.);
    flush_frame(cx);
    click(cx, 60., 50.);
    assert_eq!(submitted.borrow().as_slice(), ["90", "0"]);
}

#[gpui::test]
fn controlled_color_slider_reset_reports_the_initial_value_once(cx: &mut TestAppContext) {
    let changes = events();
    let current = Rc::new(RefCell::new(PickerColor::hsb(0., 1., 1.)));
    let for_view = current;
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let current = for_view.clone();
        let changes = changes_for_view.clone();
        let value = *current.borrow();
        let slider = ColorSlider::new("hue-controlled-reset", value, ColorChannel::Hue)
            .length(px(240.))
            .show_label(false)
            .name("hue")
            .on_change(move |color, _, _| {
                *current.borrow_mut() = *color;
                changes.borrow_mut().push(format!("{:.0}", color.hue));
            });
        let form = Form::new().field(slider.form_field().expect("named slider field"));
        let reset = form.reset_handler();
        form.child(slider)
            .child(reset_button("hue-controlled-reset-button", reset))
            .into_any_element()
    });

    click(cx, 65., 10.);
    flush_frame(cx);
    click(cx, 60., 50.);
    assert_eq!(changes.borrow().as_slice(), ["90", "0"]);
}

/// React Aria ColorField submits hex text, or the channel number when `channel`
/// is set. Disabled inputs are omitted; reset restores the seeded colour.
#[gpui::test]
fn uncontrolled_color_field_form_reads_hex_after_typing(cx: &mut TestAppContext) {
    let submitted = events();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state;
    let for_view = submitted.clone();
    let seed = PickerColor::from_hex("#FF0000").expect("red");
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let field = ColorField::new("brand-live", seed)
            .default_value(seed)
            .state(state_for_view.clone())
            .name("brand");
        let form = Form::new()
            .field(field.form_field().expect("named color field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "brand"));
            });
        let submit = form.submit_handler();
        form.child(field)
            .child(submit_button("brand-live-submit", submit))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("00ff00");
    flush_frame(cx);
    click(cx, 60., 70.);
    assert_eq!(submitted.borrow().as_slice(), ["#00FF00"]);
}

#[gpui::test]
fn uncontrolled_channel_color_field_form_reads_channel_after_step(cx: &mut TestAppContext) {
    let submitted = events();
    let state = cx.new(|cx| InputState::with_value(cx, "180"));
    let state_for_view = state;
    let for_view = submitted.clone();
    let seed = PickerColor::hsb(180., 1., 1.);
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let field = ColorField::new("hue-field-live", seed)
            .default_value(seed)
            .state(state_for_view.clone())
            .color_space(ColorSpace::Hsl)
            .channel(ColorChannel::Hue)
            .name("hue");
        let form = Form::new()
            .field(field.form_field().expect("named channel field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "hue"));
            });
        let submit = form.submit_handler();
        form.child(field)
            .child(submit_button("hue-field-live-submit", submit))
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "up");
    flush_frame(cx);
    click(cx, 60., 70.);
    assert_eq!(submitted.borrow().as_slice(), ["181"]);
}

#[gpui::test]
fn controlled_color_field_form_waits_for_owner_acceptance(cx: &mut TestAppContext) {
    let submitted = events();
    let current = Rc::new(RefCell::new(PickerColor::hsb(180., 1., 1.)));
    let state = cx.new(|cx| InputState::with_value(cx, "180"));
    let state_for_view = state;
    let for_view = current;
    let submitted_for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let current = for_view.clone();
        let submitted = submitted_for_view.clone();
        let value = *current.borrow();
        let field = ColorField::new("hue-field-owned", value)
            .state(state_for_view.clone())
            .color_space(ColorSpace::Hsl)
            .channel(ColorChannel::Hue)
            .name("hue")
            .on_change(move |color, _, _| {
                if let Some(color) = color {
                    *current.borrow_mut() = *color;
                }
            });
        let form = Form::new()
            .field(field.form_field().expect("named channel field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "hue"));
            });
        let submit = form.submit_handler();
        form.child(field)
            .child(submit_button("hue-field-owned-submit", submit))
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "up");
    flush_frame(cx);
    click(cx, 60., 70.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["181"],
        "a controlled field submits the channel only after the owner writes it back"
    );
}

#[gpui::test]
fn controlled_color_field_form_keeps_owner_value_until_accepted(cx: &mut TestAppContext) {
    let submitted = events();
    let current = Rc::new(RefCell::new(PickerColor::hsb(180., 1., 1.)));
    let state = cx.new(|cx| InputState::with_value(cx, "180"));
    let state_for_view = state;
    let for_view = current;
    let submitted_for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let current = for_view.clone();
        let submitted = submitted_for_view.clone();
        let value = *current.borrow();
        let field = ColorField::new("hue-field-ignored", value)
            .state(state_for_view.clone())
            .color_space(ColorSpace::Hsl)
            .channel(ColorChannel::Hue)
            .name("hue")
            .on_change(move |_, _, _| {});
        let form = Form::new()
            .field(field.form_field().expect("named channel field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "hue"));
            });
        let submit = form.submit_handler();
        form.child(field)
            .child(submit_button("hue-field-ignored-submit", submit))
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "up");
    flush_frame(cx);
    click(cx, 60., 70.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["180"],
        "an owner that ignores onChange keeps the last accepted channel"
    );
}

#[gpui::test]
fn disabled_color_field_is_not_a_successful_form_control(cx: &mut TestAppContext) {
    let submitted = events();
    let state = cx.new(|cx| InputState::with_value(cx, "#FF0000"));
    let state_for_view = state;
    let for_view = submitted.clone();
    let seed = PickerColor::from_hex("#FF0000").expect("red");
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let field = ColorField::new("brand-disabled", seed)
            .default_value(seed)
            .state(state_for_view.clone())
            .name("brand")
            .is_disabled(true);
        let form = Form::new()
            .field(
                field
                    .form_field()
                    .expect("disabled field remains registered"),
            )
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "brand"));
            });
        let submit = form.submit_handler();
        form.child(field)
            .child(submit_button("brand-disabled-submit", submit))
            .into_any_element()
    });

    click(cx, 60., 70.);
    assert_eq!(submitted.borrow().as_slice(), ["omitted"]);
}

#[gpui::test]
fn null_color_field_is_an_empty_required_form_value(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let field = ColorField::new("brand-empty", None::<PickerColor>)
            .default_value(None::<PickerColor>)
            .name("brand")
            .is_required(true);
        let form = Form::new().field(field.form_field().expect("named empty color field"));
        assert_eq!(submit_text(&form.data(cx), "brand"), "");
        assert_eq!(
            form.data(cx)
                .missing_required(&[gpui::SharedString::from("brand")]),
            vec![gpui::SharedString::from("brand")],
            "HeroUI's null Color value must fail native required validation"
        );
    });
}

#[gpui::test]
fn uncontrolled_color_field_reset_restores_default_before_next_submit(cx: &mut TestAppContext) {
    let submitted = events();
    let state = cx.new(|cx| InputState::with_value(cx, "180"));
    let state_for_view = state;
    let for_view = submitted.clone();
    let seed = PickerColor::hsb(180., 1., 1.);
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let field = ColorField::new("hue-field-reset", seed)
            .default_value(seed)
            .state(state_for_view.clone())
            .color_space(ColorSpace::Hsl)
            .channel(ColorChannel::Hue)
            .name("hue");
        let form = Form::new()
            .field(field.form_field().expect("named channel field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "hue"));
            });
        let submit = form.submit_handler();
        let reset = form.reset_handler();
        form.child(field)
            .child(submit_button("hue-field-reset-submit", submit))
            .child(reset_button("hue-field-reset-button", reset))
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "up");
    flush_frame(cx);
    click(cx, 60., 70.);
    flush_frame(cx);
    click(cx, 60., 122.);
    flush_frame(cx);
    click(cx, 60., 70.);
    assert_eq!(submitted.borrow().as_slice(), ["181", "180"]);
}

#[gpui::test]
fn controlled_color_field_reset_reports_the_initial_value_once(cx: &mut TestAppContext) {
    let changes = events();
    let current = Rc::new(RefCell::new(PickerColor::hsb(180., 1., 1.)));
    let state = cx.new(|cx| InputState::with_value(cx, "180"));
    let state_for_view = state;
    let for_view = current;
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let current = for_view.clone();
        let changes = changes_for_view.clone();
        let value = *current.borrow();
        let field = ColorField::new("hue-field-controlled-reset", value)
            .state(state_for_view.clone())
            .color_space(ColorSpace::Hsl)
            .channel(ColorChannel::Hue)
            .name("hue")
            .on_change(move |color, _, _| {
                if let Some(color) = color {
                    *current.borrow_mut() = *color;
                    changes.borrow_mut().push(format!("{:.0}", color.hue));
                }
            });
        let form = Form::new().field(field.form_field().expect("named channel field"));
        let reset = form.reset_handler();
        form.child(field)
            .child(reset_button("hue-field-controlled-reset-button", reset))
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "up");
    flush_frame(cx);
    click(cx, 60., 70.);
    assert_eq!(changes.borrow().as_slice(), ["181", "180"]);
}

#[gpui::test]
fn color_text_metrics_do_not_inherit_host_leading(cx: &mut TestAppContext) {
    for inherited in [12., 48.] {
        let heights = Rc::new(RefCell::new(Vec::new()));
        let seen = heights.clone();
        let cx = open_host(cx, move || {
            let seen = seen.clone();
            gpui::div()
                .flex()
                .flex_col()
                .items_start()
                .text_size(px(32.))
                .line_height(px(inherited))
                .on_children_prepainted(move |bounds, _, _| {
                    *seen.borrow_mut() = bounds.iter().map(|b| b.size.height).collect();
                })
                .child(ColorSlider::new(
                    "leading-slider",
                    PickerColor::hsb(0., 1., 1.),
                    ColorChannel::Hue,
                ))
                .child(herogpui_components::ColorPicker::new(
                    "leading-picker",
                    PickerColor::hsb(0., 1., 1.),
                ))
                .child(
                    ColorField::new("leading-field", PickerColor::hsb(0., 1., 1.)).suffix(
                        gpui::div()
                            .debug_selector(|| "leading-suffix".to_owned())
                            .child("HEX"),
                    ),
                )
                .into_any_element()
        });
        flush_frame(cx);
        assert_eq!(
            heights.borrow().as_slice(),
            [px(44.), px(24.), px(36.)],
            "host leading {inherited}"
        );
        assert_eq!(
            cx.debug_bounds("leading-suffix")
                .expect("suffix paints")
                .size
                .height,
            px(20.)
        );
    }
}

/// HeroUI's `colorName` must replace the generated hex accessible name on a
/// named swatch. The headless platform does not activate AccessKit, so the
/// source contract is the observable proof for this accessibility-only path.
#[test]
fn color_swatch_color_name_overrides_hex_accessible_name() {
    let source = source_scan::component_src("color_picker/swatch.rs");
    assert!(source.contains("color_name: Option<SharedString>"));
    assert!(source.contains("pub fn color_name"));
    assert!(source.contains("SharedString::from(self.color.to_hex())"));
    assert!(source.contains("a11y::Name::labelled(accessible_name)"));
}
