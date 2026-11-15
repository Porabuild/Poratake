//! HeroUI v3.2.7 indicator bounds, decimal steps and RTL input.
use crate::harness::{self, click, open_host, press};
use gpui::{prelude::*, px, TestAppContext};
use herogpui_components::{i18n, Slider, SliderMark};
use std::{cell::Cell, rc::Rc};

#[gpui::test]
fn decimal_steps_include_the_endpoint_without_an_extra_dot(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(300.))
            .child(
                Slider::new("decimal", 0.2)
                    .max_value(0.3)
                    .step(0.1)
                    .show_steps(true),
            )
            .into_any_element()
    });
    assert!(cx.debug_bounds("slider-step-3").is_some());
    assert!(cx.debug_bounds("slider-step-4").is_none());
    let start = cx.debug_bounds("slider-step-0").unwrap().center();
    let end = cx.debug_bounds("slider-step-3").unwrap().center();
    assert!((f32::from(end.x - start.x) - 276.).abs() < 0.5);
}

#[gpui::test]
fn excessive_dots_and_continuous_ranges_are_omitted(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(300.))
            .child(
                Slider::new("too-many", 0.)
                    .max_value(1000.)
                    .show_steps(true),
            )
            .child(
                Slider::new("continuous", 0.)
                    .continuous(true)
                    .show_steps(true),
            )
            .into_any_element()
    });
    assert!(cx.debug_bounds("slider-step-0").is_none());
}

#[gpui::test]
fn marks_filter_invalid_values_and_do_not_change_step_increments(cx: &mut TestAppContext) {
    harness::still();
    let value = Rc::new(Cell::new(20.));
    let seen = value.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        gpui::div()
            .w(px(300.))
            .child(
                Slider::new("marked", 20.)
                    .default_value(20.)
                    .step(10.)
                    .marks([
                        SliderMark::new(0.),
                        SliderMark::new(37.).child("Custom"),
                        SliderMark::new(f32::NAN),
                        SliderMark::new(101.),
                    ])
                    .on_change(move |next, _, _| seen.set(*next)),
            )
            .into_any_element()
    });
    assert!(cx.debug_bounds("slider-mark-1").is_some());
    assert!(cx.debug_bounds("slider-mark-2").is_none());
    assert!(cx.debug_bounds("slider-mark-3").is_none());
    click(cx, 60., 10.);
    press(cx, "right");
    assert!((value.get() - 30.).abs() < 0.001);
}

#[gpui::test]
fn rtl_reverses_indicator_positions_pointer_and_horizontal_keys(cx: &mut TestAppContext) {
    harness::still();
    let value = Rc::new(Cell::new(0.));
    let seen = value.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        gpui::div()
            .w(px(300.))
            .child(
                Slider::new("rtl", 0.)
                    .default_value(0.)
                    .step(10.)
                    .show_steps(true)
                    .marks([SliderMark::new(0.), SliderMark::new(100.)])
                    .on_change(move |next, _, _| seen.set(*next)),
            )
            .into_any_element()
    });
    cx.update(|window, cx| {
        i18n::set_locale("ar", cx);
        window.refresh();
    });
    let zero = cx.debug_bounds("slider-step-0").unwrap().center();
    let hundred = cx.debug_bounds("slider-step-10").unwrap().center();
    assert!(zero.x > hundred.x);
    click(cx, 150., 10.);
    assert!((value.get() - 50.).abs() < 0.001);
    press(cx, "left");
    assert!((value.get() - 60.).abs() < 0.001);
    press(cx, "right");
    assert!((value.get() - 50.).abs() < 0.001);
}

#[gpui::test]
fn custom_marks_reserve_their_intrinsic_height(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .w(px(300.))
            .child(
                Slider::new("custom-marks", 40.).marks([SliderMark::new(50.).child(
                    gpui::div()
                        .h(px(40.))
                        .w(px(60.))
                        .debug_selector(|| "tall-mark".into()),
                )]),
            )
            .child(harness::probe("after-slider"))
            .into_any_element()
    });
    let mark = cx.debug_bounds("tall-mark").unwrap();
    let after = cx.debug_bounds("after-slider").unwrap();
    assert!(
        after.top() >= mark.bottom(),
        "custom labels must contribute to the marks row"
    );
}

#[gpui::test]
fn compact_vertical_marks_match_the_usable_track_span(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        Slider::new("small-vertical", 50.)
            .size(herogpui_components::SliderSize::Sm)
            .orientation(herogpui_core::Orientation::Vertical)
            .marks([
                SliderMark::new(0.).child(harness::probe("low-mark")),
                SliderMark::new(100.).child(harness::probe("high-mark")),
            ])
            .into_any_element()
    });
    let low = cx.debug_bounds("low-mark").unwrap().center();
    let high = cx.debug_bounds("high-mark").unwrap().center();
    assert!((f32::from(low.y - high.y) - 144.).abs() < 0.5);
}

