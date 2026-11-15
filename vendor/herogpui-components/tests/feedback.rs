//! Behaviour tests for the feedback family: Toast, Alert, Avatar, Badge,
//! Meter, ProgressBar, ProgressCircle, Spinner, Skeleton.
//!
//! The store-level toast queue (cap, action, pause/resume on the bare store,
//! placement) is driven by `virtual_and_feedback.rs`; the value-label render
//! closures (percentage/valueText at in-range values) by `value_props.rs`.
//! What neither touches is the *viewport*: a placement is a layout choice
//! until one asks which coordinate the close button answers at, `pauseAll`
//! must stop a clock whose card is on screen, and the frontmost-toast slot is
//! the one place the queue's eviction order is observable.
//!
//! The value tests compare formatted strings (`"{percentage:.0}|{text}"`),
//! never floats — `clippy::float_cmp` is denied — and every percentage/text
//! expectation is grounded in React Aria's `useProgressBar`, which v3's prop
//! tables point at ("Inherits from React Aria ProgressBar"):
//!
//! ```js
//! value = clamp(value, minValue, maxValue);
//! if (!isIndeterminate && !valueLabel) {
//!   let valueToFormat = formatOptions.style === 'percent' ? percentage : value;
//!   valueLabel = formatter.format(valueToFormat);
//! }
//! ```
//!
//! Two consequences are tests here: an out-of-range value clamps (the
//! percentage and a percent-style label both land on 0/100), and an
//! indeterminate progress never generates a value label at all. Both rules
//! are where the progress defects live — an indeterminate `ProgressCircle`
//! invents a 25% read-out, and a custom-style label is formatted from the
//! *raw* value instead of the clamped one.
//!
//! Geometry is derived from the components' own constants on a 1920x1080 test
//! window (the test display's bounds). A toast card is `w(460) px(16) py(12)`
//! with one 20px title line, so it is 44px tall; its close button is the
//! 20px `size-5` square at the pinned `-end-1 -top-1` offset. Centred
//! placements span x 730..1190; `*End` hugs the right inset (x 1444..1904)
//! and `*Start` the left (x 16..476); `Top` parks the card at y 16..60 and
//! `Bottom` at y 1020..1064 — so the close centre is (1184, 1026) centred,
//! (1898, 1026) / (470, 1026) at the sides, and (1898, 22) at top-right. A
//! closable Alert is `w_full px-4 py-3` with a 14px close glyph, centre
//! (1897, 19) — the coordinate `buttons.rs` already drives for the closable
//! case, which is why `alert.rs` must only ever render one closable Alert per
//! window: its close id is the hardcoded `"alert-close"`.

use crate::harness;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use gpui::{
    canvas, point, prelude::*, px, AbsoluteLength, AnyElement, Bounds, Modifiers, Pixels,
    TestAppContext, VisualTestContext,
};
use harness::{click, events, open_host, press, Events};
use herogpui_components::{
    clear_toasts, dismiss_toast, pause_toasts, toast_store, Alert, Avatar, AvatarVariant, Badge,
    BadgeAnchor, BadgeLabel, BadgePlacement, BadgeVariant, Color, Meter, NumberFormat, ProgressBar,
    ProgressCircle, Size, Skeleton, Spinner, SpinnerSize, Toast, ToastData, ToastPlacement,
    ToastViewport,
};
use herogpui_theme::SkeletonAnimation;

/// Pins the toast card layout by enabling reduced motion **before** the first
/// frame: a toast wraps itself in `entering_zoom`, whose animation runs on
/// the executor clock this suite does not advance, so without this the card would
/// sit at its t=0 pose for the whole test. Same rule the overlay suites
/// learned; the harness applies the request before opening the host window.
fn still() {
    harness::still();
}

/// Pushes the pending frame through. Events are dispatched against the last
/// rendered frame, so anything that changes state — a scroll, a press, a
/// dismissed or pushed toast — needs a redraw before the next event, or the
/// next event hits the stale frame.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// Parks the pointer off the stack. Hover now pauses toast clocks, so a test
/// that expects a timeout after a click has to leave the region first.
fn leave_stack(cx: &mut VisualTestContext) {
    cx.simulate_mouse_move(point(px(-100.), px(-100.)), None, Modifiers::none());
    flush_frame(cx);
}

/// The snapshot the last render's closure left behind.
fn last_string(seen: &Events) -> String {
    seen.borrow()
        .last()
        .expect("the closure must have run at least once")
        .clone()
}

// ---------------------------------------------------------------------------
// Toast: the viewport's half of the queue
// ---------------------------------------------------------------------------

/// Centred placements were already shown to move the card vertically; the
/// start/end axis is the untested half. `ToastViewport` decides where the
/// card sits, so it decides which window coordinate the 20px close button
/// answers at: at `BottomEnd` the button lives at the bottom-right corner of
/// the window, at `BottomStart` at the bottom-left, and at `TopEnd` at the
/// top-right. After each move the *old* coordinate must miss — a placement
/// that does not move the card would dismiss at every position.
#[gpui::test]
fn toast_start_end_placements_move_the_close_target(cx: &mut TestAppContext) {
    still();
    cx.update(|cx| {
        Toast::new("A").timeout(Duration::ZERO).push(None, cx);
    });
    let placement = Rc::new(RefCell::new(ToastPlacement::BottomEnd));
    let holder = placement.clone();
    let cx = open_host(cx, move || {
        ToastViewport::new()
            .placement(*holder.borrow())
            .into_any_element()
    });

    // Card geometry at BottomEnd: the region is `bottom(16) right(16)`, so
    // the 460px card spans x 1920-16-460 = 1444..1904 and y 1020..1064; the
    // close button (20px, absolute right(-4) top(-4)) spans x 1888..1908,
    // y 1016..1036 — centre (1898, 1026).
    click(cx, 1898., 1026.);
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "at BottomEnd the close button must hug the bottom-right corner"
        );
    });

    // BottomStart: the region flips to `left(16)`; the card spans x 16..476
    // and the close button x 460..480, same y. The old bottom-right point is
    // now empty background, so the same click must leave the toast alone.
    *placement.borrow_mut() = ToastPlacement::BottomStart;
    cx.update(|_window, cx| {
        Toast::new("B").timeout(Duration::ZERO).push(None, cx);
    });
    flush_frame(cx);
    click(cx, 1898., 1026.);
    cx.update(|_window, cx| {
        assert_eq!(
            toast_store(cx).read(cx).toasts().len(),
            1,
            "moving to BottomStart must take the card off the bottom-right \
             point: the old coordinate now hits nothing"
        );
    });
    click(cx, 470., 1026.);
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "at BottomStart the close button must hug the bottom-left corner"
        );
    });

    // TopEnd: `top(16) right(16)` puts the card at y 16..60 and the close
    // button at y 12..32 — centre (1898, 22). The bottom-left point must now
    // miss.
    *placement.borrow_mut() = ToastPlacement::TopEnd;
    cx.update(|_window, cx| {
        Toast::new("C").timeout(Duration::ZERO).push(None, cx);
    });
    flush_frame(cx);
    click(cx, 470., 1026.);
    cx.update(|_window, cx| {
        assert_eq!(
            toast_store(cx).read(cx).toasts().len(),
            1,
            "at TopEnd the card must have left the bottom-left corner"
        );
    });
    click(cx, 1898., 22.);
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "at TopEnd the close button must hug the top-right corner"
        );
    });
}

