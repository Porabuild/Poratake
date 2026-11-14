//! Every control draws `status-focused` as the paint-time SVG overlay ring.
//!
//! The ring's accent band is rasterised at paint time into a sprite the
//! headless scene does not expose, but an *offset* ring also paints its
//! `ring-offset` gap as a quad (see `harness::Painted::ring_gaps`). So for
//! the offset controls — Switch, ToggleButton, Checkbox — the tests focus the
//! control with Tab and read that band: it exists only when the overlay ring
//! is up, sits `gap` outside the control, and its content mask shows whether
//! a clipping ancestor would cut the ring. An `sx` refinement that leaves no
//! single radius must fall back to the spread-shadow ring, so no overlay band
//! appears.
//!
//! The field family's rings are *not* offset: they are the SVG band alone,
//! with nothing a quad readback can see. Their structure (the ringless
//! chrome, the non-clipping carrier) stays pinned by the scoped source checks
//! at the bottom of this file.

use crate::harness;
use crate::source_scan;

use gpui::{prelude::*, px, AnyElement, TestAppContext};
use harness::{open_host, painted, press, settle, still, Painted};
use herogpui_components::{Checkbox, Switch, ToggleButton};
use herogpui_theme::ActiveTheme;

/// Renders `build()` 24px in, optionally Tabs into it, and returns the scene
/// with the page background and ring gap it was painted under.
fn focused(
    cx: &mut TestAppContext,
    build: fn() -> AnyElement,
    tab: bool,
) -> (Painted, gpui::Hsla, f32) {
    still();
    let vcx = open_host(cx, move || {
        gpui::div()
            .flex()
            .items_start()
            .p(px(24.))
            .child(build())
            .into_any_element()
    });
    settle(vcx);
    if tab {
        press(vcx, "tab");
    }
    let scene = painted(vcx);
    let (bg, gap) = vcx.update(|_, cx| {
        (
            cx.colors().background,
            f32::from(cx.layout().ring_offset_width),
        )
    });
    (scene, bg, gap)
}

/// Asserts the overlay ring is up exactly when focused, and never clipped.
fn assert_overlay_ring(cx: &mut TestAppContext, name: &str, build: fn() -> AnyElement) {
    let (idle, bg, gap) = focused(cx, build, false);
    assert!(
        idle.ring_gaps(bg, gap).is_empty(),
        "{name}: no ring before focus\n{}",
        idle.describe()
    );
    let (scene, _, _) = focused(cx, build, true);
    let rings = scene.ring_gaps(bg, gap);
    assert_eq!(
        rings.len(),
        1,
        "{name}: a focused control must paint one overlay ring\n{}",
        scene.describe()
    );
    assert!(
        !scene.band_is_clipped(rings[0]),
        "{name}: no ancestor may clip the overlay ring {:?}\n{}",
        scene.bounds(rings[0]),
        scene.describe()
    );
}

/// Asserts a focused control whose corners have no single radius paints no
/// overlay band (its ring is the spread shadow).
fn assert_shadow_fallback(cx: &mut TestAppContext, name: &str, build: fn() -> AnyElement) {
    let (scene, bg, gap) = focused(cx, build, true);
    assert!(
        scene.ring_gaps(bg, gap).is_empty(),
        "{name}: asymmetric corners must fall back to the shadow ring\n{}",
        scene.describe()
    );
}

#[gpui::test]
fn switch_rings_a_carrier_around_the_clipping_track(cx: &mut TestAppContext) {
    assert_overlay_ring(cx, "Switch", || Switch::new("sw").into_any_element());
    assert_shadow_fallback(cx, "Switch with an sx corner", || {
        Switch::new("sw")
            .sx(|s| s.rounded_tl(px(3.)))
            .into_any_element()
    });
}

#[gpui::test]
fn toggle_button_rings_without_a_clip(cx: &mut TestAppContext) {
    assert_overlay_ring(cx, "ToggleButton", || {
        ToggleButton::new("tb").label("Bold").into_any_element()
    });
    assert_shadow_fallback(cx, "ToggleButton with an sx corner", || {
        ToggleButton::new("tb")
            .label("Bold")
            .sx(|s| s.rounded_tl(px(3.)))
            .into_any_element()
    });
}

#[gpui::test]
fn checkbox_rings_without_a_clip(cx: &mut TestAppContext) {
    assert_overlay_ring(cx, "Checkbox", || Checkbox::new("cb").into_any_element());
}

// ---------------------------------------------------------------------------
// Remaining source-text checks: the field family's ring is the SVG band
// alone (no offset gap quad), so nothing on the painted scene shows it.
// ---------------------------------------------------------------------------

/// The implementation half of a component source, without its `#[cfg(test)]`
/// module, so a string an assertion looks for cannot be satisfied by a test.
fn implementation(file: &str) -> String {
    source_scan::component_src(file)
        .split("#[cfg(test)]")
        .next()
        .expect("the implementation section is always present")
        .to_owned()
}

#[test]
fn number_field_group_hands_its_ring_to_a_carrier() {
    let source = implementation("number_field.rs");
    assert!(
        source.contains("crate::util::apply_field_chrome_ringless("),
        "the clipping group must take the chrome without a ring"
    );
    assert!(
        source.contains("crate::util::field_ring_carrier("),
        "the ring must hang on the non-clipping carrier outside the clip"
    );
    assert!(
        source.contains("if ramp_owns_ring {"),
        "past its first flip the chrome ramp owns the ring; the carrier must \
         stand down so the group never draws two"
    );
}

#[test]
fn date_field_group_hands_its_ring_to_a_carrier() {
    let source = implementation("date_picker/field.rs");
    assert!(
        source.contains("crate::util::apply_field_chrome_ringless("),
        "the clipping `.date-input-group` must take the chrome without a ring"
    );
    assert!(
        source.contains("crate::util::field_ring_carrier("),
        "the ring must hang on the non-clipping carrier outside the clip"
    );
}

#[test]
fn input_rings_both_spellings_as_an_overlay() {
    let source = implementation("input.rs");
    assert!(
        source.contains("crate::util::apply_field_chrome_ringless("),
        "both Input spellings must take the chrome without an inline ring"
    );
    assert!(
        source.contains("crate::util::with_field_ring_overlay(field, ring, radius, cx)"),
        "the single-line field does not clip, so it hosts the overlay itself"
    );
    assert!(
        source.contains("crate::util::field_ring_carrier(field_element, ring, radius, cx)"),
        "the multi-line field clips its wrapped text, so its ring rides the \
         non-clipping carrier"
    );
}