#[gpui::test]
fn script_and_case_control_rtl_direction(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(300.))
            .child(
                Slider::new("locale-direction", 50.)
                    .step(50.)
                    .show_steps(true),
            )
            .into_any_element()
    });
    for (locale, rtl) in [
        ("AR", true),
        ("az-Arab", true),
        ("pa_Arab", true),
        ("ar-Latn", false),
        ("ku", false),
        ("pa-PK", true),
        ("az-IR", true),
        ("en", false),
    ] {
        cx.update(|window, cx| {
            i18n::set_locale(locale, cx);
            window.refresh();
        });
        let zero = cx.debug_bounds("slider-step-0").unwrap().center();
        let end = cx.debug_bounds("slider-step-2").unwrap().center();
        assert_eq!(zero.x > end.x, rtl, "{locale}");
    }
}

#[gpui::test]
fn rtl_scalar_fill_squares_the_join_with_the_right_cap(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(300.))
            .child(Slider::new("rtl-corners", 50.).sx(|el| el.rounded(px(8.))))
            .into_any_element()
    });
    cx.update(|window, cx| {
        i18n::set_locale("ar", cx);
        window.refresh();
    });
    let (quads, scale) = cx.update(|window, _| (window.painted_quads(), window.scale_factor()));
    let fill = quads
        .iter()
        .find(|quad| {
            (quad.bounds.size.width.0 / scale - 138.).abs() < 0.1
                && (quad.bounds.size.height.0 / scale - 20.).abs() < 0.1
        })
        .unwrap();
    assert!(fill.corner_radii.top_left.0 > 0.);
    assert!(fill.corner_radii.top_right.0.abs() < 0.001);
    assert!(fill.corner_radii.bottom_right.0.abs() < 0.001);
}

#[gpui::test]
fn vertical_marks_reserve_custom_label_width(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .items_start()
            .child(
                Slider::new("wide-vertical", 50.)
                    .orientation(herogpui_core::Orientation::Vertical)
                    .marks([SliderMark::new(50.).child(
                        gpui::div()
                            .w(px(100.))
                            .h(px(24.))
                            .debug_selector(|| "wide-mark".into()),
                    )]),
            )
            .child(harness::probe("after-vertical-slider"))
            .into_any_element()
    });
    let mark = cx.debug_bounds("wide-mark").unwrap();
    let after = cx.debug_bounds("after-vertical-slider").unwrap();
    assert!(
        after.left() >= mark.right(),
        "the widest mark must contribute to the vertical slider width"
    );
}

#[gpui::test]
fn decimal_endpoint_dots_use_the_selected_interval_color(cx: &mut TestAppContext) {
    use herogpui_theme::ActiveTheme;
    for range in [false, true] {
        harness::still();
        let cx = open_host(cx, move || {
            let mut slider = Slider::new("decimal-color", 0.9)
                .max_value(1.)
                .step(0.1)
                .show_steps(true)
                .thumb(|_, _| gpui::div().into_any_element());
            if range {
                slider = slider.values([0.1, 0.9]);
            }
            gpui::div().w(px(300.)).child(slider).into_any_element()
        });
        let expected = cx.update(|_, cx| cx.colors().accent.foreground.alpha(0.25));
        let scene = harness::painted(cx);
        for name in if range {
            vec!["slider-step-1", "slider-step-9"]
        } else {
            vec!["slider-step-9"]
        } {
            let dot = cx.debug_bounds(name).unwrap();
            let quad = scene
                .quads
                .iter()
                .find(|quad| {
                    let bounds = scene.bounds(quad);
                    (f32::from(bounds.origin.x - dot.origin.x)).abs() < 0.01
                        && (f32::from(bounds.origin.y - dot.origin.y)).abs() < 0.01
                        && (f32::from(bounds.size.width) - 3.).abs() < 0.01
                        && (f32::from(bounds.size.height) - 3.).abs() < 0.01
                })
                .expect("step dot must paint");
            assert_eq!(
                quad.background.as_solid(),
                Some(expected),
                "{name}, range={range}"
            );
        }
    }
}

#[gpui::test]
fn inferred_scripts_reverse_pointer_and_arrow_input_together(cx: &mut TestAppContext) {
    harness::still();
    let value = Rc::new(Cell::new(50.));
    let seen = value.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        gpui::div()
            .w(px(300.))
            .child(
                Slider::new("inferred-rtl", 50.)
                    .default_value(50.)
                    .step(10.)
                    .on_change(move |next, _, _| seen.set(*next)),
            )
            .into_any_element()
    });
    for (locale, pointer_value, left_value) in
        [("ku", 20., 10.), ("pa-PK", 80., 90.), ("az-IR", 80., 90.)]
    {
        cx.update(|window, cx| {
            i18n::set_locale(locale, cx);
            window.refresh();
        });
        click(cx, 60., 10.);
        assert!(
            (value.get() - pointer_value).abs() < 0.001,
            "{locale} pointer"
        );
        press(cx, "left");
        assert!((value.get() - left_value).abs() < 0.001, "{locale} left");
        press(cx, "right");
        assert!(
            (value.get() - pointer_value).abs() < 0.001,
            "{locale} right"
        );
    }
}
