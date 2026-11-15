//! The additive size enums step the pinned geometry they own.
//!
//! `CheckboxSize`, `RadioSize` and `TabsSize` are repository extensions under
//! the parity amendment's narrow exception (docs/agents/parity.md): every step
//! is defined in the component's own docs, and `Md` is byte-identical to the
//! pinned default. No test rendered any of the three enums, so this binary
//! makes both halves of that claim mechanical at render level:
//!
//! - An untouched component and `.size(Md)` must lay out identically. Their
//!   probe boxes are compared exactly — the same computation twice, so any
//!   drift between the default and the named step fails.
//! - `Sm` must move the documented metric: the Tabs box to 28px, the Checkbox
//!   control box to 14px and the RadioGroup control circle to 14px, each
//!   against the pinned `Md` 32/16/16.
//!
//! Where each box is read from:
//!
//! - Tabs labels are plain strings, so no element probe fits inside a tab.
//!   The primary variant's pill IS the selected tab's rect —
//!   `indicator_target` copies the tab's bounds — so one `debug_bounds` read
//!   pins height, `px` padding and label text size together: the pill is
//!   `text_width(label, 14) + 2·16` tall 32 at Md and
//!   `text_width(label, 12) + 2·12` tall 28 at Sm.
//! - A label-less Checkbox row hugs its control box, so a wrapping div's
//!   bounds are the control's.
//! - The RadioGroup control is read through the `indicator` render prop: a
//!   `size_full` canvas inside the `size(circle)` box records the circle's
//!   laid-out bounds.

use crate::harness;

use std::{cell::RefCell, rc::Rc};

use gpui::{
    canvas, point, prelude::*, px, size, Bounds, Font, FontFeatures, FontStyle, FontWeight, Pixels,
    TestAppContext, VisualTestContext, WindowTextSystem,
};
use herogpui_components::{
    Checkbox, CheckboxSize, RadioGroup, RadioOption, RadioOptionState, RadioSize, TabItem, Tabs,
    TabsSize,
};

use harness::open_host;

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// The indicator settles on the frame after the tab geometry is recorded.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        flush_frame(cx);
    }
}

fn near(value: Pixels, expected: f32) -> bool {
    (f32::from(value) - expected).abs() < 0.5
}

/// `near` for the canvas-recorded `Bounds<f32>`, which carry plain floats.
fn near_f32(value: f32, expected: f32) -> bool {
    (value - expected).abs() < 0.5
}

/// The width of `text` shaped at `size`, MEDIUM, as the tabs label paints.
fn text_width(system: &WindowTextSystem, text: &str, size: f32) -> f32 {
    let run = gpui::TextRun {
        len: text.len(),
        font: Font {
            family: ".SystemUIFont".into(),
            features: FontFeatures::default(),
            weight: FontWeight::MEDIUM,
            style: FontStyle::default(),
            fallbacks: None,
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = system.shape_line(text.to_owned().into(), px(size), &[run], None);
    f32::from(line.width)
}

/// The primary pill's bounds: the selected tab's own box.
fn pill(cx: &mut VisualTestContext, id: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(Box::leak(
        format!("Name(\"{id}\")-indicator").into_boxed_str(),
    ))
    .unwrap_or_else(|| panic!("the `{id}` tab pill must paint"))
}

/// Asserts the two boxes are the same box: identical size and left edge.
fn assert_same_box(actual: Bounds<Pixels>, pinned: Bounds<Pixels>, context: &str) {
    assert_eq!(
        actual.size, pinned.size,
        "{context}: the default and Md boxes must be identical"
    );
    assert_eq!(
        actual.origin.x, pinned.origin.x,
        "{context}: the default and Md boxes must start at the same x"
    );
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

fn tab_items() -> Vec<TabItem> {
    vec![TabItem::new("first", "First")]
}

/// `.tabs__tab` is `h-8 px-4 text-sm`; the Sm step is `h-7 px-3 text-xs`.
#[gpui::test]
fn tabs_size_steps_the_tab_box_and_md_is_the_default(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(8.))
            .child(Tabs::new("sz-tabs-default", tab_items(), "first").into_any_element())
            .child(
                Tabs::new("sz-tabs-md", tab_items(), "first")
                    .size(TabsSize::Md)
                    .into_any_element(),
            )
            .child(
                Tabs::new("sz-tabs-sm", tab_items(), "first")
                    .size(TabsSize::Sm)
                    .into_any_element(),
            )
            .into_any_element()
    });
    settle(cx);

    let default = pill(cx, "sz-tabs-default");
    let md = pill(cx, "sz-tabs-md");
    let sm = pill(cx, "sz-tabs-sm");
    assert_same_box(md, default, "Tabs");

    assert!(
        near(md.size.height, 32.),
        "the Md tab box must stay the pinned 32px, got {md:?}"
    );
    assert!(
        near(md.size.width, text_width_for(cx, "First", 14.) + 32.),
        "the Md tab box must be the 14px label plus 2·16 padding, got {md:?}"
    );
    assert!(
        near(sm.size.height, 28.),
        "the Sm tab box must step to 28px, got {sm:?}"
    );
    assert!(
        near(sm.size.width, text_width_for(cx, "First", 12.) + 24.),
        "the Sm tab box must be the 12px label plus 2·12 padding, got {sm:?}"
    );
}

