//! Behaviour tests for the FIELD and FORM components: the three group
//! controls (CheckboxGroup, RadioGroup, SwitchGroup), the bare Input, the
//! NumberField steppers, the TimeField segments and the Form's collect-and-
//! validate path.
//!
//! Everything static about them is measured by the `.shots/*.py` audits; these
//! tests drive the controls with simulated clicks and keystrokes and assert on
//! recorded callbacks, state entities the test owns, and behavioural probes
//! only — never on appearance.
//!
//! Geometry is derived from the components' own constants and from gpui's own
//! defaults, not guessed:
//!
//! - Every field row is `util::FIELD_HEIGHT` = 36px tall at the window origin
//!   unless the component adds a label/description on top.
//! - A bare `Input` is wrapped in `max_w(320)` (`Input::render`), and the
//!   field's own `px(12)` padding leaves a 20px `size-5` clear button in
//!   x 288..308, centre (298, 18) — the same maths as ComboBox's chevron in
//!   pickers.rs.
//! - The checkbox control is `size-4` = 16px (`checkbox.rs` `box_px`); the
//!   group rows are `gap(16)` apart and text-sized. A row's height is
//!   `max(16px, the label line)`, and gpui's *default* line height is `phi()`
//!   = 1.618034 × font size (gpui `style.rs`, `Default for TextStyle`; the
//!   port never overrides `line_height` on these rows), so a 14px label line
//!   is 22.65px. Row 1's 16px box therefore centres at
//!   22.65 + 16 + 22.65/2 ≈ 50. The clicks below land at (8, 11) and (8, 46);
//!   row *i*'s box is `[i·(H+16) + (H-16)/2, +16]` tall, so (8, 46) stays
//!   inside row 1's box for any line height H in ~15..25px — the assertion
//!   does not trust the exact default.
//! - A switch track is 40x20 (`switch.rs` `Size::Md`); a `SwitchGroup`'s rows
//!   are `gap(16)` apart with a 16px label line (25.89px at phi), so the two
//!   tracks centre at y ≈ 13 and y ≈ 54.8. Clicks at (20, 13) and (20, 52)
//!   land inside the respective track for any line height in ~17..30px.
//! - `NumberField` is a 220px group (`w(px(220.))` in number_field.rs) with a
//!   40px (`w-10`) stepper cell at each end: the increment button centres at
//!   (200, 18).
//!   Its documented vertical-chevron composition instead reserves the trailing
//!   24px for two 24x18px cells, centred at (208, 9) and (208, 27).
//! - A `Form` stacks its children `gap(16)` (form.rs, `gap(px(16.))`). A bare
//!   Input is 36px, so field 1 spans y 0..36, field 2 y 52..88, and the md
//!   submit `Button` (36px — `Size::Md::control_height`, herogpui-core
//!   enums.rs) spans y 104..140; the clicks are (60, 18), (60, 70), (60, 122).
//!
//! Keystrokes are delivered exactly as the shared harness does (`press`), and
//! gpui redraws a dirty window before dispatching the next key event
//! (`Window::dispatch_key_event` draws first), so a roving tab stop — the
//! radios — moves its handle between presses, which is what the arrows
//! depend on.

use crate::harness;

use std::{cell::RefCell, collections::HashSet, rc::Rc};

use gpui::{
    point, prelude::*, px, Focusable, Modifiers, SharedString, TestAppContext, VisualTestContext,
};
use herogpui_components::{
    Button, Checkbox, CheckboxGroup, CheckboxOption, CheckboxState, Form, FormData, FormField,
    Input, InputOTP, InputState, NumberField, NumberState, OtpState, RadioGroup, RadioOption,
    Switch, SwitchGroup, Time, TimeField, TimeState,
};

use harness::{click, events, open_host, press, still, Events};

/// The keys of a selection joined in a stable order.
///
/// The groups report a `HashSet`, which iterates in no particular order;
/// sorting makes the recorded report deterministic.
fn sorted_join(keys: &HashSet<SharedString>) -> String {
    let mut keys: Vec<String> = keys.iter().map(ToString::to_string).collect();
    keys.sort();
    keys.join(",")
}

fn record_checkbox_state(states: &Events, state: CheckboxState) {
    states.borrow_mut().push(format!(
        "{}|{}|{}|{}|{}|{}",
        state.is_selected,
        state.is_indeterminate,
        state.is_disabled,
        state.is_read_only,
        state.is_invalid,
        state.is_required
    ));
}

/// A submission rendered the way the gallery's own Form demo renders it:
/// `name=value` pairs in registration order, values through `as_text`.
fn record_data(data: &FormData) -> String {
    data.iter()
        .map(|(name, value)| format!("{name}={}", value.as_text()))
        .collect::<Vec<_>>()
        .join(",")
}

// ---------------------------------------------------------------------------
// CheckboxGroup
// ---------------------------------------------------------------------------

