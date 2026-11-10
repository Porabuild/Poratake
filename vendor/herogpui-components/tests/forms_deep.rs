//! Behaviour tests that take Form, InputOTP and Fieldset to their edges.
//!
//! `fields.rs` and `text_fields.rs` already drive the basic collect-and-
//! validate path (a one-field invalid submit) and the basic OTP fill/walk.
//! These tests cover the configurations nobody has driven: reset semantics,
//! both `validationBehavior` halves, a form with two invalid fields, `validate`
//! closures that the component actually runs, the OTP edges (seeded values,
//! paste, arrows, no-op backspace, click-to-caret) and Fieldset's
//! composition parts.
//!
//! The Enter tests pin the native submission contract v3's Form inherits
//! ("renders a native `<form>` element"): Enter in a focused single-line
//! registered field runs the same submission the wired submit button runs
//! (valid data → `on_submit` once; empty required data → `on_invalid`, no
//! submit, first invalid field focused), a TextArea's Enter is a newline and
//! never submits, Enter on a focused submit button submits exactly once, and
//! Enter released on a non-text compound control (a Switch) never submits.
//! A blocked Enter defers its focus move past the keystroke, so the release
//! cannot click, open or toggle the control the repair lands on. The controls
//! that own Enter keep it: a field with its own `onSubmit`, and a ComboBox
//! whose open list answers Enter. Read-only fields stay successful and
//! focusable but are barred from constraint validation, and the OTP row —
//! one text input in pinned v3 — participates like any single-line field.
//!
//! These tests describe a GPUI substitute for implicit submission, not the
//! browser's rule. A browser picks the default submitter (the first submit
//! button in tree order) and skips implicit submission when there is no
//! submit button and more than one field blocking validation; gpui children
//! are opaque, so neither the submitter nor the blocking count can be read.
//! Enter in a participating field always runs the one shared submission —
//! the desktop composition documented in `form.rs`.
//!
//! Geometry is derived from the components' own constants, not guessed:
//!
//! - A `Form` stacks its children `gap(16)` (form.rs). A bare `Input` is
//!   `util::FIELD_HEIGHT` = 36px, an `InputOTP` row is 40px
//!   (`input_otp.rs`: cells 38x40). A routed server error renders that
//!   field's own error line *under the field* — 36px field + 4px gap + a
//!   16px message = a 56px column — so a field carrying one is 20px taller
//!   than a clean one, and clearing it shifts everything below up.
//! - An `InputOTP` cell *i* spans x 46·i..46·i+38 with an 8px gap, so its
//!   centre is (46·i+19, row_top + 20) — 4 cells fill x 0..152 and every
//!   click below that targets cell 0 is (19, ...) (`input_otp.rs`).
//! - An md `Button` is `Size::Md::control_height` = 36px (herogpui-core
//!   enums.rs); an md `Switch` track is 40x20 (`switch.rs`).
//! - A `Fieldset` gaps its children by 24 and its legend is a 24px line
//!   (`field.rs`); `Description` is 16px, `FieldGroup` gaps by 16.
//!   Legend(24) + gap(24) + 36px input → the input's centre is (60, 66);
//!   a Group of one input ends at y 84, so `Fieldset.Actions` under it sits
//!   at y 108 and its `pt-1` (field.rs) puts the button centre at (30, 130).
//!   With a `Description` inserted, the group starts at y 88 and its input
//!   centre is (60, 106).
//!
//! Every interactive element carries a distinct id and every field its own
//! state entity, so no two components share keyed state on one page
//! (AGENTS.md's documented silent failure).
//!
//! Every test in this file runs in the default suite — none require
//! `-- --ignored`. The two tests that used to be ignored as defects (a
//! field's `validate` message, and a `minLength` violation, each failing to
//! block a native submission) now pass: `Input::render` stores its resolved
//! validity on `InputState`, and `Form`'s blocked set is the missing-required
//! fields union the fields whose stored validity is invalid. Verify with:
//!
//! ```text
//! cargo test -p herogpui-components --test forms_deep
//! ```

use crate::harness;

use gpui::{prelude::*, px, Focusable, SharedString, TestAppContext};
use herogpui_components::{
    Button, Checkbox, ComboBox, ComboBoxFormValue, Date, DateField, Description, FieldGroup,
    Fieldset, FieldsetActions, FieldsetLegend, Form, FormData, FormField, Input, InputOTP,
    InputState, NumberField, NumberState, OtpPattern, OtpState, PickerItem, SearchField, Select,
    SelectionMode, Switch, TextArea, ValidationBehavior, ValidationErrors,
};

use harness::{click, events, open_host, press};

/// A submission rendered the way the gallery's own Form demo renders it:
/// `name=value` pairs in registration order, values through `as_text`.
fn record_data(data: &FormData) -> String {
    data.iter()
        .map(|(name, value)| format!("{name}={}", value.as_text()))
        .collect::<Vec<_>>()
        .join(",")
}

/// The names among `required` whose value the submission lacks, sorted.
///
/// The same computation `Form::submit_handler` runs to decide the invalid
/// path, so the recorded string is what the form itself saw.
fn record_missing(data: &FormData, required: &[&str]) -> String {
    let required: Vec<SharedString> = required
        .iter()
        .map(|s| SharedString::from(s.to_string()))
        .collect();
    let mut names: Vec<String> = data
        .missing_required(&required)
        .iter()
        .map(ToString::to_string)
        .collect();
    names.sort();
    names.join(",")
}

/// Where a test grabs the *rendered* form's own submit handler: the host
/// closure stores it on the first frame, so a server-error test can drive a
/// submission without clicking through a layout that shifts as error lines
/// come and go. The handler reads the same live entities the rendered form
/// reads, because that is the rendered form's handler.
type HandlerSlot = std::rc::Rc<
    std::cell::RefCell<Option<std::sync::Arc<dyn Fn(&mut gpui::Window, &mut gpui::App) + 'static>>>,
>;

fn handler_slot() -> HandlerSlot {
    std::rc::Rc::new(std::cell::RefCell::new(None))
}

/// Runs the handler a rendered form stored in `slot`.
///
/// A pending frame is drawn first: a delivery or an edit-suppression that ran
/// in a previous update's prepaint schedules the very frame whose layout and
/// validity mirror this submission must be judged against — within one
/// update, the closure runs before the draw.
fn run_handler(slot: &HandlerSlot, cx: &mut gpui::VisualTestContext) {
    cx.update(|window, _| window.refresh());
    let handler = slot
        .borrow()
        .clone()
        .expect("the rendered form must have stored its handler");
    cx.update(|window, cx| handler(window, cx));
}

// ---------------------------------------------------------------------------
// Form
// ---------------------------------------------------------------------------

/// Items whose labels are unique, so the key can be the label itself.
fn keyed(labels: &[&str]) -> Vec<PickerItem> {
    labels
        .iter()
        .map(|l| PickerItem::new(l.to_string(), l.to_string()))
        .collect()
}

#[gpui::test]
fn disabled_text_and_number_fields_are_omitted_until_enabled(cx: &mut TestAppContext) {
    let disabled = std::rc::Rc::new(std::cell::Cell::new(true));
    let disabled_for_view = disabled.clone();
    let text_state = cx.new(|cx| InputState::with_value(cx, "stale-value"));
    let number_state = cx.new(|cx| NumberState::new(cx, 42.));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let form = Form::new()
        .field(FormField::text(text_state.clone()))
        .field(FormField::number(number_state.clone()))
        .on_submit(move |data, _, _| {
            submitted_for_form.borrow_mut().push(record_data(data));
        });
    let submit = form.submit_handler();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                Input::new(text_state.clone())
                    .name("email")
                    .is_disabled(disabled_for_view.get()),
            )
            .child(
                NumberField::new(number_state.clone())
                    .name("amount")
                    .is_disabled(disabled_for_view.get()),
            )
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        [""],
        "disabled text and number inputs must not be successful form controls"
    );

    disabled.set(false);
    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        ["", "email=stale-value,amount=42"],
        "both live fields must become successful after their enabled rerender"
    );
}

#[gpui::test]
fn disabled_otp_is_omitted_until_enabled(cx: &mut TestAppContext) {
    let disabled = std::rc::Rc::new(std::cell::Cell::new(true));
    let disabled_for_view = disabled.clone();
    let state = cx.new(|cx| {
        let mut state = OtpState::with_length(cx, 4);
        state.set_code("1234");
        state
    });
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let invalids = events();
    let invalids_for_form = invalids.clone();
    let state_for_view = state.clone();
    let form = Form::new()
        .field(FormField::code("code", state.clone()).is_required(true))
        .on_submit(move |data, _, _| {
            submitted_for_form.borrow_mut().push(record_data(data));
        })
        .on_invalid(move |data, _, _| {
            invalids_for_form
                .borrow_mut()
                .push(record_missing(data, &["code"]));
        });
    let submit = form.submit_handler();
    let cx = open_host(cx, move || {
        InputOTP::new(state_for_view.clone())
            .is_disabled(disabled_for_view.get())
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        [""],
        "a disabled OTP must be omitted even when its state holds a code"
    );
    assert!(
        invalids.borrow().is_empty(),
        "a disabled required OTP must not block submission"
    );

    disabled.set(false);
    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        ["", "code=1234"],
        "the live OTP must become successful after its enabled rerender"
    );

    cx.update(|_, cx| state.update(cx, |state, _| state.clear()));
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        invalids.borrow().as_slice(),
        ["code"],
        "the enabled required OTP must resume native validation"
    );
}

#[gpui::test]
fn form_reset_restores_declared_defaults_then_on_reset(cx: &mut TestAppContext) {
    // v3's `onReset` fires on a native form reset, which restores each control
    // to its *default* value — the controlled state the field was seeded with,
    // not an empty string. Two fields, one with and one without a declared
    // default: reset must put the first back to "ada" and leave the second
    // exactly where typing left it (the port's documented contract — a field
    // with no default "only reports itself"). `on_reset` fires *after* the
    // restore, which the recorded values prove: the closure re-reads the
    // states, so "v1=ada" in the record can only mean the restore had run.
    let resets = events();
    let recorded_resets = resets.clone();
    let name_state = cx.new(|cx| InputState::new(cx));
    let email_state = cx.new(|cx| InputState::new(cx));
    let name_for_view = name_state.clone();
    let email_for_view = email_state.clone();
    let name_for_reset = name_state.clone();
    let email_for_reset = email_state.clone();
    let cx = open_host(cx, move || {
        let resets = resets.clone();
        // A fresh clone per render: the `on_reset` closure below captures it
        // by move, and the form is rebuilt every frame.
        let name_for_reset = name_for_reset.clone();
        let email_for_reset = email_for_reset.clone();
        let form = Form::new()
            .field(
                FormField::text(name_for_view.clone())
                    .is_required(true)
                    .default_text(name_for_view.clone(), "ada"),
            )
            .field(FormField::text(email_for_view.clone()))
            .on_reset(move |_, cx| {
                let v1 = name_for_reset.read(cx).value().to_owned();
                let v2 = email_for_reset.read(cx).value().to_owned();
                resets.borrow_mut().push(format!("v1={v1},v2={v2}"));
            });
        let reset = form.reset_handler();
        form.child(Input::new(name_for_view.clone()).name("name"))
            .child(Input::new(email_for_view.clone()).name("email"))
            .child(
                gpui::div().flex().gap(px(8.)).child(
                    Button::new("fr-reset")
                        .label("Reset")
                        .on_press(move |_, window, cx| reset(window, cx)),
                ),
            )
            .into_any_element()
    });

    // Two bare Inputs stacked `gap(16)` (field 1 y 0..36, field 2 y 52..88)
    // and the md reset button below (y 104..140) — the same layout fields.rs
    // measures. Type a value into each so both have something a reset could
    // take away.
    click(cx, 60., 18.);
    cx.simulate_input("bob");
    click(cx, 60., 70.);
    cx.simulate_input("kept@x.y");
    click(cx, 60., 122.);

    assert_eq!(
        recorded_resets.borrow().as_slice(),
        ["v1=ada,v2=kept@x.y"],
        "on_reset must fire after the restore, and the restored state must be \
         the declared default, never empty"
    );
    let name = cx.update(|_, cx| name_state.read(cx).value().to_owned());
    assert_eq!(
        name, "ada",
        "a field that declared a default must be restored to it by reset"
    );
    let email = cx.update(|_, cx| email_state.read(cx).value().to_owned());
    assert_eq!(
        email, "kept@x.y",
        "a field without a declared default must keep its typed value"
    );

    // A second reset fires on_reset again and restores the same way.
    click(cx, 60., 122.);
    assert_eq!(
        recorded_resets.borrow().as_slice(),
        ["v1=ada,v2=kept@x.y", "v1=ada,v2=kept@x.y"],
        "every reset must restore and report, not just the first"
    );
}

#[gpui::test]
fn form_allow_field_override_submits_with_empty_required(cx: &mut TestAppContext) {
    // v3: "This behavior can be set at the form level or overridden at
    // individual field level." A required field that opts out (`aria`) shows
    // its message but does not block the form, even when the form itself is
    // `native` — the field's override is what the submit consults. The port
    // reads the override off the field's own state (`InputState` carries
    // `validationBehavior` beside `name`, input.rs), so the assertion is the
    // submit firing with the empty value, which a missing override would
    // have blocked.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .field(FormField::text(state.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["name"]));
            });
        let submit = form.submit_handler();
        form.child(
            Input::new(state.clone())
                .name("name")
                .validation_behavior(ValidationBehavior::Allow),
        )
        .child(
            gpui::div().flex().gap(px(8.)).child(
                Button::new("fb-allow-field")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            ),
        )
        .into_any_element()
    });

    // Field y 0..36, md submit button y 52..88 — centre (60, 70).
    click(cx, 60., 70.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["name="],
        "a field that overrode to Allow must not block the native form's submit"
    );
    assert!(
        invalid.borrow().is_empty(),
        "onInvalid must not run when no field blocks the submission"
    );
}

#[gpui::test]
fn form_allow_submits_with_missing_required(cx: &mut TestAppContext) {
    // v3: `validationBehavior="aria"` "doesn't block submission" — at the
    // form level. Even an empty required field must not route the submit to
    // onInvalid.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .validation_behavior(ValidationBehavior::Allow)
            .field(FormField::text(state.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["name"]));
            });
        let submit = form.submit_handler();
        form.child(Input::new(state.clone()).name("name"))
            .child(
                gpui::div().flex().gap(px(8.)).child(
                    Button::new("fb-allow-form")
                        .label("Submit")
                        .on_press(move |_, window, cx| submit(window, cx)),
                ),
            )
            .into_any_element()
    });

    click(cx, 60., 70.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["name="],
        "an aria form must submit even with the required field empty"
    );
    assert!(
        invalid.borrow().is_empty(),
        "the invalid path must not run when the form allows submission"
    );
}

#[gpui::test]
fn form_two_invalid_fields_report_both_then_valid_submit(cx: &mut TestAppContext) {
    // v3's `onInvalid` fires "when the form validation fails", with the first
    // invalid field focused by default; the *data* the port hands over is what
    // identifies the fields, so the record must show both missing names at
    // once — a single-field record would hide a second failure. Then the same
    // form must accept a valid submit after the failed ones, which proves the
    // invalid path did not poison the valid one. The second field is an
    // `InputOTP` registered with `FormField::code`, exercising the OTP's
    // form integration (v3's "Form Example": `name="code"` and `isRequired`).
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let name_state = cx.new(|cx| InputState::new(cx));
    let otp_state = cx.new(|cx| OtpState::with_length(cx, 4));
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .field(FormField::text(name_state.clone()).is_required(true))
            .field(FormField::code("code", otp_state.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids
                    .borrow_mut()
                    .push(record_missing(data, &["name", "code"]));
            });
        let submit = form.submit_handler();
        form.child(Input::new(name_state.clone()).name("name"))
            .child(InputOTP::new(otp_state.clone()).name("code"))
            .child(
                gpui::div().flex().gap(px(8.)).child(
                    Button::new("fb-two")
                        .label("Submit")
                        .on_press(move |_, window, cx| submit(window, cx)),
                ),
            )
            .into_any_element()
    });

    // Layout with the OTP row in the stack: input y 0..36, OTP y 52..92
    // (40px tall), md submit button y 108..144 — centres (60, 18), (19, 72),
    // (60, 126). Two submits while both fields are empty: each must report
    // the same two missing names.
    click(cx, 60., 126.);
    click(cx, 60., 126.);
    assert_eq!(
        invalid.borrow().as_slice(),
        ["code,name", "code,name"],
        "an empty required Input AND an empty required OTP must both be \
         reported by onInvalid, on every failed submit"
    );
    assert!(
        submitted.borrow().is_empty(),
        "no submission may pass while two required fields are empty"
    );

    // Fill both fields and submit again: the same form must now take the
    // valid path and report both named values.
    click(cx, 60., 18.);
    cx.simulate_input("ada");
    click(cx, 19., 72.);
    press(cx, "1");
    press(cx, "2");
    press(cx, "3");
    press(cx, "4");
    click(cx, 60., 126.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["name=ada,code=1234"],
        "a fully filled required form must submit with every named value"
    );
}

