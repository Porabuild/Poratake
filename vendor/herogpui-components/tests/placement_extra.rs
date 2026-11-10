//! The tooltip's delays, and the slider's other axis.
//!
//! These four tests were lost to a bad edit of `placement.rs` and are rebuilt
//! here rather than spliced back, so the file that holds them is the file that
//! was written for them. Everything they need is derived from the components'
//! own constants, and the arithmetic is in a comment at each site.
//!
//! Two harness facts they depend on, both recorded in AGENTS.md: a mouse event
//! hit-tests the *last rendered frame*, so every press, move and clock
//! advance is followed by a redraw; and reduced motion has to be set before the
//! first frame, because flipping it later rebuilds the animated wrapper and the
//! click in flight is lost.

use crate::harness;

use std::time::Duration;

use gpui::{point, prelude::*, px, Modifiers, MouseButton, TestAppContext, VisualTestContext};
use harness::{events, open_host, press, tooltip_open_probe, Events};
use herogpui_components::{Button, Orientation, Slider, Tooltip, TooltipPlacement};
use herogpui_theme::ActiveTheme;

/// Pins the layout by enabling reduced motion **before** the first frame.
fn still() {
    harness::still();
}

/// Pushes the pending frame through: events hit-test the last rendered frame.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// A drag is down, one move with the button held, then up -- a single jump
/// lands as a click and the component sees no motion at all.
fn drag(cx: &mut VisualTestContext, from: (f32, f32), to: (f32, f32)) {
    let modifiers = Modifiers::none();
    cx.simulate_mouse_down(point(px(from.0), px(from.1)), MouseButton::Left, modifiers);
    flush_frame(cx);
    cx.simulate_mouse_move(point(px(to.0), px(to.1)), MouseButton::Left, modifiers);
    flush_frame(cx);
    cx.simulate_mouse_up(point(px(to.0), px(to.1)), MouseButton::Left, modifiers);
    flush_frame(cx);
}

/// One press with no motion, which a slider answers by jumping to the point.
fn press_at(cx: &mut VisualTestContext, x: f32, y: f32) {
    let modifiers = Modifiers::none();
    cx.simulate_mouse_down(point(px(x), px(y)), MouseButton::Left, modifiers);
    flush_frame(cx);
    cx.simulate_mouse_up(point(px(x), px(y)), MouseButton::Left, modifiers);
    flush_frame(cx);
}

/// The tooltip's own open flag, read from inside a render phase.
///
/// `TooltipHover` lives in `Window::use_keyed_state` under the tooltip's id --
/// but the component is a `RenderOnce`, so the derive wraps it in
/// `Component<Tooltip>` and gpui pushes that type name onto the element-id
/// stack before `render` runs. A probe asking for the same key from outside
/// that wrapper resolves to a *different* slot and would read a permanent
/// `false`, so it has to enter the same id path first. `use_keyed_state` is
/// only legal during layout/prepaint/paint, which is why the probe rides in a
/// zero-size `canvas`.
fn last(seen: &Events) -> String {
    seen.borrow().last().cloned().unwrap_or_default()
}

