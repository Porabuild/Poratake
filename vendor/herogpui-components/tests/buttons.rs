//! Behaviour tests for the pressable components: Button, ButtonGroup,
//! CloseButton, ToggleButton, ToggleButtonGroup, Link, Chip and Alert.
//!
//! Everything static about them is measured by the `.shots/*.py` audits; these
//! tests drive the controls and assert on recorded callbacks and behavioural
//! probes — plus, for the press and hover contracts, on the fills and boxes
//! the painted scene shows while a control is hovered or held.
//!
//! Geometry is derived from the components' own constants, not guessed:
//!
//! - The test window is 1920x1080 (the overlays suite pins that: a modal is
//!   centred at y 498 = (1080-84)/2 and a drawer spans x 1600..1920).
//! - A `Button` is `h-[Size::control_height()]` = 36px at `Size::Md`, so a
//!   row starting at the window origin is 36px tall and its centre line is
//!   y = 18. With `full_width(true)` the button spans the whole window, so the
//!   centre column is exactly x = 960.
//! - CloseButton is a square `(box_size, icon_size) = (24, 16)` (`close_button.rs`),
//!   so its centre at the origin is (12, 12); the icon's 4px padding is
//!   irrelevant to the hit box.
//! - ToggleButton `Size::Md` is `h-9` (36px) with `px-4`
//!   (16px) around a label whose advance width is *measured* with the same
//!   text system the renderer shapes with, so its centre x is 16 + w/2.
//! - A full-width ButtonGroup gives a stretch slot (`flex_1`) to each member
//!   that *resolves* to full width — the pinned `fullWidth ??
//!   context.fullWidth` merge — so three inheriting members span 640px each
//!   and member *i*'s centre column is 320 + 640i; a member with an explicit
//!   `full_width(false)` hugs content instead, and the seams' absolutely-
//!   positioned separators take no layout space.
//! - Link sizes to its content: the click x is the measured label width over
//!   two, and y = 10 sits inside the ~19px line box whatever the machine's
//!   font metrics come to (the line is `line_height(14 * 1.3)` plus a 1px
//!   bottom padding, per the same derivation Breadcrumbs uses).
//! - An Alert is `w_full px-4 py-3` and takes end content only as composed
//!   children (v3 removed `isClosable`): a CloseButton child hugs the
//!   content's right edge (1920-16-24..1920-16) and starts at the top padding
//!   (12px), so its centre is (1920 - 16 - 12, 12 + 12) = (1892, 24).
//! - Chip is `px-2 py-0.5` around its composed children (one 20px `text-xs`
//!   line: the base `leading-5` sets `--tw-leading`, which the re-applied
//!   `text-*` sizes consume), so a CloseButton composed as the leading child
//!   starts at x = 8 (the leading padding) and the 24px close box plus the
//!   2px vertical padding make the chip 28px tall: close centre (20, 14).
//!
//! The one-frame hover/press lag is handled explicitly in
//! `button_content_render_prop_sees_press`: `track_interaction` hears about a
//! hover or press in a *handler* and stashes it in the keyed `Interaction`
//! slot, so only the render *after* the event can read it. The test forces
//! that frame (`window.refresh()`) after each event and reads the closure's
//! latest snapshot — never the one the event dispatched against.
//!
//! That test used to be `#[ignore]`d as a defect: the button panicked before a
//! single frame drew, because `anim::hover_fade` (the `transition-colors`
//! wrapper) and `util::track_interaction` (attached only when a `content`
//! closure is set) both called `.on_hover` on the button root, and gpui
//! refuses a second one ("calling on_hover more than once on the same element
//! is not supported"). `hover_fade` now reads the interaction slot the
//! `track_interaction` handler keeps when one exists, leaving that handler the
//! element's single hover listener — see `anim.rs`. The assertions below are
//! the contract the render-prop API keeps, and
//! `toggle_button_content_render_prop_sees_state` runs the same cycle on a
//! second component to prove the fix is not button-specific.

use crate::harness;
use crate::source_scan;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    point, prelude::*, px, Context, Font, FontFeatures, FontStyle, FontWeight, KeyDownEvent,
    KeyUpEvent, Keystroke, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext,
};
use herogpui_components::{
    extend, Alert, Button, ButtonGroup, Chip, ChipLabel, CloseButton, InteractiveState, Link,
    Orientation, SelectionMode, ToggleButton, ToggleButtonGroup,
};

use harness::{click, events, open_host, press};

#[gpui::test]
fn app_root_tab_moves_once_and_consumes_the_browser_default(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let first = changes.clone();
        let second = changes.clone();
        gpui::div()
            .flex()
            .child(
                Button::new("first")
                    .label("First")
                    .on_press(move |_, _, _| first.borrow_mut().push("first".into())),
            )
            .child(
                Button::new("second")
                    .label("Second")
                    .on_press(move |_, _, _| second.borrow_mut().push("second".into())),
            )
            .into_any_element()
    });
    for key in ["tab", "tab", "shift-tab"] {
        let result = cx.update(|window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: Keystroke::parse(key).unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                }),
                cx,
            )
        });
        assert!(
            !result.propagate,
            "{key} must not move the browser's DOM focus"
        );
        cx.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        });
        cx.update(|window, _| window.refresh());
        press(cx, "enter");
    }
    assert_eq!(recorded.borrow().as_slice(), ["first", "second", "first"]);
}

/// The advance width of `text` shaped the way the components shape it: gpui's
/// default `.SystemUIFont` stack at `size` px and `weight`.
///
/// ToggleButton, Button and Link labels are 14px MEDIUM. All are laid out by
/// the window's own `WindowTextSystem`,
/// so this measurement is the render's measurement (the same helper the
/// collections suite uses for Tabs and Breadcrumbs).
fn text_width(system: &gpui::WindowTextSystem, text: &str, size: f32, weight: FontWeight) -> f32 {
    let run = gpui::TextRun {
        len: text.len(),
        font: Font {
            family: ".SystemUIFont".into(),
            features: FontFeatures::default(),
            weight,
            style: FontStyle::default(),
            fallbacks: None,
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = system.shape_line(text.to_owned().into(), px(size), &[run], None);
    f32::from(line.width)
}

/// Forces the frame that carries the state a handler just changed.
///
/// Every event below ends with the window dirty but not necessarily painted,
/// and events hit-test the *last rendered frame*. A explicit refresh makes the
/// frame under test the one the next event will dispatch against, which is
/// also how the hover/press one-frame lag is turned into a determinism.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// How many animation-frame callbacks are pending: one per live animation.
/// The press ramp registers one while it runs and stops when it settles, the
/// way `checkbox_motion.rs` observes motion.
fn pending_frames(cx: &mut VisualTestContext) -> usize {
    cx.update(|window, cx| window.simulate_next_frame(cx))
}

/// Drains frames until every in-flight animation has settled, failing if one
/// never does (a re-arming animation would keep registering callbacks).
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..100 {
        if pending_frames(cx) == 0 {
            return;
        }
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(10));
    }
    panic!("button animations never settled");
}

// ---------------------------------------------------------------------------
// Button
// ---------------------------------------------------------------------------

#[gpui::test]
fn button_press_reports_once(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let presses = presses.clone();
        Button::new("btn-press")
            .label("Go")
            .full_width(true)
            .on_press(move |_, _, _| presses.borrow_mut().push("press".into()))
            .into_any_element()
    });

    // The button is at the origin, `Size::Md` tall (36px), `full_width` across
    // the 1920px window: centre (960, 18).
    click(cx, 960., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["press"],
        "a click must report exactly one press"
    );

    // gpui moves the focus to a clicked element that tracks one, so the button
    // now holds it. Enter and Space activate it on key *up* by firing the very
    // same `on_click` listener (`ClickEvent::Keyboard`); a component that also
    // bound its own Enter handler in addition to `on_click` would fire twice —
    // the double-fire class this guards.
    flush_frame(cx);
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["press", "press"],
        "Enter must report exactly one more press"
    );
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["press", "press", "press"],
        "Space must report exactly one more press"
    );
}