#[gpui::test]
fn checkbox_group_toggles_two_keys(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        CheckboxGroup::new(
            "cg-two",
            vec![
                CheckboxOption::new("alpha", "Alpha"),
                CheckboxOption::new("beta", "Beta"),
                CheckboxOption::new("gamma", "Gamma"),
            ],
        )
        .default_value(Vec::<SharedString>::new())
        .on_change(move |set, _, _| changes.borrow_mut().push(sorted_join(set)))
        .into_any_element()
    });

    // Row 0's 16px box centre is (8, 11) and row 1's (8, 46) — see the file
    // doc comment; the click lands inside row 1's box for any line height
    // in ~15..25px.
    click(cx, 8., 11.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha"],
        "clicking the first box must select its key"
    );
    click(cx, 8., 46.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta"],
        "clicking the second box must keep the first pick"
    );
    click(cx, 8., 11.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta", "beta"],
        "re-clicking a picked box must remove it, which requires the group \
         to remember the earlier picks"
    );
}

#[gpui::test]
fn checkbox_indeterminate_does_not_block_toggle(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        // `is_indeterminate` only swaps the dash for the check; the box must
        // still respond to a press.
        Checkbox::new("cb-indeterminate")
            .label("Indeterminate")
            .is_indeterminate(true)
            .on_change(move |checked, _, _| {
                changes.borrow_mut().push(format!("change:{checked}"));
            })
            .into_any_element()
    });

    // The 16px control box is centred in a 22.65px row (see the file doc
    // comment), so its band is roughly y 3..19; (8, 11) is its centre.
    click(cx, 8., 11.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:true"],
        "an indeterminate box must still report a change when clicked"
    );
}

/// Pinned HeroUI v3.2.4 hands `Checkbox.Indicator` the root field state. In
/// particular, selected and indeterminate are independent: the custom dash
/// example is indeterminate without being selected.
#[gpui::test]
fn checkbox_indicator_receives_distinct_field_state(cx: &mut TestAppContext) {
    let states = events();
    let recorded = states.clone();
    open_host(cx, move || {
        let indeterminate_states = states.clone();
        let selected_states = states.clone();
        gpui::div()
            .child(
                Checkbox::new("cb-indicator-state")
                    .is_indeterminate(true)
                    .is_disabled(true)
                    .is_read_only(true)
                    .is_invalid(true)
                    .is_required(true)
                    .indicator(move |state| {
                        record_checkbox_state(&indeterminate_states, state);
                        gpui::div().into_any_element()
                    }),
            )
            .child(
                Checkbox::new("cb-indicator-selected")
                    .default_selected(true)
                    .indicator(move |state| {
                        record_checkbox_state(&selected_states, state);
                        gpui::div().into_any_element()
                    }),
            )
            .into_any_element()
    });

    let recorded = recorded.borrow();
    assert!(
        recorded
            .iter()
            .any(|state| state == "false|true|true|true|true|true"),
        "an indeterminate indicator must receive the full field state without inventing selection"
    );
    assert!(
        recorded
            .iter()
            .any(|state| state == "true|false|false|false|false|false"),
        "a selected indicator must receive selection without inventing indeterminate state"
    );
}

/// Pinned HeroUI v3.2.4 calls Checkbox root children with the live
/// `CheckboxFieldRenderProps`, not an owner-derived snapshot.
#[gpui::test]
fn checkbox_content_receives_live_field_state(cx: &mut TestAppContext) {
    let states = events();
    let recorded = states.clone();
    let cx = open_host(cx, move || {
        let states = states.clone();
        Checkbox::new("cb-content-state")
            .is_indeterminate(true)
            .is_invalid(true)
            .is_required(true)
            .label(
                gpui::div()
                    .debug_selector(|| "cb-static-label".to_owned())
                    .child("Static label"),
            )
            .content(move |state| {
                record_checkbox_state(&states, state);
                gpui::div()
                    .debug_selector(|| "cb-live-content".to_owned())
                    .child("Live state")
                    .into_any_element()
            })
            .into_any_element()
    });

    click(cx, 8., 11.);
    assert!(
        cx.debug_bounds("cb-static-label").is_none(),
        "functional root content must replace static children"
    );
    assert!(
        cx.debug_bounds("cb-live-content").is_some(),
        "the replacement assertion requires painted functional content"
    );
    let recorded = recorded.borrow();
    assert!(
        recorded
            .iter()
            .any(|state| state == "false|true|false|false|true|true"),
        "root content must receive the initial field state"
    );
    assert!(
        recorded
            .iter()
            .any(|state| state == "true|true|false|false|true|true"),
        "uncontrolled selection must rerender root content with the live state"
    );
}

