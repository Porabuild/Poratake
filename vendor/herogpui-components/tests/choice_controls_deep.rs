//! Deeper keyboard and read-only behaviour for Switch, RadioGroup and
//! ToggleButtonGroup.
//!
//! The prop surface comes from HeroUI v3's API tables. Inherited behaviour is
//! pinned to the versions HeroUI v3.2.4 uses: react-aria 3.51.0,
//! react-stately 3.49.0 and React Aria Components 1.20.0. In those sources:
//!
//! - `useToggleState` ignores updates while read-only, but `useToggle` keeps
//!   the input focusable because only `isDisabled` reaches `disabled`.
//! - `useRadioGroup` moves DOM focus with the four arrow keys and calls
//!   `setSelectedValue`; `useRadioGroupState` rejects that selection update
//!   while read-only. Its shortcut table has no Home or End entry.
//! - `useToggleGroupState` defaults `selectionMode` to `single` and enforces
//!   `disallowEmptySelection`. `useToggleButtonGroup` delegates focus to
//!   `useToolbar`, whose pinned implementation handles only the orientation's
//!   own arrow axis and moves Tab to the group's edge so normal tabbing leaves
//!   the group.
//!
//! These tests drive those paths through gpui's real headless window. Static
//! audits cannot prove that a read-only control keeps focus, that an arrow
//! moved it, or that Tab escaped a composite widget.

use crate::harness;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{prelude::*, px, SharedString, TestAppContext, VisualTestContext};
use herogpui_components::{
    Button, Form, Orientation, RadioGroup, RadioOption, SelectionMode, Size, Switch, ToggleButton,
    ToggleButtonGroup, Toolbar, ValidationBehavior,
};

use harness::{click, events, open_host, press};

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn joined(keys: &[SharedString]) -> String {
    keys.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn three_toggles(prefix: &str) -> [ToggleButton; 3] {
    [
        ToggleButton::new(gpui::ElementId::Name(format!("{prefix}-bold").into()))
            .key("bold")
            .label("Bold"),
        ToggleButton::new(gpui::ElementId::Name(format!("{prefix}-italic").into()))
            .key("italic")
            .label("Italic"),
        ToggleButton::new(gpui::ElementId::Name(format!("{prefix}-underline").into()))
            .key("underline")
            .label("Underline"),
    ]
}

// ---------------------------------------------------------------------------
// Switch
// ---------------------------------------------------------------------------

#[gpui::test]
fn switch_read_only_ignores_pointer_press(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Switch::new("readonly-pointer")
            .is_read_only(true)
            .on_change(move |selected, _, _| changes.borrow_mut().push(selected.to_string()))
            .into_any_element()
    });

    // The default md track is 40x20, so its centre is (20, 10).
    click(cx, 20., 10.);
    assert_eq!(
        recorded.borrow().as_slice(),
        [] as [&str; 0],
        "a read-only switch must ignore a pointer press"
    );
}

#[gpui::test]
fn switch_label_row_toggles_once(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Switch::new("label-pointer")
            .label(gpui::div().w(px(64.)).child("Notifications"))
            .on_change(move |selected, _, _| changes.borrow_mut().push(selected.to_string()))
            .into_any_element()
    });

    // The label begins after the 40px track and 12px content gap. Its row is
    // the v3 Switch.Content click target, so this must toggle exactly once.
    click(cx, 72., 10.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["true"],
        "clicking the switch label must toggle the row once"
    );
}

/// The built-in label is the shared `Label` part, `.label` = `text-sm
/// font-medium`: a caller label inherits a 20px line box, not the 24px of the
/// never-applied `.switch__label` `text-base` rule v3.2.6 deleted.
#[gpui::test]
fn switch_label_inherits_the_shared_label_line_box(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        Switch::new("label-line-box")
            .label(
                gpui::div()
                    .debug_selector(|| "switch-label-probe".to_owned())
                    .child("Notifications"),
            )
            .into_any_element()
    });
    let label = cx
        .debug_bounds("switch-label-probe")
        .expect("the switch label must be laid out");
    assert!(
        (f32::from(label.size.height) - 20.).abs() < 0.5,
        "the switch label must use `.label`'s 14px/20px line box: {:?}",
        label.size
    );
}

#[gpui::test]
fn switch_read_only_stays_focusable_but_ignores_space(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let focused = Rc::new(RefCell::new(false));
    let focus_seen = focused.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let focused = focused.clone();
        Switch::new("readonly-keyboard")
            .is_read_only(true)
            .content(move |state| {
                *focused.borrow_mut() = state.is_focused;
                gpui::div().child("Read only").into_any_element()
            })
            .on_change(move |selected, _, _| changes.borrow_mut().push(selected.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    flush_frame(cx);
    assert!(
        *focus_seen.borrow(),
        "read-only is not disabled: Tab must still focus the switch"
    );

    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        [] as [&str; 0],
        "Space on a focused read-only switch must not toggle it"
    );
}

#[gpui::test]
fn switch_content_receives_complete_field_state(cx: &mut TestAppContext) {
    let states = events();
    let recorded = states.clone();
    let _cx = open_host(cx, move || {
        let states = states.clone();
        Switch::new("switch-complete-state")
            .is_selected(true)
            .is_read_only(true)
            .is_required(true)
            // `Allow` keeps `validate` live; under the default Native commit
            // gating a read-only switch has no commit trigger to reveal it.
            .validate(|_| Some("Invalid selection".into()))
            .validation_behavior(ValidationBehavior::Allow)
            .content(move |state| {
                states.borrow_mut().push(format!(
                    "selected={}:readonly={}:invalid={}:required={}",
                    state.is_selected, state.is_read_only, state.is_invalid, state.is_required
                ));
                gpui::div().into_any_element()
            })
            .into_any_element()
    });

    assert!(recorded
        .borrow()
        .iter()
        .any(|state| { state == "selected=true:readonly=true:invalid=true:required=true" }));
}

#[gpui::test]
fn switch_field_error_is_outside_the_clickable_content(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Switch::new("switch-field-error")
            .validation_errors(["Selection is unavailable"])
            .on_change(move |selected, _, _| changes.borrow_mut().push(selected.to_string()))
            .into_any_element()
    });

    // The 16px FieldError line starts after the 20px track plus the 4px
    // field gap. It is a sibling of Switch.Content and must not toggle it.
    click(cx, 60., 31.);
    assert!(recorded.borrow().is_empty());

    click(cx, 20., 10.);
    assert_eq!(recorded.borrow().as_slice(), ["true"]);
}