#[gpui::test]
fn form_server_errors_route_by_name_and_block_native_submit(cx: &mut TestAppContext) {
    // v3: `validationErrors` is a `ValidationErrors` record — server-side
    // errors "mapped by field name. Displayed immediately and cleared when
    // user modifies the field." Routing is the contract: a name lands in
    // *that* field's own error slot, and under `native` a field carrying a
    // routed message blocks. Both fields are filled, so ONLY the routed
    // message can block; the unnamed sibling must receive nothing.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let email_state = cx.new(|cx| InputState::with_value(cx, "ada@x.y"));
    let name_state = cx.new(|cx| InputState::with_value(cx, "bob"));
    let email_for_view = email_state.clone();
    let name_for_view = name_state.clone();
    let record = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new().set_many("email", ["Already registered", "Check the server"]),
    ));
    let record_for_view = record.clone();
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        // A clone per frame retains the record's identity; only an explicit
        // swap below (a genuinely new record) re-arms.
        let record = record_for_view.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::text(email_for_view.clone()))
            .field(FormField::text(name_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(Input::new(email_for_view.clone()).name("email"))
            .child(Input::new(name_for_view.clone()).name("name"))
            .into_any_element()
    });

    run_handler(&submit_slot, cx);
    assert_eq!(
        invalid.borrow().as_slice(),
        ["email=ada@x.y,name=bob"],
        "a routed server message must route a native submit to onInvalid \
         even with every field filled"
    );
    assert!(
        submitted.borrow().is_empty(),
        "a native form carrying a routed server error must not submit"
    );
    let routed = cx.update(|_, cx| {
        (
            email_state.read(cx).routed_errors().to_vec(),
            name_state.read(cx).routed_errors().to_vec(),
        )
    });
    assert_eq!(
        routed.0,
        vec![
            SharedString::from("Already registered"),
            SharedString::from("Check the server")
        ],
        "the named field must receive every message of its entry, in \
         upstream order — this is the slice its error slot renders"
    );
    assert!(
        routed.1.is_empty(),
        "the sibling the record does not name must receive nothing"
    );

    // A genuinely new (empty) record is a new response: delivery clears the
    // named field's messages and the same form submits.
    record.replace(ValidationErrors::new());
    cx.update(|window, _| window.refresh());
    let routed = cx.update(|_, cx| email_state.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "a new record that drops the name must clear the routed messages"
    );
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["email=ada@x.y,name=bob"],
        "with the routed message gone the same form must submit"
    );
}

#[gpui::test]
fn form_server_errors_unmatched_name_neither_displays_nor_blocks(cx: &mut TestAppContext) {
    // A name no registered field displays cannot block: there is nothing to
    // show, and blocking without display would be an invisible failure.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "ada@x.y"));
    let state_for_view = state.clone();
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .validation_errors(ValidationErrors::new().set("unknown-field", "Nope"))
            .field(FormField::text(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(Input::new(state_for_view.clone()).name("email"))
            .into_any_element()
    });

    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["email=ada@x.y"],
        "a name no field matches must not block the submission"
    );
    assert!(
        invalid.borrow().is_empty(),
        "the invalid path must not run for an unmatched name"
    );
    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "the unmatched message must not land in the field's error slot"
    );
}

#[gpui::test]
fn form_allow_displays_routed_errors_without_blocking(cx: &mut TestAppContext) {
    // The `aria` half: "displays errors in realtime ... doesn't block
    // submission". The routed message must be present (it is what the
    // field's error slot renders) while the submit still goes through with
    // the empty required field.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .validation_behavior(ValidationBehavior::Allow)
            .validation_errors(ValidationErrors::new().set("name", "Check this field"))
            .field(FormField::text(state_for_view.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["name"]));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(Input::new(state_for_view.clone()).name("name"))
            .into_any_element()
    });

    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert_eq!(
        routed,
        vec![SharedString::from("Check this field")],
        "the routed message must be delivered so the field displays it, \
         whatever the behavior"
    );
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["name="],
        "an aria form must submit regardless of routed server errors and \
         empty fields"
    );
    assert!(
        invalid.borrow().is_empty(),
        "the invalid path must not run when the form allows submission"
    );
}

#[gpui::test]
fn form_server_errors_clear_on_edit_and_siblings_persist(cx: &mut TestAppContext) {
    // "Displayed immediately and cleared when user modifies the field": the
    // modification is *this field's* — one edit suppresses only its own
    // routed message, and the sibling's message survives it. While any named
    // field still carries its message, the native submit stays blocked; once
    // both are answered by edits, the same form submits.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let email_state = cx.new(|cx| InputState::with_value(cx, "a@b.c"));
    let name_state = cx.new(|cx| InputState::with_value(cx, "n"));
    let email_for_view = email_state.clone();
    let name_for_view = name_state.clone();
    let record_for_view = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new()
            .set("email", "Taken")
            .set("name", "Too short"),
    ));
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let record = record_for_view.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::text(email_for_view.clone()))
            .field(FormField::text(name_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(Input::new(email_for_view.clone()).name("email"))
            .child(Input::new(name_for_view.clone()).name("name"))
            .into_any_element()
    });

    // Both fields carry a routed error, so both are 56px columns: email
    // y 0..56, name y 72..128. Draw the delivery's scheduled frame first, so
    // the click below hit-tests the error-line layout it names. Clicking the
    // name field is only valid while that layout stands, so the sibling edit
    // goes first.
    cx.update(|window, _| window.refresh());
    click(cx, 60., 90.);
    cx.simulate_input("ame");
    let routed = cx.update(|_, cx| {
        (
            email_state.read(cx).routed_errors().to_vec(),
            name_state.read(cx).routed_errors().to_vec(),
        )
    });
    assert!(
        !routed.0.is_empty(),
        "the sibling edit must not suppress email's routed message"
    );
    assert!(
        routed.1.is_empty(),
        "editing the name field must suppress only its own routed message"
    );
    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().is_empty(),
        "the surviving sibling message must keep the submit blocked"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        ["email=a@b.c,name=name"],
        "the blocked submit must report the FormData it refused"
    );

    // The email field's top band never moves, with or without its error
    // line, and a click past the text's end parks the caret at the end.
    click(cx, 310., 18.);
    cx.simulate_input("x");
    let routed = cx.update(|_, cx| email_state.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "editing the email field must suppress its own routed message"
    );
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["email=a@b.cx,name=name"],
        "with every routed message answered by an edit, the same form must \
         submit"
    );
}

#[gpui::test]
fn form_server_errors_reset_hides_and_a_clone_does_not_resurrect(cx: &mut TestAppContext) {
    // Reset hides the routed server errors. What keeps them hidden is record
    // identity: the record that delivered already spent its revision, so a
    // re-render passing the same record (every frame passes a clone) must
    // not resurrect the message. A genuinely new record re-arms — the next
    // test pins that half against the clone.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "ada@x.y"));
    let state_for_view = state.clone();
    let record = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new().set("email", "Already registered"),
    ));
    let record_for_view = record.clone();
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let record = record_for_view.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::text(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(Input::new(state_for_view.clone()).name("email"))
            .into_any_element()
    });

    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().is_empty(),
        "the routed message must block before the reset"
    );
    assert_eq!(invalid.borrow().as_slice(), ["email=ada@x.y"]);
    invalid.borrow_mut().clear();

    // Reset hides the routed errors — the same reset_handler() a rendered
    // reset button wires, driven directly so the click needs no geometry.
    // It restores declared defaults and clears every routed message.
    cx.update(|window, cx| {
        let record = record.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::text(state.clone()));
        let reset = form.reset_handler();
        reset(window, cx);
    });
    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "reset must hide the routed server errors"
    );

    // Re-renders keep passing the same record — every frame a clone, same
    // revision, identity already spent. None may resurrect the message.
    cx.update(|window, _| window.refresh());
    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "a re-render passing a clone of the record must not resurrect the \
         message the reset hid"
    );
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["email=ada@x.y"],
        "after the reset the same form must submit"
    );
    assert!(
        invalid.borrow().is_empty(),
        "the only invalid run must be the blocked submit before the reset"
    );
}

#[gpui::test]
fn form_server_errors_a_new_record_rearms_but_a_clone_does_not(cx: &mut TestAppContext) {
    // The identity contract, end to end: an edit suppresses the routed
    // message; re-delivering the SAME record (a clone, same revision) keeps
    // it suppressed; a genuinely NEW record with content equal to the old
    // one re-arms it, because a re-sent server response is a response, not
    // silence. Structural equality cannot decide this — the revision does.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "ada@x.y"));
    let state_for_view = state.clone();
    let record = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new().set("email", "Already registered"),
    ));
    let record_for_view = record.clone();
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let record = record_for_view.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::text(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(Input::new(state_for_view.clone()).name("email"))
            .into_any_element()
    });

    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().is_empty(),
        "the first delivery must block"
    );
    invalid.borrow_mut().clear();

    // The user answers the error by editing; the message suppresses. The
    // click parks the caret at the value's end (past the text), so the typed
    // character appends.
    click(cx, 310., 18.);
    cx.simulate_input("x");
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["email=ada@x.yx"],
        "the edit must have suppressed the routed message"
    );
    assert!(invalid.borrow().is_empty());

    // The same record again — a clone, the revision already spent. Nothing
    // re-arms.
    let same_record = record.borrow().clone();
    record.replace(same_record);
    cx.update(|window, _| window.refresh());
    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "a clone of the delivered record must retain its identity and keep \
         the edit's suppression"
    );
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().len(),
        2,
        "the clone must not re-block the submit"
    );

    // A genuinely new record, content-equal to the first: a new response,
    // so every named field re-arms.
    record.replace(ValidationErrors::new().set("email", "Already registered"));
    cx.update(|window, _| window.refresh());
    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert_eq!(
        routed,
        vec![SharedString::from("Already registered")],
        "the new record must re-deliver despite equal content"
    );
    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().len() == 2 && !invalid.borrow().is_empty(),
        "the re-armed message must block again"
    );
}

#[gpui::test]
fn form_server_errors_receipt_survives_first_registration_churn(cx: &mut TestAppContext) {
    // The delivery receipt is per field — each field's own state stores the
    // revision of the record its messages came from — so the registration
    // list can churn without moving anyone's receipt. The regression this
    // pins is a receipt keyed to the form rather than to each field: a form
    // that looked undelivered because one registration was replaced would
    // redeliver the same clone everywhere, resurrecting errors the user had
    // edited away and re-blocking. Here the first registration is replaced
    // between submissions: the surviving field's edit suppression must hold
    // (the same clone must not re-arm it), and the fresh replacement entity
    // must receive the current record only on the next genuinely new
    // revision.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let surviving = cx.new(|cx| InputState::with_value(cx, "keep@x.y"));
    let replaced = cx.new(|cx| InputState::with_value(cx, "old@x.y"));
    let fresh = cx.new(|cx| InputState::with_value(cx, "new@x.y"));
    let surviving_for_view = surviving.clone();
    // `replaced` is only read through the rendered field, so move it.
    let replaced_for_view = replaced;
    let fresh_for_view = fresh.clone();
    let record = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new()
            .set("keep", "Still taken")
            .set("old", "Gone"),
    ));
    let record_for_view = record.clone();
    let swap = std::rc::Rc::new(std::cell::Cell::new(false));
    let swap_for_view = swap.clone();
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        // A clone per frame retains the record's identity across the churn.
        let record = record_for_view.borrow().clone();
        let mut form = Form::new()
            .validation_errors(record)
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        // The FIRST registration swaps between two entity-backed fields;
        // the surviving field keeps its entity behind it.
        form = if swap_for_view.get() {
            form.field(FormField::text(fresh_for_view.clone()).name("fresh"))
        } else {
            form.field(FormField::text(replaced_for_view.clone()).name("old"))
        };
        let form = form.field(FormField::text(surviving_for_view.clone()).name("keep"));
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(
            Input::new(if swap_for_view.get() {
                fresh_for_view.clone()
            } else {
                replaced_for_view.clone()
            })
            .name(if swap_for_view.get() { "fresh" } else { "old" }),
        )
        .child(Input::new(surviving_for_view.clone()).name("keep"))
        .into_any_element()
    });

    // Both named fields carry a routed message: the same clone blocks.
    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().is_empty(),
        "the first delivery must block"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        ["old=old@x.y,keep=keep@x.y"],
        "the blocked submit must report both routed messages"
    );

    // The surviving field answers its error by editing; a click past the
    // text's end parks the caret at the end, so the typed run appends.
    click(cx, 310., 90.);
    cx.simulate_input("x");
    let routed = cx.update(|_, cx| surviving.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "the edit must suppress the surviving field's routed message"
    );

    // Replace the first registration with a fresh entity (the record stays
    // a clone of the same revision) and redraw. The fresh entity has never
    // seen the record, but the SURVIVING field's receipt already names it —
    // the clone must not re-arm either of them.
    swap.set(true);
    cx.update(|window, _| window.refresh());
    let routed = cx.update(|_, cx| {
        (
            surviving.read(cx).routed_errors().to_vec(),
            fresh.read(cx).routed_errors().to_vec(),
        )
    });
    assert!(
        routed.0.is_empty(),
        "replacing the first registration must not resurrect the surviving \
         field's edit-suppressed message"
    );
    assert!(
        routed.1.is_empty(),
        "the fresh entity receives only what the current record names — \
         \"fresh\" is not in it"
    );
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["fresh=new@x.y,keep=keep@x.yx"],
        "with the surviving field suppressed and the fresh name unrouted, \
         the same clone must not re-block"
    );

    // A genuinely new record naming the fresh field is a new response: the
    // fresh entity's receipt (0) differs, so it receives the current
    // record; the surviving field's suppression survives the churn too.
    record.replace(ValidationErrors::new().set("fresh", "Fresh is taken"));
    cx.update(|window, _| window.refresh());
    let routed = cx.update(|_, cx| {
        (
            surviving.read(cx).routed_errors().to_vec(),
            fresh.read(cx).routed_errors().to_vec(),
        )
    });
    assert!(
        routed.0.is_empty(),
        "a new record that does not name the surviving field must leave its \
         suppression alone"
    );
    assert_eq!(
        routed.1,
        vec![SharedString::from("Fresh is taken")],
        "the fresh replacement entity must receive the current record"
    );
    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().len() == 1 && !invalid.borrow().is_empty(),
        "the fresh field's routed message must block again"
    );
    assert_eq!(
        invalid.borrow().last().unwrap().as_str(),
        "fresh=new@x.y,keep=keep@x.yx",
        "the replaced registration must not contribute to the FormData"
    );
}

#[gpui::test]
fn form_server_errors_otp_suppress_before_the_on_complete_submit(cx: &mut TestAppContext) {
    // v3: server errors are "cleared when user modifies the field" — and the
    // completing keystroke IS that modification, so the suppression must land
    // before `onComplete` runs. The auto-submit a one-time code invites fires
    // *synchronously inside* `onComplete`, so a suppression that runs after
    // the callbacks would let the stale routed error block the very submit
    // the keystroke was meant to finish. The scenario also pins delivery and
    // the native block: the record arrives between the third and fourth
    // digit, blocks once, and the completing keystroke answers it.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let record = std::rc::Rc::new(std::cell::RefCell::new(ValidationErrors::new()));
    let record_for_view = record.clone();
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let record = record_for_view.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::code("otp", state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        let submit = form.submit_handler();
        form.child(
            InputOTP::new(state_for_view.clone())
                .on_complete(move |_, window, cx| submit(window, cx)),
        )
        .into_any_element()
    });

    // The row is 40px, so cell 0's centre is (19, 20). Three digits land;
    // nothing is routed yet.
    click(cx, 19., 20.);
    press(cx, "1");
    press(cx, "2");
    press(cx, "3");
    assert!(
        submitted.borrow().is_empty() && invalid.borrow().is_empty(),
        "no record, no completion: nothing may have submitted yet"
    );

    // The server responds between the third and fourth digit. The delivery
    // canvas runs on the next drawn frame.
    record.replace(ValidationErrors::new().set("otp", "Expired code"));
    cx.update(|window, _| window.refresh());
    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert_eq!(
        routed,
        vec![SharedString::from("Expired code")],
        "the record's entry must be routed into the OTP state by name"
    );

    // The stale routed error blocks a native submit.
    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().is_empty(),
        "the routed error must block the native submit"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        ["otp=123"],
        "the blocked submit must report the partial code"
    );

    // The completing keystroke: the accepted mutation suppresses the routed
    // error BEFORE `on_complete` runs, and the synchronous submit inside it
    // must go through.
    press(cx, "4");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["otp=1234"],
        "the auto-submit from on_complete must not be blocked by the error \
         the completing keystroke itself answered"
    );
    assert_eq!(
        invalid.borrow().len(),
        1,
        "only the pre-completion block may have run the invalid path"
    );
    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "the accepted edit must have suppressed the routed message"
    );
}

