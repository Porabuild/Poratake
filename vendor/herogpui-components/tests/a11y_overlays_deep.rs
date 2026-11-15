//! What wave 2 of the accessibility contract changed, and what can be checked.
//!
//! Read `a11y_deep.rs` first: it pins the fact that the AccessKit tree itself
//! is *not* observable from the headless platform, and that test still holds
//! for everything here. So these tests assert the other half — the half a role
//! change can actually break.
//!
//! # Why an overlay is the sharp case
//!
//! An AccessKit node id is a hash of the element's `GlobalElementId`, and that
//! id path is *also* gpui's key for per-element state: the hover slot, the
//! press latch, the scroll offset, the focus handle. Wave 2 gave several
//! overlays an element id where they had none, because an element with no id
//! produces no node and a role set on it is silently dropped:
//!
//! - `Modal`, `Drawer`, `AlertDialog` and `Popover` panels (`{id}-dialog`),
//! - each open `Accordion` panel (`{id}-{key}-panel`) and `Disclosure` body
//!   (`{id}-panel`),
//! - the `Tooltip` tip (`{key}-tip`),
//! - each toast card (the toast's own numeric id).
//!
//! Every one of those nests its existing descendants one segment deeper. A
//! panel's close button, its action button, its scroller and any control the
//! caller composed inside it all move — and two instances that ended up
//! sharing a path would share one press latch and one node while looking
//! completely normal on screen. Overlays make that worse than a form control
//! does, because an overlay mounts detached from the trigger that owns it, so
//! nothing on screen says which instance answered.
//!
//! Each test therefore drives *two* independent instances of the same overlay
//! and asserts they answer separately. Keyboard wherever the component has a
//! tab stop, so no assertion depends on a measured coordinate.

use crate::harness;

use std::collections::HashSet;

use gpui::{prelude::*, px, SharedString, TestAppContext};
use herogpui_components::{
    Accordion, AccordionItem, Button, Disclosure, DisclosureGroup, Dropdown, MenuItem, Popover,
    Toast, ToastViewport,
};
use herogpui_core::SelectionMode;

use harness::{click, events, open_host, press};

/// A pair of overlays side by side, each in its own fixed-width column so a
/// click coordinate is arithmetic rather than a guess.
fn side_by_side(left: gpui::AnyElement, right: gpui::AnyElement) -> gpui::AnyElement {
    gpui::div()
        .flex()
        .flex_row()
        .child(gpui::div().w(px(COLUMN)).child(left))
        .child(gpui::div().w(px(COLUMN)).child(right))
        .into_any_element()
}

const COLUMN: f32 = 320.;

// ---------------------------------------------------------------------------
// Disclosure and accordion: the panels that took ids
// ---------------------------------------------------------------------------

/// `Accordion`'s open panel took an id so it can report `role="group"`, and
/// each trigger row now states `role="button"` with `aria-expanded`. The panel
/// id hangs off the same `{id}-{key}` prefix the row does, so a second
/// accordion whose id collided would nest its panels onto the first one's.
/// Two accordions must expand from their own headers alone.
#[gpui::test]
fn two_accordions_expand_independently(cx: &mut TestAppContext) {
    harness::still();
    let toggled = events();
    let recorded = toggled.clone();
    let cx = open_host(cx, move || {
        let left = toggled.clone();
        let right = toggled.clone();
        side_by_side(
            Accordion::new(vec![
                AccordionItem::new("one", "Item one").content(gpui::div().h(px(40.))),
                AccordionItem::new("two", "Item two").content(gpui::div().h(px(40.))),
            ])
            .id("left-accordion")
            .on_expanded_change(move |keys: &HashSet<SharedString>, _, _| {
                left.borrow_mut().push(format!("left:{}", keys.len()));
            })
            .into_any_element(),
            Accordion::new(vec![
                AccordionItem::new("one", "Item one").content(gpui::div().h(px(40.))),
                AccordionItem::new("two", "Item two").content(gpui::div().h(px(40.))),
            ])
            .id("right-accordion")
            .on_expanded_change(move |keys: &HashSet<SharedString>, _, _| {
                right.borrow_mut().push(format!("right:{}", keys.len()));
            })
            .into_any_element(),
        )
    });

    // Each trigger row is `py-4` around a 20px line: 52px tall, so the first
    // row's centre is y 26 in either column.
    click(cx, 60., 26.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:1"],
        "the left accordion's first row must expand the left accordion alone"
    );

    click(cx, COLUMN + 60., 26.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:1", "right:1"],
        "the right accordion must answer its own header, not the left one's"
    );
}