/// A caller-owned indicator render factory must survive queue storage and be
/// invoked by the mounted card without changing the shared indicator geometry.
#[gpui::test]
fn toast_custom_indicator_content_renders_from_the_queue(cx: &mut TestAppContext) {
    still();
    let id = cx.update(|cx| {
        Toast::new("Custom indicator")
            .timeout(Duration::ZERO)
            .indicator_content(|_| {
                gpui::svg()
                    .size(px(12.))
                    .path(herogpui_components::icons::INFO_CIRCLE)
            })
            .push(None, cx)
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());
    cx.update(|_window, cx| {
        let toast = toast_store(cx)
            .read(cx)
            .toasts()
            .iter()
            .find(|toast| toast.id == id)
            .expect("the custom toast remains queued");
        assert!(toast.indicator_content.is_some());
    });
}

/// With an action present the card's children are [title, action, close], and
/// the close affordance must report the *dismissal*, never the action: a
/// click on the 20px close square takes the toast down without the action
/// handler running — two handlers, only one of them tripped.
/// (The action half — Enter on the action reports and dismisses — is driven
/// in `virtual_and_feedback.rs`.)
#[gpui::test]
fn toast_close_click_dismisses_without_running_the_action(cx: &mut TestAppContext) {
    still();
    let actions = events();
    let rec = actions.clone();
    cx.update(move |cx| {
        Toast::new("Has an action")
            .timeout(Duration::ZERO)
            .action("Undo", move |_| rec.borrow_mut().push("undo".into()))
            .push(None, cx)
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());

    // The card is bottom-centre: 460px wide across x 730..1190. The 32px Sm
    // action button drives the content height, so the card is 12+32+12 = 56px
    // tall on y 1008..1064; the close button is the 20px square at absolute
    // top(-4), spanning y 1004..1024 — centre (1184, 1014). The action button sits between the title column
    // and the close, so this click cannot reach it.
    click(cx, 1184., 1014.);
    assert_eq!(
        actions.borrow().as_slice(),
        [] as [&str; 0],
        "clicking the close must dismiss without running the action handler"
    );
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "the close click must dismiss the toast"
        );
    });
}

/// v3's `Toast.CloseButton` "accepts all CloseButton props", and `CloseButton`
/// in this port is a focusable tab stop. The toast's close affordance is a
/// hand-rolled `div().id(..)` with an `on_click`, so it must explicitly attach
/// its keyed `track_focus` handle: gpui builds the tab order from those
/// handles. The regression below proves that the close follows the action
/// button and Enter dismisses the toast instead of running the action.
#[gpui::test]
fn toast_close_button_is_keyboard_reachable(cx: &mut TestAppContext) {
    still();
    let actions = events();
    let rec = actions.clone();
    cx.update(move |cx| {
        Toast::new("Has an action")
            .timeout(Duration::ZERO)
            .action("Undo", move |_| rec.borrow_mut().push("undo".into()))
            .push(None, cx)
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());

    // The action button is the only keyboard stop in the card, so two Tabs
    // must land on the close button — the stop that follows the action.
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        actions.borrow().as_slice(),
        [] as [&str; 0],
        "Enter on the close button must dismiss without running the action"
    );
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "Enter on the close button must dismiss the toast"
        );
    });
}

/// `pauseAll` is a store flag, but its observable contract is a rendered
/// card: a paused queue must keep a toast whose timeout has long passed *on
/// screen*, and the on-screen card must still answer its close button — pausing
/// the clock is not pausing the toast. After `resumeAll`, the same viewport's
/// clock moves again and a fresh timed toast goes.
#[gpui::test]
fn toast_pause_keeps_the_rendered_card_alive_and_dismissable(cx: &mut TestAppContext) {
    still();
    // Pause before pushing: the toast's timer ticks every 100ms and reads the
    // flag on each tick, so a tick that started before the pause would have
    // run. Push first, then open the viewport — the store is app-global.
    cx.update(|cx| {
        pause_toasts(true, cx);
        Toast::new("Paused")
            .timeout(Duration::from_millis(300))
            .push(None, cx)
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());

    // 1500ms is five times the 300ms timeout: every tick has seen `paused`.
    cx.executor().advance_clock(Duration::from_millis(1500));
    cx.update(|_window, cx| {
        assert_eq!(
            toast_store(cx).read(cx).toasts().len(),
            1,
            "a paused queue must not dismiss a toast whose timeout has passed"
        );
    });

    // The card is still drawn at the bottom-centre slot, so its close button
    // still answers at (1184, 1026) — hand dismissal is not governed by the
    // clock, and `pauseAll` must not have frozen it.
    click(cx, 1184., 1026.);
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "a rendered paused toast must still be dismissable by hand"
        );
    });
    leave_stack(cx);

    // Resume: a fresh 300ms toast dies on the resumed clock. Five 100ms ticks
    // is far past its timeout. Leave the stack first: hover now pauses
    // timers, and the click above left the pointer on the card.
    cx.update(|_window, cx| {
        pause_toasts(false, cx);
        Toast::new("Post-resume")
            .timeout(Duration::from_millis(300))
            .push(None, cx);
    });
    flush_frame(cx);
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "after resume the same viewport must let the clock dismiss again"
        );
    });
}

/// Pinned v3's visibility limit never evicts queue entries. With a one-card
/// viewport, pushing A, B, C, D retains the newest-first queue [D, C, B, A]
/// while only D is drawn. Each dismissal then reveals the next-newest at the
/// same frontmost slot until even the initially hidden A becomes visible.
#[gpui::test]
fn toast_viewport_reveals_the_newest_next_as_toasts_leave(cx: &mut TestAppContext) {
    still();
    let (a, b, c, d) = cx.update(|cx| {
        (
            Toast::new("T1").timeout(Duration::ZERO).push(None, cx),
            Toast::new("T2").timeout(Duration::ZERO).push(None, cx),
            Toast::new("T3").timeout(Duration::ZERO).push(None, cx),
            Toast::new("T4").timeout(Duration::ZERO).push(None, cx),
        )
    });
    let cx = open_host(cx, || {
        ToastViewport::new()
            .max_visible_toasts(1)
            .scale_factor(0.0)
            .into_any_element()
    });

    // React Stately unshifts every toast and `maxVisibleToasts` affects only
    // rendering, so all four entries remain in newest-first order.
    cx.update(|_window, cx| {
        let ids: Vec<u64> = toast_store(cx)
            .read(cx)
            .toasts()
            .iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(
            ids,
            [d, c, b, a],
            "the fourth push must retain every toast in newest-first order"
        );
        assert!(a != b && a != c && a != d, "the ids must be distinct");
    });

    // The one-card viewport draws only D. Its close button answers at the
    // bottom-centre point (1184, 1026), and each dismissal slides the next
    // queued toast into that exact slot.
    flush_frame(cx);
    click(cx, 1184., 1026.);
    cx.update(|_window, cx| {
        let ids: Vec<u64> = toast_store(cx)
            .read(cx)
            .toasts()
            .iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(
            ids,
            [c, b, a],
            "the first click must dismiss the newest drawn (D)"
        );
    });
    flush_frame(cx);
    click(cx, 1184., 1026.);
    cx.update(|_window, cx| {
        let ids: Vec<u64> = toast_store(cx)
            .read(cx)
            .toasts()
            .iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(
            ids,
            [b, a],
            "the second click must dismiss the next-newest (C)"
        );
    });
    flush_frame(cx);
    click(cx, 1184., 1026.);
    cx.update(|_window, cx| {
        assert_eq!(
            toast_store(cx).read(cx).toasts()[0].id,
            a,
            "the third click must reveal the initially hidden oldest toast"
        );
    });
    flush_frame(cx);
    click(cx, 1184., 1026.);
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "the fourth click must dismiss the final queued toast"
        );
    });
}

