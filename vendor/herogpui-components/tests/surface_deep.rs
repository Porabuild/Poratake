//! Headless coverage for the measurable half of `Surface`.
//!
//! GPUI 0.2.2's public test API reports painted geometry (`debug_bounds`) and
//! nothing else: a background fill, border colour, corner radius or shadow
//! cannot be sampled headlessly. Upstream `.surface` is only
//! `relative text-foreground` plus the variant's fill/foreground classes —
//! the docs examples add `flex flex-col gap-3 rounded-3xl p-6` themselves via
//! `className`, so those are example classes, not Surface defaults. What the
//! port bakes in instead is a repository convenience skeleton (`flex
//! flex-col`, defaulting padding and gap to zero, no baked radius) whose
//! geometry the window can truly measure: the zero default inset/gap, the
//! `.padding(..)`/`.gap(..)` overrides, the `flex flex-col` stacking with the
//! caller's child order, and the fact that `transparent` draws no border, so
//! every variant has identical geometry. What stays invisible to geometry —
//! the per-variant `bg-surface*` / `text-*` colours, the corner radius — is
//! painted only, and belongs to the pinned `surface.css` and the static audits.

use crate::harness;

use gpui::{div, prelude::*, px, Div, TestAppContext, VisualTestContext};
use harness::open_host;
use herogpui_components::{Surface, SurfaceVariant};

/// Pushes the pending frame through before `debug_bounds` reads it.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// Geometry comparisons sit inside a tolerance instead of `==` because
/// `float_cmp` is denied and layout rounds to whole pixels anyway.
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

/// A fixed-size block marked for `debug_bounds`; without a width it stretches
/// across the flex column's content box.
fn probe(name: impl Into<String>, width: Option<f32>, height: f32) -> Div {
    let name = name.into();
    let el = div().debug_selector(move || name).h(px(height));
    match width {
        Some(w) => el.w(px(w)),
        None => el,
    }
}

/// Upstream `.surface` carries no inset of its own, so `Surface::new()` must
/// default padding and gap to zero: the first child sits on the box origin,
/// stacked children touch, a width-less child spans the full width, and the
/// wrapper hugs content exactly.
#[gpui::test]
fn surface_default_has_zero_inset_and_zero_gap(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        div()
            .w(px(300.))
            .debug_selector(|| "s-wrap".to_owned())
            .child(Surface::new().children(vec![
                probe("s-c0", Some(60.), 20.),
                probe("s-c1", Some(60.), 20.),
                probe("s-c2", None, 20.),
            ]))
            .into_any_element()
    });
    flush_frame(cx);

    let wrap = cx
        .debug_bounds("s-wrap")
        .expect("the surface wrapper must be laid out");
    let c0 = cx
        .debug_bounds("s-c0")
        .expect("the first child must be laid out");
    let c1 = cx
        .debug_bounds("s-c1")
        .expect("the second child must be laid out");
    let c2 = cx
        .debug_bounds("s-c2")
        .expect("the third child must be laid out");

    assert!(
        near(f32::from(c0.origin.x) - f32::from(wrap.origin.x), 0.)
            && near(f32::from(c0.origin.y) - f32::from(wrap.origin.y), 0.),
        "the first child must sit on the surface's content-box origin (no \
         default padding), got {:?} vs {:?}",
        c0.origin,
        wrap.origin
    );
    assert!(
        near(
            f32::from(c1.origin.y) - (f32::from(c0.origin.y) + f32::from(c0.size.height)),
            0.
        ),
        "stacked children must touch (no default gap), got {c0:?} then {c1:?}",
    );

    assert!(
        near(f32::from(c2.size.width), 300.),
        "a width-less child stretches across the full 300px content box, got {:?}",
        c2.size.width
    );
    assert!(
        near(f32::from(wrap.size.height), 3. * 20.),
        "the wrapper must hug content with zero inset (60px), got {:?}",
        wrap.size.height
    );
}

/// `padding(..)` and `.gap(..)` are repository conveniences, not upstream
/// props (the docs pass `p-6`/`gap-3` through example `className`), but they
/// must stay measurable when called. The `padding` call also exercises the
/// builder's documented `impl Into<Pixels>` shape.
#[gpui::test]
fn surface_padding_and_gap_overrides_are_measurable(cx: &mut TestAppContext) {
    let pad: f32 = 8.;
    let cx = open_host(cx, move || {
        div()
            .w(px(200.))
            .debug_selector(|| "s-narrow-wrap".to_owned())
            .child(Surface::new().padding(pad).gap(px(4.)).children(vec![
                probe("s-n0", Some(60.), 20.),
                probe("s-n1", Some(60.), 20.),
            ]))
            .into_any_element()
    });
    flush_frame(cx);

    let wrap = cx
        .debug_bounds("s-narrow-wrap")
        .expect("the wrapper must be laid out");
    let n0 = cx
        .debug_bounds("s-n0")
        .expect("the first child must be laid out");
    let n1 = cx
        .debug_bounds("s-n1")
        .expect("the second child must be laid out");

    assert!(
        near(f32::from(n0.origin.x) - f32::from(wrap.origin.x), pad)
            && near(f32::from(n0.origin.y) - f32::from(wrap.origin.y), pad),
        "padding(8px) must inset the first child by 8px on the left and top, \
         got {n0:?} vs {wrap:?}",
    );
    assert!(
        near(
            f32::from(n1.origin.y) - (f32::from(n0.origin.y) + f32::from(n0.size.height)),
            4.
        ),
        "gap(4px) must separate the two children, got {n0:?} then {n1:?}",
    );
    assert!(
        near(f32::from(wrap.size.height), 2. * 8. + 2. * 20. + 4.),
        "the wrapper must hug content plus both 8px paddings (60px), got {:?}",
        wrap.size.height
    );
}

