//! Checkbox keyboard, form, validation and focus contracts.
//!
//! HeroUI v3.2.4 uses React Aria 3.51.0, React Stately 3.49.0 and React Aria
//! Components 1.20.0. In those pinned sources, a checkbox is a native
//! `type="checkbox"` input: Space changes it, Enter does not; `disabled`
//! removes it from focus while `aria-readonly` leaves it focusable; form data
//! reads the current checked state; and native validation blocks submission
//! while ARIA validation does not. HeroUI's CheckboxGroup validation example
//! reads the repeated selected values with `FormData.getAll`.

use crate::harness;

use std::{cell::RefCell, collections::HashSet, rc::Rc};

use gpui::{prelude::*, px, SharedString, TestAppContext, VisualTestContext};
use herogpui_components::{
    Button, Checkbox, CheckboxGroup, CheckboxOption, Form, FormData, Switch, ValidationBehavior,
};

use harness::{click, events, open_host, press};

#[gpui::test]
fn standalone_checkbox_activates_with_space_but_not_enter(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Checkbox::new("keyboard-checkbox")
            .label("Updates")
            .on_change(move |selected, _, _| changes.borrow_mut().push(selected.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "enter");
    assert!(
        recorded.borrow().is_empty(),
        "Enter on a native checkbox must not change its selection"
    );

    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["true"],
        "Space on a native checkbox must toggle it once"
    );
}