#[gpui::test]
fn checkbox_controlled_content_rerenders_after_owner_accepts_change(cx: &mut TestAppContext) {
    let for_view_selected = Rc::new(RefCell::new(false));
    let states = events();
    let recorded = states.clone();
    let cx = open_host(cx, move || {
        let selected_value = *for_view_selected.borrow();
        let selected = for_view_selected.clone();
        let states = states.clone();
        Checkbox::new("cb-controlled-content-state")
            .is_selected(selected_value)
            .on_change(move |next, window, _| {
                *selected.borrow_mut() = *next;
                window.refresh();
            })
            .content(move |state| {
                record_checkbox_state(&states, state);
                gpui::div().child("Controlled state").into_any_element()
            })
            .into_any_element()
    });

    click(cx, 8., 11.);
    let recorded = recorded.borrow();
    assert!(
        recorded
            .iter()
            .any(|state| state == "false|false|false|false|false|false"),
        "controlled content must receive the owner's initial value"
    );
    assert!(
        recorded
            .iter()
            .any(|state| state == "true|false|false|false|false|false"),
        "controlled content must rerender after the owner accepts the reported change"
    );
}

// ---------------------------------------------------------------------------
// RadioGroup
// ---------------------------------------------------------------------------

#[gpui::test]
fn radio_group_arrows_move_and_select(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();
    let switches = events();
    let switched = switches.clone();
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let switches = switches.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                RadioGroup::new(
                    "rg-roving",
                    vec!["One".into(), "Two".into(), "Three".into()],
                )
                .default_value("One")
                .on_change(move |value, _, _| picks.borrow_mut().push(value.to_string())),
            )
            // A second tab stop after the group, so "Tab leaves the group"
            // is observable: Tab must move past the radios to this switch,
            // and the radios must stop answering the arrows.
            .child(
                Switch::new("rg-exit-probe").on_change(move |checked, _, _| {
                    switches.borrow_mut().push(format!("change:{checked}"));
                }),
            )
            .into_any_element()
    });

    // A radio group is ONE tab stop (radio_group.rs: "the stop is the
    // selected option, or the first when nothing is selected yet"): Tab
    // enters on row 0, and each arrow both moves the selection and pulls
    // the focus along (the selected row claims the group's handle on the
    // next render — AGENTS.md's roving tab stop).
    press(cx, "tab");
    press(cx, "down");
    press(cx, "down");
    press(cx, "up");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Two", "Three", "Two"],
        "Down Down Up must move AND select within the one tab stop"
    );

    // Tab must leave the group, not walk its rows: the next stop is the
    // switch, and a Down at the switch would have selected row 2 had the
    // radios kept the keys.
    press(cx, "tab");
    press(cx, "down");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Two", "Three", "Two"],
        "the group must not answer the arrows after Tab leaves it"
    );

    // And the focus is not lost: Space activates the switch that Tab landed
    // on, which proves the group is out of the tab order rather than merely
    // out of focus detail.
    press(cx, "space");
    assert_eq!(
        switched.borrow().as_slice(),
        ["change:true"],
        "Tab must land on the next tab stop — the switch"
    );
}

#[gpui::test]
fn radio_group_disabled_option_is_skipped(cx: &mut TestAppContext) {
    // `Radio.isDisabled` — a disabled option draws dimmed and answers no
    // clicks ("Whether the radio button is disabled" — heroui.com/react,
    // `### Radio`; Interactive States: disabled is "reduced opacity, no
    // pointer events"), and the group's arrows and roving tab stop pass it
    // by. Index 1 is disabled, so Down from row 0 lands on row 2, Up from
    // row 2 wraps past it back to row 0, and a pointer press on the disabled
    // row changes nothing.
    let picks = events();
    let picked = picks.clone();
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        RadioGroup::new(
            "rg-disabled-opt",
            vec![
                "One".into(),
                RadioOption::new("Two").is_disabled(true),
                "Three".into(),
            ],
        )
        .default_value("One")
        .on_change(move |value, _, _| picks.borrow_mut().push(value.to_string()))
        .into_any_element()
    });

    // Tab enters the group on the selected option (row 0), and the arrows
    // must skip the disabled row: Down lands on 2 and Up from there wraps
    // back to 0 — never 1.
    press(cx, "tab");
    press(cx, "down");
    press(cx, "up");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Three", "One"],
        "Down and Up must skip the disabled option and land on the enabled \
         ones around it"
    );

    // A pointer press on the disabled option must select nothing: v3's
    // `status-disabled` is `pointer-events: none`, and this port leaves the
    // row without a click handler. Row 1's 16px box centre is (8, 46) — see
    // the file doc comment.
    click(cx, 8., 46.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Three", "One"],
        "the disabled option must not answer a click"
    );

    // The enabled row beside it still does: row 0's box centre is (8, 11).
    click(cx, 8., 11.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Three", "One", "One"],
        "an enabled option beside a disabled one must still answer a click"
    );
}

#[gpui::test]
fn radio_group_first_option_disabled_stays_reachable(cx: &mut TestAppContext) {
    // AGENTS.md's roving tab stop: a stop that rests on a disabled option
    // takes the group out of the tab order. With row 0 disabled and nothing
    // selected, the group's one stop must fall on the first *enabled* option,
    // so Tab still reaches the group. React Aria then walks from that focused
    // option, so Down advances to the following enabled option.
    let picks = events();
    let picked = picks.clone();
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        RadioGroup::new(
            "rg-first-disabled",
            vec![
                RadioOption::new("One").is_disabled(true),
                "Two".into(),
                "Three".into(),
            ],
        )
        .default_value("")
        .on_change(move |value, _, _| picks.borrow_mut().push(value.to_string()))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Three"],
        "Tab must reach the group with the first option disabled, and the \
         first Down must advance from the focused option"
    );
}

