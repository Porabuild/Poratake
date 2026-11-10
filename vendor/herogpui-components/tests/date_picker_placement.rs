//! Placement contracts for the DatePicker family.
//!
//! HeroUI v3 exposes the composed calendar popover's `placement` prop and
//! defaults it to `bottom`. These tests measure the rendered panel relative to
//! its trigger so a future refactor cannot silently flatten the builder back
//! to the old bottom-start-only behavior, and they exercise the shared
//! positioner's opposite-side flip when the preferred side overflows.

use crate::harness;

use gpui::{prelude::*, px, size, TestAppContext};
use herogpui_components::{CalendarState, DatePicker, DateRangePicker, DateRangeState};

use harness::{open_host, still};

fn assert_below(cx: &mut gpui::VisualTestContext, anchor: &'static str, panel: &'static str) {
    let anchor = cx
        .debug_bounds(anchor)
        .expect("the picker trigger wrapper must be laid out");
    let panel = cx
        .debug_bounds(panel)
        .expect("the open picker popover must be laid out");
    assert!(
        f32::from(panel.top()) >= f32::from(anchor.bottom()) + 8. - 1.5,
        "bottom placement must leave the eight-pixel gap: panel={panel:?} anchor={anchor:?}"
    );
}

fn assert_above(cx: &mut gpui::VisualTestContext, anchor: &'static str, panel: &'static str) {
    let anchor = cx
        .debug_bounds(anchor)
        .expect("the picker trigger wrapper must be laid out");
    let panel = cx
        .debug_bounds(panel)
        .expect("the open picker popover must be laid out");
    assert!(
        f32::from(panel.bottom()) <= f32::from(anchor.top()) - 8. + 1.5,
        "top placement must leave the eight-pixel gap: panel={panel:?} anchor={anchor:?}"
    );
}

#[gpui::test]
fn date_picker_defaults_to_bottom(cx: &mut TestAppContext) {
    still();
    let state = cx.new(|cx| CalendarState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .pt(px(320.))
            .child(
                gpui::div()
                    .w(px(320.))
                    .debug_selector(|| "date-placement-anchor".into())
                    .child(DatePicker::new(state.clone()).default_open(true)),
            )
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    assert_below(cx, "date-placement-anchor", "date-picker-popover");
}

#[gpui::test]
fn date_picker_accepts_top_placement(cx: &mut TestAppContext) {
    still();
    let state = cx.new(|cx| CalendarState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .pt(px(320.))
            .child(
                gpui::div()
                    .w(px(320.))
                    .debug_selector(|| "date-placement-anchor".into())
                    .child(
                        DatePicker::new(state.clone())
                            .placement(herogpui_core::Placement::Top)
                            .default_open(true),
                    ),
            )
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    assert_above(cx, "date-placement-anchor", "date-picker-popover");
}

#[gpui::test]
fn date_range_picker_accepts_top_placement(cx: &mut TestAppContext) {
    still();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .pt(px(320.))
            .child(
                gpui::div()
                    .w(px(360.))
                    .debug_selector(|| "date-range-placement-anchor".into())
                    .child(
                        DateRangePicker::new(state.clone())
                            .placement(herogpui_core::Placement::Top)
                            .default_open(true),
                    ),
            )
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    assert_above(
        cx,
        "date-range-placement-anchor",
        "date-range-picker-popover",
    );
}

#[gpui::test]
fn date_picker_flips_above_when_bottom_overflows(cx: &mut TestAppContext) {
    still();
    let state = cx.new(|cx| CalendarState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .mt(px(1000.))
            .child(
                gpui::div()
                    .debug_selector(|| "date-flip-anchor".to_owned())
                    .child(DatePicker::new(state.clone()).default_open(true)),
            )
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    assert_above(cx, "date-flip-anchor", "date-picker-popover");
}

#[gpui::test]
fn date_range_picker_flips_above_when_bottom_overflows(cx: &mut TestAppContext) {
    still();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .mt(px(1000.))
            .child(
                gpui::div()
                    .debug_selector(|| "date-range-flip-anchor".to_owned())
                    .child(DateRangePicker::new(state.clone()).default_open(true)),
            )
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    assert_above(cx, "date-range-flip-anchor", "date-range-picker-popover");
}

#[gpui::test]
fn date_picker_scrolls_inside_a_short_viewport(cx: &mut TestAppContext) {
    still();
    let state = cx.new(|cx| CalendarState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .debug_selector(|| "date-short-anchor".to_owned())
            .child(DatePicker::new(state.clone()).default_open(true))
            .into_any_element()
    });

    // A one-month calendar is taller than this window. The picker must cap its
    // scroll surface to the available viewport instead of leaking below it.
    cx.simulate_resize(size(px(320.), px(180.)));
    cx.update(|window, _| window.refresh());
    let panel = cx
        .debug_bounds("date-picker-popover")
        .expect("the short-viewport picker popover must be laid out");
    let viewport = cx.update(|window, _| window.viewport_size());
    assert!(
        f32::from(panel.bottom()) <= f32::from(viewport.height) - 12. + 1.5,
        "picker panel must stay within the viewport inset: panel={panel:?} viewport={viewport:?}"
    );
    assert!(
        f32::from(panel.size.height) < 300.,
        "a short viewport must cap the calendar rather than keep its natural height: panel={panel:?}"
    );
}

#[gpui::test]
fn date_range_picker_scrolls_inside_a_short_viewport(cx: &mut TestAppContext) {
    still();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .debug_selector(|| "date-range-short-anchor".to_owned())
            .child(DateRangePicker::new(state.clone()).default_open(true))
            .into_any_element()
    });

    cx.simulate_resize(size(px(360.), px(180.)));
    cx.update(|window, _| window.refresh());
    let panel = cx
        .debug_bounds("date-range-picker-popover")
        .expect("the short-viewport range picker popover must be laid out");
    let viewport = cx.update(|window, _| window.viewport_size());
    assert!(
        f32::from(panel.bottom()) <= f32::from(viewport.height) - 12. + 1.5,
        "range picker panel must stay within the viewport inset: panel={panel:?} viewport={viewport:?}"
    );
    assert!(
        f32::from(panel.size.height) < 300.,
        "a short viewport must cap the range calendar rather than keep its natural height: panel={panel:?}"
    );
}

