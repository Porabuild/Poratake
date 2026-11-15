//! Headless coverage for the measurable half of `Chip`.
//!
//! GPUI 0.2.2's public test API reports painted geometry (`debug_bounds`) and
//! nothing else: a background fill, border colour, corner radius or shadow
//! cannot be sampled headlessly. What geometry *can* hold Chip to is that
//! every variant shares one box — v3.2.4's chip.css declares no border
//! utility at all and `.chip--tertiary` only clears the fill — and that the
//! size paddings step the same label by exact amounts around one line box:
//! 20px at every size — the base `leading-5` compiles to the `--tw-leading`
//! custom property that the size rules' `text-xs`/`text-sm` re-declarations
//! consume instead of their own pairs. The full colour
//! matrix (base rule, colour classes, and the variant/compound rules that
//! resolve `--chip-bg`/`--chip-fg`) is painted only and is pinned by the unit
//! test inside `chip.rs` against the tagged cascade instead.
//!
//! Composition is upstream's too: the `Chip` root renders arbitrary leading
//! and trailing children *in order*, and only the `ChipLabel` part carries
//! the `.chip__label` `px-0.5`. There is no `startContent`-style slot — v3's
//! with-icon demo composes an icon child and a `Chip.Label` sibling.

use crate::harness;

use gpui::{div, prelude::*, px, Bounds, Pixels, TestAppContext, VisualTestContext};
use harness::{click, events, open_host};
use herogpui_components::{Chip, ChipLabel, ChipVariant, CloseButton};
use herogpui_core::Size;

/// Pushes the pending frame through before `debug_bounds` reads it.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// Geometry comparisons sit inside a tolerance instead of `==` because
/// `float_cmp` is denied and layout rounds to whole pixels anyway.
fn near(a: impl Into<f32>, b: f32) -> bool {
    (a.into() - b).abs() < 0.5
}

fn bounds(cx: &mut VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("the {name} chip must be laid out"))
}

/// Every variant shares one borderless box. A border is counted into
/// taffy's border box, so the invented 1px chip border this port used to
/// paint made tertiary 2px wider and taller than its siblings.
#[gpui::test]
fn chip_variants_share_one_borderless_box(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(4.))
            .children(ChipVariant::ALL.map(|variant| {
                div()
                    .flex()
                    .debug_selector(move || format!("{variant:?}"))
                    .child(
                        Chip::new()
                            .child(ChipLabel::new().child("Tag"))
                            .variant(variant),
                    )
            }))
            .into_any_element()
    });
    flush_frame(cx);

    let primary = bounds(cx, "Primary");
    let secondary = bounds(cx, "Secondary");
    let tertiary = bounds(cx, "Tertiary");
    let soft = bounds(cx, "Soft");

    for (name, other) in [
        ("Secondary", &secondary),
        ("Tertiary", &tertiary),
        ("Soft", &soft),
    ] {
        assert!(
            near(other.size.width - primary.size.width, 0.)
                && near(other.size.height - primary.size.height, 0.),
            "the {name} chip must share the primary chip's borderless box, \
             got {other:?} vs {primary:?}",
        );
    }

    // With no border the medium chip is exactly its padding around one
    // `text-xs` line: `py-0.5` (2px) above and below the 20px line — the
    // base `leading-5` sets `--tw-leading`, which the `--md` re-applied
    // `text-xs` consumes instead of its own 16px pair.
    assert!(
        near(primary.size.height, 24.),
        "the medium chip must be py-0.5 around a 20px line box (24px), got {:?}",
        primary.size.height,
    );
}

/// The size rules re-apply `text-*`, but every `text-*` utility declares
/// `line-height: var(--tw-leading, <its pair>)`: the base `leading-5` sets
/// `--tw-leading`, so the 20px line survives `--sm`/`--md`'s `text-xs` and
/// `--lg`'s `text-sm` alike, and the `py-0` / `py-0.5` / `py-1` paddings
/// produce 20 / 24 / 28px heights. The `px-1` → `px-2` step widens the
/// shared 12px label by exactly 8px.
#[gpui::test]
fn chip_sizes_step_by_their_paddings(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(4.))
            .children(Size::ALL.map(|size| {
                div()
                    .flex()
                    .debug_selector(move || format!("{size:?}"))
                    .child(Chip::new().child(ChipLabel::new().child("Tag")).size(size))
            }))
            .into_any_element()
    });
    flush_frame(cx);

    let sm = bounds(cx, "Sm");
    let md = bounds(cx, "Md");
    let lg = bounds(cx, "Lg");

    assert!(
        near(sm.size.height, 20.) && near(md.size.height, 24.) && near(lg.size.height, 28.),
        "the chips must be py-0/py-0.5/py-1 around the one 20px line the base \
         leading-5 sets, got heights {:?} / {:?} / {:?}",
        sm.size.height,
        md.size.height,
        lg.size.height,
    );
    // Only sm→md share the 12px `text-xs` label, so only their width step is
    // pure padding; lg's `text-sm` label is wider on its own.
    assert!(
        near(md.size.width - sm.size.width, 8.),
        "the px-1→px-2 padding step must widen the same 12px label by 8px, got \
         widths {:?} / {:?}",
        sm.size.width,
        md.size.width,
    );
}

