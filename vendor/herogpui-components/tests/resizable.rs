//! `ResizablePanelGroup` (HeroGPUI extension): panels share the group's
//! length by percentage; a handle drag (holding the pointer capture) or the arrow,
//! Home and End keys on a focused handle resize the two panels beside it
//! within their limits, controlled or uncontrolled.

use crate::harness;

use gpui::{point, prelude::*, px, Modifiers, MouseButton, TestAppContext, VisualTestContext};
use harness::{events, open_host, press, Events};
use herogpui_components::{Orientation, ResizablePanel, ResizablePanelGroup};

/// 500px shared by the panels plus two 8px handles.
const WIDTH: f32 = 516.;
const HEIGHT: f32 = 200.;

#[derive(Clone, Copy, Default)]
struct Config {
    vertical: bool,
    controlled: bool,
    disabled: bool,
}

fn host(cx: &mut TestAppContext, config: Config) -> (Events, &mut VisualTestContext) {
    let seen = events();
    let s = seen.clone();
    let cx = open_host(cx, move || {
        let s = s.clone();
        let mut group = ResizablePanelGroup::new("split")
            .panel(
                ResizablePanel::new()
                    .default_size(30.)
                    .min_size(20.)
                    .max_size(60.)
                    .child(gpui::div().child("One")),
            )
            .panel(
                ResizablePanel::new()
                    .default_size(40.)
                    .min_size(25.)
                    .child(gpui::div().child("Two")),
            )
            .panel(
                ResizablePanel::new()
                    .default_size(30.)
                    .child(gpui::div().child("Three")),
            )
            .is_disabled(config.disabled)
            .on_resize(move |sizes, _, _| {
                let text: Vec<String> = sizes.iter().map(|v| format!("{v:.0}")).collect();
                s.borrow_mut().push(text.join(","));
            });
        if config.vertical {
            group = group.orientation(Orientation::Vertical);
        }
        if config.controlled {
            group = group.sizes([30., 40., 30.]);
        }
        let (w, h) = if config.vertical {
            (HEIGHT, WIDTH)
        } else {
            (WIDTH, HEIGHT)
        };
        gpui::div()
            .w(px(w))
            .h(px(h))
            .child(group)
            .into_any_element()
    });
    (seen, cx)
}

