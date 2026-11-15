//! Deep contracts inherited from HeroUI v3.2.4's pinned React Aria/Stately.
//!
//! Slider tracks are fixed at 600px. With `minValue=0` and `maxValue=100`,
//! x=120/300/540 map to 20/50/90. NumberField is 220x36 with its 40px
//! increment button centred at (200, 18). Every mouse mutation is followed by
//! a redraw because hit testing reads the last rendered frame.

use crate::harness;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    point, prelude::*, px, Focusable, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext,
};
use herogpui_components::{NumberField, NumberState, Slider};

use harness::{click, events, open_host, press};

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn drag(cx: &mut VisualTestContext, from: (f32, f32), to: (f32, f32)) {
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(from.0), px(from.1)),
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    flush_frame(cx);
    cx.simulate_event(MouseMoveEvent {
        position: point(px(to.0), px(to.1)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    });
    flush_frame(cx);
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: point(px(to.0), px(to.1)),
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    flush_frame(cx);
}

fn wheel(cx: &mut VisualTestContext, dy: f32) {
    cx.simulate_event(ScrollEvent {
        position: point(px(100.), px(18.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
        ..Default::default()
    });
    flush_frame(cx);
}

#[gpui::test]
fn range_thumb_clamps_at_its_neighbour_without_changing_identity(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("range-identity", 0.)
                    .values([20., 80.])
                    .on_change_all(move |values, _, _| {
                        recorded
                            .borrow_mut()
                            .push(format!("{},{}", values[0], values[1]));
                    }),
            )
            .into_any_element()
    });

    // The press activates the lower thumb. Dragging through the upper thumb
    // clamps that same lower thumb at 80 rather than switching to thumb 1.
    drag(cx, (120., 10.), (540., 10.));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["80,80"],
        "an unchanged pointer press is silent, then the lower thumb stops at its neighbour"
    );
}

#[gpui::test]
fn range_track_tie_activates_upper_thumb_for_following_keys(cx: &mut TestAppContext) {
    let recorded = events();
    let held = Rc::new(RefCell::new(vec![20., 80.]));
    let held_for_view = held;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let values = held_for_view.borrow().clone();
        let held = held_for_view.clone();
        let recorded = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("range-tie", 0.)
                    .values(values)
                    .step(5.)
                    .on_change_all(move |values, _, _| {
                        *held.borrow_mut() = values.to_vec();
                        recorded
                            .borrow_mut()
                            .push(format!("{},{}", values[0], values[1]));
                    }),
            )
            .into_any_element()
    });

    // Value 50 is equidistant from 20 and 80, so React Aria chooses the
    // end/right thumb. The following Right key must continue from that thumb.
    cx.simulate_click(point(px(300.), px(10.)), Modifiers::none());
    flush_frame(cx);
    press(cx, "right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["20,50", "20,55"],
        "an equal-distance track press must activate the upper thumb before moving it"
    );
}

#[gpui::test]
fn range_track_does_not_redirect_from_a_disabled_nearest_thumb(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("disabled-nearest", 0.)
                    .values([20., 80.])
                    .disabled_keys([1])
                    .on_change_all(move |values, _, _| {
                        recorded
                            .borrow_mut()
                            .push(format!("{},{}", values[0], values[1]));
                    }),
            )
            .into_any_element()
    });

    // The upper thumb wins the 50/50 geometry tie, but it is disabled. The
    // track press is a no-op; it must not redirect to thumb 0.
    cx.simulate_click(point(px(300.), px(10.)), Modifiers::none());
    flush_frame(cx);
    assert!(
        recorded.borrow().is_empty(),
        "a disabled nearest thumb must not redirect the track press"
    );
}

#[gpui::test]
fn range_change_end_reports_the_full_final_array_once(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let changes = for_view.clone();
        let ends = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("range-end", 0.)
                    .values([20., 80.])
                    .on_change_all(move |values, _, _| {
                        changes
                            .borrow_mut()
                            .push(format!("change:{},{}", values[0], values[1]));
                    })
                    .on_change_end_all(move |values, _, _| {
                        ends.borrow_mut()
                            .push(format!("end:{},{}", values[0], values[1]));
                    }),
            )
            .into_any_element()
    });

    // Release beyond the 600px track to prove the window-scoped listener ends
    // the drag and reports once even after the pointer leaves the hitbox.
    drag(cx, (120., 10.), (700., 10.));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:80,80", "end:80,80"],
        "an unchanged pointer press is silent, while release reports one full-array onChangeEnd"
    );
}