/// v3's tooltip waits before it appears and waits again before it goes.
///
/// `layout.rs` carries both delays (`--tooltip-delay: 1500ms` and
/// `--tooltip-close-delay: 500ms`), and the test reads them from the theme
/// rather than hardcoding, so a token change moves the test with it. The test
/// clock never advances on its own: every wait is an explicit
/// `advance_clock`, and the assertion either side of it is what proves the
/// delay is a delay rather than a coincidence.
#[gpui::test]
fn tooltip_hover_shows_after_v3_delay_and_leave_hides_after_close_delay(cx: &mut TestAppContext) {
    still();
    let seen = events();
    let for_view = seen.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .child(tooltip_open_probe("pl-tt", for_view.clone(), false))
            .child(
                Tooltip::new("Saved")
                    .id("pl-tt")
                    .child(gpui::div().id("pl-tt-trigger").w(px(120.)).h(px(36.))),
            )
            .into_any_element()
    });

    let (open_ms, close_ms) = cx.update(|_, cx| {
        let layout = cx.layout();
        (layout.tooltip_delay_ms, layout.tooltip_close_delay_ms)
    });

    // The trigger is a 120x36 box at the origin (the probe canvas above it is
    // zero-size), so its centre is (60, 18).
    cx.simulate_mouse_move(point(px(60.), px(18.)), None, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        last(&seen),
        "open:false",
        "a hover must not show the tip before v3's open delay"
    );

    // Just short of the delay it is still closed; past it, it is open.
    cx.executor()
        .advance_clock(Duration::from_millis(open_ms - 100));
    flush_frame(cx);
    assert_eq!(
        last(&seen),
        "open:false",
        "the tip must still be hidden 100ms before the delay elapses"
    );

    cx.executor().advance_clock(Duration::from_millis(200));
    flush_frame(cx);
    assert_eq!(
        last(&seen),
        "open:true",
        "the tip must appear once the open delay has elapsed"
    );

    // Leaving starts the close delay, which is the shorter of the two.
    cx.simulate_mouse_move(point(px(600.), px(600.)), None, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        last(&seen),
        "open:true",
        "a leave must not hide the tip before the close delay"
    );

    cx.executor()
        .advance_clock(Duration::from_millis(close_ms + 100));
    flush_frame(cx);
    assert_eq!(
        last(&seen),
        "open:false",
        "the tip must go once the close delay has elapsed"
    );
}

#[gpui::test]
fn tooltip_trigger_press_closes_until_a_fresh_hover(cx: &mut TestAppContext) {
    still();
    let seen = events();
    let for_view = seen.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .child(tooltip_open_probe("pl-tt-press", for_view.clone(), false))
            .child(
                Tooltip::new("Press tip")
                    .id("pl-tt-press")
                    .placement(TooltipPlacement::Bottom)
                    .delay(0)
                    .close_delay(500)
                    .child(gpui::div().w(px(120.)).h(px(36.))),
            )
            .into_any_element()
    });

    cx.simulate_mouse_move(point(px(60.), px(18.)), None, Modifiers::none());
    flush_frame(cx);
    assert_eq!(last(&seen), "open:true");

    press_at(cx, 60., 18.);
    assert_eq!(last(&seen), "open:false", "press hides the tip immediately");

    cx.simulate_mouse_move(point(px(600.), px(600.)), None, Modifiers::none());
    flush_frame(cx);
    cx.simulate_mouse_move(point(px(60.), px(18.)), None, Modifiers::none());
    flush_frame(cx);
    assert_eq!(last(&seen), "open:true", "a fresh hover may open it again");

    // The surface is a sibling of the trigger listener, matching RAC's
    // portal: pressing the open tip itself must not count as trigger press.
    press_at(cx, 20., 50.);
    assert_eq!(
        last(&seen),
        "open:true",
        "pressing the tip must not close it as a trigger press"
    );
    cx.executor().advance_clock(Duration::from_millis(600));
    flush_frame(cx);
    assert_eq!(
        last(&seen),
        "open:false",
        "leaving the trigger still closes after the delay"
    );
}

#[gpui::test]
fn tooltip_child_focus_opens_once_and_pointer_focus_stays_silent(cx: &mut TestAppContext) {
    still();
    let seen = events();
    let for_view = seen.clone();
    let pressed = events();
    let for_press = pressed.clone();
    let cx =
        open_host(cx, move || {
            let for_press = for_press.clone();
            gpui::div()
                .child(tooltip_open_probe("pl-tt-focus", for_view.clone(), true))
                .child(
                    Tooltip::new("Focus tip")
                        .id("pl-tt-focus")
                        .trigger(herogpui_components::TooltipTrigger::Focus)
                        .child(Button::new("pl-tt-focus-button").label("Focus").on_press(
                            move |_, _, _| for_press.borrow_mut().push("pressed".into()),
                        )),
                )
                .child(Button::new("pl-tt-after").label("After"))
                .into_any_element()
        });

    press(cx, "tab");
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(
        last(&seen),
        "open:true",
        "Tab reaches the caller's trigger and opens its tooltip"
    );

    press(cx, "j");
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(
        last(&seen),
        "open:false",
        "any trigger keydown dismisses an open tooltip"
    );

    // Start a new pointer-focus session. A later key changes the app-wide
    // input modality, but React Aria samples focus-visible when focus arrives,
    // so it must not synthesize a tooltip halfway through this session.
    press(cx, "tab");
    press_at(cx, 60., 18.);
    press(cx, "j");
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&seen), "open:false");
    press(cx, "enter");
    assert_eq!(
        pressed.borrow().as_slice(),
        ["pressed", "pressed"],
        "the pointer press activates once and retained focus lets Enter activate again"
    );
}

