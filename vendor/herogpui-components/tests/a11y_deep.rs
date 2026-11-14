//! What the accessibility contract changed, and what can be checked here.
//!
//! # The tree itself is not observable from a headless test
//!
//! gpui builds an AccessKit `TreeUpdate` only while accessibility is *active*
//! for the window (`Window::draw` gates the whole thing on
//! `A11y::is_active()`), and the only thing that flips that flag is the
//! platform adapter calling the activation callback gpui hands it in
//! `PlatformWindow::a11y_init`. `gpui-pre-0.3.11/src/platform.rs` defines
//! `a11y_init` as an empty default method and the test platform does not
//! override it, so on the headless platform the flag is never set, no
//! `TreeUpdate` is ever produced, and `Window::debug_a11y_tree_json` — the one
//! public reader — has nothing to return. There is no test hook to force it:
//! `Application::new_inaccessible` can only force it *off*.
//!
//! [`the_headless_harness_cannot_observe_the_accessibility_tree`] pins that
//! rather than leaving it as folklore. If a later gpui bump makes the tree
//! readable, that test fails, and the failure is the prompt to replace the
//! tests below with real node assertions.
//!
//! # So what is checked
//!
//! Two things that *are* observable, and that the wave-1 change could break:
//!
//! - **The derivations.** `a11y::Name::field`'s description join, `Range`'s
//!   clamp and its indeterminate case, and the mixed-checkbox mapping are pure
//!   functions with unit tests beside them in
//!   `crates/herogpui-components/src/a11y.rs`.
//! - **The id paths.** An AccessKit node id is a hash of the element's
//!   `GlobalElementId`, so several components had to acquire ids they did not
//!   have: `CheckboxGroup` and `RadioGroup` roots, `Slider`'s group and each of
//!   its thumbs, `NumberField`'s group. Every one of those nests its existing
//!   descendants one level deeper in the id path, which is also the key for
//!   gpui's per-element state. Two instances of a control that now share a
//!   path would share one hover slot, one press latch and one a11y node — and
//!   would look completely normal on screen. The tests below drive two
//!   instances of each and assert they still answer separately.
//!
//! Everything is keyboard-driven where it can be, so no assertion depends on a
//! measured coordinate that a layout change would silently move.
//!
//! Wave 2 — the overlays and the disclosure family — is driven the same way in
//! the sibling binary `a11y_overlays_deep.rs`. Wave 3 is `a11y_collections_deep.rs`.
//! Wave 5 — calendars and overlay-backed fields — is `a11y_pickers_deep.rs`.

use crate::harness;

use std::collections::HashSet;

use gpui::{prelude::*, px, SharedString, TestAppContext};
use herogpui_components::{
    Checkbox, CheckboxGroup, CheckboxOption, NumberField, NumberState, RadioGroup, RadioOption,
    Slider, ToggleButton, ToggleButtonGroup,
};

use harness::{click, events, open_host, press};

/// A pair of controls side by side, each in its own fixed-width column so a
/// click coordinate is arithmetic rather than a guess.
fn side_by_side(left: gpui::AnyElement, right: gpui::AnyElement) -> gpui::AnyElement {
    gpui::div()
        .flex()
        .flex_row()
        .child(gpui::div().w(px(COLUMN)).child(left))
        .child(gpui::div().w(px(COLUMN)).child(right))
        .into_any_element()
}

const COLUMN: f32 = 200.;

// ---------------------------------------------------------------------------
// What the harness can and cannot see
// ---------------------------------------------------------------------------

/// The headless platform never activates accessibility, so there is no tree to
/// assert against. Pinned here so the limitation is a checked fact rather than
/// a comment, and so a gpui bump that lifts it fails loudly.
#[gpui::test]
fn the_headless_harness_cannot_observe_the_accessibility_tree(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        Checkbox::new("observability")
            .label("Updates")
            .into_any_element()
    });

    cx.update(|window, _| {
        assert!(
            !window.is_a11y_active(),
            "the test platform's `a11y_init` is gpui's empty default, so nothing \
             can set the activation flag. If this ever passes, the tests in this \
             file should assert real AccessKit nodes instead."
        );
        assert!(
            window.debug_a11y_tree_json().is_none(),
            "`debug_a11y_tree_json` returns the last captured `TreeUpdate`, and \
             none is captured while accessibility is inactive"
        );
    });
}

// ---------------------------------------------------------------------------
// The ids the roles needed
// ---------------------------------------------------------------------------

/// `CheckboxGroup`'s root took an id so the group can report `role="group"`.
/// Its options' ids already derived from the same group id, so they are now one
/// segment deeper — and two groups that collided would tick together.
#[gpui::test]
fn two_checkbox_groups_keep_separate_selections(cx: &mut TestAppContext) {
    let picked = events();
    let recorded = picked.clone();
    let cx = open_host(cx, move || {
        let left = picked.clone();
        let right = picked.clone();
        side_by_side(
            CheckboxGroup::new("left-group", vec![CheckboxOption::new("a", "A")])
                .on_change(move |keys: &HashSet<SharedString>, _, _| {
                    left.borrow_mut().push(format!("left:{}", keys.len()));
                })
                .into_any_element(),
            CheckboxGroup::new("right-group", vec![CheckboxOption::new("a", "A")])
                .on_change(move |keys: &HashSet<SharedString>, _, _| {
                    right.borrow_mut().push(format!("right:{}", keys.len()));
                })
                .into_any_element(),
        )
    });

    click(cx, 8., 8.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:1"],
        "clicking the left group's only option must change the left group alone"
    );

    click(cx, COLUMN + 8., 8.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:1", "right:1"],
        "the right group must have its own selection, not the left group's"
    );
}

