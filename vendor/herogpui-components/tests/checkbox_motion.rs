//! Checkbox selection motion contracts.
//!
//! HeroUI v3.2.4 animates a checkbox toggle in three layers, read from the
//! pinned `checkbox.css` / `checkbox.js`: the `::before` accent fill scales
//! (`100ms linear` from `scale-70`) and fades (`200ms linear` from
//! `opacity-0`), its background eases to the accent over `200ms`, and the
//! checkmark's `strokeDashoffset` slide draws the tick over `150ms linear`
//! after `15ms`, undrawing over the base `duration-200`. The control's own
//! background eases to the accent while indeterminate (`200ms`). A
//! pre-selected box mounts settled — CSS transitions do not run on load — and
//! everything snaps under reduced motion.
//!
//! The headless test platform has no rasterizer (`TestAppContext`'s window
//! carries no `PlatformHeadlessRenderer`, so `render_to_image` has nothing to
//! sample), so these tests observe motion through the frame loop the way
//! gpui's own animation tests do: every animation in flight registers a
//! next-frame callback via `Window::request_animation_frame` and stops once it
//! settles, so `Window::simulate_next_frame` counts the live animations.
//!
//! **What that proves: animation lifetime only** — that a toggle starts
//! animating, is still animating 60ms in, and settles. It does not prove
//! visual continuity, geometry, or precise per-frame values; pixels are left
//! to the real browser.

use crate::harness;

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{point, prelude::*, px, Modifiers, TestAppContext, VisualTestContext};
use herogpui_components::{Checkbox, CheckboxState};

use harness::{click, events, open_host, press, still};

/// All three toggle layers outlive this sample: even the shortest (the fill's
/// 100ms scale) is still running 60ms after the toggle frame.
const MID_FLIGHT_MS: u64 = 60;

/// How many animation-frame callbacks are pending: one per live animation.
fn pending_frames(cx: &mut VisualTestContext) -> usize {
    cx.update(|window, cx| window.simulate_next_frame(cx))
}

/// Forces the frame that carries the state a handler or flag just changed.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// Drains frames until every in-flight animation has settled, failing if one
/// never does (a re-arming animation would keep registering callbacks).
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..100 {
        if pending_frames(cx) == 0 {
            return;
        }
        cx.executor().advance_clock(Duration::from_millis(10));
    }
    panic!("checkbox animations never settled");
}

fn advance_mid_flight(cx: &mut VisualTestContext) {
    cx.executor()
        .advance_clock(Duration::from_millis(MID_FLIGHT_MS));
}

#[gpui::test]
fn preselected_checkbox_mounts_settled_without_a_mount_animation(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        Checkbox::new("motion-preset")
            .default_selected(true)
            .label("Pre-selected")
            .into_any_element()
    });

    // A pre-selected control shows the drawn state immediately: no animation
    // may mount for it, at this frame or after a forced one.
    assert_eq!(
        pending_frames(cx),
        0,
        "a pre-selected checkbox must mount settled, not animate on mount"
    );
    flush_frame(cx);
    assert_eq!(pending_frames(cx), 0, "nothing may animate while at rest");
}

#[gpui::test]
fn selection_and_unselection_animate_and_settle(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Checkbox::new("motion-toggle")
            .label("Toggle")
            .on_change(move |selected, _, _| changes.borrow_mut().push(selected.to_string()))
            .into_any_element()
    });

    assert_eq!(pending_frames(cx), 0, "an unchecked box is at rest");

    // The row starts at the window origin and the 16px control box is its
    // first child, centred at (8, 8).
    click(cx, 8., 8.);
    assert_eq!(recorded.borrow().as_slice(), ["true"]);

    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "selection must still be animating {MID_FLIGHT_MS}ms in"
    );

    settle(cx);
    assert_eq!(pending_frames(cx), 0, "selection must have settled");

    // Unselection runs the same layers backwards and settles too.
    click(cx, 8., 8.);
    assert_eq!(recorded.borrow().as_slice(), ["true", "false"]);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "unselection must still be animating {MID_FLIGHT_MS}ms in"
    );
    settle(cx);
    assert_eq!(pending_frames(cx), 0);
}

#[gpui::test]
fn reversing_selection_mid_flight_still_animates_and_settles(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Checkbox::new("motion-reverse")
            .label("Reverse")
            .on_change(move |selected, _, _| changes.borrow_mut().push(selected.to_string()))
            .into_any_element()
    });

    // Turn around while the reveal is mid-flight. The reversal must not wedge
    // the motion slots: an animation is scheduled again right after the second
    // click and settles.
    click(cx, 8., 8.);
    advance_mid_flight(cx);
    click(cx, 8., 8.);
    flush_frame(cx);
    assert!(
        pending_frames(cx) > 0,
        "a mid-flight reversal must schedule a new animation"
    );
    settle(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["true", "false"],
        "the events must record the full reversal"
    );
    assert_eq!(pending_frames(cx), 0, "the reversal must have settled");

    // And the mechanism is still alive afterwards: one more toggle starts a
    // fresh animation.
    click(cx, 8., 8.);
    flush_frame(cx);
    assert!(
        pending_frames(cx) > 0,
        "a toggle after a mid-flight reversal must animate again"
    );
    settle(cx);
    assert_eq!(recorded.borrow().as_slice(), ["true", "false", "true"]);
}