/// Every variant draws with the same zero-inset skeleton: `.surface` itself
/// adds nothing geometric, `.surface--transparent` is only `bg-transparent`
/// with no border, and the filled variants differ only in colours geometry
/// cannot see. Each specimen is checked for the zero-inset skeleton
/// independently, and the caller's child order must survive.
/// `Surface::default()` must equal `Surface::new()`, so the specimens go
/// through `Default::default()`.
#[gpui::test]
fn surface_variants_share_one_geometry_and_keep_child_order(cx: &mut TestAppContext) {
    // One row per specimen: the wrapper key, the variant, and its three child
    // keys. `debug_bounds` takes `&'static str` selectors, so the keys are
    // spelled out rather than formatted per probe.
    const SPECIMENS: [(&str, SurfaceVariant, [&str; 3]); 4] = [
        (
            "s-v-transparent",
            SurfaceVariant::Transparent,
            [
                "s-v-transparent-c0",
                "s-v-transparent-c1",
                "s-v-transparent-c2",
            ],
        ),
        (
            "s-v-default",
            SurfaceVariant::Default,
            ["s-v-default-c0", "s-v-default-c1", "s-v-default-c2"],
        ),
        (
            "s-v-secondary",
            SurfaceVariant::Secondary,
            ["s-v-secondary-c0", "s-v-secondary-c1", "s-v-secondary-c2"],
        ),
        (
            "s-v-tertiary",
            SurfaceVariant::Tertiary,
            ["s-v-tertiary-c0", "s-v-tertiary-c1", "s-v-tertiary-c2"],
        ),
    ];
    let cx = open_host(cx, move || {
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .children(SPECIMENS.map(|(wrap, variant, children)| {
                div()
                    .w(px(240.))
                    .debug_selector(move || wrap.to_owned())
                    .child(
                        Surface::default()
                            .variant(variant)
                            .children(children.map(|name| probe(name, Some(40.), 16.))),
                    )
                    .into_any_element()
            }))
            .into_any_element()
    });
    flush_frame(cx);

    for (wrap_key, _variant, child_keys) in SPECIMENS {
        let wrap = cx
            .debug_bounds(wrap_key)
            .unwrap_or_else(|| panic!("{wrap_key} must render without panicking"));
        let children: Vec<_> = child_keys
            .map(|key| {
                cx.debug_bounds(key)
                    .unwrap_or_else(|| panic!("{key} must be laid out"))
            })
            .to_vec();

        assert!(
            near(
                f32::from(children[0].origin.x) - f32::from(wrap.origin.x),
                0.
            ) && near(
                f32::from(children[0].origin.y) - f32::from(wrap.origin.y),
                0.
            ),
            "{wrap_key} must give its first child the surface's content-box \
             origin (variants add no border or padding), got {children:?} vs \
             {wrap:?}",
        );
        assert!(
            near(f32::from(wrap.size.height), 3. * 16.),
            "{wrap_key} must hug content with zero inset (48px), got {:?}",
            wrap.size.height
        );

        for pair in children.windows(2) {
            let (before, after) = (pair[0], pair[1]);
            assert!(
                f32::from(after.origin.y) > f32::from(before.origin.y)
                    && near(f32::from(after.origin.x), f32::from(before.origin.x)),
                "{wrap_key} must stack its children top to bottom in the caller's \
                 order, got {before:?} then {after:?}",
            );
            let gap = f32::from(after.origin.y)
                - (f32::from(before.origin.y) + f32::from(before.size.height));
            assert!(
                near(gap, 0.),
                "{wrap_key} must keep zero gap between its children, got \
                 {before:?} then {after:?}",
            );
        }
    }
}

/// An unspecified `variant` must be `Default`, the `bg-surface` fill — the
/// enum's `Default` impl is part of Surface's public default behaviour.
#[test]
fn surface_variant_defaults_to_default() {
    assert_eq!(SurfaceVariant::default(), SurfaceVariant::Default);
}