#[gpui::test]
fn form_server_errors_number_field_step_suppresses(cx: &mut TestAppContext) {
    // The NumberField routes through its inner `InputState`: delivery, native
    // block and suppression all speak the same channel the text field does.
    // A user-driven step (the up arrow, which `report_bump` handles) is a
    // modification, so it must suppress the routed message and unblock the
    // same form.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| NumberState::new(cx, 5.));
    let state_for_view = state.clone();
    let record_for_view = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new().set("amount", "Out of range"),
    ));
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let record = record_for_view.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::number(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(NumberField::new(state_for_view.clone()).name("amount"))
            .into_any_element()
    });

    // The group is 36px tall, so the delivery frame puts the message under
    // it; the input band itself never moves.
    cx.update(|window, _| window.refresh());
    let routed = cx.update(|_, cx| state.read(cx).input.read(cx).routed_errors().to_vec());
    assert_eq!(
        routed,
        vec![SharedString::from("Out of range")],
        "the record's entry must be routed into the inner InputState by name"
    );

    // The routed error blocks a native submit.
    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().is_empty(),
        "the routed error must block the native submit"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        ["amount=5"],
        "the blocked submit must report the current value"
    );

    // A user-driven step is a modification: the up arrow steps 5 -> 6 and
    // suppresses the routed message, unblocking the same form.
    click(cx, 110., 18.);
    press(cx, "up");
    let routed = cx.update(|_, cx| state.read(cx).input.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "the step must have suppressed the routed message"
    );
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["amount=6"],
        "the stepped value must submit now the routed error is answered"
    );
    assert!(
        invalid.borrow().len() == 1,
        "only the pre-step block may have run the invalid path"
    );
}

#[gpui::test]
fn form_server_errors_input_edit_unblocks_a_synchronous_on_change_submit(cx: &mut TestAppContext) {
    // The mirror a native submit reads is written by `Input::render`, so an
    // accepted edit must refresh it in the same stroke as the suppression:
    // an `on_change` that submits the form synchronously (the auto-submit
    // v3's own docs invite) runs before any frame can catch up, and without
    // the refresh it is blocked by the routed error the very keystroke
    // answered — the OTP bug, one field over.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "ada@x.y"));
    let state_for_view = state.clone();
    let record_for_view = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new().set("email", "Already registered"),
    ));
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let record = record_for_view.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::text(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        let submit = form.submit_handler();
        *slot_for_view.borrow_mut() = Some(submit.clone());
        form.child(
            Input::new(state_for_view.clone())
                .name("email")
                // The auto-submit: the form is judged synchronously inside
                // `on_change`, before any frame has redrawn the field.
                .on_change(move |_, window, cx| submit(window, cx)),
        )
        .into_any_element()
    });

    // The routed error blocks a native submit.
    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().is_empty(),
        "the routed error must block the native submit"
    );
    assert_eq!(invalid.borrow().as_slice(), ["email=ada@x.y"]);
    invalid.borrow_mut().clear();

    // The answering edit: a click past the text's end parks the caret at the
    // end, so the typed run appends. The keystroke itself suppresses the
    // routed message, refreshes the stored mirror, and — still inside the
    // key handler — submits through `on_change`.
    click(cx, 310., 18.);
    cx.simulate_input("x");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["email=ada@x.yx"],
        "the synchronous submit inside on_change must not be blocked by the \
         routed error the edit itself answered"
    );
    assert!(
        invalid.borrow().is_empty(),
        "the answered error must not route this submit to onInvalid"
    );
    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "the accepted edit must have suppressed the routed message"
    );
    // The refreshed mirror is the one the next submit reads too — still
    // unblocked, and reporting the edited value.
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["email=ada@x.yx", "email=ada@x.yx"],
        "the refreshed mirror must keep the form unblocked on the next \
         submit as well"
    );
}

#[gpui::test]
fn form_server_errors_number_step_unblocks_a_synchronous_on_change_submit(cx: &mut TestAppContext) {
    // NumberField routes through its inner `InputState`, whose stored
    // validity `NumberField::render` writes. A user-driven step must
    // suppress the routed message AND refresh that inner mirror before its
    // `on_change` runs: a synchronous submit inside the callback is judged
    // before any frame can catch up, and without the refresh the inner
    // state still carries the routed error the step itself answered.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| NumberState::new(cx, 5.));
    let state_for_view = state.clone();
    let record_for_view = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new().set("amount", "Out of range"),
    ));
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let record = record_for_view.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::number(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        let submit = form.submit_handler();
        *slot_for_view.borrow_mut() = Some(submit.clone());
        form.child(
            NumberField::new(state_for_view.clone())
                .name("amount")
                // The auto-submit: judged synchronously inside `on_change`.
                .on_change(move |_, window, cx| submit(window, cx)),
        )
        .into_any_element()
    });

    // The routed error blocks a native submit.
    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().is_empty(),
        "the routed error must block the native submit"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        ["amount=5"],
        "the blocked submit must report the current value"
    );
    invalid.borrow_mut().clear();

    // The answering step: the click focuses the input (the group is 36px
    // tall, the field band y 0..36), and the up arrow steps 5 -> 6 through
    // `report_bump`, which must suppress and refresh before `on_change`
    // submits synchronously.
    click(cx, 110., 18.);
    press(cx, "up");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["amount=6"],
        "the synchronous submit inside on_change must not be blocked by the \
         routed error the step itself answered"
    );
    assert!(
        invalid.borrow().is_empty(),
        "the answered error must not route this submit to onInvalid"
    );
    let routed = cx.update(|_, cx| state.read(cx).input.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "the step must have suppressed the routed message"
    );
}

#[gpui::test]
fn form_server_errors_late_named_field_receives_the_current_record(cx: &mut TestAppContext) {
    // A field registered before its control first renders has no name yet —
    // and must stamp no receipt. A receipt taken before the name exists
    // spends the record's revision on an empty delivery, so the error could
    // never reach the field once its control renders and publishes its
    // name. Here the record is already present while the control is
    // missing; the later frame renders it against the same record (a
    // clone), and the field must receive the error then.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "ada@x.y"));
    let state_for_view = state.clone();
    let record_for_view = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new().set("email", "Already registered"),
    ));
    let shown = std::rc::Rc::new(std::cell::Cell::new(false));
    let shown_for_view = shown.clone();
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let record = record_for_view.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::text(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        // The registration is constant; only the control's first render is
        // deferred. The form renders either way, so the delivery runs on
        // every frame.
        if shown_for_view.get() {
            form.child(Input::new(state_for_view.clone()).name("email"))
                .into_any_element()
        } else {
            form.into_any_element()
        }
    });

    // The control has not rendered: the field has no name to route into,
    // and nothing may be written on its behalf — not even a receipt.
    cx.update(|window, _| window.refresh());
    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert!(
        routed.is_empty(),
        "a field whose control has not rendered has no name to route into"
    );

    // The control renders and publishes its name. The record is the same
    // clone — identical revision — and the field must receive its entry
    // now, because the earlier unnamed frames spent no receipt.
    shown.set(true);
    cx.update(|window, _| window.refresh());
    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert_eq!(
        routed,
        vec![SharedString::from("Already registered")],
        "the record delivered while the field was unnamed must still reach \
         it once its control renders under the same revision"
    );
    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().is_empty(),
        "the late-delivered message must block the native submit"
    );
    assert_eq!(invalid.borrow().as_slice(), ["email=ada@x.y"]);
}

#[gpui::test]
fn form_read_only_field_displays_routed_errors_without_blocking_or_focus(cx: &mut TestAppContext) {
    // A read-only field displays its routed message but is barred from
    // constraint validation: it neither blocks nor takes the failed-submit
    // focus, even when it is the FIRST field the record names. The block
    // and the focus belong to the eligible field behind it.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let ro_state = cx.new(|cx| InputState::with_value(cx, "locked"));
    let b_state = cx.new(|cx| InputState::with_value(cx, "bee"));
    let ro_for_view = ro_state.clone();
    let b_for_view = b_state.clone();
    let record_for_view = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new()
            .set("ro", "Locked")
            .set("b", "Too short"),
    ));
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let record = record_for_view.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::text(ro_for_view.clone()))
            .field(FormField::text(b_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(
            Input::new(ro_for_view.clone())
                .name("ro")
                .is_read_only(true),
        )
        .child(Input::new(b_for_view.clone()).name("b"))
        .into_any_element()
    });

    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().is_empty(),
        "the eligible field's routed message must block the submit"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        ["ro=locked,b=bee"],
        "the blocked submit must report both values — the read-only field \
         stayed successful"
    );
    let focused = cx.update(|window, cx| {
        (
            ro_state.read(cx).focus_handle(cx).is_focused(window),
            b_state.read(cx).focus_handle(cx).is_focused(window),
        )
    });
    assert!(
        !focused.0 && focused.1,
        "the failed-submit focus must skip the read-only field and land on \
         the first eligible one"
    );
    let routed = cx.update(|_, cx| ro_state.read(cx).routed_errors().to_vec());
    assert_eq!(
        routed,
        vec![SharedString::from("Locked")],
        "the read-only field must still display its routed message"
    );
}

#[gpui::test]
fn form_disabled_field_displays_routed_errors_without_blocking(cx: &mut TestAppContext) {
    // A disabled control is not a successful form control: it can never
    // block, whatever its state carries. The routed message is still
    // delivered so the field displays it — display and blocking are
    // independent halves of the routing.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "stale"));
    let state_for_view = state.clone();
    let record_for_view = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new().set("d", "Gone"),
    ));
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let record = record_for_view.borrow().clone();
        let form = Form::new()
            .validation_errors(record)
            .field(FormField::text(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(
            Input::new(state_for_view.clone())
                .name("d")
                .is_disabled(true),
        )
        .into_any_element()
    });

    let routed = cx.update(|_, cx| state.read(cx).routed_errors().to_vec());
    assert_eq!(
        routed,
        vec![SharedString::from("Gone")],
        "the disabled field must receive its routed message so it can display"
    );
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        [""],
        "the disabled field is not successful: it submits nothing and its \
         routed message cannot block"
    );
    assert!(
        invalid.borrow().is_empty(),
        "a disabled field must never route the submit to onInvalid"
    );
}

#[gpui::test]
fn form_duplicate_names_validate_each_field_not_the_record(cx: &mut TestAppContext) {
    // Two fields registered under one name are validated independently: the
    // submission record's first-wins `get` must never decide required
    // validity or focus. Here the FIRST "email" is filled and the second is
    // empty — reading the record alone would call the form complete and
    // submit. The per-field check blocks, and the failed-submit focus lands
    // on the empty field, not the filled one. FormData itself stays exact:
    // `get` still returns the first entry, the browser `getAll` shape.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let first_state = cx.new(|cx| InputState::with_value(cx, "a@b.c"));
    let second_state = cx.new(|cx| InputState::new(cx));
    let first_for_view = first_state.clone();
    let second_for_view = second_state.clone();
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .field(
                FormField::text(first_for_view.clone())
                    .name("email")
                    .is_required(true),
            )
            .field(
                FormField::text(second_for_view.clone())
                    .name("email")
                    .is_required(true),
            )
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(Input::new(first_for_view.clone()).name("email"))
            .child(Input::new(second_for_view.clone()).name("email"))
            .into_any_element()
    });

    // The FormData itself must stay browser-exact: first entry wins.
    let record_text = cx.update(|_, cx| {
        let form = Form::new()
            .field(FormField::text(first_state.clone()).name("email"))
            .field(FormField::text(second_state.clone()).name("email"));
        form.data(cx).text("email").map(|v| v.to_string())
    });
    assert_eq!(
        record_text.as_deref(),
        Some("a@b.c"),
        "FormData::get must keep its first-wins semantics"
    );

    run_handler(&submit_slot, cx);
    assert!(
        submitted.borrow().is_empty(),
        "the empty duplicate must block — the record's filled first entry \
         must not stand in for it"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        ["email=a@b.c,email="],
        "the blocked submit reports the record, which keeps every successful \
         entry — both duplicates — with first-wins `get`"
    );
    let focused = cx.update(|window, cx| {
        (
            first_state.read(cx).focus_handle(cx).is_focused(window),
            second_state.read(cx).focus_handle(cx).is_focused(window),
        )
    });
    assert!(
        !focused.0 && focused.1,
        "the failed-submit focus must land on the empty duplicate, not the \
         filled one the record happens to expose"
    );
}

#[gpui::test]
fn form_validate_runs_with_the_live_value(cx: &mut TestAppContext) {
    // v3's `validate` is "a function the component runs": the field evaluates
    // it against the current value and shows the returned message. The port
    // runs it in render (validation.rs), which no audit can see — prove it
    // behaviourally by recording the calls and asserting the last one carried
    // the value as typed, for both the Input and the InputOTP.
    let input_calls = events();
    let input_rec = input_calls.clone();
    let otp_calls = events();
    let otp_rec = otp_calls.clone();
    let input_state = cx.new(|cx| InputState::new(cx));
    let otp_state = cx.new(|cx| OtpState::with_length(cx, 4));
    let cx = open_host(cx, move || {
        let input_calls = input_calls.clone();
        let otp_calls = otp_calls.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                Input::new(input_state.clone())
                    .name("name")
                    .validate(move |value| {
                        input_calls.borrow_mut().push(value.to_owned());
                        (value.chars().count() < 3).then(|| "Too short".into())
                    }),
            )
            .child(InputOTP::new(otp_state.clone()).validate(move |code| {
                otp_calls.borrow_mut().push(code.to_owned());
                None
            }))
            .into_any_element()
    });

    // Input at y 0..36, OTP row at y 52..92 (40px) — centres (60, 18) and
    // (19, 72).
    click(cx, 60., 18.);
    cx.simulate_input("abc");
    assert_eq!(
        input_rec.borrow().last().map(String::as_str),
        Some("abc"),
        "the Input's validate closure must have been run with the value as \
         typed, not a stale read"
    );

    click(cx, 19., 72.);
    press(cx, "1");
    press(cx, "2");
    assert_eq!(
        otp_rec.borrow().last().map(String::as_str),
        Some("12"),
        "the InputOTP's validate closure must have been run with the assembled \
         code"
    );
}

#[gpui::test]
fn form_native_submit_focuses_first_invalid_field(cx: &mut TestAppContext) {
    // v3's `onInvalid` row: "Handler called when the form validation fails.
    // By default, the first invalid field will be focused." The reproduction
    // is a native form whose only required field is empty: after pressing
    // Submit the field must hold the focus. The port's submit_handler
    // (form.rs) only routes to onInvalid — nothing moves the focus, so the
    // assert below fails with the field unfocused.
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let invalids = invalids.clone();
        let form = Form::new()
            .field(FormField::text(state_for_view.clone()).is_required(true))
            .on_submit(|_, _, _| {})
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["name"]));
            });
        let submit = form.submit_handler();
        form.child(Input::new(state_for_view.clone()).name("name"))
            .child(
                gpui::div().flex().gap(px(8.)).child(
                    Button::new("fd-focus-submit")
                        .label("Submit")
                        .on_press(move |_, window, cx| submit(window, cx)),
                ),
            )
            .into_any_element()
    });

    // Field y 0..36, button y 52..88 — the button click both triggers the
    // invalid path and (in v3) hands the focus to the failed field.
    click(cx, 60., 70.);
    assert_eq!(
        invalid.borrow().as_slice(),
        ["name"],
        "the submit must have run the invalid path before focus is considered"
    );
    let focused = cx.update(|window, cx| {
        let handle = state.read(cx).focus_handle(cx);
        handle.is_focused(window)
    });
    assert!(
        focused,
        "the first invalid field must be focused after a failed native submit \
         (v3: 'By default, the first invalid field will be focused'); it is \
         not, which proves the submit path never moves the focus"
    );
}

