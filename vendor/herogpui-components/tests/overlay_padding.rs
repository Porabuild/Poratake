//! Overlay panel padding measured on the RESTING panel.
//!
//! `Popover::padding` / `Toast::padding` resolve one pair that the resting
//! panel's chain and its entry-animation `ZoomBox` both consume. Under
//! reduced motion the zoom returns the panel untouched, so the chain is the
//! only consumer there — the reduced-motion bounds test holds the resting
//! panel to the resolved pair. With motion on, the popover's entry zoom and
//! the toast's stack-exit zoom are observed on the executor clock and must paint
//! the same pair.
//!
//! The popover panel carries no debug selector of its own, so the probe is
//! caller content inside it. The panel is a `flex_col` whose padding is
//! symmetric, so a widthless probe stretched across the content box answers
//! the x inset twice over: its width is the pinned 260px panel width minus
//! both x insets. Its top edge sits one y inset below the panel's top edge,
//! which is the measured trigger bottom plus the documented 8px `offset`
//! (Bottom placement, no arrow, and no flip — the host window is tall
//! enough that the panel never turns upward).

use crate::harness;

use gpui::{prelude::*, px, TestAppContext, VisualTestContext};
use herogpui_components::Popover;

use harness::open_host;

/// The popover panel's pinned width (`.w(px(260.))`, `Popover::render`).
const PANEL_WIDTH: f32 = 260.;
/// The positioner's default `offset` ("8px in v3", `Popover::new`).
const OFFSET: f32 = 8.;

fn near(value: f32, expected: f32) -> bool {
    (value - expected).abs() < 0.5
}

/// A zero-behaviour 20px content block with no width of its own, so the
/// panel's cross-axis stretch makes its measured width the panel's content
/// width.
fn panel_probe(name: &'static str) -> gpui::AnyElement {
    gpui::div()
        .h(px(20.))
        .debug_selector(move || name.to_owned())
        .into_any_element()
}

/// A fixed 40x36 trigger block whose box is measurable, so the panel's
/// placement arithmetic never rests on an assumed trigger size.
fn trigger_probe(name: &'static str) -> gpui::AnyElement {
    gpui::div()
        .w(px(40.))
        .h(px(36.))
        .debug_selector(move || name.to_owned())
        .into_any_element()
}

/// One controlled-open popover over a fixed trigger. `wide` picks between
/// the `.padding(px(24.))` instance and the stock default.
fn popover(id: &'static str, wide: bool) -> Popover {
    let popover = Popover::new(trigger_probe(Box::leak(
        format!("trig-{id}").into_boxed_str(),
    )))
    .id(id)
    .is_open(true)
    .should_flip(false);
    let popover = if wide {
        popover.padding(px(24.))
    } else {
        popover
    };
    popover.child(panel_probe(Box::leak(
        format!("probe-{id}").into_boxed_str(),
    )))
}

/// Asserts the probe spans the pinned panel width minus both x insets —
/// symmetric `.px()` padding makes that the x inset exactly — and sits one
/// `padding_y` below the panel's top edge (Bottom placement, the measured
/// trigger bottom plus `offset`). The horizontal origin is deliberately not
/// asserted: a panel centred on a trigger near the left edge is clamped into
/// the viewport, which is placement policy `placement.rs` already pins.
fn assert_resting_insets(cx: &mut VisualTestContext, id: &str, padding_x: f32, padding_y: f32) {
    let trigger = cx
        .debug_bounds(Box::leak(format!("trig-{id}").into_boxed_str()))
        .unwrap_or_else(|| panic!("the {id} trigger must paint"));
    let probe = cx
        .debug_bounds(Box::leak(format!("probe-{id}").into_boxed_str()))
        .unwrap_or_else(|| panic!("the open {id} panel's content must paint"));
    assert!(
        near(f32::from(probe.size.width), PANEL_WIDTH - 2. * padding_x),
        "{id}: the stretched probe must span the pinned panel width minus \
         both x insets, got {} (probe {probe:?})",
        f32::from(probe.size.width)
    );
    let panel_top = f32::from(trigger.origin.y) + f32::from(trigger.size.height) + OFFSET;
    let y_inset = f32::from(probe.origin.y) - panel_top;
    assert!(
        near(y_inset, padding_y),
        "{id}: the resting panel's y inset must be the resolved padding, got \
         {y_inset} (probe {probe:?}, trigger {trigger:?})"
    );
}