#[gpui::test]
fn switch_form_reads_live_uncontrolled_value_and_reset(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let changes = events.clone();
        let submits = events.clone();
        let switch = Switch::new("live-switch-form")
            .name("notifications")
            .value("enabled")
            .on_change(move |selected, _, _| {
                changes.borrow_mut().push(format!("change:{selected}"));
            });
        let form = Form::new()
            .field(switch.form_field().expect("named switch field"))
            .on_submit(move |data, _, _| {
                submits.borrow_mut().push(format!(
                    "submit:{}",
                    data.text("notifications")
                        .map_or_else(|| "omitted".to_owned(), |value| value.to_string())
                ));
            });
        let submit = form.submit_handler();
        let reset = form.reset_handler();
        form.child(switch)
            .child(
                Button::new("live-switch-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .child(
                Button::new("live-switch-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    press(cx, "tab");
    press(cx, "space");
    press(cx, "tab");
    press(cx, "space");
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "space");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:true", "submit:enabled", "submit:omitted"],
        "submission must read the current uncontrolled state and reset must restore defaultSelected"
    );
}

#[gpui::test]
fn controlled_switch_reset_reports_default_selected(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let changes = events.clone();
        let switch = Switch::new("controlled-switch-reset")
            .name("notifications")
            .is_selected(true)
            .default_selected(false)
            .on_change(move |selected, _, _| changes.borrow_mut().push(selected.to_string()));
        let form = Form::new().field(switch.form_field().expect("named switch field"));
        let reset = form.reset_handler();
        form.child(switch)
            .child(
                Button::new("controlled-switch-reset-button")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["false"]);
}

#[gpui::test]
fn controlled_switch_form_waits_for_owner_to_accept_change(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let changes = events.clone();
        let submits = events.clone();
        let switch = Switch::new("controlled-switch-form")
            .name("notifications")
            .value("enabled")
            .is_selected(false)
            .on_change(move |selected, _, _| {
                changes.borrow_mut().push(format!("change:{selected}"));
            });
        let form = Form::new()
            .field(switch.form_field().expect("named switch field"))
            .on_submit(move |data, _, _| {
                submits.borrow_mut().push(format!(
                    "submit:{}",
                    data.text("notifications")
                        .map_or_else(|| "omitted".to_owned(), |value| value.to_string())
                ));
            });
        let submit = form.submit_handler();
        form.child(switch)
            .child(
                Button::new("controlled-switch-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:true", "submit:omitted"],
        "a controlled press only reports intent; form data changes after the owner renders it"
    );
}

#[gpui::test]
fn disabled_switch_is_not_a_successful_form_control(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let switch = Switch::new("disabled-switch-snapshot")
            .name("notifications")
            .value("enabled")
            .is_selected(true)
            .is_disabled(true);
        let form = Form::new().field(switch.form_field().expect("named switch field"));
        assert!(
            form.data(cx).get("notifications").is_none(),
            "disabled omission must hold before first render"
        );
    });

    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let submits = events.clone();
        let switch = Switch::new("disabled-switch-form")
            .name("notifications")
            .value("enabled")
            .is_selected(true)
            .is_disabled(true);
        let form = Form::new()
            .field(switch.form_field().expect("named switch field"))
            .on_submit(move |data, _, _| {
                submits.borrow_mut().push(
                    data.text("notifications")
                        .map_or_else(|| "omitted".to_owned(), |value| value.to_string()),
                );
            });
        let submit = form.submit_handler();
        form.child(switch)
            .child(
                Button::new("disabled-switch-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["omitted"]);
}

#[gpui::test]
fn invalid_switch_blocks_form_and_receives_focus(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let changes = events.clone();
        let invalids = events.clone();
        let submits = events.clone();
        let switch = Switch::new("invalid-switch-form")
            .name("notifications")
            .validate(|selected| (!selected).then(|| "Enable notifications".into()))
            .on_change(move |selected, _, _| {
                changes.borrow_mut().push(format!("change:{selected}"));
            });
        let form = Form::new()
            .field(switch.form_field().expect("named switch field"))
            .on_invalid(move |_, _, _| invalids.borrow_mut().push("invalid".to_owned()))
            .on_submit(move |_, _, _| submits.borrow_mut().push("submit".to_owned()));
        let submit = form.submit_handler();
        form.child(switch)
            .child(
                Button::new("invalid-switch-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "space");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["invalid", "change:true"]);
}

// ---------------------------------------------------------------------------
// RadioGroup
// ---------------------------------------------------------------------------

#[gpui::test]
fn radio_indicator_receives_field_state_and_reacts_to_selection(cx: &mut TestAppContext) {
    let states = events();
    let recorded = states.clone();
    let cx = open_host(cx, move || {
        let states = states.clone();
        RadioGroup::new(
            "custom-radio-indicator",
            vec![
                RadioOption::new("Free plan").value("free"),
                RadioOption::new("Pro plan").value("pro"),
            ],
        )
        .default_value("free")
        .is_required(true)
        .error_message("Choose a plan")
        .indicator(move |label, state| {
            states.borrow_mut().push(format!(
                "{label}:selected={}:invalid={}:required={}",
                state.is_selected, state.is_invalid, state.is_required
            ));
            gpui::div().into_any_element()
        })
        .into_any_element()
    });

    assert!(recorded
        .borrow()
        .iter()
        .any(|state| { state == "Free plan:selected=true:invalid=true:required=true" }));
    assert!(recorded
        .borrow()
        .iter()
        .any(|state| { state == "Pro plan:selected=false:invalid=true:required=true" }));

    click(cx, 48., 46.);
    flush_frame(cx);
    assert!(recorded
        .borrow()
        .iter()
        .any(|state| { state == "Pro plan:selected=true:invalid=true:required=true" }));
}

#[gpui::test]
fn per_radio_field_error_is_outside_the_clickable_content(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        RadioGroup::new(
            "radio-option-error",
            vec![
                RadioOption::new("Free plan")
                    .value("free")
                    .error_message("Unavailable in this region"),
                RadioOption::new("Pro plan").value("pro"),
            ],
        )
        .on_change(move |value, _, _| changes.borrow_mut().push(value.to_string()))
        .into_any_element()
    });

    // The error is a sibling below Radio.Content and must not inherit its
    // click listener.
    click(cx, 80., 31.);
    assert!(recorded.borrow().is_empty());

    // Its 16px line plus the 4px radio gap moves the second option down.
    click(cx, 48., 60.);
    assert_eq!(recorded.borrow().as_slice(), ["pro"]);
}

#[gpui::test]
fn invalid_radio_group_blocks_form_and_focuses_its_roving_stop(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let changes = events.clone();
        let invalids = events.clone();
        let submits = events.clone();
        let group = RadioGroup::new(
            "invalid-radio-form",
            vec![
                RadioOption::new("Free plan").value("free"),
                RadioOption::new("Pro plan").value("pro"),
            ],
        )
        .name("plan")
        .error_message("Choose a plan")
        .on_change(move |value, _, _| changes.borrow_mut().push(format!("change:{value}")));
        let form = Form::new()
            .field(group.form_field().expect("named radio field"))
            .on_invalid(move |_, _, _| invalids.borrow_mut().push("invalid".to_owned()))
            .on_submit(move |_, _, _| submits.borrow_mut().push("submit".to_owned()));
        let submit = form.submit_handler();
        form.child(group)
            .child(
                Button::new("invalid-radio-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "space");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["invalid", "change:free"],
        "native invalid submission must focus the group's current radio rather than submit"
    );
}

#[gpui::test]
fn disabled_radio_group_is_not_a_successful_form_control(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let group = RadioGroup::new(
            "disabled-radio-snapshot",
            vec![RadioOption::new("Free plan").value("free")],
        )
        .name("plan")
        .default_value("free")
        .is_disabled(true);
        let form = Form::new().field(group.form_field().expect("named radio field"));
        assert!(
            form.data(cx).get("plan").is_none(),
            "disabled omission must be true before the first render as well as after it"
        );
    });

    let submits = events();
    let recorded = submits.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let group = RadioGroup::new(
            "disabled-radio-form",
            vec![RadioOption::new("Free plan").value("free")],
        )
        .name("plan")
        .default_value("free")
        .is_disabled(true);
        let form = Form::new()
            .field(group.form_field().expect("named radio field"))
            .on_submit(move |data, _, _| {
                submits.borrow_mut().push(
                    data.text("plan")
                        .map_or_else(|| "omitted".to_owned(), |value| value.to_string()),
                );
            });
        let submit = form.submit_handler();
        form.child(group)
            .child(
                Button::new("disabled-radio-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["omitted"]);
}

#[gpui::test]
fn radio_group_reset_restores_the_uncontrolled_default(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let changes = events.clone();
        let submits = events.clone();
        let group = RadioGroup::new(
            "reset-radio-form",
            vec![
                RadioOption::new("Free plan").value("free"),
                RadioOption::new("Pro plan").value("pro"),
            ],
        )
        .name("plan")
        .default_value("free")
        .on_change(move |value, _, _| changes.borrow_mut().push(format!("change:{value}")));
        let form = Form::new()
            .field(group.form_field().expect("named radio field"))
            .on_submit(move |data, _, _| {
                submits.borrow_mut().push(format!(
                    "submit:{}",
                    data.text("plan").expect("selected radio value")
                ));
            });
        let reset = form.reset_handler();
        let submit = form.submit_handler();
        form.child(group)
            .child(
                Button::new("reset-radio-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .child(
                Button::new("reset-radio-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "tab");
    press(cx, "space");
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["change:pro", "submit:free"]);
}

#[gpui::test]
fn controlled_radio_reset_reports_the_default_to_its_owner(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(SharedString::from("pro")));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let group = RadioGroup::new(
            "controlled-reset-radio",
            vec![
                RadioOption::new("Free plan").value("free"),
                RadioOption::new("Pro plan").value("pro"),
            ],
        )
        .name("plan")
        .value(selected.borrow().as_ref())
        .default_value("free")
        .on_change({
            let selected = selected.clone();
            let changes = changes.clone();
            move |value, _, _| {
                *selected.borrow_mut() = value.clone();
                changes.borrow_mut().push(format!("change:{value}"));
            }
        });
        let form = Form::new().field(group.form_field().expect("named radio field"));
        let reset = form.reset_handler();
        form.child(group)
            .child(
                Button::new("controlled-reset-radio-button")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["change:free"]);
}

#[gpui::test]
fn radio_group_read_only_arrows_move_focus_without_selecting(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let read_only = Rc::new(RefCell::new(true));
    let read_only_for_view = read_only.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        RadioGroup::new(
            "readonly-radios",
            vec!["One".into(), "Two".into(), "Three".into()],
        )
        .default_value("One")
        .is_read_only(*read_only_for_view.borrow())
        .on_change(move |index, _, _| changes.borrow_mut().push(index.to_string()))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    flush_frame(cx);

    assert_eq!(
        recorded.borrow().as_slice(),
        [] as [&str; 0],
        "moving focus in a read-only group must not report a selection"
    );
    *read_only.borrow_mut() = false;
    flush_frame(cx);
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Two"],
        "after read-only ends, Space must activate the row Down focused"
    );
}

#[gpui::test]
fn radio_group_home_and_end_are_unconsumed_no_ops(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let bubbled = events();
    let outer_seen = bubbled.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let bubbled = bubbled.clone();
        gpui::div()
            .on_key_down(move |event, _, _| {
                if matches!(event.keystroke.key.as_str(), "home" | "end") {
                    bubbled.borrow_mut().push(event.keystroke.key.clone());
                }
            })
            .child(
                RadioGroup::new(
                    "radio-home-end",
                    vec!["One".into(), "Two".into(), "Three".into()],
                )
                .default_value("Two")
                .on_change(move |index, _, _| changes.borrow_mut().push(index.to_string())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "home");
    press(cx, "end");

    assert_eq!(
        outer_seen.borrow().as_slice(),
        ["home", "end"],
        "Home and End must bubble because the pinned radio hook does not consume them"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        [] as [&str; 0],
        "Home and End must not change a RadioGroup selection"
    );
}

/// Arrow navigation over a large option list with a disabled entry in the
/// middle. Shared walk/value-list ownership is proven in `radio_group.rs`;
/// this test pins that the walk still skips the disabled option in both
/// directions and reports the values it lands on.
#[gpui::test]
fn radio_group_arrows_walk_many_options_and_skip_disabled(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let options: Vec<RadioOption> = (0..40)
            .map(|i| {
                let option = RadioOption::new(format!("Option {i}")).value(format!("v{i}"));
                if i == 20 {
                    option.is_disabled(true)
                } else {
                    option
                }
            })
            .collect();
        RadioGroup::new("many-radios", options)
            .default_value("v0")
            .on_change(move |value, _, _| changes.borrow_mut().push(value.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    // Walk down from the selected stop at index 0 to index 19.
    for _ in 0..19 {
        press(cx, "down");
    }
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("v19"),
        "nineteen downs from index 0 must land on index 19"
    );

    // The step that would land on the disabled index 20 must skip it.
    press(cx, "down");
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("v21"),
        "a disabled option must not become the roving stop"
    );
    assert!(
        !recorded.borrow().iter().any(|value| value == "v20"),
        "a disabled option must never report a selection"
    );

    // And back up across the same gap.
    press(cx, "up");
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("v19"),
        "up must skip the disabled option in the other direction"
    );
}

#[gpui::test]
fn radio_group_read_only_pointer_moves_focus_without_selection(cx: &mut TestAppContext) {
    let read_only = Rc::new(RefCell::new(true));
    let read_only_for_view = read_only.clone();
    let selected = Rc::new(RefCell::new(String::new()));
    let selected_seen = selected.clone();
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let selected = selected.clone();
        let changes = changes.clone();
        RadioGroup::new(
            "readonly-pointer-radios",
            vec!["One".into(), "Two".into(), "Three".into()],
        )
        .value("One")
        .is_read_only(*read_only_for_view.borrow())
        .option_content(move |label, state| {
            if state.is_selected {
                *selected.borrow_mut() = label.to_string();
            }
            gpui::div().child(label.to_string()).into_any_element()
        })
        .on_change(move |value, _, _| changes.borrow_mut().push(value.to_string()))
        .into_any_element()
    });

    click(cx, 48., 46.);
    flush_frame(cx);
    assert_eq!(
        selected_seen.borrow().as_str(),
        "One",
        "read-only pointer focus must not change the controlled selection"
    );
    assert!(recorded.borrow().is_empty());
    *read_only.borrow_mut() = false;
    flush_frame(cx);
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Two"],
        "after read-only ends, Space must activate the row the pointer focused"
    );
}