#[gpui::test]
fn form_failed_submit_focus_skips_unnamed_fields(cx: &mut TestAppContext) {
    // The blocking lists collect NAMES — each `filter_map`s `field_name` —
    // so an unnamed field can never block a submission, however empty and
    // required it is. The failed-submit focus consults the same union, so it
    // must never land on the unnamed field either: the focus belongs to the
    // first NAMED blocker behind it. A focus that landed on the unnamed
    // field would point the user at an error the report never mentions.
    let invalids = events();
    let invalid = invalids.clone();
    let unnamed = cx.new(|cx| InputState::new(cx));
    let named = cx.new(|cx| InputState::new(cx));
    let unnamed_for_view = unnamed.clone();
    let named_for_view = named.clone();
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let invalids = invalids.clone();
        let form = Form::new()
            // Registered first and required-empty, but carries no name: it
            // is not in the submission and cannot block.
            .field(
                FormField::text(unnamed_for_view.clone())
                    .is_required(true),
            )
            .field(
                FormField::text(named_for_view.clone())
                    .name("named")
                    .is_required(true),
            )
            .on_submit(|_, _, _| {})
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(Input::new(unnamed_for_view.clone()))
            .child(Input::new(named_for_view.clone()).name("named"))
            .into_any_element()
    });

    run_handler(&submit_slot, cx);
    assert_eq!(
        invalid.borrow().as_slice(),
        ["named="],
        "only the named field may block: the unnamed one is not a form field"
    );
    let focused = cx.update(|window, cx| {
        (
            unnamed.read(cx).focus_handle(cx).is_focused(window),
            named.read(cx).focus_handle(cx).is_focused(window),
        )
    });
    assert!(
        !focused.0 && focused.1,
        "the failed-submit focus must skip the unnamed field and land on the \
         named blocker"
    );
}

#[gpui::test]
fn form_native_blocks_on_field_validate(cx: &mut TestAppContext) {
    // v3's Form Validation section: "Provide custom validation functions on
    // TextField components", and `validationBehavior` `native` "blocks form
    // submission on errors" — a field whose `validate` returns a message is
    // in error, so the native submit must route to onInvalid. The value here
    // satisfies `required` (it is non-empty), so the ONLY failure is the
    // validate message; the resolved validity is stored on `InputState`, and
    // the submit consults it, so onSubmit must not run and onInvalid must
    // report the blocked submission.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .field(FormField::text(state.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["name"]));
            });
        let submit = form.submit_handler();
        form.child(
            Input::new(state.clone())
                .name("name")
                .validate(|value| (value.chars().count() < 3).then(|| "Name too short".into())),
        )
        .child(
            gpui::div().flex().gap(px(8.)).child(
                Button::new("fd-validate-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            ),
        )
        .into_any_element()
    });

    // The field is still uncommitted — `validate`'s message joins the
    // display only after blur or submit — so no error line sits between the
    // field and the button and the button rests at 52..88: centre (60, 70).
    // The stored validity already says the field is in error, which is what
    // the submit consults.
    click(cx, 60., 18.);
    cx.simulate_input("ab");
    click(cx, 60., 70.);
    assert!(
        submitted.borrow().is_empty(),
        "a field validate failure must block the native submit so onSubmit \
         never fires (v3: 'blocks form submission on errors') — a fire \
         means the field's stored validity did not block the native submit"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        [""],
        "a field validate failure must route the native submit to onInvalid"
    );
}

#[gpui::test]
fn form_submit_commits_the_display_and_reset_clears_it(cx: &mut TestAppContext) {
    // React Stately's native contract, end to end: the displayed validity is
    // a snapshot committed by a change-blur or by submission — an untouched
    // failing field paints no error until Submit commits it — and the form's
    // reset (`resetValidation`) returns the display to valid without
    // committing anything new.
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let submit_slot = handler_slot();
    let submit_for_view = submit_slot.clone();
    let reset_slot = handler_slot();
    let reset_for_view = reset_slot.clone();
    let cx = open_host(cx, move || {
        let invalids = invalids.clone();
        let form = Form::new()
            .field(FormField::text(state.clone()).is_required(true))
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["name"]));
            });
        *submit_for_view.borrow_mut() = Some(form.submit_handler());
        *reset_for_view.borrow_mut() = Some(form.reset_handler());
        form.child(
            gpui::div()
                .debug_selector(|| "fd-commit-field".to_owned())
                .child(
                    Input::new(state.clone())
                        .name("name")
                        .label("Name")
                        .is_required(true)
                        .validate(|value| (value.len() < 3).then(|| "Name too short".into())),
                ),
        )
        .into_any_element()
    });
    cx.run_until_parked();
    let rest = cx.debug_bounds("fd-commit-field").unwrap().size.height;
    // Submit with the field untouched and failing: the commit paints the
    // error line the pending validity was already holding.
    run_handler(&submit_slot, cx);
    harness::wait_ms(cx, 800);
    let shown = cx.debug_bounds("fd-commit-field").unwrap().size.height;
    assert!(
        shown > rest,
        "a submission must commit every field's pending validity into the \
         displayed snapshot — the untouched failing field's error must paint"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        ["name"],
        "the same submission must still route to onInvalid for the missing \
         required value"
    );
    // Reset clears the displayed snapshot; the error row animates out.
    run_handler(&reset_slot, cx);
    harness::wait_ms(cx, 800);
    assert_eq!(
        cx.debug_bounds("fd-commit-field").unwrap().size.height,
        rest,
        "reset must clear the displayed validation back to valid"
    );
}

#[gpui::test]
fn form_mixed_fields_commit_every_gated_error_on_submit_and_reset_clears(cx: &mut TestAppContext) {
    // One Form mixing a text field, a number field, a checkbox and a date
    // field: submission commits every field's pending validity into its
    // displayed snapshot at once — so all four gated errors paint together —
    // while the live validity still blocks the submit; reset clears them all.
    let input_state = cx.new(|cx| InputState::new(cx));
    let number_state = cx.new(|cx| NumberState::new(cx, 5.));
    let date_state = cx.new(|cx| InputState::new(cx));
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let submit_slot = handler_slot();
    let submit_for_view = submit_slot.clone();
    let reset_slot = handler_slot();
    let reset_for_view = reset_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let input_state = input_state.clone();
        let number_state = number_state.clone();
        let date_state = date_state.clone();
        let input = Input::new(input_state.clone())
            .name("name")
            .label("Name")
            .validate(|_| Some("bad name".into()));
        let number = NumberField::new(number_state.clone())
            .name("amount")
            .validate(|value: &f64| (*value < 18.).then(|| "too small".into()));
        let checkbox = Checkbox::new("mixed-check")
            .name("agree")
            .label("Agree")
            .validate(|checked: &bool| (!*checked).then(|| "must agree".into()));
        let date = DateField::new(date_state)
            .name("day")
            .validate(|value: &Option<Date>| value.is_none().then(|| "pick a date".into()));
        let form = Form::new()
            .field(FormField::text(input_state))
            .field(FormField::number(number_state))
            .field(checkbox.form_field().expect("named checkbox field"))
            .field(date.form_field().expect("named date field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids
                    .borrow_mut()
                    .push(record_missing(data, &["name", "amount", "agree", "day"]));
            });
        *submit_for_view.borrow_mut() = Some(form.submit_handler());
        *reset_for_view.borrow_mut() = Some(form.reset_handler());
        form.child(
            gpui::div()
                .debug_selector(|| "f-name".to_owned())
                .child(input),
        )
        .child(
            gpui::div()
                .debug_selector(|| "f-amount".to_owned())
                .child(number),
        )
        .child(
            gpui::div()
                .debug_selector(|| "f-agree".to_owned())
                .child(checkbox),
        )
        .child(
            gpui::div()
                .debug_selector(|| "f-day".to_owned())
                .child(date),
        )
        .into_any_element()
    });
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    let height = |cx: &mut gpui::VisualTestContext, name: &'static str| {
        f32::from(cx.debug_bounds(name).expect("field paints").size.height)
    };
    let rest = [
        height(cx, "f-name"),
        height(cx, "f-amount"),
        height(cx, "f-agree"),
        height(cx, "f-day"),
    ];
    // Untouched and failing: nothing paints yet.
    assert!(
        invalid.borrow().is_empty() && submitted.borrow().is_empty(),
        "no submission has run yet"
    );
    // Submit commits every gated snapshot at once: all four error rows paint
    // together and the live validity still blocks the submit.
    run_handler(&submit_slot, cx);
    harness::wait_ms(cx, 800);
    let shown = [
        height(cx, "f-name"),
        height(cx, "f-amount"),
        height(cx, "f-agree"),
        height(cx, "f-day"),
    ];
    for (name, (before, after)) in ["name", "amount", "agree", "day"]
        .iter()
        .zip(rest.iter().zip(shown.iter()))
    {
        assert!(
            after > before,
            "submit must commit the {name} field's gated error into the display"
        );
    }
    assert!(
        submitted.borrow().is_empty(),
        "the all-invalid form must still block the submit"
    );
    assert_eq!(
        invalid.borrow().len(),
        1,
        "the blocked submit routes to onInvalid once"
    );
    // Reset clears every displayed snapshot; all four error rows animate out.
    run_handler(&reset_slot, cx);
    harness::wait_ms(cx, 800);
    for (name, before) in ["name", "amount", "agree", "day"].iter().zip(rest.iter()) {
        assert_eq!(
            height(
                cx,
                match *name {
                    "name" => "f-name",
                    "amount" => "f-amount",
                    "agree" => "f-agree",
                    _ => "f-day",
                }
            ),
            *before,
            "reset must clear the {name} field's displayed error"
        );
    }
}

#[gpui::test]
fn form_native_blocks_on_otp_validate(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| {
        let mut state = OtpState::with_length(cx, 4);
        state.set_code("0000");
        state
    });
    let state_for_view = state.clone();
    let form = Form::new()
        .field(FormField::code("code", state.clone()))
        .on_submit(move |data, _, _| {
            submitted.borrow_mut().push(record_data(data));
        })
        .on_invalid(move |data, _, _| {
            invalid.borrow_mut().push(record_data(data));
        });
    let submit = form.submit_handler();
    let cx = open_host(cx, move || {
        InputOTP::new(state_for_view.clone())
            .validate(|_| Some("Code is invalid".into()))
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert!(
        submits.borrow().is_empty(),
        "an OTP validate failure must block native submission"
    );
    assert_eq!(
        invalids.borrow().as_slice(),
        ["code=0000"],
        "an OTP validate failure must route the current FormData to onInvalid"
    );
    let focused = cx.update(|window, cx| state.read(cx).focus_handle(cx).is_focused(window));
    assert!(focused, "the invalid OTP must receive failed-submit focus");
}

#[gpui::test]
fn form_native_blocks_on_min_length(cx: &mut TestAppContext) {
    // v3's Form Validation section: "Use built-in HTML5 validation attributes
    // (required, minLength, pattern, etc.)". Under `native` a `minLength`
    // violation is a field error like any other: submission is blocked and
    // onInvalid fires — the violation is resolved into the field's
    // `InputState` validity, which the submit consults, so onSubmit must not
    // run with the 2-char value and onInvalid must report it.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .field(FormField::text(state.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["name"]));
            });
        let submit = form.submit_handler();
        form.child(Input::new(state.clone()).name("name").min_length(5))
            .child(
                gpui::div().flex().gap(px(8.)).child(
                    Button::new("fd-minlen-submit")
                        .label("Submit")
                        .on_press(move |_, window, cx| submit(window, cx)),
                ),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("ab");
    click(cx, 60., 70.);
    assert!(
        submitted.borrow().is_empty(),
        "a minLength violation must block the native submit — a fire means \
         the field's stored validity did not block the native submit"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        [""],
        "a minLength violation must route the native submit to onInvalid"
    );
}

#[gpui::test]
fn form_enter_in_focused_field_runs_the_submit_button_path(cx: &mut TestAppContext) {
    // v3's Form "renders a native `<form>` element" (the pinned docs), so
    // Enter pressed in a focused single-line text control is the browser's
    // implicit submission and must run the same validation-and-route path
    // the wired submit button runs: one `on_submit` carrying the typed
    // value, `on_invalid` never. The same record arriving from both
    // activations — the keystroke and the click — is the proof that both
    // doors share one submission implementation.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .field(FormField::text(state_for_view.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["name"]));
            });
        let submit = form.submit_handler();
        form.child(Input::new(state_for_view.clone()).name("name"))
            .child(
                gpui::div().flex().gap(px(8.)).child(
                    Button::new("fe-enter-valid")
                        .label("Submit")
                        .on_press(move |_, window, cx| submit(window, cx)),
                ),
            )
            .into_any_element()
    });

    // Field y 0..36, md button y 52..88. Clicking a `track_focus` element
    // transfers the focus (gpui registers a focus-transferring mouse-down),
    // so the field holds the focus when Enter goes down.
    click(cx, 60., 18.);
    cx.simulate_input("ada");
    press(cx, "enter");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["name=ada"],
        "Enter in a focused single-line registered field must run the \
         submission path exactly once with the typed value"
    );
    assert!(
        invalid.borrow().is_empty(),
        "a valid Enter submission must never route to on_invalid"
    );

    // The button click must produce the same record: one path, two doors.
    click(cx, 60., 70.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["name=ada", "name=ada"],
        "the submit button and the Enter keystroke must submit identical \
         data through one shared implementation"
    );
}

#[gpui::test]
fn form_enter_with_empty_required_focuses_first_invalid(cx: &mut TestAppContext) {
    // The blocked half of the same native path: Enter with the required
    // fields empty must not submit, must route to on_invalid once, and —
    // v3, verbatim — "the first invalid field will be focused". The focus
    // moves mid-keystroke here (the second field holds it when Enter goes
    // down, the first registered field receives it), which is safe for
    // these fields because a text field binds no click listener for gpui's
    // key-up activation to fire.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let name_state = cx.new(|cx| InputState::new(cx));
    let name_for_view = name_state.clone();
    let email_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .field(FormField::text(name_for_view.clone()).is_required(true))
            .field(FormField::text(email_for_view.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids
                    .borrow_mut()
                    .push(record_missing(data, &["name", "email"]));
            });
        form.child(Input::new(name_for_view.clone()).name("name"))
            .child(Input::new(email_for_view.clone()).name("email"))
            .into_any_element()
    });

    // Field 1 y 0..36, field 2 y 52..88 — focus the second, press Enter.
    click(cx, 60., 70.);
    press(cx, "enter");
    assert_eq!(
        invalid.borrow().as_slice(),
        ["email,name"],
        "Enter with both required fields empty must run the invalid path, \
         reporting both missing names"
    );
    assert!(
        submitted.borrow().is_empty(),
        "a blocked Enter must never reach on_submit"
    );
    let focused = cx.update(|window, cx| name_state.read(cx).focus_handle(cx).is_focused(window));
    assert!(
        focused,
        "a blocked Enter must focus the first registered invalid field (v3: \
         'By default, the first invalid field will be focused'), not the \
         field the keystroke started in"
    );
}

#[gpui::test]
fn form_textarea_enter_is_a_newline_and_never_submits(cx: &mut TestAppContext) {
    // A native form never implicitly submits from a `<textarea>`: Enter is
    // a newline there. The port's multiline-ness lives on the TextArea
    // builder, not on the shared `InputState`, so a registration names the
    // control kind — `FormField::text_area` reads exactly like
    // `FormField::text` but never participates in implicit submission.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .field(FormField::text_area(state_for_view.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["bio"]));
            });
        form.child(TextArea::new(state_for_view.clone()).name("bio"))
            .into_any_element()
    });

    // An EMPTY multi-line field hugs its content (~32px wide); a click at
    // (10, 40) is the proven-inside point (text_fields.rs). Enter must land
    // as a newline in the value and nowhere else.
    click(cx, 10., 40.);
    cx.simulate_input("hello");
    press(cx, "enter");
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        value, "hello\n",
        "Enter in a focused TextArea must insert a newline into the value"
    );
    assert!(
        submitted.borrow().is_empty(),
        "a TextArea's Enter must never submit the form"
    );
    assert!(
        invalid.borrow().is_empty(),
        "a TextArea's newline must not run the invalid path either"
    );
}

