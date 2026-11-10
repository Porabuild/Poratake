//! ColorField focus-loss contracts: React Aria's built-in blur commit and the
//! `on_blur` hook.
//!
//! Pinned React Aria merges `onBlur: commit` into a colour field's input
//! (`useColorField`), and a channel field inherits NumberField's blur commit.
//! Pinned `useColorFieldState::commit` restores text that no longer parses to
//! the *formatted* last-committed value, leaves valid text alone, and keeps an
//! emptied field empty (its committed null) — without firing `onChange` for
//! the rejected text. HeroGPUI commits on every keystroke; blur only adds the
//! restore, and it is built in, not gated on the hook.
//!
//! GPUI blanks focus-event paths for inactive windows, including its headless
//! test platform, so these tests exercise the render-time observation leg the
//! component shares with `util::on_focus_leave`: moving focus to another tab
//! stop is observed on the next frame. Every field here has an editable
//! sibling because a lone tab stop cannot lose focus.

use crate::harness;

use crate::source_scan;

use gpui::{prelude::*, px, TestAppContext, VisualTestContext};
use herogpui_components::{ColorChannel, ColorField, InputState, PickerColor, ValidationBehavior};
use source_scan::{component_src, scope_contains};

use harness::{click, events, open_host, press};

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn text(cx: &mut VisualTestContext, state: &gpui::Entity<InputState>) -> String {
    cx.update(|_, cx| state.read(cx).value().to_owned())
}

/// The committed value's formatted text — `#FF0000`, not the raw `zz` —
/// replaces text that does not parse, and no `onChange` fires for it.
#[gpui::test]
fn invalid_text_reverts_to_the_committed_value_on_blur(cx: &mut TestAppContext) {
    let recorded = events();
    let red = PickerColor::from_hex("#FF0000").expect("red");
    let green = PickerColor::from_hex("#00FF00").expect("green");
    let state = cx.new(|cx| InputState::with_value(cx, "#FF0000"));
    let state_for_view = state.clone();
    let sibling = cx.new(|cx| InputState::with_value(cx, "#00FF00"));
    let sibling_for_view = sibling;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorField::new("blur-invalid", red)
                    .default_value(red)
                    .state(state_for_view.clone())
                    .on_change(move |color, _, _| {
                        recorded
                            .borrow_mut()
                            .push(color.map_or_else(|| "none".into(), |_| "color".into()));
                    }),
            )
            .child(
                ColorField::new("blur-target", green)
                    .default_value(green)
                    .state(sibling_for_view.clone()),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "ctrl-a");
    cx.simulate_input("zz");
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["none", "none"],
        "every keystroke still commits; text that cannot parse reports None"
    );
    assert_eq!(text(cx, &state), "zz");

    let reported = recorded.borrow().len();
    press(cx, "tab");
    flush_frame(cx);
    assert_eq!(
        text(cx, &state),
        "#FF0000",
        "blur restores the formatted last-committed value"
    );
    assert_eq!(
        recorded.borrow().len(),
        reported,
        "the revert must not fire on_change for the invalid text"
    );
}

/// The channel field inherits NumberField's blur commit: text typed past its
/// last valid prefix restores the formatted committed channel value.
#[gpui::test]
fn channel_field_reverts_partial_text_to_the_formatted_value_on_blur(cx: &mut TestAppContext) {
    let recorded = events();
    let hue = PickerColor::hsb(180., 1., 1.);
    let state = cx.new(|cx| InputState::with_value(cx, "180"));
    let state_for_view = state.clone();
    let sibling = cx.new(|cx| InputState::with_value(cx, "180"));
    let sibling_for_view = sibling;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorField::new("blur-channel", hue)
                    .default_value(hue)
                    .state(state_for_view.clone())
                    .channel(ColorChannel::Hue)
                    .on_change(move |color, _, _| {
                        recorded
                            .borrow_mut()
                            .push(color.map_or_else(|| "none".into(), |_| "color".into()));
                    }),
            )
            .child(
                ColorField::new("blur-channel-target", hue)
                    .default_value(hue)
                    .state(sibling_for_view.clone())
                    .channel(ColorChannel::Hue),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "ctrl-a");
    cx.simulate_input("18");
    cx.simulate_input("x");
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["color", "color", "none"],
        "18 commits as it is typed; the trailing x reports None"
    );
    assert_eq!(text(cx, &state), "18x");

    press(cx, "tab");
    flush_frame(cx);
    assert_eq!(
        text(cx, &state),
        "18",
        "blur restores the formatted committed channel value"
    );
    assert_eq!(recorded.borrow().as_slice(), ["color", "color", "none"]);
}

