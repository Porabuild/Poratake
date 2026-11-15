//! Slider form integration for live values, disabled controls and reset.
//!
//! HeroUI v3.2.4 documents Slider's `value`/`defaultValue`, `onChange`,
//! `isDisabled`, and per-thumb `name`. The pinned React Aria 3.51.0 source
//! creates a hidden range input for each thumb, writes its current `value`,
//! passes through `name`, and sets `disabled` from the slider/thumb state.
//! These tests exercise the equivalent FormField bridge across real renders.

use crate::harness;

use std::{cell::RefCell, rc::Rc};

use gpui::{prelude::*, px, TestAppContext, VisualTestContext};
use harness::{click, events, open_host};
use herogpui_components::{Button, Form, FormData, Slider};

type Submit = std::sync::Arc<dyn Fn(&mut gpui::Window, &mut gpui::App)>;

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn submit_text(data: &FormData, name: &str) -> String {
    data.get(name)
        .map_or_else(|| "omitted".to_owned(), |value| value.as_text().to_string())
}

fn submit_button(id: &'static str, submit: Submit) -> Button {
    Button::new(id)
        .label("Submit")
        .on_press(move |_, window, cx| submit(window, cx))
}

#[gpui::test]
fn uncontrolled_slider_form_reads_value_after_pointer_change(cx: &mut TestAppContext) {
    let submitted = events();
    let for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let slider = Slider::new("slider-live-single", 25.)
            .default_value(25.)
            .name("volume");
        let form = Form::new()
            .field(slider.form_field().expect("named slider field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "volume"));
            });
        let submit = form.submit_handler();
        form.child(gpui::div().w(px(600.)).child(slider))
            .child(submit_button("slider-live-submit", submit))
            .into_any_element()
    });

    click(cx, 450., 10.);
    flush_frame(cx);
    click(cx, 60., 52.);
    flush_frame(cx);

    assert_eq!(submitted.borrow().as_slice(), ["75"]);
}

#[gpui::test]
fn controlled_range_slider_form_reads_parent_values_after_change(cx: &mut TestAppContext) {
    let submitted = events();
    let changes = events();
    let current = Rc::new(RefCell::new(vec![20., 80.]));
    let for_view = current;
    let submitted_for_view = submitted.clone();
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let current = for_view.clone();
        let submitted = submitted_for_view.clone();
        let changes = changes_for_view.clone();
        let values = current.borrow().clone();
        let slider = Slider::new("slider-live-range", 0.)
            .values(values)
            .start_name("minimum")
            .end_name("maximum")
            .on_change_all(move |values, _, _| {
                *current.borrow_mut() = values.to_vec();
                changes.borrow_mut().push(format!("{values:?}"));
            });
        let mut form = Form::new().on_submit(move |data: &FormData, _, _| {
            submitted.borrow_mut().push(format!(
                "{}:{}",
                submit_text(data, "minimum"),
                submit_text(data, "maximum")
            ));
        });
        for field in slider.form_fields() {
            form = form.field(field);
        }
        let submit = form.submit_handler();
        form.child(gpui::div().w(px(600.)).child(slider))
            .child(submit_button("slider-live-range-submit", submit))
            .into_any_element()
    });

    // The range is 600px wide; x=300 maps to 50 and is equidistant from the
    // initial thumbs, so the lower thumb wins the stable tie.
    click(cx, 180., 10.);
    flush_frame(cx);
    click(cx, 60., 52.);
    flush_frame(cx);

    assert_eq!(changes.borrow().as_slice(), ["[30.0, 80.0]"]);
    assert_eq!(submitted.borrow().as_slice(), ["30:80"]);
}

#[gpui::test]
fn disabled_slider_is_not_a_successful_form_control(cx: &mut TestAppContext) {
    let submitted = events();
    let for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let slider = Slider::new("slider-live-disabled", 25.)
            .default_value(25.)
            .name("volume")
            .is_disabled(true);
        let form = Form::new()
            .field(
                slider
                    .form_field()
                    .expect("disabled field remains registered"),
            )
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "volume"));
            });
        let submit = form.submit_handler();
        form.child(gpui::div().w(px(600.)).child(slider))
            .child(submit_button("slider-live-disabled-submit", submit))
            .into_any_element()
    });

    click(cx, 60., 52.);
    assert_eq!(submitted.borrow().as_slice(), ["omitted"]);
}

