//! HeroUI v3.2.5 toast stack: `isExpanded`, hover/focus expand, timer pause,
//! hidden overflow, `exitDuration`, and the Alt+T hotkey.

use crate::harness;

use std::time::Duration;

use gpui::{point, prelude::*, px, Modifiers, TestAppContext};
use harness::{open_host, painted, press, still};
use herogpui_components::{
    dismiss_toast, toast_store, Toast, ToastHotkey, ToastViewport, DEFAULT_TOAST_EXIT_DURATION,
};
use herogpui_theme::{set_reduce_motion, ActiveTheme, ThemeProvider};

fn flush_frame(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn expanded(cx: &mut gpui::VisualTestContext) -> bool {
    cx.update(|_window, cx| toast_store(cx).read(cx).is_expanded())
}

fn interaction_paused(cx: &mut gpui::VisualTestContext) -> bool {
    cx.update(|_window, cx| toast_store(cx).read(cx).is_interaction_paused())
}

#[gpui::test]
fn single_toast_never_expands_even_when_forced(cx: &mut TestAppContext) {
    still();
    cx.update(|cx| {
        Toast::new("Only").timeout(Duration::ZERO).push(None, cx);
    });
    let cx = open_host(cx, || {
        ToastViewport::new().is_expanded(true).into_any_element()
    });
    assert!(
        !expanded(cx),
        "a single toast must stay collapsed, matching pinned activeToastCount > 1"
    );
}

#[gpui::test]
fn is_expanded_opens_the_stack_without_pausing_timers(cx: &mut TestAppContext) {
    still();
    cx.update(|cx| {
        Toast::new("Older")
            .timeout(Duration::from_millis(300))
            .push(None, cx);
        Toast::new("Newer")
            .timeout(Duration::from_millis(300))
            .push(None, cx);
    });
    let cx = open_host(cx, || {
        ToastViewport::new().is_expanded(true).into_any_element()
    });
    assert!(
        expanded(cx),
        "isExpanded must open a two-toast stack without hovering"
    );
    assert!(
        !interaction_paused(cx),
        "forced expansion must not pause timers; pinned HeroUI documents that"
    );

    cx.executor().advance_clock(Duration::from_millis(500));
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "isExpanded must leave the dismissal clocks running"
        );
    });
}

#[gpui::test]
fn expanded_stack_uses_measured_absolute_card_offsets(cx: &mut TestAppContext) {
    still();
    let (older, newer) = cx.update(|cx| {
        let older = Toast::new("An older notification")
            .description("A description that gives this card measurable height.")
            .timeout(Duration::ZERO)
            .push(None, cx);
        let newer = Toast::new("Newest notification")
            .timeout(Duration::ZERO)
            .push(None, cx);
        (older, newer)
    });
    let cx = open_host(cx, || {
        ToastViewport::new().is_expanded(true).into_any_element()
    });
    // The first frame records natural card heights; the next frame consumes
    // them to position the absolute stack.
    flush_frame(cx);
    flush_frame(cx);
    let older_key: &'static str = Box::leak(format!("toast-slot-{older}").into_boxed_str());
    let newer_key: &'static str = Box::leak(format!("toast-slot-{newer}").into_boxed_str());
    let older_bounds = cx
        .debug_bounds(older_key)
        .expect("the older toast has an absolute stack slot");
    let newer_bounds = cx
        .debug_bounds(newer_key)
        .expect("the newer toast has an absolute stack slot");
    assert!(
        older_bounds.origin.y < newer_bounds.origin.y,
        "the older toast must be offset above the frontmost card"
    );
    assert_eq!(
        older_bounds.bottom() + px(12.),
        newer_bounds.origin.y,
        "expanded cards must use measured height plus the configured gap"
    );
}

