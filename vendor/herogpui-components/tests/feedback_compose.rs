//! Behaviour tests for three feedback/composition parity gaps:
//!
//! - **Avatar's image-error path** — v3 documents `Avatar.Image.onError`
//!   ("Callback when there's an error loading the image") and
//!   `Avatar.Fallback.delayMs` ("Delay before showing fallback (prevents
//!   flash)"), and its own example drives a deliberately broken URL. This
//!   port used to pick image-vs-initials at build time, so a broken src drew
//!   an empty box. It now loads through a custom gpui image source whose
//!   failure fires `on_error` and arms the fallback, gated by `delay_ms`.
//!
//!   The test platform ships no asset source, so **every** src fails there —
//!   exactly the condition this suite needs. The load is a spawned task, not
//!   a synchronous call: the test executor never advances on its own, so each
//!   test pumps it (`advance_clock`) and forces the frame that observes the
//!   result (`flush_frame`) — a stale frame would answer the probe instead.
//!
//! - **Alert's composed children** — v3's Interactive States: "it can contain
//!   interactive elements like buttons or close buttons", and the v2->v3
//!   migration guide's "With Action Button (End Content)" composes the button
//!   as a child. `Alert` now implements `ParentElement`, so the button is
//!   driven both by the pointer and by the keyboard.
//!
//! - **Badge's default colour** — v3's table gives `color` a default of
//!   `"default"`; the seed was `Danger`. A colour is a theme token, not a
//!   behaviour — nothing a callback can report distinguishes one fill from
//!   another — so this test pins what a badge *can* be probed for (the
//!   anchored badge is inert to the pointer) and the seed change lives in the
//!   source diff, which is what AGENTS.md's "say why rather than asserting on
//!   a colour" asks for.
//!
//! Geometry is derived from the components' own constants (window 1920x1080,
//! as the buttons suite pins):
//!
//! - A `Size::Md` avatar is a 40px box at the origin: centre (20, 20).
//! - A closable/composed Alert is `w_full px-4 py-3`: content's right edge is
//!   1920-16; a composed `Size::Md` Button is 36px tall with `px-4` (16px)
//!   each side and no border, so its centre x is 1904 - 16 - w/2 where w is
//!   the measured label width, and its centre y is 12 + 18 = 30.
//! - A `Size::Md` badge is a 28px box anchored a quarter of its own box (7px)
//!   past the anchor's top-right corner (`translate(25%, -25%)`); a 64px
//!   anchor at the origin puts the badge's box at x 43..71, y -7..21, centre
//!   (57, 7).

use crate::harness;

use std::time::Duration;

use gpui::{
    prelude::*, px, Font, FontFeatures, FontStyle, FontWeight, TestAppContext, VisualTestContext,
};
use herogpui_components::{Alert, Avatar, Badge, BadgeAnchor, BadgeLabel, Button};

use harness::{click, events, open_host, press};

/// Forces the frame that carries the state a just-completed task changed.
/// Events hit-test the *last rendered frame*, and the executor has no frame
/// of its own, so every pump below ends with an explicit refresh.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// The advance width of a label shaped the way the components shape it —
/// gpui's `.SystemUIFont` at 14px MEDIUM is Button's label style, so this
/// measurement is the render's measurement.
fn text_width(system: &gpui::WindowTextSystem, text: &str) -> f32 {
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
    let line = system.shape_line(text.to_owned().into(), px(14.), &[run], None);
    f32::from(line.width)
}

// ---------------------------------------------------------------------------
// Avatar: image-error fallback
// ---------------------------------------------------------------------------

/// The broken src must fire `on_error` exactly once: the first `advance_clock`
/// lets the spawned asset load (and fail) under the test executor, the flush
/// makes the img's loader observe the error, and the next pump runs the latch
/// task that reports it. Later frames — including the one that draws the
/// fallback — must not re-fire it.
#[gpui::test]
fn avatar_src_failure_fires_on_error_once(cx: &mut TestAppContext) {
    let errors = events();
    let recorded = errors.clone();
    let cx = open_host(cx, move || {
        let errors = errors.clone();
        Avatar::new("src-failure")
            .name("Jane Doe")
            // No scheme and a slash: gpui classifies this as an embedded
            // resource, and the test platform has no asset source, so the
            // load fails deterministically — every src does.
            .src("images/avatar-broken.png")
            .on_error(move |_, _| errors.borrow_mut().push("error".into()))
            .into_any_element()
    });

    // Frame 1 drew with the load still pending. Pump the executor so the
    // load fails, then draw the frame whose loader sees the failure.
    cx.executor().advance_clock(Duration::from_millis(1));
    flush_frame(cx);
    flush_frame(cx);
    // The latch task now runs: it marks the error and reports it.
    cx.executor().advance_clock(Duration::from_millis(1));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["error"],
        "a failing src must fire on_error once when the load errors"
    );

    // With no `delay_ms`, the fallback's ready frame draws immediately; the
    // error must not re-fire on any of it, nor on later stale frames.
    flush_frame(cx);
    cx.executor().advance_clock(Duration::from_millis(300));
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["error"],
        "on_error must fire exactly once, not once per frame"
    );
}