/// `RadioGroup`'s root took an id for `role="radiogroup"`, and every option's
/// id already hung off it. A radio group is one tab stop, so two of them are
/// two stops — which is also what proves the roving focus handles did not
/// collide.
#[gpui::test]
fn two_radio_groups_keep_separate_selections(cx: &mut TestAppContext) {
    let picked = events();
    let recorded = picked.clone();
    let cx = open_host(cx, move || {
        let left = picked.clone();
        let right = picked.clone();
        side_by_side(
            RadioGroup::new(
                "left-radios",
                vec![RadioOption::new("One"), RadioOption::new("Two")],
            )
            .on_change(move |value: &SharedString, _, _| {
                left.borrow_mut().push(format!("left:{value}"));
            })
            .into_any_element(),
            RadioGroup::new(
                "right-radios",
                vec![RadioOption::new("One"), RadioOption::new("Two")],
            )
            .on_change(move |value: &SharedString, _, _| {
                right.borrow_mut().push(format!("right:{value}"));
            })
            .into_any_element(),
        )
    });

    press(cx, "tab");
    press(cx, "down");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:Two"],
        "the first tab stop is the left group, and Down selects within it"
    );

    press(cx, "tab");
    press(cx, "down");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:Two", "right:Two"],
        "the second tab stop must be the right group: one focus handle each, or \
         the two groups share a roving stop"
    );
}

/// `Slider` took an id on its group (`role="group"`) and one per thumb
/// (`role="slider"`), which is the deepest new nesting in the wave. Two
/// sliders that shared a path would move together.
#[gpui::test]
fn two_sliders_move_independently(cx: &mut TestAppContext) {
    let moved = events();
    let recorded = moved.clone();
    let cx = open_host(cx, move || {
        let left = moved.clone();
        let right = moved.clone();
        side_by_side(
            Slider::new("left-slider", 50.)
                .on_change(move |value: &f32, _, _| {
                    left.borrow_mut().push(format!("left:{value}"));
                })
                .into_any_element(),
            Slider::new("right-slider", 50.)
                .on_change(move |value: &f32, _, _| {
                    right.borrow_mut().push(format!("right:{value}"));
                })
                .into_any_element(),
        )
    });

    press(cx, "tab");
    press(cx, "right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:51"],
        "the first tab stop is the left slider, and Right steps it once"
    );

    press(cx, "tab");
    press(cx, "right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:51", "right:51"],
        "the right slider must hold its own value, not the left one's"
    );
}

/// `NumberField`'s box took an id for `role="group"`, and each stepper is
/// named "Increase {label}" / "Decrease {label}" on a `role="button"` node.
/// The steppers' ids already derived from the state entity, so the group id
/// nests them one segment deeper; two fields that collided would step
/// together.
#[gpui::test]
fn two_number_fields_step_independently(cx: &mut TestAppContext) {
    let stepped = events();
    let recorded = stepped.clone();
    let left_state = cx.new(|cx| NumberState::new(cx, 1.));
    let right_state = cx.new(|cx| NumberState::new(cx, 10.));
    let left_view = left_state.clone();
    let right_view = right_state.clone();
    let cx = open_host(cx, move || {
        let left = stepped.clone();
        let right = stepped.clone();
        side_by_side(
            NumberField::new(left_view.clone())
                .label("Left")
                .full_width(true)
                .on_change(move |value: &f64, _, _| {
                    left.borrow_mut().push(format!("left:{value}"));
                })
                .into_any_element(),
            NumberField::new(right_view.clone())
                .label("Right")
                .full_width(true)
                .on_change(move |value: &f64, _, _| {
                    right.borrow_mut().push(format!("right:{value}"));
                })
                .into_any_element(),
        )
    });

    press(cx, "tab");
    press(cx, "up");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:2"],
        "the first tab stop is the left field, and Up steps it once"
    );

    press(cx, "tab");
    press(cx, "up");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:2", "right:11"],
        "the right field must hold its own number, not the left one's"
    );

    let values = cx.update(|_, cx| (left_state.read(cx).value(), right_state.read(cx).value()));
    assert_eq!(
        values,
        (2., 11.),
        "each field's own state must carry its own value"
    );
}

/// A toggle button inside a single-selection group reports `role="radio"`
/// instead of `role="button"`, which is a *conditional* contract: the member
/// only learns which one it is from the group. Two groups must not hand each
/// other their selection.
#[gpui::test]
fn two_toggle_button_groups_select_independently(cx: &mut TestAppContext) {
    let picked = events();
    let recorded = picked.clone();
    let cx = open_host(cx, move || {
        let left = picked.clone();
        let right = picked.clone();
        side_by_side(
            ToggleButtonGroup::new("left-toggles")
                .child_toggle(ToggleButton::new("left-bold").label("B"))
                .child_toggle(ToggleButton::new("left-italic").label("I"))
                .on_change(move |keys: &[SharedString], _, _| {
                    left.borrow_mut().push(format!("left:{}", keys.join(",")));
                })
                .into_any_element(),
            ToggleButtonGroup::new("right-toggles")
                .child_toggle(ToggleButton::new("right-bold").label("B"))
                .child_toggle(ToggleButton::new("right-italic").label("I"))
                .on_change(move |keys: &[SharedString], _, _| {
                    right.borrow_mut().push(format!("right:{}", keys.join(",")));
                })
                .into_any_element(),
        )
    });

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:left-bold"],
        "the first tab stop is the left group's first member"
    );

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:left-bold", "right:right-bold"],
        "the right group must keep its own selection"
    );
}