/// The revert restores the LAST committed value, not the field's first-render
/// seed: a valid change commits per keystroke and moves the committed value
/// off the seed, so text typed past it must restore that change's formatted
/// value on blur.
#[gpui::test]
fn invalid_text_reverts_to_the_last_committed_value_not_the_seed(cx: &mut TestAppContext) {
    let recorded = events();
    let red = PickerColor::from_hex("#FF0000").expect("red");
    let green = PickerColor::from_hex("#00FF00").expect("green");
    let state = cx.new(|cx| InputState::with_value(cx, "#FF0000"));
    let state_for_view = state.clone();
    let sibling = cx.new(|cx| InputState::with_value(cx, "#00FF00"));
    let sibling_for_view = sibling;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorField::new("blur-last", red)
                    .default_value(red)
                    .state(state_for_view.clone())
                    .on_change(move |color, _, _| {
                        recorded
                            .borrow_mut()
                            .push(color.map_or_else(|| "none".into(), |_| "color".into()));
                    }),
            )
            .child(
                ColorField::new("blur-last-target", green)
                    .default_value(green)
                    .state(sibling_for_view.clone()),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "ctrl-a");
    cx.simulate_input("00ff00");
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("color"),
        "the completed hex commits, so the committed value moves off the seed"
    );
    cx.simulate_input("zz");
    flush_frame(cx);
    assert_eq!(text(cx, &state), "00ff00zz");

    let reported = recorded.borrow().len();
    press(cx, "tab");
    flush_frame(cx);
    assert_eq!(
        text(cx, &state),
        "#00FF00",
        "blur restores the last committed value, not the first-render seed"
    );
    assert_eq!(
        recorded.borrow().len(),
        reported,
        "the revert must not fire on_change for the invalid text"
    );
}

/// Valid typed text is untouched by the revert path and stays exactly as
/// typed.
#[gpui::test]
fn valid_text_survives_blur_unchanged(cx: &mut TestAppContext) {
    let recorded = events();
    let red = PickerColor::from_hex("#FF0000").expect("red");
    let green = PickerColor::from_hex("#00FF00").expect("green");
    let state = cx.new(|cx| InputState::with_value(cx, "#FF0000"));
    let state_for_view = state.clone();
    let sibling = cx.new(|cx| InputState::with_value(cx, "#00FF00"));
    let sibling_for_view = sibling;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorField::new("blur-valid", red)
                    .default_value(red)
                    .state(state_for_view.clone())
                    .on_change(move |color, _, _| {
                        recorded
                            .borrow_mut()
                            .push(color.map_or_else(|| "none".into(), |_| "color".into()));
                    }),
            )
            .child(
                ColorField::new("blur-valid-target", green)
                    .default_value(green)
                    .state(sibling_for_view.clone()),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "ctrl-a");
    cx.simulate_input("00ff00");
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("color"),
        "the completed hex commits through the every-keystroke path"
    );

    press(cx, "tab");
    flush_frame(cx);
    assert_eq!(
        text(cx, &state),
        "00ff00",
        "valid text must not be rewritten or reverted"
    );
}

/// The hook fires once per focus loss, before the built-in revert, and the
/// revert still runs.
#[gpui::test]
fn on_blur_fires_once_per_focus_loss(cx: &mut TestAppContext) {
    let blurs = events();
    let red = PickerColor::from_hex("#FF0000").expect("red");
    let green = PickerColor::from_hex("#00FF00").expect("green");
    let state = cx.new(|cx| InputState::with_value(cx, "#FF0000"));
    let state_for_view = state.clone();
    let state_at_blur = state.clone();
    let sibling = cx.new(|cx| InputState::with_value(cx, "#00FF00"));
    let sibling_for_view = sibling;
    let for_view = blurs.clone();
    let cx = open_host(cx, move || {
        let blurs = for_view.clone();
        let state_at_blur = state_at_blur.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorField::new("blur-hook", red)
                    .default_value(red)
                    .state(state_for_view.clone())
                    .on_blur(move |_, cx| {
                        blurs
                            .borrow_mut()
                            .push(format!("blur:{}", state_at_blur.read(cx).value()));
                    }),
            )
            .child(
                ColorField::new("blur-hook-target", green)
                    .default_value(green)
                    .state(sibling_for_view.clone()),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "ctrl-a");
    cx.simulate_input("zz");
    flush_frame(cx);
    press(cx, "tab");
    flush_frame(cx);
    assert_eq!(
        blurs.borrow().as_slice(),
        ["blur:zz"],
        "the hook fires once and sees the raw text, the way React Aria chains onBlur ahead of commit"
    );
    assert_eq!(
        text(cx, &state),
        "#FF0000",
        "the built-in revert still runs"
    );

    click(cx, 60., 18.);
    press(cx, "ctrl-a");
    cx.simulate_input("zz");
    flush_frame(cx);
    press(cx, "tab");
    flush_frame(cx);
    assert_eq!(
        blurs.borrow().as_slice(),
        ["blur:zz", "blur:zz"],
        "each focus loss fires the hook exactly once"
    );
}

/// An emptied field is React Aria's committed null: blur leaves it empty
/// instead of restoring the previous colour.
#[gpui::test]
fn emptied_text_stays_empty_on_blur(cx: &mut TestAppContext) {
    let recorded = events();
    let red = PickerColor::from_hex("#FF0000").expect("red");
    let green = PickerColor::from_hex("#00FF00").expect("green");
    let state = cx.new(|cx| InputState::with_value(cx, "#FF0000"));
    let state_for_view = state.clone();
    let sibling = cx.new(|cx| InputState::with_value(cx, "#00FF00"));
    let sibling_for_view = sibling;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorField::new("blur-empty", red)
                    .default_value(red)
                    .state(state_for_view.clone())
                    .on_change(move |color, _, _| {
                        recorded
                            .borrow_mut()
                            .push(color.map_or_else(|| "none".into(), |_| "color".into()));
                    }),
            )
            .child(
                ColorField::new("blur-empty-target", green)
                    .default_value(green)
                    .state(sibling_for_view.clone()),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "ctrl-a");
    press(cx, "backspace");
    flush_frame(cx);
    assert_eq!(text(cx, &state), "");

    let reported = recorded.borrow().len();
    press(cx, "tab");
    flush_frame(cx);
    assert_eq!(
        text(cx, &state),
        "",
        "empty text is the committed null state, not invalid text to restore"
    );
    assert_eq!(recorded.borrow().len(), reported);
}