#[gpui::test]
fn button_disabled_skips_focus_while_pending_retains_it_without_pressing(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let disabled = presses.clone();
        let pending = presses.clone();
        let probe = presses.clone();
        // Three full-width buttons stacked with 4px gaps: disabled at
        // y 0..36 (centre 18), pending at y 40..76 (centre 58), the probe at
        // y 80..116 (centre 98).
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                Button::new("btn-disabled")
                    .label("Disabled")
                    .full_width(true)
                    .is_disabled(true)
                    .on_press(move |_, _, _| disabled.borrow_mut().push("disabled".into())),
            )
            .child(
                Button::new("btn-pending")
                    .label("Pending")
                    .full_width(true)
                    .is_pending(true)
                    .on_press(move |_, _, _| pending.borrow_mut().push("pending".into())),
            )
            .child(
                Button::new("btn-probe")
                    .label("Probe")
                    .full_width(true)
                    .on_press(move |_, _, _| probe.borrow_mut().push("probe".into())),
            )
            .into_any_element()
    });

    // From the unfocused root, Tab must skip disabled and stop on pending.
    press(cx, "tab");
    flush_frame(cx);
    press(cx, "space");
    assert!(
        recorded.borrow().is_empty(),
        "a focused pending button must not activate"
    );

    // The next Tab moves from pending to the live probe.
    press(cx, "tab");
    flush_frame(cx);
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["probe"],
        "Tab after the focusable pending button must reach the probe"
    );

    // Pointer activation is suppressed for both inert states as well.
    click(cx, 960., 18.);
    click(cx, 960., 58.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["probe"],
        "disabled and pending buttons must not record pointer presses"
    );
}

// ---------------------------------------------------------------------------
// ButtonGroup
// ---------------------------------------------------------------------------

#[gpui::test]
fn button_group_reports_each_child(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let one = presses.clone();
        let two = presses.clone();
        let three = presses.clone();
        // A full-width group of three: each member's slot is flex_1, so
        // member *i* spans x 640i..640(i+1) and its centre column is
        // 320 + 640i, all at y = 18.
        ButtonGroup::new()
            .full_width(true)
            .button(
                Button::new("bg-one")
                    .label("One")
                    .on_press(move |_, _, _| one.borrow_mut().push("one".into())),
            )
            .button(
                Button::new("bg-two")
                    .label("Two")
                    .on_press(move |_, _, _| two.borrow_mut().push("two".into())),
            )
            .button(
                Button::new("bg-three")
                    .label("Three")
                    .on_press(move |_, _, _| three.borrow_mut().push("three".into())),
            )
            .into_any_element()
    });

    click(cx, 320., 18.);
    click(cx, 960., 18.);
    click(cx, 1600., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["one", "two", "three"],
        "each member of the group must report its own press, exactly once"
    );
}

/// Pinned v3 passes group props through context as defaults: a direct child
/// inherits `isDisabled` when unset, but an explicit `isDisabled={false}` wins.
#[gpui::test]
fn button_group_child_can_override_inherited_disabled(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let inherited = presses.clone();
        let override_enabled = presses.clone();
        ButtonGroup::new()
            .full_width(true)
            .is_disabled(true)
            .button(
                Button::new("bg-inherited-disabled")
                    .label("Inherited")
                    .on_press(move |_, _, _| inherited.borrow_mut().push("inherited".into())),
            )
            .button(
                Button::new("bg-override-enabled")
                    .label("Override")
                    .is_disabled(false)
                    .on_press(move |_, _, _| {
                        override_enabled.borrow_mut().push("override".into());
                    }),
            )
            .into_any_element()
    });

    click(cx, 480., 18.);
    click(cx, 1440., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["override"],
        "an explicit child value must override the ButtonGroup default"
    );
}

/// Pinned button.tsx resolves a direct member's width as
/// `finalFullWidth = fullWidth ?? context.fullWidth`, so an explicit
/// `fullWidth={false}` must free its share of a full-width group's row: the
/// two inheriting members split the leftover and the hugging one keeps its
/// content width. The port used to hand every member an equal `flex_1` slot
/// whenever the group was full-width, which erased the child override.
#[gpui::test]
fn button_group_full_width_context_preserves_explicit_child_false(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let one = presses.clone();
        let two = presses.clone();
        let three = presses.clone();
        ButtonGroup::new()
            .full_width(true)
            .button(
                Button::new("bgf-inherit-one")
                    .label("One")
                    .on_press(move |_, _, _| one.borrow_mut().push("one".into())),
            )
            .button(
                Button::new("bgf-explicit-false")
                    .label("Two")
                    // Explicit override: the group's fullWidth context must
                    // not stretch this member.
                    .full_width(false)
                    .on_press(move |_, _, _| two.borrow_mut().push("two".into())),
            )
            .button(
                Button::new("bgf-inherit-three")
                    .label("Three")
                    .on_press(move |_, _, _| three.borrow_mut().push("three".into())),
            )
            .into_any_element()
    });

    // "Two" hugs its content: 16px padding each side of the measured 14px
    // MEDIUM label. The group fills the 1920px window and the two inheriting
    // members split the leftover, so One spans 0..side, Two
    // side..side+w_two and Three side+w_two..1920, all at y = 18.
    let w_two = cx
        .update(|window, _| text_width(window.text_system(), "Two", 14.0, FontWeight::MEDIUM))
        + 32.;
    let side = (1920. - w_two) / 2.;

    click(cx, side / 2., 18.);
    click(cx, side - 50., 18.);
    click(cx, side + w_two / 2., 18.);
    click(cx, side + w_two + 50., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["one", "one", "two", "three"],
        "an explicit fullWidth=false member must hug content while the \
         inheriting members share the leftover width"
    );
}

/// The override works in the other direction too: `fullWidth={true}` on one
/// member of a group that is not full-width must stretch that member across
/// the row, with the hugging members keeping their content width at the
/// edges. The old slot logic only stretched when the *group* was full-width,
/// so this member stayed content-sized in the centre.
#[gpui::test]
fn button_group_full_width_child_true_stretches_without_group_full_width(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let one = presses.clone();
        let two = presses.clone();
        let three = presses.clone();
        ButtonGroup::new()
            .button(
                Button::new("bgf-hug-one")
                    .label("One")
                    .on_press(move |_, _, _| one.borrow_mut().push("one".into())),
            )
            .button(
                Button::new("bgf-explicit-true")
                    .label("Two")
                    .full_width(true)
                    .on_press(move |_, _, _| two.borrow_mut().push("two".into())),
            )
            .button(
                Button::new("bgf-hug-three")
                    .label("Three")
                    .on_press(move |_, _, _| three.borrow_mut().push("three".into())),
            )
            .into_any_element()
    });

    // One hugs 0..w_one and Three hugs 1920-w_three..1920; the explicit
    // member's stretch slot spans everything between them, at y = 18.
    let w_one = cx
        .update(|window, _| text_width(window.text_system(), "One", 14.0, FontWeight::MEDIUM))
        + 32.;
    let w_three = cx
        .update(|window, _| text_width(window.text_system(), "Three", 14.0, FontWeight::MEDIUM))
        + 32.;

    click(cx, w_one + 60., 18.);
    click(cx, (w_one + 1920. - w_three) / 2., 18.);
    click(cx, 1920. - w_three - 60., 18.);
    click(cx, 1920. - w_three / 2., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["two", "two", "two", "three"],
        "an explicit fullWidth=true member must stretch while its hugging \
         neighbours keep their content width"
    );
}

/// A vertical group is `flex-col`, and v3's `items-center` would leave every
/// member at its own content width — mixed label widths stack into a
/// staircase instead of one joined control. The column stretches the cross
/// axis, so every slot shares the width of the widest member and one left
/// edge, while each member keeps its own 36px (Md) row height.
#[gpui::test]
fn button_group_vertical_members_share_one_column(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        ButtonGroup::new()
            .orientation(Orientation::Vertical)
            .separators(true)
            .button(Button::new("bgvc-top").label("Top"))
            .button(Button::new("bgvc-wide").label("A much wider middle"))
            .button(Button::new("bgvc-bot").label("Bot"))
            .into_any_element()
    });
    flush_frame(cx);

    // `debug_bounds` keys on `&'static str`; the slots are indexed, so the
    // probe names are built at run time and leaked.
    let slot = |cx: &mut VisualTestContext, i: usize| {
        cx.debug_bounds(Box::leak(format!("button-group-slot-{i}").into_boxed_str()))
            .expect("every member paints")
    };
    let slots: Vec<_> = (0..3).map(|i| slot(cx, i)).collect();
    let column_width = slots[0].size.width;
    for (i, bounds) in slots.iter().enumerate() {
        assert_eq!(
            bounds.size.width, column_width,
            "member {i} must share the column width, bounds {bounds:?}"
        );
        assert_eq!(
            bounds.origin.x, slots[0].origin.x,
            "member {i} must share the column's left edge, bounds {bounds:?}"
        );
        assert_eq!(
            bounds.size.height,
            px(36.),
            "member {i} keeps the Md control height, bounds {bounds:?}"
        );
    }
    // The stretch is real: the narrow labels span past their content, up to
    // the widest member's own content width (the probe reads the slot; the
    // member fills it).
    let widest = slot(cx, 1).size.width;
    let narrow = slot(cx, 0).size.width;
    let w_top = px(cx
        .update(|window, _| text_width(window.text_system(), "Top", 14.0, FontWeight::MEDIUM))
        + 32.);
    assert!(
        narrow > w_top,
        "the narrow member stretches past its own content width {w_top}, got {narrow}"
    );
    assert_eq!(
        narrow, widest,
        "the column width is the widest member's, not the window's"
    );
}