/// A side-plus-alignment placement reaches the calendar panel end to end:
/// `EndTop` on a trigger at the window's top-left hangs beside the trigger's
/// end edge, top-aligned with it — shifted onto the 12px viewport inset,
/// exactly as React Aria's `getDelta` cross-axis shift requires for a
/// trigger against the edge — and stays inside the viewport.
#[gpui::test]
fn date_picker_end_top_hangs_beside_the_trigger_top_aligned(cx: &mut TestAppContext) {
    still();
    let state = cx.new(|cx| CalendarState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .child(
                gpui::div()
                    .w(px(320.))
                    .debug_selector(|| "date-side-anchor".into())
                    .child(
                        DatePicker::new(state.clone())
                            .placement(herogpui_core::Placement::EndTop)
                            .default_open(true),
                    ),
            )
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    let anchor = cx
        .debug_bounds("date-side-anchor")
        .expect("the picker trigger wrapper must be laid out");
    let panel = cx
        .debug_bounds("date-picker-popover")
        .expect("the open picker popover must be laid out");
    let viewport = cx.update(|window, _| window.viewport_size());

    assert!(
        f32::from(panel.left()) >= f32::from(anchor.right()) + 8. - 1.5,
        "a side panel must hang one gap clear of the trigger's end edge: \
         panel={panel:?} anchor={anchor:?}"
    );
    assert!(
        f32::from(panel.top()) >= 12. - 1.5
            && (f32::from(panel.top()) - f32::from(anchor.top().max(px(12.)))).abs() < 1.5,
        "the top alignment pins the panel toward the trigger's top edge and \
         never above the viewport inset: panel={panel:?} anchor={anchor:?}"
    );
    assert!(
        f32::from(panel.right()) <= f32::from(viewport.width) - 12. + 1.5
            && f32::from(panel.bottom()) <= f32::from(viewport.height) - 12. + 1.5,
        "the side panel must stay inside the viewport insets: panel={panel:?} \
         viewport={viewport:?}"
    );
}

/// The logical aliases are the same value as their physical spellings in this
/// LTR-only port, so the range panel they open must land on identical
/// pixels: `StartBottom` is `LeftBottom`, flipped to the end side because a
/// top-left trigger has no room on its start side.
#[gpui::test]
fn date_range_picker_start_bottom_matches_left_bottom(cx: &mut TestAppContext) {
    for placement in [
        herogpui_core::Placement::LeftBottom,
        herogpui_core::Placement::StartBottom,
    ] {
        still();
        let state = cx.new(|cx| DateRangeState::new(cx));
        let cx = open_host(cx, move || {
            gpui::div()
                .child(
                    gpui::div()
                        .w(px(320.))
                        .debug_selector(|| "date-alias-anchor".into())
                        .child(
                            DateRangePicker::new(state.clone())
                                .placement(placement)
                                .default_open(true),
                        ),
                )
                .into_any_element()
        });

        cx.update(|window, _| window.refresh());
        let anchor = cx
            .debug_bounds("date-alias-anchor")
            .expect("the range trigger wrapper must be laid out");
        let panel = cx
            .debug_bounds("date-range-picker-popover")
            .expect("the open range popover must be laid out");

        assert!(
            f32::from(panel.left()) >= f32::from(anchor.right()) + 8. - 1.5,
            "{placement:?} must open beside the trigger's end edge: \
             panel={panel:?} anchor={anchor:?}"
        );

        if placement == herogpui_core::Placement::LeftBottom {
            LEFT_BOTTOM_PANEL.with(|cell| *cell.borrow_mut() = Some(panel));
        } else {
            LEFT_BOTTOM_PANEL.with(|cell| {
                let physical = cell
                    .borrow()
                    .expect("the physical spelling must open first");
                assert_eq!(
                    panel, physical,
                    "{placement:?} must land exactly where LeftBottom does in \
                     this LTR-only port"
                );
            });
        }
    }
}

thread_local! {
    /// The `LeftBottom` panel bounds captured by
    /// `date_range_picker_start_bottom_matches_left_bottom` for its alias
    /// comparison.
    static LEFT_BOTTOM_PANEL: std::cell::RefCell<Option<gpui::Bounds<gpui::Pixels>>> =
        const { std::cell::RefCell::new(None) };
}