#[gpui::test]
fn radio_option_value_drives_callback_and_form_not_visible_label(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let group = RadioGroup::new(
            "radio-valued-form",
            vec![
                RadioOption::new("Free plan").value("free"),
                RadioOption::new("Pro plan").value("pro"),
            ],
        )
        .name("plan")
        .default_value("pro");
        let form = Form::new().field(group.form_field().expect("named radio field"));
        assert_eq!(form.data(cx).text("plan"), Some(SharedString::from("pro")));

        for group in [
            RadioGroup::new(
                "radio-controlled-before-default",
                vec![
                    RadioOption::new("Free plan").value("free"),
                    RadioOption::new("Pro plan").value("pro"),
                ],
            )
            .name("plan")
            .value("free")
            .default_value("pro"),
            RadioGroup::new(
                "radio-default-before-controlled",
                vec![
                    RadioOption::new("Free plan").value("free"),
                    RadioOption::new("Pro plan").value("pro"),
                ],
            )
            .name("plan")
            .default_value("pro")
            .value("free"),
        ] {
            let form = Form::new().field(group.form_field().expect("named radio field"));
            assert_eq!(
                form.data(cx).text("plan"),
                Some(SharedString::from("free")),
                "controlled value must win over the default before first render"
            );
        }
    });

    let labels = events();
    let labels_seen = labels.clone();
    let changes = events();
    let changed = changes.clone();
    let cx = open_host(cx, move || {
        let labels = labels.clone();
        let changes = changes.clone();
        RadioGroup::new(
            "radio-valued-callback",
            vec![
                RadioOption::new("Free plan").value("free"),
                RadioOption::new("Pro plan").value("pro"),
            ],
        )
        .option_content(move |label, _| {
            labels.borrow_mut().push(label.to_string());
            gpui::div().child(label.to_string()).into_any_element()
        })
        .on_change(move |value, _, _| changes.borrow_mut().push(value.to_string()))
        .into_any_element()
    });

    assert!(labels_seen.borrow().iter().any(|label| label == "Pro plan"));
    click(cx, 48., 46.);
    assert_eq!(changed.borrow().as_slice(), ["pro"]);
}

