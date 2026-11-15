//! Headless coverage for the divider a `Toolbar` draws between its groups.
//!
//! `.toolbar` restyles the separator that crosses its flow rather than leaving
//! it to `separator.css`:
//!
//! ```css
//! .toolbar {
//!   .separator--vertical   { @apply h-1/2 self-center; }
//!   .separator--horizontal { @apply w-1/2 justify-center justify-self-center; }
//! }
//! ```
//!
//! So a bar's divider is half the bar's cross size and centred in it — an 18px
//! tick in v3's 36px bar — not a line down the bar's whole edge. The port drew
//! the full edge, which read as a hard division of the group rather than a
//! hint between two, and was at its most obvious on `is_attached`, where the
//! rule ran into the rounded pill's padding at both ends.
//!
//! Only the length and the centring are measurable headlessly: gpui 0.2.2's
//! test API reports painted geometry and nothing else, so the divider's fill
//! (`--separator`) and its corner radius belong to the pinned stylesheet and
//! the static audits, which read both.

use crate::harness;

use gpui::{div, prelude::*, px, TestAppContext, VisualTestContext};
use harness::open_host;
use herogpui_components::Toolbar;
use herogpui_core::Orientation;

/// Pushes the pending frame through before `debug_bounds` reads it.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// Geometry comparisons sit inside a tolerance instead of `==` because
/// `float_cmp` is denied and layout rounds to whole pixels anyway.
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

/// The bar's controls, standing in for a button group: a definite block is
/// what gives the bar a cross size for the divider to take half of.
fn control(name: &'static str, w: f32, h: f32) -> gpui::Div {
    div()
        .debug_selector(move || name.to_owned())
        .w(px(w))
        .h(px(h))
}

