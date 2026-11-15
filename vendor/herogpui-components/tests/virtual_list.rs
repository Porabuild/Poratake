//! `VirtualList` (HeroGPUI extension): variable row heights are measured,
//! only rows near the viewport are built, and `scroll_to_item` moves a row
//! into view under both strategies.

use crate::harness;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use gpui::{prelude::*, px, TestAppContext, VisualTestContext};
use harness::open_host;
use herogpui_components::{VirtualList, VirtualListHandle, VirtualListScroll};

const ROWS: usize = 1_000;
const VIEWPORT: f32 = 300.;

/// Row `ix` is 20px tall, or 60px for every fifth row.
fn row_height(ix: usize) -> f32 {
    if ix.is_multiple_of(5) {
        60.
    } else {
        20.
    }
}

fn host(
    cx: &mut TestAppContext,
) -> (
    VirtualListHandle,
    Rc<RefCell<BTreeSet<usize>>>,
    &mut VisualTestContext,
) {
    let handle = VirtualListHandle::new(ROWS);
    let built = Rc::new(RefCell::new(BTreeSet::new()));
    let (h, b) = (handle.clone(), built.clone());
    let cx = open_host(cx, move || {
        let b = b.clone();
        VirtualList::new("vl", &h, move |ix, _, _| {
            b.borrow_mut().insert(ix);
            gpui::div()
                .h(px(row_height(ix)))
                .w_full()
                .into_any_element()
        })
        .height(px(VIEWPORT))
        .into_any_element()
    });
    (handle, built, cx)
}

fn frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

#[gpui::test]
fn only_rows_near_the_viewport_are_built(cx: &mut TestAppContext) {
    let (handle, built, cx) = host(cx);
    frame(cx);
    let built = built.borrow();
    assert!(built.contains(&0));
    assert!(built.len() < 60, "built {} of {ROWS} rows", built.len());
    assert!(!built.contains(&(ROWS - 1)));
    // Variable heights are measured, not assumed uniform.
    let first = handle.bounds_for_item(0).unwrap();
    let second = handle.bounds_for_item(1).unwrap();
    assert_eq!(first.size.height, px(60.));
    assert_eq!(second.size.height, px(20.));
    assert_eq!(second.origin.y, first.origin.y + px(60.));
}

#[gpui::test]
fn scroll_to_item_top_puts_the_row_at_the_top(cx: &mut TestAppContext) {
    let (handle, _, cx) = host(cx);
    frame(cx);
    handle.scroll_to_item(500, VirtualListScroll::Top);
    frame(cx);
    assert_eq!(handle.first_visible_item(), 500);
    let row = handle.bounds_for_item(500).expect("row 500 rendered");
    assert_eq!(row.origin.y, px(0.));
}

#[gpui::test]
fn scroll_to_item_reveal_brings_the_row_fully_into_view(cx: &mut TestAppContext) {
    let (handle, _, cx) = host(cx);
    frame(cx);
    // A row laid out in the overdraw band just below the viewport scrolls
    // the least distance: it lands on the bottom edge.
    let below = (0..ROWS)
        .find(|&ix| {
            handle
                .bounds_for_item(ix)
                .is_some_and(|b| b.bottom() > px(VIEWPORT))
        })
        .unwrap();
    handle.scroll_to_item(below, VirtualListScroll::Reveal);
    frame(cx);
    let row = handle.bounds_for_item(below).unwrap();
    assert!((f32::from(row.bottom()) - VIEWPORT).abs() < 0.5, "{row:?}");
    // An already visible row does not move.
    handle.scroll_to_item(below, VirtualListScroll::Reveal);
    frame(cx);
    assert_eq!(
        handle.bounds_for_item(below).unwrap().origin.y,
        row.origin.y
    );
    // A never-measured row far below still ends fully visible (at the top).
    handle.scroll_to_item(700, VirtualListScroll::Reveal);
    frame(cx);
    let far = handle.bounds_for_item(700).expect("row 700 rendered");
    assert!(
        far.origin.y >= px(0.) && far.bottom() <= px(VIEWPORT),
        "{far:?}"
    );
}

#[gpui::test]
fn item_count_changes_are_honoured(cx: &mut TestAppContext) {
    let (handle, built, cx) = host(cx);
    frame(cx);
    handle.set_item_count(3);
    built.borrow_mut().clear();
    frame(cx);
    assert_eq!(handle.item_count(), 3);
    assert_eq!(*built.borrow(), BTreeSet::from([0, 1, 2]));
    handle.splice(3..3, 2);
    frame(cx);
    assert_eq!(handle.item_count(), 5);
    // Out-of-range targets clamp to the last row instead of panicking.
    handle.scroll_to_item(99, VirtualListScroll::Top);
    frame(cx);
    assert!(handle.bounds_for_item(4).is_some());
}

// ---- uniform mode ----------------------------------------------------------

const UNIFORM_ROW: f32 = 30.;

fn uniform_host(
    cx: &mut TestAppContext,
) -> (
    VirtualListHandle,
    Rc<RefCell<BTreeSet<usize>>>,
    &mut VisualTestContext,
) {
    let handle = VirtualListHandle::uniform(ROWS);
    let built = Rc::new(RefCell::new(BTreeSet::new()));
    let (h, b) = (handle.clone(), built.clone());
    let cx = open_host(cx, move || {
        let b = b.clone();
        VirtualList::new("uvl", &h, move |ix, _, _| {
            b.borrow_mut().insert(ix);
            gpui::div().h(px(UNIFORM_ROW)).w_full().into_any_element()
        })
        .height(px(VIEWPORT))
        .into_any_element()
    });
    (handle, built, cx)
}

