//!
//! DateField and TimeField follow the v3 form contract: disabled fields are
//! unsuccessful, read-only fields remain successful, reset restores an
//! uncontrolled default, and a controlled owner is told the default so it can
//! accept it. The registered value follows the live entity, not a snapshot.

use crate::harness;

use std::{cell::Cell, rc::Rc};

use gpui::{prelude::*, AppContext, Context, Entity, TestAppContext};
use herogpui_components::{
    Date, DateField, Form, InputState, Time, TimeField, TimeGranularity, TimeState,
};
use herogpui_theme::ThemeProvider;

use harness::{events, open_host};

fn time_text(time: Option<Time>) -> String {
    time.map(|time| format!("{:02}:{:02}", time.hour, time.minute))
        .unwrap_or_default()
}

/// Renders a controlled TimeField so each frame can pass a fresh `.value`.
struct ControlledTimeField {
    state: Entity<TimeState>,
    current: Rc<Cell<Option<Time>>>,
    default: Time,
    disabled: Rc<Cell<bool>>,
    changes: harness::Events,
}

impl Render for ControlledTimeField {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut Context<'_, Self>,
    ) -> impl IntoElement {
        let current = self.current.get();
        TimeField::new(self.state.clone())
            .name("time")
            .value(current)
            .default_value(self.default)
            .is_disabled(self.disabled.get())
            .on_change({
                let current = self.current.clone();
                let changes = self.changes.clone();
                move |time, _, _| {
                    current.set(*time);
                    changes.borrow_mut().push(time_text(*time));
                }
            })
            .into_any_element()
    }
}

fn open_controlled_time_field(
    cx: &mut TestAppContext,
    view: ControlledTimeField,
) -> &mut gpui::VisualTestContext {
    cx.update(ThemeProvider::init);
    let (_view, cx) = cx.add_window_view(|_, _| view);
    cx
}

#[gpui::test]
fn enabled_date_field_submits_current_value_and_reset_restores_default(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::with_value(cx, "2025-01-15"));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let default = Date::new(2025, 1, 15);
    let field = DateField::new(state.clone())
        .name("date")
        .default_value(default)
        .form_field()
        .expect("named DateField");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(data.text("date").unwrap_or_default().to_string());
    });
    let submit = form.submit_handler();
    let reset = form.reset_handler();
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DateField::new(state_for_view.clone())
            .name("date")
            .default_value(default)
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["2025-01-15"]);

    cx.update(|_, cx| state.update(cx, |state, _| state.set_value("2025-02-03")));
    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["2025-01-15", "2025-02-03"]);

    cx.update(|window, cx| reset(window, cx));
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        ["2025-01-15", "2025-02-03", "2025-01-15"]
    );
}

#[gpui::test]
fn disabled_date_field_is_omitted_from_submission(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::with_value(cx, "2025-01-15"));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let field = DateField::new(state.clone())
        .name("date")
        .is_disabled(true)
        .form_field()
        .expect("named DateField");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form.borrow_mut().push(
            data.text("date")
                .unwrap_or_else(|| "<omitted>".into())
                .to_string(),
        );
    });
    let submit = form.submit_handler();
    let cx = open_host(cx, move || {
        DateField::new(state.clone())
            .name("date")
            .is_disabled(true)
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["<omitted>"]);
}

#[gpui::test]
fn disabled_date_field_becomes_successful_after_rerender(cx: &mut TestAppContext) {
    let disabled = Rc::new(Cell::new(true));
    let disabled_for_view = disabled.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "2025-01-15"));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let field = DateField::new(state.clone())
        .name("date")
        .form_field()
        .expect("named DateField");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form.borrow_mut().push(
            data.text("date")
                .unwrap_or_else(|| "<omitted>".into())
                .to_string(),
        );
    });
    let submit = form.submit_handler();
    let cx = open_host(cx, move || {
        DateField::new(state.clone())
            .name("date")
            .is_disabled(disabled_for_view.get())
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["<omitted>"]);

    disabled.set(false);
    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["<omitted>", "2025-01-15"]);
}

#[gpui::test]
fn read_only_date_field_remains_successful(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::with_value(cx, "2025-01-15"));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let field = DateField::new(state.clone())
        .name("date")
        .is_read_only(true)
        .form_field()
        .expect("named DateField");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form.borrow_mut().push(
            data.text("date")
                .unwrap_or_else(|| "<omitted>".into())
                .to_string(),
        );
    });
    let submit = form.submit_handler();
    let cx = open_host(cx, move || {
        DateField::new(state.clone())
            .name("date")
            .is_read_only(true)
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["2025-01-15"]);
}

#[gpui::test]
fn enabled_time_field_submits_current_value_and_reset_restores_default(cx: &mut TestAppContext) {
    let default = Time::new(9, 0);
    let state = cx.new(|cx| TimeState::with_value(cx, default));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let field = cx.update(|cx| {
        TimeField::new(state.clone())
            .name("time")
            .default_value(default)
            .form_field(cx)
            .expect("named TimeField")
    });
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(data.text("time").unwrap_or_default().to_string());
    });
    let submit = form.submit_handler();
    let reset = form.reset_handler();
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        TimeField::new(state_for_view.clone())
            .name("time")
            .default_value(default)
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["09:00"]);

    cx.update(|_, cx| {
        state.update(cx, |state, _| state.value = Some(Time::new(14, 30)));
    });
    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["09:00", "14:30"]);

    cx.update(|window, cx| reset(window, cx));
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        ["09:00", "14:30", "09:00"],
        "reset must restore the uncontrolled default into the live entity"
    );
}

