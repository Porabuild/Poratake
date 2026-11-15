//! `TitleBar` and `WindowBorder` (HeroGPUI extension): drawn controls report
//! their window actions through the hook, a press-and-move on the empty bar
//! is one window move, a plain click is none, a double-click zooms, presses
//! on the bar's children stay theirs, the platform `Auto` controls, and the
//! border's pass-through under server-side decorations.
//!
//! The headless test platform leaves `start_window_move`, `zoom` and
//! `minimize` unimplemented, so every test intercepts the actions; the
//! platform calls themselves are `WindowAction::perform`.

use crate::harness;

use gpui::{
    point, prelude::*, px, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    TestAppContext, VisualTestContext,
};
use harness::{events, open_host, still, Events};
use herogpui_components::{Button, TitleBar, TitleBarControls, WindowBorder};

fn host(cx: &mut TestAppContext, controls: TitleBarControls) -> (Events, &mut VisualTestContext) {
    still();
    let seen = events();
    let s = seen.clone();
    let cx = open_host(cx, move || {
        let (act, share) = (s.clone(), s.clone());
        WindowBorder::new()
            .child(
                TitleBar::new("chrome")
                    .title("Untitled")
                    .controls(controls)
                    .on_window_action(move |action, _, _| {
                        act.borrow_mut().push(format!("{action:?}"));
                    })
                    .child(
                        gpui::div().debug_selector(|| "share".into()).child(
                            Button::new("share")
                                .label("Share")
                                .on_press(move |_, _, _| share.borrow_mut().push("share".into())),
                        ),
                    ),
            )
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

fn center(cx: &mut VisualTestContext, name: &'static str) -> gpui::Point<gpui::Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} laid out"))
        .center()
}

fn down(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, clicks: usize) {
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: clicks,
        first_mouse: false,
    });
    frame(cx);
}

fn up(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, clicks: usize) {
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: clicks,
    });
    frame(cx);
}

fn drag_to(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>) {
    cx.simulate_event(MouseMoveEvent {
        position: at,
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    });
    frame(cx);
}

/// A point on the bar's empty drag area, right of the title and the child.
fn empty_bar(cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    let bar = cx.debug_bounds("chrome-title-bar").expect("the bar");
    point(px(600.), bar.center().y)
}

fn log(seen: &Events) -> Vec<String> {
    seen.borrow().clone()
}

#[gpui::test]
fn custom_controls_report_minimize_zoom_and_close(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, TitleBarControls::Custom);
    frame(cx);
    for part in ["chrome-minimize", "chrome-zoom", "chrome-close"] {
        let at = center(cx, part);
        cx.simulate_click(at, Modifiers::none());
        frame(cx);
    }
    assert_eq!(log(&seen), ["Minimize", "Zoom", "Close"]);
    // The controls sit at the end of the bar.
    let close = cx.debug_bounds("chrome-close").unwrap();
    let bar = cx.debug_bounds("chrome-title-bar").unwrap();
    assert_eq!(close.right(), bar.right());
}

#[gpui::test]
fn a_press_and_move_on_the_empty_bar_is_one_window_move(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, TitleBarControls::Hidden);
    frame(cx);
    let at = empty_bar(cx);
    // A plain click moves nothing.
    down(cx, at, 1);
    up(cx, at, 1);
    assert!(log(&seen).is_empty());
    // Press, then move: one move, however far the pointer goes.
    down(cx, at, 1);
    drag_to(cx, point(at.x + px(5.), at.y));
    drag_to(cx, point(at.x + px(40.), at.y + px(3.)));
    up(cx, point(at.x + px(40.), at.y + px(3.)), 1);
    assert_eq!(log(&seen), ["Move"]);
}

#[gpui::test]
fn a_double_click_on_the_empty_bar_zooms(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, TitleBarControls::Hidden);
    frame(cx);
    let at = empty_bar(cx);
    down(cx, at, 1);
    up(cx, at, 1);
    down(cx, at, 2);
    // The second press of a double-click does not arm a move.
    drag_to(cx, point(at.x + px(10.), at.y));
    up(cx, at, 2);
    let expected: &[&str] = if cfg!(target_os = "windows") {
        // The OS zooms on its caption there; the bar reports nothing.
        &[]
    } else {
        &["DoubleClick"]
    };
    assert_eq!(log(&seen), expected);
}

#[gpui::test]
fn presses_on_the_bars_children_stay_theirs(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, TitleBarControls::Hidden);
    frame(cx);
    let at = center(cx, "share");
    cx.simulate_click(at, Modifiers::none());
    frame(cx);
    assert_eq!(log(&seen), ["share"]);
    down(cx, at, 1);
    drag_to(cx, point(at.x + px(30.), at.y));
    up(cx, point(at.x + px(30.), at.y), 1);
    assert!(
        !log(&seen).iter().any(|entry| entry == "Move"),
        "dragging from a child is not a window move: {:?}",
        log(&seen)
    );
}

#[gpui::test]
fn auto_controls_follow_the_host_platform(cx: &mut TestAppContext) {
    let (_seen, cx) = host(cx, TitleBarControls::Auto);
    frame(cx);
    // The test platform reports server-side decorations, so Linux draws
    // none either; Windows draws OS-performed buttons.
    let drawn = cx.debug_bounds("chrome-close").is_some();
    assert_eq!(drawn, cfg!(target_os = "windows"));
    assert_eq!(
        cx.debug_bounds("chrome-minimize").is_none(),
        !cfg!(target_os = "windows")
    );
    // macOS reserves the traffic lights' width before the title.
    let bar = cx.debug_bounds("chrome-title-bar").unwrap();
    let share = cx.debug_bounds("share").unwrap();
    if cfg!(target_os = "macos") {
        assert!(share.left() - bar.left() > herogpui_components::TRAFFIC_LIGHTS_WIDTH);
    }
}

#[gpui::test]
fn hidden_controls_draw_nothing_and_the_border_passes_through(cx: &mut TestAppContext) {
    let (_seen, cx) = host(cx, TitleBarControls::Hidden);
    frame(cx);
    assert!(cx.debug_bounds("chrome-close").is_none());
    // Server-side decorations (the test platform's): no inset, the bar starts
    // at the window's top-left corner and spans it.
    let bar = cx.debug_bounds("chrome-title-bar").unwrap();
    assert_eq!(bar.origin, point(px(0.), px(0.)));
    assert_eq!(bar.size.height, herogpui_components::TITLE_BAR_HEIGHT);
}