fn text_width_for(cx: &mut VisualTestContext, text: &str, size: f32) -> f32 {
    cx.update(|window, _| text_width(window.text_system(), text, size))
}

// ---------------------------------------------------------------------------
// Checkbox
// ---------------------------------------------------------------------------

/// A label-less Checkbox row hugs its control box, so the wrapper's bounds
/// are the control's.
#[gpui::test]
fn checkbox_size_steps_the_control_box_and_md_is_the_default(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(8.))
            .child(
                gpui::div()
                    .debug_selector(|| "sz-check-default".to_owned())
                    .child(Checkbox::new("sz-check-default-el")),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "sz-check-md".to_owned())
                    .child(Checkbox::new("sz-check-md-el").size(CheckboxSize::Md)),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "sz-check-sm".to_owned())
                    .child(Checkbox::new("sz-check-sm-el").size(CheckboxSize::Sm)),
            )
            .into_any_element()
    });
    settle(cx);

    let default = box_of(cx, "sz-check-default");
    let md = box_of(cx, "sz-check-md");
    let sm = box_of(cx, "sz-check-sm");
    assert_same_box(md, default, "Checkbox");

    assert!(
        near(md.size.width, 16.) && near(md.size.height, 16.),
        "the Md control box must stay the pinned 16px, got {md:?}"
    );
    assert!(
        near(sm.size.width, 14.) && near(sm.size.height, 14.),
        "the Sm control box must step to 14px, got {sm:?}"
    );
}

fn box_of(cx: &mut VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("the `{name}` box must paint"))
}

// ---------------------------------------------------------------------------
// RadioGroup
// ---------------------------------------------------------------------------

/// Recorded control-circle bounds, one entry per frame.
type Circles = Rc<RefCell<Vec<Bounds<f32>>>>;

/// An `RadioGroup::indicator` render prop that reports the `size(circle)`
/// box it fills, standing in for the built-in dot.
fn circle_probe(
    recorded: Circles,
) -> impl Fn(&gpui::SharedString, RadioOptionState) -> gpui::AnyElement {
    move |_label, _state| {
        let recorded = recorded.clone();
        canvas(
            move |bounds: Bounds<Pixels>, _, _| {
                recorded.borrow_mut().push(Bounds {
                    origin: point(f32::from(bounds.origin.x), f32::from(bounds.origin.y)),
                    size: size(f32::from(bounds.size.width), f32::from(bounds.size.height)),
                });
                bounds
            },
            |_, _, _, _| {},
        )
        .size_full()
        .into_any_element()
    }
}

fn last_circle(recorded: &Circles, id: &str) -> Bounds<f32> {
    recorded
        .borrow()
        .last()
        .copied()
        .unwrap_or_else(|| panic!("the `{id}` control circle must paint"))
}