/// `Disclosure`'s body took an id for `role="group"`, which nests everything
/// the caller composed inside it — and inside a `DisclosureGroup` the body's
/// id is derived from the *item key*, which the group reuses across rows. Two
/// groups holding the same keys must still open separately, or one row's body
/// would carry the other's element state.
#[gpui::test]
fn two_disclosure_groups_open_independently(cx: &mut TestAppContext) {
    harness::still();
    let toggled = events();
    let recorded = toggled.clone();
    let cx = open_host(cx, move || {
        let left = toggled.clone();
        let right = toggled.clone();
        side_by_side(
            DisclosureGroup::new("left-group")
                .item("a", "Section A", gpui::div().h(px(40.)))
                .on_expanded_change(move |keys: &HashSet<SharedString>, _, _| {
                    left.borrow_mut().push(format!("left:{}", keys.len()));
                })
                .into_any_element(),
            DisclosureGroup::new("right-group")
                .item("a", "Section A", gpui::div().h(px(40.)))
                .on_expanded_change(move |keys: &HashSet<SharedString>, _, _| {
                    right.borrow_mut().push(format!("right:{}", keys.len()));
                })
                .into_any_element(),
        )
    });

    // The trigger is a `Button`, which is a keyboard tab stop; the two groups
    // are therefore the first two stops on the page.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:1"],
        "the first tab stop is the left group's trigger"
    );

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:1", "right:1"],
        "the second stop must be the right group: one trigger each, or the two \
         groups share a tab stop and an expansion"
    );
}

/// A single `Disclosure` composed twice with the *same* body content: the body
/// id derives from the disclosure's own id, so two of them must not fold into
/// one. Driven from the keyboard, which also proves the two triggers are
/// separate focus handles.
#[gpui::test]
fn two_disclosures_toggle_their_own_bodies(cx: &mut TestAppContext) {
    harness::still();
    let toggled = events();
    let recorded = toggled.clone();
    let cx = open_host(cx, move || {
        let left = toggled.clone();
        let right = toggled.clone();
        side_by_side(
            Disclosure::new("left-disclosure", "Details")
                .on_expanded_change(move |expanded: &bool, _, _| {
                    left.borrow_mut().push(format!("left:{expanded}"));
                })
                .child(gpui::div().h(px(40.)))
                .into_any_element(),
            Disclosure::new("right-disclosure", "Details")
                .on_expanded_change(move |expanded: &bool, _, _| {
                    right.borrow_mut().push(format!("right:{expanded}"));
                })
                .child(gpui::div().h(px(40.)))
                .into_any_element(),
        )
    });

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(recorded.borrow().as_slice(), ["left:true"]);

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:true", "right:true"],
        "the right disclosure must open on its own trigger; a shared body id \
         would also mean a shared expansion slot"
    );
}

// ---------------------------------------------------------------------------
// Popover: the dialog panel that took an id
// ---------------------------------------------------------------------------

/// `Popover`'s panel took an id for `role="dialog"`, so the close button it
/// composes — `{id}-close` — now lives at `{id}-dialog/{id}-close`. Two open
/// popovers must close from their own affordance: a shared path would give
/// both panels one press latch, and the wrong one would answer.
#[gpui::test]
fn two_open_popovers_close_from_their_own_button(cx: &mut TestAppContext) {
    harness::still();
    let closed = events();
    let recorded = closed.clone();
    let cx = open_host(cx, move || {
        let left = closed.clone();
        let right = closed.clone();
        side_by_side(
            Popover::new(Button::new("left-trigger").label("Open"))
                .id("left-popover")
                .default_open(true)
                .title("Left")
                .show_close_button(true)
                .on_open_change(move |open: &bool, _, _| {
                    left.borrow_mut().push(format!("left:{open}"));
                })
                .into_any_element(),
            Popover::new(Button::new("right-trigger").label("Open"))
                .id("right-popover")
                .default_open(true)
                .title("Right")
                .show_close_button(true)
                .on_open_change(move |open: &bool, _, _| {
                    right.borrow_mut().push(format!("right:{open}"));
                })
                .into_any_element(),
        )
    });
    cx.run_until_parked();

    // Both panels claim focus as they open and `util::trap_tab` keeps Tab
    // inside the claimant, so the *last* panel to open owns the keyboard —
    // here the right one. That ordering is what makes this a real two-instance
    // test rather than a coordinate guess: the close button the keyboard
    // reaches belongs to a specific panel, and it must report that panel.
    press(cx, "tab");
    press(cx, "enter");
    cx.run_until_parked();
    assert_eq!(
        recorded.borrow().as_slice(),
        ["right:false"],
        "the focused panel is the right one, and its close button must close \
         the right popover alone"
    );

    // Closing the right panel hands the focus back to the right *trigger*, so
    // a forward Tab from there would reopen it. Shift-Tab walks back into the
    // panel that is still open, whose close must be its own — a shared
    // `{id}-dialog/{id}-close` path would have made the first Enter answer for
    // both panels at once.
    press(cx, "shift-tab");
    press(cx, "enter");
    cx.run_until_parked();
    assert_eq!(
        recorded.borrow().as_slice(),
        ["right:false", "left:false"],
        "the left panel must keep its own close button, not share the right \
         panel's press latch"
    );
}

// ---------------------------------------------------------------------------
// Dropdown: the roles that are conditional on the selection mode
// ---------------------------------------------------------------------------