#[gpui::test]
fn uncontrolled_slider_reset_restores_default_before_next_submit(cx: &mut TestAppContext) {
    let submitted = events();
    let for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let slider = Slider::new("slider-live-reset", 25.)
            .default_value(25.)
            .name("volume");
        let form = Form::new()
            .field(slider.form_field().expect("named slider field"))
            .on_submit(move |data: &FormData, _, _| {
                submitted.borrow_mut().push(submit_text(data, "volume"));
            });
        let submit = form.submit_handler();
        let reset = form.reset_handler();
        form.child(gpui::div().w(px(600.)).child(slider))
            .child(submit_button("slider-live-reset-submit", submit))
            .child(
                Button::new("slider-live-reset-button")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 450., 10.);
    flush_frame(cx);
    click(cx, 60., 52.);
    flush_frame(cx);
    click(cx, 60., 109.);
    flush_frame(cx);
    click(cx, 60., 52.);

    assert_eq!(submitted.borrow().as_slice(), ["75", "25"]);
}

#[gpui::test]
fn controlled_slider_reset_reports_the_initial_value_once(cx: &mut TestAppContext) {
    let changes = events();
    let current = Rc::new(RefCell::new(25.));
    let for_view = current;
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let current = for_view.clone();
        let changes = changes_for_view.clone();
        let value = *current.borrow();
        let slider = Slider::new("slider-live-controlled-reset", value)
            .value(value)
            .name("volume")
            .on_change(move |value, _, _| {
                *current.borrow_mut() = *value;
                changes.borrow_mut().push(format!("{value}"));
            });
        let form = Form::new().field(slider.form_field().expect("named slider field"));
        let reset = form.reset_handler();
        form.child(gpui::div().w(px(600.)).child(slider))
            .child(
                Button::new("slider-live-controlled-reset-button")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 450., 10.);
    flush_frame(cx);
    click(cx, 60., 52.);

    assert_eq!(changes.borrow().as_slice(), ["75", "25"]);
}

#[gpui::test]
fn controlled_range_reset_reports_both_values_once(cx: &mut TestAppContext) {
    let changes = events();
    let current = Rc::new(RefCell::new(vec![20., 80.]));
    let for_view = current;
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let current = for_view.clone();
        let changes = changes_for_view.clone();
        let values = current.borrow().clone();
        let slider = Slider::new("slider-live-range-reset", 0.)
            .values(values)
            .start_name("minimum")
            .end_name("maximum")
            .on_change_all(move |values, _, _| {
                *current.borrow_mut() = values.to_vec();
                changes.borrow_mut().push(format!("{values:?}"));
            });
        let mut form = Form::new();
        for field in slider.form_fields() {
            form = form.field(field);
        }
        let reset = form.reset_handler();
        form.child(gpui::div().w(px(600.)).child(slider))
            .child(
                Button::new("slider-live-range-reset-button")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 180., 10.);
    flush_frame(cx);
    assert_eq!(changes.borrow().as_slice(), ["[30.0, 80.0]"]);

    click(cx, 60., 52.);
    assert_eq!(
        changes.borrow().as_slice(),
        ["[30.0, 80.0]", "[20.0, 80.0]"]
    );
}

