//! Behaviour tests for the per-part disabling added to Pagination:
//! `Pagination.Link.isDisabled`, `Pagination.Previous.isDisabled` and
//! `Pagination.Next.isDisabled`.
//!
//! `tests/nav_deep.rs` pins the whole-bar and bound-based disabling (the
//! arrows at page 1 / the last page, the live active cell, inert ellipses and
//! the Tab walk). This suite drives the *individual* knobs: v3's part tables
//! document `isDisabled` on the Link and on the Previous/Next buttons, and a
//! monolithic port projects all three onto one builder,
//! `Pagination::disabled_keys`, keyed by the page each control navigates to:
//! a page number in `1..=total` disables that link, `0` names Previous and
//! `total + 1` names Next. Every assertion is behavioural (recorded
//! callbacks, or a probe click that must record nothing), never appearance.
//!
//! ---------------------------------------------------------------------------
//! The v3 contract, quoted from https://heroui.com/react/llms-full.txt
//! (the Pagination page's `## API Reference`, September 2026 snapshot):
//!
//! ### Pagination.Link
//!
//! | Prop         | Type                      | Default | Description                      |
//! | `isActive`   | `boolean`                 | `false` | Whether this is the current page |
//! | `isDisabled` | `boolean`                 | `false` | Whether the link is disabled     |
//! | `onPress`    | `(e: PressEvent) => void` | -       | Press handler (from React Aria)  |
//!
//! ### Pagination.Previous / Pagination.Next
//!
//! | Prop         | Type                      | Default | Description                                         |
//! | `isDisabled` | `boolean`                 | `false` | Whether the button is disabled                      |
//! | `onPress`    | `(e: PressEvent) => void` | -       | Press handler (from React Aria)                     |
//!
//! The same page's Accessibility section adds the behavioural contract this
//! suite checks: "Disabled states properly communicated to assistive
//! technology via `isDisabled`", with press handlers "from React Aria"
//! normalising pointer *and* keyboard presses — so a disabled link or button
//! must fire no press at all. AGENTS.md states the tab-order half: a
//! disabled control must leave the tab order (`track_focus` is what puts it
//! in), and it was a real defect in this very file for the arrows earlier —
//! `nav_deep.rs` now guards those; the tests below extend the rule to a
//! per-link disable.
//!
//! ---------------------------------------------------------------------------
//! Geometry, reusing `tests/nav_deep.rs`'s derivation (which comments the
//! arithmetic): `size-md` cells are 32px squares at y 0..32 (centre y 16); a
//! nav button is `px-2.5` (10px each side) around a 14px glyph = 34px; the
//! row gaps items by `gap-1` (4px). Prev spans x 0..34 (centre 17), page
//! cell *k* spans 38+36k .. 70+36k (centre 54+36k), and the next button
//! starts at 38+36·(cell count) — 218 for five cells (centre 235). Five
//! cells are rendered because `total <= 2*siblings + 5`, and every fixture
//! below uses total 5 to keep the whole window visible.
//!
//! One harness fact shapes the ordering of every test: gpui moves the focus
//! to the element a click lands on, so a pointer probe *before* a keyboard
//! walk makes the walk start from the probed control. The keyboard walk
//! therefore always comes first, and the pointer probes after it.
//!
//! Each instance gets its own element id; two components sharing an id share
//! their keyed state, which AGENTS.md documents as a silent failure. The
//! `press` helper releases the last key because gpui activates a focused
//! element's click listeners on key **up**.

use crate::harness;
use crate::source_scan;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{prelude::*, TestAppContext};
use herogpui_components::Pagination;

use harness::{click, events, open_host, press};

/// Remaining source-text check: `aria-current` reaches only the AccessKit
/// tree, which the headless platform never builds (`a11y_deep.rs`).
#[test]
fn active_page_publishes_accesskit_current_page_state() {
    let source = source_scan::component_src("pagination.rs");
    assert!(source.contains("a11y_current(a11y::AriaCurrent::Page)"));
    assert!(source.contains("when(active"));
}

