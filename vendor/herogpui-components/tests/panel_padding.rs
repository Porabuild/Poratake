//! Phase 3 overlay panel padding: an unset override keeps v3's stock insets,
//! and a set value resolves both axes on the resting panel.
//!
//! Measured on the painted scene: the panel is the smallest solid-filled quad
//! around a caller-owned probe (the popover's content, the toast's custom
//! indicator), and the probe's offset from that quad's corner is the resolved
//! inset pair. The motion half — the entry/exit zoom interpolating the same
//! pair — is `overlay_padding.rs`.

use crate::harness;

use std::time::Duration;

use gpui::{prelude::*, px, Bounds, Pixels, TestAppContext, VisualTestContext};
use harness::{open_host, painted, probe, still};
use herogpui_components::{Popover, Toast, ToastViewport};

/// `(x, y)` of the probe inside the smallest filled quad that contains it.
fn inset(cx: &mut VisualTestContext, selector: &'static str) -> (f32, f32) {
    let probe: Bounds<Pixels> = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} must be laid out"));
    let scene = painted(cx);
    let panel = scene
        .around(probe)
        .into_iter()
        .filter(|q| q.background.as_solid().is_some_and(|c| c.a > 0.))
        .min_by(|a, b| {
            let area = |q: &&gpui::Quad| q.bounds.size.width.0 * q.bounds.size.height.0;
            area(a).total_cmp(&area(b))
        })
        .unwrap_or_else(|| panic!("{selector}: no painted panel around the probe"));
    let panel = scene.bounds(panel);
    (
        f32::from(probe.origin.x - panel.origin.x),
        f32::from(probe.origin.y - panel.origin.y),
    )
}

fn near(got: (f32, f32), want: (f32, f32)) -> bool {
    (got.0 - want.0).abs() < 0.5 && (got.1 - want.1).abs() < 0.5
}

#[gpui::test]
fn popover_padding_resolves_both_axes_on_the_resting_panel(cx: &mut TestAppContext) {
    for (padding, want) in [(None, (16., 16.)), (Some(24.), (24., 24.))] {
        still();
        let vcx = open_host(cx, move || {
            let popover = Popover::new(gpui::div().w(px(40.)).h(px(36.)))
                .id("pp")
                .is_open(true)
                .should_flip(false)
                .child(probe("pp-content"));
            match padding {
                Some(p) => popover.padding(px(p)),
                None => popover,
            }
            .into_any_element()
        });
        let got = inset(vcx, "pp-content");
        assert!(
            near(got, want),
            "Popover padding {padding:?}: the content must sit {want:?} inside \
             the painted panel, got {got:?}"
        );
    }
}

#[gpui::test]
fn toast_padding_resolves_both_axes_on_the_resting_card(cx: &mut TestAppContext) {
    // v3's `.toast` is `px-4 py-3`; the custom indicator sits in its own
    // `p-1` box at the card's content corner.
    for (padding, want) in [(None, (16. + 4., 12. + 4.)), (Some(24.), (28., 28.))] {
        still();
        cx.update(|cx| {
            let toast = Toast::new("Saved")
                .timeout(Duration::ZERO)
                .indicator_content(|_| probe("toast-ind"));
            match padding {
                Some(p) => toast.padding(px(p)),
                None => toast,
            }
            .push(None, cx);
        });
        let vcx = open_host(cx, || ToastViewport::new().into_any_element());
        let got = inset(vcx, "toast-ind");
        assert!(
            near(got, want),
            "Toast padding {padding:?}: the indicator must sit {want:?} inside \
             the painted card, got {got:?}"
        );
        vcx.update(|_, cx| herogpui_components::clear_toasts(cx));
    }
}