#[gpui::test]
fn one_element_values_drag_reports_array_callbacks(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let changes = for_view.clone();
        let ends = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("one-controlled", 0.)
                    .values([90.])
                    .on_change_all(move |values, _, _| {
                        changes.borrow_mut().push(format!("change:{}", values[0]));
                    })
                    .on_change_end_all(move |values, _, _| {
                        ends.borrow_mut().push(format!("end:{}", values[0]));
                    }),
            )
            .into_any_element()
    });

    drag(cx, (540., 10.), (60., 10.));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:10", "end:10"],
        "the one-element array form must report through its array callbacks"
    );
}

#[gpui::test]
fn one_element_default_values_keyboard_updates_its_array_state(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let changes = for_view.clone();
        let ends = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("one-uncontrolled", 0.)
                    .default_values([90.])
                    .on_change_all(move |values, _, _| {
                        changes.borrow_mut().push(format!("change:{}", values[0]));
                    })
                    .on_change_end_all(move |values, _, _| {
                        ends.borrow_mut().push(format!("end:{}", values[0]));
                    }),
            )
            .into_any_element()
    });

    cx.simulate_click(point(px(540.), px(10.)), Modifiers::none());
    flush_frame(cx);
    recorded.borrow_mut().clear();
    press(cx, "left");
    flush_frame(cx);
    press(cx, "left");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:89", "end:89", "change:88", "end:88"],
        "the uncontrolled one-element array must persist each keyboard step"
    );
}

#[gpui::test]
fn slider_pointer_and_keyboard_snap_from_min_value(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("min-lattice", 3.)
                    .default_value(3.)
                    .min_value(3.)
                    .max_value(23.)
                    .step(5.)
                    .on_change(move |value, _, _| {
                        recorded.borrow_mut().push(format!("{value}"));
                    }),
            )
            .into_any_element()
    });

    // x=210 is 35% of the track: raw value 10. The min-anchored lattice
    // 3,8,13,... snaps it to 8, and Right advances to 13.
    cx.simulate_click(point(px(210.), px(10.)), Modifiers::none());
    flush_frame(cx);
    press(cx, "right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["8", "13"],
        "pointer and keyboard stepping must share the lattice anchored at minValue"
    );
}

#[gpui::test]
fn slider_fractional_steps_round_to_step_precision(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(Vec::<u32>::new()));
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("fractional-slider", 0.1)
                    .default_value(0.1)
                    .min_value(0.1)
                    .max_value(0.5)
                    .step(0.1)
                    .on_change(move |value, _, _| recorded.borrow_mut().push(value.to_bits())),
            )
            .into_any_element()
    });

    cx.simulate_click(point(px(300.), px(10.)), Modifiers::none());
    flush_frame(cx);
    press(cx, "right");
    assert_eq!(
        recorded.borrow().as_slice(),
        [0.3_f32.to_bits(), 0.4_f32.to_bits()],
        "fractional slider values must be rounded to the step's decimal precision"
    );
}

#[gpui::test]
fn slider_accepts_positive_exponent_steps(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(Vec::<u32>::new()));
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("exponent-slider", 0.)
                    .default_value(0.)
                    .max_value(0.000001)
                    .step(0.0000001)
                    .on_change(move |value, _, _| recorded.borrow_mut().push(value.to_bits())),
            )
            .into_any_element()
    });

    cx.simulate_click(point(px(300.), px(10.)), Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        &[0.0000005_f32.to_bits()],
        "a positive exponent-sized step must not be floored to 0.0001"
    );
}

#[gpui::test]
fn number_field_wheel_steps_only_while_focused(cx: &mut TestAppContext) {
    let recorded = events();
    let state = cx.new(|cx| NumberState::new(cx, 4.));
    let state_for_view = state;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        NumberField::new(state_for_view.clone())
            .min_value(3.)
            .max_value(23.)
            .step(5.)
            .on_change(move |value, _, _| recorded.borrow_mut().push(format!("{value}")))
            .into_any_element()
    });

    wheel(cx, 1.);
    press(cx, "tab");
    wheel(cx, 1.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["8"],
        "an unfocused wheel must be ignored and a focused wheel must use the min lattice"
    );
}