#[gpui::test]
fn radio_option_disabled_is_skipped_by_arrows_and_pointer(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        RadioGroup::new(
            "radio-option-disabled",
            vec![
                RadioOption::new("One").value("one"),
                RadioOption::new("Two").value("two").is_disabled(true),
                RadioOption::new("Three").value("three"),
            ],
        )
        .default_value("one")
        .on_change(move |value, _, _| changes.borrow_mut().push(value.to_string()))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    click(cx, 8., 46.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["three"],
        "RadioOption::is_disabled must remove the option from arrows and pointer selection"
    );
}

#[gpui::test]
fn radio_group_empty_options_render_and_leave_tab_order(cx: &mut TestAppContext) {
    let probes = events();
    let recorded = probes.clone();
    let cx = open_host(cx, move || {
        let probes = probes.clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(RadioGroup::new("empty-radio-group", vec![]))
            .child(Switch::new("empty-radio-probe").on_change(move |_, _, _| {
                probes.borrow_mut().push("probe".into());
            }))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["probe"]);
}

#[gpui::test]
fn radio_group_form_field_reads_changed_uncontrolled_value(cx: &mut TestAppContext) {
    let submissions = events();
    let recorded = submissions.clone();
    let cx = open_host(cx, move || {
        let submissions = submissions.clone();
        let group = RadioGroup::new(
            "live-radio-form",
            vec![
                RadioOption::new("Free plan").value("free"),
                RadioOption::new("Pro plan").value("pro"),
            ],
        )
        .name("plan")
        .default_value("free");
        let form = Form::new()
            .field(group.form_field().expect("named radio field"))
            .on_submit(move |data, _, _| {
                submissions.borrow_mut().push(
                    data.text("plan")
                        .expect("submitted radio value")
                        .to_string(),
                );
            });
        let submit = form.submit_handler();
        gpui::div()
            .flex()
            .flex_col()
            .child(group)
            .child(Button::new("live-radio-submit").label("Submit").on_press(
                move |_, window, cx| {
                    submit(window, cx);
                },
            ))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["pro"],
        "FormField must read the keyed uncontrolled selection after it changes"
    );
}

// ---------------------------------------------------------------------------
// ToggleButtonGroup
// ---------------------------------------------------------------------------

#[gpui::test]
fn toggle_button_group_defaults_to_single_selection(cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let state = state.clone();
        let changes = changes.clone();
        let current = state.borrow().clone();
        let [bold, italic, underline] = three_toggles("default-single");
        ToggleButtonGroup::new("default-single-group")
            .full_width(true)
            .selected_keys(current)
            .on_selection_change(move |next, _, _| {
                *state.borrow_mut() = next.to_vec();
                changes.borrow_mut().push(joined(next));
            })
            .child_toggle(bold)
            .child_toggle(italic)
            .child_toggle(underline)
            .into_any_element()
    });

    // A full-width three-member group divides the 1920px test window into
    // 640px slots. Their centres are x=320 and x=960; md height centres y=18.
    click(cx, 320., 18.);
    click(cx, 960., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bold", "italic"],
        "the default Single mode must replace Bold with Italic"
    );
}