// ---------------------------------------------------------------------------
// CloseButton
// ---------------------------------------------------------------------------

#[gpui::test]
fn close_button_reports_and_disabled_is_inert(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let dead_presses = events();
    let dead_recorded = dead_presses.clone();
    let probe_presses = events();
    let probe_recorded = probe_presses.clone();
    let cx = open_host(cx, move || {
        let close = presses.clone();
        let dead = dead_presses.clone();
        let probe = probe_presses.clone();
        // `.close-button` is a 24px square (`h-6`, box_size 24). The disabled
        // one sits first at y 0..24 — Tab must step *past* it — then the
        // enabled one at y 28..52 (4px gap) and the full-width probe button at
        // y 56..92.
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                CloseButton::new("cb-disabled")
                    .is_disabled(true)
                    .on_press(move |_, _, _| dead.borrow_mut().push("dead".into())),
            )
            .child(
                CloseButton::new("cb-enabled")
                    .on_press(move |_, _, _| close.borrow_mut().push("close".into())),
            )
            .child(
                Button::new("cb-probe")
                    .label("Probe")
                    .full_width(true)
                    .on_press(move |_, _, _| probe.borrow_mut().push("probe".into())),
            )
            .into_any_element()
    });

    // Centre of the enabled box at y 28..52: (12, 40).
    click(cx, 12., 40.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["close"],
        "the close button must report exactly one press"
    );

    // The disabled one sits first, at y 0..24: centre (12, 12) — and must be
    // inert to the pointer.
    click(cx, 12., 12.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["close"],
        "a disabled close button must not report a press"
    );
    assert!(
        dead_recorded.borrow().is_empty(),
        "the disabled close button's own handler must never run"
    );

    // The disabled button must also leave the tab order. The pointer pressed
    // nothing focusable, so the focus is back on the host root: the first Tab
    // reaches the enabled close button (a real tab stop), and the second must
    // skip the disabled one and land on the probe — if the disabled button
    // were a stop, the second Tab would park on it and Space would die there.
    press(cx, "tab");
    press(cx, "tab");
    flush_frame(cx);
    press(cx, "space");
    assert_eq!(
        probe_recorded.borrow().as_slice(),
        ["probe"],
        "Tab must skip the disabled close button and reach the probe"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["close"],
        "the enabled close button must not fire again"
    );
}

// ---------------------------------------------------------------------------
// ToggleButton & ToggleButtonGroup
// ---------------------------------------------------------------------------

#[gpui::test]
fn toggle_button_uncontrolled_toggles(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let press_changes = changes.clone();
        // `default_selected` seeds the button's *own* state; `is_selected`
        // would be the controlled prop and hand the value back to nobody —
        // the uncontrolled path is the one that toggles.
        ToggleButton::new("tb-standalone")
            .label("Bold")
            .default_selected(true)
            .on_change(move |selected, _, _| changes.borrow_mut().push(format!("{selected}")))
            .on_press(move |_, _, _| press_changes.borrow_mut().push("press".into()))
            .into_any_element()
    });

    // The toggle has `px-4` (16px) around the measured 14px label, so its
    // centre x is 16 + w/2; it is 36px tall
    // (`h-9`), centre y = 18.
    let w =
        cx.update(|window, _| text_width(window.text_system(), "Bold", 14.0, FontWeight::MEDIUM));
    let centre_x = 16. + w / 2.;

    click(cx, centre_x, 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["false", "press"],
        "the first click must report the changed selection before onPress"
    );
    click(cx, centre_x, 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["false", "press", "true", "press"],
        "the second click must select it and keep the callback order"
    );
}

#[gpui::test]
fn toggle_button_group_single_and_multiple(cx: &mut TestAppContext) {
    let single_changes = events();
    let single_seen = single_changes.clone();
    let single_state = Rc::new(RefCell::new(Vec::new()));
    let multi_changes = events();
    let multi_seen = multi_changes.clone();
    let multi_state = Rc::new(RefCell::new(Vec::new()));

    let third_state = single_state;
    let fourth_state = multi_state;
    let cx = open_host(cx, move || {
        let single_changes = single_changes.clone();
        let multi_changes = multi_changes.clone();
        // `Rc::clone` (function form, not `.clone()`): the callback closure
        // must be `'static`, so it needs its own handle to the state; the
        // render reads the same state through another handle.
        let single_state = Rc::clone(&third_state);
        let multi_state = Rc::clone(&fourth_state);
        // The group is controlled by state the test owns (there is no
        // `uncontrolled` seed for a group); the recorded callback is the
        // assertion, and the owned state lets the next frame reflect the pick.
        // Two groups stacked 320px apart so the first (36px tall) never
        // overlaps the second; member *i* of each full-width group spans
        // x 640i..640(i+1), centre 320 + 640i.
        // The cloned keys are bound to a local so the RefCell borrow ends
        // before the callback closure moves the `Rc` into itself.
        let single_selected = single_state.borrow().iter().cloned().collect::<Vec<_>>();
        let multi_selected = multi_state.borrow().iter().cloned().collect::<Vec<_>>();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(320.))
            .child(
                ToggleButtonGroup::new("buttons-single-group")
                    .full_width(true)
                    .selection_mode(SelectionMode::Single)
                    .selected_keys(single_selected)
                    .on_selection_change(move |next, _, _| {
                        *single_state.borrow_mut() = next.to_vec();
                        let joined = next
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(",");
                        single_changes.borrow_mut().push(joined);
                    })
                    .child_toggle(ToggleButton::new("grp-s-b").key("bold").label("Bold"))
                    .child_toggle(ToggleButton::new("grp-s-i").key("italic").label("Italic"))
                    .child_toggle(
                        ToggleButton::new("grp-s-u")
                            .key("underline")
                            .label("Underline"),
                    ),
            )
            .child(
                ToggleButtonGroup::new("buttons-multiple-group")
                    .full_width(true)
                    .selection_mode(SelectionMode::Multiple)
                    .selected_keys(multi_selected)
                    .on_selection_change(move |next, _, _| {
                        *multi_state.borrow_mut() = next.to_vec();
                        let joined = next
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(",");
                        multi_changes.borrow_mut().push(joined);
                    })
                    .child_toggle(ToggleButton::new("grp-m-b").key("bold").label("Bold"))
                    .child_toggle(ToggleButton::new("grp-m-i").key("italic").label("Italic"))
                    .child_toggle(
                        ToggleButton::new("grp-m-u")
                            .key("underline")
                            .label("Underline"),
                    ),
            )
            .into_any_element()
    });

    // Single (first group, y = 18): Bold -> Italic (replaces) -> Italic
    // (clears, `disallowEmptySelection` is off) -> Bold.
    click(cx, 320., 18.);
    assert_eq!(single_seen.borrow().as_slice(), ["bold"]);
    click(cx, 960., 18.);
    assert_eq!(single_seen.borrow().as_slice(), ["bold", "italic"]);
    click(cx, 960., 18.);
    assert_eq!(single_seen.borrow().as_slice(), ["bold", "italic", ""]);
    click(cx, 320., 18.);
    assert_eq!(
        single_seen.borrow().as_slice(),
        ["bold", "italic", "", "bold"],
        "single mode must replace the pick and clear on re-click"
    );

    // Multiple (second group starts at 36 + 320, so y = 374): Bold -> Bold,
    // Italic (accumulates) -> Bold (toggles off).
    click(cx, 320., 374.);
    assert_eq!(multi_seen.borrow().as_slice(), ["bold"]);
    click(cx, 960., 374.);
    assert_eq!(multi_seen.borrow().as_slice(), ["bold", "bold,italic"]);
    click(cx, 320., 374.);
    assert_eq!(
        multi_seen.borrow().as_slice(),
        ["bold", "bold,italic", "italic"],
        "multiple mode must accumulate picks and toggle one off"
    );
}

