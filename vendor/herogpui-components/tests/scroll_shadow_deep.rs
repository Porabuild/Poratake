//! Behaviour coverage for ScrollShadow paths not exercised by the vertical
//! visibility tests in `virtual_and_feedback.rs`.

use crate::harness;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{point, prelude::*, px, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext};

use harness::open_host;
use herogpui_components::{Orientation, ScrollShadow, ScrollShadowVisibility};

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn wheel_h(cx: &mut VisualTestContext, x: f32, y: f32, dx: f32) {
    cx.simulate_event(ScrollEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Pixels(point(px(dx), px(0.))),
        ..Default::default()
    });
    flush_frame(cx);
}

/// Horizontal orientation must derive visibility from the x-axis. Ten 40px
/// blocks with 8px gaps make 472px of content in a 160px viewport, so the
/// exact 312px range can be driven from edge to edge without relying on a
/// screenshot of the gradients.
#[gpui::test]
fn horizontal_scroll_shadow_reports_each_edge(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(Vec::new()));
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ScrollShadow::new("ss-horizontal")
            .orientation(Orientation::Horizontal)
            .max_w(px(160.))
            .visibility(ScrollShadowVisibility::Auto)
            .on_visibility_change(move |visibility, _, _| {
                recorded.borrow_mut().push(*visibility);
            })
            .children((0..10).map(|_| {
                gpui::div()
                    .w(px(40.))
                    .h(px(40.))
                    .flex_shrink_0()
                    .into_any_element()
            }))
            .into_any_element()
    });
    flush_frame(cx);

    assert_eq!(
        recorded.borrow().as_slice(),
        [ScrollShadowVisibility::Right],
        "at the left edge only the right shadow must be visible"
    );

    wheel_h(cx, 80., 20., -156.);
    assert_eq!(
        recorded.borrow().as_slice(),
        [ScrollShadowVisibility::Right, ScrollShadowVisibility::Both,],
        "mid-scroll both horizontal edges must be visible"
    );

    wheel_h(cx, 80., 20., -156.);
    assert_eq!(
        recorded.borrow().as_slice(),
        [
            ScrollShadowVisibility::Right,
            ScrollShadowVisibility::Both,
            ScrollShadowVisibility::Left,
        ],
        "at the right edge only the left shadow must be visible"
    );

    wheel_h(cx, 80., 20., 312.);
    assert_eq!(
        recorded.borrow().as_slice(),
        [
            ScrollShadowVisibility::Right,
            ScrollShadowVisibility::Both,
            ScrollShadowVisibility::Left,
            ScrollShadowVisibility::Right,
        ],
        "returning to the left edge must restore the right shadow"
    );
}

/// `offset` delays both edge transitions. With a 24px offset, exactly 24px of
/// movement must keep the leading shadow hidden; the 25th pixel reveals it.
/// The trailing shadow likewise disappears 24px before the physical end.
#[gpui::test]
fn horizontal_scroll_shadow_honours_offset_at_both_ends(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(Vec::new()));
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ScrollShadow::new("ss-horizontal-offset")
            .orientation(Orientation::Horizontal)
            .max_w(px(160.))
            .offset(px(24.))
            .on_visibility_change(move |visibility, _, _| {
                recorded.borrow_mut().push(*visibility);
            })
            .children((0..10).map(|_| {
                gpui::div()
                    .w(px(40.))
                    .h(px(40.))
                    .flex_shrink_0()
                    .into_any_element()
            }))
            .into_any_element()
    });
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        [ScrollShadowVisibility::Right]
    );

    wheel_h(cx, 80., 20., -24.);
    assert_eq!(
        recorded.borrow().as_slice(),
        [ScrollShadowVisibility::Right],
        "exactly the leading offset must not reveal the left shadow"
    );
    wheel_h(cx, 80., 20., -1.);
    assert_eq!(
        recorded.borrow().as_slice(),
        [ScrollShadowVisibility::Right, ScrollShadowVisibility::Both,],
        "the first pixel past the offset must reveal both shadows"
    );

    // At -287 the right edge is still 25px away; one more pixel reaches the
    // 24px trailing threshold and hides that shadow.
    wheel_h(cx, 80., 20., -262.);
    assert_eq!(recorded.borrow().len(), 2);
    wheel_h(cx, 80., 20., -1.);
    assert_eq!(
        recorded.borrow().as_slice(),
        [
            ScrollShadowVisibility::Right,
            ScrollShadowVisibility::Both,
            ScrollShadowVisibility::Left,
        ],
        "the trailing shadow must disappear at its offset threshold"
    );

    wheel_h(cx, 80., 20., 264.);
    assert_eq!(
        recorded.borrow().as_slice(),
        [
            ScrollShadowVisibility::Right,
            ScrollShadowVisibility::Both,
            ScrollShadowVisibility::Left,
            ScrollShadowVisibility::Right,
        ],
        "returning to exactly the leading offset must hide the left shadow"
    );
}

/// v3's `useScrollShadow` subscribes and calls `onVisibilityChange` only while
/// `visibility === "auto"`. A controlled visibility is already the caller's
/// state, so rendering it must not echo that value back as a synthetic change.
#[gpui::test]
fn controlled_visibility_does_not_report_a_synthetic_change(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(Vec::new()));
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ScrollShadow::new("ss-controlled")
            .orientation(Orientation::Horizontal)
            .max_w(px(160.))
            .visibility(ScrollShadowVisibility::Both)
            .on_visibility_change(move |visibility, _, _| {
                recorded.borrow_mut().push(*visibility);
            })
            .children((0..10).map(|_| {
                gpui::div()
                    .w(px(40.))
                    .h(px(40.))
                    .flex_shrink_0()
                    .into_any_element()
            }))
            .into_any_element()
    });
    flush_frame(cx);
    wheel_h(cx, 80., 20., -156.);

    assert!(
        recorded.borrow().is_empty(),
        "an explicit visibility must render without emitting onVisibilityChange"
    );
}
