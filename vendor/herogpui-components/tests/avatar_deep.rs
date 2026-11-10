//! Avatar runtime behavior against the pinned v3.2.6 contract (HeroUI
//! `avatar.css` + Radix Avatar 1.2.6 fallback semantics).
//!
//! The test platform ships no asset source, so an embedded `src` path fails
//! deterministically and a `gpui::ImageSource::Custom` loader stages the
//! exact pending → success / pending → error sequencing the fallback state
//! machine must survive. The custom fallback is the visibility signal: a
//! canvas probe that flips its flag exactly on the frames where the fallback
//! paints. (`debug_bounds` cannot play this role — gpui's frame clear keeps
//! stale selector bounds, so a removed element would still "be there".)

use crate::harness;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    prelude::*, px, App, ImageCacheError, ImageFormat, ImageSource, RenderImage, TestAppContext,
    VisualTestContext, Window,
};
use harness::{events, open_host};
use herogpui_components::Avatar;
use herogpui_core::Size;

#[gpui::test]
fn fallback_lines_keep_the_size_specific_leading(cx: &mut TestAppContext) {
    // `.avatar--sm .avatar__fallback` is `text-xs` (16px lines),
    // `.avatar__fallback` `text-sm` (20px) and `--lg` `text-base` (24px).
    for (size, height) in [(Size::Sm, 32.), (Size::Md, 40.), (Size::Lg, 48.)] {
        for inherited in [None, Some(48.)] {
            let cx = open_host(cx, move || {
                gpui::div()
                    .when_some(inherited, |root, leading| root.line_height(px(leading)))
                    .child(
                        Avatar::new("fallback-leading").size(size).fallback(
                            gpui::div()
                                .debug_selector(|| "fallback-lines".to_owned())
                                .flex()
                                .flex_col()
                                .flex_shrink_0()
                                .child("A")
                                .child("B"),
                        ),
                    )
                    .into_any_element()
            });
            let bounds = cx
                .debug_bounds("fallback-lines")
                .expect("fallback must render");
            assert_eq!(
                bounds.size.height,
                px(height),
                "{size:?}, parent={inherited:?}"
            );
        }
    }
}

/// A valid 1×1 RGBA PNG; gpui decodes it through its own image pipeline.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0xcf, 0x50,
    0x0f, 0x00, 0x03, 0x86, 0x01, 0x80, 0x5a, 0x34, 0x7d, 0x6b, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// A second valid 1×1 PNG with a distinct image-content identity.
const TINY_PNG_B: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

/// The custom-fallback probe. Its flag is set exactly on the frames where
/// the fallback paints, so a fresh flag plus one forced frame is a precise
/// "is the fallback in the tree right now" check.
fn probe(painted: Rc<Cell<bool>>) -> gpui::AnyElement {
    gpui::canvas(move |_, _, _| painted.set(true), |_, _, _, _| {})
        .size(px(20.))
        .into_any_element()
}

/// Whether the fallback paints on a freshly forced frame.
fn paints_fallback(cx: &mut VisualTestContext, flag: &Rc<Cell<bool>>) -> bool {
    flag.set(false);
    cx.update(|window, _| window.refresh());
    flag.get()
}