// ---------------------------------------------------------------------------
// Link
// ---------------------------------------------------------------------------

#[gpui::test]
fn link_press_reports_and_disabled_is_inert(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let go = presses.clone();
        let dead = presses.clone();
        // Links size to their content; the measured label sets the click x.
        // The row: enabled link at x 0..w1, 8px gap, disabled at
        // x w1+8 .. w1+8+w2. A link's box is one 14px line (~19px) plus a
        // 1px bottom padding, so y = 10 is inside it.
        gpui::div()
            .flex()
            .flex_row()
            .gap(px(8.))
            .child(
                Link::new("lnk-go")
                    .label("Home")
                    .on_press(move |_, _, _| go.borrow_mut().push("go".into())),
            )
            .child(
                Link::new("lnk-dead")
                    .label("Disabled")
                    .is_disabled(true)
                    .on_press(move |_, _, _| dead.borrow_mut().push("dead".into())),
            )
            .into_any_element()
    });

    let w1 =
        cx.update(|window, _| text_width(window.text_system(), "Home", 14.0, FontWeight::MEDIUM));
    let w2 = cx
        .update(|window, _| text_width(window.text_system(), "Disabled", 14.0, FontWeight::MEDIUM));

    click(cx, w1 / 2., 10.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["go"],
        "an enabled link must report exactly one press"
    );
    click(cx, w1 + 8. + w2 / 2., 10.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["go"],
        "a disabled link must not report a press"
    );
}

/// A disabled Link must leave the tab order, like every other disabled control
/// in this port (`track_focus` is what puts an element in the order; Button,
/// CloseButton and ToggleButton all omit it when disabled, and AGENTS.md says
/// the same: v3 gives a disabled control nothing to move to).
///
/// Link used to register its focus handle and ring it unconditionally, so Tab
/// landed on the dead control and Space did nothing — which reads exactly like
/// the broken keyboard AGENTS.md warns about. `track_focus` and
/// `ring_if_focused` are now gated on interactivity like every other
/// pressable's, and this test pins the behaviour: one Tab skips the disabled
/// link and lands on the live probe behind it.
#[gpui::test]
fn disabled_link_leaves_tab_order(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let probe = presses.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                Link::new("lnk-skip")
                    .label("Disabled")
                    .is_disabled(true)
                    .on_press(|_, _, _| {}),
            )
            .child(
                Button::new("lnk-probe")
                    .label("Probe")
                    .full_width(true)
                    .on_press(move |_, _, _| probe.borrow_mut().push("probe".into())),
            )
            .into_any_element()
    });

    // One Tab from the root must skip the disabled link entirely and land on
    // the probe; Space then activates it.
    press(cx, "tab");
    flush_frame(cx);
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["probe"],
        "a disabled link must not be a tab stop"
    );
}

// ---------------------------------------------------------------------------
// Chip
// ---------------------------------------------------------------------------

#[gpui::test]
fn chip_close_reports_and_plain_chip_has_nothing_to_press(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let close = presses.clone();
        // v3's Chip has no close affordance at all — its API table documents
        // children/className/color/variant/size and nothing else, and this
        // port matches (the port's own docs route removable chips to TagGroup,
        // whose on_remove is driven in collections.rs). The closest pressable
        // surface a Chip can host is a CloseButton composed as a leading
        // child: the `px-2` root starts that child at x = 8, and the 24px
        // close box plus the 2px vertical padding around the 20px label line
        // make the chip 28px tall — close centre (20, 14).
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                Chip::new()
                    .child(
                        CloseButton::new("chip-x")
                            .on_press(move |_, _, _| close.borrow_mut().push("close".into())),
                    )
                    .child(ChipLabel::new().child("Tag")),
            )
            // A plain chip below it (y 32..56) with no composed affordance.
            .child(Chip::new().child(ChipLabel::new().child("Tag")))
            .into_any_element()
    });

    // The composed close button reports exactly once.
    click(cx, 20., 14.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["close"],
        "a close affordance composed into a chip must report its press"
    );

    // The plain chip has no press surface anywhere: neither the centre nor
    // the trailing slot where a close button would sit records anything (it
    // has no callback and takes no child slot, so a click cannot even reach
    // one — the probe proves nothing registered).
    click(cx, 24., 40.);
    click(cx, 40., 40.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["close"],
        "a plain chip must have nothing to press"
    );
}

// ---------------------------------------------------------------------------
// Alert
// ---------------------------------------------------------------------------

/// v3's migration guide removes `isClosable`/`onClose`: Alert takes end
/// content only as composed children, and the close affordance is an ordinary
/// `CloseButton` child. The composed one must be both pointer-reachable — a
/// click on it records a press while a click on the alert body records
/// nothing — and keyboard-reachable: one Tab from the host root lands on it,
/// and Space activates it. A sibling alert with no composed close affordance
/// must be dead air at the spot where the removed built-in glyph used to sit.
#[gpui::test]
fn alert_composed_close_button_reports_and_alert_has_no_built_in_close(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let presses = presses.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                Alert::new("Saved")
                    .description("Your changes are live.")
                    .child(
                        CloseButton::new("alert-composed-x")
                            .on_press(move |_, _, _| presses.borrow_mut().push("close".into())),
                    ),
            )
            .child(Alert::new("No close here"))
            .into_any_element()
    });

    // The first alert is `w_full px-4 py-3`: its composed 24px CloseButton
    // hugs the content's right edge (x 1880..1904) and starts at the 12px top
    // padding, so its centre is (1892, 24). The text column's title and
    // description are two 20px lines plus the 2px gap (42px), so the alert
    // spans y 0..66 and the plain sibling starts at y 74; its would-be glyph
    // spot from the removed built-in close is (1897, 74 + 19) = (1897, 93).

    // Keyboard first, while the focus still sits on the host root: one Tab
    // reaches the composed CloseButton — the only stop inside either alert —
    // and Space activates it through the very click listener the pointer uses.
    press(cx, "tab");
    flush_frame(cx);
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["close"],
        "a CloseButton composed into an Alert must be reachable and activatable by keyboard"
    );

    // Pointer next: a click on the composed CloseButton reports the dismissal.
    click(cx, 1892., 24.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["close", "close"],
        "a CloseButton composed into an Alert must report its press"
    );

    // A press on the alert body must not dismiss: it has no handler of its
    // own, and the composed close button sits over 1300px away.
    click(cx, 500., 24.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["close", "close"],
        "the alert body must not report a close"
    );

    // The sibling alert composes nothing, so where Alert's removed built-in
    // close glyph used to answer there must be nothing to press.
    click(cx, 1897., 93.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["close", "close"],
        "an alert with no composed close affordance must have nothing at the removed built-in close position"
    );
}

// ---------------------------------------------------------------------------
// Render props
// ---------------------------------------------------------------------------

/// Pinned React Aria keeps a pending Button focusable while disabling its
/// hover and press interactions, and hands `isPending` to the children render
/// function independently from `isDisabled`.
#[gpui::test]
fn button_content_render_prop_reports_pending_and_retains_focus(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new((false, false, false, false, false)));
    let record = seen.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        Button::new("btn-pending-state")
            .full_width(true)
            .is_pending(true)
            .content(move |state: InteractiveState| {
                *record.borrow_mut() = (
                    state.is_pending,
                    state.is_disabled,
                    state.is_focused,
                    state.is_hovered,
                    state.is_pressed,
                );
                gpui::div().child("pending".to_owned()).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        *seen.borrow(),
        (true, false, false, false, false),
        "pending must be reported separately from disabled"
    );

    press(cx, "tab");
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, false, true, false, false),
        "a pending button must retain its focus stop without becoming interactive"
    );

    let centre = point(px(960.), px(18.));
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, false, true, false, false),
        "pending must suppress hover and press render state"
    );
}