#[gpui::test]
fn toggle_button_group_numeric_and_string_ids_keep_their_own_keys(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        ToggleButtonGroup::new("numeric-id-group")
            .full_width(true)
            .default_selected_keys(["7"])
            .on_selection_change(move |next, _, _| changes.borrow_mut().push(joined(next)))
            .child_toggle(ToggleButton::new(7).label("Seven"))
            .child_toggle(ToggleButton::new("bold").label("Bold"))
            .into_any_element()
    });

    // A full-width two-member group divides the 1920px test window into 960px
    // slots; md height centres them at y=18.
    click(cx, 480., 18.);
    click(cx, 1440., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["", "bold"],
        "the numeric member must answer its public-spelling seed and string ids must stay intact"
    );
}

#[gpui::test]
fn toggle_button_group_default_selected_keys_holds_uncontrolled_state(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let [bold, italic, underline] = three_toggles("default-selected");
        ToggleButtonGroup::new("default-selected-group")
            .full_width(true)
            .default_selected_keys(["bold"])
            .on_selection_change(move |next, _, _| changes.borrow_mut().push(joined(next)))
            .child_toggle(bold)
            .child_toggle(italic)
            .child_toggle(underline)
            .into_any_element()
    });

    click(cx, 320., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        [""],
        "pressing the initially selected Bold button must clear the uncontrolled seed"
    );
}

#[gpui::test]
fn toggle_button_group_controlled_empty_overrides_default_seed(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let [bold, italic, underline] = three_toggles("controlled-empty");
        ToggleButtonGroup::new("controlled-empty-group")
            .full_width(true)
            .selected_keys(Vec::<SharedString>::new())
            .default_selected_keys(["bold"])
            .on_selection_change(move |next, _, _| changes.borrow_mut().push(joined(next)))
            .child_toggle(bold)
            .child_toggle(italic)
            .child_toggle(underline)
            .into_any_element()
    });

    click(cx, 320., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bold"],
        "an explicitly controlled empty selection must override defaultSelectedKeys"
    );
}

#[gpui::test]
fn toggle_button_group_disabled_propagates_to_children(cx: &mut TestAppContext) {
    let selections = events();
    let selected = selections.clone();
    let probes = events();
    let probed = probes.clone();
    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let probes = probes.clone();
        let [bold, italic, underline] = three_toggles("disabled-group");
        let bold = bold.is_disabled(false);
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                ToggleButtonGroup::new("disabled-group")
                    .is_disabled(true)
                    .on_selection_change(move |next, _, _| {
                        selections.borrow_mut().push(joined(next));
                    })
                    .child_toggle(bold)
                    .child_toggle(italic)
                    .child_toggle(underline),
            )
            .child(
                Switch::new("disabled-group-probe").on_change(move |_, _, _| {
                    probes.borrow_mut().push("probe".into());
                }),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    assert_eq!(
        selected.borrow().as_slice(),
        ["bold"],
        "an explicitly enabled child must override the disabled group context"
    );
    assert!(probed.borrow().is_empty());
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(
        probed.borrow().as_slice(),
        ["probe"],
        "the remaining disabled group children must leave the tab order"
    );
}

#[gpui::test]
fn toggle_button_group_horizontal_arrows_stop_at_edges(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let [bold, italic, underline] = three_toggles("horizontal-edges");
        ToggleButtonGroup::new("horizontal-edges-group")
            .on_selection_change(move |next, _, _| changes.borrow_mut().push(joined(next)))
            .child_toggle(bold)
            .child_toggle(italic)
            .child_toggle(underline)
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "left");
    press(cx, "space");
    press(cx, "right");
    press(cx, "right");
    press(cx, "right");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bold", "underline"],
        "Left at the first member and Right at the last must not wrap"
    );
}