/// v3's `ChipRoot` renders its children verbatim in order; the labelled text
/// is one `Chip.Label` part among them. The `gap-0.5` root separates the
/// parts, and the label keeps the 20px `leading-5` line box the base rule
/// sets.
#[gpui::test]
fn chip_children_render_in_order_around_the_label(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        div()
            .flex()
            .child(
                Chip::new()
                    .child(
                        div()
                            .w(px(8.))
                            .h(px(8.))
                            .debug_selector(|| "chip-lead".into()),
                    )
                    .child(ChipLabel::new().child("Information"))
                    .child(
                        div()
                            .w(px(8.))
                            .h(px(8.))
                            .debug_selector(|| "chip-trail".into()),
                    ),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let lead = bounds(cx, "chip-lead");
    let label = bounds(cx, "chip-label");
    let trail = bounds(cx, "chip-trail");
    let root = bounds(cx, "chip");

    assert!(
        lead.origin.x >= root.origin.x
            && trail.origin.x + trail.size.width <= root.origin.x + root.size.width,
        "the composed parts must paint inside the chip root's own box, got root \
         {root:?} with lead at {:?} and trail ending at {:?}",
        lead.origin.x,
        trail.origin.x + trail.size.width,
    );
    assert!(
        lead.origin.x < label.origin.x && label.origin.x < trail.origin.x,
        "the leading dot, the label, and the trailing dot must render in the \
         order they were composed, got x {:?} / {:?} / {:?}",
        lead.origin.x,
        label.origin.x,
        trail.origin.x,
    );
    assert!(
        near(label.origin.x - (lead.origin.x + lead.size.width), 2.)
            && near(trail.origin.x - (label.origin.x + label.size.width), 2.),
        "the gap-0.5 root must separate the composed parts by 2px, got lead→ \
         label {:?} and label→trail {:?}",
        label.origin.x - (lead.origin.x + lead.size.width),
        trail.origin.x - (label.origin.x + label.size.width),
    );
    assert!(
        near(label.size.height, 20.),
        "the label must keep the inherited leading-5 line box (20px), got {:?}",
        label.size.height,
    );
}

/// The `.chip__label` `px-0.5` belongs to `ChipLabel` alone. A bare text
/// child — v3's unwrapped non-string children — takes no label padding, and
/// neither does an arbitrary icon/dot child.
#[gpui::test]
fn label_padding_belongs_only_to_chip_label(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .debug_selector(|| "bare".into())
                    .child(Chip::new().child("Tag")),
            )
            .child(
                div()
                    .flex()
                    .debug_selector(|| "labelled".into())
                    .child(Chip::new().child(ChipLabel::new().child("Tag"))),
            )
            .child(
                div().flex().debug_selector(|| "dotted".into()).child(
                    Chip::new()
                        .child(
                            div()
                                .w(px(8.))
                                .h(px(8.))
                                .debug_selector(|| "chip-dot".into()),
                        )
                        .child(ChipLabel::new().child("i")),
                ),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let bare = bounds(cx, "bare");
    let labelled = bounds(cx, "labelled");
    let dot = bounds(cx, "chip-dot");

    // `px-0.5` is 2px per side around the same text.
    assert!(
        near(labelled.size.width - bare.size.width, 4.),
        "a ChipLabel child must add exactly the px-0.5 label padding over a \
         bare child (4px), got {:?} vs {:?}",
        labelled.size.width,
        bare.size.width,
    );
    assert!(
        near(dot.size.width, 8.) && near(dot.size.height, 8.),
        "an arbitrary dot child must not receive the label padding, got {:?}",
        dot.size,
    );
    assert!(
        near(bare.size.height, labelled.size.height.into()),
        "the two chips must stay one line tall around the same text",
    );
}

/// A composed pressable child is reachable through the chip, while a plain
/// chip — root, label, or inert dot child — records nothing anywhere it can
/// be clicked: the port's Chip has no press surface of its own.
#[gpui::test]
fn plain_chip_children_are_inert(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let close = presses.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                Chip::new()
                    .child(
                        CloseButton::new("chip-x")
                            .on_press(move |_, _, _| close.borrow_mut().push("close".into())),
                    )
                    .child(ChipLabel::new().child("Tag")),
            )
            .child(
                Chip::new()
                    .child(div().w(px(8.)).h(px(8.)))
                    .child(ChipLabel::new().child("Dotted")),
            )
            .into_any_element()
    });
    flush_frame(cx);

    // The composed close button reports exactly once: the `px-2` root starts
    // the 24px close box at x = 8, and the chip — `py-0.5` around its 20px
    // label line — grows to the close box plus that padding: 28px tall,
    // close centre (20, 14).
    click(cx, 20., 14.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["close"],
        "a close affordance composed into a chip must report its press"
    );

    // Neither the plain chip's centre and trailing slot nor its inert dot
    // child records anything: Chip takes no child slot of its own, so a
    // click cannot reach an unregistered surface.
    click(cx, 30., 40.);
    click(cx, 48., 40.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["close"],
        "a plain chip must have nothing to press"
    );
}