/// HeroUI's `maxVisibleToasts` is visual only. Its queue gives React Aria every
/// toast and marks overflow hidden, so covered entries still spend their clock.
#[gpui::test]
fn toast_overflow_auto_dismisses_while_visually_hidden(cx: &mut TestAppContext) {
    still();
    let (hidden, front) = cx.update(|cx| {
        let hidden = Toast::new("Hidden")
            .timeout(Duration::from_millis(300))
            .push(None, cx);
        let front = Toast::new("Front").timeout(Duration::ZERO).push(None, cx);
        (hidden, front)
    });
    let cx = open_host(cx, || {
        ToastViewport::new()
            .max_visible_toasts(1)
            .scale_factor(0.0)
            .into_any_element()
    });

    cx.executor().advance_clock(Duration::from_millis(1500));
    cx.update(|_window, cx| {
        let store = toast_store(cx).read(cx);
        let ids = store
            .toasts()
            .iter()
            .map(|toast| toast.id)
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [front],
            "maxVisibleToasts must not pause a visually hidden toast's clock"
        );
        assert_eq!(store.visible_toasts(1)[0].id, front);
        assert!(store.toasts().iter().all(|toast| toast.id != hidden));
    });
}

/// v3 documents `onClose` as "Callback when toast is closed" — once, however
/// it closes. A persistent toast (timeout 0) hand-dismissed through its
/// rendered close button must report exactly one close, and a timed toast
/// dismissed by its own clock must report exactly one too, and neither may
/// fire the other's handler.
#[gpui::test]
fn toast_on_close_fires_once_for_each_dismissal_path(cx: &mut TestAppContext) {
    still();
    let hand_dismissed = Rc::new(RefCell::new(0usize));
    let timed_out = Rc::new(RefCell::new(0usize));
    let hand = hand_dismissed.clone();
    let timed = timed_out.clone();
    // Push the timed toast first and the persistent one second so the
    // persistent card is the newest: on a two-card stack the newest sits at
    // the bottom slot, whose close button the click below targets.
    cx.update(move |cx| {
        Toast::new("Times out")
            .timeout(Duration::from_millis(300))
            .on_close(move |_| *timed.borrow_mut() += 1)
            .push(None, cx);
        Toast::new("Stays")
            .timeout(Duration::ZERO)
            .on_close(move |_| *hand.borrow_mut() += 1)
            .push(None, cx);
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());

    // Hand dismissal of the persistent toast: `dismiss_toast` runs the
    // toast's `onClose` exactly once, synchronously with the click. The timed
    // sibling's clock has not moved, so its handler must not have run.
    click(cx, 1184., 1026.);
    assert_eq!(
        *hand_dismissed.borrow(),
        1,
        "hand dismissal must report the close exactly once"
    );
    assert_eq!(
        *timed_out.borrow(),
        0,
        "the sibling's onClose must not fire"
    );
    leave_stack(cx);

    // The timed toast's own clock dismisses it: the tick loop fires onClose
    // once when the toast leaves. 500ms is past the 300ms timeout. Leave
    // the stack first so the click's leftover hover does not pause it.
    cx.executor().advance_clock(Duration::from_millis(500));
    assert_eq!(
        *timed_out.borrow(),
        1,
        "a timed-out toast must report its close exactly once"
    );
    assert_eq!(
        *hand_dismissed.borrow(),
        1,
        "the timer must not re-report the hand-dismissed toast"
    );
}

/// A timed toast carries two `onClose` owners: `dismiss_toast` runs the
/// handler when the card's button is pressed, and the toast's timer loop runs
/// the *same* handler when it wakes to find the toast already gone. A toast
/// that the user closes by hand therefore reports `onClose` twice — v3's
/// documented contract is "Callback when toast is closed", once.
#[gpui::test]
fn toast_on_close_fires_once_when_a_timed_toast_is_dismissed_by_hand(cx: &mut TestAppContext) {
    still();
    let closed = Rc::new(RefCell::new(0usize));
    let count = closed.clone();
    cx.update(move |cx| {
        Toast::new("Timed")
            .timeout(Duration::from_millis(300))
            .on_close(move |_| *count.borrow_mut() += 1)
            .push(None, cx)
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());

    // The close click dismisses the toast and runs onClose once, synchronously.
    click(cx, 1184., 1026.);
    cx.update(|_window, cx| {
        assert!(
            toast_store(cx).read(cx).toasts().is_empty(),
            "the click must dismiss the toast"
        );
    });
    assert_eq!(
        *closed.borrow(),
        1,
        "the hand dismissal itself must report exactly one close"
    );

    // The timer loop wakes on its next 100ms tick, sees the toast is gone and
    // calls onClose again before exiting.
    cx.executor().advance_clock(Duration::from_millis(500));
    assert_eq!(
        *closed.borrow(),
        1,
        "the dormant timer must not report a second close"
    );
}

/// Pinned React Stately's `ToastQueue.clear()` drops the queue without calling
/// each toast's `onClose`. A timed Rust toast still has a sleeping task, so the
/// clear path must retire that task's close claim as well as removing the row;
/// otherwise its next 100ms tick reports a close that upstream never emits.
#[gpui::test]
fn toast_clear_does_not_report_timed_toasts_as_individually_closed(cx: &mut TestAppContext) {
    still();
    let closed = Rc::new(RefCell::new(0usize));
    let count = closed.clone();
    cx.update(move |cx| {
        Toast::new("Cleared")
            .timeout(Duration::from_millis(300))
            .on_close(move |_| *count.borrow_mut() += 1)
            .push(None, cx);
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());

    cx.update(|_window, cx| clear_toasts(cx));
    cx.update(|_window, cx| {
        assert!(toast_store(cx).read(cx).toasts().is_empty());
    });
    cx.executor().advance_clock(Duration::from_millis(500));
    assert_eq!(
        *closed.borrow(),
        0,
        "clearing the queue must not later invoke an individual onClose"
    );
}

/// A custom queue may reinsert an explicit key. Clearing the old toast retires
/// only that lifecycle; the replacement must still report its own dismissal.
#[gpui::test]
fn toast_reused_id_gets_a_fresh_close_lifecycle(cx: &mut TestAppContext) {
    let id = cx.update(|cx| Toast::new("Old").timeout(Duration::ZERO).push(None, cx));
    cx.update(clear_toasts);
    let closed = std::sync::Arc::new(AtomicUsize::new(0));
    let count = closed.clone();
    cx.update(move |cx| {
        toast_store(cx).update(cx, |store, cx| {
            store.insert(ToastData {
                id,
                color: Color::Accent,
                title: "Replacement".into(),
                description: None,
                closable: true,
                indicator: None,
                indicator_content: None,
                indicator_set: false,
                is_loading: false,
                action: None,
                on_close: Some(std::sync::Arc::new(move |_| {
                    count.fetch_add(1, Ordering::Relaxed);
                })),
                close_hover_bg: None,
                padding: None,
                radius: None,
            });
            cx.notify();
        });
    });

    cx.update(|cx| dismiss_toast(id, cx));
    assert_eq!(
        closed.load(Ordering::Relaxed),
        1,
        "reusing a cleared id must not suppress the replacement's onClose"
    );
}

/// The public queue method has the same callback contract as the convenience
/// free function: one close, one callback, and no later timer duplicate.
#[gpui::test]
fn toast_store_close_reports_on_close_once(cx: &mut TestAppContext) {
    let closed = Rc::new(RefCell::new(0usize));
    let count = closed.clone();
    let id = cx.update(move |cx| {
        Toast::new("Queue close")
            .timeout(Duration::from_millis(300))
            .on_close(move |_| *count.borrow_mut() += 1)
            .push(None, cx)
    });

    cx.update(|cx| {
        let store = toast_store(cx);
        herogpui_components::ToastStore::close(&store, id, cx);
    });
    assert_eq!(*closed.borrow(), 1);
    cx.executor().advance_clock(Duration::from_millis(500));
    assert_eq!(
        *closed.borrow(),
        1,
        "the retired timer must not duplicate ToastStore::close"
    );
}

/// A queue-supplied key must reserve its numeric slot. Otherwise the next
/// Toast::push would reuse that key and one close would remove both entries.
#[gpui::test]
fn toast_store_add_keeps_generated_keys_unique(cx: &mut TestAppContext) {
    let explicit = cx.update(|cx| {
        let store = toast_store(cx);
        herogpui_components::ToastStore::add(
            &store,
            ToastData {
                id: 7,
                color: Color::Default,
                title: "Explicit".into(),
                description: None,
                closable: true,
                indicator: None,
                indicator_content: None,
                indicator_set: false,
                is_loading: false,
                action: None,
                on_close: None,
                close_hover_bg: None,
                padding: None,
                radius: None,
            },
            cx,
        )
    });
    let generated = cx.update(|cx| {
        Toast::new("Generated")
            .timeout(Duration::ZERO)
            .push(None, cx)
    });

    assert_eq!((explicit, generated), (7, 8));
}

/// Pinned v3 makes every non-frontmost toast pointer-inert and hides its close
/// button from interaction. A stacked viewport must therefore expose exactly
/// the newest toast's close action to Tab, regardless of paint order.
#[gpui::test]
fn toast_stack_keyboard_reaches_only_the_frontmost_close(cx: &mut TestAppContext) {
    still();
    let (older, front) = cx.update(|cx| {
        let older = Toast::new("Older").timeout(Duration::ZERO).push(None, cx);
        let front = Toast::new("Front").timeout(Duration::ZERO).push(None, cx);
        (older, front)
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());

    press(cx, "tab");
    press(cx, "enter");
    cx.update(|_window, cx| {
        let ids = toast_store(cx)
            .read(cx)
            .toasts()
            .iter()
            .map(|toast| toast.id)
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [older],
            "the first reachable close action must belong to the frontmost toast"
        );
        assert!(ids.iter().all(|id| *id != front));
    });
}

/// The pinned `toast(message)` helper stores `variant: "default"` unless the
/// caller selects a semantic variant or uses a variant convenience method.
#[gpui::test]
fn toast_new_uses_the_default_variant(cx: &mut TestAppContext) {
    let id = cx.update(|cx| Toast::new("Plain").timeout(Duration::ZERO).push(None, cx));
    cx.update(|cx| {
        let store = toast_store(cx);
        let toast = store
            .read(cx)
            .toasts()
            .iter()
            .find(|toast| toast.id == id)
            .expect("new toast is queued");
        assert_eq!(
            toast.color,
            Color::Default,
            "Toast::new must match the pinned helper's default variant"
        );
    });
}

// ---------------------------------------------------------------------------
// Alert
// ---------------------------------------------------------------------------

/// Every documented `status` renders without panicking — one host, five
/// alerts. Alert has no built-in close affordance (v3 removed `isClosable`),
/// and none of these composes one, so nothing inside them is interactive and
/// they can all share a window.
#[gpui::test]
fn alert_every_status_renders_without_panicking(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(Alert::new("Default status"))
            .child(Alert::new("Accent status").status(Color::Accent))
            .child(Alert::new("Success status").status(Color::Success))
            .child(Alert::new("Warning status").status(Color::Warning))
            .child(Alert::new("Danger status").status(Color::Danger))
            .into_any_element()
    });
    // open_host rendered the page; a status that panicked drawing itself would
    // have died there. The alerts are informational and take no state, so a
    // press anywhere on the body must not record anything — there is no
    // handler at all to reach.
    click(cx, 30., 25.);
    click(cx, 1900., 75.);
    click(cx, 100., 125.);
}

/// A CloseButton composed into an Alert answers at (1892, 24) — `buttons.rs`
/// drives that. The no-affordance half is the missing claim: with no
/// `isClosable` prop Alert provides no built-in close behavior, so the old
/// built-in glyph coordinate must be dead air. A probe element below the
/// alert proves the probe machinery works, so "nothing recorded" means the
/// alert really consumed the press.
#[gpui::test]
fn alert_without_close_has_nothing_to_press_at_the_close_spot(cx: &mut TestAppContext) {
    let recorded = events();
    let probe = recorded.clone();
    let cx = open_host(cx, move || {
        let probe = probe.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(Alert::new("Unsaved changes").description("No close here"))
            .child(
                gpui::div()
                    .id("fb-alert-probe")
                    .w(px(40.))
                    .h(px(20.))
                    .cursor_pointer()
                    .on_click(move |_, _, _| probe.borrow_mut().push("probe".into())),
            )
            .into_any_element()
    });

    // The alert is `w_full px-4 py-3` with both a title and a description:
    // the content column is two 20px lines plus the 2px gap (42px), so the
    // alert spans y 0..66. A close glyph would sit at x 1890..1904 centre
    // (1897, 19).
    click(cx, 1897., 19.);
    assert!(
        recorded.borrow().is_empty(),
        "a non-closable alert must have nothing at the close position"
    );

    // The probe sits 10px below the alert, y 76..96, centre (20, 86).
    click(cx, 20., 86.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["probe"],
        "the probe below must confirm the click machinery works"
    );
}

// ---------------------------------------------------------------------------
// Meter / ProgressBar / ProgressCircle: clamping and the value label
// ---------------------------------------------------------------------------

/// React Aria clamps `value` into `[minValue, maxValue]` before computing the
/// percentage, so a value past `max` reports 100% and one below `min` reports
/// 0% — never an overflowing fraction. The percentage and the percent-style
/// valueText must move together through the `value_content` closure.
#[gpui::test]
fn progress_bar_clamps_value_outside_the_range(cx: &mut TestAppContext) {
    let seen = events();
    let record = seen.clone();
    let value = Rc::new(RefCell::new(150.0f32));
    let for_view = value.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        let now = *for_view.borrow();
        ProgressBar::new("feedback-progress-clamp")
            .value(now)
            .show_value_label(true)
            .value_content(move |percentage, text, _| {
                record.borrow_mut().push(format!("{percentage:.0}|{text}"));
                gpui::div().child(text.to_owned()).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        last_string(&seen),
        "100|100%",
        "a value of 150 against max 100 must clamp to 100%, not overflow"
    );
    *value.borrow_mut() = -20.0;
    flush_frame(cx);
    assert_eq!(
        last_string(&seen),
        "0|0%",
        "a value below min must clamp to 0%"
    );
    *value.borrow_mut() = 0.0;
    flush_frame(cx);
    assert_eq!(last_string(&seen), "0|0%", "the floor of the range is 0%");
    *value.borrow_mut() = 100.0;
    flush_frame(cx);
    assert_eq!(
        last_string(&seen),
        "100|100%",
        "the ceiling of the range is 100%"
    );
}

/// `formatOptions` selects how the generated label is written — v3's Custom
/// Value Scale example drives `{style: "currency", currency: "USD"}` on a
/// 0..1000 scale and reads "$750.00" back. The non-percent styles format the
/// *value* (not the fraction), so the text moves with the value while the
/// percentage tracks the fraction.
#[gpui::test]
fn progress_bar_format_options_change_the_value_text(cx: &mut TestAppContext) {
    let seen = events();
    let record = seen.clone();
    let value = Rc::new(RefCell::new(750.0f32));
    let for_view = value.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        let now = *for_view.borrow();
        ProgressBar::new("feedback-progress-format")
            .value(now)
            .max_value(1000.0)
            .show_value_label(true)
            .format_options(NumberFormat::currency("USD"))
            .value_content(move |percentage, text, _| {
                record.borrow_mut().push(format!("{percentage:.0}|{text}"));
                gpui::div().child(text.to_owned()).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        last_string(&seen),
        "75|$750.00",
        "750 of 1000 must report 75% and the currency-formatted value"
    );
    *value.borrow_mut() = 500.0;
    flush_frame(cx);
    assert_eq!(
        last_string(&seen),
        "50|$500.00",
        "the formatted text must follow the value"
    );
}

/// The Meter forwards its own range and format to the bar that draws it; a
/// nonzero `minValue` puts the fraction arithmetic under test — 750 across
/// 500..1000 is a *half*, not three quarters. Both range edges report the
/// clamped 0%/100% with the value-formatted text.
#[gpui::test]
fn meter_custom_range_formats_and_clamps(cx: &mut TestAppContext) {
    let seen = events();
    let record = seen.clone();
    let value = Rc::new(RefCell::new(750.0f32));
    let for_view = value.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        let now = *for_view.borrow();
        Meter::new("feedback-meter-format", now)
            .min_value(500.0)
            .max_value(1000.0)
            .show_value(true)
            .format_options(NumberFormat::currency("USD"))
            .value_content(move |percentage, text| {
                record.borrow_mut().push(format!("{percentage:.0}|{text}"));
                gpui::div().child(text.to_owned()).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        last_string(&seen),
        "50|$750.00",
        "750 across 500..1000 spans half the range, not three quarters"
    );
    *value.borrow_mut() = 500.0;
    flush_frame(cx);
    assert_eq!(
        last_string(&seen),
        "0|$500.00",
        "the custom minimum must be the 0% edge"
    );
    *value.borrow_mut() = 1000.0;
    flush_frame(cx);
    assert_eq!(
        last_string(&seen),
        "100|$1,000.00",
        "the custom maximum must be the 100% edge, grouped and formatted"
    );
}

/// The ring clamps the same way the bar does: the percentage is normalised
/// into 0..100 before it is handed to `value_content`, so no overflow ever
/// reaches the closure.
#[gpui::test]
fn progress_circle_clamps_value_outside_the_range(cx: &mut TestAppContext) {
    let seen = events();
    let record = seen.clone();
    let value = Rc::new(RefCell::new(150.0f32));
    let for_view = value.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        let now = *for_view.borrow();
        ProgressCircle::new()
            .value(now)
            .show_value_label(true)
            .value_content(move |percentage, text, _| {
                record.borrow_mut().push(format!("{percentage:.0}|{text}"));
                gpui::div().child(text.to_owned()).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        last_string(&seen),
        "100|100%",
        "a ring value of 150 must clamp to 100%"
    );
    *value.borrow_mut() = -5.0;
    flush_frame(cx);
    assert_eq!(
        last_string(&seen),
        "0|0%",
        "a ring value below 0 must clamp"
    );
}

/// `formatOptions` reaches the circle as well: the text follows the value in
/// the requested style while the percentage keeps tracking the fraction.
#[gpui::test]
fn progress_circle_format_options_change_the_value_text(cx: &mut TestAppContext) {
    let seen = events();
    let record = seen.clone();
    let value = Rc::new(RefCell::new(60.0f32));
    let for_view = value.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        let now = *for_view.borrow();
        ProgressCircle::new()
            .value(now)
            .show_value_label(true)
            .format_options(NumberFormat::currency("USD"))
            .value_content(move |percentage, text, _| {
                record.borrow_mut().push(format!("{percentage:.0}|{text}"));
                gpui::div().child(text.to_owned()).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        last_string(&seen),
        "60|$60.00",
        "60 of 100 must report 60% and the currency text"
    );
    *value.borrow_mut() = 30.0;
    flush_frame(cx);
    assert_eq!(
        last_string(&seen),
        "30|$30.00",
        "the circle's text must follow its value too"
    );
}

/// An indeterminate progress cannot state a percentage, so the bar hands the
/// closure an *empty* value text — React Aria generates no value label at all
/// while `isIndeterminate` is set. The closure still runs (a "Loading…"
/// renderer draws its own text), proving the empty string is the contract,
/// not a missing call. A nonzero stored value keeps the percentage assertion
/// from passing vacuously.
#[gpui::test]
fn progress_bar_indeterminate_reports_no_value_text(cx: &mut TestAppContext) {
    let seen = events();
    let record = seen.clone();
    open_host(cx, move || {
        let record = record.clone();
        ProgressBar::new("feedback-progress-indeterminate")
            .value(73.0)
            .value_label("must stay hidden")
            .is_indeterminate(true)
            .show_value_label(true)
            .value_content(move |percentage, text, is_indeterminate| {
                record.borrow_mut().push(format!("{percentage:.0}|{text}"));
                assert!(is_indeterminate);
                gpui::div().child(text.to_owned()).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        last_string(&seen),
        "0|",
        "an indeterminate bar must report no percentage text, only the empty label"
    );
}

/// React Aria clamps `value` *before* formatting, so a custom-style label on
/// an out-of-range value shows the clamped amount: 1500 against max 1000
/// reads "$1,000.00", and 400 against min 500 reads "$500.00". The fill,
/// percentage and label must all use that same clamped value.
#[gpui::test]
fn meter_value_text_uses_the_clamped_value_for_custom_formats(cx: &mut TestAppContext) {
    let seen = events();
    let record = seen.clone();
    let value = Rc::new(RefCell::new(1500.0f32));
    let for_view = value.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        let now = *for_view.borrow();
        Meter::new("feedback-meter-clamp", now)
            .min_value(500.0)
            .max_value(1000.0)
            .show_value(true)
            .format_options(NumberFormat::currency("USD"))
            .value_content(move |percentage, text| {
                record.borrow_mut().push(format!("{percentage:.0}|{text}"));
                gpui::div().child(text.to_owned()).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        last_string(&seen),
        "100|$1,000.00",
        "an over-max value must clamp both the percentage and the formatted label"
    );
    *value.borrow_mut() = 400.0;
    flush_frame(cx);
    assert_eq!(
        last_string(&seen),
        "0|$500.00",
        "a below-min value must clamp both the percentage and the formatted label"
    );
}

/// The circle follows React Aria's indeterminate value contract too: its
/// quarter arc is geometry rather than a reported percentage or value label.
#[gpui::test]
fn progress_circle_indeterminate_reports_no_value_text(cx: &mut TestAppContext) {
    let seen = events();
    let record = seen.clone();
    open_host(cx, move || {
        let record = record.clone();
        ProgressCircle::new()
            .is_indeterminate(true)
            .show_value_label(true)
            .value_content(move |percentage, text, is_indeterminate| {
                record.borrow_mut().push(format!("{percentage:.0}|{text}"));
                assert!(is_indeterminate);
                gpui::div().child(text.to_owned()).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        last_string(&seen),
        "0|",
        "an indeterminate ring must report no percentage text, only the empty label"
    );
}

/// Drives frames until every finite animation has had time to finish —
/// 60 × 16ms covers the 300ms fill with margin — and returns the live
/// animation count at the end. Each in-flight `with_animation` registers
/// one next-frame callback per painted frame, so the steady-state count is
/// the number of animations still running (a repeating spin never ends).
/// Like `checkbox_motion`'s drain, no `refresh` is forced: the animation
/// callbacks' own notifies drive the redraws.
fn pending_animation_floor(cx: &mut VisualTestContext) -> usize {
    let mut pending = 0;
    for _ in 0..60 {
        pending = cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.executor().advance_clock(Duration::from_millis(16));
    }
    pending
}

/// Two id-less rings must keep separate fill tweens. The old constant
/// fallback key made every anonymous ring share one keyed slot -- one
/// `Rc<Cell>` fraction -- so the two targets retargeted the slot every frame,
/// the fill animation restarted forever, and (with an indeterminate sibling
/// settling the shared cell at 0.25) determinate rings painted the quarter
/// arc. `simulate_next_frame` counts live animation callbacks: with separate
/// slots both fills settle on mount and nothing is left pending.
#[gpui::test]
fn idless_progress_circles_keep_separate_fill_tweens(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .gap(px(8.))
            .children([
                ProgressCircle::new().value(60.).into_any_element(),
                ProgressCircle::new().value(30.).into_any_element(),
                ProgressCircle::new()
                    .is_indeterminate(true)
                    .into_any_element(),
            ])
            .into_any_element()
    });
    harness::settle(cx);
    // Only the indeterminate ring's endless spin may still be animating.
    assert!(
        pending_animation_floor(cx) <= 1,
        "determinate fills must settle on their own keyed slots"
    );
}

/// The per-instance fallback key must not cost the fill transition: an
/// id-less ring whose value changes still eases to the new fraction instead
/// of snapping, because the signature slot seeds the new tween from the
/// fraction last painted.
#[gpui::test]
fn idless_progress_circle_value_change_still_eases(cx: &mut TestAppContext) {
    let value = Rc::new(RefCell::new(25.0f32));
    let for_view = value.clone();
    let cx = open_host(cx, move || {
        let now = *for_view.borrow();
        ProgressCircle::new().value(now).into_any_element()
    });
    harness::settle(cx);
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });

    *value.borrow_mut() = 85.0;
    harness::settle(cx);
    let mid_flight = cx.update(|window, cx| window.simulate_next_frame(cx));
    assert!(
        mid_flight >= 1,
        "an id-less value change must mount the fill animation, not snap"
    );
    assert_eq!(
        pending_animation_floor(cx),
        0,
        "the fill animation must settle once it reaches the new fraction"
    );
}

// ---------------------------------------------------------------------------
// Spinner / Skeleton / Badge / Avatar: every documented variant renders
// ---------------------------------------------------------------------------

/// Spinner is appearance-only — its v3 table is `size`, `color`, `className`
/// and nothing that could change behaviour or report a callback. The value
/// this smoke adds is the render: every documented size (sm/md/lg/xl) and
/// every documented colour (current via `Color::Default`, accent, success,
/// warning, danger) in one window, plus the `duration_ms` speed setter used by
/// v3's Speed example. A variant that panicked drawing itself (the
/// `Button::content` class of defect) dies at `open_host`.
#[gpui::test]
fn spinner_renders_every_documented_size_and_color(cx: &mut TestAppContext) {
    open_host(cx, || {
        let sizes = [
            SpinnerSize::Sm,
            SpinnerSize::Md,
            SpinnerSize::Lg,
            SpinnerSize::Xl,
        ];
        let colors = [
            Color::Default,
            Color::Accent,
            Color::Success,
            Color::Warning,
            Color::Danger,
        ];
        let mut row = gpui::div().flex().flex_wrap().gap(px(8.));
        for (i, size) in sizes.into_iter().enumerate() {
            for (j, color) in colors.into_iter().enumerate() {
                row = row.child(
                    Spinner::new(gpui::ElementId::Name(format!("fb-spinner-{i}-{j}").into()))
                        .size(size)
                        .color(color),
                );
            }
        }
        row.child(Spinner::new("fb-spinner-slow").duration_ms(2000))
            .into_any_element()
    });
}

#[gpui::test]
fn spinner_stops_scheduling_frames_when_motion_is_disabled(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, || Spinner::new("motion-spinner").into_any_element());
    assert_eq!(cx.update(|window, cx| window.simulate_next_frame(cx)), 0);
    cx.update(|_, cx| herogpui_theme::set_reduce_motion(false, cx));
    flush_frame(cx);
    assert!(cx.update(|window, cx| window.simulate_next_frame(cx)) > 0);
    cx.update(|_, cx| herogpui_theme::set_reduce_motion(true, cx));
    flush_frame(cx);
    // A previously queued frame may still run once after the preference changes.
    cx.update(|window, cx| window.simulate_next_frame(cx));
    assert_eq!(cx.update(|window, cx| window.simulate_next_frame(cx)), 0);
}

#[gpui::test]
fn spinner_keeps_its_diameter_when_a_flex_parent_is_smaller(cx: &mut TestAppContext) {
    for vertical in [false, true] {
        let measured = Rc::new(RefCell::new(None));
        let captured = measured.clone();
        open_host(cx, move || {
            let captured = captured.clone();
            gpui::div()
                .flex()
                .when(vertical, |row| row.flex_col())
                .size(px(8.))
                .child(Spinner::new("constrained-spinner"))
                .child(
                    canvas(
                        move |bounds, _, _| *captured.borrow_mut() = Some(bounds),
                        |_, _, _, _| {},
                    )
                    .size(px(1.))
                    .flex_shrink_0(),
                )
                .into_any_element()
        });
        let bounds = measured.borrow().expect("following marker must render");
        let offset = if vertical {
            bounds.origin.y
        } else {
            bounds.origin.x
        };
        assert_eq!(offset, px(24.), "Spinner must keep its 24px diameter");
    }
}

/// Wraps one spinner in a content-hugging flex row carrying a debug selector,
/// so the spinner's own box is readable through `debug_bounds` without adding
/// debug furniture to the component's rendered tree.
fn boxed_spinner(selector: &'static str, spinner: Spinner) -> AnyElement {
    gpui::div()
        .flex()
        .debug_selector(move || selector.to_owned())
        .child(spinner)
        .into_any_element()
}

/// `size_px` overrides the diameter the size step derives: the box renders at
/// the named pixels, the explicit override wins in either builder order, and
/// a spinner without the override keeps the documented 24px default.
#[gpui::test]
fn spinner_size_px_overrides_the_derived_diameter(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(8.))
            .child(boxed_spinner(
                "spin-px-14",
                Spinner::new("spin-px-14-spinner").size_px(px(14.)),
            ))
            .child(boxed_spinner(
                "spin-px-after-size",
                Spinner::new("spin-px-after-size-spinner")
                    .size(SpinnerSize::Xl)
                    .size_px(px(14.)),
            ))
            .child(boxed_spinner(
                "spin-px-before-size",
                Spinner::new("spin-px-before-size-spinner")
                    .size_px(px(14.))
                    .size(SpinnerSize::Xl),
            ))
            .child(boxed_spinner(
                "spin-default",
                Spinner::new("spin-default-spinner"),
            ))
            .into_any_element()
    });
    for (selector, expected) in [
        ("spin-px-14", 14.),
        ("spin-px-after-size", 14.),
        ("spin-px-before-size", 14.),
        ("spin-default", 24.),
    ] {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} box must paint"));
        assert_eq!(bounds.size.width, px(expected), "{selector} width");
        assert_eq!(bounds.size.height, px(expected), "{selector} height");
    }
}

/// `glyph` renders the caller's element in place of the arc, laid out centered
/// inside the spinner-owned diameter box, and the rotation machinery is
/// unchanged: with motion on the spinner still schedules animation frames and
/// `duration_ms` still feeds the same repeated animation.
#[gpui::test]
fn spinner_glyph_replaces_the_arc_and_keeps_spinning(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .child(
                boxed_spinner(
                    "spin-glyph",
                    Spinner::new("spin-glyph-spinner")
                        .size_px(px(14.))
                        .duration_ms(1500)
                        .glyph(
                            gpui::svg()
                                .size(px(10.))
                                .path(herogpui_components::icons::PLUS)
                                .debug_selector(|| "spin-glyph-svg".into()),
                        ),
                )
                .into_any_element(),
            )
            .into_any_element()
    });
    let box_bounds = cx
        .debug_bounds("spin-glyph")
        .expect("spinner box must paint");
    assert_eq!(
        box_bounds.size.width,
        px(14.),
        "the box keeps size_px's diameter"
    );
    let glyph_bounds = cx
        .debug_bounds("spin-glyph-svg")
        .expect("the caller glyph must render in place of the arc");
    assert_eq!(
        glyph_bounds.size.width,
        px(10.),
        "the glyph keeps its own 10px layout box"
    );
    assert_eq!(
        glyph_bounds.origin.x - box_bounds.origin.x,
        px(2.),
        "the glyph is centered in the diameter box"
    );
    assert!(
        cx.update(|window, cx| window.simulate_next_frame(cx)) > 0,
        "a glyph spinner must keep scheduling rotation frames"
    );
}

/// The glyph variant keeps the reduce-motion suppression too: the caller
/// element replaces the arc but nothing schedules frames, and with no
/// `size_px` the container still takes the documented 24px diameter.
#[gpui::test]
fn spinner_glyph_stops_scheduling_when_motion_is_disabled(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .child(
                boxed_spinner(
                    "spin-glyph-still",
                    Spinner::new("spin-glyph-still-spinner").glyph(
                        gpui::div()
                            .size(px(12.))
                            .bg(gpui::black())
                            .debug_selector(|| "spin-glyph-still-div".into()),
                    ),
                )
                .into_any_element(),
            )
            .into_any_element()
    });
    let box_bounds = cx
        .debug_bounds("spin-glyph-still")
        .expect("spinner box must paint");
    assert_eq!(box_bounds.size.width, px(24.), "the default diameter holds");
    let glyph_bounds = cx
        .debug_bounds("spin-glyph-still-div")
        .expect("the caller glyph must render in place of the arc");
    assert_eq!(glyph_bounds.size.width, px(12.));
    assert_eq!(
        cx.update(|window, cx| window.simulate_next_frame(cx)),
        0,
        "reduced motion must leave a glyph spinner static"
    );
}

