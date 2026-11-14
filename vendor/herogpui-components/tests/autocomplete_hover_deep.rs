//! The pinned trigger-hover suppression contract:
//! `.autocomplete__trigger:hover:not(:has(.autocomplete__clear-button:hover))`
//! (and the same guard on `.autocomplete--secondary .autocomplete__trigger`).
//! While the pointer is on the clear button inside the trigger, the trigger's
//! own hover fill must be suppressed — the exact double-hover effect the
//! upstream `:not(:has(..))` rule exists to prevent.
//!
//! gpui 0.2.2 cannot express `:has` on a parent and a parent hitbox stays
//! hovered while a child's is, so the component carries the decision in keyed
//! state fed by the clear button's own hover listener. These tests drive real
//! hover coordinates through that listener and read the *rendered decision*
//! through the trigger's `debug_selector` probe (`…-trigger-suppressed-…`).
//!
//! Painted colors are not observable headlessly (`Frame` is `pub(crate)`), so
//! the painted refinement itself — suppress leaves the resting style, else
//! the pinned hover token — is pinned by the source-shape unit test inside
//! `autocomplete.rs`, the same split `chip_deep.rs` documents.
//!
//! GPUI clears debug bounds each frame and recomputes hover after layout,
//! so the current probe also tracks a stationary pointer when controls move.

use crate::harness;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    point, prelude::*, px, size, Bounds, Modifiers, MouseButton, Pixels, Point, SharedString,
    TestAppContext, VisualTestContext,
};
use herogpui_components::{Autocomplete, FieldVariant, InputState, PickerItem};

use harness::{events, open_host};

/// Pushes the pending frame through before `debug_bounds` reads it.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// Items whose labels are unique, so the key can be the label itself.
fn keyed(labels: &[&str]) -> Vec<PickerItem> {
    labels
        .iter()
        .map(|l| PickerItem::new(l.to_string(), l.to_string()))
        .collect()
}

/// `debug_bounds` keys on `&'static str`; the probe names carry the
/// component's instance id, so they are built at run time and leaked.
fn probe(name: String) -> &'static str {
    Box::leak(name.into_boxed_str())
}

/// Geometry comparisons sit inside a tolerance instead of `==` because
/// `float_cmp` is denied and layout rounds to whole pixels anyway.
fn near(a: impl Into<Pixels>, b: impl Into<Pixels>) -> bool {
    (f32::from(a.into()) - f32::from(b.into())).abs() < 0.5
}

fn same_bounds(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
    near(a.origin.x, b.origin.x)
        && near(a.origin.y, b.origin.y)
        && near(a.size.width, b.size.width)
        && near(a.size.height, b.size.height)
}