#[gpui::test]
fn second_granularity_time_field_submits_and_resets_with_seconds(cx: &mut TestAppContext) {
    let default = Time::new(9, 8).with_second(7);
    let current = Time::new(14, 30).with_second(45);
    let state = cx.new(|cx| TimeState::with_value(cx, default));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let field = cx.update(|cx| {
        TimeField::new(state.clone())
            .name("time")
            .default_value(default)
            .granularity(TimeGranularity::Second)
            .form_field(cx)
            .expect("named second TimeField")
    });
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(data.text("time").unwrap_or_default().to_string());
    });
    let submit = form.submit_handler();
    let reset = form.reset_handler();
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        TimeField::new(state_for_view.clone())
            .name("time")
            .default_value(default)
            .granularity(TimeGranularity::Second)
            .into_any_element()
    });

    cx.update(|_, cx| state.update(cx, |state, _| state.value = Some(current)));
    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| submit(window, cx));
    cx.update(|window, cx| reset(window, cx));
    cx.update(|window, cx| submit(window, cx));

    assert_eq!(submitted.borrow().as_slice(), ["14:30:45", "09:08:07"]);
}

#[gpui::test]
fn disabled_time_field_is_omitted_from_submission(cx: &mut TestAppContext) {
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 0)));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let field = cx.update(|cx| {
        TimeField::new(state.clone())
            .name("time")
            .is_disabled(true)
            .form_field(cx)
            .expect("named TimeField")
    });
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form.borrow_mut().push(
            data.text("time")
                .unwrap_or_else(|| "<omitted>".into())
                .to_string(),
        );
    });
    let submit = form.submit_handler();
    let cx = open_host(cx, move || {
        TimeField::new(state.clone())
            .name("time")
            .is_disabled(true)
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["<omitted>"]);
}

#[gpui::test]
fn disabled_time_field_becomes_successful_after_rerender(cx: &mut TestAppContext) {
    let disabled = Rc::new(Cell::new(true));
    let disabled_for_view = disabled.clone();
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 0)));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let field = cx.update(|cx| {
        TimeField::new(state.clone())
            .name("time")
            .form_field(cx)
            .expect("named TimeField")
    });
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form.borrow_mut().push(
            data.text("time")
                .unwrap_or_else(|| "<omitted>".into())
                .to_string(),
        );
    });
    let submit = form.submit_handler();
    let cx = open_host(cx, move || {
        TimeField::new(state.clone())
            .name("time")
            .is_disabled(disabled_for_view.get())
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["<omitted>"]);

    disabled.set(false);
    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["<omitted>", "09:00"]);
}

#[gpui::test]
fn read_only_time_field_remains_successful(cx: &mut TestAppContext) {
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 0)));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let field = cx.update(|cx| {
        TimeField::new(state.clone())
            .name("time")
            .is_read_only(true)
            .form_field(cx)
            .expect("named TimeField")
    });
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form.borrow_mut().push(
            data.text("time")
                .unwrap_or_else(|| "<omitted>".into())
                .to_string(),
        );
    });
    let submit = form.submit_handler();
    let cx = open_host(cx, move || {
        TimeField::new(state.clone())
            .name("time")
            .is_read_only(true)
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["09:00"]);
}

#[gpui::test]
fn controlled_time_field_reset_reports_default_for_owner_acceptance(cx: &mut TestAppContext) {
    let default = Time::new(9, 0);
    let current = Rc::new(Cell::new(Some(default)));
    let changes = events();
    let state = cx.new(|cx| TimeState::with_value(cx, default));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let field = cx.update(|cx| {
        TimeField::new(state.clone())
            .name("time")
            .value(current.get())
            .default_value(default)
            .form_field(cx)
            .expect("named TimeField")
    });
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(data.text("time").unwrap_or_default().to_string());
    });
    let submit = form.submit_handler();
    let reset = form.reset_handler();
    let cx = open_controlled_time_field(
        cx,
        ControlledTimeField {
            state,
            current: current.clone(),
            default,
            disabled: Rc::new(Cell::new(false)),
            changes: changes.clone(),
        },
    );

    current.set(Some(Time::new(14, 30)));
    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["14:30"]);

    cx.update(|window, cx| reset(window, cx));
    assert_eq!(
        changes.borrow().as_slice(),
        ["09:00"],
        "controlled reset must report defaultValue so the owner can accept it"
    );
    assert_eq!(current.get(), Some(default));

    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        ["14:30", "09:00"],
        "after the owner accepts the reset, submit must read the restored default"
    );
}

#[gpui::test]
fn disabled_controlled_time_field_reset_still_reports_default_to_owner(cx: &mut TestAppContext) {
    let default = Time::new(9, 0);
    let current = Rc::new(Cell::new(Some(Time::new(14, 30))));
    let changes = events();
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(14, 30)));
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let field = cx.update(|cx| {
        TimeField::new(state.clone())
            .name("time")
            .value(current.get())
            .default_value(default)
            .is_disabled(true)
            .form_field(cx)
            .expect("named TimeField")
    });
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form.borrow_mut().push(
            data.text("time")
                .unwrap_or_else(|| "<omitted>".into())
                .to_string(),
        );
    });
    let submit = form.submit_handler();
    let reset = form.reset_handler();
    let cx = open_controlled_time_field(
        cx,
        ControlledTimeField {
            state,
            current: current.clone(),
            default,
            disabled: Rc::new(Cell::new(true)),
            changes: changes.clone(),
        },
    );

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["<omitted>"]);

    cx.update(|window, cx| reset(window, cx));
    assert_eq!(
        changes.borrow().as_slice(),
        ["09:00"],
        "disabled blocks submit, not the reset that reports the controlled default"
    );
    assert_eq!(current.get(), Some(default));
}