#[gpui::test]
fn tooltip_sequence_is_exclusive_and_reuses_global_warmup(cx: &mut TestAppContext) {
    still();
    let first_seen = events();
    let second_seen = events();
    let third_seen = events();
    let first_probe = first_seen.clone();
    let second_probe = second_seen.clone();
    let third_probe = third_seen.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .child(tooltip_open_probe(
                "pl-tt-first",
                first_probe.clone(),
                false,
            ))
            .child(
                Tooltip::new("First")
                    .id("pl-tt-first")
                    .delay(100)
                    .close_delay(650)
                    .child(gpui::div().w(px(120.)).h(px(36.))),
            )
            .child(tooltip_open_probe(
                "pl-tt-second",
                second_probe.clone(),
                false,
            ))
            .child(
                Tooltip::new("Second")
                    .id("pl-tt-second")
                    .delay(100)
                    .close_delay(650)
                    .child(gpui::div().w(px(120.)).h(px(36.))),
            )
            .child(tooltip_open_probe(
                "pl-tt-third",
                third_probe.clone(),
                false,
            ))
            .child(
                Tooltip::new("Third")
                    .id("pl-tt-third")
                    .delay(100)
                    .close_delay(650)
                    .child(gpui::div().w(px(120.)).h(px(36.))),
            )
            .into_any_element()
    });

    cx.simulate_mouse_move(point(px(60.), px(18.)), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(100));
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&first_seen), "open:true");
    assert_eq!(last(&second_seen), "open:false");

    // The first open warms the app-wide manager. Moving directly to a second
    // tooltip closes the first and opens the second without another 100ms.
    cx.simulate_mouse_move(point(px(180.), px(18.)), None, Modifiers::none());
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&first_seen), "open:false");
    assert_eq!(last(&second_seen), "open:true");

    // GPUI reports B-in before A-out for this sibling order. Parking on B
    // beyond A's stale 650ms deadline must not cool the live sequence: C is
    // still immediate when the pointer finally moves again.
    cx.executor().advance_clock(Duration::from_millis(700));
    flush_frame(cx);
    cx.simulate_mouse_move(point(px(300.), px(18.)), None, Modifiers::none());
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&second_seen), "open:false");
    assert_eq!(last(&third_seen), "open:true");

    // A custom close delay longer than 500ms also extends the global cooldown.
    cx.simulate_mouse_move(point(px(600.), px(600.)), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(500));
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&third_seen), "open:true");

    cx.simulate_mouse_move(point(px(60.), px(18.)), None, Modifiers::none());
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&first_seen), "open:true");
    assert_eq!(last(&third_seen), "open:false");

    // After the full 650ms, a new sequence is cold again.
    cx.simulate_mouse_move(point(px(600.), px(600.)), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(650));
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&first_seen), "open:false");

    cx.simulate_mouse_move(point(px(60.), px(18.)), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(60));
    flush_frame(cx);
    assert_eq!(last(&first_seen), "open:false");

    // Switching while still cold cancels the first pending timer and starts a
    // full delay for the second; the first timer's remaining 40ms cannot win.
    cx.simulate_mouse_move(point(px(180.), px(18.)), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(40));
    flush_frame(cx);
    assert_eq!(last(&first_seen), "open:false");
    assert_eq!(last(&second_seen), "open:false");
    cx.executor().advance_clock(Duration::from_millis(60));
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&first_seen), "open:false");
    assert_eq!(last(&second_seen), "open:true");
}