fn frame(cx: &mut VisualTestContext) {
    for _ in 0..2 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn bounds(cx: &mut VisualTestContext, name: &str) -> gpui::Bounds<gpui::Pixels> {
    cx.debug_bounds(Box::leak(name.to_owned().into_boxed_str()))
        .unwrap_or_else(|| panic!("{name} laid out"))
}

/// The panels' lengths along the group's axis, rounded to whole pixels.
fn lengths(cx: &mut VisualTestContext, vertical: bool) -> Vec<f32> {
    (0..3)
        .map(|ix| {
            let b = bounds(cx, &format!("split-panel-{ix}"));
            f32::from(if vertical {
                b.size.height
            } else {
                b.size.width
            })
            .round()
        })
        .collect()
}

fn log(seen: &Events) -> Vec<String> {
    seen.borrow().clone()
}

#[gpui::test]
fn default_sizes_share_the_space_between_the_handles(cx: &mut TestAppContext) {
    let (_, cx) = host(cx, Config::default());
    frame(cx);
    assert_eq!(lengths(cx, false), [150., 200., 150.]);
    let handle = bounds(cx, "split-handle-0");
    assert_eq!(handle.size.width, px(8.));
    assert_eq!(handle.origin.x, px(150.));
}

#[gpui::test]
fn a_drag_resizes_the_pair_clamps_and_outlives_the_handle(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    let start = bounds(cx, "split-handle-0").center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    frame(cx);
    // 50px of the 500 the panels share is 10%.
    let step = point(start.x + px(50.), start.y);
    cx.simulate_mouse_move(step, Some(MouseButton::Left), Modifiers::none());
    frame(cx);
    assert_eq!(lengths(cx, false), [200., 150., 150.]);
    // Far past the limits, and off the group entirely: the second panel's
    // 25% minimum stops the first at 45% (under its own 60% maximum), and
    // the third panel never moves.
    let far = point(px(700.), px(400.));
    cx.simulate_mouse_move(far, Some(MouseButton::Left), Modifiers::none());
    frame(cx);
    assert_eq!(lengths(cx, false), [225., 125., 150.]);
    // The resize itself comes from window-level move listeners; what the
    // capture adds is that the handle's hitbox owns the pointer (and so its
    // resize cursor) off the group too.
    assert!(
        cx.update(|window, _| window.captured_hitbox().is_some()),
        "the drag holds the pointer capture outside the group"
    );
    // And back below the first panel's 20% minimum.
    let back = point(px(-50.), start.y);
    cx.simulate_mouse_move(back, Some(MouseButton::Left), Modifiers::none());
    frame(cx);
    assert_eq!(lengths(cx, false), [100., 250., 150.]);
    cx.simulate_mouse_up(back, MouseButton::Left, Modifiers::none());
    frame(cx);
    assert!(
        cx.update(|window, _| window.captured_hitbox().is_none()),
        "the release gives the pointer back"
    );
    // Released: moving no longer resizes.
    cx.simulate_mouse_move(step, None, Modifiers::none());
    frame(cx);
    assert_eq!(lengths(cx, false), [100., 250., 150.]);
    assert_eq!(log(&seen), ["40,30,30", "45,25,30", "20,50,30"]);
}

#[gpui::test]
fn arrow_keys_step_and_home_end_jump_to_the_limits(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    press(cx, "tab");
    frame(cx);
    for key in ["right", "shift-right", "home", "end", "down", "up"] {
        press(cx, key);
        frame(cx);
    }
    assert_eq!(lengths(cx, false), [225., 125., 150.]);
    // The second handle is the next tab stop and moves only its own pair;
    // Left is held by the second panel's 25% minimum.
    press(cx, "tab");
    frame(cx);
    press(cx, "left");
    frame(cx);
    press(cx, "right");
    frame(cx);
    assert_eq!(
        log(&seen),
        [
            "35,35,30", // Right: +5
            "45,25,30", // Shift+Right: +20, held by the second panel's minimum
            "20,50,30", // Home: the first panel's minimum
            "45,25,30", // End: as far as the pair allows; Up/Down are off-axis
            "45,30,25", // the second handle
        ]
    );
}

#[gpui::test]
fn a_vertical_group_resizes_with_up_and_down(cx: &mut TestAppContext) {
    let config = Config {
        vertical: true,
        ..Config::default()
    };
    let (seen, cx) = host(cx, config);
    frame(cx);
    assert_eq!(lengths(cx, true), [150., 200., 150.]);
    press(cx, "tab");
    frame(cx);
    press(cx, "right");
    frame(cx);
    press(cx, "down");
    frame(cx);
    assert_eq!(lengths(cx, true), [175., 175., 150.]);
    let start = bounds(cx, "split-handle-0").center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    frame(cx);
    let up = point(start.x, start.y - px(50.));
    cx.simulate_mouse_move(up, Some(MouseButton::Left), Modifiers::none());
    frame(cx);
    cx.simulate_mouse_up(up, MouseButton::Left, Modifiers::none());
    frame(cx);
    assert_eq!(log(&seen), ["35,35,30", "25,45,30"]);
}

#[gpui::test]
fn controlled_sizes_report_and_render_only_what_they_are_given(cx: &mut TestAppContext) {
    let config = Config {
        controlled: true,
        ..Config::default()
    };
    let (seen, cx) = host(cx, config);
    frame(cx);
    press(cx, "tab");
    frame(cx);
    press(cx, "right");
    frame(cx);
    press(cx, "right");
    frame(cx);
    assert_eq!(log(&seen), ["35,35,30", "35,35,30"]);
    assert_eq!(lengths(cx, false), [150., 200., 150.]);
}

#[gpui::test]
fn a_disabled_group_neither_drags_nor_takes_focus(cx: &mut TestAppContext) {
    let config = Config {
        disabled: true,
        ..Config::default()
    };
    let (seen, cx) = host(cx, config);
    frame(cx);
    let start = bounds(cx, "split-handle-0").center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    frame(cx);
    let step = point(start.x + px(50.), start.y);
    cx.simulate_mouse_move(step, Some(MouseButton::Left), Modifiers::none());
    frame(cx);
    cx.simulate_mouse_up(step, MouseButton::Left, Modifiers::none());
    press(cx, "tab");
    frame(cx);
    press(cx, "right");
    frame(cx);
    assert!(log(&seen).is_empty());
    assert_eq!(lengths(cx, false), [150., 200., 150.]);
}

/// A 516x200 host around whatever group `build` returns, logging every
/// resize. `build` runs on every render.
fn custom_host(
    cx: &mut TestAppContext,
    build: impl Fn() -> ResizablePanelGroup + 'static,
) -> (Events, &mut VisualTestContext) {
    let seen = events();
    let s = seen.clone();
    let cx = open_host(cx, move || {
        let s = s.clone();
        gpui::div()
            .w(px(WIDTH))
            .h(px(HEIGHT))
            .child(build().on_resize(move |sizes, _, _| {
                let text: Vec<String> = sizes.iter().map(|v| format!("{v:.0}")).collect();
                s.borrow_mut().push(text.join(","));
            }))
            .into_any_element()
    });
    (seen, cx)
}

fn three_panels() -> Vec<ResizablePanel> {
    vec![
        ResizablePanel::new().min_size(20.).max_size(60.),
        ResizablePanel::new().min_size(25.),
        ResizablePanel::new(),
    ]
}

#[gpui::test]
fn moves_between_frames_compare_with_the_current_sizes(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    let start = bounds(cx, "split-handle-0").center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    frame(cx);
    // To the first panel's 20% minimum, and a frame renders it.
    let low = point(start.x - px(100.), start.y);
    cx.simulate_mouse_move(low, Some(MouseButton::Left), Modifiers::none());
    frame(cx);
    // Two moves in one dispatch, before any frame: out to 30% and back past
    // the minimum, which clamps to the 20% the last frame rendered.
    let mid = point(start.x, start.y);
    cx.update(|window, cx| {
        for position in [mid, low] {
            window.dispatch_event(
                gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                    position,
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Modifiers::none(),
                }),
                cx,
            );
        }
    });
    frame(cx);
    assert_eq!(lengths(cx, false), [100., 250., 150.], "not left at 30%");
    cx.simulate_mouse_up(low, MouseButton::Left, Modifiers::none());
    frame(cx);
    assert_eq!(log(&seen), ["20,50,30", "30,40,30", "20,50,30"]);
}