#[gpui::test]
fn radio_size_steps_the_control_circle_and_md_is_the_default(cx: &mut TestAppContext) {
    let defaults = Circles::default();
    let mds = Circles::default();
    let sms = Circles::default();
    let defaults_for_view = defaults.clone();
    let mds_for_view = mds.clone();
    let sms_for_view = sms.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(8.))
            .child(
                RadioGroup::new("sz-radio-default", vec![RadioOption::new("Option")])
                    .indicator(circle_probe(defaults_for_view.clone()))
                    .into_any_element(),
            )
            .child(
                RadioGroup::new("sz-radio-md", vec![RadioOption::new("Option")])
                    .size(RadioSize::Md)
                    .indicator(circle_probe(mds_for_view.clone()))
                    .into_any_element(),
            )
            .child(
                RadioGroup::new("sz-radio-sm", vec![RadioOption::new("Option")])
                    .size(RadioSize::Sm)
                    .indicator(circle_probe(sms_for_view.clone()))
                    .into_any_element(),
            )
            .into_any_element()
    });
    settle(cx);

    let default = last_circle(&defaults, "sz-radio-default");
    let md = last_circle(&mds, "sz-radio-md");
    let sm = last_circle(&sms, "sz-radio-sm");
    assert_eq!(
        md.size, default.size,
        "RadioGroup: the default and Md circles must be identical"
    );
    // The default-vs-Md identity is exact on purpose: identical layout must
    // produce bit-identical boxes, so the comparison tolerates no epsilon.
    #[allow(clippy::float_cmp)]
    {
        assert_eq!(
            md.origin.x, default.origin.x,
            "RadioGroup: the default and Md circles must start at the same x"
        );
    }

    assert!(
        near_f32(md.size.width, 16.) && near_f32(md.size.height, 16.),
        "the Md control circle must stay the pinned 16px, got {md:?}"
    );
    assert!(
        near_f32(sm.size.width, 14.) && near_f32(sm.size.height, 14.),
        "the Sm control circle must step to 14px, got {sm:?}"
    );
}

/// A long supporting line determines the intrinsic component width: its shaped
/// width plus the indentation. This measures both actual Description/ErrorMessage
/// paths without inserting a probe that could change their layout.
#[gpui::test]
fn compact_supporting_text_tracks_the_control_and_label_gap(cx: &mut TestAppContext) {
    const SUPPORT: &str = "Supporting text long enough to determine the component width";
    let cx = open_host(cx, || {
        let mut host = gpui::div().flex().flex_col().items_start().gap(px(8.));
        for (size, name) in [
            (CheckboxSize::Sm, "check-sm"),
            (CheckboxSize::Md, "check-md"),
        ] {
            for error in [false, true] {
                let key = format!("{name}-{error}");
                let check = Checkbox::new(key.clone()).size(size).label("A");
                let check = if error {
                    check.is_invalid(true).error_message(SUPPORT)
                } else {
                    check.description(SUPPORT)
                };
                host = host.child(gpui::div().debug_selector(move || key).child(check));
            }
        }
        for (size, name) in [(RadioSize::Sm, "radio-sm"), (RadioSize::Md, "radio-md")] {
            for error in [false, true] {
                let key = format!("{name}-{error}");
                let option = if error {
                    RadioOption::new("A").error_message(SUPPORT)
                } else {
                    RadioOption::new("A").description(SUPPORT)
                };
                host = host.child(
                    gpui::div()
                        .debug_selector({
                            let key = key.clone();
                            move || key
                        })
                        .child(RadioGroup::new(key, vec![option]).size(size)),
                );
            }
        }
        host.child(
            gpui::div()
                .debug_selector(|| "support-reference".to_owned())
                .child(herogpui_components::Description::new(SUPPORT)),
        )
        .into_any_element()
    });
    settle(cx);
    let text = f32::from(box_of(cx, "support-reference").size.width);
    for (name, indent) in [
        ("check-sm", 26.),
        ("check-md", 28.),
        ("radio-sm", 24.),
        ("radio-md", 28.),
    ] {
        for error in [false, true] {
            let key = Box::leak(format!("{name}-{error}").into_boxed_str());
            let bounds = box_of(cx, key);
            assert!(near(bounds.size.width, text + indent), "{key}: supporting text must be indented {indent}px, got {bounds:?} with text width {text}");
        }
    }
}