/// Entering an inert pending period must clear the keyed interaction slot.
/// Otherwise a pointer that leaves while handlers are detached resurfaces as
/// a stale hover as soon as pending ends.
#[gpui::test]
fn button_pending_transition_clears_stale_interaction_state(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new((false, false)));
    let record = seen.clone();
    let pending = Rc::new(RefCell::new(false));
    let for_view = pending.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        Button::new("btn-pending-transition")
            .full_width(true)
            .is_pending(*for_view.borrow())
            .content(move |state: InteractiveState| {
                *record.borrow_mut() = (state.is_hovered, state.is_pending);
                gpui::div().child("state".to_owned()).into_any_element()
            })
            .into_any_element()
    });

    cx.simulate_mouse_move(
        point(px(960.), px(18.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    assert_eq!(*seen.borrow(), (true, false));

    *pending.borrow_mut() = true;
    flush_frame(cx);
    assert_eq!(*seen.borrow(), (false, true));

    cx.simulate_mouse_move(
        point(px(4.), px(500.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    *pending.borrow_mut() = false;
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (false, false),
        "ending pending must not revive hover recorded before the inert period"
    );
}

/// A disabled native button cannot retain focus. GPUI keeps the keyed focus
/// handle alive across renders, so the render-prop state must gate that stale
/// handle when `is_disabled` changes after focus was already inside.
#[gpui::test]
fn disabling_button_clears_render_prop_focus(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new((false, false, false)));
    let record = seen.clone();
    let disabled = Rc::new(RefCell::new(false));
    let for_view = disabled.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        Button::new("btn-disable-focus")
            .full_width(true)
            .is_disabled(*for_view.borrow())
            .content(move |state: InteractiveState| {
                *record.borrow_mut() =
                    (state.is_focused, state.is_focus_visible, state.is_disabled);
                gpui::div().child("state".to_owned()).into_any_element()
            })
            .into_any_element()
    });

    press(cx, "tab");
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, true, false),
        "the enabled button must report keyboard focus"
    );

    *disabled.borrow_mut() = true;
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (false, false, true),
        "disabling Button.Content must stop reporting focus"
    );

    *disabled.borrow_mut() = false;
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (false, false, false),
        "re-enabling must not restore focus without a new user action"
    );
}

#[gpui::test]
fn button_content_render_prop_sees_press(cx: &mut TestAppContext) {
    // v3 hands a button's children a function and passes in
    // `{isHovered, isPressed, isFocused, isFocusVisible}`. This port computes
    // them and hands them over as `util::InteractiveState` — but gpui reports
    // a hover and a press to a *handler*, so the render can only read what the
    // last frame recorded: both values are one frame behind the pointer. The
    // test therefore forces a frame after each event (`flush_frame`) and reads
    // the closure's latest snapshot, which is the frame *after* the one the
    // event dispatched against.
    let seen = Rc::new(RefCell::new((false, false)));
    let record = seen.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        Button::new("btn-state")
            .full_width(true)
            .content(move |state: InteractiveState| {
                *record.borrow_mut() = (state.is_hovered, state.is_pressed);
                gpui::div().child("state".to_owned()).into_any_element()
            })
            .into_any_element()
    });

    let centre = point(px(960.), px(18.));

    // The first render drew before any pointer event: nothing hovered, nothing
    // pressed.
    assert_eq!(*seen.borrow(), (false, false), "initial state must be idle");

    // Move the pointer onto the button: `on_hover` stashes hover=true, and the
    // forced frame hands it to the closure.
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, false),
        "the frame after the move must see the hover"
    );

    // Press down: `on_mouse_down` stashes pressed=true; no mouse-up has run,
    // so the frame after the down sees hover and press together.
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, true),
        "the frame after the down must see the press"
    );

    // Release: `on_mouse_up` clears the press; the hover survives.
    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, false),
        "the frame after the up must see the press lifted"
    );

    // Leave the button entirely: the hover clears too.
    cx.simulate_mouse_move(
        point(px(4.), px(500.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (false, false),
        "the frame after leaving must see the hover lifted"
    );
}

/// `hover_bg` names the colour the fade eases *to*, so — unlike an `sx`
/// background on its own, which pins both endpoints — the animated fill really
/// is mounted and re-keyed while the pointer crosses the button. That fill is
/// the thing the single-hover-listener fix in `anim::hover_fade` exists to
/// protect, so this drives the same hover/press sequence as
/// `button_content_render_prop_sees_press` with the custom hover colour set
/// and no `sx`: the interaction slot must still see every transition.
#[gpui::test]
fn button_hover_bg_keeps_the_interaction_slot_reporting(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new((false, false)));
    let record = seen.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        Button::new("btn-hover-bg")
            .full_width(true)
            .hover_bg(gpui::rgba(0x804000ff))
            .content(move |state: InteractiveState| {
                *record.borrow_mut() = (state.is_hovered, state.is_pressed);
                gpui::div().child("state".to_owned()).into_any_element()
            })
            .into_any_element()
    });

    let centre = point(px(960.), px(18.));

    assert_eq!(*seen.borrow(), (false, false), "initial state must be idle");

    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, false),
        "the frame after the move must see the hover"
    );

    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, true),
        "the frame after the down must see the press"
    );

    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, false),
        "the frame after the up must see the press lifted"
    );

    cx.simulate_mouse_move(
        point(px(4.), px(500.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (false, false),
        "the frame after leaving must see the hover lifted"
    );
}

#[gpui::test]
fn button_content_render_prop_sees_keyboard_press(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(false));
    let record = seen.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        gpui::div()
            .child(Button::new("btn-key-state").full_width(true).content(
                move |state: InteractiveState| {
                    *record.borrow_mut() = state.is_pressed;
                    gpui::div().child("state".to_owned()).into_any_element()
                },
            ))
            .child(Button::new("btn-key-next").label("Next"))
            .into_any_element()
    });

    press(cx, "tab");
    cx.simulate_keystrokes("enter");
    flush_frame(cx);
    assert!(
        *seen.borrow(),
        "the frame after Enter down must report the held keyboard press"
    );

    cx.simulate_keystrokes("tab");
    flush_frame(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    flush_frame(cx);
    assert!(
        !*seen.borrow(),
        "the frame after Enter up must report the keyboard press lifted"
    );

    press(cx, "shift-tab");
    cx.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse("space").unwrap(),
        is_held: true,
        prefer_character_input: false,
    });
    flush_frame(cx);
    assert!(
        !*seen.borrow(),
        "a repeated Space without its first keydown must not begin a press"
    );

    cx.simulate_keystrokes("space");
    flush_frame(cx);
    assert!(
        *seen.borrow(),
        "the frame after Space down must report the held keyboard press"
    );

    cx.simulate_mouse_up(
        point(px(4.), px(500.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    flush_frame(cx);
    assert!(
        *seen.borrow(),
        "an unrelated mouse-up must not release a held keyboard press"
    );

    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });
    flush_frame(cx);
    assert!(
        !*seen.borrow(),
        "the frame after Space up must report the keyboard press lifted"
    );
}

#[gpui::test]
fn button_keyboard_press_releases_when_activation_moves_focus_out_of_root(
    cx: &mut TestAppContext,
) {
    let pressed = Rc::new(RefCell::new(false));
    cx.update(herogpui_theme::ThemeProvider::init);
    let observed_pressed = pressed.clone();
    let (view, cx) = cx.add_window_view(move |_, cx| FocusTransferHost {
        target: cx.focus_handle(),
        pressed: observed_pressed.clone(),
    });
    cx.simulate_mouse_move(
        point(px(-100.), px(-100.)),
        None::<MouseButton>,
        Modifiers::none(),
    );

    press(cx, "tab");
    cx.simulate_keystrokes("enter");
    flush_frame(cx);
    assert!(
        *pressed.borrow(),
        "Enter down must report the held keyboard press before focus transfer"
    );
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    flush_frame(cx);
    assert!(
        cx.update(|window, cx| view.read(cx).target.is_focused(window)),
        "button activation must move focus to the external focus handle"
    );
    assert!(
        !*pressed.borrow(),
        "Enter up must release the keyboard press after focus leaves the app root"
    );
}

struct FocusTransferHost {
    target: gpui::FocusHandle,
    pressed: Rc<RefCell<bool>>,
}

impl Render for FocusTransferHost {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<'_, Self>,
    ) -> impl IntoElement {
        let target = self.target.clone();
        let pressed = self.pressed.clone();
        extend::app_focus_root(
            gpui::div().child(
                Button::new("btn-focus-transfer")
                    .content(move |state: InteractiveState| {
                        *pressed.borrow_mut() = state.is_pressed;
                        gpui::div()
                            .child("transfer focus".to_owned())
                            .into_any_element()
                    })
                    .on_press(move |_, window, cx| {
                        window.focus(&target, cx);
                        cx.stop_propagation();
                    }),
            ),
            window,
            cx,
        )
    }
}