#[gpui::test]
fn a_controlled_drag_reports_each_change_once(cx: &mut TestAppContext) {
    let config = Config {
        controlled: true,
        ..Config::default()
    };
    let (seen, cx) = host(cx, config);
    frame(cx);
    let start = bounds(cx, "split-handle-0").center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    frame(cx);
    let step = point(start.x + px(50.), start.y);
    cx.simulate_mouse_move(step, Some(MouseButton::Left), Modifiers::none());
    frame(cx);
    // The same position again reports nothing new; back to the start
    // reports the start, though the controlled sizes never moved from it.
    cx.simulate_mouse_move(step, Some(MouseButton::Left), Modifiers::none());
    frame(cx);
    cx.simulate_mouse_move(start, Some(MouseButton::Left), Modifiers::none());
    frame(cx);
    cx.simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
    frame(cx);
    assert_eq!(log(&seen), ["40,30,30", "30,40,30"]);
}

#[gpui::test]
fn default_sizes_start_inside_the_panels_limits(cx: &mut TestAppContext) {
    let (seen, cx) = custom_host(cx, || {
        ResizablePanelGroup::new("split").panels(vec![
            ResizablePanel::new()
                .default_size(10.)
                .min_size(20.)
                .child(gpui::div().child("One")),
            ResizablePanel::new()
                .default_size(90.)
                .child(gpui::div().child("Two")),
        ])
    });
    frame(cx);
    let first = f32::from(bounds(cx, "split-panel-0").size.width).round();
    // 20% of the 508px the two panels share.
    assert!((first - 102.).abs() < 0.5, "{first}");
    // Already at its minimum: Left (and Home) leave it there rather than
    // growing it.
    press(cx, "tab");
    frame(cx);
    press(cx, "left");
    frame(cx);
    press(cx, "home");
    frame(cx);
    assert!(log(&seen).is_empty(), "{:?}", log(&seen));
    press(cx, "right");
    frame(cx);
    assert_eq!(log(&seen), ["25,75"]);
}