#[gpui::test]
fn controlled_one_element_array_reset_reports_its_initial_value(cx: &mut TestAppContext) {
    let changes = events();
    let current = Rc::new(RefCell::new(vec![20.]));
    let for_view = current;
    let changes_for_view = changes.clone();
    let cx = open_host(cx, move || {
        let current = for_view.clone();
        let changes = changes_for_view.clone();
        let values = current.borrow().clone();
        let slider = Slider::new("slider-one-controlled-reset", 0.)
            .values(values)
            .thumb_names(["volume"])
            .on_change_all(move |values, _, _| {
                *current.borrow_mut() = values.to_vec();
                changes.borrow_mut().push(format!("{}", values[0]));
            });
        let mut form = Form::new();
        for field in slider.form_fields() {
            form = form.field(field);
        }
        let reset = form.reset_handler();
        form.child(gpui::div().w(px(600.)).child(slider))
            .child(
                Button::new("slider-one-controlled-reset-button")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 480., 10.);
    flush_frame(cx);
    click(cx, 60., 52.);

    assert_eq!(changes.borrow().as_slice(), ["80", "20"]);
}

#[gpui::test]
fn uncontrolled_range_default_values_persist_and_reset(cx: &mut TestAppContext) {
    let submitted = events();
    let for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let slider = Slider::new("slider-default-range", 0.)
            .default_values([20., 80.])
            .thumb_names(["minimum", "maximum"]);
        let mut form = Form::new().on_submit(move |data: &FormData, _, _| {
            submitted.borrow_mut().push(format!(
                "{}:{}",
                submit_text(data, "minimum"),
                submit_text(data, "maximum")
            ));
        });
        for field in slider.form_fields() {
            form = form.field(field);
        }
        let submit = form.submit_handler();
        let reset = form.reset_handler();
        form.child(gpui::div().w(px(600.)).child(slider))
            .child(submit_button("slider-default-range-submit", submit))
            .child(
                Button::new("slider-default-range-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 52.);
    flush_frame(cx);
    click(cx, 180., 10.);
    flush_frame(cx);
    click(cx, 60., 52.);
    flush_frame(cx);
    click(cx, 60., 109.);
    flush_frame(cx);
    click(cx, 60., 52.);

    assert_eq!(
        submitted.borrow().as_slice(),
        ["20:80", "30:80", "20:80"],
        "an uncontrolled range keeps both seeded values, then restores both on reset"
    );
}

#[gpui::test]
fn one_element_default_values_reset_through_the_array_state(cx: &mut TestAppContext) {
    let changes = events();
    let submitted = events();
    let changes_for_view = changes.clone();
    let submitted_for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let changes = changes_for_view.clone();
        let submitted = submitted_for_view.clone();
        let slider = Slider::new("slider-one-array-reset", 0.)
            .default_values([20.])
            .thumb_names(["volume"])
            .on_change_all(move |values, _, _| {
                changes.borrow_mut().push(format!("{}", values[0]));
            });
        let mut form = Form::new().on_submit(move |data: &FormData, _, _| {
            submitted.borrow_mut().push(submit_text(data, "volume"));
        });
        for field in slider.form_fields() {
            form = form.field(field);
        }
        let submit = form.submit_handler();
        let reset = form.reset_handler();
        form.child(gpui::div().w(px(600.)).child(slider))
            .child(submit_button("slider-one-array-submit", submit))
            .child(
                Button::new("slider-one-array-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 480., 10.);
    flush_frame(cx);
    click(cx, 60., 52.);
    flush_frame(cx);
    click(cx, 60., 109.);
    flush_frame(cx);
    click(cx, 60., 52.);

    assert_eq!(changes.borrow().as_slice(), ["80", "20"]);
    assert_eq!(submitted.borrow().as_slice(), ["80", "20"]);
}

#[gpui::test]
fn three_thumb_names_follow_current_values_and_omit_disabled_middle(cx: &mut TestAppContext) {
    let submitted = events();
    let for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let slider = Slider::new("slider-three-form", 0.)
            .default_values([10., 50., 90.])
            .thumb_names(["low", "middle", "high"])
            .disabled_keys([1]);
        let mut form = Form::new().on_submit(move |data: &FormData, _, _| {
            let values = data
                .iter()
                .map(|(name, value)| format!("{name}={}", value.as_text()))
                .collect::<Vec<_>>()
                .join(",");
            submitted.borrow_mut().push(values);
        });
        for field in slider.form_fields() {
            form = form.field(field);
        }
        let submit = form.submit_handler();
        form.child(gpui::div().w(px(600.)).child(slider))
            .child(submit_button("slider-three-form-submit", submit))
            .into_any_element()
    });

    click(cx, 480., 10.);
    flush_frame(cx);
    click(cx, 60., 52.);

    assert_eq!(
        submitted.borrow().as_slice(),
        ["low=10,high=80"],
        "each named thumb submits its current value while a disabled middle thumb is omitted"
    );
}

#[gpui::test]
fn first_successful_named_thumb_owns_reset_when_the_first_thumb_is_disabled(
    cx: &mut TestAppContext,
) {
    let submitted = events();
    let for_view = submitted.clone();
    let cx = open_host(cx, move || {
        let submitted = for_view.clone();
        let slider = Slider::new("slider-disabled-first-reset", 0.)
            .default_values([10., 50.])
            .thumb_names(["low", "high"])
            .disabled_keys([0]);
        let mut form = Form::new().on_submit(move |data: &FormData, _, _| {
            submitted.borrow_mut().push(submit_text(data, "high"));
        });
        for field in slider.form_fields() {
            form = form.field(field);
        }
        let submit = form.submit_handler();
        let reset = form.reset_handler();
        form.child(gpui::div().w(px(600.)).child(slider))
            .child(submit_button("slider-disabled-first-submit", submit))
            .child(
                Button::new("slider-disabled-first-reset")
                    .label("Reset")
                    .on_press(move |_, window, cx| reset(window, cx)),
            )
            .into_any_element()
    });

    click(cx, 60., 52.);
    click(cx, 480., 10.);
    flush_frame(cx);
    click(cx, 60., 52.);
    click(cx, 60., 109.);
    flush_frame(cx);
    click(cx, 60., 52.);

    assert_eq!(
        submitted.borrow().as_slice(),
        ["50", "80", "50"],
        "reset must be registered on the first successful named thumb"
    );
}
