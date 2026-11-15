//! Typography and Prose against the pinned v3.2.4 `typography.css`.
//!
//! Upstream resolves each `type` through Tailwind 4.3.0's default text scale
//! plus explicit `leading-*` utilities: h1 `text-4xl` (36/40), h2 `text-3xl`
//! (30/36), h3 `text-2xl` (24/32), h4 `text-xl` (20/28), h5 `text-lg` (18/28),
//! h6 `text-base` (16/24), body `text-base leading-7` (16/28), body-sm
//! `text-sm leading-6` (14/24), body-xs `text-xs leading-5` (12/20) and code
//! `text-sm` (14/20). Headings add `font-semibold`; `.typography--code` adds
//! the `rounded-md bg-default px-1.5 py-0.5 font-mono` chip paint; and
//! `.typography-prose` is a plain block that sets `text-foreground` and
//! nothing else.
//!
//! Text runs are read through a canvas probe child, which sees the merged
//! `window.text_style()` inside the component's own style scope; geometry is
//! measured through `debug_bounds` probes like the card suite. The chip's
//! background colour and radius leave no trace in layout and are covered by
//! code reading plus the design audit, not here.

use crate::harness;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    canvas, prelude::*, px, AbsoluteLength, Bounds, Hsla, Pixels, TestAppContext, TextAlign,
    VisualTestContext, WhiteSpace,
};
use harness::open_host;
use herogpui_components::{
    FontWeight, ParagraphSize, Prose, TextAlign as Align, TextColor, Typography, TypographyType,
};
use herogpui_theme::ActiveTheme;

/// What the merged text style looks like inside a rendered `Typography`.
#[derive(Clone, Debug)]
struct Run {
    size: f32,
    line_height: f32,
    weight: f32,
    family: gpui::SharedString,
    color: Hsla,
    align: TextAlign,
    nowrap: bool,
    ellipsis: bool,
}

/// A zero-size canvas that records the inherited text style at paint time,
/// which is where the owning `Typography` has pushed its refinement.
fn run_probe(out: Rc<RefCell<Option<Run>>>) -> gpui::Canvas<()> {
    canvas(
        |_, _, _| {},
        move |_, _, window, _| {
            let style = window.text_style();
            let rem = window.rem_size();
            *out.borrow_mut() = Some(Run {
                size: f32::from(style.font_size.to_pixels(rem)),
                line_height: f32::from(
                    style
                        .line_height
                        .to_pixels(AbsoluteLength::Pixels(rem), rem),
                ),
                weight: style.font_weight.0,
                family: style.font_family.clone(),
                color: style.color,
                align: style.text_align,
                nowrap: style.white_space == WhiteSpace::Nowrap,
                ellipsis: style.text_overflow.is_some(),
            });
        },
    )
    .size_0()
}

/// A fixed-height filler that reports its laid-out bounds under `name`.
fn probe_div(name: &'static str, height: f32) -> gpui::Div {
    gpui::div()
        .h(px(height))
        .w_full()
        .debug_selector(move || name.to_owned())
}

fn need(cx: &mut VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} must paint"))
}

fn run_of(probe: &Rc<RefCell<Option<Run>>>) -> Run {
    probe
        .borrow()
        .clone()
        .unwrap_or_else(|| panic!("probe must paint"))
}

/// Asserts a measured `(size, line-height, weight)` triple against pinned
/// pixel constants. Compared as formatted values because the repo denies
/// `clippy::float_cmp` and every expected number is an exact whole pixel.
fn assert_metrics(run: &Run, size: f32, line_height: f32, weight: f32, what: &str) {
    assert_eq!(
        format!("{}/{}/{}", run.size, run.line_height, run.weight),
        format!("{size}/{line_height}/{weight}"),
        "{what}"
    );
}

/// `(size, line-height, weight)` for every documented type, resolved from the
/// tagged `typography.css` through the Tailwind 4.3.0 default theme.
fn scale() -> Vec<(TypographyType, f32, f32, f32)> {
    vec![
        (TypographyType::H1, 36., 40., 600.),
        (TypographyType::H2, 30., 36., 600.),
        (TypographyType::H3, 24., 32., 600.),
        (TypographyType::H4, 20., 28., 600.),
        (TypographyType::H5, 18., 28., 600.),
        (TypographyType::H6, 16., 24., 600.),
        (TypographyType::Body, 16., 28., 400.),
        (TypographyType::BodySm, 14., 24., 400.),
        (TypographyType::BodyXs, 12., 20., 400.),
        (TypographyType::Code, 14., 20., 400.),
    ]
}

/// The whole documented scale resolves to the pinned pixel pairs.
#[test]
fn metrics_table_matches_the_tagged_css() {
    for (kind, size, line_height, _) in scale() {
        let (measured, measured_line) = kind.metrics();
        assert_eq!(
            format!("{}/{}", f32::from(measured), f32::from(measured_line)),
            format!("{size}/{line_height}"),
            "{kind:?} metrics"
        );
    }
}