#[gpui::test]
fn controlled_sizes_are_normalized_to_100(cx: &mut TestAppContext) {
    let (seen, cx) = custom_host(cx, || {
        ResizablePanelGroup::new("split")
            .panels(three_panels())
            .sizes([2., 1., 1.])
    });
    frame(cx);
    assert_eq!(lengths(cx, false), [250., 125., 125.]);
    press(cx, "tab");
    frame(cx);
    press(cx, "left");
    frame(cx);
    assert_eq!(log(&seen), ["45,30,25"]);
}

#[gpui::test]
fn all_zero_sizes_split_evenly(cx: &mut TestAppContext) {
    let (_, cx) = custom_host(cx, || {
        ResizablePanelGroup::new("split")
            .panels(three_panels())
            .sizes([0., 0., 0.])
    });
    frame(cx);
    assert_eq!(lengths(cx, false), [167., 167., 167.]);
}

#[gpui::test]
fn removing_panels_mid_drag_ends_the_drag(cx: &mut TestAppContext) {
    let count = std::rc::Rc::new(std::cell::Cell::new(3));
    let c = count.clone();
    let (seen, cx) = custom_host(cx, move || {
        ResizablePanelGroup::new("split").panels(
            (0..c.get()).map(|ix| ResizablePanel::new().child(gpui::div().child(format!("{ix}")))),
        )
    });
    frame(cx);
    let start = bounds(cx, "split-handle-1").center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    frame(cx);
    count.set(2);
    frame(cx);
    // The dragged handle is gone: a move neither panics nor resizes.
    let step = point(start.x + px(50.), start.y);
    cx.simulate_mouse_move(step, Some(MouseButton::Left), Modifiers::none());
    frame(cx);
    cx.simulate_mouse_up(step, MouseButton::Left, Modifiers::none());
    frame(cx);
    assert!(log(&seen).is_empty(), "{:?}", log(&seen));
}

// ---- pixel limits, collapsing, on_resize_end ---------------------------