// ---------------------------------------------------------------------------
// SwitchGroup
// ---------------------------------------------------------------------------

#[gpui::test]
fn switch_group_reports_each_switch(cx: &mut TestAppContext) {
    let first = events();
    let first_rec = first.clone();
    let second = events();
    let second_rec = second.clone();
    let cx = open_host(cx, move || {
        let first = first.clone();
        let second = second.clone();
        SwitchGroup::new()
            .child(
                Switch::new("sg-1")
                    .label("One")
                    .on_change(move |checked, _, _| {
                        first.borrow_mut().push(format!("change:{checked}"));
                    }),
            )
            .child(
                Switch::new("sg-2")
                    .label("Two")
                    .on_change(move |checked, _, _| {
                        second.borrow_mut().push(format!("change:{checked}"));
                    }),
            )
            .into_any_element()
    });

    // Each switch's own click handler is what must fire: the md track is the
    // whole hit target (the label has none), and the two tracks centre at
    // (20, 13) and (20, 52) — see the file doc comment; each click lands
    // inside the right track for any line height in ~17..30px.
    click(cx, 20., 13.);
    click(cx, 20., 52.);
    assert_eq!(
        first_rec.borrow().as_slice(),
        ["change:true"],
        "clicking the first track must report that switch's change"
    );
    assert_eq!(
        second_rec.borrow().as_slice(),
        ["change:true"],
        "clicking the second track must report its own change, not the first's"
    );
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

#[gpui::test]
fn input_typing_and_clear(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Input::new(state)
            .is_clearable(true)
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .into_any_element()
    });

    // Click into the field (36px tall at the origin, capped at 320px wide),
    // which focuses it; each keystroke must both land in the state the test
    // owns and be reported.
    click(cx, 60., 18.);
    cx.simulate_input("hello");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["h", "he", "hel", "hell", "hello"],
        "typing must report the growing value on every keystroke"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "hello", "the InputState must hold what was typed");

    // The clear button is a 20px box at the field's inline end: the 320px
    // wrapper minus the field's px(12) padding minus half the box = 298,
    // vertically centred at 18 (`Input::render`, same geometry as
    // ComboBox's chevron in pickers.rs).
    click(cx, 298., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["h", "he", "hel", "hell", "hello", ""],
        "the clear button must report an empty value"
    );
    let cleared = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(cleared, "", "clearing must empty the InputState");
}

// ---------------------------------------------------------------------------
// NumberField
// ---------------------------------------------------------------------------

#[gpui::test]
fn number_field_steppers_and_bounds(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| NumberState::new(cx, 90.0));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        // Component-level bounds overrule the state's; the steppers bump by
        // `step` and clamp to `[min, max]`.
        NumberField::new(state)
            .min_value(0.0)
            .max_value(100.0)
            .step(10.0)
            .on_change(move |value, _, _| changes.borrow_mut().push(format!("{value}")))
            .into_any_element()
    });

    // The group is 220px wide and 36px tall with a 40px (`w-10`) increment
    // cell at the end: centre (200, 18) (number_field.rs). From 90, one step
    // hits the 100 maximum and a second step must stay there without reporting
    // a duplicate change.
    click(cx, 200., 18.);
    click(cx, 200., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["100"],
        "a bound no-op must not synthesize a duplicate on_change callback"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_string());
    assert_eq!(value, "100", "the NumberState must hold the clamped value");
}

#[gpui::test]
fn number_field_vertical_steppers_drive_both_directions(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| NumberState::new(cx, 10.0));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        NumberField::new(state_for_view.clone())
            .step(2.0)
            .vertical_steppers(true)
            .increment_icon(gpui::div())
            .decrement_icon(gpui::div())
            .on_change(move |value, _, _| changes.borrow_mut().push(format!("{value}")))
            .into_any_element()
    });

    // The trailing stack spans x=196..220. Its upper and lower 18px halves
    // are the increment and decrement buttons respectively.
    click(cx, 208., 9.);
    click(cx, 208., 27.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["12", "10"],
        "both custom vertical stepper cells must retain their press behaviour"
    );
    let value = cx.update(|_, cx| state.read(cx).value());
    assert_eq!(value.to_bits(), 10.0_f64.to_bits());
}

/// RAC's `useNumberField` keeps the input focused while a stepper is pressed
/// (`onButtonPressStart`, pointerType mouse), which is what makes the group
/// focus-within and paints the state ring. gpui's mouse-down default would
/// refocus the nearest ancestor and steal the handle back in the same
/// dispatch, so the stepper prevents the default after focusing the input.
#[gpui::test]
fn number_field_stepper_press_keeps_focus_on_the_input(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| NumberState::new(cx, 10.0));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        NumberField::new(state_for_view.clone())
            .step(2.0)
            .on_change(move |value, _, _| changes.borrow_mut().push(format!("{value}")))
            .into_any_element()
    });
    settle(cx);

    // Press and release the increment cell (centre 200, 18): one bump, and
    // the input — not the app root — holds the focus afterwards.
    click(cx, 200., 18.);
    flush_frame(cx);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["12"],
        "the stepper press must report exactly one increment"
    );
    cx.update(|window, cx| {
        let input_focus = state.read(cx).input.read(cx).focus_handle(cx);
        assert!(
            input_focus.is_focused(window),
            "the stepper press must leave focus on the input"
        );
    });
}