#[gpui::test]
fn reduced_motion_snaps_selection_without_animating(cx: &mut TestAppContext) {
    still();
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Checkbox::new("motion-reduced")
            .label("Reduced")
            .on_change(move |selected, _, _| changes.borrow_mut().push(selected.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["true"]);

    // The state still changed, but instantly: no animation element is ever
    // mounted, so the very first drain finds no pending frame.
    flush_frame(cx);
    assert_eq!(
        pending_frames(cx),
        0,
        "reduced motion must snap selection instead of animating it"
    );

    press(cx, "space");
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["true", "false"],
        "unselection must still apply under reduced motion"
    );
    assert_eq!(pending_frames(cx), 0);
}

#[gpui::test]
fn controlled_selection_follows_external_state_only(cx: &mut TestAppContext) {
    let selected = Rc::new(Cell::new(false));
    let seed = selected.clone();
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let selected = seed.clone();
        let changes = changes.clone();
        Checkbox::new("motion-controlled")
            .is_selected(selected.get())
            .label("Controlled")
            .on_change(move |next, _, _| {
                // Deliberately does not feed back: the caller owns the value.
                changes.borrow_mut().push(next.to_string());
            })
            .into_any_element()
    });

    assert_eq!(pending_frames(cx), 0, "the controlled box mounts at rest");

    // Activating reports the change but moves nothing: the value stays the
    // caller's, so no motion runs.
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["true"]);
    flush_frame(cx);
    assert_eq!(
        pending_frames(cx),
        0,
        "a controlled toggle the caller rejects must not animate"
    );

    // The caller moving the value is what animates, in both directions.
    selected.set(true);
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(pending_frames(cx) > 0, "an external selection must animate");
    settle(cx);

    selected.set(false);
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "an external unselection must animate"
    );
    settle(cx);
    assert_eq!(pending_frames(cx), 0);
}

#[gpui::test]
fn indeterminate_transitions_animate_the_control_both_ways(cx: &mut TestAppContext) {
    let indeterminate = Rc::new(Cell::new(false));
    let seed = indeterminate.clone();
    let cx = open_host(cx, move || {
        let indeterminate = seed.clone();
        Checkbox::new("motion-indeterminate")
            .default_selected(true)
            .is_indeterminate(indeterminate.get())
            .label("Indeterminate")
            .into_any_element()
    });

    // Selected and settled.
    assert_eq!(pending_frames(cx), 0);

    // Upstream keeps the selected fill visible under `data-indeterminate` and
    // only moves the control: its background eases to the accent (200ms) as
    // the check swaps for the dash, so the flip animates.
    indeterminate.set(true);
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "selected -> indeterminate must animate the control"
    );
    settle(cx);

    // And back: the control background eases off the accent as the tick
    // returns.
    indeterminate.set(false);
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "indeterminate -> selected must animate the control"
    );
    settle(cx);
    assert_eq!(pending_frames(cx), 0);
}

/// Regression guard for the correction pass: disabling the control under the
/// pointer must clear the stale hovered/pressed slot. Until that change lands,
/// the fill keeps the hover accent and the disabled half of this test finds no
/// animation. Expected to fail until then; the parent runs the final tests.
#[gpui::test]
fn disable_while_hovered_clears_the_stale_hover_colour(cx: &mut TestAppContext) {
    let disabled = Rc::new(Cell::new(false));
    let seed = disabled.clone();
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let disabled = seed.clone();
        let changes = changes.clone();
        Checkbox::new("motion-disable-hovered")
            .default_selected(true)
            .is_disabled(disabled.get())
            .label("Disable while hovered")
            .on_change(move |selected, _, _| changes.borrow_mut().push(selected.to_string()))
            .into_any_element()
    });

    // Hover the control: the visible fill's background eases to the hover
    // accent.
    cx.simulate_mouse_move(point(px(8.), px(8.)), None, Modifiers::none());
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(pending_frames(cx) > 0, "hovering must ease the fill colour");
    settle(cx);

    // Disabling under the pointer must not leave the hover colour painted:
    // the stale hovered slot is cleared, which is only visible as motion —
    // the fill easing back off the hover accent.
    disabled.set(true);
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "disabling a hovered checkbox must ease off the stale hover colour"
    );
    settle(cx);
    assert_eq!(pending_frames(cx), 0);

    // And the disabled control no longer reacts: no change, no motion.
    click(cx, 8., 8.);
    flush_frame(cx);
    assert!(
        recorded.borrow().is_empty(),
        "a disabled checkbox must not report changes"
    );
    assert_eq!(pending_frames(cx), 0);
}

#[gpui::test]
fn custom_indicator_keeps_the_fill_motion_and_sees_the_state(cx: &mut TestAppContext) {
    let seen: Rc<std::cell::RefCell<Vec<bool>>> = Rc::new(std::cell::RefCell::new(Vec::new()));
    let recorded_states = seen.clone();
    let cx = open_host(cx, move || {
        let seen = recorded_states.clone();
        Checkbox::new("motion-custom-indicator")
            .label("Custom indicator")
            .indicator(move |state: CheckboxState| {
                seen.borrow_mut().push(state.is_selected);
                gpui::div().size(px(6.)).into_any_element()
            })
            .into_any_element()
    });

    // A caller-drawn mark replaces the stroke canvas, but the fill is the
    // component's own and still animates.
    assert_eq!(pending_frames(cx), 0);
    click(cx, 8., 8.);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "selection must animate the fill behind a custom indicator too"
    );
    settle(cx);

    // The render function saw the unchecked state first and the selected one
    // last — it is handed the live field state on every frame it redraws in.
    let seen = seen.borrow();
    assert_eq!(
        seen.first(),
        Some(&false),
        "the indicator must see the rest state"
    );
    assert_eq!(
        seen.last(),
        Some(&true),
        "the indicator must see the settled selected state"
    );
    assert_eq!(pending_frames(cx), 0);
}