/// Pinned react-aria 3.51.0's standalone `useToolbar` stops propagation for
/// every handled along-axis arrow, even when its focus manager cannot move at
/// an edge. Cross-axis arrows remain available to the parent below.
#[gpui::test]
fn toggle_button_group_consumes_owned_edge_arrow(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let bubbled = events();
    let outer_seen = bubbled.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let bubbled = bubbled.clone();
        let [bold, italic, _] = three_toggles("owned-edge");
        gpui::div()
            .on_key_down(move |event, _, _| {
                if event.keystroke.key == "right" {
                    bubbled.borrow_mut().push("right".into());
                }
            })
            .child(
                ToggleButtonGroup::new("owned-edge-group")
                    .on_selection_change(move |next, _, _| {
                        changes.borrow_mut().push(joined(next));
                    })
                    .child_toggle(bold)
                    .child_toggle(italic),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "right");
    press(cx, "space");
    assert!(
        outer_seen.borrow().is_empty(),
        "a standalone group must consume its along-axis arrow at the edge"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["italic"],
        "the edge arrow must hold focus on the last member"
    );
}

#[gpui::test]
fn nested_toggle_button_group_releases_edge_arrow_to_toolbar(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let presses = presses.clone();
        let [bold, italic, _] = three_toggles("nested-toolbar");
        Toolbar::new()
            .child(
                ToggleButtonGroup::new("nested-toolbar-group")
                    .child_toggle(bold)
                    .child_toggle(italic),
            )
            .child(
                Button::new("nested-toolbar-next")
                    .label("Next")
                    .on_press(move |_, _, _| presses.borrow_mut().push("next".into())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "right");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["next"],
        "an unavailable group-edge arrow must reach the enclosing Toolbar"
    );
}

#[gpui::test]
fn toggle_button_group_skips_disabled_member_with_arrows(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let [bold, italic, underline] = three_toggles("disabled-arrow");
        ToggleButtonGroup::new("disabled-arrow-group")
            .on_selection_change(move |next, _, _| changes.borrow_mut().push(joined(next)))
            .child_toggle(bold)
            .child_toggle(italic.is_disabled(true))
            .child_toggle(underline)
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["underline"]);
}

#[gpui::test]
fn toggle_button_group_composes_child_press_after_selection(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let child_events = events.clone();
        let child_changes = events.clone();
        let selection_events = events.clone();
        ToggleButtonGroup::new("composed-child-press-group")
            .full_width(true)
            .on_selection_change(move |next, _, _| {
                selection_events
                    .borrow_mut()
                    .push(format!("selection:{}", joined(next)));
            })
            .child_toggle(
                ToggleButton::new("composed-child-press")
                    .key("bold")
                    .label("Bold")
                    .on_change(move |_, _, _| {
                        child_changes.borrow_mut().push("child-change".into());
                    })
                    .on_press(move |_, _, _| child_events.borrow_mut().push("child".into())),
            )
            .into_any_element()
    });

    click(cx, 960., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["selection:bold", "child"],
        "group selection must run once before child onPress and replace child onChange"
    );
}

#[gpui::test]
fn toggle_button_group_controlled_without_callback_silences_child_change(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        ToggleButtonGroup::new("controlled-child-change-group")
            .full_width(true)
            .selected_keys(["bold"])
            .child_toggle(
                ToggleButton::new("controlled-child-change")
                    .key("bold")
                    .label("Bold")
                    .on_change(move |_, _, _| events.borrow_mut().push("child-change".into())),
            )
            .into_any_element()
    });

    click(cx, 320., 18.);
    assert!(
        recorded.borrow().is_empty(),
        "a controlled group must own selection even without a group callback"
    );
}

#[gpui::test]
fn toggle_button_group_horizontal_uses_horizontal_arrows(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let [bold, italic, underline] = three_toggles("horizontal-axis");
        ToggleButtonGroup::new("horizontal-axis-group")
            .selection_mode(SelectionMode::Single)
            .on_selection_change(move |next, _, _| changes.borrow_mut().push(joined(next)))
            .child_toggle(bold)
            .child_toggle(italic)
            .child_toggle(underline)
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "space");
    press(cx, "left");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["italic", "bold"],
        "Right must move focus to Italic and Left back to Bold before Space activates each"
    );
}

#[gpui::test]
fn toggle_button_group_horizontal_ignores_vertical_arrows(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let bubbled = events();
    let outer_seen = bubbled.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let bubbled = bubbled.clone();
        let [bold, italic, underline] = three_toggles("horizontal-cross-axis");
        gpui::div()
            .on_key_down(move |event, _, _| {
                if matches!(event.keystroke.key.as_str(), "up" | "down") {
                    bubbled.borrow_mut().push(event.keystroke.key.clone());
                }
            })
            .child(
                ToggleButtonGroup::new("horizontal-cross-axis-group")
                    .selection_mode(SelectionMode::Single)
                    .on_selection_change(move |next, _, _| {
                        changes.borrow_mut().push(joined(next));
                    })
                    .child_toggle(bold)
                    .child_toggle(italic)
                    .child_toggle(underline),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "up");
    press(cx, "space");
    assert_eq!(
        outer_seen.borrow().as_slice(),
        ["down", "up"],
        "cross-axis arrows must remain available to an enclosing surface"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bold"],
        "Down and Up are cross-axis keys and must leave focus on Bold"
    );
}

#[gpui::test]
fn toggle_button_group_vertical_uses_vertical_arrows(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let [bold, italic, underline] = three_toggles("vertical-axis");
        ToggleButtonGroup::new("vertical-axis-group")
            .orientation(Orientation::Vertical)
            .selection_mode(SelectionMode::Single)
            .on_selection_change(move |next, _, _| changes.borrow_mut().push(joined(next)))
            .child_toggle(bold)
            .child_toggle(italic)
            .child_toggle(underline)
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "space");
    press(cx, "up");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["italic", "bold"],
        "Down must move focus to Italic and Up back to Bold before Space activates each"
    );
}