// ---------------------------------------------------------------------------
// TimeField
// ---------------------------------------------------------------------------

#[gpui::test]
fn time_field_segments_answer_arrows(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 30)));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        TimeField::new(state_for_view.clone())
            .on_change(move |time, _, _| {
                let text = time
                    .map(|t| format!("{:02}:{:02}", t.hour, t.minute))
                    .unwrap_or_default();
                changes.borrow_mut().push(text);
            })
            .into_any_element()
    });

    // The field is the page's only tab stop (`TimeField` takes a
    // `tab_stop(true)` handle) and the focused segment starts at Hour
    // (time_field.rs `TimeState`). No coordinates are involved: the arrows
    // are read by the group's `on_key_down`.
    press(cx, "tab");
    press(cx, "up");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["10:30"],
        "Up must step the focused hour segment by one hour"
    );

    // Right walks the segment list to Minute; Up steps it.
    press(cx, "right");
    press(cx, "up");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["10:30", "10:31"],
        "Right must move to the minute segment and Up must step it"
    );
}

// ---------------------------------------------------------------------------
// Form
// ---------------------------------------------------------------------------

#[gpui::test]
fn form_submit_reports_named_fields(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let name_state = cx.new(|cx| InputState::new(cx));
    let email_state = cx.new(|cx| InputState::new(cx));
    let name_for_view = name_state;
    let email_for_view = email_state;
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let name_state = name_for_view.clone();
        let email_state = email_for_view.clone();
        // The gallery wires it exactly like this: `Form::field(..)` tells the
        // form which inputs it owns, the inputs' `name` props ride on their
        // states, and `submit_handler()` is what a submit button calls
        // (gpui gives a child no way to reach its ancestor form).
        let form = Form::new()
            .field(FormField::text(name_state.clone()).is_required(true))
            .field(FormField::text(email_state.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        let submit = form.submit_handler();
        form.child(Input::new(name_state).name("name"))
            .child(Input::new(email_state).name("email"))
            .child(
                gpui::div().flex().gap(px(8.)).child(
                    Button::new("form-submit")
                        .label("Submit")
                        .on_press(move |_, window, cx| submit(window, cx)),
                ),
            )
            .into_any_element()
    });

    // Layout: two bare Inputs stacked `gap(16)` (field 1 y 0..36, field 2
    // y 52..88) and the md submit button below (y 104..140) — see the file
    // doc comment.
    click(cx, 60., 18.);
    cx.simulate_input("ada");
    click(cx, 60., 70.);
    cx.simulate_input("ada@example.com");
    click(cx, 60., 122.);

    assert_eq!(
        submitted.borrow().as_slice(),
        ["name=ada,email=ada@example.com"],
        "submitting must report every named field, in registration order"
    );
    assert!(
        invalid.borrow().is_empty(),
        "a filled required field must not block the submission"
    );
}

#[gpui::test]
fn form_validation_blocks_submit(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let missing = invalids.clone();
    let name_state = cx.new(|cx| InputState::new(cx));
    let email_state = cx.new(|cx| InputState::new(cx));
    let name_for_view = name_state;
    let email_for_view = email_state;
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let name_state = name_for_view.clone();
        let email_state = email_for_view.clone();
        let form = Form::new()
            .field(FormField::text(name_state.clone()).is_required(true))
            .field(FormField::text(email_state.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                // The submit handler's block check is exactly
                // `data.missing_required(&required_names)`, so re-deriving
                // the required list here records which names it reported.
                let names = data
                    .missing_required(&[SharedString::from("name")])
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                invalids.borrow_mut().push(format!("missing:{names}"));
            });
        let submit = form.submit_handler();
        form.child(Input::new(name_state).name("name"))
            .child(Input::new(email_state).name("email"))
            .child(
                gpui::div().flex().gap(px(8.)).child(
                    Button::new("form-submit")
                        .label("Submit")
                        .on_press(move |_, window, cx| submit(window, cx)),
                ),
            )
            .into_any_element()
    });

    // Fill only the non-required field: `name` stays empty. Same spots as
    // the submit test — field 2 at (60, 70), submit at (60, 122).
    click(cx, 60., 70.);
    cx.simulate_input("ada@example.com");
    click(cx, 60., 122.);

    assert_eq!(
        missing.borrow().as_slice(),
        ["missing:name"],
        "an empty required field must route the submission to on_invalid \
         with that name reported"
    );
    assert!(
        submitted.borrow().is_empty(),
        "onSubmit must not fire while a required field is empty"
    );
}