#[gpui::test]
fn checkbox_group_item_activates_with_space_but_not_enter(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        CheckboxGroup::new(
            "keyboard-group",
            vec![CheckboxOption::new("email", "Email")],
        )
        .on_change(move |selected, _, _| {
            changes.borrow_mut().push(
                selected
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "enter");
    assert!(
        recorded.borrow().is_empty(),
        "Enter on a grouped native checkbox must not change its selection"
    );

    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["email"]);
}

#[gpui::test]
fn standalone_checkbox_form_data_tracks_live_uncontrolled_state(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let checkbox = Checkbox::new("form-checkbox")
            .name("terms")
            .value("accepted")
            .label("Accept terms");
        let form = Form::new()
            .field(checkbox.form_field().expect("named checkbox field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(match data.get("terms") {
                    Some(value) => format!("value={}", value.as_text()),
                    None => "omitted".to_owned(),
                });
            });
        let submit = form.submit_handler();
        form.child(checkbox)
            .child(
                Button::new("checkbox-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    // The 14px label makes the checkbox row about 22.65px high; after the
    // Form's 16px gap, the 36px button is centred near y=57.
    click(cx, 60., 57.);
    click(cx, 8., 11.);
    click(cx, 60., 57.);

    assert_eq!(
        submitted.borrow().as_slice(),
        ["omitted", "value=accepted"],
        "an unchecked checkbox is absent and a checked one submits its live value"
    );
}

#[gpui::test]
fn checkbox_group_form_data_tracks_live_selected_values(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let group = CheckboxGroup::new(
            "form-group",
            vec![
                CheckboxOption::new("sms", "SMS"),
                CheckboxOption::new("email", "Email"),
            ],
        )
        .name("preferences");
        let form = Form::new()
            .field(group.form_field().expect("named checkbox group field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(
                    data.get_all("preferences")
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            });
        let submit = form.submit_handler();
        form.child(group)
            .child(
                Button::new("group-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    // Two checkbox rows occupy about 61px; the Form gap puts the submit
    // button centre near y=95.
    click(cx, 8., 11.);
    click(cx, 8., 46.);
    click(cx, 60., 95.);
    click(cx, 8., 11.);
    click(cx, 60., 95.);

    assert_eq!(
        submitted.borrow().as_slice(),
        ["sms,email", "email"],
        "CheckboxGroup form data must follow the live uncontrolled selection in control order"
    );
}

#[gpui::test]
fn form_reset_restores_standalone_checkbox_default(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let checkbox = Checkbox::new("reset-checkbox")
            .name("terms")
            .value("accepted")
            .default_selected(true)
            .label("Accept terms");
        let form = Form::new()
            .field(checkbox.form_field().expect("named checkbox field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(
                    data.text("terms")
                        .map_or_else(|| "omitted".to_owned(), |value| value.to_string()),
                );
            });
        let submit = form.submit_handler();
        let reset = form.reset_handler();
        form.child(checkbox)
            .child(
                Button::new("reset-checkbox-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .child(
                Button::new("reset-checkbox-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 8., 11.);
    click(cx, 60., 57.);
    click(cx, 60., 109.);
    click(cx, 60., 57.);

    assert_eq!(
        submitted.borrow().as_slice(),
        ["omitted", "accepted"],
        "native reset must restore defaultSelected in rendered state and subsequent FormData"
    );
}

#[gpui::test]
fn form_reset_restores_checkbox_group_default_values(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let group = CheckboxGroup::new(
            "reset-group",
            vec![
                CheckboxOption::new("sms", "SMS"),
                CheckboxOption::new("email", "Email"),
            ],
        )
        .name("preferences")
        .default_value(["sms".into()]);
        let form = Form::new()
            .field(group.form_field().expect("named checkbox group field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(
                    data.get_all("preferences")
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            });
        let submit = form.submit_handler();
        let reset = form.reset_handler();
        form.child(group)
            .child(
                Button::new("reset-group-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .child(
                Button::new("reset-group-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 8., 11.);
    click(cx, 8., 46.);
    click(cx, 60., 95.);
    click(cx, 60., 147.);
    click(cx, 60., 95.);

    assert_eq!(
        submitted.borrow().as_slice(),
        ["email", "sms"],
        "native reset must restore defaultValue in rendered state and subsequent get_all data"
    );
}

#[gpui::test]
fn form_reset_reports_controlled_checkbox_default_to_owner(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(false));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let selected_value = *selected.borrow();
        let checkbox = Checkbox::new("controlled-reset-checkbox")
            .name("terms")
            .value("accepted")
            .is_selected(selected_value)
            .default_selected(true)
            .label("Accept terms")
            .on_change({
                let selected = selected.clone();
                let changes = changes.clone();
                move |next, _, _| {
                    *selected.borrow_mut() = *next;
                    changes.borrow_mut().push(format!("change:{next}"));
                }
            });
        let form = Form::new()
            .field(checkbox.form_field().expect("named checkbox field"))
            .on_submit({
                let changes = changes.clone();
                move |data: &FormData, _, _| {
                    changes.borrow_mut().push(format!(
                        "submit:{}",
                        data.text("terms")
                            .map_or_else(|| "omitted".to_owned(), |value| value.to_string())
                    ));
                }
            });
        let reset = form.reset_handler();
        let submit = form.submit_handler();
        form.child(checkbox)
            .child(
                Button::new("controlled-reset-checkbox-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .child(
                Button::new("controlled-reset-checkbox-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 109.);
    click(cx, 60., 57.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:true", "submit:accepted"],
        "controlled reset must report defaultSelected so the owner can update"
    );
}

#[gpui::test]
fn form_reset_reports_controlled_group_defaults_to_owner(cx: &mut TestAppContext) {
    let selected: Rc<RefCell<HashSet<SharedString>>> =
        Rc::new(RefCell::new(HashSet::from(["email".into()])));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let selected_value = selected.borrow().clone();
        let group = CheckboxGroup::new(
            "controlled-reset-group",
            vec![
                CheckboxOption::new("sms", "SMS"),
                CheckboxOption::new("email", "Email"),
            ],
        )
        .name("preferences")
        .value(selected_value)
        .default_value(["sms".into()])
        .on_change({
            let selected = selected.clone();
            let changes = changes.clone();
            move |next, _, _| {
                *selected.borrow_mut() = next.clone();
                let mut values: Vec<_> = next.iter().map(ToString::to_string).collect();
                values.sort();
                changes
                    .borrow_mut()
                    .push(format!("change:{}", values.join(",")));
            }
        });
        let form = Form::new()
            .field(group.form_field().expect("named checkbox group field"))
            .on_submit({
                let changes = changes.clone();
                move |data: &FormData, _, _| {
                    changes.borrow_mut().push(format!(
                        "submit:{}",
                        data.get_all("preferences")
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    ));
                }
            });
        let reset = form.reset_handler();
        let submit = form.submit_handler();
        form.child(group)
            .child(
                Button::new("controlled-reset-group-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .child(
                Button::new("controlled-reset-group-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 147.);
    click(cx, 60., 95.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:sms", "submit:sms"],
        "controlled reset must report defaultValue so the owner can update"
    );
}

#[gpui::test]
fn form_reset_reports_disabled_controlled_checkbox_default_to_owner(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(false));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let selected_value = *selected.borrow();
        let checkbox = Checkbox::new("disabled-controlled-reset-checkbox")
            .name("terms")
            .value("accepted")
            .is_selected(selected_value)
            .default_selected(true)
            .is_disabled(true)
            .label("Unavailable terms")
            .on_change({
                let selected = selected.clone();
                let changes = changes.clone();
                move |next, _, _| {
                    *selected.borrow_mut() = *next;
                    changes.borrow_mut().push(format!("change:{next}"));
                }
            });
        let form = Form::new().field(checkbox.form_field().expect("named checkbox field"));
        let reset = form.reset_handler();
        form.child(checkbox)
            .child(
                Button::new("disabled-controlled-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 57.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:true"],
        "disabled controlled checkboxes still notify their owner on reset"
    );
}

#[gpui::test]
fn form_reset_reports_read_only_controlled_checkbox_default_to_owner(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(false));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let selected_value = *selected.borrow();
        let checkbox = Checkbox::new("readonly-controlled-reset-checkbox")
            .name("terms")
            .value("accepted")
            .is_selected(selected_value)
            .default_selected(true)
            .is_read_only(true)
            .label("Read only terms")
            .on_change({
                let selected = selected.clone();
                let changes = changes.clone();
                move |next, _, _| {
                    *selected.borrow_mut() = *next;
                    changes.borrow_mut().push(format!("change:{next}"));
                }
            });
        let form = Form::new().field(checkbox.form_field().expect("named checkbox field"));
        let reset = form.reset_handler();
        form.child(checkbox)
            .child(
                Button::new("readonly-controlled-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 57.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:true"],
        "read-only controlled checkboxes still notify their owner on reset"
    );
}

#[gpui::test]
fn form_reset_reports_disabled_controlled_group_defaults_to_owner(cx: &mut TestAppContext) {
    let selected: Rc<RefCell<HashSet<SharedString>>> =
        Rc::new(RefCell::new(HashSet::from(["email".into()])));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let selected_value = selected.borrow().clone();
        let group = CheckboxGroup::new(
            "disabled-controlled-reset-group",
            vec![
                CheckboxOption::new("sms", "SMS"),
                CheckboxOption::new("email", "Email"),
            ],
        )
        .name("preferences")
        .value(selected_value)
        .default_value(["sms".into()])
        .is_disabled(true)
        .on_change({
            let selected = selected.clone();
            let changes = changes.clone();
            move |next, _, _| {
                *selected.borrow_mut() = next.clone();
                changes.borrow_mut().push(
                    next.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
        });
        let form = Form::new().field(group.form_field().expect("named group field"));
        let reset = form.reset_handler();
        form.child(group)
            .child(
                Button::new("disabled-controlled-reset-group")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 95.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["sms"],
        "disabled controlled groups still notify their owner on reset"
    );
}

#[gpui::test]
fn form_reset_reports_read_only_controlled_group_defaults_to_owner(cx: &mut TestAppContext) {
    let selected: Rc<RefCell<HashSet<SharedString>>> =
        Rc::new(RefCell::new(HashSet::from(["email".into()])));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let selected_value = selected.borrow().clone();
        let group = CheckboxGroup::new(
            "readonly-controlled-reset-group",
            vec![
                CheckboxOption::new("sms", "SMS"),
                CheckboxOption::new("email", "Email"),
            ],
        )
        .name("preferences")
        .value(selected_value)
        .default_value(["sms".into()])
        .is_read_only(true)
        .on_change({
            let selected = selected.clone();
            let changes = changes.clone();
            move |next, _, _| {
                *selected.borrow_mut() = next.clone();
                changes.borrow_mut().push(
                    next.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
        });
        let form = Form::new().field(group.form_field().expect("named group field"));
        let reset = form.reset_handler();
        form.child(group)
            .child(
                Button::new("readonly-controlled-reset-group")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 95.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["sms"],
        "read-only controlled groups still notify their owner on reset"
    );
}

#[gpui::test]
fn disabled_checked_checkbox_is_not_submitted_or_validated(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let submits = events.clone();
        let invalids = events.clone();
        let checkbox = Checkbox::new("disabled-successful-checkbox")
            .name("terms")
            .value("accepted")
            .default_selected(true)
            .is_disabled(true)
            .is_required(true)
            .validate(|_| Some("Disabled controls do not validate".into()))
            .label("Unavailable terms");
        let form = Form::new()
            .field(checkbox.form_field().expect("named checkbox field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(format!(
                    "submit:{}",
                    if data.get("terms").is_none() {
                        "omitted"
                    } else {
                        "present"
                    }
                ));
            })
            .on_invalid(move |_, _, _| invalids.borrow_mut().push("invalid".to_owned()));
        let submit = form.submit_handler();
        form.child(checkbox)
            .child(
                Button::new("disabled-successful-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    // Native validation is commit-gated, so the configured message no longer
    // occupies the FieldError row at rest and the button sits one error row
    // (~16px) higher; disabled form semantics still omit the control.
    click(cx, 60., 61.);
    assert_eq!(recorded.borrow().as_slice(), ["submit:omitted"]);
}

#[gpui::test]
fn disabled_group_options_are_omitted_from_get_all(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let group = CheckboxGroup::new(
            "disabled-option-group",
            vec![
                CheckboxOption::new("sms", "SMS").is_disabled(true),
                CheckboxOption::new("email", "Email"),
            ],
        )
        .name("preferences")
        .default_value(["sms".into(), "email".into()]);
        let form = Form::new()
            .field(group.form_field().expect("named checkbox group field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(
                    data.get_all("preferences")
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            });
        let submit = form.submit_handler();
        form.child(group)
            .child(
                Button::new("disabled-option-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 95.);
    assert_eq!(submitted.borrow().as_slice(), ["email"]);
}

#[gpui::test]
fn disabled_invalid_group_is_not_submitted_or_validated(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let submits = events.clone();
        let invalids = events.clone();
        let group = CheckboxGroup::new(
            "disabled-invalid-group",
            vec![CheckboxOption::new("sms", "SMS")],
        )
        .name("preferences")
        .default_value(["sms".into()])
        .is_disabled(true)
        .is_required(true)
        .is_invalid(true);
        let form = Form::new()
            .field(group.form_field().expect("named checkbox group field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(format!(
                    "submit:{}",
                    if data.get("preferences").is_none() {
                        "omitted"
                    } else {
                        "present"
                    }
                ));
            })
            .on_invalid(move |_, _, _| invalids.borrow_mut().push("invalid".to_owned()));
        let submit = form.submit_handler();
        form.child(group)
            .child(
                Button::new("disabled-invalid-group-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 57.);
    assert_eq!(recorded.borrow().as_slice(), ["submit:omitted"]);
}

#[gpui::test]
fn invalid_group_blocks_and_focuses_first_enabled_option(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let changes = events.clone();
        let invalids = events.clone();
        let submits = events.clone();
        let group = CheckboxGroup::new(
            "invalid-focus-group",
            vec![
                CheckboxOption::new("disabled", "Disabled").is_disabled(true),
                CheckboxOption::new("enabled", "Enabled"),
            ],
        )
        .name("preferences")
        .is_invalid(true)
        .on_change(move |next, _, _| {
            let value = next.iter().next().map_or("none", SharedString::as_ref);
            changes.borrow_mut().push(format!("change:{value}"));
        });
        let form = Form::new()
            .field(group.form_field().expect("named checkbox group field"))
            .on_invalid(move |_, _, _| invalids.borrow_mut().push("invalid".to_owned()))
            .on_submit(move |_, _, _| submits.borrow_mut().push("submit".to_owned()));
        let submit = form.submit_handler();
        form.child(group)
            .child(
                Button::new("invalid-focus-group-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 95.);
    press(cx, "space");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["invalid", "change:enabled"],
        "invalid group must block and focus its first enabled option"
    );
}

#[gpui::test]
fn checkbox_native_custom_validation_blocks_and_focuses(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let changes = events.clone();
        let invalids = events.clone();
        let submits = events.clone();
        let checkbox = Checkbox::new("validated-checkbox")
            .name("terms")
            .value("accepted")
            .label("Accept terms")
            .validate(|selected| (!selected).then(|| "Terms are required".into()))
            .on_change(move |selected, _, _| {
                changes.borrow_mut().push(format!("change:{selected}"));
            });
        let form = Form::new()
            .field(checkbox.form_field().expect("named checkbox field"))
            .on_invalid(move |_, _, _| invalids.borrow_mut().push("invalid".to_owned()))
            .on_submit(move |_, _, _| submits.borrow_mut().push("submit".to_owned()));
        let submit = form.submit_handler();
        form.child(checkbox)
            .child(
                Button::new("validated-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    // Native validation is commit-gated, so at rest there is no error line and
    // the submit button sits one error row (~16px) higher than the live
    // `Allow` layout the sibling test clicks.
    click(cx, 60., 61.);
    press(cx, "space");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["invalid", "change:true"],
        "native custom validation must block submit and leave focus on the invalid checkbox"
    );
}

#[gpui::test]
fn checkbox_aria_custom_validation_does_not_block(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let invalids = events.clone();
        let submits = events.clone();
        let checkbox = Checkbox::new("aria-checkbox")
            .name("terms")
            .label("Accept terms")
            .validate(|selected| (!selected).then(|| "Terms are required".into()))
            .validation_behavior(ValidationBehavior::Allow);
        let form = Form::new()
            .field(checkbox.form_field().expect("named checkbox field"))
            .on_invalid(move |_, _, _| invalids.borrow_mut().push("invalid".to_owned()))
            .on_submit(move |_, _, _| submits.borrow_mut().push("submit".to_owned()));
        let submit = form.submit_handler();
        form.child(checkbox)
            .child(
                Button::new("aria-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    // ARIA validation still displays its message even though it does not block
    // submission, so the button sits below that line.
    click(cx, 60., 77.);
    assert_eq!(recorded.borrow().as_slice(), ["submit"]);
}

#[gpui::test]
fn checkbox_description_is_outside_the_clickable_content_row(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Checkbox::new("described-checkbox")
            .label("Weekly digest")
            .description("One email every Monday morning.")
            .on_change(move |selected, _, _| changes.borrow_mut().push(selected.to_string()))
            .into_any_element()
    });

    // The field root is a column: the description starts below the content
    // row and must not inherit that row's click listener.
    click(cx, 60., 31.);
    assert!(recorded.borrow().is_empty());

    click(cx, 60., 11.);
    assert_eq!(recorded.borrow().as_slice(), ["true"]);
}

#[gpui::test]
fn checkbox_validation_message_occupies_the_field_error_row(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                Checkbox::new("invalid-anatomy-checkbox")
                    .label("Accept terms")
                    .validation_errors(["Terms are required"]),
            )
            .child(
                Button::new("after-invalid-checkbox")
                    .label("Continue")
                    .on_press(move |_, _, _| events.borrow_mut().push("pressed".to_owned())),
            )
            .into_any_element()
    });

    // Without the sibling FieldError row this was the button's centre. The
    // 20px label + 16px outer gap + 18px half-button = 54px.
    // The message adds 16px plus the root's 4px gap.
    click(cx, 60., 54.);
    assert!(recorded.borrow().is_empty());

    click(cx, 60., 74.);
    assert_eq!(recorded.borrow().as_slice(), ["pressed"]);
}

#[gpui::test]
fn standalone_disabled_skips_focus_while_read_only_keeps_it(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                Checkbox::new("disabled")
                    .is_disabled(true)
                    .label("Disabled"),
            )
            .child(
                Checkbox::new("read-only")
                    .is_read_only(true)
                    .label("Read only")
                    .on_change({
                        let events = events.clone();
                        move |_, _, _| events.borrow_mut().push("checkbox".to_owned())
                    }),
            )
            .child(
                Switch::new("after-checkboxes")
                    .on_change(move |_, _, _| events.borrow_mut().push("switch".to_owned())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    assert!(
        recorded.borrow().is_empty(),
        "Tab must skip disabled and land on read-only, which cannot toggle"
    );
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["switch"]);
}

#[gpui::test]
fn group_disabled_skips_focus_while_read_only_keeps_it(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                CheckboxGroup::new(
                    "disabled-group",
                    vec![CheckboxOption::new("disabled", "Disabled")],
                )
                .is_disabled(true),
            )
            .child(
                CheckboxGroup::new(
                    "read-only-group",
                    vec![CheckboxOption::new("readonly", "Read only")],
                )
                .is_read_only(true)
                .on_change({
                    let events = events.clone();
                    move |_, _, _| events.borrow_mut().push("group".to_owned())
                }),
            )
            .child(
                Switch::new("after-groups")
                    .on_change(move |_, _, _| events.borrow_mut().push("switch".to_owned())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    assert!(
        recorded.borrow().is_empty(),
        "Tab must skip a disabled group and land on the read-only group item"
    );
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(recorded.borrow().as_slice(), ["switch"]);
}

// ---------------------------------------------------------------------------
// Commit-gated validation display (React Stately native timing)
// ---------------------------------------------------------------------------

/// Height of a `debug_selector`-wrapped field: the error row adds ~16px plus
/// the field's gap, so a gated error is invisible until a commit lands.
fn gated_height(cx: &mut VisualTestContext, name: &'static str) -> f32 {
    cx.debug_bounds(name)
        .expect("field paints")
        .size
        .height
        .into()
}

/// Clicks the top of a field's control band — above any error row — so the
/// press lands on the label/control row that toggles the checkbox.
fn click_control_band(cx: &mut VisualTestContext, name: &'static str) {
    let b = cx.debug_bounds(name).expect("field paints");
    // The field stretches to the window width but its clickable row is only
    // content-wide at the left; press the label/control band near the top,
    // above any error row.
    let x = f32::from(b.origin.x) + 40.0;
    let y = f32::from(b.origin.y) + 10.0;
    click(cx, x, y);
}

#[gpui::test]
fn checkbox_native_validate_is_gated_until_a_pointer_toggle_commits(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                gpui::div().debug_selector(|| "cb".to_owned()).child(
                    Checkbox::new("gated-checkbox")
                        .label("Accept terms")
                        .validate(|_| Some("Always invalid".into())),
                ),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let rest = gated_height(cx, "cb");
    // Pointer toggle commits the gated snapshot on the next render.
    click_control_band(cx, "cb");
    harness::wait_ms(cx, 800);
    let committed = gated_height(cx, "cb");
    assert!(
        committed > rest,
        "a pointer toggle must commit the gated validation and show the error"
    );
}

#[gpui::test]
fn checkbox_native_validate_is_gated_until_a_space_toggle_commits(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                gpui::div().debug_selector(|| "cb".to_owned()).child(
                    Checkbox::new("gated-checkbox")
                        .label("Accept terms")
                        .validate(|_| Some("Always invalid".into())),
                ),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let rest = gated_height(cx, "cb");
    press(cx, "tab");
    press(cx, "space");
    harness::wait_ms(cx, 800);
    assert!(
        gated_height(cx, "cb") > rest,
        "a Space toggle must commit the gated validation and show the error"
    );
}

#[gpui::test]
fn checkbox_native_validate_controlled_parent_still_commits(cx: &mut TestAppContext) {
    // A controlled checkbox whose owner ignores `on_change` keeps its value,
    // but the toggle still queues the commit — the notify is what lands it.
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                gpui::div().debug_selector(|| "cb".to_owned()).child(
                    Checkbox::new("controlled-checkbox")
                        .is_selected(false)
                        .label("Accept terms")
                        .validate(|_| Some("Always invalid".into()))
                        .on_change(|_, _, _| {}),
                ),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let rest = gated_height(cx, "cb");
    click_control_band(cx, "cb");
    harness::wait_ms(cx, 800);
    assert!(
        gated_height(cx, "cb") > rest,
        "the commit must land even when a controlled parent ignores on_change"
    );
}

#[gpui::test]
fn checkbox_native_validate_toggle_back_to_valid_clears(cx: &mut TestAppContext) {
    // Event-type field: the committed snapshot tracks the toggled value, so
    // toggling back to a valid selection clears the displayed error.
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                gpui::div().debug_selector(|| "cb".to_owned()).child(
                    Checkbox::new("clear-checkbox")
                        .label("Accept terms")
                        .validate(|selected| selected.then(|| "Checked is not allowed".into())),
                ),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let rest = gated_height(cx, "cb");
    // unchecked -> checked: invalid, committed by the toggle.
    click_control_band(cx, "cb");
    harness::wait_ms(cx, 800);
    let shown = gated_height(cx, "cb");
    assert!(
        shown > rest,
        "checking the box must commit and show the error"
    );
    // checked -> unchecked: valid again, the next toggle's commit clears it.
    click_control_band(cx, "cb");
    harness::wait_ms(cx, 800);
    assert_eq!(
        gated_height(cx, "cb"),
        rest,
        "toggling back to a valid value must clear the displayed error"
    );
}

#[gpui::test]
fn checkbox_allow_validate_shows_live_at_rest(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                gpui::div().debug_selector(|| "cb".to_owned()).child(
                    Checkbox::new("allow-checkbox")
                        .label("Accept terms")
                        .validate(|selected| (!selected).then(|| "Terms are required".into()))
                        .validation_behavior(ValidationBehavior::Allow),
                ),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "plain".to_owned())
                    .child(Checkbox::new("plain-checkbox").label("Plain")),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    assert!(
        gated_height(cx, "cb") > gated_height(cx, "plain"),
        "Allow must paint the validate error live at rest"
    );
}