#[gpui::test]
fn form_enter_on_focused_submit_button_submits_exactly_once(cx: &mut TestAppContext) {
    // Click the submit button — which focuses it *and* submits — then press
    // Enter while it holds the focus. gpui activates a focused element's
    // click listeners on key-up, so the button owns exactly one submission
    // per activation; a form-root handler that also fired on the key-down
    // would make it two. Two records total, each the same data.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let form = Form::new()
            .field(FormField::text(state_for_view.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["name"]));
            });
        let submit = form.submit_handler();
        form.child(Input::new(state_for_view.clone()).name("name"))
            .child(
                gpui::div().flex().gap(px(8.)).child(
                    Button::new("fe-enter-button")
                        .label("Submit")
                        .on_press(move |_, window, cx| submit(window, cx)),
                ),
            )
            .into_any_element()
    });

    // Field y 0..36 (fill it so the form is submittable), button y 52..88.
    click(cx, 60., 18.);
    cx.simulate_input("ada");
    click(cx, 60., 70.);
    press(cx, "enter");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["name=ada", "name=ada"],
        "Enter on a focused submit button must submit exactly once per \
         activation — one record from the click, one from the keystroke"
    );
    assert!(
        invalid.borrow().is_empty(),
        "a valid submission must never route to on_invalid"
    );
}

#[gpui::test]
fn form_enter_in_a_compound_control_never_submits(cx: &mut TestAppContext) {
    // A required text field plus a Switch — a non-text compound control.
    // Fill the field, Tab to the switch (the app focus root's arrow, and
    // the way the switch tests focus a control without a pointer), press
    // Enter: the switch toggles through its own key-up activation — the
    // record proves the key really reached it — and the form does not
    // submit. Implicit submission is a text-field behaviour; an Enter
    // bubbling out of a compound control must not be read as one.
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let changes = events();
    let recorded = changes.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let changes = changes.clone();
        let form = Form::new()
            .field(FormField::text(state_for_view.clone()).is_required(true))
            .field(FormField::flag("notify", false))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["name"]));
            });
        form.child(Input::new(state_for_view.clone()).name("name"))
            .child(
                Switch::new("fe-enter-switch").on_change(move |checked, _, _| {
                    changes.borrow_mut().push(checked.to_string());
                }),
            )
            .into_any_element()
    });

    // Field y 0..36 — fill it so a spurious submission would have a record
    // to show; Switch track y 52..72, reached by Tab from the field.
    click(cx, 60., 18.);
    cx.simulate_input("ada");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["true"],
        "the Enter must have reached the focused switch and toggled it — \
         without this record the no-submit assertion would prove nothing"
    );
    assert!(
        submitted.borrow().is_empty(),
        "Enter released on a focused compound control must not submit the form"
    );
    assert!(
        invalid.borrow().is_empty(),
        "a compound control's Enter must run neither submission path"
    );
}

#[gpui::test]
fn form_enter_blocked_submission_defers_focus_so_the_release_cannot_activate(
    cx: &mut TestAppContext,
) {
    // The reproduction the inline focus move failed: a required text field
    // (filled) and a required empty Select, registered through
    // `Select::form_field`. Enter in the text field blocks on the select's
    // missing value, and the repair focuses the select trigger. gpui
    // activates a focused element's click listeners on key *release*, and
    // the release arrives after the focus moved, against a frame drawn with
    // the trigger focused — so an inline move opened the panel. The form
    // defers the move past the keystroke and disarms the release instead:
    // the trigger ends up focused with the panel still closed, which the
    // final Down proves (it is the closed trigger's own open key).
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let opens = events();
    let opened = opens.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let opens = opens.clone();
        let select = Select::new(
            "fe-defer-select",
            vec![
                PickerItem::new("typst", "Typst"),
                PickerItem::new("rust", "Rust"),
                PickerItem::new("go", "Go"),
            ],
        )
        .name("tool")
        .is_required(true)
        .on_open_change(move |open, _, _| {
            opens.borrow_mut().push(format!("open:{open}"));
        });
        let select_field = select.form_field().expect("named select field");
        let form = Form::new()
            .field(FormField::text(state_for_view.clone()).is_required(true))
            .field(select_field)
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["tool"]));
            });
        form.child(Input::new(state_for_view.clone()).name("name"))
            .child(select)
            .into_any_element()
    });

    // Field y 0..36, select trigger y 52..88. Fill the field so the ONLY
    // blocker is the select, then Enter in the field.
    click(cx, 60., 18.);
    cx.simulate_input("ada");
    press(cx, "enter");
    assert_eq!(
        invalid.borrow().as_slice(),
        ["tool"],
        "the blocked Enter must run the invalid path with the select's missing \
         name"
    );
    assert!(
        submitted.borrow().is_empty(),
        "a blocked Enter must never reach on_submit"
    );
    assert!(
        opened.borrow().is_empty(),
        "the release that follows a blocked Enter must not click the newly \
         focused select trigger open — this is the activation the deferred \
         focus exists to prevent"
    );

    // The Down probe: the closed select trigger answers Down by opening, and
    // an input answers it with nothing. An open record here proves the focus
    // really moved to the trigger even though the release clicked nothing.
    press(cx, "down");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the first invalid field must hold the focus after the blocked Enter \
         (v3: 'By default, the first invalid field will be focused'), and its \
         panel must still be closed"
    );
}

#[gpui::test]
fn form_field_with_own_on_submit_suppresses_implicit_submission(cx: &mut TestAppContext) {
    // A field with its own `onSubmit` owns Enter the way a native input with
    // a keydown handler for the key stops the form's implicit submission:
    // the callback fires and the keystroke is stopped from bubbling, so the
    // form stays silent. Without the stop the same Enter would both call the
    // field's callback and run the form's submission — two actions, one key.
    let own_submits = events();
    let own = own_submits.clone();
    let form_submits = events();
    let submitted = form_submits.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let own = own_submits.clone();
        let submitted = form_submits.clone();
        let form = Form::new()
            .field(FormField::text(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(record_data(data));
            });
        form.child(Input::new(state_for_view.clone()).name("name").on_submit(
            move |value: &str, _, _| {
                own.borrow_mut().push(value.to_owned());
            },
        ))
        .into_any_element()
    });

    // Field y 0..36. Two Enters: each fires the field's own callback once
    // and the form never submits.
    click(cx, 60., 18.);
    cx.simulate_input("ada");
    press(cx, "enter");
    press(cx, "enter");
    assert_eq!(
        own.borrow().as_slice(),
        ["ada", "ada"],
        "Enter must reach the field's own onSubmit on every press"
    );
    assert!(
        submitted.borrow().is_empty(),
        "a field that owns Enter must stop it from also submitting the form"
    );
}

#[gpui::test]
fn form_search_field_without_on_submit_bubbles_to_implicit_submission(cx: &mut TestAppContext) {
    // The other half of the same contract: a SearchField with no `onSubmit`
    // answers Enter with nothing of its own, so the keystroke still bubbles
    // and the form submits — a plain field's bubbling Enter is how it
    // submits its form.
    let form_submits = events();
    let submitted = form_submits.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submitted = form_submits.clone();
        let form = Form::new()
            .field(FormField::text(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(record_data(data));
            });
        form.child(SearchField::new(state_for_view.clone()).name("query"))
            .into_any_element()
    });

    // The search field's group is a 36px row at the window origin.
    click(cx, 60., 18.);
    cx.simulate_input("ada");
    press(cx, "enter");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["query=ada"],
        "a SearchField without onSubmit must let Enter bubble into the form's \
         implicit submission"
    );
}

#[gpui::test]
fn form_combo_box_open_enter_selects_and_closed_enter_submits(cx: &mut TestAppContext) {
    // While the suggestion list is open, Enter belongs to the list: with a
    // cursor row it picks the row and closes, and the same keystroke must
    // not also run the form's submission. Closed, the ComboBox answers Enter
    // with nothing — the query already matches the selection — so the
    // keystroke bubbles and the form submits.
    let form_submits = events();
    let submitted = form_submits.clone();
    let picks = events();
    let picked = picks.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submitted = form_submits.clone();
        let picked = picks.clone();
        let combo = ComboBox::new(state_for_view.clone(), keyed(&["Typst", "Rust", "Go"]))
            .name("tool")
            .on_change(move |item, _, _| picked.borrow_mut().push(item.to_string()));
        // `ComboBox::form_field` carries the name, which the raw text state
        // alone does not.
        let combo_field = combo.form_field().expect("named combo field");
        let form = Form::new()
            .field(combo_field)
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(record_data(data));
            });
        form.child(combo).into_any_element()
    });

    // The field is a 36px row at the origin (centre (60, 18)); the default
    // Focus trigger opens the list on the click, Down puts the cursor on row
    // 0, and Enter picks it.
    click(cx, 60., 18.);
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Typst"],
        "Enter on an open list must pick the cursor row"
    );
    assert!(
        submitted.borrow().is_empty(),
        "an Enter the open list consumed must not also submit the form"
    );

    // Closed again, the query is the selection, so the ComboBox has nothing
    // to answer Enter with: the keystroke bubbles and the form submits with
    // the picked value.
    press(cx, "enter");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["tool=Typst"],
        "a closed ComboBox must let Enter bubble into the form's implicit \
         submission"
    );
    assert_eq!(
        picked.borrow().as_slice(),
        ["Typst"],
        "the closed Enter must not pick anything further"
    );
}

#[gpui::test]
fn form_number_field_resolved_validity_blocks_and_enter_submits(cx: &mut TestAppContext) {
    // v3's NumberField runs the same validation the other fields do: a
    // `validate` failure resolves into the field's validity — mirrored onto
    // the inner InputState by `NumberField::render` — and under `native` it
    // blocks, with the failed field focused. `<input type=number>` is a
    // single-line text control, so a valid Enter submits from it.
    let form_submits = events();
    let submitted = form_submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| NumberState::new(cx, 5.));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let submitted = form_submits.clone();
        let invalid = invalids.clone();
        let form = Form::new()
            .field(FormField::number(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalid.borrow_mut().push(record_missing(data, &["amount"]));
            });
        form.child(
            NumberField::new(state_for_view.clone())
                .name("amount")
                .validate(|value: &f64| (*value < 18.).then(|| "Must be 18 or more".into())),
        )
        .into_any_element()
    });

    // The group is 220px wide and 36px tall; click into the input area
    // (clear of the 40px stepper cells) and press Enter on the failing 5.
    click(cx, 110., 18.);
    press(cx, "enter");
    assert!(
        submitted.borrow().is_empty(),
        "a NumberField validate failure must block the native submit"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        [""],
        "a NumberField validate failure must route the submit to onInvalid"
    );
    let focused = cx.update(|window, cx| {
        state
            .read(cx)
            .input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
    });
    assert!(
        focused,
        "the invalid number field must receive the failed-submit focus"
    );

    // Fix the value and press Enter again: the resolved validity clears and
    // the same door submits.
    cx.update(|_, cx| {
        state.update(cx, |s, cx| {
            s.set_value(42., cx);
            cx.notify();
        });
    });
    press(cx, "enter");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["amount=42"],
        "Enter in a valid number field must submit the form"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        [""],
        "the valid Enter must not run the invalid path again"
    );
}

#[gpui::test]
fn form_read_only_field_is_successful_but_barred_from_validation(cx: &mut TestAppContext) {
    // Native gates, three ways: a read-only field stays successful — its
    // value submits — and focusable, but constraint validation bars it, so
    // neither `isRequired` emptiness nor `isInvalid` blocks. Lift the
    // read-only flag and the same form blocks on the very same field, which
    // is what ties the bar to the flag rather than to an accident of the
    // registration.
    let read_only = std::rc::Rc::new(std::cell::Cell::new(true));
    let read_only_for_view = read_only.clone();
    let form_submits = events();
    let submitted = form_submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submitted = form_submits.clone();
        let invalid = invalids.clone();
        let form = Form::new()
            .field(FormField::text(state_for_view.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalid.borrow_mut().push(record_missing(data, &["name"]));
            });
        let submit = form.submit_handler();
        form.child(
            Input::new(state_for_view.clone())
                .name("name")
                .is_read_only(read_only_for_view.get())
                .is_invalid(true)
                .error_message("Locked"),
        )
        .child(
            gpui::div().flex().gap(px(8.)).child(
                Button::new("fe-readonly-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            ),
        )
        .into_any_element()
    });

    // The invalid field renders its error line, so the field column is
    // 36px + gap(4) + a 16px message = 56px tall, and the md button sits at
    // y 72..108 — centre (60, 90).
    click(cx, 60., 90.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["name="],
        "a read-only field must stay successful: its value submits even when \
         empty and flagged invalid"
    );
    assert!(
        invalid.borrow().is_empty(),
        "constraint validation must bar a read-only field: neither its \
         required emptiness nor its stored error may block"
    );

    // Lift read-only: the same empty, still-invalid field now blocks.
    read_only.set(false);
    cx.update(|window, _| window.refresh());
    click(cx, 60., 90.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["name="],
        "the lifted flag must let validation block again — no second submit"
    );
    assert_eq!(
        invalid.borrow().as_slice(),
        ["name"],
        "the same field without read-only must route the submit to onInvalid \
         with its missing name"
    );
}

#[gpui::test]
fn form_otp_enter_participates_in_implicit_submission(cx: &mut TestAppContext) {
    // Pinned v3 builds InputOTP on a single text input, and its cells share
    // one focus handle: a focused Enter participates exactly like a
    // single-line field's — blocked while the required code is missing, and
    // submitting once it is filled.
    let form_submits = events();
    let submitted = form_submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state_for_view = cx.new(|cx| OtpState::with_length(cx, 4));
    let cx = open_host(cx, move || {
        let submitted = form_submits.clone();
        let invalid = invalids.clone();
        let form = Form::new()
            .field(FormField::code("code", state_for_view.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalid.borrow_mut().push(record_missing(data, &["code"]));
            });
        form.child(InputOTP::new(state_for_view.clone()))
            .into_any_element()
    });

    // Cell 0 spans x 0..38 in the 40px row: centre (19, 20). The empty
    // required code blocks, and the OTP is the invalid field that holds the
    // focus already.
    click(cx, 19., 20.);
    press(cx, "enter");
    assert_eq!(
        invalid.borrow().as_slice(),
        ["code"],
        "a focused Enter on an empty required OTP must run the invalid path"
    );
    assert!(
        submitted.borrow().is_empty(),
        "the blocked OTP Enter must not submit"
    );

    // Fill the code and press Enter again: the same door submits.
    press(cx, "1");
    press(cx, "2");
    press(cx, "3");
    press(cx, "4");
    press(cx, "enter");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["code=1234"],
        "a focused Enter on a filled OTP must submit the form"
    );
}

#[gpui::test]
fn form_enter_modifiers_match_the_rendered_control(cx: &mut TestAppContext) {
    // The rendered single-line control submits on plain and shift+enter —
    // its own Enter branch only refuses ctrl/alt/platform chords — so the
    // form's implicit submission must match: shift+enter submits, a chord
    // does nothing at all, and the field's value is untouched by any of
    // them.
    let form_submits = events();
    let submitted = form_submits.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submitted = form_submits.clone();
        let form = Form::new()
            .field(FormField::text(state_for_view.clone()))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(record_data(data));
            });
        form.child(Input::new(state_for_view.clone()).name("name"))
            .into_any_element()
    });

    // Field y 0..36.
    click(cx, 60., 18.);
    cx.simulate_input("ada");
    press(cx, "ctrl-enter");
    press(cx, "alt-enter");
    press(cx, "cmd-enter");
    assert!(
        submitted.borrow().is_empty(),
        "a ctrl/alt/platform-modified Enter must neither submit the form nor \
         reach the field's own Enter branch"
    );
    press(cx, "shift-enter");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["name=ada"],
        "shift+enter must submit, matching the rendered single-line control"
    );
}

// ---------------------------------------------------------------------------
// InputOTP
// ---------------------------------------------------------------------------

#[gpui::test]
fn input_otp_autofocus_focuses_and_accepts_input_without_a_click(cx: &mut TestAppContext) {
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        InputOTP::new(state_for_view.clone())
            .auto_focus(true)
            .into_any_element()
    });

    let focused = cx.update(|window, cx| state.read(cx).focus_handle(cx).is_focused(window));
    assert!(focused, "autoFocus must focus the first OTP slot on mount");

    press(cx, "1");
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        code, "1",
        "typing must reach an autofocused OTP without a pointer press"
    );
}