// ---------------------------------------------------------------------------
// Field chrome motion
//
// `.input-otp__slot` and `.number-field__group` declare the identical shell
// transition — `background-color 150ms var(--ease-smooth), border-color 150ms
// var(--ease-smooth), box-shadow 150ms var(--ease-out)`, with
// `motion-reduce:transition-none` after it — and both move focus, invalid and
// hover chrome through the shared `anim::field_chrome_ramp`. The headless
// platform has no rasterizer, so these tests observe motion the way
// checkbox_motion.rs does: every animation in flight registers a next-frame
// callback, so the count of pending frames is the count of live animations.
// They prove animation lifetime and nothing about per-frame pixels.
// ---------------------------------------------------------------------------

/// How many animation-frame callbacks are pending: one per live animation.
fn pending_frames(cx: &mut VisualTestContext) -> usize {
    cx.update(|window, cx| window.simulate_next_frame(cx))
}

/// Forces the frame that carries the state a flag just changed.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// Drains frames until every in-flight animation has settled, failing if one
/// never does (a re-arming animation would keep registering callbacks).
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..200 {
        if pending_frames(cx) == 0 {
            return;
        }
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(10));
    }
    panic!("field chrome animations never settled");
}

/// Even the longest chrome track (150ms) is still running this far in.
const MID_FLIGHT_MS: u64 = 60;

fn advance_mid_flight(cx: &mut VisualTestContext) {
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(MID_FLIGHT_MS));
}

#[gpui::test]
fn number_field_chrome_mounts_settled_and_interpolates_focus(cx: &mut TestAppContext) {
    let state = cx.new(|cx| NumberState::new(cx, 5.0));
    let state_for_view = state;
    // Placed below the window's default pointer position: a field under the
    // pointer at mount is legitimately hovered, and would ease its hover
    // chrome in — this test needs the untouched resting mount.
    let cx = open_host(cx, move || {
        gpui::div()
            .mt(px(200.))
            .child(NumberField::new(state_for_view.clone()))
            .into_any_element()
    });

    // A field that mounts unfocused shows its resting chrome immediately: no
    // animation may mount for it, at this frame or after a forced one.
    assert_eq!(
        pending_frames(cx),
        0,
        "an unfocused NumberField must mount settled"
    );
    flush_frame(cx);
    assert_eq!(pending_frames(cx), 0);

    // Focus (the field is the page's only tab stop): the ring eases in on the
    // pinned `box-shadow 150ms var(--ease-out)` track. The inner input's own
    // repeating caret blink also starts here, so nothing may be asserted to
    // settle until focus is gone.
    press(cx, "tab");
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "taking focus must animate the focus ring, not swap it in"
    );

    // And blur: the caret stops with focus, and the ring eases back out.
    cx.update(|window, cx| window.blur(cx));
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "losing focus must animate the ring out, not swap it off"
    );
    settle(cx);
    assert_eq!(pending_frames(cx), 0);
}

#[gpui::test]
fn number_field_focus_snaps_under_reduced_motion(cx: &mut TestAppContext) {
    still();
    let state = cx.new(|cx| NumberState::new(cx, 5.0));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        NumberField::new(state_for_view.clone()).into_any_element()
    });

    // `motion-reduce:transition-none` keeps the state's property values but
    // removes the timing: the ring lands on its endpoint with no animation.
    press(cx, "tab");
    flush_frame(cx);
    assert_eq!(
        pending_frames(cx),
        0,
        "reduced motion must snap the focus chrome"
    );
    flush_frame(cx);
    assert_eq!(pending_frames(cx), 0);
}

#[gpui::test]
fn number_field_invalid_chrome_interpolates(cx: &mut TestAppContext) {
    let invalid = Rc::new(std::cell::Cell::new(false));
    let seed = invalid.clone();
    let state = cx.new(|cx| NumberState::new(cx, 5.0));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let invalid = seed.clone();
        NumberField::new(state_for_view.clone())
            .is_invalid(invalid.get())
            .into_any_element()
    });
    settle(cx);

    // Flipping to invalid fades the danger outline in: the border track is
    // transparent -> danger at the outline's width, over the pinned 150ms.
    invalid.set(true);
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "becoming invalid must animate the danger outline in"
    );
    settle(cx);

    // And back: the same track fades the outline out again.
    invalid.set(false);
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "recovering from invalid must animate the danger outline out"
    );
    settle(cx);
    assert_eq!(pending_frames(cx), 0);
}

#[gpui::test]
fn number_field_hover_interpolates_the_group_surface(cx: &mut TestAppContext) {
    let state = cx.new(|cx| NumberState::new(cx, 5.0));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        NumberField::new(state_for_view.clone()).into_any_element()
    });
    settle(cx);

    // The group is a 220x36 box at the window origin; a pointer over its
    // middle hovers it, and the fill and border ease to the hover tokens on
    // their shared 150ms smooth track.
    cx.simulate_mouse_move(point(px(110.), px(18.)), None, Modifiers::none());
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "hovering the group must ease the hover surface in"
    );
    settle(cx);

    // Leaving eases the same tracks back to the resting endpoints.
    cx.simulate_mouse_move(point(px(-100.), px(-100.)), None, Modifiers::none());
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "leaving the group must ease the hover surface out"
    );
    settle(cx);
    assert_eq!(pending_frames(cx), 0);
}