/// Skeleton's v3 table is `animationType` plus `className` — no callback and
/// no state, so the only behavioural claim is renderability: the default
/// (deferred to the `--skeleton-animation` token), `none`, `pulse` and
/// `shimmer` each draw without panicking, with a sized box and with children.
/// The pulse and shimmer paths run their animation closures here (no reduced
/// motion in this test), which is the half a screenshot of a still page never
/// exercises.
#[gpui::test]
fn skeleton_renders_every_animation_type(cx: &mut TestAppContext) {
    open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                Skeleton::new()
                    .id("fb-skeleton-none")
                    .w(px(120.))
                    .h(px(24.))
                    .animation_type(SkeletonAnimation::None),
            )
            .child(
                Skeleton::new()
                    .id("fb-skeleton-pulse")
                    .w(px(120.))
                    .h(px(24.))
                    .animation_type(SkeletonAnimation::Pulse),
            )
            .child(
                Skeleton::new()
                    .id("fb-skeleton-shimmer")
                    .w(px(120.))
                    .h(px(24.))
                    .animation_type(SkeletonAnimation::Shimmer),
            )
            .child(
                Skeleton::new()
                    .id("fb-skeleton-default")
                    .w(px(120.))
                    .h(px(24.)),
            )
            .child(
                Skeleton::new()
                    .id("fb-skeleton-child")
                    .w(px(120.))
                    .h(px(24.))
                    .child(gpui::div().child("x")),
            )
            .into_any_element()
    });
}