/// Every heading level and the body default carry the pinned scale, and
/// headings default to semibold while body copy stays normal.
#[gpui::test]
fn heading_and_body_types_resolve_the_pinned_scale(cx: &mut TestAppContext) {
    let probes: Vec<_> = (0..7).map(|_| Rc::new(RefCell::new(None))).collect();
    let handles = probes.clone();
    let _cx = open_host(cx, move || {
        let mut col = gpui::div().flex().flex_col();
        let levels = [
            (1, TypographyType::H1, "One"),
            (2, TypographyType::H2, "Two"),
            (3, TypographyType::H3, "Three"),
            (4, TypographyType::H4, "Four"),
            (5, TypographyType::H5, "Five"),
            (6, TypographyType::H6, "Six"),
        ];
        for (index, (level, _kind, label)) in levels.into_iter().enumerate() {
            col = col.child(
                Typography::heading(level, label)
                    .child(run_probe(handles[index].clone()))
                    .into_any_element(),
            );
        }
        col.child(
            Typography::new("Body")
                .child(run_probe(handles[6].clone()))
                .into_any_element(),
        )
        .into_any_element()
    });

    for (probe, (kind, size, line_height, weight)) in probes.iter().zip(scale()) {
        let run = run_of(probe);
        assert_metrics(
            &run,
            size,
            line_height,
            weight,
            &format!("{kind:?} rendered run"),
        );
    }
}

/// `Paragraph`'s `size` prop maps onto body, body-sm and body-xs.
#[gpui::test]
fn paragraph_sizes_resolve_the_body_scale(cx: &mut TestAppContext) {
    let probes: Vec<_> = (0..3).map(|_| Rc::new(RefCell::new(None))).collect();
    let handles = probes.clone();
    let _cx = open_host(cx, move || {
        let mut col = gpui::div().flex().flex_col();
        let sizes = [
            (ParagraphSize::Base, TypographyType::Body),
            (ParagraphSize::Sm, TypographyType::BodySm),
            (ParagraphSize::Xs, TypographyType::BodyXs),
        ];
        for (index, (size, _kind)) in sizes.into_iter().enumerate() {
            col = col.child(
                Typography::paragraph(size, "Paragraph")
                    .child(run_probe(handles[index].clone()))
                    .into_any_element(),
            );
        }
        col.into_any_element()
    });

    let body_scale: Vec<_> = scale()
        .into_iter()
        .filter(|(kind, _, _, _)| {
            matches!(
                kind,
                TypographyType::Body | TypographyType::BodySm | TypographyType::BodyXs
            )
        })
        .collect();
    for (probe, (kind, size, line_height, weight)) in probes.iter().zip(body_scale) {
        let run = run_of(probe);
        assert_metrics(
            &run,
            size,
            line_height,
            weight,
            &format!("{kind:?} paragraph run"),
        );
    }
}

/// `Typography.Code` runs in the mono family at 14/20 and the chip's
/// `py-0.5` padding makes its block 24px tall above the next sibling.
#[gpui::test]
fn code_renders_the_mono_chip(cx: &mut TestAppContext) {
    let probe = Rc::new(RefCell::new(None));
    let handle = probe.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(200.))
            .flex()
            .flex_col()
            .child(
                Typography::code("x")
                    .child(run_probe(handle.clone()))
                    .into_any_element(),
            )
            .child(probe_div("code-after", 20.))
            .into_any_element()
    });

    let run = run_of(&probe);
    assert_metrics(&run, 14., 20., 400., "code run");
    assert_eq!(
        run.family, "Consolas",
        "the mono stack resolves to Consolas"
    );

    let after = need(cx, "code-after");
    assert_eq!(
        after.origin.y,
        px(24.),
        "chip height is the 20px leading plus 2px top and bottom py-0.5"
    );
}

/// The `weight` builder overrides the type's default on headings and body.
#[gpui::test]
fn weight_builder_overrides_the_type_default(cx: &mut TestAppContext) {
    let probes: Vec<_> = (0..2).map(|_| Rc::new(RefCell::new(None))).collect();
    let handles = probes.clone();
    let _cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                Typography::heading(1, "Overridden")
                    .weight(FontWeight::Bold)
                    .child(run_probe(handles[0].clone()))
                    .into_any_element(),
            )
            .child(
                Typography::new("Body")
                    .weight(FontWeight::Medium)
                    .child(run_probe(handles[1].clone()))
                    .into_any_element(),
            )
            .into_any_element()
    });

    assert_eq!(
        run_of(&probes[0]).weight.to_string(),
        "700",
        "heading weight override"
    );
    assert_eq!(
        run_of(&probes[1]).weight.to_string(),
        "500",
        "body weight override"
    );
}