#[gpui::test]
fn input_otp_autofocus_is_a_no_op_when_disabled(cx: &mut TestAppContext) {
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        InputOTP::new(state_for_view.clone())
            .auto_focus(true)
            .is_disabled(true)
            .into_any_element()
    });

    let focused = cx.update(|window, cx| state.read(cx).focus_handle(cx).is_focused(window));
    assert!(!focused, "autoFocus must not move focus to a disabled OTP");
    press(cx, "1");
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(code, "", "a disabled OTP must still refuse keystrokes");
}

#[gpui::test]
fn input_otp_disabled_autofocus_does_not_rerun_when_enabled(cx: &mut TestAppContext) {
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let disabled = std::rc::Rc::new(std::cell::Cell::new(true));
    let disabled_for_view = disabled.clone();
    let cx = open_host(cx, move || {
        InputOTP::new(state_for_view.clone())
            .auto_focus(true)
            .is_disabled(disabled_for_view.get())
            .into_any_element()
    });

    disabled.set(false);
    cx.update(|window, _| window.refresh());
    let focused = cx.update(|window, cx| state.read(cx).focus_handle(cx).is_focused(window));
    assert!(
        !focused,
        "autofocus is a mount-time decision and must not rerun when a disabled OTP is enabled"
    );
}

#[gpui::test]
fn input_otp_partial_seed_completes_once_and_again_after_correction(cx: &mut TestAppContext) {
    // A caller-seeded partial value (v3's `value="12"` controlled seed) must
    // leave the next digit for the first empty slot — `set_code` pads with
    // blanks and parks the cursor at the code's end (input_otp.rs), so typing
    // continues at cell 2. `onComplete` (v3: "Handler called when all slots
    // are filled") must fire exactly once for the completion, and again only
    // after the code is broken and re-completed. The second "1234" is the
    // correction: backspace removes the last digit, typing it back completes
    // the field a second time.
    let changes = events();
    let recorded = changes.clone();
    let completes = events();
    let completed = completes.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    cx.update(|cx| state.update(cx, |s, _| s.set_code("12")));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let completes = completes.clone();
        InputOTP::new(state_for_view.clone())
            .on_change(move |code, _, _| changes.borrow_mut().push(code.to_owned()))
            .on_complete(move |code, _, _| completes.borrow_mut().push(code.to_owned()))
            .into_any_element()
    });

    // Cell 0 centre (19, 20) focuses the row; the seeded cursor is already at
    // cell 2, so the first pressed digit lands there.
    click(cx, 19., 20.);
    press(cx, "3");
    press(cx, "4");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["123", "1234"],
        "typing from a seeded '12' must grow the code through the empty slots"
    );
    assert_eq!(
        completed.borrow().as_slice(),
        ["1234"],
        "on_complete must fire exactly once when the seeded code is completed"
    );

    // The correction: clear the last slot, refill it — on_complete again.
    press(cx, "backspace");
    press(cx, "4");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["123", "1234", "123", "1234"],
        "backspace then a digit must each report the assembled code"
    );
    assert_eq!(
        completed.borrow().as_slice(),
        ["1234", "1234"],
        "re-completing the code after a correction must fire on_complete again"
    );
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(code, "1234", "the OtpState must hold the corrected code");
}

#[gpui::test]
fn input_otp_seed_longer_than_slots_truncates(cx: &mut TestAppContext) {
    // Seeding more characters than there are cells must drop the overflow
    // (`set_code` reads one char per cell), and the cursor must rest on the
    // last cell so the next keystroke overwrites rather than panicking off
    // the end. `onComplete` must not fire from the seed itself — it is an
    // event, not a state assertion — but must fire on the user's next edit.
    let completes = events();
    let completed = completes.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    cx.update(|cx| state.update(cx, |s, _| s.set_code("1234567")));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let completes = completes.clone();
        InputOTP::new(state_for_view.clone())
            .on_complete(move |code, _, _| completes.borrow_mut().push(code.to_owned()))
            .into_any_element()
    });

    let seeded = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        seeded, "1234",
        "seeding 7 chars into 4 cells must keep only the first 4"
    );

    click(cx, 19., 20.);
    press(cx, "5");
    assert_eq!(
        completed.borrow().as_slice(),
        ["1235"],
        "on_complete must fire only on the user's keystroke, never from the seed"
    );
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        code, "1235",
        "the keystroke after an over-long seed must overwrite the last cell"
    );
}

#[gpui::test]
fn input_otp_arrows_clamp_at_both_ends(cx: &mut TestAppContext) {
    // The arrows move the caret within the cells (`left`/`right` in
    // input_otp.rs); both ends clamp — Left from cell 0 stays, Right from the
    // last cell stays. Caret position is private to `OtpState`, so the probe
    // is where the next digit lands: a clamped caret writes to the cell it
    // could not leave, and a wrap would write somewhere else.
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        InputOTP::new(state_for_view.clone())
            .on_change(move |code, _, _| changes.borrow_mut().push(code.to_owned()))
            .into_any_element()
    });

    // Click cell 0 (19, 20); Left from the first cell clamps, so the "1"
    // lands in cell 0 and the caret steps to cell 1.
    click(cx, 19., 20.);
    press(cx, "left");
    press(cx, "1");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1"],
        "Left from cell 0 must not move: the '1' still landed in cell 0"
    );

    // Right to cell 2; "2" lands there — cells are now 1, _, 2, _. Then
    // Right twice from cell 3 (the caret reached it by typing): the first
    // clamps (cursor+1 == len) and the second cannot overrun at all. A "3"
    // typed from the clamped caret overwrites cell 3 in place — if either
    // Right had left the last cell the digit would land elsewhere, and if it
    // had walked past the end the index would panic.
    press(cx, "right");
    press(cx, "2");
    press(cx, "right");
    press(cx, "right");
    press(cx, "3");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1", "12", "123"],
        "Right must step within the cells and clamp at the last one: the '3' \
         overwrote the final cell instead of falling off the row"
    );

    // Left four times from the last cell walks to 0 and clamps on the fourth
    // press; a "5" typed from there overwrites cell 0 — if Left had wrapped
    // past the start, the digit would land at the other end instead.
    press(cx, "left");
    press(cx, "left");
    press(cx, "left");
    press(cx, "left");
    press(cx, "5");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1", "12", "123", "523"],
        "Left must walk to cell 0 and clamp there: the '5' overwrote cell 0"
    );
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(code, "523", "the OtpState must hold the clamped arithmetic");
}

#[gpui::test]
fn input_otp_paste_transformer_runs_before_fill(cx: &mut TestAppContext) {
    // v3 documents `pasteTransformer` as the hook that rewrites pasted text
    // ("Transform pasted text (e.g., remove hyphens)"). The port applies it
    // before the slots fill (input_otp.rs), so a transformer that maps "1" to
    // "A" must be visible in the code — the transformer ran, not the raw
    // clipboard. The Alphanumeric pattern admits the result, and a full paste
    // fires onComplete exactly as a typed code does.
    let completes = events();
    let completed = completes.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let completes = completes.clone();
        let changes = changes.clone();
        InputOTP::new(state_for_view.clone())
            .pattern(OtpPattern::Alphanumeric)
            .paste_transformer(|text| text.replace('1', "A"))
            .on_change(move |code, _, _| changes.borrow_mut().push(code.to_owned()))
            .on_complete(move |code, _, _| completes.borrow_mut().push(code.to_owned()))
            .into_any_element()
    });

    // The test platform's clipboard is an in-memory slot the test writes
    // (TestAppContext::write_to_clipboard, gpui test_context.rs), and the
    // component reads the same slot on Ctrl+V.
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("1234".to_owned()));
    click(cx, 19., 20.);
    press(cx, "ctrl-v");
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        code, "A234",
        "the paste transformer must rewrite the clipboard text before the \
         slots fill"
    );
    assert_eq!(
        changed.borrow().as_slice(),
        ["A234"],
        "a valid transformed paste must fire on_change exactly once"
    );
    assert_eq!(
        completed.borrow().as_slice(),
        ["A234"],
        "a paste that fills every slot must fire on_complete with the \
         transformed code"
    );
}

/// Pinned `input-otp` reports an accepted paste through `onChange`, then fires
/// `onComplete` only when that paste transitions the field from partial to
/// full. Empty/rejected pastes do neither, and an accepted same-value paste on
/// an already-complete field still reports the value without completing twice.
#[gpui::test]
fn input_otp_paste_callbacks_follow_acceptance_and_completion_transitions(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changed = for_view.clone();
        let completed = for_view.clone();
        InputOTP::new(state_for_view.clone())
            .pattern(OtpPattern::Digits)
            .on_change(move |code, _, _| changed.borrow_mut().push(format!("change:{code}")))
            .on_complete(move |code, _, _| {
                completed.borrow_mut().push(format!("complete:{code}"));
            })
            .into_any_element()
    });

    click(cx, 19., 20.);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("1".to_owned()));
    press(cx, "ctrl-v");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:1"],
        "a partial paste changes the value but must not complete"
    );

    recorded.borrow_mut().clear();
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("letters".to_owned()));
    press(cx, "ctrl-v");
    assert!(
        recorded.borrow().is_empty(),
        "a paste with no accepted characters must report neither callback"
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).code()),
        "1",
        "a paste rejected by the explicit digits pattern must leave the value unchanged"
    );

    cx.update(|_, cx| state.update(cx, |state, _| state.set_code("1234")));
    recorded.borrow_mut().clear();
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("4".to_owned()));
    press(cx, "ctrl-v");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:1234"],
        "an accepted same-value paste reports on_change but does not complete twice"
    );

    cx.update(|_, cx| state.update(cx, |state, _| state.set_code("123")));
    recorded.borrow_mut().clear();
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("4".to_owned()));
    press(cx, "ctrl-v");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:1234", "complete:1234"],
        "the transition to full must report change before completion"
    );
}

/// `slot` is a GPUI render extension that receives the slot index and current
/// character. The closure must receive each slot independently and update
/// after editing rather than remaining a construction-time snapshot.
#[gpui::test]
fn input_otp_slot_content_tracks_each_character(cx: &mut TestAppContext) {
    let snapshots = events();
    let snapshots_for_view = snapshots.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 2));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let snapshots = snapshots_for_view.clone();
        InputOTP::new(state_for_view.clone())
            .slot(move |index, value| {
                snapshots
                    .borrow_mut()
                    .push(format!("{index}:{}", value.unwrap_or('_')));
                gpui::div()
                    .child(value.unwrap_or('_').to_string())
                    .into_any_element()
            })
            .into_any_element()
    });

    assert!(
        snapshots.borrow().ends_with(&["0:_".into(), "1:_".into()]),
        "the initial frame must identify both empty slots"
    );

    click(cx, 19., 20.);
    press(cx, "7");
    assert!(
        snapshots.borrow().ends_with(&["0:7".into(), "1:_".into()]),
        "editing the first slot must update only that slot's render value"
    );
}

#[gpui::test]
fn input_otp_paste_subjects_to_pattern(cx: &mut TestAppContext) {
    // v3's `pattern` prop: "Regex pattern for allowed characters". The paste
    // branch of the port skips non-alphanumeric characters and uppercases the
    // rest, but never consults `pattern`, so a digits-only field ("1234" in
    // the clipboard) accepts "12ab34" as "12AB" — the letters should have
    // been dropped and the digits "34" should have filled the remaining
    // cells, leaving "1234". The paste path is the only one that can land a
    // letter in a digits field: the keystroke path checks `pattern.accepts`.
    let completes = events();
    let completed = completes.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let completes = completes.clone();
        InputOTP::new(state_for_view.clone())
            .on_complete(move |code, _, _| completes.borrow_mut().push(code.to_owned()))
            .into_any_element()
    });

    cx.write_to_clipboard(gpui::ClipboardItem::new_string("12ab34".to_owned()));
    click(cx, 19., 20.);
    press(cx, "ctrl-v");
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        code, "1234",
        "pasting into a digits field must drop the letters and fill the \
         digits"
    );
    assert_eq!(
        completed.borrow().as_slice(),
        ["1234"],
        "the completed code must be the pattern-filtered one"
    );
}

#[gpui::test]
fn input_otp_backspace_on_empty_first_cell_fires_no_change(cx: &mut TestAppContext) {
    // An empty field, cursor on cell 0, presses Backspace. Nothing can be
    // cleared; v3's `onChange` is "Handler called when the value changes", so
    // no change event may fire. The port's backspace branch (input_otp.rs)
    // always reports after the update, even when the update was a no-op.
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        InputOTP::new(state_for_view.clone())
            .on_change(move |code, _, _| changes.borrow_mut().push(code.to_owned()))
            .into_any_element()
    });

    click(cx, 19., 20.);
    press(cx, "backspace");
    assert!(
        recorded.borrow().is_empty(),
        "a backspace that clears nothing must not report a change — the \
         handler fired with the unchanged empty code instead"
    );
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(code, "", "the empty code must stay empty");
}

#[gpui::test]
fn input_otp_click_moves_the_caret(cx: &mut TestAppContext) {
    // A full code, then a click on cell 0 and one digit: the caret must have
    // moved to the clicked slot, so the digit overwrites cell 0 ("9234").
    // The port's `on_mouse_down` calls `window.focus` only and leaves the
    // cursor at the last cell, so the digit overwrites cell 3 instead
    // ("1239"). The code string discriminates: both writes are 'complete'
    // length-4 codes, only the caret position differs.
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        InputOTP::new(state_for_view.clone()).into_any_element()
    });

    click(cx, 19., 20.);
    press(cx, "1");
    press(cx, "2");
    press(cx, "3");
    press(cx, "4");
    // Cell 0's centre again — this click must re-home the caret.
    click(cx, 19., 20.);
    press(cx, "9");
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        code, "9234",
        "clicking a slot must move the caret there so the next digit \
         overwrites that cell — the digit landed on the last cell instead"
    );
}

#[gpui::test]
fn input_otp_disabled_click_does_not_focus(cx: &mut TestAppContext) {
    // v3's `isDisabled` ("Whether the input is disabled") means the control
    // answers no pointer: a click must neither focus it nor start the caret,
    // and the keys must change nothing. The disabled pointer handler returns
    // before asking the window to focus its inert handle.
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        InputOTP::new(state_for_view.clone())
            .is_disabled(true)
            .into_any_element()
    });

    click(cx, 19., 20.);
    let focused = cx.update(|window, cx| {
        let handle = state.read(cx).focus_handle(cx);
        handle.is_focused(window)
    });
    assert!(
        !focused,
        "a click on a disabled OTP must not focus it (pointer-events-none)"
    );
    press(cx, "1");
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(code, "", "a disabled OTP must refuse the keystroke too");
}

// ---------------------------------------------------------------------------
// Fieldset
// ---------------------------------------------------------------------------

#[gpui::test]
fn fieldset_group_spaces_consecutive_fields_16px(cx: &mut TestAppContext) {
    // `.fieldset__field_group` is `w-full space-y-4` in v3.2.4's fieldset.css:
    // `--spacing` × 4 = 16px between consecutive fields and nothing above the
    // first. The rendered stack is a flex column whose gap must be that 16px.
    let cx = open_host(cx, || {
        FieldGroup::new()
            .child(
                gpui::div()
                    .size(px(20.))
                    .debug_selector(move || "field-group-first".to_owned()),
            )
            .child(
                gpui::div()
                    .size(px(20.))
                    .debug_selector(move || "field-group-second".to_owned()),
            )
            .into_any_element()
    });

    let first = cx
        .debug_bounds("field-group-first")
        .expect("the first field probe must paint");
    let second = cx
        .debug_bounds("field-group-second")
        .expect("the second field probe must paint");
    assert_eq!(
        first.origin.y,
        px(0.),
        "space-y puts no margin above the first field"
    );
    assert_eq!(
        second.origin.y,
        px(36.),
        "consecutive fields sit 20px + the pinned 16px space-y-4 apart"
    );
}