#[gpui::test]
fn toggle_button_group_vertical_arrows_stop_at_edges(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let [bold, italic, underline] = three_toggles("vertical-edges");
        ToggleButtonGroup::new("vertical-edges-group")
            .orientation(Orientation::Vertical)
            .on_selection_change(move |next, _, _| changes.borrow_mut().push(joined(next)))
            .child_toggle(bold)
            .child_toggle(italic)
            .child_toggle(underline)
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "up");
    press(cx, "space");
    press(cx, "down");
    press(cx, "down");
    press(cx, "down");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bold", "underline"],
        "Up at the first member and Down at the last must not wrap"
    );
}

#[gpui::test]
fn toggle_button_group_vertical_ignores_horizontal_arrows(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let bubbled = events();
    let outer_seen = bubbled.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let bubbled = bubbled.clone();
        let [bold, italic, underline] = three_toggles("vertical-cross-axis");
        gpui::div()
            .on_key_down(move |event, _, _| {
                if matches!(event.keystroke.key.as_str(), "left" | "right") {
                    bubbled.borrow_mut().push(event.keystroke.key.clone());
                }
            })
            .child(
                ToggleButtonGroup::new("vertical-cross-axis-group")
                    .orientation(Orientation::Vertical)
                    .selection_mode(SelectionMode::Single)
                    .on_selection_change(move |next, _, _| {
                        changes.borrow_mut().push(joined(next));
                    })
                    .child_toggle(bold)
                    .child_toggle(italic)
                    .child_toggle(underline),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "left");
    press(cx, "space");
    assert_eq!(
        outer_seen.borrow().as_slice(),
        ["right", "left"],
        "cross-axis arrows must remain available to an enclosing surface"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bold"],
        "Right and Left are cross-axis keys and must leave focus on Bold"
    );
}

#[gpui::test]
fn toggle_button_group_tab_leaves_for_the_next_control(cx: &mut TestAppContext) {
    let selections = events();
    let selected = selections.clone();
    let probes = events();
    let probed = probes.clone();
    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let probes = probes.clone();
        let [bold, italic, underline] = three_toggles("tab-exit");
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                ToggleButtonGroup::new("tab-exit-group")
                    .selection_mode(SelectionMode::Single)
                    .on_selection_change(move |next, _, _| {
                        selections.borrow_mut().push(joined(next));
                    })
                    .child_toggle(bold)
                    .child_toggle(italic)
                    .child_toggle(underline),
            )
            .child(Switch::new("tab-exit-probe").on_change(move |_, _, _| {
                probes.borrow_mut().push("probe".into());
            }))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(
        selected.borrow().as_slice(),
        [] as [&str; 0],
        "Tab must not land on another member of the same toggle group"
    );
    assert_eq!(
        probed.borrow().as_slice(),
        ["probe"],
        "one Tab from inside the group must reach the following control"
    );
}

#[gpui::test]
fn toggle_button_group_reverse_tab_restores_last_focused_member(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let [bold, italic, underline] = three_toggles("restore-last");
        gpui::div()
            .on_key_down(|event, window, cx| {
                if event.keystroke.key == "tab" {
                    cx.stop_propagation();
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                }
            })
            .flex()
            .flex_col()
            .child(
                ToggleButtonGroup::new("restore-last-group")
                    .on_selection_change(move |next, _, _| {
                        changes.borrow_mut().push(joined(next));
                    })
                    .child_toggle(bold)
                    .child_toggle(italic)
                    .child_toggle(underline),
            )
            .child(Switch::new("restore-last-after").on_change(|_, _, _| {}))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "tab");
    press(cx, "shift-tab");
    flush_frame(cx);
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["italic"],
        "reverse Tab must restore the group's last-focused enabled member"
    );
}

#[gpui::test]
fn toggle_button_group_reentry_skips_a_now_disabled_last_member(cx: &mut TestAppContext) {
    let disable_italic = Rc::new(RefCell::new(false));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let disabled = *disable_italic.borrow();
        let disable_on_probe = disable_italic.clone();
        let changes = changes.clone();
        let [bold, italic, underline] = three_toggles("restore-disabled");
        gpui::div()
            .on_key_down(|event, window, cx| {
                if event.keystroke.key == "tab" {
                    cx.stop_propagation();
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                }
            })
            .flex()
            .flex_col()
            .child(
                ToggleButtonGroup::new("restore-disabled-group")
                    .on_selection_change(move |next, _, _| {
                        changes.borrow_mut().push(joined(next));
                    })
                    .child_toggle(bold)
                    .child_toggle(italic.is_disabled(disabled))
                    .child_toggle(underline),
            )
            .child(
                Switch::new("restore-disabled-after").on_change(move |_, _, _| {
                    *disable_on_probe.borrow_mut() = true;
                }),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "tab");
    press(cx, "space");
    flush_frame(cx);
    press(cx, "shift-tab");
    flush_frame(cx);
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["underline"],
        "focus re-entry must stay on the enabled edge when the remembered member became disabled"
    );
}

#[gpui::test]
fn toggle_button_group_disallow_empty_keeps_the_only_selection(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let [bold, italic, underline] = three_toggles("disallow-empty");
        ToggleButtonGroup::new("disallow-empty-group")
            .full_width(true)
            .selection_mode(SelectionMode::Single)
            .selected_keys(["bold"])
            .disallow_empty_selection(true)
            .on_selection_change(move |next, _, _| changes.borrow_mut().push(joined(next)))
            .child_toggle(bold)
            .child_toggle(italic)
            .child_toggle(underline)
            .into_any_element()
    });

    click(cx, 320., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bold"],
        "re-pressing the only selected item must keep it selected"
    );
}