/// `delay_ms` holds the fallback back after the failure: `on_error` still
/// fires as soon as the load errors (v3's onError is not gated), while the
/// fallback's ready frame only comes after the window elapses. The gate is
/// the loader returning "loading" instead of the error, which paint alone can
/// show — so this pins the two callback-observable halves: the error is
/// reported promptly and exactly once, and advancing the clock past the
/// window draws the fallback frame without re-firing anything.
#[gpui::test]
fn avatar_delay_ms_holds_the_fallback_until_the_window_elapses(cx: &mut TestAppContext) {
    let errors = events();
    let recorded = errors.clone();
    let cx = open_host(cx, move || {
        let errors = errors.clone();
        Avatar::new("delay")
            .name("NA")
            .src("images/avatar-broken.png")
            .delay_ms(600)
            .on_error(move |_, _| errors.borrow_mut().push("error".into()))
            .into_any_element()
    });

    // The failure is observed and reported while the fallback is still gated.
    cx.executor().advance_clock(Duration::from_millis(1));
    flush_frame(cx);
    flush_frame(cx);
    cx.executor().advance_clock(Duration::from_millis(1));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["error"],
        "the error must be reported as soon as the load fails, before the delay elapses"
    );

    // Most of the way through the window: still exactly one report.
    cx.executor().advance_clock(Duration::from_millis(400));
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["error"],
        "advancing within the delay window must not re-fire on_error"
    );

    // Past the window: the fallback's ready frame draws; still exactly once.
    cx.executor().advance_clock(Duration::from_millis(200));
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["error"],
        "the ready frame and everything after it must not re-fire on_error"
    );
}

// ---------------------------------------------------------------------------
// Alert: composed children
// ---------------------------------------------------------------------------

/// v3 composes an action button as an ordinary Alert child (the migration
/// guide's "With Action Button (End Content)"). The composed button must be
/// both pointer-reachable — a click on it records a press while a click on
/// the alert body records nothing — and keyboard-reachable: one Tab from the
/// host root lands on it, and Space activates it.
#[gpui::test]
fn alert_composed_button_reports_and_takes_focus(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let presses = presses.clone();
        Alert::new("You have no credits left")
            .description("Upgrade to a paid plan to continue")
            .child(
                Button::new("alert-upgrade")
                    .label("Upgrade")
                    .on_press(move |_, _, _| presses.borrow_mut().push("upgrade".into())),
            )
            .into_any_element()
    });

    // The alert is `w_full px-4 py-3`. The composed button ends at the
    // content's right edge (1920 - 16) and is `px-4` (16px) each side around
    // its measured 14px label, so its centre x is 1904 - 16 - w/2; the alert
    // row is `items_start`, so the 36px button starts at the 12px top padding
    // and its centre y is 12 + 18 = 30.
    let w = cx.update(|window, _| text_width(window.text_system(), "Upgrade"));
    let centre_x = 1904. - 16. - w / 2.;

    // Keyboard first, while the focus still sits on the host root: one Tab
    // reaches the button — the only stop inside the alert — and Space
    // activates it through the very click listener the pointer uses.
    press(cx, "tab");
    flush_frame(cx);
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["upgrade"],
        "a Button composed into an Alert must be reachable and activatable by keyboard"
    );

    // Pointer next: a click on the button reports rather than dismissing.
    click(cx, centre_x, 30.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["upgrade", "upgrade"],
        "a Button composed into an Alert must report its press"
    );

    // A press on the alert body must record nothing: only the composed child
    // is interactive.
    click(cx, 500., 30.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["upgrade", "upgrade"],
        "the alert body itself must not report a press"
    );
}

// ---------------------------------------------------------------------------
// Badge: v3 default colour, and its only callback-probeable behaviour
// ---------------------------------------------------------------------------

/// v3's table gives `color` a default of `"default"`; `Badge::new()` used to
/// seed `Danger`. The seed is a theme token — no callback distinguishes one
/// fill from another, which is the "never assert on a colour" rule — so the
/// driveable half is that the badge renders at that default and stays inert:
/// clicks on the badge and on its anchor must record nothing at all.
#[gpui::test]
fn badge_default_color_stays_behind_the_anchor_and_is_inert(cx: &mut TestAppContext) {
    let recorded = events();
    let cx = open_host(cx, move || {
        // A 64px anchor at the origin with the 28px `Size::Md` badge on its
        // top-right corner (`offset = -28/4 = -7`): the badge's box is
        // x 43..71, y -7..21, centre (57, 7); the anchor's centre is (32, 32).
        // The row is flex because the anchor wrapper only hugs its child
        // there (v3's `inline-flex`; a block parent would stretch it).
        gpui::div()
            .flex()
            .child(
                BadgeAnchor::new()
                    .child(gpui::div().w(px(64.)).h(px(64.)).child("anchor"))
                    .child(Badge::new().child(BadgeLabel::new().child("5"))),
            )
            .into_any_element()
    });

    // Centre of the badge box: (57, 7).
    click(cx, 57., 7.);
    // Centre of the anchor: (32, 32).
    click(cx, 32., 32.);
    assert!(
        recorded.borrow().is_empty(),
        "a default badge and its anchor must record nothing when clicked"
    );
}