#[gpui::test]
fn fieldset_parts_compose_and_controls_stay_interactive(cx: &mut TestAppContext) {
    // v3's `Fieldset.Legend`/`.Group`/`.Actions` compose (the docs' Basic
    // anatomy: Legend, Description, Group of fields, Actions) and the controls
    // inside keep answering — a container imposes nothing. One input, one
    // switch and one button, each with its own recorder: a click must reach
    // exactly its own control, which also proves the parts above them (the
    // legend, the description) do not swallow the hits.
    let changes = events();
    let recorded = changes.clone();
    let toggles = events();
    let toggled = toggles.clone();
    let presses = events();
    let pressed = presses.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let toggles = toggles.clone();
        let presses = presses.clone();
        Fieldset::new()
            .child(FieldsetLegend::new("Profile settings"))
            .child(Description::new("Update your profile information."))
            .child(
                FieldGroup::new()
                    .child(
                        Input::new(state_for_view.clone()).on_change(move |text, _, _| {
                            changes.borrow_mut().push(text.to_owned());
                        }),
                    )
                    .child(Switch::new("fs-switch").on_change(move |checked, _, _| {
                        toggles.borrow_mut().push(format!("change:{checked}"));
                    })),
            )
            .child(
                FieldsetActions::new().child(
                    Button::new("fs-save")
                        .label("Save")
                        .on_press(move |_, _, _| presses.borrow_mut().push("save".to_owned())),
                ),
            )
            .into_any_element()
    });

    // Legend 24px, gap 24, Description 16px, gap 24 → the group starts at
    // y 88: input y 88..124 (centre 60, 106), switch y 140..160 (track 40x20,
    // centre 20, 150), group height 72 → Actions y 184, md button 188..224
    // (centre 30, 206 — the 4px pt-1 sits above it). All derived from
    // field.rs's gaps and the component heights above.
    click(cx, 60., 106.);
    cx.simulate_input("ada");
    click(cx, 20., 150.);
    click(cx, 30., 206.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a", "ad", "ada"],
        "the input inside the composed fieldset must answer typing"
    );
    assert_eq!(
        toggled.borrow().as_slice(),
        ["change:true"],
        "the switch inside the group must answer its own click"
    );
    assert_eq!(
        pressed.borrow().as_slice(),
        ["save"],
        "the button inside Actions must answer its own click"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "ada", "the InputState must hold the typed value");
}

#[gpui::test]
fn fieldset_nested_fieldset_keeps_inner_controls_alive(cx: &mut TestAppContext) {
    // A Fieldset nested in a Fieldset (native HTML allows it) must not break
    // the controls inside either level: both fields keep their own state and
    // their own recorder, so a click that lands in the inner field's band
    // must not leak to the outer one (or vice versa).
    let outer_changes = events();
    let outer_recorded = outer_changes.clone();
    let inner_changes = events();
    let inner_recorded = inner_changes.clone();
    let outer_state = cx.new(|cx| InputState::new(cx));
    let inner_state = cx.new(|cx| InputState::new(cx));
    let outer_for_view = outer_state.clone();
    let inner_for_view = inner_state.clone();
    let cx = open_host(cx, move || {
        let outer_changes = outer_changes.clone();
        let inner_changes = inner_changes.clone();
        Fieldset::new()
            .child(FieldsetLegend::new("Outer"))
            .child(
                FieldGroup::new().child(Input::new(outer_for_view.clone()).on_change(
                    move |text, _, _| {
                        outer_changes.borrow_mut().push(text.to_owned());
                    },
                )),
            )
            .child(Fieldset::new().child(FieldsetLegend::new("Inner")).child(
                FieldGroup::new().child(Input::new(inner_for_view.clone()).on_change(
                    move |text, _, _| {
                        inner_changes.borrow_mut().push(text.to_owned());
                    },
                )),
            ))
            .into_any_element()
    });

    // Outer legend 0..24, gap 24 → outer group y 48: outer input 48..84
    // (centre 60, 66). The outer group ends at y 84, gap 24 → inner fieldset
    // y 108: inner legend 108..132, gap 24 → inner group y 156: inner input
    // 156..192 (centre 60, 174).
    click(cx, 60., 66.);
    cx.simulate_input("outer");
    click(cx, 60., 174.);
    cx.simulate_input("inner");
    assert_eq!(
        outer_recorded.borrow().as_slice(),
        ["o", "ou", "out", "oute", "outer"],
        "the outer field must keep typing through the nested fieldset"
    );
    assert_eq!(
        inner_recorded.borrow().as_slice(),
        ["i", "in", "inn", "inne", "inner"],
        "the inner field must keep typing inside the nested fieldset"
    );
    let outer_value = cx.update(|_, cx| outer_state.read(cx).value().to_owned());
    let inner_value = cx.update(|_, cx| inner_state.read(cx).value().to_owned());
    assert_eq!(
        outer_value, "outer",
        "the outer InputState must hold its text"
    );
    assert_eq!(
        inner_value, "inner",
        "the inner InputState must hold its text"
    );
}

#[gpui::test]
fn fieldset_actions_submit_drives_the_forms_submit(cx: &mut TestAppContext) {
    // v3's Basic Fieldset wraps the whole group in a Form and puts its
    // submit button in `Fieldset.Actions`; the button must drive the form's
    // submit handler — the collection crosses the form → fieldset → actions
    // boundary and the named value lands on onSubmit.
    let submits = events();
    let submitted = submits.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let form = Form::new()
            .field(FormField::text(state.clone()).is_required(true))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            });
        let submit = form.submit_handler();
        form.child(
            Fieldset::new()
                .child(FieldsetLegend::new("Billing address"))
                .child(FieldGroup::new().child(Input::new(state.clone()).name("street")))
                .child(
                    FieldsetActions::new().child(
                        Button::new("fda-submit")
                            .label("Save")
                            .on_press(move |_, window, cx| submit(window, cx)),
                    ),
                ),
        )
        .into_any_element()
    });

    // Legend 0..24, gap 24 → group y 48: input 48..84 (centre 60, 66);
    // group ends at y 84, gap 24 → Actions y 108: md button 112..148
    // (centre 30, 130 — the 4px pt-1 sits above it).
    click(cx, 60., 66.);
    cx.simulate_input("5th Ave");
    click(cx, 30., 130.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["street=5th Ave"],
        "the button inside Fieldset.Actions must drive the Form's onSubmit \
         with the named field"
    );
}

#[gpui::test]
fn fieldset_actions_reset_drives_the_forms_reset(cx: &mut TestAppContext) {
    // The reset half of the same anatomy: a `type="reset"` button inside
    // `Fieldset.Actions` restores the declared default through the Form and
    // fires onReset — the composition must not swallow either.
    let resets = events();
    let recorded_resets = resets.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let state_for_reset = state.clone();
    let cx = open_host(cx, move || {
        let resets = resets.clone();
        // A fresh clone per render: the `on_reset` closure below captures it
        // by move, and the form is rebuilt every frame.
        let state_for_reset = state_for_reset.clone();
        let form = Form::new()
            .field(
                FormField::text(state_for_view.clone())
                    .is_required(true)
                    .default_text(state_for_view.clone(), "Main St"),
            )
            .on_reset(move |_, cx| {
                let v = state_for_reset.read(cx).value().to_owned();
                resets.borrow_mut().push(format!("v={v}"));
            });
        let reset = form.reset_handler();
        form.child(
            Fieldset::new()
                .child(FieldsetLegend::new("Billing address"))
                .child(FieldGroup::new().child(Input::new(state_for_view.clone()).name("street")))
                .child(
                    FieldsetActions::new().child(
                        Button::new("fda-reset")
                            .label("Cancel")
                            .on_press(move |_, window, cx| reset(window, cx)),
                    ),
                ),
        )
        .into_any_element()
    });

    click(cx, 60., 66.);
    cx.simulate_input("5th Ave");
    click(cx, 30., 130.);
    assert_eq!(
        recorded_resets.borrow().as_slice(),
        ["v=Main St"],
        "the reset button inside Actions must restore the declared default and \
         report it through onReset"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        value, "Main St",
        "the InputState must hold the restored value"
    );
}

// ---------------------------------------------------------------------------
// Select
// ---------------------------------------------------------------------------

fn select_cities() -> Vec<PickerItem> {
    vec![
        PickerItem::new("Alpha", "Alpha"),
        PickerItem::new("Beta", "Beta"),
        PickerItem::new("Gamma", "Gamma"),
    ]
}

fn submit_select(data: &FormData, name: &str) -> String {
    data.get(name)
        .map_or_else(|| "omitted".to_owned(), |value| value.as_text().to_string())
}

/// Advances the test clock past `anim::EXITING_MS` (100ms) plus slack and
/// forces the repaint the exit timer only scheduled. A submit/reset click must
/// not land on the exiting, still-mounted Select panel: it occludes the Form
/// buttons until the exit finishes (`panel.occlude` keeps hit-testing it).
fn let_select_exit_finish(cx: &mut gpui::VisualTestContext) {
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(
            herogpui_components::anim::EXITING_MS + 200,
        ));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

#[gpui::test]
fn select_form_field_reads_changed_uncontrolled_value(cx: &mut TestAppContext) {
    let submitted = events();
    let for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let select = Select::new("live-select-form", select_cities())
            .name("city")
            .default_value(Some("Alpha".into()));
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_select(data, "city"));
            });
        let submit = form.submit_handler();
        form.child(select)
            .child(
                Button::new("live-select-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    // Trigger centre (60, 18). Gamma is row 2 of the open list:
    // y = 66 + 2*36 = 138. After the pick the panel closes, so the Form's
    // 16px gap puts the md submit button at y 52..88. The closing panel still
    // occludes that button until its exit finishes, so let it finish first.
    click(cx, 60., 18.);
    click(cx, 60., 138.);
    let_select_exit_finish(cx);
    click(cx, 60., 70.);

    assert_eq!(
        submitted.borrow().as_slice(),
        ["Gamma"],
        "FormField must read the keyed uncontrolled selection after it changes"
    );
}

#[gpui::test]
fn disabled_select_is_not_a_successful_form_control(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let select = Select::new("disabled-select-snapshot", select_cities())
            .name("city")
            .default_value(Some("Alpha".into()))
            .is_required(true)
            .is_disabled(true);
        let form = Form::new().field(select.form_field().expect("named select field"));
        assert!(
            form.data(cx).get("city").is_none(),
            "disabled omission must be true before the first render as well as after it"
        );
    });

    let submitted = events();
    let invalids = events();
    let submitted_for_view = submitted.clone();
    let invalids_for_view = invalids.clone();
    let cx = open_host(cx, move || {
        let submitted = submitted_for_view.clone();
        let invalids = invalids_for_view.clone();
        let select = Select::new("disabled-select-form", select_cities())
            .name("city")
            .default_value(Some("Alpha".into()))
            .is_required(true)
            .is_disabled(true);
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_select(data, "city"));
            })
            .on_invalid(move |_, _, _| invalids.borrow_mut().push("invalid".to_owned()));
        let submit = form.submit_handler();
        form.child(select)
            .child(
                Button::new("disabled-select-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 70.);
    assert_eq!(submitted.borrow().as_slice(), ["omitted"]);
    assert!(
        invalids.borrow().is_empty(),
        "a disabled required Select must not block submission"
    );
}

#[gpui::test]
fn disabled_select_becomes_successful_after_rerender(cx: &mut TestAppContext) {
    let disabled = std::rc::Rc::new(std::cell::Cell::new(true));
    let disabled_for_view = disabled.clone();
    let submitted = events();
    let submitted_for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = submitted_for_view.clone();
        let select = Select::new("enabled-select-form", select_cities())
            .name("city")
            .default_value(Some("Alpha".into()))
            .is_disabled(disabled_for_view.get());
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_select(data, "city"));
            });
        let submit = form.submit_handler();
        form.child(select)
            .child(
                Button::new("enabled-select-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 70.);
    assert_eq!(submitted.borrow().as_slice(), ["omitted"]);

    disabled.set(false);
    cx.update(|window, _| window.refresh());
    click(cx, 60., 70.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["omitted", "Alpha"],
        "the live Select must become successful after its enabled rerender"
    );
}

#[gpui::test]
fn select_reset_restores_the_uncontrolled_default(cx: &mut TestAppContext) {
    let submitted = events();
    let submitted_for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = submitted_for_view.clone();
        let select = Select::new("reset-select-form", select_cities())
            .name("city")
            .default_value(Some("Alpha".into()));
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_select(data, "city"));
            });
        let submit = form.submit_handler();
        let reset = form.reset_handler();
        form.child(select)
            .child(
                Button::new("reset-select-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .child(
                Button::new("reset-select-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "end");
    press(cx, "enter");
    let_select_exit_finish(cx);
    click(cx, 60., 70.);
    click(cx, 60., 122.);
    click(cx, 60., 70.);

    assert_eq!(
        submitted.borrow().as_slice(),
        ["Gamma", "Alpha"],
        "native reset must restore defaultValue in rendered state and subsequent FormData"
    );
}

#[gpui::test]
fn controlled_select_form_reads_parent_value_only_after_acceptance(cx: &mut TestAppContext) {
    let selected = std::rc::Rc::new(std::cell::RefCell::new(Some(SharedString::from("Alpha"))));
    let accept = std::rc::Rc::new(std::cell::Cell::new(false));
    let submitted = events();
    let changes = events();
    let selected_for_view = selected;
    let accept_for_view = accept.clone();
    let submitted_for_view = submitted.clone();
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let submitted = submitted_for_view.clone();
        let changes = changes_for_view.clone();
        let selected = selected_for_view.clone();
        let accept = accept_for_view.clone();
        let current = selected.borrow().clone();
        let select = Select::new("controlled-select-form", select_cities())
            .name("city")
            .value(current)
            .on_change(move |next, _, _| {
                changes.borrow_mut().push(format!("{next:?}"));
                if accept.get() {
                    *selected.borrow_mut() = next.clone();
                }
            });
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_select(data, "city"));
            });
        let submit = form.submit_handler();
        form.child(select)
            .child(
                Button::new("controlled-select-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    click(cx, 60., 102.);
    let_select_exit_finish(cx);
    click(cx, 60., 70.);
    assert_eq!(changes.borrow().as_slice(), ["Some(\"Beta\")"]);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["Alpha"],
        "a controlled Select must keep submitting the owner's value until it is accepted"
    );

    accept.set(true);
    click(cx, 60., 18.);
    click(cx, 60., 102.);
    let_select_exit_finish(cx);
    click(cx, 60., 70.);
    assert_eq!(
        changes.borrow().as_slice(),
        ["Some(\"Beta\")", "Some(\"Beta\")"]
    );
    assert_eq!(
        submitted.borrow().as_slice(),
        ["Alpha", "Beta"],
        "once the owner writes the reported value back, FormField must submit it"
    );
}

#[gpui::test]
fn controlled_select_reset_reports_the_default_to_its_owner(cx: &mut TestAppContext) {
    let selected = std::rc::Rc::new(std::cell::RefCell::new(Some(SharedString::from("Beta"))));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let select = Select::new("controlled-reset-select", select_cities())
            .name("city")
            .value(selected.borrow().clone())
            .default_value(Some("Alpha".into()))
            .on_change({
                let selected = selected.clone();
                let changes = changes.clone();
                move |next, _, _| {
                    *selected.borrow_mut() = next.clone();
                    changes.borrow_mut().push(format!("change:{next:?}"));
                }
            });
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_submit({
                let changes = changes.clone();
                move |data: &FormData, _, _| {
                    changes
                        .borrow_mut()
                        .push(format!("submit:{}", submit_select(data, "city")));
                }
            });
        let reset = form.reset_handler();
        let submit = form.submit_handler();
        form.child(select)
            .child(
                Button::new("controlled-reset-select-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .child(
                Button::new("controlled-reset-select-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 122.);
    click(cx, 60., 70.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:Some(\"Alpha\")", "submit:Alpha"],
        "controlled reset must report defaultValue so the owner can update"
    );
}

#[gpui::test]
fn invalid_select_blocks_form_and_receives_focus(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let invalids = events.clone();
        let submits = events.clone();
        let opens = events.clone();
        let select = Select::new("invalid-select-form", select_cities())
            .name("city")
            .default_value(Some("Alpha".into()))
            .is_invalid(true)
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            });
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_invalid(move |_, _, _| invalids.borrow_mut().push("invalid".to_owned()))
            .on_submit(move |_, _, _| submits.borrow_mut().push("submit".to_owned()));
        let submit = form.submit_handler();
        form.child(select)
            .child(
                Button::new("invalid-select-submit")
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
        ["invalid", "open:true"],
        "native invalid submission must focus the Select trigger rather than submit"
    );
}

#[gpui::test]
fn required_empty_select_blocks_form_and_receives_focus(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let invalids = events.clone();
        let submits = events.clone();
        let opens = events.clone();
        let select = Select::new("required-select-form", select_cities())
            .name("city")
            .is_required(true)
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            });
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_missing(data, &["city"]));
            })
            .on_submit(move |_, _, _| submits.borrow_mut().push("submit".to_owned()));
        let submit = form.submit_handler();
        form.child(select)
            .child(
                Button::new("required-select-submit")
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
        ["city", "open:true"],
        "a required empty Select must block native submit and take the focus"
    );
}

