//! A pointer crossing must repaint the view for every control whose hover
//! feedback is an interpolated ramp.
//!
//! `anim::hover_fade` and `anim::field_chrome_ramp` keep the pointer state in
//! keyed state and record it from an `on_hover` listener that deliberately does
//! not call `cx.notify()` — notifying there turns gpui's paint-time listener
//! reconciliation into an unbounded re-render whenever the element's id is not
//! stable across frames. What repaints instead is gpui's own capture-phase
//! `MouseMoveEvent` handler, and that handler is installed *and notifies* only
//! when the element carries a hover style, so both helpers set one themselves.
//!
//! These tests are the guard on that substitution. They move the pointer and
//! never call `window.refresh()`, so the redraw they observe can only have come
//! from the notify path; with the hover style removed from either helper, every
//! case here fails. The controls are chosen to cover both listeners and both
//! shapes of the fade's hover style: `Input` carries the immediate border
//! endpoint, `Pagination`'s buttons carry none, and `NumberField`'s group goes
//! through `field_chrome_ramp`'s empty refinement.

use crate::harness;

use std::cell::Cell;
use std::rc::Rc;

use gpui::{point, prelude::*, px, AnyElement, Modifiers, TestAppContext};
use herogpui_components::{Input, InputState, NumberField, NumberState, Pagination};

use harness::open_host;

/// A point inside the control the host draws at the window origin, and one far
/// outside it. `open_host` already parks the pointer outside the window.
const INSIDE: (f32, f32) = (12., 12.);

/// Renders `content` under a render counter and returns the count and context.
///
/// The host calls the content closure once per render, so the counter is a
/// direct read of "did this view redraw", which is what a notify buys.
fn counted(
    cx: &mut TestAppContext,
    content: impl Fn() -> AnyElement + 'static,
) -> (Rc<Cell<usize>>, &mut gpui::VisualTestContext) {
    let renders = Rc::new(Cell::new(0usize));
    let counter = renders.clone();
    let cx = open_host(cx, move || {
        counter.set(counter.get() + 1);
        content()
    });
    (renders, cx)
}

/// Moves the pointer onto the control and reports whether the view redrew.
///
/// No `window.refresh()`: a redraw here is the notify, not the harness.
fn redraws_on_crossing(cx: &mut gpui::VisualTestContext, renders: &Rc<Cell<usize>>, label: &str) {
    // Settle whatever the first frames scheduled, so the count below moves only
    // for the crossing itself.
    cx.update(|window, _| window.refresh());
    let before = renders.get();
    cx.simulate_mouse_move(point(px(INSIDE.0), px(INSIDE.1)), None, Modifiers::none());
    let after = renders.get();
    assert!(
        after > before,
        "{label}: moving the pointer onto the control must repaint the view \
         ({before} renders before the crossing, {after} after). The hover ramp \
         records without notifying, so the redraw has to come from the gpui \
         hover style the helper installs."
    );
}

#[gpui::test]
fn input_repaints_when_the_pointer_crosses_it(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let (renders, cx) = counted(cx, move || Input::new(state.clone()).into_any_element());
    redraws_on_crossing(cx, &renders, "Input");
}

#[gpui::test]
fn number_field_group_repaints_when_the_pointer_crosses_it(cx: &mut TestAppContext) {
    let state = cx.new(|cx| NumberState::new(cx, 42.));
    let (renders, cx) = counted(cx, move || {
        NumberField::new(state.clone()).into_any_element()
    });
    redraws_on_crossing(cx, &renders, "NumberField");
}

#[gpui::test]
fn pagination_button_repaints_when_the_pointer_crosses_it(cx: &mut TestAppContext) {
    // Page 3 of 5, so the leading "previous" arrow the pointer lands on is
    // enabled: `nav_button` installs the fade only for an enabled arrow.
    let (renders, cx) = counted(cx, || {
        Pagination::new("hover-repaint-pager", 3, 5).into_any_element()
    });
    redraws_on_crossing(cx, &renders, "Pagination");
}