/// One suppression cycle against real hover coordinates: resting, onto the
/// clear button, a repaint under suppression, then back onto the trigger's
/// value area. Fails against an unsuppressed trigger because the suppressed
/// decision is never rendered at all.
fn assert_suppression_cycle(cx: &mut VisualTestContext, base: &str) {
    let suppressed = probe(format!("{base}-trigger-suppressed-true"));
    let unsuppressed = probe(format!("{base}-trigger-suppressed-false"));
    let clear_probe = probe(format!("{base}-clear"));

    flush_frame(cx);
    let resting = cx
        .debug_bounds(unsuppressed)
        .expect("the resting frame must render the unsuppressed decision");
    let clear = cx
        .debug_bounds(clear_probe)
        .expect("a selected Autocomplete must render its clear button");
    let clear_centre: Point<Pixels> = clear.center();

    cx.simulate_mouse_move(clear_centre, None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    let held = cx.debug_bounds(suppressed).expect(
        "hovering the clear button must suppress the trigger hover \
             (pinned `.autocomplete__trigger:hover:not(\
             :has(.autocomplete__clear-button:hover))`)",
    );
    assert!(
        same_bounds(held, resting),
        "the suppressed decision must repaint the same resting trigger box"
    );

    // Resizing moves the clear button away from the stationary pointer.
    cx.simulate_resize(size(px(2400.), px(800.)));
    flush_frame(cx);
    let clear_wide = cx
        .debug_bounds(clear_probe)
        .expect("clear button follows layout");
    let wide = cx
        .debug_bounds(unsuppressed)
        .expect("moving the clear button lifts suppression");
    assert!(wide.size.width > resting.size.width);
    assert!(cx.debug_bounds(suppressed).is_none());

    cx.simulate_mouse_move(clear_wide.center(), None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    assert!(same_bounds(
        cx.debug_bounds(suppressed)
            .expect("the new clear position suppresses hover"),
        wide
    ));
    assert!(cx.debug_bounds(unsuppressed).is_none());

    cx.simulate_mouse_move(
        point(px(60.), px(18.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    let lifted = cx
        .debug_bounds(unsuppressed)
        .expect("leaving clear lifts suppression");
    assert!(same_bounds(lifted, wide));
    assert!(cx.debug_bounds(suppressed).is_none());
}

fn open_autocomplete(
    cx: &mut TestAppContext,
    variant: FieldVariant,
) -> (&mut VisualTestContext, String) {
    let state = cx.new(|cx| InputState::new(cx));
    let base = format!("autocomplete-{}", state.entity_id().as_u64());
    let state_for_view = state;
    let cx = open_host(cx, move || {
        // `defaultValue` mounts the clear button without any interaction;
        // `full_width` gives the resize steps above a geometry channel.
        Autocomplete::new(state_for_view.clone(), keyed(&["Rust", "Go", "Typst"]))
            .default_value(["Rust"])
            .variant(variant)
            .full_width(true)
            .into_any_element()
    });
    (cx, base)
}

#[gpui::test]
fn hovering_the_clear_button_suppresses_the_primary_trigger_hover(cx: &mut TestAppContext) {
    let (cx, base) = open_autocomplete(cx, FieldVariant::Primary);
    assert_suppression_cycle(cx, &base);
}

#[gpui::test]
fn hovering_the_clear_button_suppresses_the_secondary_trigger_hover(cx: &mut TestAppContext) {
    // The pinned sheet restates the guard for the secondary variant:
    // `&:hover:not(:has(.autocomplete__clear-button:hover))` over
    // `--autocomplete-trigger-bg-hover`.
    let (cx, base) = open_autocomplete(cx, FieldVariant::Secondary);
    assert_suppression_cycle(cx, &base);
}

/// A real press at `at`: down, a repaint, up, a repaint — so a keyed press
/// state is observable between the halves, the way a finger rests on a button.
fn press_halves(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
}

#[gpui::test]
fn clearing_then_reselecting_does_not_leak_a_stale_suppression(cx: &mut TestAppContext) {
    // The keyed suppression flag is fed by the clear button's own hover
    // listener. Clearing makes the button empty (`data-empty` is
    // `pointer-events-none`), and reselecting re-arms it — if the flag could
    // survive that cycle, the trigger would stay suppressed with the pointer
    // nowhere near the button.
    let state = cx.new(|cx| InputState::new(cx));
    let base = format!("autocomplete-{}", state.entity_id().as_u64());
    let selection: Rc<RefCell<Vec<SharedString>>> = Rc::new(RefCell::new(vec!["Rust".into()]));
    let seen = events();
    let state_for_view = state;
    let sel_for_value = selection.clone();
    let seen_for_view = seen.clone();
    let cx = open_host(cx, move || {
        let sel_for_cb = sel_for_value.clone();
        let seen_for_cb = seen_for_view.clone();
        let keys: Vec<SharedString> = sel_for_value.borrow().clone();
        Autocomplete::new(state_for_view.clone(), keyed(&["Rust", "Go", "Typst"]))
            .value(keys)
            .full_width(true)
            .on_selection_change_all(move |keys, _, _| {
                *sel_for_cb.borrow_mut() = keys.to_vec();
                seen_for_cb
                    .borrow_mut()
                    .push(format!("selection:{}", keys.len()));
            })
            .into_any_element()
    });

    let suppressed = probe(format!("{base}-trigger-suppressed-true"));
    let unsuppressed = probe(format!("{base}-trigger-suppressed-false"));
    let clear_probe = probe(format!("{base}-clear"));

    flush_frame(cx);
    let resting = cx
        .debug_bounds(unsuppressed)
        .expect("the resting frame must render the unsuppressed decision");
    let clear = cx
        .debug_bounds(clear_probe)
        .expect("a selected Autocomplete must render its clear button");

    cx.simulate_mouse_move(clear.center(), None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    cx.debug_bounds(suppressed)
        .expect("hovering the clear button must suppress the trigger hover");

    press_halves(cx, clear.center());
    assert!(
        seen.borrow().iter().any(|e| e == "selection:0"),
        "the press on the clear button must clear: {:?}",
        seen.borrow()
    );

    // The pointer leaves over the trigger's value area while the button is
    // empty and pointer-inert: no event can reach the detached listener, so
    // only the inert-frame normalization can drop the stale hover.
    cx.simulate_mouse_move(
        point(px(60.), px(18.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);

    // Widen: the current frame must show only the unsuppressed decision.
    cx.simulate_resize(size(px(2400.), px(800.)));
    flush_frame(cx);
    let wide = cx
        .debug_bounds(unsuppressed)
        .expect("after a clear the trigger must hover unconditionally");
    assert!(
        wide.size.width > resting.size.width,
        "the widened unsuppressed frame must repaint fresh bounds \
         (wide={wide:?}, resting={resting:?})"
    );
    assert!(
        cx.debug_bounds(suppressed).is_none(),
        "suppression must not survive clear-and-move"
    );

    // Reselect through the controlled value: the button re-arms, and the
    // suppression must still be off until a real hover lands on it again.
    *selection.borrow_mut() = vec!["Rust".into()];
    cx.simulate_resize(size(px(1800.), px(800.)));
    flush_frame(cx);
    let narrower = cx
        .debug_bounds(unsuppressed)
        .expect("the reselected trigger must hover unconditionally");
    assert!(
        narrower.size.width < wide.size.width,
        "the reselected frame must repaint fresh bounds \
         (narrower={narrower:?}, wide={wide:?})"
    );
    assert!(
        cx.debug_bounds(suppressed).is_none(),
        "suppression must not resurrect when clear re-arms"
    );
}

#[gpui::test]
fn pressing_the_clear_button_scales_the_visual_to_0_93(cx: &mut TestAppContext) {
    // `.autocomplete__clear-button:active, &[data-pressed="true"]` is
    // `transform: scale(0.93)`. gpui 0.2.2 cannot scale a div, so the hit box
    // stays put and a centered visual box carries the scale; both are
    // observable through `debug_bounds`.
    let (cx, base) = open_autocomplete(cx, FieldVariant::Primary);
    let hit_probe = probe(format!("{base}-clear"));
    let visual_probe = probe(format!("{base}-clear-visual"));

    flush_frame(cx);
    let hit = cx
        .debug_bounds(hit_probe)
        .expect("a selected Autocomplete must render its clear button");
    let resting = cx
        .debug_bounds(visual_probe)
        .expect("the clear button's visual box must be mounted");
    assert!(
        near(resting.size.width, 20.),
        "the resting visual is the pinned `size-5` box (painted {resting:?})"
    );

    cx.simulate_mouse_down(hit.center(), MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    let pressed = cx
        .debug_bounds(visual_probe)
        .expect("the pressed visual must repaint");
    assert!(
        near(pressed.size.width, 20. * 0.93),
        "the pinned `:active` scale is 0.93 (painted {pressed:?})"
    );

    cx.simulate_mouse_up(hit.center(), MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    let released = cx
        .debug_bounds(visual_probe)
        .expect("the released visual must repaint");
    assert!(
        near(released.size.width, 20.),
        "the scale must release with the press (painted {released:?})"
    );
}

#[gpui::test]
fn an_empty_selection_mounts_an_invisible_inert_clear_button(cx: &mut TestAppContext) {
    // Pinned v3 composes the clear button unconditionally: an empty selection
    // only sets `data-empty`, which is `pointer-events-none opacity-0` — the
    // part stays mounted, lets every press fall through to the trigger, and
    // can never suppress the trigger hover.
    let state = cx.new(|cx| InputState::new(cx));
    let base = format!("autocomplete-{}", state.entity_id().as_u64());
    let seen = events();
    let state_for_view = state;
    let seen_for_view = seen.clone();
    let cx = open_host(cx, move || {
        let seen_open = seen_for_view.clone();
        Autocomplete::new(state_for_view.clone(), keyed(&["Rust", "Go"]))
            .full_width(true)
            .on_open_change(move |open, _, _| {
                seen_open.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    let unsuppressed = probe(format!("{base}-trigger-suppressed-false"));
    let suppressed = probe(format!("{base}-trigger-suppressed-true"));
    let clear_probe = probe(format!("{base}-clear"));

    flush_frame(cx);
    cx.debug_bounds(unsuppressed)
        .expect("the resting frame must render the unsuppressed decision");
    let clear = cx
        .debug_bounds(clear_probe)
        .expect("the empty clear button must stay mounted (`data-empty` only hides it)");

    cx.simulate_mouse_move(clear.center(), None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    assert!(
        cx.debug_bounds(suppressed).is_none(),
        "a pointer-inert empty clear button must never suppress the trigger"
    );

    cx.simulate_click(clear.center(), Modifiers::none());
    flush_frame(cx);
    assert!(
        seen.borrow().iter().any(|e| e == "open:true"),
        "the empty button's press must fall through to the trigger's toggle: {:?}",
        seen.borrow()
    );
}

#[gpui::test]
fn a_disabled_selection_keeps_a_mounted_but_inert_clear_button(cx: &mut TestAppContext) {
    // Pinned v3 renders the part with `disabled={isDisabled}` rather than
    // unmounting it: mounted and dimmed with the trigger, but a press clears
    // nothing and no hover decision is ever suppressed.
    let state = cx.new(|cx| InputState::new(cx));
    let base = format!("autocomplete-{}", state.entity_id().as_u64());
    let seen = events();
    let state_for_view = state;
    let seen_for_view = seen.clone();
    let cx = open_host(cx, move || {
        let seen_cb = seen_for_view.clone();
        let seen_clear = seen_for_view.clone();
        Autocomplete::new(state_for_view.clone(), keyed(&["Rust", "Go"]))
            .default_value(["Rust"])
            .is_disabled(true)
            .full_width(true)
            .on_selection_change_all(move |keys, _, _| {
                seen_cb
                    .borrow_mut()
                    .push(format!("selection:{}", keys.len()));
            })
            .on_clear(move |_, _| seen_clear.borrow_mut().push("clear".to_owned()))
            .into_any_element()
    });

    let clear_probe = probe(format!("{base}-clear"));
    let suppressed = probe(format!("{base}-trigger-suppressed-true"));

    flush_frame(cx);
    let clear = cx
        .debug_bounds(clear_probe)
        .expect("the disabled clear button must stay mounted (`disabled={isDisabled}`)");
    cx.simulate_click(clear.center(), Modifiers::none());
    flush_frame(cx);
    assert!(
        seen.borrow().is_empty(),
        "a disabled clear button must not clear: {:?}",
        seen.borrow()
    );
    assert!(
        cx.debug_bounds(suppressed).is_none(),
        "a disabled trigger never reports suppression"
    );
}

#[gpui::test]
fn a_read_only_selection_keeps_a_working_clear_button(cx: &mut TestAppContext) {
    // Pinned v3 has no read-only gate to check against: RAC 1.20.0's `Select`
    // has no `isReadOnly` at all, the part is gated only by `disabled`, and
    // pinned react-stately 3.49.0's `selectionManager.setSelectedKeys` is
    // unguarded. The clear button therefore mounts, suppresses the trigger
    // hover while hovered, and clears — exactly like an enabled control.
    let state = cx.new(|cx| InputState::new(cx));
    let base = format!("autocomplete-{}", state.entity_id().as_u64());
    let seen = events();
    let state_for_view = state;
    let seen_for_view = seen.clone();
    let cx = open_host(cx, move || {
        let seen_cb = seen_for_view.clone();
        let seen_clear = seen_for_view.clone();
        Autocomplete::new(state_for_view.clone(), keyed(&["Rust", "Go"]))
            .default_value(["Rust"])
            .is_read_only(true)
            .on_selection_change_all(move |keys, _, _| {
                seen_cb
                    .borrow_mut()
                    .push(format!("selection:{}", keys.len()));
            })
            .on_clear(move |_, _| seen_clear.borrow_mut().push("clear".to_owned()))
            .into_any_element()
    });

    let clear_probe = probe(format!("{base}-clear"));
    let suppressed = probe(format!("{base}-trigger-suppressed-true"));

    flush_frame(cx);
    let clear = cx
        .debug_bounds(clear_probe)
        .expect("the read-only clear button must stay mounted (pinned v3 has no read-only gate)");
    cx.simulate_mouse_move(clear.center(), None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    cx.debug_bounds(suppressed)
        .expect("hovering the read-only clear button must suppress the trigger hover");

    press_halves(cx, clear.center());
    assert!(
        seen.borrow().iter().any(|e| e == "selection:0"),
        "the read-only clear button must clear: {:?}",
        seen.borrow()
    );
}

#[gpui::test]
fn suppression_is_instance_scoped(cx: &mut TestAppContext) {
    // The keyed slot is derived from the instance id, so one control's hover
    // must never reach another control's trigger — even when both re-render
    // in the same window and frame.
    let state_a = cx.new(|cx| InputState::new(cx));
    let state_b = cx.new(|cx| InputState::new(cx));
    let base_a = format!("autocomplete-{}", state_a.entity_id().as_u64());
    let base_b = format!("autocomplete-{}", state_b.entity_id().as_u64());
    let a = state_a;
    let b = state_b;
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                Autocomplete::new(a.clone(), keyed(&["Rust", "Go"]))
                    .default_value(["Rust"])
                    .full_width(true),
            )
            .child(
                Autocomplete::new(b.clone(), keyed(&["Rust", "Go"]))
                    .default_value(["Go"])
                    .full_width(true),
            )
            .into_any_element()
    });

    let clear_a = probe(format!("{base_a}-clear"));
    let clear_b = probe(format!("{base_b}-clear"));
    let suppressed_a = probe(format!("{base_a}-trigger-suppressed-true"));
    let suppressed_b = probe(format!("{base_b}-trigger-suppressed-true"));
    let unsuppressed_a = probe(format!("{base_a}-trigger-suppressed-false"));
    let unsuppressed_b = probe(format!("{base_b}-trigger-suppressed-false"));

    flush_frame(cx);
    let clear_a_bounds = cx
        .debug_bounds(clear_a)
        .expect("the first instance must mount its clear button");
    let clear_b_bounds = cx
        .debug_bounds(clear_b)
        .expect("the second instance must mount its clear button");
    assert!(
        clear_b_bounds.origin.y > clear_a_bounds.origin.y,
        "the two triggers must stack (a={clear_a_bounds:?}, b={clear_b_bounds:?})"
    );
    let resting_a = cx
        .debug_bounds(unsuppressed_a)
        .expect("the first instance must render its resting decision");

    cx.simulate_mouse_move(
        clear_a_bounds.center(),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    cx.simulate_resize(size(px(2400.), px(800.)));
    flush_frame(cx);

    let moved_clear_a = cx
        .debug_bounds(clear_a)
        .expect("A clear button follows layout");
    assert!(
        cx.debug_bounds(suppressed_a).is_none(),
        "resizing moved A away from the pointer"
    );
    cx.simulate_mouse_move(
        moved_clear_a.center(),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);

    let wide_a = cx
        .debug_bounds(suppressed_a)
        .expect("hovering A's clear button must suppress A's trigger");
    assert!(
        wide_a.size.width > resting_a.size.width,
        "A's suppressed decision must repaint at the widened layout \
         (wide={wide_a:?}, resting={resting_a:?})"
    );
    let wide_b = cx
        .debug_bounds(unsuppressed_b)
        .expect("B must keep hovering unconditionally");
    assert!(
        near(wide_a.size.width, wide_b.size.width),
        "both full-width triggers share the widened width, so the match can \
         only come from fresh paints (a={wide_a:?}, b={wide_b:?})"
    );
    assert!(
        cx.debug_bounds(suppressed_b).is_none(),
        "hovering A's clear button must never suppress B's trigger"
    );
    assert!(
        cx.debug_bounds(unsuppressed_a).is_none(),
        "A must only paint its suppressed decision while hovered"
    );
}
