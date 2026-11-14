//! Deep behaviour tests for `Link` against the pinned HeroUI v3.2.4 Link and
//! React Aria 1.20's `useLink`:
//!
//! - activation semantics: `onPress` fires *and* `href` opens — a RAC press on
//!   an anchor does both — through real hit-tested pointer coordinates and
//!   focused Enter/Space activation, plus the `autoFocus` first-frame grab and
//!   the disabled element's absence from pointer and keyboard reach.
//! - the documented `render` function: the closure is handed the link's
//!   interactive state (`{isHovered, isPressed, isFocused, isFocusVisible,
//!   isDisabled}`). The hover and the keyboard press are real tracked state
//!   (`util::interaction` + `util::track_interaction`), reported one frame
//!   late where gpui tells a handler — so every assertion follows an event
//!   with an explicit `flush_frame`. A disabled link keeps reporting
//!   `isDisabled: true` with the tracked states pinned false.
//!
//! Geometry: the host wraps the link in a flex row so it hugs its label; a
//! link renders its label at the inherited 16px `font-medium`, so the label's
//! centre is `x = text_width / 2` and `y = 10` — the same measured-advance
//! approach `render_props.rs` uses, never a guessed width.

use crate::harness;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, point, prelude::*, px, Font, FontFeatures, FontStyle, FontWeight, KeyUpEvent, Keystroke,
    Modifiers, MouseButton, TestAppContext, VisualTestContext,
};
use harness::{click, events, open_host, press};
use herogpui_components::{InteractiveState, Link};

// ---------------------------------------------------------------------------
// Shared plumbing
// ---------------------------------------------------------------------------

/// Forces the frame that carries the state a handler just wrote. Events hit
/// test the last rendered frame, and the render closure can only read what the
/// previous frame stashed in its interaction slot, so the refresh is what
/// turns a handler's write into a value the closure recorded.
pub fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// The interactive state the `render` closure recorded on the last frame.
type Recorded = Rc<RefCell<InteractiveState>>;

fn recorded_state(record: &Recorded) -> InteractiveState {
    *record.borrow()
}

/// The advance width of `text` shaped the way the link shapes it: gpui's
/// default `.SystemUIFont` stack at 16px, `font-medium`.
fn label_width(window: &gpui::Window, text: &str) -> f32 {
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
    let line = window
        .text_system()
        .shape_line(text.to_owned().into(), px(16.), &[run], None);
    f32::from(line.width)
}

/// The centre of a lone hugging link labelled `text`, in host coordinates.
fn link_centre(window: &gpui::Window, text: &str) -> gpui::Point<gpui::Pixels> {
    point(px(label_width(window, text) / 2.), px(10.))
}

/// The centre of a link whose `render` closure draws the fixed 40x20 box the
/// state tests use. The closure *is* the link's content, so a sizeless element
/// would collapse the link to a zero-area target no event could reach —
/// the same fixed box `render_props.rs` hands its closures.
fn render_centre() -> gpui::Point<gpui::Pixels> {
    point(px(20.), px(10.))
}

fn render_box() -> gpui::AnyElement {
    div().w(px(40.)).h(px(20.)).into_any_element()
}

// ---------------------------------------------------------------------------
// Activation — pointer, keyboard, autoFocus, disabled
// ---------------------------------------------------------------------------

/// A pointer press on an `href` link with an `onPress` does both jobs: RAC's
/// press fires the callback and the anchor navigates.
#[gpui::test]
fn click_reports_press_and_opens_href(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        div()
            .flex()
            .flex_row()
            .child(
                Link::new("link")
                    .label("Open")
                    .href("#/open")
                    .on_press(move |_, _, _| recorded.borrow_mut().push("press".into())),
            )
            .into_any_element()
    });

    let centre = cx.update(|window, _| link_centre(window, "Open"));
    click(cx, centre.x.into(), centre.y.into());
    assert_eq!(
        recorded.borrow().as_slice(),
        ["press"],
        "the pointer press must report on_press"
    );
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("#/open"),
        "the same press must open the link's href"
    );
}

/// `href` with no `onPress` still navigates — the anchor's own job.
#[gpui::test]
fn href_alone_opens_its_url(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        div()
            .flex()
            .flex_row()
            .child(Link::new("link").label("Docs").href("#/docs"))
            .into_any_element()
    });

    let centre = cx.update(|window, _| link_centre(window, "Docs"));
    click(cx, centre.x.into(), centre.y.into());
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("#/docs"),
        "an href link must navigate without any callback configured"
    );
}

/// A focused link activates on Enter — gpui fires the click on key *up*, which
/// is why the harness `press` is used rather than bare keystrokes.
#[gpui::test]
fn enter_activates_a_focused_link(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        div()
            .flex()
            .flex_row()
            .child(
                Link::new("link")
                    .label("Open")
                    .href("#/open")
                    .on_press(move |_, _, _| recorded.borrow_mut().push("press".into())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["press"],
        "Enter on the focused link must fire on_press"
    );
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("#/open"),
        "Enter activation must also open the href"
    );
}