/// A menu row's role is not fixed: `useMenuItem.js` reports `menuitem`,
/// `menuitemradio` or `menuitemcheckbox` depending on the *menu's* selection
/// mode, and carries `aria-checked` only when the menu selects at all. That is
/// a conditional contract read from the parent, exactly like a toggle button
/// in a single-selection group — so the thing that can break it is two menus
/// handing each other their selection. Two dropdowns in different modes must
/// keep their own.
#[gpui::test]
fn two_dropdown_menus_select_independently(cx: &mut TestAppContext) {
    harness::still();
    let picked = events();
    let recorded = picked.clone();
    let cx = open_host(cx, move || {
        let left = picked.clone();
        let right = picked.clone();
        side_by_side(
            Dropdown::new(
                "left-dropdown",
                Button::new("left-trigger").label("Open"),
                vec![MenuItem::new("a", "A"), MenuItem::new("b", "B")],
                true,
            )
            .selection_mode(SelectionMode::Single)
            .on_selection_change(move |keys: &[SharedString], _, _| {
                left.borrow_mut().push(format!("left:{}", keys.join(",")));
            })
            .into_any_element(),
            Dropdown::new(
                "right-dropdown",
                Button::new("right-trigger").label("Open"),
                vec![MenuItem::new("a", "A"), MenuItem::new("b", "B")],
                true,
            )
            .selection_mode(SelectionMode::Multiple)
            .on_selection_change(move |keys: &[SharedString], _, _| {
                right.borrow_mut().push(format!("right:{}", keys.join(",")));
            })
            .into_any_element(),
        )
    });
    cx.run_until_parked();

    // Both menus are open below their triggers. The trigger button is 36px
    // tall and the panel opens 8px below it with 6px of its own padding, so
    // the first 36px row's centre is y 36 + 8 + 6 + 18 = 68.
    click(cx, 60., 68.);
    cx.run_until_parked();
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:a"],
        "the left menu's first row must select in the left menu alone"
    );

    click(cx, COLUMN + 60., 68.);
    cx.run_until_parked();
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:a", "right:a"],
        "the right menu must report its own selection, not the left menu's"
    );
}

// ---------------------------------------------------------------------------
// A note on the other three dialogs
// ---------------------------------------------------------------------------
//
// `Modal`, `Drawer` and `AlertDialog` took the identical `{id}-dialog` panel
// id that `Popover` did, and their composed `{id}-close-{n}` triggers moved
// one segment with it. They are not driven again here: two of them open at
// once cannot be tab-driven the way the popovers above are, because each one
// claims the window focus as it opens and `util::trap_tab` then holds it, so
// only the last claimant answers the keyboard and the second assertion would
// be testing the claim order rather than the ids. Their close triggers,
// scrollers and dismissal stacks are already driven instance-by-instance in
// `overlays.rs`, `overlay_stack_dialogs_deep.rs` and `drawer_deep.rs`, which
// is where a broken id path would surface.

// ---------------------------------------------------------------------------
// Toast: the card that took an id
// ---------------------------------------------------------------------------

/// Each toast card took the toast's own numeric id for `role="alertdialog"`,
/// which nests its close affordance, its action button and its spinner one
/// segment deeper. Two toasts stacked in one region must therefore still close
/// one at a time, and the right one.
#[gpui::test]
fn two_stacked_toasts_close_one_at_a_time(cx: &mut TestAppContext) {
    harness::still();
    let actions = events();
    let first = actions.clone();
    let second = actions.clone();
    cx.update(move |cx| {
        Toast::new("First")
            .timeout(std::time::Duration::ZERO)
            .action("Undo first", move |_| {
                first.borrow_mut().push("first-action".into());
            })
            .push(None, cx);
        Toast::new("Second")
            .timeout(std::time::Duration::ZERO)
            .action("Undo second", move |_| {
                second.borrow_mut().push("second-action".into());
            })
            .push(None, cx);
    });
    let cx = open_host(cx, || ToastViewport::new().into_any_element());
    cx.run_until_parked();

    let titles = |cx: &mut gpui::VisualTestContext| {
        cx.update(|_, cx| {
            herogpui_components::toast_store(cx)
                .read(cx)
                .toasts()
                .iter()
                .map(|t| t.title.to_string())
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(
        titles(cx),
        ["Second", "First"],
        "React Stately unshifts, so the newest toast is frontmost"
    );

    // Only the frontmost card is interactive; its action is the first tab stop
    // and its close is the second.
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    cx.run_until_parked();
    assert_eq!(
        actions.borrow().as_slice(),
        [] as [&str; 0],
        "the close must not run either card's action handler"
    );
    assert_eq!(
        titles(cx),
        ["First"],
        "closing the frontmost card must leave the other one standing — a \
         shared card path would have taken both, or the wrong one"
    );

    // The card that was behind is now frontmost and answers on its own.
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    cx.run_until_parked();
    assert!(
        titles(cx).is_empty(),
        "the second card must close from its own affordance"
    );
    assert_eq!(
        actions.borrow().as_slice(),
        [] as [&str; 0],
        "neither action handler may have run"
    );
}