/// Badge's v3 table is children (dot when omitted), color, variant, size and
/// placement — all appearance, with the anchor relation the only structure.
/// Every variant, size, the four placements and the dot half render in one
/// window; a variant that breaks at draw time (not a single one is
/// interactive enough to be clickable) dies at `open_host`.
#[gpui::test]
fn badge_renders_every_variant_size_placement_and_dot(cx: &mut TestAppContext) {
    open_host(cx, || {
        let mut col = gpui::div().flex().flex_col().gap(px(16.));
        // Variant × size with content; a distinct anchor child per badge.
        for (i, variant) in BadgeVariant::ALL.into_iter().enumerate() {
            for (j, size) in [Size::Sm, Size::Md, Size::Lg].into_iter().enumerate() {
                col = col.child(
                    BadgeAnchor::new()
                        .child(gpui::div().w(px(40.)).h(px(40.)))
                        .child(
                            Badge::new()
                                .variant(variant)
                                .size(size)
                                .color(Color::Accent)
                                .child(BadgeLabel::new().child((i * 3 + j).to_string())),
                        ),
                );
            }
        }
        // Every placement.
        for placement in [
            BadgePlacement::TopLeft,
            BadgePlacement::TopRight,
            BadgePlacement::BottomLeft,
            BadgePlacement::BottomRight,
        ] {
            col = col.child(
                BadgeAnchor::new()
                    .child(gpui::div().w(px(40.)).h(px(40.)))
                    .child(
                        Badge::new()
                            .placement(placement)
                            .child(BadgeLabel::new().child("5")),
                    ),
            );
        }
        // Dot badges (label omitted) in every size.
        for size in [Size::Sm, Size::Md, Size::Lg] {
            col = col.child(
                BadgeAnchor::new()
                    .child(gpui::div().w(px(40.)).h(px(40.)))
                    .child(Badge::new().size(size).color(Color::Success)),
            );
        }
        col.into_any_element()
    });
}