/// Space is the other activation key gpui binds to a focused click target.
#[gpui::test]
fn space_activates_a_focused_link(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        div()
            .flex()
            .flex_row()
            .child(
                Link::new("link")
                    .label("Open")
                    .href("#/open")
                    .on_press(move |_, _, _| recorded.borrow_mut().push("press".into())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["press"],
        "Space on the focused link must fire on_press"
    );
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("#/open"),
        "Space activation must also open the href"
    );
}

/// `autoFocus` seats the focus on the first frame, so the very first Enter
/// reaches the link with no Tab at all.
#[gpui::test]
fn auto_focus_reaches_the_link_before_any_tab(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        div()
            .flex()
            .flex_row()
            .child(
                Link::new("link")
                    .label("Open")
                    .href("#/open")
                    .auto_focus(true)
                    .on_press(move |_, _, _| recorded.borrow_mut().push("press".into())),
            )
            .into_any_element()
    });

    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["press"],
        "autoFocus must have seated the focus on the first frame"
    );
    assert_eq!(cx.opened_url().as_deref(), Some("#/open"));
}

/// A disabled link is inert on both channels: `status-disabled` removes the
/// pointer reach and drops the element from the tab order, so neither the
/// callback nor the href can be reached. (The enabled contrast for the tab
/// stop is `enter_activates_a_focused_link`.)
#[gpui::test]
fn disabled_link_ignores_pointer_and_keyboard(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        div()
            .flex()
            .flex_row()
            .child(
                Link::new("link")
                    .label("Open")
                    .href("#/open")
                    .is_disabled(true)
                    .on_press(move |_, _, _| recorded.borrow_mut().push("press".into())),
            )
            .into_any_element()
    });

    let centre = cx.update(|window, _| link_centre(window, "Open"));
    click(cx, centre.x.into(), centre.y.into());
    press(cx, "tab");
    press(cx, "enter");
    press(cx, "space");
    assert!(
        recorded.borrow().is_empty(),
        "a disabled link must not fire on_press from pointer or keys"
    );
    assert_eq!(
        cx.opened_url(),
        None,
        "a disabled link must never open its href"
    );
}

// ---------------------------------------------------------------------------
// The documented `render` function and its interactive state
// ---------------------------------------------------------------------------

/// The pointer states: the hover and the press are real tracked state, one
/// frame behind the handler, and a click focus is *not* focus-visible.
#[gpui::test]
fn render_closure_receives_pointer_states(cx: &mut TestAppContext) {
    let record: Recorded = Rc::new(RefCell::new(InteractiveState::default()));
    let for_view = record.clone();
    let cx = open_host(cx, move || {
        let record = for_view.clone();
        div()
            .flex()
            .flex_row()
            .child(
                Link::new("link")
                    .label("Open")
                    .render(move |state| {
                        *record.borrow_mut() = state;
                        render_box()
                    })
                    .into_any_element(),
            )
            .into_any_element()
    });

    flush_frame(cx);
    let first = recorded_state(&record);
    assert!(
        first == InteractiveState::default(),
        "before any input the closure must report all-false state, got {first:?}"
    );

    cx.update(|window, _| window.activate_window());
    let centre = render_centre();
    // gpui's hover listener compares against the hit-test of the last *paint*,
    // so the first move only moves the mouse; the repaint after it, then the
    // second move, is what flips the hover.
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    let hovered = recorded_state(&record);
    assert!(
        hovered.is_hovered && !hovered.is_pressed,
        "the frame after the move must hand is_hovered to the closure, got {hovered:?}"
    );

    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    let pressed = recorded_state(&record);
    assert!(
        pressed.is_pressed && pressed.is_hovered,
        "the frame after the down must hand is_pressed to the closure, got {pressed:?}"
    );

    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    let released = recorded_state(&record);
    assert!(
        !released.is_pressed,
        "the up must release the press, got {released:?}"
    );
    assert!(
        released.is_focused && !released.is_focus_visible,
        "a pointer-activated link holds the focus without a ring, got {released:?}"
    );
}

/// The keyboard states: Tab focus is focus-visible, Enter presses through the
/// same tracked slot, and the key-up releases it.
#[gpui::test]
fn render_closure_receives_keyboard_states(cx: &mut TestAppContext) {
    let record: Recorded = Rc::new(RefCell::new(InteractiveState::default()));
    let for_view = record.clone();
    let cx = open_host(cx, move || {
        let record = for_view.clone();
        div()
            .flex()
            .flex_row()
            .child(
                Link::new("link")
                    .label("Open")
                    .render(move |state| {
                        *record.borrow_mut() = state;
                        render_box()
                    })
                    .into_any_element(),
            )
            .into_any_element()
    });

    press(cx, "tab");
    flush_frame(cx);
    let focused = recorded_state(&record);
    assert!(
        focused.is_focused && focused.is_focus_visible,
        "keyboard focus must arrive with the ring flag, got {focused:?}"
    );

    cx.simulate_keystrokes("enter");
    flush_frame(cx);
    let down = recorded_state(&record);
    assert!(
        down.is_pressed,
        "Enter down must press the focused link for the closure, got {down:?}"
    );

    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    flush_frame(cx);
    let up = recorded_state(&record);
    assert!(
        !up.is_pressed,
        "Enter up must release the keyboard press, got {up:?}"
    );
}