#[gpui::test]
fn toggle_button_group_ids_isolate_identical_child_ids(cx: &mut TestAppContext) {
    let first_bold = Rc::new(RefCell::new(false));
    let first_seen = first_bold.clone();
    let second_italic = Rc::new(RefCell::new(false));
    let second_seen = second_italic.clone();
    let _cx = open_host(cx, move || {
        let first_bold = first_bold.clone();
        let second_italic = second_italic.clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(
                ToggleButtonGroup::new("isolated-first-group")
                    .default_selected_keys(["bold"])
                    .child_toggle(ToggleButton::new("shared-bold").key("bold").content(
                        move |state| {
                            *first_bold.borrow_mut() = state.is_selected;
                            gpui::div().child("Bold").into_any_element()
                        },
                    ))
                    .child_toggle(
                        ToggleButton::new("shared-italic")
                            .key("italic")
                            .label("Italic"),
                    ),
            )
            .child(
                ToggleButtonGroup::new("isolated-second-group")
                    .default_selected_keys(["italic"])
                    .child_toggle(ToggleButton::new("shared-bold").key("bold").label("Bold"))
                    .child_toggle(ToggleButton::new("shared-italic").key("italic").content(
                        move |state| {
                            *second_italic.borrow_mut() = state.is_selected;
                            gpui::div().child("Italic").into_any_element()
                        },
                    )),
            )
            .into_any_element()
    });

    assert!(
        *first_seen.borrow(),
        "the first group's Bold seed must remain selected"
    );
    assert!(
        *second_seen.borrow(),
        "the second group must own a separate Italic seed despite identical child ids"
    );
}

#[gpui::test]
fn toggle_button_group_reorder_preserves_uncontrolled_selection(cx: &mut TestAppContext) {
    let reordered = Rc::new(RefCell::new(false));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let reordered_for_change = reordered.clone();
        let changes = changes.clone();
        let order = if *reordered.borrow() {
            ["italic", "bold", "underline"]
        } else {
            ["bold", "italic", "underline"]
        };
        let mut group = ToggleButtonGroup::new("reorder-stable-group")
            .full_width(true)
            .default_selected_keys(["bold"])
            .on_selection_change(move |next, _, _| {
                *reordered_for_change.borrow_mut() = true;
                changes.borrow_mut().push(joined(next));
            });
        for key in order {
            group = group.child_toggle(
                ToggleButton::new(gpui::ElementId::Name(format!("reorder-{key}").into()))
                    .key(key)
                    .label(key),
            );
        }
        group.into_any_element()
    });

    click(cx, 960., 18.);
    flush_frame(cx);
    click(cx, 320., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["italic", ""],
        "reordering children must not reseed the group's selected keys"
    );
}

#[gpui::test]
fn toggle_button_group_size_propagates_unless_child_overrides(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let [bold, italic, underline] = three_toggles("group-size");
        ToggleButtonGroup::new("group-size-group")
            .full_width(true)
            .size(Size::Lg)
            .on_selection_change(move |next, _, _| changes.borrow_mut().push(joined(next)))
            .child_toggle(bold)
            .child_toggle(italic.size(Size::Sm))
            .child_toggle(underline)
            .into_any_element()
    });

    click(cx, 320., 37.);
    click(cx, 960., 37.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bold"],
        "the group must make an unspecified child large while an explicit small child stays small"
    );
}

#[gpui::test]
fn choice_content_keeps_twenty_pixel_lines(cx: &mut TestAppContext) {
    for kind in 0..5 {
        for leading in [None, Some(48.)] {
            let cx = open_host(cx, move || {
                let control = match kind {
                    0 => herogpui_components::Checkbox::new("leading-checkbox")
                        .label("First\nSecond")
                        .into_any_element(),
                    1 => RadioGroup::new("leading-radio", vec![RadioOption::new("First\nSecond")])
                        .into_any_element(),
                    2 => Switch::new("leading-switch")
                        .content(|_| gpui::div().child("First\nSecond").into_any_element())
                        .into_any_element(),
                    3 => herogpui_components::CheckboxGroup::new(
                        "leading-checkbox-group",
                        vec![herogpui_components::CheckboxOption::new("option", "Label")
                            .description("First\nSecond")],
                    )
                    .into_any_element(),
                    _ => Switch::new("leading-switch-label")
                        .label("First\nSecond")
                        .into_any_element(),
                };
                gpui::div()
                    .when_some(leading, |el, leading| el.line_height(px(leading)))
                    .child(
                        gpui::div()
                            .debug_selector(|| "choice-leading".into())
                            .child(control),
                    )
                    .into_any_element()
            });
            assert_eq!(
                cx.debug_bounds("choice-leading")
                    .expect("choice content paints")
                    .size
                    .height,
                px(match kind {
                    3 => 56.,
                    _ => 40.,
                }),
                "pinned label/description lines; kind={kind}, host={leading:?}"
            );
        }
    }
}

fn gated_switch_height(cx: &mut VisualTestContext, name: &'static str) -> f32 {
    f32::from(cx.debug_bounds(name).expect("switch paints").size.height)
}

#[gpui::test]
fn switch_native_validate_is_gated_until_a_toggle_commits(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                gpui::div().debug_selector(|| "sw".to_owned()).child(
                    Switch::new("gated-switch")
                        .label("Enable alerts")
                        .validate(|_| Some("Always invalid".into())),
                ),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let rest = gated_switch_height(cx, "sw");
    press(cx, "tab");
    press(cx, "space");
    harness::wait_ms(cx, 800);
    assert!(
        gated_switch_height(cx, "sw") > rest,
        "a toggle must commit the gated validation and show the error"
    );
}

#[gpui::test]
fn switch_allow_validate_shows_live_at_rest(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                gpui::div().debug_selector(|| "sw".to_owned()).child(
                    Switch::new("allow-switch")
                        .label("Enable alerts")
                        .validation_behavior(ValidationBehavior::Allow)
                        .validate(|_| Some("Always invalid".into())),
                ),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    // Allow paints the live validity at rest, so the error row is present
    // without any commit.
    assert!(
        gated_switch_height(cx, "sw") > px(20.).into(),
        "Allow must paint the live validation error at rest"
    );
}