#[gpui::test]
fn button_content_render_prop_tracks_interleaved_keyboard_presses(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new((false, false)));
    let record = seen.clone();
    let cx = open_host(cx, move || {
        let first = record.clone();
        let second = record.clone();
        gpui::div()
            .child(Button::new("btn-key-first").content(move |state| {
                first.borrow_mut().0 = state.is_pressed;
                gpui::div().child("first".to_owned()).into_any_element()
            }))
            .child(Button::new("btn-key-second").content(move |state| {
                second.borrow_mut().1 = state.is_pressed;
                gpui::div().child("second".to_owned()).into_any_element()
            }))
            .into_any_element()
    });

    press(cx, "tab");
    cx.simulate_keystrokes("enter");
    flush_frame(cx);
    cx.simulate_keystrokes("tab");
    flush_frame(cx);
    cx.simulate_keystrokes("space");
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, true),
        "moving focus may begin a second press without losing the first key's release"
    );

    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, false),
        "Space up must release only the matching second press"
    );

    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (false, false),
        "Enter up must still release the first press after focus moved"
    );
}

/// A second component's render prop must survive a full pointer cycle too, so
/// the single-hover-listener fix is not button-specific. `ToggleButton` takes
/// the same `content` closure and hands `isSelected` over with the rest of
/// `util::InteractiveState`, so this drives the same hover/press sequence as
/// the Button test and additionally asserts the toggle bit flips with a click.
#[gpui::test]
fn toggle_button_content_render_prop_sees_state(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new((false, false, false)));
    let record = seen.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        // `default_selected(true)` seeds the button's *own* state, so the first
        // click flips the closure's `is_selected` from true to false (the
        // uncontrolled path — `is_selected(true)` would hand the value to
        // nobody and the toggle would be inert).
        ToggleButton::new("tb-state")
            .default_selected(true)
            .content(move |state: InteractiveState| {
                *record.borrow_mut() = (state.is_hovered, state.is_pressed, state.is_selected);
                // A fixed 64x16 box stands in for the label a caller's render
                // function would draw. ToggleButton `Size::Md` is 36px tall
                // (`h-9`) with `px-4` (16px) each side, so the centre column is
                // 16 + 32 = 48 and the centre row is 18.
                gpui::div()
                    .w(px(64.))
                    .h(px(16.))
                    .child("bold".to_owned())
                    .into_any_element()
            })
            .into_any_element()
    });

    let centre = point(px(48.), px(18.));

    // The first render drew before any pointer event, and `default_selected`
    // seeded the selection: idle and selected.
    assert_eq!(
        *seen.borrow(),
        (false, false, true),
        "initial state must be idle and selected"
    );

    // Move the pointer onto the toggle: the interaction slot hears the hover,
    // and the forced frame hands it to the closure.
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, false, true),
        "the frame after the move must see the hover"
    );

    // Press down: the press lands alongside the hover; no mouse-up has run.
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, true, true),
        "the frame after the down must see the press"
    );

    // Release: the press lifts, and the click the up completes toggles the
    // button's own state off — the frame after the up reports all three.
    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, false, false),
        "the up must lift the press and toggle the selection off"
    );

    // Leave the toggle entirely: the hover clears too.
    cx.simulate_mouse_move(
        point(px(4.), px(500.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (false, false, false),
        "the frame after leaving must see the hover lifted"
    );
}

/// `ToggleButtonRenderProps` includes the focus and disabled fields as well as
/// the pointer and selection fields driven above. Keyboard focus must reach
/// the closure without changing selection, and the app's focus-visible flag
/// must be reflected on the following frame.
#[gpui::test]
fn toggle_button_content_render_prop_sees_focus_state(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new((false, false, false, false)));
    let record = seen.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        ToggleButton::new("tb-focus-state")
            .default_selected(true)
            .content(move |state: InteractiveState| {
                *record.borrow_mut() = (
                    state.is_focused,
                    state.is_focus_visible,
                    state.is_disabled,
                    state.is_selected,
                );
                gpui::div()
                    .w(px(64.))
                    .h(px(16.))
                    .child("bold".to_owned())
                    .into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        *seen.borrow(),
        (false, false, false, true),
        "the initial render must report an enabled, unfocused selected toggle"
    );

    press(cx, "tab");
    flush_frame(cx);
    assert_eq!(
        *seen.borrow(),
        (true, true, false, true),
        "keyboard focus and its visible flag must reach the closure without changing selection"
    );
}

/// A disabled toggle still renders its content closure, but it is neither a
/// focus stop nor a press target. The closure must receive that disabled state
/// while both pointer and keyboard activation stay inert.
#[gpui::test]
fn disabled_toggle_button_content_reports_disabled_and_stays_inert(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new((false, false, false)));
    let record = seen.clone();
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        let changes = changes.clone();
        ToggleButton::new("tb-disabled-state")
            .default_selected(true)
            .is_disabled(true)
            .content(move |state: InteractiveState| {
                *record.borrow_mut() = (state.is_focused, state.is_disabled, state.is_selected);
                gpui::div()
                    .w(px(64.))
                    .h(px(16.))
                    .child("bold".to_owned())
                    .into_any_element()
            })
            .on_change(move |selected, _, _| {
                changes.borrow_mut().push(format!("{selected}"));
            })
            .into_any_element()
    });

    assert_eq!(
        *seen.borrow(),
        (false, true, true),
        "the disabled state must reach the content closure"
    );

    click(cx, 48., 18.);
    press(cx, "tab enter");
    flush_frame(cx);
    assert!(
        recorded.borrow().is_empty(),
        "a disabled toggle must answer neither pointer nor keyboard activation"
    );
    assert_eq!(
        *seen.borrow(),
        (false, true, true),
        "a disabled toggle must remain unfocused and selected"
    );
}