/// A disabled link still renders through its closure — v3 hands render
/// functions `isDisabled` — but every tracked state stays false.
#[gpui::test]
fn render_closure_on_a_disabled_link_reports_is_disabled(cx: &mut TestAppContext) {
    let record: Recorded = Rc::new(RefCell::new(InteractiveState::default()));
    let for_view = record.clone();
    let cx = open_host(cx, move || {
        let record = for_view.clone();
        div()
            .flex()
            .flex_row()
            .child(
                Link::new("link")
                    .label("Open")
                    .is_disabled(true)
                    .render(move |state| {
                        *record.borrow_mut() = state;
                        render_box()
                    })
                    .into_any_element(),
            )
            .into_any_element()
    });

    let centre = render_centre();
    click(cx, centre.x.into(), centre.y.into());
    flush_frame(cx);
    let state = recorded_state(&record);
    assert!(
        state.is_disabled,
        "the closure must be told the link is disabled, got {state:?}"
    );
    assert!(
        !state.is_hovered && !state.is_pressed && !state.is_focused,
        "a disabled link must not track pointer or focus state, got {state:?}"
    );
}

// ---------------------------------------------------------------------------
// Pinned composition: a caller icon gets none of the default icon's spacing
// ---------------------------------------------------------------------------

fn need(cx: &mut VisualTestContext, name: &'static str) -> gpui::Bounds<gpui::Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} must paint"))
}

/// Upstream derives `data-default-icon` from `!children` (link.tsx), so the
/// pinned `.link__icon[data-default-icon="true"] { ms-1 pb-1.5 }` applies only
/// to the built-in arrow of a childless `<Link.Icon />` — a link this port
/// never draws. The Rust `icon(element)` is the arbitrary-children path, and
/// `.link` has no gap of its own, so a caller icon's slot starts flush against
/// the label on either side. The child is centered in the pinned 0.75em slot;
/// the default-arrow margin is not added to a caller-supplied icon.
#[gpui::test]
fn custom_icon_carries_none_of_the_default_icon_spacing(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(8.))
            .child(
                div().flex().debug_selector(|| "wrap-end".to_owned()).child(
                    Link::new("ln-end")
                        .label("Open")
                        .icon(
                            div()
                                .w(px(10.))
                                .h(px(10.))
                                .debug_selector(|| "icon-end".to_owned()),
                        )
                        .href("#/open"),
                ),
            )
            .child(
                div()
                    .flex()
                    .debug_selector(|| "wrap-start".to_owned())
                    .child(
                        Link::new("ln-start")
                            .label("Open")
                            .icon(
                                div()
                                    .w(px(10.))
                                    .h(px(10.))
                                    .debug_selector(|| "icon-start".to_owned()),
                            )
                            .icon_first(true)
                            .href("#/open"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .debug_selector(|| "wrap-bare".to_owned())
                    .child(Link::new("ln-bare").label("Open").href("#/open")),
            )
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    let shaped = cx.update(|window, _| label_width(window, "Open"));
    // The laid-out label width, measured off the iconless link itself:
    // shaping's fractional advance and the laid-out line differ by subpixel
    // rounding, and the spacing assertions below are exact.
    let bare = need(cx, "wrap-bare");
    assert!(
        (f32::from(bare.size.width) - shaped).abs() < 1.,
        "an iconless link hugs its label ({shaped}px shaped), got {}px",
        f32::from(bare.size.width)
    );
    let label = bare.size.width;

    let (end_wrap, end_icon) = (need(cx, "wrap-end"), need(cx, "icon-end"));
    assert_eq!(
        end_icon.origin.x - end_wrap.origin.x,
        label + px(1.),
        "the trailing custom icon must be centered in its 12px slot"
    );
    assert_eq!(
        end_wrap.size.width,
        label + px(12.),
        "the root hugs label + the pinned 0.75em custom-icon slot"
    );

    let (start_wrap, start_icon) = (need(cx, "wrap-start"), need(cx, "icon-start"));
    assert_eq!(
        start_icon.origin.x - start_wrap.origin.x,
        px(1.),
        "the leading custom icon must be centered in its 12px slot"
    );
    assert_eq!(
        start_wrap.size.width,
        px(12.) + label,
        "the root hugs the pinned custom-icon slot + label"
    );
}