#[gpui::test]
fn number_field_wheel_disabled_blocks_focused_wheel(cx: &mut TestAppContext) {
    let recorded = events();
    let state = cx.new(|cx| NumberState::new(cx, 3.));
    let state_for_view = state;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        NumberField::new(state_for_view.clone())
            .min_value(3.)
            .max_value(23.)
            .step(5.)
            .is_wheel_disabled(true)
            .on_change(move |value, _, _| recorded.borrow_mut().push(format!("{value}")))
            .into_any_element()
    });

    press(cx, "tab");
    wheel(cx, 1.);
    assert!(
        recorded.borrow().is_empty(),
        "isWheelDisabled must suppress wheel stepping even while focused"
    );
}

#[gpui::test]
fn number_field_unbounded_step_uses_the_zero_lattice(cx: &mut TestAppContext) {
    let recorded = events();
    let state = cx.new(|cx| NumberState::new(cx, 4.));
    let state_for_view = state;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        NumberField::new(state_for_view.clone())
            .step(5.)
            .on_change(move |value, _, _| recorded.borrow_mut().push(format!("{value}")))
            .into_any_element()
    });

    cx.simulate_click(point(px(200.), px(18.)), Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["5"],
        "without minValue, React Stately anchors the step lattice at zero"
    );
}

#[gpui::test]
fn number_field_fractional_step_rounds_to_step_precision(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(Vec::<u64>::new()));
    let state = cx.new(|cx| NumberState::new(cx, 0.2));
    let state_for_view = state;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        NumberField::new(state_for_view.clone())
            .min_value(0.1)
            .max_value(0.5)
            .step(0.1)
            .on_change(move |value, _, _| recorded.borrow_mut().push(value.to_bits()))
            .into_any_element()
    });

    cx.simulate_click(point(px(200.), px(18.)), Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        [0.3_f64.to_bits()],
        "fractional NumberField values must be rounded to the step's decimal precision"
    );
}

#[gpui::test]
fn number_field_accepts_positive_exponent_steps(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(Vec::<u64>::new()));
    let state = cx.new(|cx| NumberState::new(cx, 0.));
    let state_for_view = state;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        NumberField::new(state_for_view.clone())
            .min_value(0.)
            .max_value(0.000001)
            .step(0.0000001)
            .on_change(move |value, _, _| recorded.borrow_mut().push(value.to_bits()))
            .into_any_element()
    });

    click(cx, 200., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        &[0.0000001_f64.to_bits()],
        "a positive exponent-sized step must not be floored to 0.0001"
    );
}

#[gpui::test]
fn number_field_consumes_focused_horizontal_bound_wheel(cx: &mut TestAppContext) {
    let bubbled = Rc::new(Cell::new(false));
    let for_view = bubbled.clone();
    let state = cx.new(|cx| NumberState::new(cx, 1.));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let bubbled = for_view.clone();
        gpui::div()
            .on_scroll(move |_, _, _| bubbled.set(true))
            .child(
                NumberField::new(state_for_view.clone())
                    .min_value(0.)
                    .max_value(1.)
                    .step(1.),
            )
            .into_any_element()
    });

    press(cx, "tab");
    cx.simulate_event(ScrollEvent {
        position: point(px(100.), px(18.)),
        delta: ScrollDelta::Pixels(point(px(1.), px(0.))),
        ..Default::default()
    });
    flush_frame(cx);
    assert!(
        !bubbled.get(),
        "focused NumberField must consume horizontal wheel input at a bound"
    );
}

#[gpui::test]
fn number_field_stepper_repeats_after_400ms_then_every_60ms(cx: &mut TestAppContext) {
    let recorded = events();
    let state = cx.new(|cx| NumberState::new(cx, 4.));
    let state_for_view = state;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        NumberField::new(state_for_view.clone())
            .min_value(3.)
            .max_value(23.)
            .step(5.)
            .on_change(move |value, _, _| recorded.borrow_mut().push(format!("{value}")))
            .into_any_element()
    });

    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(200.), px(18.)),
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["8"],
        "press start must step immediately"
    );

    cx.executor().advance_clock(Duration::from_millis(399));
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["8"],
        "the first repeat must wait the full 400ms"
    );

    cx.executor().advance_clock(Duration::from_millis(1));
    flush_frame(cx);
    cx.executor().advance_clock(Duration::from_millis(60));
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["8", "13", "18"],
        "holding must repeat once at 400ms and again 60ms later"
    );

    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: point(px(200.), px(18.)),
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    flush_frame(cx);
    cx.executor().advance_clock(Duration::from_millis(600));
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["8", "13", "18"],
        "releasing the stepper must cancel later repeats"
    );
}