/// Two panels over the 508px they share in a 516px host, logging `on_resize`
/// as `a,b` and `on_resize_end` as `end:a,b`.
fn pair_host(
    cx: &mut TestAppContext,
    first: impl Fn() -> ResizablePanel + 'static,
) -> (Events, &mut VisualTestContext) {
    let seen = events();
    let s = seen.clone();
    let cx = open_host(cx, move || {
        let (s, e) = (s.clone(), s.clone());
        let text = |sizes: &[f32]| {
            sizes
                .iter()
                .map(|v| format!("{v:.0}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        gpui::div()
            .w(px(WIDTH))
            .h(px(HEIGHT))
            .child(
                ResizablePanelGroup::new("split")
                    .panel(first().child(gpui::div().child("One")))
                    .panel(ResizablePanel::new().child(gpui::div().child("Two")))
                    .on_resize(move |sizes, _, _| s.borrow_mut().push(text(sizes)))
                    .on_resize_end(move |sizes, _, _| {
                        e.borrow_mut().push(format!("end:{}", text(sizes)));
                    }),
            )
            .into_any_element()
    });
    (seen, cx)
}

/// The first panel's width in whole pixels.
fn first_width(cx: &mut VisualTestContext) -> i32 {
    f32::from(bounds(cx, "split-panel-0").size.width).round() as i32
}

fn drag_handle(cx: &mut VisualTestContext, dx: f32) {
    let start = bounds(cx, "split-handle-0").center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    frame(cx);
    let to = point(start.x + px(dx), start.y);
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    frame(cx);
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    frame(cx);
}

#[gpui::test]
fn a_pixel_minimum_holds_at_start_on_keys_and_in_a_drag(cx: &mut TestAppContext) {
    let (seen, cx) = pair_host(cx, || {
        ResizablePanel::new()
            .default_size(10.)
            .min_size_px(px(127.))
    });
    frame(cx);
    // 127px of 508 is 25%: the 10% default starts there.
    assert_eq!(first_width(cx), 127);
    press(cx, "tab");
    frame(cx);
    press(cx, "left");
    frame(cx);
    press(cx, "home");
    frame(cx);
    drag_handle(cx, -200.);
    assert_eq!(first_width(cx), 127);
    assert!(log(&seen).is_empty(), "{:?}", log(&seen));
}

#[gpui::test]
fn a_pixel_maximum_caps_a_drag(cx: &mut TestAppContext) {
    let (seen, cx) = pair_host(cx, || ResizablePanel::new().max_size_px(px(381.)));
    frame(cx);
    drag_handle(cx, 400.);
    // 381px of 508 is 75%.
    assert_eq!(first_width(cx), 381);
    assert_eq!(log(&seen), ["75,25", "end:75,25"]);
}

#[gpui::test]
fn a_collapsible_panel_collapses_past_halfway_and_expands_back(cx: &mut TestAppContext) {
    let (seen, cx) = pair_host(cx, || {
        ResizablePanel::new()
            .default_size(40.)
            .min_size(20.)
            .collapsible(true)
    });
    frame(cx);
    // 40% -> 12%: over halfway (10%) between 0 and the 20% minimum, so
    // the drag holds at the minimum.
    drag_handle(cx, -142.);
    assert_eq!(first_width(cx), 102);
    // 20% -> 8%: past halfway, it collapses.
    drag_handle(cx, -61.);
    assert_eq!(first_width(cx), 0);
    // 0% -> 11%: past halfway back, it expands to the minimum.
    drag_handle(cx, 56.);
    assert_eq!(first_width(cx), 102);
    assert_eq!(
        log(&seen),
        [
            "20,80",
            "end:20,80",
            "0,100",
            "end:0,100",
            "20,80",
            "end:20,80"
        ]
    );
}

#[gpui::test]
fn enter_collapses_and_restores_and_keys_end_each_resize(cx: &mut TestAppContext) {
    let (seen, cx) = pair_host(cx, || {
        ResizablePanel::new()
            .default_size(40.)
            .min_size(20.)
            .collapsible(true)
    });
    frame(cx);
    press(cx, "tab");
    frame(cx);
    for key in ["right", "enter", "enter", "left", "left", "left"] {
        press(cx, key);
        frame(cx);
    }
    assert_eq!(
        log(&seen),
        [
            "45,55",
            "end:45,55",
            // Enter collapses the panel before the handle...
            "0,100",
            "end:0,100",
            // ...and restores the size it had.
            "45,55",
            "end:45,55",
            "40,60",
            "end:40,60",
            "35,65",
            "end:35,65",
            "30,70",
            "end:30,70",
        ]
    );
    // At the minimum an arrow toward it collapses, and back expands to it.
    for key in ["home", "right", "left", "left"] {
        press(cx, key);
        frame(cx);
    }
    assert_eq!(
        log(&seen)[12..],
        [
            "0,100",
            "end:0,100", // Home: the collapsed size is the lowest
            "20,80",
            "end:20,80", // Right from collapsed: the minimum
            "0,100",
            "end:0,100", // Left from the minimum: collapsed
        ]
    );
}

#[gpui::test]
fn a_press_without_a_move_does_not_end_a_resize(cx: &mut TestAppContext) {
    let (seen, cx) = pair_host(cx, ResizablePanel::new);
    frame(cx);
    drag_handle(cx, 0.);
    assert!(log(&seen).is_empty(), "{:?}", log(&seen));
    // A drag reports every move but ends once, with the final sizes.
    let start = bounds(cx, "split-handle-0").center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    frame(cx);
    for dx in [25.4, 50.8] {
        let to = point(start.x + px(dx), start.y);
        cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
        frame(cx);
    }
    cx.simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
    frame(cx);
    assert_eq!(log(&seen), ["55,45", "60,40", "end:60,40"]);
}