/// `field_chrome_ramp(.., ring_escapes_clip = true)` cannot paint the ring
/// as a shadow: gpui fills a spread shadow, so the background-coloured gap
/// ring covered the whole group interior once the layer landed after the
/// subtree — and no child, deferred or not, escapes the group's
/// `overflow-hidden` clip. The ramp now hands the caller a hollow overlay —
/// its `ring-offset` band is a quad `ring_gaps` can find (the shadow painted
/// none, so this fails on the old layer) — which NumberField hangs on the
/// non-clipping `field_ring_carrier`, and no background fill may sit inside
/// the group.
#[gpui::test]
fn number_field_deferred_focus_ring_is_hollow(cx: &mut TestAppContext) {
    let state = cx.new(|cx| NumberState::new(cx, 5.0));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        gpui::div()
            // The 2px ring offset hangs the band outside the group's box;
            // keep it inside the window so `band_is_clipped` reads ancestor
            // clips, not the viewport edge.
            .ml(px(8.))
            .mt(px(200.))
            .child(NumberField::new(state_for_view.clone()))
            .into_any_element()
    });
    settle(cx);

    // Focus -> blur -> focus: the ramp is past generation 0 on the second
    // focus, so the deferred ring layer is the one painting. `settle` only
    // runs while blurred — the focused input's caret blink never ends.
    press(cx, "tab");
    flush_frame(cx);
    advance_mid_flight(cx);
    cx.update(|window, cx| window.blur(cx));
    flush_frame(cx);
    settle(cx);
    press(cx, "tab");
    flush_frame(cx);
    for _ in 0..30 {
        pending_frames(cx);
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(16));
    }
    flush_frame(cx);

    let scene = harness::painted(cx);
    let (bg, gap) = cx.update(|_, cx| {
        use herogpui_theme::ActiveTheme;
        (
            cx.colors().background,
            f32::from(cx.layout().ring_offset_width),
        )
    });
    let rings = scene.ring_gaps(bg, gap);
    assert_eq!(
        rings.len(),
        1,
        "the focused NumberField must paint one offset-overlay ring band\n{}",
        scene.describe()
    );
    assert!(
        !scene.band_is_clipped(rings[0]),
        "the carrier-mounted overlay escapes the group's overflow clip\n{}",
        scene.describe()
    );

    // Hollow: the band inflates the group bounds by `gap`; nothing filled
    // with the page background may sit inside that interior — the old gap
    // shadow filled exactly this area after the content had painted.
    let band = scene.bounds(rings[0]);
    let interior = gpui::Bounds {
        origin: point(
            px(f32::from(band.origin.x) + gap),
            px(f32::from(band.origin.y) + gap),
        ),
        size: gpui::size(
            px(f32::from(band.size.width) - 2.0 * gap),
            px(f32::from(band.size.height) - 2.0 * gap),
        ),
    };
    for quad in scene.filled(bg) {
        assert!(
            !harness::contains(interior, scene.bounds(quad)),
            "a background fill covers the NumberField interior\n{}",
            scene.describe()
        );
    }
}

/// v3 rounds only a stepper cell's outer corners (`rounded-s-field` /
/// `rounded-e-field` on the group): the press fill carries the group's inner
/// radius on those corners and stays square on the seam side. Horizontal:
/// decrement owns the left corners, increment the right.
#[gpui::test]
fn number_field_stepper_press_fill_rounds_only_the_outer_corners(cx: &mut TestAppContext) {
    let state = cx.new(|cx| NumberState::new(cx, 5.0));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        NumberField::new(state_for_view.clone()).into_any_element()
    });
    settle(cx);
    let pressed_bg = cx.update(|_, cx| {
        use herogpui_theme::ActiveTheme;
        cx.colors().field.foreground.alpha(0.1)
    });

    let corners_at = |cx: &mut VisualTestContext, x: f32, y: f32| {
        cx.simulate_mouse_down(
            point(px(x), px(y)),
            gpui::MouseButton::Left,
            Modifiers::none(),
        );
        flush_frame(cx);
        advance_mid_flight(cx);
        let scene = harness::painted(cx);
        cx.simulate_mouse_up(
            point(px(x), px(y)),
            gpui::MouseButton::Left,
            Modifiers::none(),
        );
        let quad = scene
            .filled(pressed_bg)
            .into_iter()
            .find(|q| {
                let b = scene.bounds(q);
                f32::from(b.origin.x) >= 0.0
                    && f32::from(b.origin.x) + f32::from(b.size.width) <= 220.0
                    && f32::from(b.origin.y) >= 0.0
                    && f32::from(b.origin.y) + f32::from(b.size.height) <= 36.0
            })
            .unwrap_or_else(|| panic!("no pressed stepper fill painted\n{}", scene.describe()));
        scene.corners(quad)
    };

    // Decrement (20, 18): top-left + bottom-left rounded, seam side square.
    let decrement = corners_at(cx, 20., 18.);
    assert!(
        decrement[0] > 0.5 && decrement[3] > 0.5,
        "the decrement press fill must round the group's left corners: {decrement:?}"
    );
    assert!(
        decrement[1] < 0.05 && decrement[2] < 0.05,
        "the decrement press fill must stay square on the seam side: {decrement:?}"
    );

    // Increment (200, 18): top-right + bottom-right rounded.
    let increment = corners_at(cx, 200., 18.);
    assert!(
        increment[1] > 0.5 && increment[2] > 0.5,
        "the increment press fill must round the group's right corners: {increment:?}"
    );
    assert!(
        increment[0] < 0.05 && increment[3] < 0.05,
        "the increment press fill must stay square on the seam side: {increment:?}"
    );
}