/// The built-in revert is parity behaviour, not a feature of the hook: a field
/// with no `on_blur` still restores unparseable text.
#[gpui::test]
fn revert_without_the_hook_and_a_click_blur(cx: &mut TestAppContext) {
    let red = PickerColor::from_hex("#FF0000").expect("red");
    let green = PickerColor::from_hex("#00FF00").expect("green");
    let state = cx.new(|cx| InputState::with_value(cx, "#FF0000"));
    let state_for_view = state.clone();
    let sibling = cx.new(|cx| InputState::with_value(cx, "#00FF00"));
    let sibling_for_view = sibling;
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                ColorField::new("blur-no-hook", red)
                    .default_value(red)
                    .state(state_for_view.clone()),
            )
            .child(
                ColorField::new("blur-no-hook-target", green)
                    .default_value(green)
                    .state(sibling_for_view.clone()),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "ctrl-a");
    cx.simulate_input("zz");
    flush_frame(cx);
    assert_eq!(text(cx, &state), "zz");

    // Focus also leaves through the pointer: pressing the sibling input
    // blurs the first field, with no keyboard involved.
    click(cx, 60., 74.);
    flush_frame(cx);
    assert_eq!(
        text(cx, &state),
        "#FF0000",
        "a pointer blur reverts the text without any on_blur hook"
    );
}

/// The active-window leg cannot be driven on the headless platform — gpui
/// blanks its focus events there — so its wiring is pinned in source, the way
/// `tabs_deep.rs` pins paint-only contracts. The `on_focus_out` listener must
/// clear its subscription slot when it fires and be re-armed every frame,
/// because the revert closure captures that frame's committed value: a
/// listener left armed across keystrokes restores the seed a real window has
/// already typed past.
#[test]
fn the_focus_out_listener_disarms_on_fire_so_every_frame_re_arms_fresh() {
    let source = component_src("color_picker/field.rs");
    // No armed-once gate may sit in front of the registration: every render
    // must rebuild the listener with a fresh revert.
    assert!(
        !source.contains("let armed ="),
        "the blur listener must re-arm every frame; an armed-once gate pins \
         the first render's committed value onto every later blur"
    );
    // The listener is a one-shot: it downgrades the subscription slot and
    // clears it on fire, between its registration and the store.
    let listener_at = source
        .find("window.on_focus_out(&blur_focus")
        .expect("the enabled path must arm an on_focus_out listener");
    let stored_at = source[listener_at..]
        .find("*slot = Some(listener)")
        .expect("the listener must be stored in the subscription slot");
    let arming = &source[listener_at..listener_at + stored_at];
    assert!(
        arming.contains("disarmer.upgrade()") && arming.contains("*slot = None"),
        "the on_focus_out listener must clear the subscription slot when it \
         fires, so the next frame re-arms with a fresh committed value"
    );
    // The disarmer must be this slot's own weak handle, not another state's.
    scope_contains(
        &source,
        "window.on_focus_out(&blur_focus",
        "blur_subscription.downgrade()",
    )
    .unwrap_or_else(|err| panic!("{err}"));
    // The render-time leg defers the revert out of render, so the InputState
    // mutation and the caller's hook run the way the event leg runs them.
    let leg_at = source
        .find("The inactive-window leg")
        .expect("the inactive-window leg must keep its comment");
    let disabled_at = source[leg_at..]
        .find("} else if blur_subscription.read(cx).is_some()")
        .expect("the disabled-path disarm must follow the enabled path");
    assert!(
        source[leg_at..leg_at + disabled_at].contains("window.defer(cx"),
        "the render-time leg must defer the revert out of render"
    );
}