// ---------------------------------------------------------------------------
// Badge geometry: the 25% anchor overhang, the pinned leading, `shrink-0`
// ---------------------------------------------------------------------------

/// The badge's own painted box, read through the component's debug selector.
/// The selector map keeps only the last painted match, so every probed badge
/// gets its own window.
fn badge_box(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    cx.debug_bounds("badge")
        .unwrap_or_else(|| panic!("badge must paint"))
}

fn bounds_str(b: &Bounds<Pixels>) -> String {
    format!(
        "{:.1}..{:.1} x {:.1}..{:.1}",
        f32::from(b.origin.x),
        f32::from(b.origin.x + b.size.width),
        f32::from(b.origin.y),
        f32::from(b.origin.y + b.size.height)
    )
}

/// A 64px anchor whose top-left sits at (100, 40), via a padded flex row, with
/// an optional dot/label badge of the given size. The row is flex because the
/// anchor wrapper only hugs its child there: v3's `.badge-anchor` is
/// `inline-flex`, which GPUI 0.2.2 has no equivalent of (a block parent
/// stretches the wrapper across its width).
fn anchored_badge(
    placement: BadgePlacement,
    content: Option<&'static str>,
    size: Size,
) -> AnyElement {
    let mut badge = Badge::new().size(size).placement(placement);
    if let Some(text) = content {
        badge = badge.child(BadgeLabel::new().child(text));
    }
    gpui::div()
        .pt(px(40.))
        .pl(px(100.))
        .flex()
        .child(
            BadgeAnchor::new()
                .child(gpui::div().w(px(64.)).h(px(64.)))
                .child(badge),
        )
        .into_any_element()
}