/// A horizontal bar is divided by a vertical rule: half the bar's height,
/// centred on it. The bar is 40px tall here, so the tick is 20px with 10px of
/// clear space above and below — and it is *not* the 40px the full-edge rule
/// drew.
#[gpui::test]
fn a_horizontal_toolbars_divider_is_half_its_height_and_centred(cx: &mut TestAppContext) {
    let bar_h = 40.;
    let cx = open_host(cx, move || {
        div()
            .w(px(300.))
            .child(
                Toolbar::new()
                    .id("tb-h")
                    .orientation(Orientation::Horizontal)
                    .child(control("tb-h-left", 60., bar_h))
                    .separator()
                    .child(control("tb-h-right", 60., bar_h)),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let slot = cx
        .debug_bounds("toolbar-separator")
        .expect("the divider's slot must be laid out");
    let mark = cx
        .debug_bounds("toolbar-separator-mark")
        .expect("the divider's mark must be laid out");

    assert!(
        near(f32::from(slot.size.height), bar_h),
        "the slot spans the bar's height so the mark has something to halve, \
         got {:?} against {bar_h}",
        slot.size.height,
    );
    assert!(
        near(f32::from(mark.size.height), bar_h / 2.),
        "`.toolbar .separator--vertical` is `h-1/2`, so a {bar_h}px bar's \
         divider is {}px, got {:?}",
        bar_h / 2.,
        mark.size.height,
    );
    assert!(
        near(
            f32::from(mark.origin.y) - f32::from(slot.origin.y),
            bar_h / 4.,
        ),
        "`self-center` leaves a quarter of the bar clear above the divider, \
         got {:?} below a slot at {:?}",
        mark.origin.y,
        slot.origin.y,
    );
    assert!(
        near(
            f32::from(slot.origin.y) + f32::from(slot.size.height),
            f32::from(mark.origin.y) + f32::from(mark.size.height) + bar_h / 4.,
        ),
        "the clear space below the divider matches the space above it",
    );
}

/// A vertical bar is divided by a horizontal rule, so the same rule applies to
/// the other axis: `w-1/2`, centred across the bar's width.
#[gpui::test]
fn a_vertical_toolbars_divider_is_half_its_width_and_centred(cx: &mut TestAppContext) {
    let bar_w = 80.;
    let cx = open_host(cx, move || {
        div()
            .w(px(bar_w))
            .child(
                Toolbar::new()
                    .id("tb-v")
                    .orientation(Orientation::Vertical)
                    .child(control("tb-v-top", bar_w, 24.))
                    .separator()
                    .child(control("tb-v-bottom", bar_w, 24.)),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let slot = cx
        .debug_bounds("toolbar-separator")
        .expect("the divider's slot must be laid out");
    let mark = cx
        .debug_bounds("toolbar-separator-mark")
        .expect("the divider's mark must be laid out");

    assert!(
        near(f32::from(slot.size.width), bar_w),
        "the slot spans the bar's width, got {:?} against {bar_w}",
        slot.size.width,
    );
    assert!(
        near(f32::from(mark.size.width), bar_w / 2.),
        "`.toolbar .separator--horizontal` is `w-1/2`, so a {bar_w}px bar's \
         divider is {}px, got {:?}",
        bar_w / 2.,
        mark.size.width,
    );
    assert!(
        near(
            f32::from(mark.origin.x) - f32::from(slot.origin.x),
            bar_w / 4.,
        ),
        "`justify-self-center` leaves a quarter of the bar clear to the left, \
         got {:?} beside a slot at {:?}",
        mark.origin.x,
        slot.origin.x,
    );
}

/// The divider crosses whichever way the bar runs, and `Toolbar::separator`
/// takes that from the bar rather than the caller — the orientation cannot be
/// passed, so it cannot be passed wrongly. A vertical divider in a vertical
/// bar would be a rule *along* the flow, dividing nothing.
#[gpui::test]
fn a_toolbars_divider_crosses_the_bars_own_flow(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        div()
            .w(px(300.))
            .child(
                Toolbar::new()
                    .id("tb-cross")
                    .orientation(Orientation::Horizontal)
                    .child(control("tb-cross-a", 40., 32.))
                    .separator()
                    .child(control("tb-cross-b", 40., 32.)),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let mark = cx
        .debug_bounds("toolbar-separator-mark")
        .expect("the divider's mark must be laid out");

    assert!(
        f32::from(mark.size.height) > f32::from(mark.size.width),
        "a horizontal bar's divider stands upright, got {:?}",
        mark.size,
    );

    let a = cx
        .debug_bounds("tb-cross-a")
        .expect("the first control must be laid out");
    let b = cx
        .debug_bounds("tb-cross-b")
        .expect("the second control must be laid out");
    assert!(
        f32::from(a.origin.x) < f32::from(mark.origin.x)
            && f32::from(mark.origin.x) < f32::from(b.origin.x),
        "the divider sits between the two groups it divides, got {:?} between \
         {:?} and {:?}",
        mark.origin.x,
        a.origin.x,
        b.origin.x,
    );
}

/// `is_attached` keeps the halving. This is the arrangement the full-edge rule
/// looked worst in: `.toolbar--attached` is `p-1`, so a full-height rule ran
/// straight into the pill's own padding and touched its rounded edge.
#[gpui::test]
fn an_attached_toolbars_divider_stays_clear_of_the_pills_padding(cx: &mut TestAppContext) {
    let control_h = 32.;
    // `.toolbar--attached` is `p-1` — four pixels on each side.
    let pad = 4.;
    let cx = open_host(cx, move || {
        div()
            .w(px(300.))
            .debug_selector(|| "tb-att-wrap".to_owned())
            .child(
                Toolbar::new()
                    .id("tb-att")
                    .is_attached(true)
                    .child(control("tb-att-a", 40., control_h))
                    .separator()
                    .child(control("tb-att-b", 40., control_h)),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let slot = cx
        .debug_bounds("toolbar-separator")
        .expect("the divider's slot must be laid out");
    let mark = cx
        .debug_bounds("toolbar-separator-mark")
        .expect("the divider's mark must be laid out");
    let a = cx
        .debug_bounds("tb-att-a")
        .expect("the first control must be laid out");

    assert!(
        near(f32::from(slot.size.height), control_h),
        "the padded bar's content box is the controls' height, got {:?} \
         against {control_h}",
        slot.size.height,
    );
    assert!(
        near(f32::from(mark.size.height), control_h / 2.),
        "the attached bar halves its divider too, got {:?} against {}",
        mark.size.height,
        control_h / 2.,
    );
    // The pill's edge is `pad` above the controls; the divider now starts a
    // further quarter of the content box down, so the two never meet.
    let pill_top = f32::from(a.origin.y) - pad;
    assert!(
        f32::from(mark.origin.y) - pill_top > pad,
        "the divider clears the pill's {pad}px padding, got a mark at {:?} \
         against a pill edge at {pill_top}",
        mark.origin.y,
    );
}