#[gpui::test]
fn focus_only_tooltip_pointer_is_inert_and_does_not_warm_sequence(cx: &mut TestAppContext) {
    still();
    let focus_seen = events();
    let hover_seen = events();
    let focus_probe = focus_seen.clone();
    let hover_probe = hover_seen.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .child(tooltip_open_probe(
                "pl-tt-focus-only",
                focus_probe.clone(),
                false,
            ))
            .child(
                Tooltip::new("Focus only")
                    .id("pl-tt-focus-only")
                    .trigger(herogpui_components::TooltipTrigger::Focus)
                    .delay(50)
                    .child(gpui::div().w(px(120.)).h(px(36.))),
            )
            .child(tooltip_open_probe(
                "pl-tt-after-focus-only",
                hover_probe.clone(),
                false,
            ))
            .child(
                Tooltip::new("Hover")
                    .id("pl-tt-after-focus-only")
                    .delay(50)
                    .child(gpui::div().w(px(120.)).h(px(36.))),
            )
            .into_any_element()
    });

    cx.simulate_mouse_move(point(px(60.), px(18.)), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(50));
    flush_frame(cx);
    assert_eq!(last(&focus_seen), "open:false");

    cx.simulate_mouse_move(point(px(180.), px(18.)), None, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        last(&hover_seen),
        "open:false",
        "pointer time over a focus-only trigger must not warm the next tooltip"
    );
    cx.executor().advance_clock(Duration::from_millis(50));
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&hover_seen), "open:true");
}

#[gpui::test]
fn pointer_leave_does_not_cool_a_focus_only_tooltip(cx: &mut TestAppContext) {
    still();
    let focus_seen = events();
    let hover_seen = events();
    let focus_probe = focus_seen.clone();
    let hover_probe = hover_seen.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .child(tooltip_open_probe(
                "pl-tt-focus-warm",
                focus_probe.clone(),
                true,
            ))
            .child(
                Tooltip::new("Focus warm")
                    .id("pl-tt-focus-warm")
                    .trigger(herogpui_components::TooltipTrigger::Focus)
                    .child(
                        gpui::div()
                            .w(px(120.))
                            .h(px(36.))
                            .child(Button::new("pl-tt-focus-warm-button").label("Focus")),
                    ),
            )
            .child(tooltip_open_probe(
                "pl-tt-hover-after-focus",
                hover_probe.clone(),
                false,
            ))
            .child(
                Tooltip::new("Hover after focus")
                    .id("pl-tt-hover-after-focus")
                    .delay(100)
                    .child(gpui::div().w(px(120.)).h(px(36.))),
            )
            .into_any_element()
    });

    press(cx, "tab");
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&focus_seen), "open:true");

    cx.simulate_mouse_move(point(px(60.), px(18.)), None, Modifiers::none());
    cx.simulate_mouse_move(point(px(600.), px(600.)), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(500));
    flush_frame(cx);
    assert_eq!(last(&focus_seen), "open:true");

    cx.simulate_mouse_move(point(px(180.), px(18.)), None, Modifiers::none());
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&focus_seen), "open:false");
    assert_eq!(
        last(&hover_seen),
        "open:true",
        "the focus-open sequence stays warm across irrelevant pointer leave"
    );
}

#[gpui::test]
fn default_tooltip_pointer_leave_closes_its_keyboard_focus_session(cx: &mut TestAppContext) {
    still();
    let seen = events();
    let probe = seen.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .child(tooltip_open_probe("pl-tt-focus-leave", probe.clone(), true))
            .child(
                Tooltip::new("Focus then leave")
                    .id("pl-tt-focus-leave")
                    .close_delay(100)
                    .child(
                        gpui::div()
                            .w(px(120.))
                            .h(px(36.))
                            .child(Button::new("pl-tt-focus-leave-button").label("Focus")),
                    ),
            )
            .into_any_element()
    });

    press(cx, "tab");
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&seen), "open:true");

    cx.simulate_mouse_move(point(px(60.), px(18.)), None, Modifiers::none());
    cx.simulate_mouse_move(point(px(600.), px(600.)), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(99));
    flush_frame(cx);
    assert_eq!(last(&seen), "open:true");
    cx.executor().advance_clock(Duration::from_millis(1));
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(last(&seen), "open:false");
}