/// Forces enough frames for a load transition and its spawned callbacks to
/// settle before the next assertion.
fn settle(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

#[gpui::test]
fn pending_load_shows_the_fallback_until_success_replaces_it(cx: &mut TestAppContext) {
    let stage = Rc::new(Cell::new(0u8));
    let source = gated_source(stage.clone());
    let fallback_painted = Rc::new(Cell::new(false));
    let flag = fallback_painted.clone();
    let cx = open_host(cx, move || {
        Avatar::new("pending")
            .name("JD")
            .src(source.clone())
            .fallback(probe(flag.clone()))
            .into_any_element()
    });

    assert!(
        paints_fallback(cx, &fallback_painted),
        "the fallback must render while the image is pending"
    );

    stage.set(2);
    settle(cx);
    assert!(
        !paints_fallback(cx, &fallback_painted),
        "the loaded image must replace the fallback"
    );
}

#[gpui::test]
fn a_failed_load_shows_the_fallback_and_fires_on_error_once(cx: &mut TestAppContext) {
    let seen = events();
    let stage = Rc::new(Cell::new(0u8));
    let source = gated_source(stage.clone());
    let fallback_painted = Rc::new(Cell::new(false));
    let flag = fallback_painted.clone();
    let frame_seen = seen.clone();
    let cx = open_host(cx, move || {
        let reported = frame_seen.clone();
        Avatar::new("failure")
            .name("JD")
            .src(source.clone())
            .on_error(move |_, _| reported.borrow_mut().push("error".into()))
            .fallback(probe(flag.clone()))
            .into_any_element()
    });

    settle(cx);
    assert!(seen.borrow().is_empty(), "a pending load reports nothing");
    assert!(
        paints_fallback(cx, &fallback_painted),
        "the fallback renders while the image is pending"
    );

    stage.set(1);
    settle(cx);
    assert!(
        paints_fallback(cx, &fallback_painted),
        "the fallback must render after the error"
    );
    assert_eq!(seen.borrow().as_slice(), ["error"]);

    settle(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        ["error"],
        "on_error must fire exactly once"
    );
}

/// Radix Avatar 1.2.6: a complete image with `naturalWidth == 0` is `error`
/// (1.1.11 left it `loading`), so a decoded zero-size image keeps the
/// fallback and reports `on_error` once, never `on_load`.
#[gpui::test]
fn zero_sized_image_fails_and_shows_the_fallback(cx: &mut TestAppContext) {
    let seen = events();
    let empty = Arc::new(RenderImage::new(Vec::new()));
    let fallback_painted = Rc::new(Cell::new(false));
    let flag = fallback_painted.clone();
    let frame_seen = seen.clone();
    let cx = open_host(cx, move || {
        let errors = frame_seen.clone();
        let loads = frame_seen.clone();
        Avatar::new("zero-size")
            .name("JD")
            .src(ImageSource::Render(empty.clone()))
            .on_error(move |_, _| errors.borrow_mut().push("error".into()))
            .on_load(move |_, _| loads.borrow_mut().push("load".into()))
            .fallback(probe(flag.clone()))
            .into_any_element()
    });

    settle(cx);
    assert!(
        paints_fallback(cx, &fallback_painted),
        "a zero-size image must leave the fallback in place"
    );
    assert_eq!(seen.borrow().as_slice(), ["error"]);

    settle(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        ["error"],
        "on_error must fire exactly once and on_load never"
    );
}

#[gpui::test]
fn on_load_fires_once_when_the_image_arrives(cx: &mut TestAppContext) {
    let seen = events();
    let stage = Rc::new(Cell::new(0u8));
    let source = gated_source(stage.clone());
    let fallback_painted = Rc::new(Cell::new(false));
    let flag = fallback_painted.clone();
    let frame_seen = seen.clone();
    let cx = open_host(cx, move || {
        let reported = frame_seen.clone();
        Avatar::new("success")
            .name("JD")
            .src(source.clone())
            .on_load(move |_, _| reported.borrow_mut().push("load".into()))
            .fallback(probe(flag.clone()))
            .into_any_element()
    });

    settle(cx);
    assert!(seen.borrow().is_empty(), "a pending load reports nothing");

    stage.set(2);
    settle(cx);
    assert!(
        !paints_fallback(cx, &fallback_painted),
        "the loaded image must replace the fallback"
    );
    assert_eq!(seen.borrow().as_slice(), ["load"]);

    settle(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        ["load"],
        "on_load must fire exactly once"
    );
}

#[gpui::test]
fn a_ready_image_renders_without_reporting_a_failure(cx: &mut TestAppContext) {
    let seen = events();
    let image = Arc::new(gpui::Image::from_bytes(ImageFormat::Png, TINY_PNG.to_vec()));
    let fallback_painted = Rc::new(Cell::new(false));
    let flag = fallback_painted.clone();
    let frame_seen = seen.clone();
    let cx = open_host(cx, move || {
        let errors = frame_seen.clone();
        let loads = frame_seen.clone();
        Avatar::new("ready")
            .name("JD")
            .src(ImageSource::Image(image.clone()))
            .on_error(move |_, _| errors.borrow_mut().push("error".into()))
            .on_load(move |_, _| loads.borrow_mut().push("load".into()))
            .fallback(probe(flag.clone()))
            .into_any_element()
    });

    settle(cx);
    assert!(
        !paints_fallback(cx, &fallback_painted),
        "an image that is already available never shows the fallback"
    );
    assert_eq!(
        seen.borrow().as_slice(),
        ["load"],
        "the ready image reports success, never error"
    );
}

#[gpui::test]
fn delay_ms_counts_from_mount_even_while_the_load_is_pending(cx: &mut TestAppContext) {
    let seen = events();
    // The load never resolves: the old error-anchored delay would never
    // show the fallback at all.
    let source = gated_source(Rc::new(Cell::new(0u8)));
    let fallback_painted = Rc::new(Cell::new(false));
    let flag = fallback_painted.clone();
    let frame_seen = seen.clone();
    let cx = open_host(cx, move || {
        let reported = frame_seen.clone();
        Avatar::new("delay")
            .name("NA")
            .src(source.clone())
            .delay_ms(600)
            .on_error(move |_, _| reported.borrow_mut().push("error".into()))
            .fallback(probe(flag.clone()))
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        !paints_fallback(cx, &fallback_painted),
        "the delay window from mount must hold the fallback back"
    );

    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    assert!(
        paints_fallback(cx, &fallback_painted),
        "the fallback must appear when the mount delay elapses, with no error"
    );
    assert!(seen.borrow().is_empty());
}

#[gpui::test]
fn an_error_inside_the_delay_window_waits_for_the_mount_window(cx: &mut TestAppContext) {
    let seen = events();
    let stage = Rc::new(Cell::new(0u8));
    let source = gated_source(stage.clone());
    let fallback_painted = Rc::new(Cell::new(false));
    let flag = fallback_painted.clone();
    let frame_seen = seen.clone();
    let cx = open_host(cx, move || {
        let reported = frame_seen.clone();
        Avatar::new("delay")
            .name("NA")
            .src(source.clone())
            .delay_ms(600)
            .on_error(move |_, _| reported.borrow_mut().push("error".into()))
            .fallback(probe(flag.clone()))
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    cx.run_until_parked();

    // The load fails 300ms into the 600ms window from mount.
    stage.set(1);
    cx.executor().advance_clock(Duration::from_millis(300));
    settle(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        ["error"],
        "the failure is reported when it happens"
    );
    assert!(
        !paints_fallback(cx, &fallback_painted),
        "the failure must not shortcut or restart the mount delay window"
    );

    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    assert!(
        paints_fallback(cx, &fallback_painted),
        "the fallback shows when the 600ms window from mount ends"
    );
    assert_eq!(seen.borrow().as_slice(), ["error"]);
}

/// Two avatars pointing at one source are two component instances: Radix
/// tracks pending/loaded/errored per instance, so each sibling fires its own
/// `on_error`/`on_load` once and runs its own `delay_ms` window. The
/// constructor id is what separates them; identical ids would share one
/// lifecycle slot.
#[gpui::test]
fn same_source_siblings_each_fire_on_error_once(cx: &mut TestAppContext) {
    let seen_a = events();
    let seen_b = events();
    let stage = Rc::new(Cell::new(0u8));
    let source = gated_source(stage.clone());
    let reported_a = seen_a.clone();
    let reported_b = seen_b.clone();
    let cx = open_host(cx, move || {
        let report_a = reported_a.clone();
        let report_b = reported_b.clone();
        gpui::div()
            .flex()
            .gap(px(4.))
            .child(
                Avatar::new("sibling-a")
                    .name("JD")
                    .src(source.clone())
                    .on_error(move |_, _| report_a.borrow_mut().push("error".into())),
            )
            .child(
                Avatar::new("sibling-b")
                    .name("JD")
                    .src(source.clone())
                    .on_error(move |_, _| report_b.borrow_mut().push("error".into())),
            )
            .into_any_element()
    });

    stage.set(1);
    settle(cx);
    settle(cx);
    assert_eq!(
        seen_a.borrow().as_slice(),
        ["error"],
        "the first sibling's on_error must fire exactly once"
    );
    assert_eq!(
        seen_b.borrow().as_slice(),
        ["error"],
        "the second sibling's on_error must fire exactly once"
    );
}

#[gpui::test]
fn same_source_siblings_each_fire_on_load_once(cx: &mut TestAppContext) {
    let seen_a = events();
    let seen_b = events();
    let stage = Rc::new(Cell::new(0u8));
    let source = gated_source(stage.clone());
    let reported_a = seen_a.clone();
    let reported_b = seen_b.clone();
    let cx = open_host(cx, move || {
        let report_a = reported_a.clone();
        let report_b = reported_b.clone();
        gpui::div()
            .flex()
            .gap(px(4.))
            .child(
                Avatar::new("sibling-a")
                    .name("JD")
                    .src(source.clone())
                    .on_load(move |_, _| report_a.borrow_mut().push("load".into())),
            )
            .child(
                Avatar::new("sibling-b")
                    .name("JD")
                    .src(source.clone())
                    .on_load(move |_, _| report_b.borrow_mut().push("load".into())),
            )
            .into_any_element()
    });

    stage.set(2);
    settle(cx);
    settle(cx);
    assert_eq!(
        seen_a.borrow().as_slice(),
        ["load"],
        "the first sibling's on_load must fire exactly once"
    );
    assert_eq!(
        seen_b.borrow().as_slice(),
        ["load"],
        "the second sibling's on_load must fire exactly once"
    );
}

#[gpui::test]
fn same_source_siblings_run_independent_delay_windows(cx: &mut TestAppContext) {
    // The load never resolves, so only the delay windows move the fallbacks.
    let source = gated_source(Rc::new(Cell::new(0u8)));
    let painted_a = Rc::new(Cell::new(false));
    let painted_b = Rc::new(Cell::new(false));
    let flag_a = painted_a.clone();
    let flag_b = painted_b.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .gap(px(4.))
            .child(
                Avatar::new("short-window")
                    .name("NA")
                    .src(source.clone())
                    .delay_ms(100)
                    .fallback(probe(flag_a.clone())),
            )
            .child(
                Avatar::new("long-window")
                    .name("NA")
                    .src(source.clone())
                    .delay_ms(600)
                    .fallback(probe(flag_b.clone())),
            )
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        !paints_fallback(cx, &painted_a) && !paints_fallback(cx, &painted_b),
        "both windows hold their fallbacks back from mount"
    );

    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    assert!(
        paints_fallback(cx, &painted_a),
        "the 100ms sibling's fallback appears at its own window"
    );
    assert!(
        !paints_fallback(cx, &painted_b),
        "the 600ms sibling must not inherit the 100ms sibling's window"
    );

    cx.executor().advance_clock(Duration::from_millis(400));
    cx.run_until_parked();
    assert!(
        paints_fallback(cx, &painted_b),
        "the 600ms sibling's fallback appears at its own window"
    );
}

/// Without `custom_source_key`, a custom loader carries no value identity, so
/// the closure's allocation is not the source: two distinct loaders on one
/// instance — alternated per frame here — share the instance's single
/// lifecycle slot and the error fires exactly once. A key derived from the
/// loader's pointer would read every frame as a source change and fire (or
/// ABA-reuse stale latches) per allocation.
#[gpui::test]
fn custom_loader_identity_is_the_instance_not_the_closure_allocation(cx: &mut TestAppContext) {
    let seen = events();
    let stage = Rc::new(Cell::new(0u8));
    let loader_a = gated_source(stage.clone());
    let loader_b = gated_source(stage.clone());
    let flip = Rc::new(Cell::new(0u64));
    let frame_seen = seen.clone();
    let cx = open_host(cx, move || {
        let reported = frame_seen.clone();
        let n = flip.get();
        flip.set(n + 1);
        let source = if n.is_multiple_of(2) {
            loader_a.clone()
        } else {
            loader_b.clone()
        };
        Avatar::new("alternate-loaders")
            .name("JD")
            .src(source)
            .on_error(move |_, _| reported.borrow_mut().push("error".into()))
            .into_any_element()
    });

    stage.set(1);
    settle(cx);
    settle(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        ["error"],
        "alternating custom loader allocations must not restart the lifecycle: \
         on_error fires exactly once for the instance"
    );
}

/// The idiomatic inline `src(ImageSource::Custom(move |..| ..))` rebuilds the
/// loader closure every frame — a fresh `Arc` allocation per frame. That must
/// not restart the mount-time delay window: it is armed once, from mount.
#[gpui::test]
fn an_inline_custom_loader_keeps_its_delay_window_across_rebuilds(cx: &mut TestAppContext) {
    let seen = events();
    // The load never resolves, so only the delay window moves the fallback.
    let stage = Rc::new(Cell::new(0u8));
    let fallback_painted = Rc::new(Cell::new(false));
    let flag = fallback_painted.clone();
    let frame_seen = seen.clone();
    let stage_for_view = stage.clone();
    let cx = open_host(cx, move || {
        let reported = frame_seen.clone();
        let flag = flag.clone();
        let stage = stage_for_view.clone();
        Avatar::new("inline-loader")
            .name("NA")
            .src(gated_source(stage))
            .delay_ms(600)
            .on_error(move |_, _| reported.borrow_mut().push("error".into()))
            .fallback(probe(flag))
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        !paints_fallback(cx, &fallback_painted),
        "the delay window from mount must hold the fallback back"
    );

    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    assert!(
        paints_fallback(cx, &fallback_painted),
        "the delay window must not be restarted by the per-frame loader rebuilds"
    );
    assert!(seen.borrow().is_empty());

    stage.set(1);
    settle(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        ["error"],
        "a fresh no-key inline loader must keep the instance's error latch"
    );
    settle(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        ["error"],
        "rebuilding the no-key inline loader must not refire on_error"
    );
}

#[gpui::test]
fn changing_custom_loader_from_ok_to_ok_reports_the_distinct_output(cx: &mut TestAppContext) {
    let seen = events();
    let outputs = Rc::new(RefCell::new(Vec::<(char, usize)>::new()));
    let source_a = labeled_source('A', TINY_PNG, outputs.clone());
    let source_b = labeled_source('B', TINY_PNG_B, outputs.clone());
    let use_b = Rc::new(Cell::new(false));
    let use_b_for_view = use_b.clone();
    let seen_for_view = seen.clone();
    let fallback_painted = Rc::new(Cell::new(false));
    let flag = fallback_painted.clone();
    let cx = open_host(cx, move || {
        let use_b = use_b_for_view.get();
        let source = if use_b {
            source_b.clone()
        } else {
            source_a.clone()
        };
        let label = if use_b { 'B' } else { 'A' };
        let reported = seen_for_view.clone();
        Avatar::new("custom-ok-ok")
            .name("JD")
            .src(source)
            .custom_source_key(format!("source-{label}"))
            .on_load(move |_, _| reported.borrow_mut().push(format!("load:{label}")))
            .fallback(probe(flag.clone()))
            .into_any_element()
    });

    settle(cx);
    assert_eq!(seen.borrow().as_slice(), ["load:A"]);
    assert!(!paints_fallback(cx, &fallback_painted));
    let first_output = outputs
        .borrow()
        .iter()
        .rev()
        .find(|(label, _)| *label == 'A')
        .map(|(_, id)| *id)
        .expect("loader A must provide a rendered image");

    use_b.set(true);
    settle(cx);
    assert_eq!(seen.borrow().as_slice(), ["load:A", "load:B"]);
    assert!(!paints_fallback(cx, &fallback_painted));
    let second_output = outputs
        .borrow()
        .iter()
        .rev()
        .find(|(label, _)| *label == 'B')
        .map(|(_, id)| *id)
        .expect("loader B must provide a rendered image");
    assert_ne!(
        first_output, second_output,
        "A and B must be distinct image outputs"
    );
}

#[gpui::test]
fn changing_custom_loader_ok_to_error_to_ok_does_not_keep_stale_latches(cx: &mut TestAppContext) {
    let seen = events();
    let stage = Rc::new(Cell::new(0u8));
    let source_a = staged_source(2, TINY_PNG);
    let source_b = staged_source(1, TINY_PNG_B);
    let source_c = staged_source(2, TINY_PNG);
    let source_for_view = Rc::new([source_a, source_b, source_c]);
    let stage_for_view = stage.clone();
    let seen_for_view = seen.clone();
    let fallback_painted = Rc::new(Cell::new(false));
    let flag = fallback_painted.clone();
    let cx = open_host(cx, move || {
        let index = stage_for_view.get();
        let source = source_for_view[index as usize].clone();
        let label = ['A', 'B', 'C'][index as usize];
        let reported_load = seen_for_view.clone();
        let reported_error = seen_for_view.clone();
        Avatar::new("custom-ok-error-ok")
            .name("JD")
            .src(source)
            .custom_source_key(format!("source-{label}"))
            .on_load(move |_, _| reported_load.borrow_mut().push(format!("load:{label}")))
            .on_error(move |_, _| reported_error.borrow_mut().push(format!("error:{label}")))
            .fallback(probe(flag.clone()))
            .into_any_element()
    });

    settle(cx);
    assert_eq!(seen.borrow().as_slice(), ["load:A"]);

    stage.set(1);
    settle(cx);
    assert_eq!(seen.borrow().as_slice(), ["load:A", "error:B"]);
    assert!(paints_fallback(cx, &fallback_painted));

    stage.set(2);
    settle(cx);
    assert!(!paints_fallback(cx, &fallback_painted));
    assert_eq!(
        seen.borrow().as_slice(),
        ["load:A", "error:B", "load:C"],
        "a new custom source must not inherit either prior outcome latch"
    );
}

/// A source that completes while mounted reports even if the avatar switches
/// sources before the reporting task drains: `Avatar.Image.onLoad` fires on
/// the first observed success, and tiny sources decode synchronously, so the
/// observation belongs to the mounted source. (The stale path — a pending
/// source unmounted before its completion arrives — stays suppressed by the
/// generation guard, pinned deterministically in `avatar::outcome_guard`.)
#[gpui::test]
fn immediately_completing_loader_reports_while_mounted(cx: &mut TestAppContext) {
    let seen = events();
    let use_b = Rc::new(Cell::new(false));
    let source_a = staged_source(2, TINY_PNG);
    let source_b = staged_source(1, TINY_PNG_B);
    let use_b_for_view = use_b.clone();
    let seen_for_view = seen.clone();
    let cx = open_host(cx, move || {
        let use_b = use_b_for_view.get();
        let source = if use_b {
            source_b.clone()
        } else {
            source_a.clone()
        };
        let label = if use_b { 'B' } else { 'A' };
        let reported_load = seen_for_view.clone();
        let reported_error = seen_for_view.clone();
        Avatar::new("custom-stale-callback")
            .name("JD")
            .src(source)
            .custom_source_key(format!("source-{label}"))
            .on_load(move |_, _| reported_load.borrow_mut().push(format!("load:{label}")))
            .on_error(move |_, _| reported_error.borrow_mut().push(format!("error:{label}")))
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    use_b.set(true);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().as_slice(),
        ["load:A", "error:B"],
        "A completed while mounted so its load reports; B then fails on its own turn"
    );
}

/// A different source starts its own lifecycle on the same instance: the
/// load/error latches reset, so the new source's outcome is reported too,
/// and returning to the first source reports it again from scratch.
#[gpui::test]
fn changing_the_source_resets_the_lifecycle(cx: &mut TestAppContext) {
    let seen = events();
    let stage = Rc::new(Cell::new(0u8));
    let embedded = Rc::new(Cell::new(false));
    let frame_seen = seen.clone();
    let stage_in = stage.clone();
    let embedded_in = embedded.clone();
    let cx = open_host(cx, move || {
        let errors = frame_seen.clone();
        let loads = frame_seen.clone();
        let custom = gated_source(stage_in.clone());
        // An unregistered non-URI path classifies as an embedded resource and
        // fails deterministically on this platform.
        let source: ImageSource = if embedded_in.get() {
            "images/avatar-broken.png".into()
        } else {
            custom
        };
        Avatar::new("swap")
            .name("JD")
            .src(source)
            .on_error(move |_, _| errors.borrow_mut().push("error".into()))
            .on_load(move |_, _| loads.borrow_mut().push("load".into()))
            .into_any_element()
    });

    stage.set(1);
    settle(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        ["error"],
        "the custom source fails once"
    );

    embedded.set(true);
    settle(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        ["error", "error"],
        "the embedded source starts its own lifecycle: its failure is reported too"
    );

    embedded.set(false);
    stage.set(2);
    settle(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        ["error", "error", "load"],
        "returning to the custom source resets the latches again and reports its load"
    );
}

/// Removing an image resets its per-image latches. Re-adding the same source
/// is a new `Avatar.Image` lifecycle, so its callback must be delivered again.
#[gpui::test]
fn removing_and_readding_the_same_source_restarts_the_image_lifecycle(cx: &mut TestAppContext) {
    let seen = events();
    let source = gated_source(Rc::new(Cell::new(1u8)));
    let present = Rc::new(Cell::new(true));
    let source_for_view = source;
    let present_for_view = present.clone();
    let frame_seen = seen.clone();
    let cx = open_host(cx, move || {
        let source = source_for_view.clone();
        let present = present_for_view.clone();
        let reported = frame_seen.clone();
        let avatar = Avatar::new("remove-readd")
            .name("JD")
            .on_error(move |_, _| reported.borrow_mut().push("error".into()));
        if present.get() {
            avatar.src(source).into_any_element()
        } else {
            avatar.into_any_element()
        }
    });

    settle(cx);
    assert_eq!(seen.borrow().as_slice(), ["error"]);

    present.set(false);
    settle(cx);
    present.set(true);
    settle(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        ["error", "error"],
        "re-adding the same source must fire its new image lifecycle callback"
    );
}

/// `delayMs` belongs to the mounted fallback, not to an individual image
/// source. Swapping the image before the window ends must not restart it.
#[gpui::test]
fn changing_the_source_does_not_restart_the_fallback_delay(cx: &mut TestAppContext) {
    let first = gated_source(Rc::new(Cell::new(0u8)));
    let second: ImageSource = "images/avatar-second.png".into();
    let switched = Rc::new(Cell::new(false));
    let painted = Rc::new(Cell::new(false));
    let switched_for_view = switched.clone();
    let painted_for_view = painted.clone();
    let cx = open_host(cx, move || {
        let source = if switched_for_view.get() {
            second.clone()
        } else {
            first.clone()
        };
        Avatar::new("delay-source-swap")
            .name("NA")
            .src(source)
            .delay_ms(600)
            .fallback(probe(painted_for_view.clone()))
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(!paints_fallback(cx, &painted));

    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    switched.set(true);
    settle(cx);
    assert!(
        !paints_fallback(cx, &painted),
        "the fallback remains hidden before the original 600ms deadline"
    );

    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(
        paints_fallback(cx, &painted),
        "a source swap must not restart the mounted fallback delay"
    );
}

#[gpui::test]
fn custom_fallback_children_show_when_there_is_no_source(cx: &mut TestAppContext) {
    let fallback_painted = Rc::new(Cell::new(false));
    let flag = fallback_painted.clone();
    let cx = open_host(cx, move || {
        Avatar::new("sourceless")
            .fallback(probe(flag.clone()))
            .into_any_element()
    });

    assert!(
        paints_fallback(cx, &fallback_painted),
        "a sourceless avatar always renders its fallback children"
    );
}

/// The pinned `.avatar` sizes: 32, 40 and 48px edge lengths. `debug_bounds`
/// is safe here: the wrappers stay in the tree for the whole test.
#[gpui::test]
fn sizes_paint_the_pinned_edge_lengths(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(4.))
            .child(
                gpui::div()
                    .flex()
                    .debug_selector(|| "wrap-sm".to_owned())
                    .child(Avatar::new("sm").name("JD").size(Size::Sm)),
            )
            .child(
                gpui::div()
                    .flex()
                    .debug_selector(|| "wrap-md".to_owned())
                    .child(Avatar::new("md").name("JD")),
            )
            .child(
                gpui::div()
                    .flex()
                    .debug_selector(|| "wrap-lg".to_owned())
                    .child(Avatar::new("lg").name("JD").size(Size::Lg)),
            )
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    for (name, edge) in [("wrap-sm", 32.), ("wrap-md", 40.), ("wrap-lg", 48.)] {
        let bounds = cx
            .debug_bounds(name)
            .unwrap_or_else(|| panic!("{name} must paint"));
        assert_eq!(bounds.size.width, px(edge), "{name} width");
        assert_eq!(bounds.size.height, px(edge), "{name} height");
    }
}

/// The port renders `name` as the uppercase first characters of its first
/// two words, and `?` when there is nothing to take a character from.
#[test]
fn initials_cover_the_documented_edge_cases() {
    assert_eq!(Avatar::initials(""), "?");
    assert_eq!(Avatar::initials("   "), "?");
    assert_eq!(Avatar::initials("jane"), "J");
    assert_eq!(Avatar::initials("jane doe"), "JD");
    assert_eq!(Avatar::initials("jane   doe"), "JD");
    assert_eq!(Avatar::initials("  jane doe  "), "JD");
    assert_eq!(Avatar::initials("jane doe smith"), "JD");
    assert_eq!(Avatar::initials("ärnulf öberg"), "ÄÖ");
}

/// The exact loader shape `ImageSource::Custom` declares in gpui 0.2.2.
type AvatarLoader =
    Arc<dyn Fn(&mut Window, &mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>>>;

/// A custom loader with a stage the test flips between frames:
/// 0 = pending, 1 = error, 2 = success. Every call builds a fresh
/// `Arc`-allocated closure; without `custom_source_key`, rebuilding one must
/// not change the avatar's behavior.
// gpui's `ImageSource::Custom` requires the loader in an `Arc`, but the
// headless harness is single-threaded and stages it with an `Rc`.
#[expect(clippy::arc_with_non_send_sync)]
fn gated_source(stage: Rc<Cell<u8>>) -> ImageSource {
    let image = Arc::new(gpui::Image::from_bytes(ImageFormat::Png, TINY_PNG.to_vec()));
    let loader: AvatarLoader =
        Arc::new(move |window: &mut Window, cx: &mut App| match stage.get() {
            0 => None,
            1 => Some(Err(ImageCacheError::Io(Arc::new(std::io::Error::other(
                "broken avatar source",
            ))))),
            _ => image.clone().use_render_image(window, cx).map(Ok),
        });
    ImageSource::Custom(loader)
}

#[expect(clippy::arc_with_non_send_sync)]
fn labeled_source(
    label: char,
    bytes: &'static [u8],
    outputs: Rc<RefCell<Vec<(char, usize)>>>,
) -> ImageSource {
    let image = Arc::new(gpui::Image::from_bytes(ImageFormat::Png, bytes.to_vec()));
    let loader: AvatarLoader = Arc::new(move |window: &mut Window, cx: &mut App| {
        let result = image.clone().use_render_image(window, cx).map(Ok);
        if let Some(Ok(rendered)) = &result {
            outputs.borrow_mut().push((label, rendered.id.0));
        }
        result
    });
    ImageSource::Custom(loader)
}

fn staged_source(outcome: u8, bytes: &'static [u8]) -> ImageSource {
    let image = Arc::new(gpui::Image::from_bytes(ImageFormat::Png, bytes.to_vec()));
    let loader: AvatarLoader = Arc::new(move |window: &mut Window, cx: &mut App| {
        if outcome == 1 {
            Some(Err(ImageCacheError::Io(Arc::new(std::io::Error::other(
                "broken avatar source",
            )))))
        } else {
            image.clone().use_render_image(window, cx).map(Ok)
        }
    });
    ImageSource::Custom(loader)
}