/// v3 presses with `transform: scale(0.97)` about the centre, riding
/// `button.css`'s `transition: transform 250ms var(--ease-smooth)`. The port
/// has no paint transform, so the pressed skin is inset on all four sides of
/// a stable slot and the inset ramps over the pinned timeline: the first
/// frame after the mouse down is still at rest, frames keep coming while it
/// runs, and the settled endpoint shrinks on both axes, keeps its centre,
/// and lifts its bottom edge — a top-anchored shrink (bottom edge stuck on
/// the resting line) fails here.
#[gpui::test]
fn button_press_collapses_toward_its_centre(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Option<gpui::Bounds<gpui::Pixels>>>> = Rc::new(RefCell::new(None));
    let sink = seen.clone();
    let cx = open_host(cx, move || {
        let sink = sink.clone();
        Button::new("btn-centre")
            .full_width(true)
            .child(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, _, _, _| {
                        *sink.borrow_mut() = Some(bounds);
                    },
                )
                .size_full(),
            )
            .into_any_element()
    });
    flush_frame(cx);
    let rest = seen.borrow().expect("the content probe painted at rest");
    let rest_centre = point(
        px(f32::from(rest.origin.x) + f32::from(rest.size.width) / 2.),
        px(f32::from(rest.origin.y) + f32::from(rest.size.height) / 2.),
    );
    let rest_bottom = f32::from(rest.origin.y) + f32::from(rest.size.height);

    cx.simulate_mouse_move(rest_centre, None, Modifiers::none());
    flush_frame(cx);
    cx.simulate_mouse_down(rest_centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(
        pending_frames(cx) > 0,
        "a press must start the transform ramp, not snap"
    );

    settle(cx);
    let pressed = seen
        .borrow()
        .expect("the content probe painted while pressed");
    let pressed_centre = point(
        px(f32::from(pressed.origin.x) + f32::from(pressed.size.width) / 2.),
        px(f32::from(pressed.origin.y) + f32::from(pressed.size.height) / 2.),
    );
    let pressed_bottom = f32::from(pressed.origin.y) + f32::from(pressed.size.height);

    let width_ratio = f32::from(pressed.size.width) / f32::from(rest.size.width);
    let height_ratio = f32::from(pressed.size.height) / f32::from(rest.size.height);
    assert!(
        (width_ratio - 0.97).abs() < 0.01 && (height_ratio - 0.97).abs() < 0.01,
        "both axes scale by 0.97, got width {width_ratio} height {height_ratio} (rest {rest:?}, pressed {pressed:?})"
    );
    assert!(
        (f32::from(pressed_centre.x) - f32::from(rest_centre.x)).abs() < 0.25
            && (f32::from(pressed_centre.y) - f32::from(rest_centre.y)).abs() < 0.25,
        "the press keeps its centre, rest {rest_centre:?} pressed {pressed_centre:?}"
    );
    assert!(
        rest_bottom - pressed_bottom > 0.4,
        "the bottom edge lifts with the press, rest {rest_bottom} pressed {pressed_bottom}"
    );

    cx.simulate_mouse_up(rest_centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(
        pending_frames(cx) > 0,
        "the release must ramp back rather than snap"
    );
    settle(cx);
    let released = seen
        .borrow()
        .expect("the content probe painted after release");
    assert!(
        (f32::from(released.size.height) - f32::from(rest.size.height)).abs() < 0.01,
        "the button springs back after release, got {released:?}"
    );
}

/// Releasing mid-press turns the ramp around: the transform track starts a new
/// generation from the frame actually on screen rather than restarting from
/// rest, keeps animating, and settles back at the resting box with the click
/// intact. The CSS transition being interrupted behaves the same way — it
/// eases from the current computed value.
#[gpui::test]
fn button_release_mid_press_resumes_from_the_painted_frame(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Option<gpui::Bounds<gpui::Pixels>>>> = Rc::new(RefCell::new(None));
    let sink = seen.clone();
    let cx = open_host(cx, move || {
        let sink = sink.clone();
        Button::new("btn-reverse")
            .child(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, _, _, _| {
                        *sink.borrow_mut() = Some(bounds);
                    },
                )
                .w(px(120.))
                .h(px(20.)),
            )
            .into_any_element()
    });
    flush_frame(cx);
    let rest = seen.borrow().expect("the content probe painted at rest");
    let centre = point(
        px(f32::from(rest.origin.x) + f32::from(rest.size.width) / 2.),
        px(f32::from(rest.origin.y) + f32::from(rest.size.height) / 2.),
    );

    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(60));
    assert!(
        pending_frames(cx) > 0,
        "the press ramp is still running 60ms in"
    );

    // Turn around mid-flight: a fresh generation mounts and keeps animating.
    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(
        pending_frames(cx) > 0,
        "a mid-press release must schedule the return ramp"
    );
    settle(cx);
    let released = seen
        .borrow()
        .expect("the content probe painted after release");
    assert!(
        (f32::from(released.size.height) - f32::from(rest.size.height)).abs() < 0.01
            && (f32::from(released.size.width) - f32::from(rest.size.width)).abs() < 0.01,
        "the interrupted press must settle back at rest, got {released:?}"
    );

    // And the mechanism is still alive: the next press ramps again.
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(
        pending_frames(cx) > 0,
        "a press after a mid-flight reversal must animate again"
    );
    settle(cx);
}

/// `motion-reduce:transition-none` keeps the pressed property values but
/// removes the timing: the pressed geometry and fill land on the first frame
/// and no animation is ever mounted.
#[gpui::test]
fn reduced_motion_press_snaps_without_animating(cx: &mut TestAppContext) {
    harness::still();
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Button::new("btn-still")
            .label("Still")
            .on_press(move |_, _, _| recorded.borrow_mut().push("press".into()))
            .into_any_element()
    });
    flush_frame(cx);

    let at = point(px(24.), px(18.));
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        pending_frames(cx),
        0,
        "reduced motion must snap the press instead of animating it"
    );

    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert_eq!(pending_frames(cx), 0, "the release must snap too");
    assert_eq!(
        presses.borrow().as_slice(),
        ["press"],
        "reduced motion must not swallow the press"
    );
}

/// The fills a control paints resting, hovered, and after the pointer has
/// left again, with motion on and each fade given the real time to settle.
/// The settled fade layer stays mounted and paints its endpoint, so the
/// resting endpoint it eases back to is on the scene after the leave.
fn resting_and_hovered_fills(
    cx: &mut TestAppContext,
    build: impl Fn() -> gpui::AnyElement + 'static,
) -> (Vec<gpui::Hsla>, Vec<gpui::Hsla>, Vec<gpui::Hsla>) {
    let vcx = open_host(cx, move || {
        gpui::div()
            .flex()
            .items_start()
            .p(px(24.))
            .child(build())
            .into_any_element()
    });
    let resting = harness::painted(vcx).solids();
    vcx.simulate_mouse_move(point(px(30.), px(30.)), None, Modifiers::none());
    harness::wait_ms(vcx, 300);
    let hovered = harness::painted(vcx).solids();
    vcx.simulate_mouse_move(point(px(600.), px(600.)), None, Modifiers::none());
    harness::wait_ms(vcx, 300);
    let left = harness::painted(vcx).solids();
    (resting, hovered, left)
}

/// `ToggleButton::hover_bg` follows the Button contract: the fade rests on the
/// `sx` background (or the variant's resting colour) and eases to the named
/// hover colour through the shared resolver.
#[gpui::test]
fn toggle_button_reads_the_hover_override_and_the_sx_background(cx: &mut TestAppContext) {
    let (rest, over): (gpui::Hsla, gpui::Hsla) =
        (gpui::rgb(0x224466).into(), gpui::rgb(0x88aa22).into());
    let (resting, hovered, left) = resting_and_hovered_fills(cx, move || {
        ToggleButton::new("tb-hover")
            .label("Bold")
            .sx(move |s| s.bg(rest))
            .hover_bg(over)
            .into_any_element()
    });
    assert!(
        harness::has_color(&resting, rest) && !harness::has_color(&resting, over),
        "the resting endpoint must be the sx background, painted {resting:?}"
    );
    assert!(
        harness::has_color(&hovered, over),
        "the hover endpoint must be the named override, painted {hovered:?}"
    );
    let stock_rest = cx.update(|cx| {
        use herogpui_theme::ActiveTheme;
        cx.colors().default.color
    });
    assert!(
        harness::has_color(&left, rest) && !harness::has_color(&left, stock_rest),
        "leaving must ease back to the sx background, not the variant's \
         stock fill, painted {left:?}"
    );

    let (_, stock_hover, _) = resting_and_hovered_fills(cx, move || {
        ToggleButton::new("tb-hover-stock")
            .label("Bold")
            .sx(move |s| s.bg(rest))
            .into_any_element()
    });
    assert!(
        !harness::has_color(&stock_hover, over),
        "without the override the hover must not paint it"
    );
}

/// The skin's box while the left button is held on it, sampled right after
/// the press and again once any ramp has settled, next to its resting box,
/// plus the fills of the frame right after the press.
fn press_boxes(
    cx: &mut TestAppContext,
    build: impl Fn() -> gpui::AnyElement + 'static,
    pick: impl Fn(&harness::Painted) -> gpui::Bounds<gpui::Pixels>,
) -> (
    gpui::Bounds<gpui::Pixels>,
    gpui::Bounds<gpui::Pixels>,
    gpui::Bounds<gpui::Pixels>,
    Vec<gpui::Hsla>,
) {
    let vcx = open_host(cx, move || {
        gpui::div()
            .flex()
            .items_start()
            .p(px(24.))
            .child(build())
            .into_any_element()
    });
    harness::wait_ms(vcx, 50);
    let resting = pick(&harness::painted(vcx));
    let at = resting.center();
    vcx.simulate_mouse_move(at, None, Modifiers::none());
    vcx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    vcx.update(|window, _| window.refresh());
    vcx.run_until_parked();
    let early_scene = harness::painted(vcx);
    let early = pick(&early_scene);
    harness::wait_ms(vcx, 400);
    let settled = pick(&harness::painted(vcx));
    vcx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    (resting, early, settled, early_scene.solids())
}

/// The widest filled quad: a toggle's painted skin.
fn skin(scene: &harness::Painted) -> gpui::Bounds<gpui::Pixels> {
    scene
        .quads
        .iter()
        .filter(|q| q.background.as_solid().is_some_and(|c| c.a > 0.))
        .max_by(|a, b| a.bounds.size.width.0.total_cmp(&b.bounds.size.width.0))
        .map(|q| scene.bounds(q))
        .expect("the control must paint its skin")
}