#[gpui::test]
fn number_field_stepper_repeat_stops_when_unmounted(cx: &mut TestAppContext) {
    let recorded = events();
    let show = Rc::new(Cell::new(true));
    let show_for_view = show.clone();
    let state = cx.new(|cx| NumberState::new(cx, 0.));
    let state_for_view = state;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        if !show_for_view.get() {
            return gpui::div().into_any_element();
        }
        let recorded = for_view.clone();
        NumberField::new(state_for_view.clone())
            .step(1.)
            .on_change(move |value, _, _| recorded.borrow_mut().push(format!("{value}")))
            .into_any_element()
    });

    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(200.), px(18.)),
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    flush_frame(cx);
    assert_eq!(recorded.borrow().as_slice(), ["1"]);

    cx.executor().advance_clock(Duration::from_millis(400));
    flush_frame(cx);
    assert_eq!(recorded.borrow().as_slice(), ["1", "2"]);
    show.set(false);
    flush_frame(cx);
    cx.executor().advance_clock(Duration::from_millis(600));
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1", "2"],
        "unmounting the stepper must cancel its detached repeat task"
    );
}

#[gpui::test]
#[allow(clippy::float_cmp)]
fn number_field_value_builder_seeds_once_and_outranks_default_value(cx: &mut TestAppContext) {
    let state = cx.new(|cx| NumberState::new(cx, 0.));
    let state_for_view = state.clone();
    // The borrowed handle is the point: a caller keeping its own handle clones
    // nothing at the call site, and the chain needs no `&mut App`.
    let cx = open_host(cx, move || {
        NumberField::new(&state_for_view)
            .default_value(1.)
            .value(9.)
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).value());
    assert_eq!(
        seeded, 9.,
        "value must seed NumberState at first render and outrank defaultValue"
    );

    // The builder runs again on every refresh carrying the same `value`; the
    // seed must not rewrite the state a step has moved.
    press(cx, "tab");
    press(cx, "up");
    flush_frame(cx);
    let stepped = cx.update(|_, cx| state.read(cx).value());
    assert_eq!(
        stepped, 10.,
        "a re-render must not re-apply the value seed over the stepped state"
    );
}

fn nf_height(cx: &mut VisualTestContext, name: &'static str) -> f32 {
    f32::from(
        cx.debug_bounds(name)
            .expect("number field paints")
            .size
            .height,
    )
}

fn nf_validate_field(
    cx: &mut TestAppContext,
    value: f64,
    behavior: Option<herogpui_components::ValidationBehavior>,
) -> &mut VisualTestContext {
    let state = cx.new(|cx| NumberState::new(cx, value));
    let state_for_view = state;
    open_host(cx, move || {
        let field = NumberField::new(state_for_view.clone())
            .min_value(0.)
            .max_value(100.)
            .step(1.)
            .validate(|v| (*v < 10.).then(|| "Too small".into()));
        let field = match behavior {
            Some(b) => field.validation_behavior(b),
            None => field,
        };
        gpui::div()
            .debug_selector(|| "nf".to_owned())
            .child(field)
            .into_any_element()
    })
}

#[gpui::test]
fn number_field_native_validate_is_gated_until_a_stepper_commits(cx: &mut TestAppContext) {
    let cx = nf_validate_field(cx, 5., None);
    cx.run_until_parked();
    let rest = nf_height(cx, "nf");
    // Increment button (right edge of the 220px field) commits on click.
    cx.simulate_click(point(px(200.), px(18.)), Modifiers::none());
    flush_frame(cx);
    harness::wait_ms(cx, 800);
    assert!(
        nf_height(cx, "nf") > rest,
        "a stepper click must commit the gated validation and show the error"
    );
}