/// v3 anchors every badge to its anchor's corner with `top/right/bottom/left:
/// 0` plus `transform: translate(±25%, ±25%)` (`badge.css`, placement block) —
/// an outward overhang of a quarter of the badge's own box: 4px sm, 7px md,
/// 8px lg. Dot badges make the box exactly the min size, so the overhang is
/// directly readable: an md dot on a 64px anchor at (100, 40) must span x
/// 143..171, y 33..61. The port applies the translate from the badge's
/// laid-out box at prepaint, so a badge grown past its min box (a longer
/// label) overhangs its quarter-box too — pinned in `badge_parts.rs`.
#[gpui::test]
fn badge_overhangs_the_anchor_by_a_quarter_of_its_box(cx: &mut TestAppContext) {
    {
        let cx = open_host(cx, || {
            anchored_badge(BadgePlacement::TopRight, None, Size::Md)
        });
        assert_eq!(
            bounds_str(&badge_box(cx)),
            "143.0..171.0 x 33.0..61.0",
            "the md top-right dot must overhang 7px past the anchor's corner"
        );
    }
    // lg: 32px box, 8px overhang → x 92..124, y 32..64.
    {
        let cx = open_host(cx, || {
            anchored_badge(BadgePlacement::TopLeft, None, Size::Lg)
        });
        assert_eq!(
            bounds_str(&badge_box(cx)),
            "92.0..124.0 x 32.0..64.0",
            "the lg top-left dot must overhang 8px past the anchor's corner"
        );
    }
    // sm: 16px box, 4px overhang → x 96..112, y 92..108.
    {
        let cx = open_host(cx, || {
            anchored_badge(BadgePlacement::BottomLeft, None, Size::Sm)
        });
        assert_eq!(
            bounds_str(&badge_box(cx)),
            "96.0..112.0 x 92.0..108.0",
            "the sm bottom-left dot must overhang 4px past the anchor's corner"
        );
    }
    // A labelled md badge keeps the same 28px min box, so the same corner
    // geometry holds with content ("5" plus `badge__label`'s px-0.5 still
    // fits the min width).
    {
        let cx = open_host(cx, || {
            anchored_badge(BadgePlacement::TopRight, Some("5"), Size::Md)
        });
        assert_eq!(
            bounds_str(&badge_box(cx)),
            "143.0..171.0 x 33.0..61.0",
            "the md top-right label badge must overhang 7px past the anchor's corner"
        );
    }
}