/// `toggle-button.css` declares the press as a transition with the same
/// tracks as `button.css`, so a standalone toggle rides the shared ramp — it
/// shrinks over time rather than snapping — while a grouped member keeps the
/// instant background swap and no scale.
#[gpui::test]
fn toggle_button_press_rides_the_pinned_ramp(cx: &mut TestAppContext) {
    let (resting, early, settled, _) = press_boxes(
        cx,
        || {
            ToggleButton::new("tb-ramp")
                .label("Bold")
                .into_any_element()
        },
        skin,
    );
    let (w0, w1, w2) = (
        f32::from(resting.size.width),
        f32::from(early.size.width),
        f32::from(settled.size.width),
    );
    assert!(
        w2 < w0 - 0.5,
        "a held standalone toggle must scale its skin down ({w0} -> {w2})"
    );
    assert!(
        w1 > w2 + 0.2,
        "the scale must ramp, not snap: {w1} right after the press, {w2} settled"
    );

    let (resting, _, settled, _) = press_boxes(
        cx,
        move || {
            ToggleButtonGroup::new("tg-ramp")
                .child_toggle(ToggleButton::new("tg-a").label("A"))
                .child_toggle(ToggleButton::new("tg-b").label("B"))
                .into_any_element()
        },
        |scene| {
            scene
                .quads
                .iter()
                .find(|q| {
                    let b = scene.bounds(q);
                    f32::from(b.origin.x) < 40. && q.background.as_solid().is_some_and(|c| c.a > 0.)
                })
                .map(|q| scene.bounds(q))
                .expect("the first member must paint")
        },
    );
    assert_eq!(
        resting.size, settled.size,
        "a grouped member must not scale on press"
    );

    // Remaining source-text check: the grouped member's instant active swap
    // to its hover fill, gated on no fade being mounted — with the fade
    // active its pressed endpoint is the same `hover_bg`, painted once on
    // the fill.
    let src = source_scan::component_src("toggle_button.rs");
    assert!(
        src.contains("e.active(move |style| style.bg(hover_bg))") && src.contains("fade.is_none()"),
        "a grouped member suppresses the scale and keeps the instant endpoint \
         only where no fade owns the colour"
    );
}

/// `button.css` presses to the variant's `--button-bg-pressed` token over the
/// scaled box, never by dimming the whole button. Outline's pressed fill is
/// `bg-default`, distinct from its hover wash.
#[gpui::test]
fn button_press_uses_the_pinned_background_endpoint(cx: &mut TestAppContext) {
    use herogpui_components::Variant;
    use herogpui_theme::ActiveTheme;
    harness::still();
    let vcx = open_host(cx, || {
        gpui::div()
            .flex()
            .items_start()
            .p(px(24.))
            .child(
                Button::new("b-press")
                    .label("Save")
                    .variant(Variant::Outline),
            )
            .into_any_element()
    });
    let pressed_token = vcx.update(|_, cx| cx.colors().default.color);
    let resting = harness::painted(vcx);
    assert!(
        !resting.solids().contains(&pressed_token),
        "the resting outline button must not paint the pressed token"
    );
    let at = point(px(40.), px(42.));
    vcx.simulate_mouse_move(at, None, Modifiers::none());
    vcx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    let pressed = harness::painted(vcx);
    let skins = pressed.filled(pressed_token);
    assert!(
        !skins.is_empty(),
        "the held button must paint the variant's pressed token\n{}",
        pressed.describe()
    );
    assert!(
        pressed.quads.iter().all(|q| q
            .background
            .as_solid()
            .is_none_or(|c| { (c.a - 1.).abs() < 1e-3 || c.a < 1e-3 })),
        "a press recolours; it never dims the button\n{}",
        pressed.describe()
    );
    vcx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
}

#[gpui::test]
fn custom_press_styles_paint_only_the_resolved_skin(cx: &mut TestAppContext) {
    let color = gpui::hsla(0.73, 0.83, 0.42, 1.);
    for kind in 0..3 {
        let (rest, _, held, _) = press_boxes(
            cx,
            move || match kind {
                0 => Button::new("styled-button")
                    .label("A")
                    .sx(move |el| el.size(px(28.)).p(px(0.)).bg(color))
                    .into_any_element(),
                1 => ToggleButton::new("styled-toggle")
                    .label("A")
                    .sx(move |el| el.size(px(28.)).p(px(0.)).bg(color))
                    .into_any_element(),
                _ => CloseButton::new("styled-close")
                    .sx(move |el| el.size(px(28.)).p(px(0.)).bg(color))
                    .into_any_element(),
            },
            move |scene| {
                scene
                    .filled(color)
                    .into_iter()
                    .map(|quad| scene.bounds(quad))
                    .max_by(|a, b| a.size.width.partial_cmp(&b.size.width).unwrap())
                    .expect("custom skin must paint")
            },
        );
        assert_eq!(rest.size, gpui::size(px(28.), px(28.)), "kind {kind}");
        assert!(
            held.size.width < rest.size.width && held.size.height < rest.size.height,
            "kind {kind}: {rest:?} -> {held:?}"
        );
    }
}

#[gpui::test]
fn content_fit_button_remeasures_after_its_label_changes(cx: &mut TestAppContext) {
    let label = Rc::new(RefCell::new("A"));
    let rendered = label.clone();
    let vcx = open_host(cx, move || {
        Button::new("dynamic-label")
            .label(*rendered.borrow())
            .into_any_element()
    });
    let resting = skin(&harness::painted(vcx));
    *label.borrow_mut() = "A much longer label";
    vcx.update(|window, _| window.refresh());
    let updated = skin(&harness::painted(vcx));
    assert!(
        updated.size.width > resting.size.width + px(40.),
        "{resting:?} -> {updated:?}"
    );
}

#[gpui::test]
fn relative_width_button_skin_fills_its_resting_slot(cx: &mut TestAppContext) {
    let color = gpui::hsla(0.67, 0.83, 0.42, 1.);
    let vcx = open_host(cx, move || {
        gpui::div()
            .w(px(200.))
            .flex()
            .child(
                Button::new("relative-width")
                    .label("Save")
                    .sx(move |el| el.w(gpui::relative(0.5)).bg(color)),
            )
            .into_any_element()
    });
    let painted = harness::painted(vcx);
    let widths: Vec<_> = painted
        .filled(color)
        .into_iter()
        .map(|quad| painted.bounds(quad).size.width)
        .collect();
    assert!(widths.contains(&px(100.)), "{widths:?}");
}

#[gpui::test]
fn identical_button_ids_in_distinct_windows_measure_independently(cx: &mut TestAppContext) {
    let first_width = {
        let first = open_host(cx, || {
            Button::new("same-window-local-id")
                .label("A")
                .into_any_element()
        });
        skin(&harness::painted(first)).size.width
    };
    let second = open_host(cx, || {
        Button::new("same-window-local-id")
            .label("A much longer window-local label")
            .into_any_element()
    });
    let second_width = skin(&harness::painted(second)).size.width;
    assert!(
        second_width > first_width + px(40.),
        "{first_width:?} -> {second_width:?}"
    );
}

#[gpui::test]
fn explicit_button_metrics_override_theme_style_metrics(cx: &mut TestAppContext) {
    use herogpui_theme::{ActiveTheme, ButtonStyle, ComponentTheme, ComponentThemes, Theme};
    let color = gpui::hsla(0.67, 0.83, 0.42, 1.);
    let vcx = open_host(cx, move || {
        Button::new("metric-precedence")
            .label("A")
            .recipe("metrics")
            .height(px(28.))
            .width(px(40.))
            .min_width(px(40.))
            .padding_x(px(0.))
            .radius(px(4.))
            .sx(move |el| el.bg(color))
            .into_any_element()
    });
    vcx.update(|_, cx| {
        let base = cx.theme().clone();
        herogpui_theme::set_theme(
            Theme::builder("metrics", base)
                .components(ComponentThemes::default().button(
                    ComponentTheme::new(ButtonStyle::default()).recipe(
                        "metrics",
                        ButtonStyle::default().style(|el| {
                            el.h(px(60.))
                                .w(px(200.))
                                .min_w(px(100.))
                                .px(px(24.))
                                .rounded(px(20.))
                        }),
                    ),
                ))
                .build(),
            cx,
        );
    });
    let scene = harness::painted(vcx);
    let painted: Vec<_> = scene
        .filled(color)
        .into_iter()
        .map(|quad| scene.bounds(quad).size)
        .collect();
    assert!(
        painted
            .iter()
            .all(|size| *size == gpui::size(px(40.), px(28.))),
        "{painted:?}"
    );
    assert!(!painted.is_empty());
}