#[gpui::test]
fn number_field_native_validate_arrow_up_commits(cx: &mut TestAppContext) {
    let cx = nf_validate_field(cx, 5., None);
    cx.run_until_parked();
    let rest = nf_height(cx, "nf");
    press(cx, "tab");
    press(cx, "up");
    harness::wait_ms(cx, 800);
    assert!(
        nf_height(cx, "nf") > rest,
        "ArrowUp must commit the gated validation and show the error"
    );
}

#[gpui::test]
fn number_field_native_validate_home_end_commit(cx: &mut TestAppContext) {
    let cx = nf_validate_field(cx, 5., None);
    cx.run_until_parked();
    let rest = nf_height(cx, "nf");
    press(cx, "tab");
    // Home/End step the value to a bound and either commits the gated
    // snapshot. End would land on max_value 100, which passes `validate`, so
    // Home lands on min_value 0, which fails it and makes the commit visible.
    press(cx, "home");
    harness::wait_ms(cx, 800);
    assert!(
        nf_height(cx, "nf") > rest,
        "Home must commit the gated validation and show the error"
    );
}

#[gpui::test]
fn number_field_blur_without_edit_does_not_commit(cx: &mut TestAppContext) {
    let cx = nf_validate_field(cx, 5., None);
    cx.run_until_parked();
    let rest = nf_height(cx, "nf");
    press(cx, "tab");
    // Move focus away without editing: left_focus compares the unchanged text,
    // so nothing commits and the gated error stays hidden.
    press(cx, "tab");
    harness::wait_ms(cx, 800);
    assert_eq!(
        nf_height(cx, "nf"),
        rest,
        "focus then blur with no edit must not commit the gated validation"
    );
}

#[gpui::test]
fn number_field_type_then_blur_commits(cx: &mut TestAppContext) {
    let state = cx.new(|cx| NumberState::new(cx, 20.));
    let state_for_view = state;
    let other = cx.new(|cx| herogpui_components::InputState::new(cx));
    let plain = other.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                gpui::div().debug_selector(|| "nf".to_owned()).child(
                    NumberField::new(state_for_view.clone())
                        .min_value(0.)
                        .max_value(100.)
                        .step(1.)
                        .validate(|v| (*v < 10.).then(|| "Too small".into())),
                ),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "other".to_owned())
                    .child(herogpui_components::Input::new(plain.clone()).label("Other")),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let rest = nf_height(cx, "nf");
    // Focus the number input, replace the value with an invalid one, then move
    // focus to the other field: the blur commits the gated snapshot.
    cx.simulate_click(point(px(60.), px(18.)), Modifiers::none());
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("5");
    cx.update(|window, cx| {
        let handle = other.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    harness::wait_ms(cx, 800);
    assert!(
        nf_height(cx, "nf") > rest,
        "editing then blurring must commit the gated validation and show the error"
    );
}

#[gpui::test]
fn number_field_typing_after_commit_keeps_display_until_enter(cx: &mut TestAppContext) {
    let cx = nf_validate_field(cx, 5., None);
    cx.run_until_parked();
    let rest = nf_height(cx, "nf");
    // Commit the invalid value with ArrowUp (5 -> 6, still < 10).
    press(cx, "tab");
    press(cx, "up");
    harness::wait_ms(cx, 800);
    let shown = nf_height(cx, "nf");
    assert!(shown > rest, "ArrowUp must commit and show the error");
    // Type a now-valid value while focused: live validity turns valid but the
    // displayed snapshot stays stale, so the error row must remain.
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("15");
    cx.run_until_parked();
    assert_eq!(
        nf_height(cx, "nf"),
        shown,
        "typing after a commit must not refresh the displayed error"
    );
    // Enter is the commit trigger that lands the now-valid snapshot.
    press(cx, "enter");
    harness::wait_ms(cx, 800);
    assert_eq!(
        nf_height(cx, "nf"),
        rest,
        "Enter must commit the fresh valid snapshot and clear the error"
    );
}

#[gpui::test]
fn number_field_allow_validate_shows_live_at_rest(cx: &mut TestAppContext) {
    let cx = nf_validate_field(cx, 5., Some(herogpui_components::ValidationBehavior::Allow));
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    assert!(
        nf_height(cx, "nf") > px(36.).into(),
        "Allow must paint the live validation error at rest"
    );
}