/// `align` maps start/center/end onto GPUI's text alignment; `justify` has no
/// GPUI equivalent and falls back to start, as documented.
#[gpui::test]
fn alignment_maps_onto_gpui_text_align(cx: &mut TestAppContext) {
    let alignments = [
        (Align::Start, TextAlign::Left),
        (Align::Center, TextAlign::Center),
        (Align::End, TextAlign::Right),
        (Align::Justify, TextAlign::Left),
    ];
    let probes: Vec<_> = (0..alignments.len())
        .map(|_| Rc::new(RefCell::new(None)))
        .collect();
    let handles = probes.clone();
    let _cx = open_host(cx, move || {
        let mut col = gpui::div().flex().flex_col();
        for (index, (align, _)) in alignments.into_iter().enumerate() {
            col = col.child(
                Typography::new("Aligned")
                    .align(align)
                    .child(run_probe(handles[index].clone()))
                    .into_any_element(),
            );
        }
        col.into_any_element()
    });

    for (probe, (align, want)) in probes.iter().zip(alignments) {
        assert_eq!(run_of(probe).align, want, "{align:?} alignment");
    }
}

/// `color` resolves onto the theme's foreground and muted roles.
#[gpui::test]
fn color_variants_resolve_to_theme_roles(cx: &mut TestAppContext) {
    let probes: Vec<_> = (0..2).map(|_| Rc::new(RefCell::new(None))).collect();
    let handles = probes.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                Typography::new("Default")
                    .child(run_probe(handles[0].clone()))
                    .into_any_element(),
            )
            .child(
                Typography::new("Muted")
                    .color(TextColor::Muted)
                    .child(run_probe(handles[1].clone()))
                    .into_any_element(),
            )
            .into_any_element()
    });

    let (foreground, muted) = cx.update(|_, cx| (cx.colors().foreground, cx.colors().muted));
    assert_eq!(run_of(&probes[0]).color, foreground, "default color");
    assert_eq!(run_of(&probes[1]).color, muted, "muted color");
}

/// `truncate` turns on nowrap plus an ellipsis; the default wraps.
#[gpui::test]
fn truncate_sets_nowrap_and_ellipsis(cx: &mut TestAppContext) {
    let probes: Vec<_> = (0..2).map(|_| Rc::new(RefCell::new(None))).collect();
    let handles = probes.clone();
    let _cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                Typography::new("A long run of text")
                    .truncate(true)
                    .child(run_probe(handles[0].clone()))
                    .into_any_element(),
            )
            .child(
                Typography::new("Wrapping text")
                    .child(run_probe(handles[1].clone()))
                    .into_any_element(),
            )
            .into_any_element()
    });

    let truncated = run_of(&probes[0]);
    assert!(truncated.nowrap, "truncate implies nowrap");
    assert!(truncated.ellipsis, "truncate implies an ellipsis");
    let wrapped = run_of(&probes[1]);
    assert!(!wrapped.nowrap, "the default wraps");
    assert!(!wrapped.ellipsis, "the default does not ellipsize");
}

/// `Prose` is upstream's plain block: `text-foreground` and nothing else.
/// Children stack touching (upstream's preflight gives paragraphs no margin —
/// a port gap would be an invented metric), plain children inherit the
/// default text style, and already-semantic children keep their own metrics.
#[gpui::test]
fn prose_is_a_plain_foreground_block(cx: &mut TestAppContext) {
    let inherited = Rc::new(RefCell::new(None));
    let control = Rc::new(RefCell::new(None));
    let semantic = Rc::new(RefCell::new(None));
    let (inherited_handle, control_handle, semantic_handle) =
        (inherited.clone(), control.clone(), semantic.clone());
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                Prose::new()
                    .child(probe_div("prose-a", 20.))
                    .child(probe_div("prose-b", 20.))
                    .child(run_probe(inherited_handle.clone()))
                    .child(
                        Typography::new("Semantic")
                            .child(run_probe(semantic_handle.clone()))
                            .into_any_element(),
                    ),
            )
            .child(run_probe(control_handle.clone()))
            .into_any_element()
    });

    let first = need(cx, "prose-a");
    let second = need(cx, "prose-b");
    assert!(
        second.origin.y > first.origin.y,
        "children keep their order"
    );
    assert_eq!(
        second.origin.y,
        first.origin.y + first.size.height,
        "prose children touch: upstream adds no margins and no gap"
    );

    let (foreground, _muted) = cx.update(|_, cx| (cx.colors().foreground, cx.colors().muted));
    let plain = run_of(&inherited);
    assert_eq!(plain.color, foreground, "prose sets text-foreground");
    let outside = run_of(&control);
    assert_eq!(
        plain.line_height.to_string(),
        outside.line_height.to_string(),
        "plain prose children inherit the default leading, not an invented one"
    );

    let semantic_run = run_of(&semantic);
    assert_metrics(
        &semantic_run,
        16.,
        28.,
        400.,
        "semantic children keep the body metrics",
    );
}
