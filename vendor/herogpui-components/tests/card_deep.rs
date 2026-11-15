//! Card anatomy and geometry against the pinned v3.2.4 `card.css`.
//!
//! Upstream pins `.card` to `flex flex-col gap-3 p-4` with
//! `border-radius: min(32px, var(--radius-3xl))`, while the `__header`,
//! `__title`, `__description`, `__content` and `__footer` parts carry none
//! of the card's padding or gap.
//! The card's own root is not probeable, so every assertion below is
//! measured through `debug_bounds` probes inside the parts and a sibling
//! probe after the card: part order, the 16px inset on all four sides, the
//! 12px part gap, fixed and default widths, and the part text metrics.
//! Paint-level surfaces (background, border, shadow, radius, overflow
//! clipping) leave no trace in layout and are covered by the `.shots`
//! audits plus code reading, not here.

use crate::harness;

use gpui::{prelude::*, px, Bounds, Pixels, TestAppContext};
use harness::open_host;
use herogpui_components::{
    Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle, CardVariant,
};

/// A fixed-height filler that reports its laid-out bounds under `name`.
/// `w_full` also gives it width inside the footer's flex row.
fn probe(name: &'static str, height: f32) -> gpui::Div {
    gpui::div()
        .h(px(height))
        .w_full()
        .debug_selector(move || name.to_owned())
}

/// A 20px square for measuring row positions inside the footer.
fn square(name: &'static str) -> gpui::Div {
    gpui::div()
        .size(px(20.))
        .debug_selector(move || name.to_owned())
}

/// A one-line text leaf whose measured height is the active line height.
fn line(name: &'static str, text: &'static str) -> gpui::Div {
    gpui::div()
        .debug_selector(move || name.to_owned())
        .child(text)
}

fn need(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} must paint"))
}

/// The full anatomy: header, body, and footer stack in that order inside the
/// card's 16px padding, separated by the pinned 12px gap. With `w(240)` the
/// card is 240x116 (16 + 20 + 12 + 20 + 12 + 20 + 16); the sibling probe
/// after the card starts exactly at its bottom edge.
#[gpui::test]
fn card_stacks_header_body_footer_in_the_pinned_geometry(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .child(
                Card::new()
                    .w(px(240.))
                    .child(CardHeader::new().child(probe("card-h-probe", 20.)))
                    .child(CardContent::new().child(probe("card-b-probe", 20.)))
                    .child(CardFooter::new().child(probe("card-f-probe", 20.))),
            )
            .child(probe("card-after", 20.))
            .into_any_element()
    });

    for (name, y) in [
        ("card-h-probe", 16.),
        ("card-b-probe", 48.),
        ("card-f-probe", 80.),
    ] {
        let part = need(cx, name);
        assert_eq!(
            part.origin.x,
            px(16.),
            "{name} sits behind the 16px padding"
        );
        assert_eq!(
            part.size.width,
            px(208.),
            "{name} spans the 240px card's padded content box"
        );
        assert_eq!(
            part.origin.y,
            px(y),
            "{name} proves the header/body/footer order and the 12px gap"
        );
        assert_eq!(part.size.height, px(20.));
    }
    let after = need(cx, "card-after");
    assert_eq!(
        after.origin.y,
        px(116.),
        "the card must end 16px below its last part: the bottom padding"
    );
    assert_eq!(after.origin.x, px(0.), "the sibling sits outside the card");
    assert_eq!(after.size.width, px(1920.), "the test display is 1920 wide");
}

/// `.card__footer` is `flex flex-row items-center` and nothing else: v3 gives
/// it no gap of its own (the card's gap separates the parts), so two footer
/// children must touch. The port used to invent an 8px gap.
#[gpui::test]
fn card_footer_children_touch_without_an_invented_gap(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        Card::new()
            .w(px(240.))
            .child(CardFooter::new().children([square("card-f-a"), square("card-f-b")]))
            .into_any_element()
    });

    let first = need(cx, "card-f-a");
    let second = need(cx, "card-f-b");
    assert_eq!(
        first.origin.x,
        px(16.),
        "the card padding is the only inset"
    );
    assert_eq!(
        second.origin.x,
        px(36.),
        "the footer row must not add a gap of its own"
    );
    assert_eq!(
        first.origin.y, second.origin.y,
        "items_center aligns the row"
    );
}