fn assert_both_popovers(cx: &mut VisualTestContext) {
    // The stock default is v3's `.popover__dialog` `p-4` — 16 on both axes —
    // which is what proves the chain and the zoom resolve one pair.
    assert_resting_insets(cx, "plain", 16., 16.);
    assert_resting_insets(cx, "wide", 24., 24.);
}

#[gpui::test]
fn popover_padding_reaches_the_resting_panel_with_reduced_motion(cx: &mut TestAppContext) {
    // Under reduced motion `entering_zoom` returns the panel untouched, so
    // the chain below is the only thing that can carry the padding.
    harness::still();
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .child(popover("plain", false))
            .child(popover("wide", true))
            .into_any_element()
    });
    cx.update(|window, _| window.refresh());
    assert_both_popovers(cx);
}

/// The motion path, observed on the executor clock: gpui's `with_animation`
/// measures background-executor time, so the test advances the executor past
/// the entry. The zoom sets the panel's padding itself on every
/// frame, including the settled one, so a zoom fed a second resolution of the
/// padding (the stock 16px instead of the instance's 24px) would land the
/// panel on the wrong inset. Mid-entry the panel is still scaled down, which
/// shows the zoom, not the resting chain alone, is on this path.
#[gpui::test]
fn popover_entry_zoom_lands_on_the_resolved_padding(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .child(popover("plain", false))
            .child(popover("wide", true))
            .into_any_element()
    });
    cx.run_until_parked();
    let probe = cx
        .debug_bounds("probe-wide")
        .expect("the entering panel's content must paint");
    let entering = f32::from(probe.size.width);
    assert!(
        entering < PANEL_WIDTH - 2. * 24. - 1.,
        "mid-entry the zoom must still be scaling the panel and its 24px pair \
         down, the content measured {entering}px wide"
    );
    harness::wait_ms(cx, 400);
    assert_both_popovers(cx);
}

/// The toast half: a card that leaves from behind the front one exits
/// through the stack zoom, which re-applies the card's padding pair from its
/// first frame. A zoom fed the stock `px-4 py-3` instead of the instance's
/// 24px would make the card's content jump inward the moment it starts to
/// leave. The probe is the caller-owned indicator, `p-1` inside the card.
#[gpui::test]
fn toast_stack_exit_zoom_starts_from_the_resolved_padding(cx: &mut TestAppContext) {
    use std::time::Duration;

    use herogpui_components::{toast_store, Toast, ToastStore, ToastViewport};

    let older = cx.update(|cx| {
        let older = Toast::new("Older")
            .timeout(Duration::ZERO)
            .padding(px(24.))
            .indicator_content(|_| harness::probe("older-ind"))
            .push(None, cx);
        Toast::new("Newer")
            .timeout(Duration::ZERO)
            .padding(px(24.))
            .push(None, cx);
        older
    });
    let cx = open_host(cx, || {
        ToastViewport::new().is_expanded(true).into_any_element()
    });
    harness::wait_ms(cx, 500);

    let inset = |cx: &mut VisualTestContext| {
        let probe = cx
            .debug_bounds("older-ind")
            .expect("the older card's indicator must paint");
        let scene = harness::painted(cx);
        let card = scene
            .around(probe)
            .into_iter()
            .filter(|q| q.background.as_solid().is_some_and(|c| c.a > 0.))
            .min_by(|a, b| {
                let area = |q: &&gpui::Quad| q.bounds.size.width.0 * q.bounds.size.height.0;
                area(a).total_cmp(&area(b))
            })
            .map(|q| scene.bounds(q))
            .expect("the older card must paint its surface");
        (
            f32::from(probe.origin.x - card.origin.x),
            f32::from(probe.origin.y - card.origin.y),
        )
    };
    let resting = inset(cx);
    assert!(
        (resting.0 - 28.).abs() < 0.5 && (resting.1 - 28.).abs() < 0.5,
        "the resting card must inset its content by 24 + 4, got {resting:?}"
    );

    cx.update(|_, cx| {
        let store = toast_store(cx);
        ToastStore::close(&store, older, cx);
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let leaving = inset(cx);
    assert!(
        (leaving.0 - resting.0).abs() < 1.5 && (leaving.1 - resting.1).abs() < 1.5,
        "the exit zoom must start from the card's own padding pair: resting \
         {resting:?}, first exit frame {leaving:?}"
    );
}