/// The vertical stepper column (x 196..220) splits into an 18px increment
/// half and an 18px decrement half; v3 rounds only the cell's outer corner —
/// top-right for increment, bottom-right for decrement.
#[gpui::test]
fn number_field_vertical_stepper_press_fill_rounds_the_outer_corner(cx: &mut TestAppContext) {
    let state = cx.new(|cx| NumberState::new(cx, 5.0));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        NumberField::new(state_for_view.clone())
            .vertical_steppers(true)
            .increment_icon(gpui::div())
            .decrement_icon(gpui::div())
            .into_any_element()
    });
    settle(cx);
    let pressed_bg = cx.update(|_, cx| {
        use herogpui_theme::ActiveTheme;
        cx.colors().field.foreground.alpha(0.1)
    });

    let corners_at = |cx: &mut VisualTestContext, x: f32, y: f32| {
        cx.simulate_mouse_down(
            point(px(x), px(y)),
            gpui::MouseButton::Left,
            Modifiers::none(),
        );
        flush_frame(cx);
        advance_mid_flight(cx);
        let scene = harness::painted(cx);
        cx.simulate_mouse_up(
            point(px(x), px(y)),
            gpui::MouseButton::Left,
            Modifiers::none(),
        );
        let quad = scene
            .filled(pressed_bg)
            .into_iter()
            .find(|q| f32::from(scene.bounds(q).origin.x) >= 190.0)
            .unwrap_or_else(|| panic!("no pressed stepper fill painted\n{}", scene.describe()));
        scene.corners(quad)
    };

    // Increment (208, 9): only the top-right corner is rounded.
    let increment = corners_at(cx, 208., 9.);
    assert!(
        increment[1] > 0.5,
        "the vertical increment press fill must round the top-right corner: {increment:?}"
    );
    assert!(
        increment[0] < 0.05 && increment[2] < 0.05 && increment[3] < 0.05,
        "the vertical increment press fill must stay square elsewhere: {increment:?}"
    );

    // Decrement (208, 27): only the bottom-right corner is rounded.
    let decrement = corners_at(cx, 208., 27.);
    assert!(
        decrement[2] > 0.5,
        "the vertical decrement press fill must round the bottom-right corner: {decrement:?}"
    );
    assert!(
        decrement[0] < 0.05 && decrement[1] < 0.05 && decrement[3] < 0.05,
        "the vertical decrement press fill must stay square elsewhere: {decrement:?}"
    );
}

#[gpui::test]
fn input_otp_invalid_slots_interpolate_the_danger_outline(cx: &mut TestAppContext) {
    let invalid = Rc::new(std::cell::Cell::new(false));
    let seed = invalid.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let invalid = seed.clone();
        InputOTP::new(state_for_view.clone())
            .is_invalid(invalid.get())
            .into_any_element()
    });
    settle(cx);

    // Every slot carries the danger outline, fading in on the shared
    // border-color track.
    invalid.set(true);
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "becoming invalid must animate the slots' danger outlines in"
    );
    settle(cx);

    invalid.set(false);
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "recovering from invalid must animate the slots' danger outlines out"
    );
    settle(cx);
    assert_eq!(pending_frames(cx), 0);
}

#[gpui::test]
fn input_otp_hover_interpolates_the_slot_surface(cx: &mut TestAppContext) {
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        InputOTP::new(state_for_view.clone()).into_any_element()
    });
    settle(cx);

    // Cell 0 spans x 8..46 (the zero-width origin item, then the 38px cell),
    // y 0..40. Hovering it eases the slot fill and border to the hover
    // endpoints; an unfocused row mounts no caret, so the pending frames are
    // the chrome tracks alone.
    cx.simulate_mouse_move(point(px(27.), px(20.)), None, Modifiers::none());
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "hovering a slot must ease its hover chrome in"
    );
    settle(cx);

    cx.simulate_mouse_move(point(px(-100.), px(-100.)), None, Modifiers::none());
    flush_frame(cx);
    advance_mid_flight(cx);
    assert!(
        pending_frames(cx) > 0,
        "leaving a slot must ease its hover chrome out"
    );
    settle(cx);
    assert_eq!(pending_frames(cx), 0);
}