#[gpui::test]
fn link_render_prop_receives_active_page_and_keeps_it_pressable(cx: &mut TestAppContext) {
    let states = Rc::new(RefCell::new(HashMap::<usize, bool>::new()));
    let for_view = states.clone();
    let recorded = events();
    let for_press = recorded.clone();
    let page = Rc::new(Cell::new(2usize));
    let page_for_view = page;
    let cx = open_host(cx, move || {
        let states = for_view.clone();
        let recorded = for_press.clone();
        let page = page_for_view.clone();
        Pagination::new("parts-pg-render", page.get(), 5)
            .link(move |page, is_active| {
                states.borrow_mut().insert(page, is_active);
                gpui::div().child(page.to_string()).into_any_element()
            })
            .on_change(move |next, window, _| {
                page.set(*next);
                recorded.borrow_mut().push(next.to_string());
                window.refresh();
            })
            .into_any_element()
    });

    assert_eq!(
        *states.borrow(),
        HashMap::from([(1, false), (2, true), (3, false), (4, false), (5, false)]),
        "every visible link must receive its page number and only the current page is active"
    );

    // Medium page 2 is the second 32px cell after the 34px previous button:
    // prev 0..34, gap 4, page 1 38..70, gap 4, page 2 74..106.
    click(cx, 90., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2"],
        "replacing an active link's child must not disable its press handler"
    );

    click(cx, 235., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2", "3"],
        "the next button must report page 3"
    );
    assert_eq!(
        *states.borrow(),
        HashMap::from([(1, false), (2, false), (3, true), (4, false), (5, false)]),
        "after the owner accepts navigation, the render prop must move isActive to page 3"
    );
}

/// With a large `total`, the bar still shows the windowed cells and those
/// cells still answer presses. Bounded handle allocation is proven by the
/// `visible_pages` unit tests in `pagination.rs`; this test pins geometry.
#[gpui::test]
fn large_total_window_cells_stay_pressable(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Pagination::new("perf-pg-window", 1, 2000)
            .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // Window for page 1 of 2000: 1, 2, ellipsis, 2000 at centres 54, 90, 126,
    // 162 on y 16 — the same 32px cells and 4px gaps as the five-page bar.
    click(cx, 90., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2"],
        "a shown cell of a large collection must still report its page"
    );

    click(cx, 162., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2", "2000"],
        "the trailing page past the ellipsis must answer its press"
    );
}

#[gpui::test]
fn custom_navigation_icons_own_the_previous_and_next_slots(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Pagination::new("parts-pg-icons", 3, 5)
            .previous_icon(gpui::div().w(gpui::px(40.)).h(gpui::px(14.)).child("P"))
            .next_icon(gpui::div().w(gpui::px(40.)).h(gpui::px(14.)).child("N"))
            .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // A 40px icon plus 10px padding on each side makes each nav button 60px.
    // x=50 is inside the custom Previous slot but outside the default 34px
    // button. Five 32px cells and six 4px gaps put custom Next at 244..304,
    // so x=294 likewise exists only when the custom slot is actually drawn.
    click(cx, 50., 16.);
    click(cx, 294., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2", "4"],
        "the custom icon geometry must participate in both nav buttons without changing presses"
    );
}

/// `Pagination.Link.isDisabled`: a link whose page number is in
/// `disabled_keys` answers neither a click nor Enter, holds no tab stop, and
/// leaves its neighbours alone. Fixture: page 2 of 5 with page 4 disabled —
/// prev, all five cells (1 2 3 4 5) and next render; cell 2 is the active
/// page and cell 4 (centre x 162) is the disabled one. The Tab walk must go
/// prev -> 1 -> 2 (active and live) -> 3 -> 5 -> next — the Enter
/// after cell 3 landing on cell 5 is what proves cell 4 is not a stop — and
/// a click at cell 4's seat must record nothing while a click on either
/// adjacent cell still reports its page.
#[gpui::test]
fn disabled_link_answers_nothing_is_skipped_and_neighbours_report(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Pagination::new("parts-pg-link", 2, 5)
            .disabled_keys(HashSet::from([4]))
            .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // Five cells at centres 54+36k: 54, 90, 126, 162 (= the disabled page 4),
    // 198; prev at 17, next at 235, all on y 16.
    press(cx, "tab");
    press(cx, "enter"); // prev -> report page 1
    press(cx, "tab");
    press(cx, "enter"); // cell 1 -> report page 1
    press(cx, "tab");
    press(cx, "enter"); // cell 2 (active and `aria-current`) -> report page 2
    press(cx, "tab");
    press(cx, "enter"); // cell 3 -> report page 3
    press(cx, "tab");
    press(cx, "enter"); // cell 5: the disabled cell 4 must hold no tab stop
    press(cx, "tab");
    press(cx, "enter"); // next -> report page 3 (2 + 1)
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1", "1", "2", "3", "5", "3"],
        "Tab must walk prev, every enabled cell and next, skipping the \
         disabled cell 4 — got {:?}",
        recorded.borrow()
    );

    // The disabled seat answers no pointer press, and its neighbours still do.
    click(cx, 126., 16.); // cell 3, adjacent on the left
    click(cx, 198., 16.); // cell 5, adjacent on the right
    click(cx, 162., 16.); // the disabled cell 4
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1", "1", "2", "3", "5", "3", "3", "5"],
        "a click on the disabled cell must record nothing while the adjacent \
         cells 3 and 5 still report their own pages — got {:?}",
        recorded.borrow()
    );
}