#[gpui::test]
fn uniform_list_builds_only_the_viewport_rows(cx: &mut TestAppContext) {
    let (handle, built, cx) = uniform_host(cx);
    frame(cx);
    assert!(handle.is_uniform());
    // 300 / 30 = 10 rows, plus row 0 measured.
    assert_eq!(*built.borrow(), (0..10).collect::<BTreeSet<_>>());
    assert_eq!(handle.viewport_bounds().size.height, px(VIEWPORT));
    let row = handle.bounds_for_item(3).expect("row 3 on screen");
    assert_eq!(row.origin.y, px(90.));
    assert_eq!(row.size.height, px(UNIFORM_ROW));
    assert!(handle.bounds_for_item(10).is_none());
    assert!(handle.is_scrolled_to_top());
    assert!(!handle.is_scrolled_to_end());
    // Row count x measured row, less the viewport.
    assert_eq!(
        handle.remaining_below(),
        Some(px(ROWS as f32 * UNIFORM_ROW - VIEWPORT))
    );
}

#[gpui::test]
fn uniform_center_centres_an_off_screen_row_and_clamps_at_the_ends(cx: &mut TestAppContext) {
    let (handle, _, cx) = uniform_host(cx);
    frame(cx);
    handle.scroll_to_item(500, VirtualListScroll::Center);
    frame(cx);
    let row = handle.bounds_for_item(500).expect("row 500 on screen");
    assert_eq!(row.center().y, px(VIEWPORT / 2.));
    // A row already fully visible does not move (non-strict centring).
    handle.scroll_to_item(499, VirtualListScroll::Center);
    frame(cx);
    assert_eq!(handle.bounds_for_item(500).unwrap().origin.y, row.origin.y);
    // The last row cannot be centred: the scroll clamps to the content end.
    handle.scroll_to_item(usize::MAX, VirtualListScroll::Center);
    frame(cx);
    let last = handle
        .bounds_for_item(ROWS - 1)
        .expect("last row on screen");
    assert_eq!(last.bottom(), px(VIEWPORT));
    assert!(handle.is_scrolled_to_end());
    assert_eq!(handle.remaining_below(), Some(px(0.)));
    // And the first row clamps to the top.
    handle.scroll_to_item(0, VirtualListScroll::Center);
    frame(cx);
    assert!(handle.is_scrolled_to_top());
    assert_eq!(handle.first_visible_item(), 0);
}

#[gpui::test]
fn uniform_top_reveal_and_scroll_by(cx: &mut TestAppContext) {
    let (handle, _, cx) = uniform_host(cx);
    frame(cx);
    handle.scroll_to_item(200, VirtualListScroll::Top);
    frame(cx);
    assert_eq!(handle.first_visible_item(), 200);
    assert_eq!(handle.bounds_for_item(200).unwrap().origin.y, px(0.));
    // Reveal scrolls the least distance: a row below lands on the bottom edge.
    handle.scroll_to_item(215, VirtualListScroll::Reveal);
    frame(cx);
    assert_eq!(handle.bounds_for_item(215).unwrap().bottom(), px(VIEWPORT));
    handle.scroll_by(px(-UNIFORM_ROW * 2.));
    frame(cx);
    // Row 213 ended 60px above the bottom edge; it now ends on it.
    assert_eq!(handle.bounds_for_item(213).unwrap().bottom(), px(VIEWPORT));
    assert!(handle.bounds_for_item(215).is_none(), "scrolled out below");
    // Clamped at the top.
    handle.scroll_by(px(-1.0e7));
    frame(cx);
    assert!(handle.is_scrolled_to_top());
}

#[gpui::test]
fn uniform_splice_keeps_the_scroll_and_set_item_count_resets_it(cx: &mut TestAppContext) {
    let (handle, built, cx) = uniform_host(cx);
    frame(cx);
    handle.scroll_to_item(100, VirtualListScroll::Top);
    frame(cx);
    handle.splice(ROWS..ROWS, 50);
    frame(cx);
    assert_eq!(handle.item_count(), ROWS + 50);
    assert_eq!(handle.first_visible_item(), 100);
    handle.set_item_count(3);
    built.borrow_mut().clear();
    frame(cx);
    assert_eq!(handle.item_count(), 3);
    assert_eq!(handle.first_visible_item(), 0);
    assert_eq!(*built.borrow(), BTreeSet::from([0, 1, 2]));
    // A list that does not scroll is at both ends.
    assert!(handle.is_scrolled_to_top() && handle.is_scrolled_to_end());
}

#[gpui::test]
fn measured_center_centres_a_laid_out_row(cx: &mut TestAppContext) {
    let (handle, _, cx) = host(cx);
    frame(cx);
    // A row in the overdraw band just below the viewport.
    let below = (0..ROWS)
        .find(|&ix| {
            handle
                .bounds_for_item(ix)
                .is_some_and(|b| b.bottom() > px(VIEWPORT))
        })
        .unwrap();
    handle.scroll_to_item(below, VirtualListScroll::Center);
    frame(cx);
    let row = handle.bounds_for_item(below).unwrap();
    assert!(
        (f32::from(row.center().y) - VIEWPORT / 2.).abs() < 0.5,
        "{row:?}"
    );
}