/// `.card__header`, `.card__content` and `.card__footer` carry no text style
/// of their own, so plain part text must measure exactly like page text. The
/// pinned leading lives on the dedicated `CardTitle` (`leading-6`) and
/// `CardDescription` (`leading-5`) parts. GPUI's default line box is not an
/// integer and the harness snaps its reported height to the device pixel
/// grid, so every part line and the page reference are measured as the first
/// child of their own card at the same y: identical position, identical
/// rounding.
#[gpui::test]
fn card_part_text_metrics_match_the_pinned_leading(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .child(
                Card::new()
                    .w(px(240.))
                    .child(CardHeader::new().child(line("card-h-text", "Text"))),
            )
            .child(
                Card::new()
                    .w(px(240.))
                    .child(CardContent::new().child(line("card-b-text", "Text"))),
            )
            .child(
                Card::new()
                    .w(px(240.))
                    .child(CardFooter::new().child(line("card-f-text", "Text"))),
            )
            .child(
                Card::new()
                    .w(px(240.))
                    .child(line("card-page-text", "Text")),
            )
            .child(
                Card::new()
                    .w(px(240.))
                    .child(CardTitle::new().child(line("card-title-text", "Title"))),
            )
            .child(
                Card::new()
                    .w(px(240.))
                    .child(CardDescription::new().child(line("card-desc-text", "Description"))),
            )
            .into_any_element()
    });

    assert_eq!(
        need(cx, "card-title-text").size.height,
        px(24.),
        "CardTitle must keep the title's leading-6"
    );
    assert_eq!(
        need(cx, "card-desc-text").size.height,
        px(20.),
        "CardDescription must keep the description's leading-5"
    );
    let page = f32::from(need(cx, "card-page-text").size.height);
    for name in ["card-h-text", "card-b-text", "card-f-text"] {
        let height = f32::from(need(cx, name).size.height);
        assert!(
            (height - page).abs() < 0.01,
            "{name} must not impose a text style of its own: {height} vs page {page}"
        );
    }
}

/// The full v3 anatomy composes six public parts: Root > Header >
/// (Title, Description), Content, Footer. `.card__header` has no gap of its
/// own, so the title and description lines touch inside the header; the
/// card's 12px gap separates header (16 + 24 + 20 = 60), body, and footer.
#[gpui::test]
fn card_composes_the_six_upstream_parts_in_order(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        Card::new()
            .w(px(240.))
            .child(
                CardHeader::new()
                    .child(CardTitle::new().child(line("card-six-title", "Title")))
                    .child(CardDescription::new().child(line("card-six-desc", "Description"))),
            )
            .child(CardContent::new().child(probe("card-six-body", 20.)))
            .child(CardFooter::new().child(probe("card-six-footer", 20.)))
            .into_any_element()
    });

    let title = need(cx, "card-six-title");
    assert_eq!(title.origin.x, px(16.), "the title opens the padded box");
    assert_eq!(title.origin.y, px(16.), "the title is the first part");
    assert_eq!(title.size.height, px(24.), "the title keeps leading-6");

    let desc = need(cx, "card-six-desc");
    assert_eq!(
        desc.origin.y,
        px(40.),
        "the description follows the title with no header gap"
    );
    assert_eq!(desc.size.height, px(20.), "the description keeps leading-5");

    let body = need(cx, "card-six-body");
    assert_eq!(
        body.origin.y,
        px(72.),
        "the 12px card gap separates header (16+24+20=60) from the body"
    );
    assert_eq!(body.size.height, px(20.));

    let footer = need(cx, "card-six-footer");
    assert_eq!(
        footer.origin.y,
        px(104.),
        "the 12px card gap separates the body from the footer"
    );
    assert_eq!(footer.size.height, px(20.));
    assert_eq!(
        footer.size.width,
        px(208.),
        "the footer spans the padded content box"
    );
}

/// `w` is optional: without it the card is a block that fills the host.
#[gpui::test]
fn card_without_w_fills_the_host_width(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .child(Card::new().child(probe("card-h-probe", 20.)))
            .child(probe("card-after", 20.))
            .into_any_element()
    });

    let part = need(cx, "card-h-probe");
    assert_eq!(part.size.width, px(1888.), "1920 minus the 16px padding");
    let after = need(cx, "card-after");
    assert_eq!(
        after.origin.y,
        px(52.),
        "16 + 20 + 16: the card hugs its single part at full width"
    );
}

/// Variants are paint-level choices (`bg-*`, shadow, or nothing): all four
/// must compose identical part geometry in `ALL` order.
#[gpui::test]
fn card_variants_compose_identical_layout(cx: &mut TestAppContext) {
    const PROBES: [&str; 4] = [
        "card-v-transparent-h",
        "card-v-default-h",
        "card-v-secondary-h",
        "card-v-tertiary-h",
    ];
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .children(CardVariant::ALL.iter().zip(PROBES).map(|(variant, name)| {
                Card::new()
                    .variant(*variant)
                    .w(px(240.))
                    .child(CardHeader::new().child(probe(name, 20.)))
                    .into_any_element()
            }))
            .into_any_element()
    });

    for (i, name) in PROBES.iter().enumerate() {
        let header = need(cx, name);
        assert_eq!(header.origin.x, px(16.), "variant {i} keeps the 16px inset");
        assert_eq!(
            header.size.width,
            px(208.),
            "variant {i} spans the padded content box"
        );
        assert_eq!(
            header.origin.y,
            px(16. + (i as f32) * 52.),
            "variant {i} stacks with no extra spacing and hugs its parts"
        );
    }
}