/// `Pagination.Previous.isDisabled`: the key `0` force-disables Previous on
/// a page where the bounds would allow it (page 3 of 5). The arrow must
/// answer no click and hold no tab stop — the first stop is cell 1, so the
/// first Enter reports page 1, where a live prev would have been the first
/// stop and reported page 2. The Next arrow and the cells are untouched,
/// which is what proves the gate is per-arrow rather than wholesale.
#[gpui::test]
fn disabled_previous_answers_nothing_and_leaves_tab_order(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Pagination::new("parts-pg-prev", 3, 5)
            .disabled_keys(HashSet::from([0]))
            .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // Page 3 of 5: prev (x 0..34, centre 17) would normally report page 2.
    press(cx, "tab");
    press(cx, "enter"); // cell 1: prev holds no stop, or the Enter was page 2
    press(cx, "tab");
    press(cx, "enter"); // cell 2 -> report page 2
    press(cx, "tab");
    press(cx, "enter"); // cell 3 (the active page) -> report page 3
    press(cx, "tab");
    press(cx, "enter"); // cell 4 -> report page 4
    press(cx, "tab");
    press(cx, "enter"); // cell 5 -> report page 5
    press(cx, "tab");
    press(cx, "enter"); // next -> report page 4 (3 + 1)
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1", "2", "3", "4", "5", "4"],
        "the force-disabled prev must hold no tab stop (the first Enter \
         reports cell 1, not page 2) while the cells and Next still report — \
         got {:?}",
        recorded.borrow()
    );

    // Its own seat answers no pointer press; the cells and Next still do.
    click(cx, 17., 16.);
    click(cx, 162., 16.); // cell 4
    click(cx, 235., 16.); // next
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1", "2", "3", "4", "5", "4", "4", "4"],
        "a click on the force-disabled prev must record nothing while cell 4 \
         and Next still report — got {:?}",
        recorded.borrow()
    );
}

/// `Pagination.Next.isDisabled`: the key `total + 1` (6 for a total of 5)
/// force-disables Next on a page where the bounds would allow it. The arrow
/// answers no click, and after cell 5 the Tab walk wraps to prev — an enter
/// there reports page 2, where a live Next would have been the next stop and
/// reported page 4. Previous and the cells are untouched.
#[gpui::test]
fn disabled_next_answers_nothing_and_leaves_tab_order(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Pagination::new("parts-pg-next", 3, 5)
            .disabled_keys(HashSet::from([6]))
            .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // Page 3 of 5: next (starts at 38+36*5 = 218, centre 235) would normally
    // report page 4.
    press(cx, "tab");
    press(cx, "enter"); // prev -> report page 2
    press(cx, "tab");
    press(cx, "enter"); // cell 1 -> report page 1
    press(cx, "tab");
    press(cx, "enter"); // cell 2 -> report page 2
    press(cx, "tab");
    press(cx, "enter"); // cell 3 (the active page) -> report page 3
    press(cx, "tab");
    press(cx, "enter"); // cell 4 -> report page 4
    press(cx, "tab");
    press(cx, "enter"); // cell 5 -> report page 5
    press(cx, "tab");
    press(cx, "enter"); // next is no stop: the walk wraps to prev -> page 2
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2", "1", "2", "3", "4", "5", "2"],
        "the force-disabled next must hold no tab stop (after cell 5 the walk \
         wraps to prev instead of reporting page 4), while prev and the \
         enabled cells still report — got {:?}",
        recorded.borrow()
    );

    // Its own seat answers no pointer press; prev still does.
    click(cx, 235., 16.);
    click(cx, 17., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2", "1", "2", "3", "4", "5", "2", "2"],
        "a click on the force-disabled next must record nothing while prev \
         still reports page 2 — got {:?}",
        recorded.borrow()
    );
}

/// The per-link gate must not regress the whole-bar disable: under
/// `is_disabled(true)` *no* cell answers a press (the click listener is
/// attached only for a link that is not disabled) and the bar contributes no
/// tab stop at all, so Tab stays put and Enter activates nothing.
#[gpui::test]
fn whole_bar_disabled_cells_answer_nothing_and_bar_holds_no_stop(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Pagination::new("parts-pg-bar", 1, 5)
            .is_disabled(true)
            .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // No stop anywhere in the bar: Tab from the root moves nowhere and Enter
    // has nothing focused that owns a click listener.
    press(cx, "tab");
    press(cx, "enter");
    // Cell 2 (centre 90) is a non-active cell whose click the port used to
    // attach even under `is_disabled`; the active cell 1 (centre 54), prev
    // (17) and next (235) complete the sweep.
    click(cx, 90., 16.);
    click(cx, 54., 16.);
    click(cx, 17., 16.);
    click(cx, 235., 16.);
    assert!(
        recorded.borrow().is_empty(),
        "a wholly disabled pagination must answer no press and hold no tab \
         stop — got {:?}",
        recorded.borrow()
    );
}