#[gpui::test]
fn hover_expands_the_stack_and_pauses_timers(cx: &mut TestAppContext) {
    still();
    cx.update(|cx| {
        Toast::new("Older")
            .timeout(Duration::from_millis(300))
            .push(None, cx);
        Toast::new("Newer")
            .timeout(Duration::from_millis(300))
            .push(None, cx);
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());
    assert!(!expanded(cx), "the stack starts collapsed");

    // Bottom-centre front card: x 730..1190, y 1020..1064.
    cx.simulate_mouse_move(point(px(960.), px(1042.)), None, Modifiers::none());
    flush_frame(cx);
    assert!(expanded(cx), "hovering the region must expand the stack");
    assert!(
        interaction_paused(cx),
        "hover must suspend timers the way pinned interaction pause does"
    );

    cx.executor().advance_clock(Duration::from_millis(500));
    cx.update(|_window, cx| {
        assert_eq!(
            toast_store(cx).read(cx).toasts().len(),
            2,
            "a hovered stack must not dismiss while the pointer is inside"
        );
    });
}

/// v3's `.toast-region--bottom` is `left-1/2 -translate-x-1/2` around a
/// region only as wide as the toasts: hover must count over the centred
/// 460px column (x 730..1190 in this 1920px window), not the whole window.
#[gpui::test]
fn hover_counts_only_over_the_centred_stack(cx: &mut TestAppContext) {
    still();
    cx.update(|cx| {
        Toast::new("Older").timeout(Duration::ZERO).push(None, cx);
        Toast::new("Newer").timeout(Duration::ZERO).push(None, cx);
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());
    assert!(!expanded(cx), "the stack starts collapsed");

    // Inside the region's vertical band but beside the toast column: the
    // full-width region used to expand on this pointer position.
    cx.simulate_mouse_move(point(px(1900.), px(1040.)), None, Modifiers::none());
    flush_frame(cx);
    assert!(
        !expanded(cx),
        "hover beside the toast column must not expand the stack"
    );
    assert!(
        !interaction_paused(cx),
        "a pointer outside the column must not pause the timers"
    );

    // Moving over the front card still expands the stack.
    cx.simulate_mouse_move(point(px(960.), px(1042.)), None, Modifiers::none());
    flush_frame(cx);
    assert!(expanded(cx), "hovering the card must expand the stack");
}

/// Narrowing the hover region to the toast column must not move the cards:
/// the painted card quads keep the centred 460px x-range.
#[gpui::test]
fn centred_region_keeps_the_cards_centred(cx: &mut TestAppContext) {
    still();
    cx.update(|cx| {
        Toast::new("Older").timeout(Duration::ZERO).push(None, cx);
        Toast::new("Newer").timeout(Duration::ZERO).push(None, cx);
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());
    flush_frame(cx);

    let scene = painted(cx);
    let surface = cx.update(|_, cx| cx.colors().surface.background);
    let mut card_lefts: Vec<f32> = scene
        .filled(surface)
        .into_iter()
        .map(|quad| scene.bounds(quad))
        .filter(|bounds| (f32::from(bounds.size.width) - 460.).abs() < 1.)
        .map(|bounds| f32::from(bounds.origin.x))
        .collect();
    card_lefts.sort_by(|a, b| a.total_cmp(b));
    card_lefts.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    assert!(
        !card_lefts.is_empty(),
        "the toast cards must paint their surface fill"
    );
    assert!(
        card_lefts.iter().all(|x| (*x - 730.).abs() < 1.),
        "every card must keep the centred 730px left edge: {card_lefts:?}"
    );
}

#[gpui::test]
fn overflow_stays_queued_and_is_marked_hidden(cx: &mut TestAppContext) {
    still();
    let (hidden, front) = cx.update(|cx| {
        let hidden = Toast::new("Hidden").timeout(Duration::ZERO).push(None, cx);
        let front = Toast::new("Front").timeout(Duration::ZERO).push(None, cx);
        (hidden, front)
    });
    let cx = open_host(cx, || {
        ToastViewport::new()
            .max_visible_toasts(1)
            .into_any_element()
    });
    cx.update(|_window, cx| {
        let store = toast_store(cx).read(cx);
        assert_eq!(store.toasts().len(), 2, "overflow stays in the queue");
        assert_eq!(store.visible_toasts(1)[0].id, front);
        assert_eq!(
            store
                .hidden_toasts(1)
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>(),
            [hidden],
            "the older toast is data-hidden, not dropped"
        );
        assert!(!store.is_expanded());
    });
}

#[gpui::test]
fn dismissed_toast_stays_mounted_for_exit_duration(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ThemeProvider::init(cx);
        set_reduce_motion(false, cx);
        let id = Toast::new("Leaving").timeout(Duration::ZERO).push(None, cx);
        dismiss_toast(id, cx);
        let store = toast_store(cx).read(cx);
        assert!(
            store.toasts().is_empty(),
            "close removes the toast from the live queue immediately"
        );
        assert_eq!(
            store.exiting().len(),
            1,
            "the card stays mounted for exitDuration"
        );
        assert_eq!(store.exiting()[0].id, id);
        assert_eq!(DEFAULT_TOAST_EXIT_DURATION, Duration::from_millis(300));
    });
    cx.executor().advance_clock(Duration::from_millis(299));
    cx.update(|cx| {
        assert_eq!(
            toast_store(cx).read(cx).exiting().len(),
            1,
            "the exiting card must still be mounted just before exitDuration"
        );
    });
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.update(|cx| {
        assert!(
            toast_store(cx).read(cx).exiting().is_empty(),
            "the exiting card must drop once exitDuration elapses"
        );
    });
}

#[gpui::test]
fn reduced_motion_skips_the_exit_hold(cx: &mut TestAppContext) {
    cx.update(|cx| {
        ThemeProvider::init(cx);
        set_reduce_motion(true, cx);
        let id = Toast::new("Gone").timeout(Duration::ZERO).push(None, cx);
        dismiss_toast(id, cx);
        let store = toast_store(cx).read(cx);
        assert!(store.toasts().is_empty());
        assert!(
            store.exiting().is_empty(),
            "reduced motion must drop the card immediately"
        );
    });
}

#[gpui::test]
fn alt_t_focuses_the_region_and_expands(cx: &mut TestAppContext) {
    still();
    cx.update(|cx| {
        Toast::new("Older").timeout(Duration::ZERO).push(None, cx);
        Toast::new("Newer").timeout(Duration::ZERO).push(None, cx);
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());
    assert!(!expanded(cx));
    press(cx, "alt-t");
    flush_frame(cx);
    assert!(
        expanded(cx),
        "the default Alt+T hotkey must focus the region and expand the stack"
    );
}

#[gpui::test]
fn disabled_hotkey_does_not_expand(cx: &mut TestAppContext) {
    still();
    cx.update(|cx| {
        Toast::new("Older").timeout(Duration::ZERO).push(None, cx);
        Toast::new("Newer").timeout(Duration::ZERO).push(None, cx);
    });
    let cx = open_host(cx, || {
        ToastViewport::new()
            .hotkey(ToastHotkey::disabled())
            .into_any_element()
    });
    press(cx, "alt-t");
    flush_frame(cx);
    assert!(
        !expanded(cx),
        "hotkey={{[]}} must leave the stack collapsed"
    );
}