fn gated_color_height(cx: &mut VisualTestContext, name: &'static str) -> f32 {
    f32::from(
        cx.debug_bounds(name)
            .expect("color field paints")
            .size
            .height,
    )
}

/// An editable ColorField (bound `InputState` -> inner `Input` with
/// `display_override`) plus a sibling to receive focus on blur.
fn editable_gated_color<'a>(
    cx: &'a mut TestAppContext,
    text: &'static str,
    channel: Option<ColorChannel>,
    behavior: Option<ValidationBehavior>,
) -> &'a mut VisualTestContext {
    let red = PickerColor::from_hex("#FF0000").expect("red");
    let green = PickerColor::from_hex("#00FF00").expect("green");
    let state = cx.new(|cx| InputState::with_value(cx, text));
    let state_for_view = state;
    let sibling = cx.new(|cx| InputState::with_value(cx, "#00FF00"));
    let sibling_for_view = sibling;
    open_host(cx, move || {
        let mut field = ColorField::new("cf-gated", red)
            .default_value(red)
            .state(state_for_view.clone())
            .validate(|_| Some("Always invalid".into()));
        if let Some(channel) = channel {
            field = field.channel(channel);
        }
        if let Some(behavior) = behavior {
            field = field.validation_behavior(behavior);
        }
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(gpui::div().debug_selector(|| "cf".to_owned()).child(field))
            .child(
                gpui::div().debug_selector(|| "target".to_owned()).child(
                    ColorField::new("cf-target", green)
                        .default_value(green)
                        .state(sibling_for_view.clone()),
                ),
            )
            .into_any_element()
    })
}

#[gpui::test]
fn color_field_native_validate_is_gated_until_blur_commits(cx: &mut TestAppContext) {
    let cx = editable_gated_color(cx, "#FF0000", None, None);
    cx.run_until_parked();
    let rest = gated_color_height(cx, "cf");
    // Change the hex text then leave the field: blur commits the gated
    // snapshot and the inner Input's error row appears.
    click(cx, 60., 18.);
    press(cx, "ctrl-a");
    cx.simulate_input("00FF00");
    press(cx, "tab");
    harness::wait_ms(cx, 800);
    assert!(
        gated_color_height(cx, "cf") > rest,
        "editing then blurring must commit the gated validation and show the error"
    );
}

#[gpui::test]
fn color_field_native_validate_enter_commits(cx: &mut TestAppContext) {
    let cx = editable_gated_color(cx, "#FF0000", None, None);
    cx.run_until_parked();
    let rest = gated_color_height(cx, "cf");
    click(cx, 60., 18.);
    press(cx, "enter");
    harness::wait_ms(cx, 800);
    assert!(
        gated_color_height(cx, "cf") > rest,
        "Enter must commit the gated validation and show the error"
    );
}

#[gpui::test]
fn color_field_native_validate_channel_step_commits(cx: &mut TestAppContext) {
    let cx = editable_gated_color(cx, "128", Some(ColorChannel::Red), None);
    cx.run_until_parked();
    let rest = gated_color_height(cx, "cf");
    click(cx, 60., 18.);
    press(cx, "up");
    harness::wait_ms(cx, 800);
    assert!(
        gated_color_height(cx, "cf") > rest,
        "ArrowUp on a channel field must commit the gated validation and show the error"
    );
}

#[gpui::test]
fn color_field_blur_without_edit_on_empty_text_commits_nothing(cx: &mut TestAppContext) {
    let cx = editable_gated_color(cx, "", None, None);
    cx.run_until_parked();
    let rest = gated_color_height(cx, "cf");
    // Focus then blur with the text still empty: neither "changed" nor
    // "non-empty and parses" holds, so nothing commits.
    click(cx, 60., 18.);
    press(cx, "tab");
    harness::wait_ms(cx, 800);
    assert_eq!(
        gated_color_height(cx, "cf"),
        rest,
        "focus then blur on empty, unedited text must not commit the gated validation"
    );
}

#[gpui::test]
fn color_field_allow_validate_shows_live_at_rest(cx: &mut TestAppContext) {
    let cx = editable_gated_color(cx, "#FF0000", None, Some(ValidationBehavior::Allow));
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    assert!(
        gated_color_height(cx, "cf") > px(36.).into(),
        "Allow must paint the live validation error at rest"
    );
}