/// A vertical slider's keyboard: up is *more*.
///
/// The axis inverts the pointer geometry but not the keys -- Up/Right raise the
/// value and Down/Left lower it either way, which is what React Aria does and
/// what a caller expects from `Home`/`End`. Values are integral and compared as
/// strings: `clippy::float_cmp` is denied, and a formatted comparison also
/// documents exactly what the callback reported.
#[gpui::test]
fn vertical_slider_keyboard_steps_up_and_clamps(cx: &mut TestAppContext) {
    let seen = events();
    let for_view = seen.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(60.))
            .h(px(400.))
            .child(
                Slider::new("pl-vslider", 50.)
                    .orientation(Orientation::Vertical)
                    .default_value(50.)
                    .min_value(0.)
                    .max_value(100.)
                    .step(10.)
                    .on_change(move |v, _, _| recorded.borrow_mut().push(format!("{v}"))),
            )
            .into_any_element()
    });

    // One Tab reaches the slider's track (its only focusable), then the keys
    // step by `step` and Home/End reach the bounds.
    press(cx, "tab");
    flush_frame(cx);
    press(cx, "up");
    press(cx, "up");
    press(cx, "down");
    press(cx, "end");
    press(cx, "up");
    press(cx, "home");
    press(cx, "down");
    assert_eq!(
        seen.borrow().as_slice(),
        ["60", "70", "60", "100", "0"],
        "up must raise the value, down lower it, both ends must clamp, and a \
         clamped no-op must not report a change"
    );
}

/// A vertical slider's *pointer* geometry: the fraction is a fraction of the
/// track's height, measured up from its bottom edge.
///
/// Before the fix, `set_from_pointer`'s predecessor divided `-y` by the track's
/// **width** after subtracting its `origin.x`, so a vertical track (20px wide,
/// y growing downward) produced a negative numerator and every press and every
/// drag clamped to the minimum -- a vertical slider could not be moved by the
/// pointer at all.
#[gpui::test]
fn vertical_slider_drag_is_derived_from_track_height(cx: &mut TestAppContext) {
    let seen = events();
    let for_view = seen.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(60.))
            .h(px(400.))
            .child(
                Slider::new("pl-vdrag", 0.)
                    .orientation(Orientation::Vertical)
                    .default_value(0.)
                    .min_value(0.)
                    .max_value(100.)
                    .step(25.)
                    .on_change(move |v, _, _| recorded.borrow_mut().push(format!("{v}"))),
            )
            .into_any_element()
    });

    // A vertical track is 20px wide and a fixed 160px tall, sat at the
    // wrapper's top-left, so the enclosing column's height does not enter the
    // arithmetic. x = 10 is its centre line, and zero
    // is the *bottom*: y = 160 reads 0 and y = 0 reads 100, so the 25 mark sits
    // at y = 160 - 0.25*160 = 120 and the 75 mark at y = 40.
    press_at(cx, 10., 120.);
    assert_eq!(
        seen.borrow().as_slice(),
        ["25"],
        "a press a quarter of the way up the track must report 25"
    );

    drag(cx, (10., 120.), (10., 40.));
    assert_eq!(
        seen.borrow().as_slice(),
        ["25", "75"],
        "the unchanged drag press must stay silent, then the pull up reports 75"
    );
}

/// `step` snaps the pointer to marks, and both ends clamp.
///
/// A horizontal slider is `w_full`, so the 600px wrapper is what makes the
/// track's length knowable: with min 0, max 100 and step 25 the marks sit
/// every 150px. A drag's press only reports when it changes the value, and a
/// drag whose end point leaves the track's hitbox never delivers the move at
/// all -- 620 on a 600px track silently did nothing.
#[gpui::test]
fn slider_step_snaps_pointer_to_marks_and_clamps_both_ends(cx: &mut TestAppContext) {
    let seen = events();
    let for_view = seen.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("pl-steps", 50.)
                    .default_value(50.)
                    .min_value(0.)
                    .max_value(100.)
                    .step(25.)
                    .on_change(move |v, _, _| recorded.borrow_mut().push(format!("{v}"))),
            )
            .into_any_element()
    });

    // x = 5 is 0.8 on the range and snaps to the 0 mark; the first drag runs
    // 80 (13.3 -> 25) to 460 (76.7 -> 75); the second starts on the current
    // 75 mark, so its unchanged press stays silent before the pull to 590
    // (98.3) clamps at 100.
    press_at(cx, 5., 10.);
    drag(cx, (80., 10.), (460., 10.));
    drag(cx, (460., 10.), (590., 10.));
    assert_eq!(
        seen.borrow().as_slice(),
        ["0", "25", "75", "100"],
        "every pointer position must snap to the nearest mark and clamp at \
         both ends of the range"
    );
}
