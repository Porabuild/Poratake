//! Per-component `radius` builders, proved on the painted scene rather than
//! on source text: without the builder the component paints its owning
//! `util::*_radius` helper's value, and with it the instance radius replaces
//! that value on the painted box.

use crate::harness;

use gpui::{prelude::*, px, AnyElement, App, Pixels, TestAppContext};
use harness::{open_host, still};
use herogpui_components::extend::{
    container_radius, control_radius, hairline_radius, key_radius, mark_radius, small_radius,
    soft_radius,
};
use herogpui_components::{
    Accordion, AccordionItem, AccordionVariant, Alert, Avatar, Badge, Button, Card, Checkbox, Chip,
    CloseButton, Kbd, Meter, ProgressBar, RadioGroup, RadioOption, Skeleton, Tag, TagGroup,
    ToggleButton, Typography,
};

/// An override no helper or token produces, and small enough that no box
/// under test clamps it to half its side.
const OVERRIDE: f32 = 1.75;

type Build = fn(Option<Pixels>) -> AnyElement;
type Helper = fn(&App) -> Pixels;

fn with<T>(el: T, radius: Option<Pixels>, set: impl FnOnce(T, Pixels) -> T) -> T {
    match radius {
        Some(r) => set(el, r),
        None => el,
    }
}

/// `(name, owning helper, build)`. Components whose default follows a size
/// step name the helper of their default `Md` step (Avatar, Badge,
/// ProgressBar, Meter, TagGroup); Meter delegates to ProgressBar's.
fn cases() -> Vec<(&'static str, Helper, Build)> {
    vec![
        ("Button", control_radius, |r| {
            with(Button::new("b").label("Save"), r, Button::radius).into_any_element()
        }),
        ("Card", container_radius, |r| {
            with(Card::new().child("Body"), r, Card::radius).into_any_element()
        }),
        ("Chip", soft_radius, |r| {
            with(Chip::new().child("Chip"), r, Chip::radius).into_any_element()
        }),
        ("Kbd", key_radius, |r| {
            with(Kbd::new().child("K"), r, Kbd::radius).into_any_element()
        }),
        ("ToggleButton", control_radius, |r| {
            with(
                ToggleButton::new("t").label("Bold"),
                r,
                ToggleButton::radius,
            )
            .into_any_element()
        }),
        ("Alert", control_radius, |r| {
            with(Alert::new("Heads up"), r, Alert::radius).into_any_element()
        }),
        ("CloseButton", small_radius, |r| {
            with(CloseButton::new("c"), r, CloseButton::radius).into_any_element()
        }),
        ("Checkbox", mark_radius, |r| {
            with(Checkbox::new("cb"), r, Checkbox::radius).into_any_element()
        }),
        ("RadioGroup", key_radius, |r| {
            with(
                RadioGroup::new("rg", vec![RadioOption::new("One")]),
                r,
                RadioGroup::radius,
            )
            .into_any_element()
        }),
        ("Typography::code", mark_radius, |r| {
            with(Typography::code("let x"), r, Typography::radius).into_any_element()
        }),
        ("Skeleton", hairline_radius, |r| {
            with(Skeleton::new().w(px(80.)).h(px(20.)), r, Skeleton::radius).into_any_element()
        }),
        ("Accordion (Surface)", container_radius, |r| {
            with(
                Accordion::new(vec![AccordionItem::new("a", "Section")])
                    .variant(AccordionVariant::Surface),
                r,
                Accordion::radius,
            )
            .into_any_element()
        }),
        ("Avatar", control_radius, |r| {
            with(Avatar::new("av").name("Ada Lovelace"), r, Avatar::radius).into_any_element()
        }),
        ("ProgressBar", hairline_radius, |r| {
            with(ProgressBar::new("p").value(40.), r, ProgressBar::radius).into_any_element()
        }),
        ("Meter", hairline_radius, |r| {
            with(Meter::new("m", 40.), r, Meter::radius).into_any_element()
        }),
        ("Badge", control_radius, |r| {
            with(Badge::new().child("3"), r, Badge::radius).into_any_element()
        }),
        ("TagGroup", small_radius, |r| {
            with(
                TagGroup::new("tg", vec![Tag::new("a", "Alpha")]),
                r,
                TagGroup::radius,
            )
            .into_any_element()
        }),
    ]
}

/// All four corners at `value`, after the clamp to half the shorter side
/// that the painter applies to an oversized radius (a pill).
fn uniform(quad: &gpui::Quad, scale: f32, value: f32) -> bool {
    let size = quad.bounds.size;
    let value = value.min(size.width.0.min(size.height.0) / scale / 2.);
    let c = quad.corner_radii;
    [c.top_left, c.top_right, c.bottom_right, c.bottom_left]
        .iter()
        .all(|corner| (corner.0 / scale - value).abs() < 0.05)
}

/// Every painted quad's four corners, for one render of `build(radius)`,
/// plus the helper's value read under the same theme.
fn paint(
    cx: &mut TestAppContext,
    build: Build,
    radius: Option<Pixels>,
    helper: Helper,
) -> (Vec<gpui::Quad>, f32, f32) {
    still();
    let vcx = open_host(cx, move || {
        gpui::div()
            .p(px(24.))
            .w(px(480.))
            .child(build(radius))
            .into_any_element()
    });
    for _ in 0..3 {
        vcx.update(|window, _| window.refresh());
        vcx.run_until_parked();
    }
    vcx.update(|window, cx| {
        (
            window.painted_quads(),
            window.scale_factor(),
            f32::from(helper(cx)),
        )
    })
}

#[gpui::test]
fn the_override_replaces_the_painted_helper_radius(cx: &mut TestAppContext) {
    let mut checked = 0;
    for (name, helper, build) in cases() {
        let (quads, scale, value) = paint(cx, build, None, helper);
        let helper_boxes = quads.iter().filter(|q| uniform(q, scale, value)).count();
        assert!(
            !quads.iter().any(|q| uniform(q, scale, OVERRIDE)) && helper_boxes > 0,
            "{name}: the default must paint the helper's {value}px, not the override"
        );
        checked += 1;

        // The override must land on a box that painted the helper's value,
        // not on an extra one: fewer helper-valued boxes remain.
        let (quads, scale, _) = paint(cx, build, Some(px(OVERRIDE)), helper);
        let remaining = quads.iter().filter(|q| uniform(q, scale, value)).count();
        assert!(
            quads.iter().any(|q| uniform(q, scale, OVERRIDE)) && remaining < helper_boxes,
            "{name}: `.radius({OVERRIDE})` must replace the painted {value}px \
             ({helper_boxes} helper boxes before, {remaining} after)"
        );
        checked += 1;
    }
    assert_eq!(checked, 34, "all 17 cases ran both of their checks");
}