/// The pinned leading from `badge.css`: `.badge` and `.badge--sm` are
/// `leading-[1.34]`, `.badge--lg` is `leading-[1.43]` — unitless multipliers
/// of the badge's own text size. Read through a paint-time text-style probe
/// inside the badge's label.
#[gpui::test]
fn badge_leading_matches_the_pinned_css(cx: &mut TestAppContext) {
    for (size, expected) in [
        (Size::Sm, "10.00/13.40"),
        (Size::Md, "12.00/16.08"),
        (Size::Lg, "14.00/20.02"),
    ] {
        let seen = Rc::new(RefCell::new(None::<(f32, f32)>));
        {
            open_host(cx, {
                let seen = seen.clone();
                move || {
                    gpui::div()
                        .pt(px(40.))
                        .pl(px(100.))
                        .flex()
                        .child(
                            BadgeAnchor::new()
                                .child(gpui::div().w(px(64.)).h(px(64.)))
                                .child(
                                    Badge::new().size(size).child(
                                        canvas(|_, _, _| {}, {
                                            let seen = seen.clone();
                                            move |_, _, window, _| {
                                                let style = window.text_style();
                                                let rem = window.rem_size();
                                                let font = style.font_size.to_pixels(rem);
                                                *seen.borrow_mut() = Some((
                                                    f32::from(font),
                                                    f32::from(style.line_height.to_pixels(
                                                        AbsoluteLength::Pixels(font),
                                                        rem,
                                                    )),
                                                ));
                                            }
                                        })
                                        .size_0(),
                                    ),
                                ),
                        )
                        .into_any_element()
                }
            });
        }
        let (font, leading) = seen.borrow().unwrap_or_else(|| panic!("probe must paint"));
        assert_eq!(
            format!("{font:.2}/{leading:.2}"),
            expected,
            "{size:?} badge must carry its pinned text size and leading"
        );
    }
}

/// `.badge-anchor` is `relative inline-flex shrink-0`; GPUI's default
/// `flex_shrink` is 1.0, so without the ported `flex_shrink_0` the anchor
/// wrapper compresses in an overflowing row: an 80px row with a 64px anchor
/// and a 64px sibling shrinks the wrapper to 40px, while `shrink-0` must keep
/// it at the 64px content width.
#[gpui::test]
fn badge_anchor_keeps_its_width_in_an_overflowing_row(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .pt(px(20.))
            .flex()
            .w(px(80.))
            .child(
                BadgeAnchor::new()
                    .child(gpui::div().w(px(64.)).h(px(16.)))
                    .child(Badge::new().child(BadgeLabel::new().child("5"))),
            )
            .child(gpui::div().w(px(64.)).h(px(16.)))
            .into_any_element()
    });
    let anchor = cx
        .debug_bounds("badge-anchor")
        .unwrap_or_else(|| panic!("badge anchor must paint"));
    assert_eq!(
        format!("{:.1}", f32::from(anchor.size.width)),
        "64.0",
        "the badge anchor must not shrink below its content in an overflowing row"
    );
}

/// Avatar's v3 table is size/color/variant on the root, `Avatar.Image`
/// (src/onLoad/onError) and `Avatar.Fallback` (initials, delayMs). Every
/// variant renders here with and without a `src`; the load half is the one
/// behaviour the port genuinely lacks and the platform cannot observe:
/// v3's "Fallback with delay" example shows a broken src being replaced by
/// the fallback, but this port chooses image-or-initials at build time and
/// exposes no `onError`, so a failing `src` draws an empty box. The test
/// platform's asset source answers `Ok(None)` for every path — the port's
/// docs note that a missing svg loads as None — so the src-bearing avatar
/// below *always* fails to load, and the smoke is that it renders without
/// panicking.
#[gpui::test]
fn avatar_renders_every_variant_with_and_without_src(cx: &mut TestAppContext) {
    open_host(cx, || {
        let fallback = Avatar::new("fallback").name("Jane Doe");
        let soft = Avatar::new("soft")
            .name("Jane Doe")
            .variant(AvatarVariant::Soft)
            .color(Color::Accent);
        let large = Avatar::new("large")
            .name("Jane Doe")
            .size(Size::Lg)
            .color(Color::Success);
        // A src that cannot load anywhere (the platform has no asset source);
        // v3 would fall back to initials, this port's static choice keeps the
        // broken box.
        let with_src = Avatar::new("with-src")
            .name("Jane Doe")
            .src("fb-avatar-missing.png")
            .color(Color::Warning);
        let small_src = Avatar::new("small-src")
            .name("Jane Doe")
            .size(Size::Sm)
            .src("fb-avatar-missing.png");
        gpui::div()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .child(fallback)
            .child(soft)
            .child(large)
            .child(with_src)
            .child(small_src)
            .into_any_element()
    });
}

#[gpui::test]
fn progress_and_meter_text_keep_twenty_pixel_lines(cx: &mut TestAppContext) {
    for leading in [None, Some(48.)] {
        for meter in [false, true] {
            for label_only in [false, true] {
                let cx = open_host(cx, move || {
                    let content = if meter {
                        Meter::new("leading-meter", 50.)
                            .label(if label_only { "First\nSecond" } else { "Label" })
                            .show_value(!label_only)
                            .value_content(|_, _| {
                                gpui::div().child("50\npercent").into_any_element()
                            })
                            .into_any_element()
                    } else {
                        ProgressBar::new("leading-progress")
                            .value(50.)
                            .label(if label_only { "First\nSecond" } else { "Label" })
                            .show_value_label(!label_only)
                            .value_content(|_, _, _| {
                                gpui::div().child("50\npercent").into_any_element()
                            })
                            .into_any_element()
                    };
                    gpui::div()
                        .w(px(300.))
                        .when_some(leading, |el, leading| el.line_height(px(leading)))
                        .child(
                            gpui::div()
                                .debug_selector(|| "leading-bar".into())
                                .child(content),
                        )
                        .into_any_element()
                });
                let bounds = cx.debug_bounds("leading-bar").expect("bar must paint");
                assert_eq!(bounds.size.height, px(52.), "two 20px lines, 4px gap and 8px track; meter={meter}, label_only={label_only}, host={leading:?}");
            }
        }
    }
}

#[gpui::test]
fn toast_action_moves_below_text_on_narrow_windows_and_still_activates(cx: &mut TestAppContext) {
    still();
    let actions = events();
    let seen = actions.clone();
    cx.update(move |cx| {
        Toast::new("Saved")
            .timeout(Duration::ZERO)
            .action("Undo", move |_| seen.borrow_mut().push("undo".into()))
            .push(None, cx);
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());
    let wide_title = cx.debug_bounds("toast-title").unwrap();
    let wide_action = cx.debug_bounds("toast-action").unwrap();
    assert!(wide_action.left() >= wide_title.right());
    cx.simulate_resize(gpui::size(px(500.), px(800.)));
    flush_frame(cx);
    let narrow_title = cx.debug_bounds("toast-title").unwrap();
    let narrow_action = cx.debug_bounds("toast-action").unwrap();
    assert!(narrow_action.top() >= narrow_title.bottom() + px(8.));
    assert!((f32::from(narrow_action.left() - narrow_title.left())).abs() < 0.5);
    let at = narrow_action.center();
    click(cx, at.x.into(), at.y.into());
    assert_eq!(&*actions.borrow(), &["undo"]);
    cx.update(|_, cx| assert!(toast_store(cx).read(cx).toasts().is_empty()));
}