#[gpui::test]
fn multiple_select_form_data_tracks_live_selected_values(cx: &mut TestAppContext) {
    let selected = std::rc::Rc::new(std::cell::RefCell::new(vec![SharedString::from("Alpha")]));
    let submitted = events();
    let selected_for_view = selected;
    let submitted_for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = submitted_for_view.clone();
        let selected = selected_for_view.clone();
        let current = selected.borrow().clone();
        let select = Select::new("multiple-select-form", select_cities())
            .name("cities")
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(current)
            .on_selection_change_all(move |next, _, _| {
                *selected.borrow_mut() = next.to_vec();
            });
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(
                    data.get_all("cities")
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            });
        let submit = form.submit_handler();
        form.child(select)
            .child(
                Button::new("multiple-select-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    // Multiple picks do not close the list; close it via the trigger before
    // the submit button at y=70 can be reached. The closing panel still
    // occludes that button until its exit finishes, so let it finish first.
    click(cx, 60., 18.);
    click(cx, 60., 102.);
    click(cx, 60., 18.);
    let_select_exit_finish(cx);
    click(cx, 60., 70.);

    assert_eq!(
        submitted.borrow().as_slice(),
        ["Alpha,Beta"],
        "a multiple Select must submit the live Keys after the owner accepts them"
    );
}

#[gpui::test]
fn uncontrolled_multiple_select_form_resets_to_its_default_values(cx: &mut TestAppContext) {
    let submitted = events();
    let submitted_for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = submitted_for_view.clone();
        let select = Select::new("default-multiple-select-form", select_cities())
            .name("cities")
            .selection_mode(SelectionMode::Multiple)
            .default_selected_keys(["Alpha".into(), "Gamma".into()]);
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(
                    data.get_all("cities")
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            });
        let submit = form.submit_handler();
        let reset = form.reset_handler();
        form.child(select)
            .child(
                Button::new("default-multiple-select-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .child(
                Button::new("default-multiple-select-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    // Add Beta to the default Alpha/Gamma set, close the list, and submit.
    // The closing panel still occludes the submit button until its exit
    // finishes, so let it finish first.
    click(cx, 60., 18.);
    click(cx, 60., 102.);
    click(cx, 60., 18.);
    let_select_exit_finish(cx);
    click(cx, 60., 70.);

    // Reset restores the initial array held by the uncontrolled Select.
    click(cx, 60., 122.);
    click(cx, 60., 70.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["Alpha,Beta,Gamma", "Alpha,Gamma"],
        "FormData must follow the live multiple selection and its uncontrolled reset"
    );
}

#[gpui::test]
fn controlled_multiple_select_form_waits_for_owner_acceptance(cx: &mut TestAppContext) {
    let changes = events();
    let submitted = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let selection_changes = changes.clone();
        let select = Select::new("rejected-multiple-select-form", select_cities())
            .name("cities")
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(["Alpha".into()])
            .on_selection_change_all(move |next, _, _| {
                selection_changes.borrow_mut().push(format!(
                    "change:{}",
                    next.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            });
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_submit(move |data: &FormData, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("submit:{}", submit_select(data, "cities")));
            });
        let submit = form.submit_handler();
        form.child(select)
            .child(
                Button::new("rejected-multiple-select-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    click(cx, 60., 102.);
    click(cx, 60., 18.);
    let_select_exit_finish(cx);
    click(cx, 60., 70.);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["change:Alpha,Beta", "submit:Alpha"],
        "a controlled proposal must not reach FormData until its owner accepts it"
    );
}

#[gpui::test]
fn disabled_multiple_select_is_omitted_from_submission(cx: &mut TestAppContext) {
    let submitted = events();
    let submitted_for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = submitted_for_view.clone();
        let select = Select::new("disabled-multiple-select", select_cities())
            .name("cities")
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(["Alpha".into(), "Beta".into()])
            .is_disabled(true);
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_select(data, "cities"));
            });
        let submit = form.submit_handler();
        form.child(select)
            .child(
                Button::new("disabled-multiple-select-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 70.);
    assert_eq!(submitted.borrow().as_slice(), ["omitted"]);
}

#[gpui::test]
fn controlled_multiple_select_reset_reports_the_default_to_its_owner(cx: &mut TestAppContext) {
    let selected = std::rc::Rc::new(std::cell::RefCell::new(vec![SharedString::from("Alpha")]));
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let current = selected.borrow().clone();
        let select = Select::new("controlled-reset-multiple-select", select_cities())
            .name("cities")
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(current)
            .on_selection_change_all({
                let selected = selected.clone();
                let changes = changes.clone();
                move |next, _, _| {
                    *selected.borrow_mut() = next.to_vec();
                    changes.borrow_mut().push(
                        next.iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                    );
                }
            });
        let form = Form::new()
            .field(select.form_field().expect("named select field"))
            .on_submit({
                let changes = changes.clone();
                move |data: &FormData, _, _| {
                    changes.borrow_mut().push(format!(
                        "submit:{}",
                        data.get_all("cities")
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    ));
                }
            });
        let reset = form.reset_handler();
        let submit = form.submit_handler();
        form.child(select)
            .child(
                Button::new("controlled-reset-multiple-submit")
                    .label("Submit")
                    .on_press(move |_, window, cx| submit(window, cx)),
            )
            .child(
                Button::new("controlled-reset-multiple-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 18.);
    click(cx, 60., 102.);
    click(cx, 60., 18.);
    let_select_exit_finish(cx);
    click(cx, 60., 122.);
    click(cx, 60., 70.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha,Beta", "Alpha", "submit:Alpha"],
        "controlled multiple reset must report the first-render selection so the owner can update"
    );
}

/// The pinned `formValue` contract (React Aria Components 1.20.0 defaults it
/// to `"key"`): a named ComboBox submits the picked item's *key*, while the
/// input shows the item's label. Keys and labels differ here, so the
/// submission tells them apart.
#[gpui::test]
fn form_combo_box_submits_the_selected_key_by_default(cx: &mut TestAppContext) {
    let form_submits = events();
    let submitted = form_submits.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submitted = form_submits.clone();
        let combo = ComboBox::new(
            state_for_view.clone(),
            vec![
                PickerItem::new("rust-key", "Rust"),
                PickerItem::new("go-key", "Go"),
            ],
        )
        .name("lang");
        let form = Form::new()
            .field(combo.form_field().expect("named combo field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(record_data(data));
            });
        form.child(combo).into_any_element()
    });

    // The Focus trigger opens on the click; Down seats the cursor on the
    // first row and Enter picks it, filling the input with the label "Rust".
    click(cx, 60., 18.);
    press(cx, "down");
    press(cx, "enter");
    assert!(
        submitted.borrow().is_empty(),
        "the pick itself must not submit"
    );

    // Closed, Enter bubbles into the implicit submission, which serializes
    // the key, not the label the input shows.
    press(cx, "enter");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["lang=rust-key"],
        "formValue defaults to the selected key"
    );
}

/// `allowsCustomValue` forces `formValue="text"` in pinned React Aria
/// Components 1.20.0: the named field submits the typed text, whatever keys
/// the collection carries, and the custom commit leaves the key selection
/// null.
#[gpui::test]
fn form_combo_box_submits_the_text_under_allows_custom_value(cx: &mut TestAppContext) {
    let form_submits = events();
    let submitted = form_submits.clone();
    let slices = events();
    let sliced = slices.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        let submitted = form_submits.clone();
        let slices = slices.clone();
        let combo = ComboBox::new(
            state_for_view.clone(),
            vec![
                PickerItem::new("rust-key", "Rust"),
                PickerItem::new("go-key", "Go"),
            ],
        )
        .name("lang")
        .allows_custom_value(true)
        .on_selection_change_all(move |keys, _, _| {
            slices.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        });
        let form = Form::new()
            .field(combo.form_field().expect("named combo field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(record_data(data));
            });
        form.child(combo).into_any_element()
    });

    // Typing an unmatched value closes the filtered list; Enter commits the
    // custom value on the closed field and — pinned React Aria prevents the
    // default only while the menu is open — bubbles into the implicit
    // submission, which serializes the typed text.
    click(cx, 60., 18.);
    cx.simulate_input("Zig");
    press(cx, "enter");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["lang=Zig"],
        "allowsCustomValue must switch the submission to the typed text"
    );
    assert!(
        sliced.borrow().is_empty(),
        "the custom commit must leave the key selection null with nothing to change"
    );
}

/// `ComboBox::validate` resolves into the inner input's stored validity, which
/// the field's `live_text` registration reads, so a native submit blocks on it
/// and the failed field takes the focus — the focus handle the render
/// publishes into the live form state.
#[gpui::test]
fn form_combo_box_validate_blocks_native_submit_and_focuses(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let state_for_assert = state;
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let combo = ComboBox::new(state_for_view.clone(), keyed(&["Rust", "Go"]))
            .name("lang")
            .validate(|text| {
                if text == "Zig" {
                    Some("Zig is not on the list".into())
                } else {
                    None
                }
            });
        let form = Form::new()
            .field(combo.form_field().expect("named combo field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(combo).into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("Zig");
    run_handler(&submit_slot, cx);
    assert_eq!(
        invalid.borrow().as_slice(),
        ["lang="],
        "the validate failure must route the native submit to onInvalid; the \
         key-mode form value of a selection-less field is the one empty value"
    );
    assert!(
        submitted.borrow().is_empty(),
        "a native submit must not pass while the field's validate rejects the text"
    );
    assert!(
        cx.update(|window, cx| state_for_assert
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)),
        "a blocked submit must focus the failed field"
    );

    // Clearing the rejected text clears the stored validity, and picking a
    // row gives the submission its key.
    press(cx, "ctrl-a");
    press(cx, "backspace");
    press(cx, "down");
    press(cx, "enter");
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["lang=Rust"],
        "with the validate failure gone and a row picked, the same form submits"
    );
}

/// `validationBehavior: "allow"` shows the field's message without blocking:
/// the same validate failure that blocked the native submit above must let
/// this form through.
#[gpui::test]
fn form_combo_box_allow_validation_submits_anyway(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let combo = ComboBox::new(state_for_view.clone(), keyed(&["Rust", "Go"]))
            .name("lang")
            .validate(|text| {
                if text == "Zig" {
                    Some("Zig is not on the list".into())
                } else {
                    None
                }
            })
            .validation_behavior(ValidationBehavior::Allow);
        let form = Form::new()
            .field(combo.form_field().expect("named combo field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(combo).into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("Zig");
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["lang="],
        "allow must submit past the failed validate; the key-mode form value \
         of a selection-less field is the one empty value"
    );
    assert!(invalid.borrow().is_empty(), "allow must not run onInvalid");
}

/// The form's `validationErrors` record routes by name into the ComboBox's
/// input state — the slot its error slot renders — and a routed message
/// blocks a native submit; the user editing the field clears the messages,
/// after which the same form submits.
#[gpui::test]
fn form_combo_box_server_errors_display_block_and_clear(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let invalids = events();
    let invalid = invalids.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let state_for_assert = state;
    let record_for_view = std::rc::Rc::new(std::cell::RefCell::new(
        ValidationErrors::new().set_many("lang", ["Not a language we ship"]),
    ));
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let invalids = invalids.clone();
        let record = record_for_view.borrow().clone();
        let view_combo = ComboBox::new(state_for_view.clone(), keyed(&["Rust", "Go"])).name("lang");
        let form = Form::new()
            .validation_errors(record)
            .field(view_combo.form_field().expect("named combo field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            })
            .on_invalid(move |data: &FormData, _, _| {
                invalids.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(view_combo).into_any_element()
    });

    run_handler(&submit_slot, cx);
    assert_eq!(
        invalid.borrow().as_slice(),
        ["lang="],
        "the routed server message must block the native submit"
    );
    assert_eq!(
        cx.update(|_, cx| state_for_assert
            .read(cx)
            .routed_errors()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()),
        vec!["Not a language we ship".to_owned()],
        "the routed message must land in the field's own error slot"
    );

    // Editing the field suppresses its routed messages; the submission the
    // same record used to block now goes through.
    click(cx, 60., 18.);
    cx.simulate_input("R");
    run_handler(&submit_slot, cx);
    assert!(
        cx.update(|_, cx| state_for_assert.read(cx).routed_errors().is_empty()),
        "an edit to the field must clear its routed messages"
    );
    assert_eq!(
        submitted.borrow().as_slice(),
        ["lang="],
        "with the routed message cleared the same form must submit"
    );
}

/// A reset restores the default selection and the label it resolves to, and
/// reports the restored text through `onInputChange` — the way pinned
/// react-stately's `resetInputValue` re-derives the text and fires the
/// controlled input's change. A second reset with nothing to change stays
/// silent.
#[gpui::test]
fn form_combo_box_reset_restores_the_default_label_and_reports_the_input_change(
    cx: &mut TestAppContext,
) {
    let input_changes = events();
    let input_recorder = input_changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let state_for_assert = state;
    let reset_slot = handler_slot();
    let slot_for_view = reset_slot.clone();
    let cx = open_host(cx, move || {
        let input_changes = input_recorder.clone();
        let combo = ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta"]))
            .name("lang")
            .default_value(["Alpha"])
            .on_input_change(move |text, _, _| {
                input_changes.borrow_mut().push(text.to_owned());
            });
        let form = Form::new().field(combo.form_field().expect("named combo field"));
        *slot_for_view.borrow_mut() = Some(form.reset_handler());
        form.child(combo).into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("Zig");
    run_handler(&reset_slot, cx);
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "Alpha",
        "the reset must restore the default selection's label"
    );
    assert_eq!(
        input_changes.borrow().last().map(String::as_str),
        Some("Alpha"),
        "the restored text must be reported through onInputChange"
    );

    // A second reset restores the same value: nothing changed, so the input
    // callback stays silent.
    run_handler(&reset_slot, cx);
    assert_eq!(
        input_changes.borrow().last().map(String::as_str),
        Some("Alpha"),
        "a reset whose restore changes nothing must not re-report the text"
    );
}

/// Pinned React Aria Components 1.20.0's key-mode serialization maps an empty
/// `selectedKeys` — single or multiple — to one hidden input with value `""`,
/// so a named ComboBox with nothing chosen still submits `name=""`. A
/// group-backed field omits itself instead.
#[gpui::test]
fn form_combo_box_empty_selection_submits_one_empty_value(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let single_for_view = cx.new(|cx| InputState::new(cx));
    let multiple_for_view = cx.new(|cx| InputState::new(cx));
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let single = ComboBox::new(single_for_view.clone(), keyed(&["Rust", "Go"])).name("lang");
        let multiple = ComboBox::new(multiple_for_view.clone(), keyed(&["Rust", "Go"]))
            .name("langs")
            .selection_mode(SelectionMode::Multiple);
        let form = Form::new()
            .field(single.form_field().expect("named single combo field"))
            .field(multiple.form_field().expect("named multiple combo field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(format!(
                    "{}|{:?}|{:?}",
                    record_data(data),
                    data.get_all("lang"),
                    data.get_all("langs")
                ));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(single).child(multiple).into_any_element()
    });

    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["lang=,langs=|[\"\"]|[\"\"]"],
        "an empty selection must serialize one empty value per named field, \
         in FormData.text and FormData.get_all alike"
    );
}

/// `formValue="text"` without `allowsCustomValue`: the named field submits
/// the input text — the picked row's label — instead of its key.
#[gpui::test]
fn form_combo_box_form_value_text_submits_the_label_without_custom_values(cx: &mut TestAppContext) {
    let submits = events();
    let submitted = submits.clone();
    let state_for_view = cx.new(|cx| InputState::new(cx));
    let submit_slot = handler_slot();
    let slot_for_view = submit_slot.clone();
    let cx = open_host(cx, move || {
        let submits = submits.clone();
        let combo = ComboBox::new(
            state_for_view.clone(),
            vec![
                PickerItem::new("rust-key", "Rust"),
                PickerItem::new("go-key", "Go"),
            ],
        )
        .name("lang")
        .form_value(ComboBoxFormValue::Text);
        let form = Form::new()
            .field(combo.form_field().expect("named combo field"))
            .on_submit(move |data: &FormData, _, _| {
                submits.borrow_mut().push(record_data(data));
            });
        *slot_for_view.borrow_mut() = Some(form.submit_handler());
        form.child(combo).into_any_element()
    });

    // The Focus trigger opens on the click; Down seats the cursor on the
    // first row and Enter picks it, filling the input with the label "Rust".
    click(cx, 60., 18.);
    press(cx, "down");
    press(cx, "enter");
    run_handler(&submit_slot, cx);
    assert_eq!(
        submitted.borrow().as_slice(),
        ["lang=Rust"],
        "formValue text must submit the picked label, not the key"
    );
}
