//! Interaction tests — driving the controls, not looking at them.
//!
//! Every static axis of this port is measured by the `.shots/*.py` audits;
//! what they cannot see is whether a control *functions*. These tests open a
//! real gpui window on the headless test platform and simulate clicks and
//! keystrokes against it. The window, the recorder type and the input helpers
//! live in the shared `tests/harness/mod.rs` module.

use crate::harness;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{point, prelude::*, px, KeyUpEvent, Keystroke, Modifiers, TestAppContext};
use herogpui_components::{Checkbox, PickerItem, Select, Switch};

use harness::{open_host, Events};

#[gpui::test]
fn checkbox_click_toggles(cx: &mut TestAppContext) {
    let events: Events = Rc::new(RefCell::new(Vec::new()));
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        Checkbox::new("cb")
            .label("Toggle")
            .on_change(move |checked, _, _| {
                events.borrow_mut().push(format!("change:{checked}"));
            })
            .into_any_element()
    });

    // The row starts at the window origin and the 16px control box is its
    // first child, centred at (8, 8).
    cx.simulate_click(point(px(8.), px(8.)), Modifiers::none());
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:true"],
        "first click must check the box"
    );

    cx.simulate_click(point(px(8.), px(8.)), Modifiers::none());
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:true", "change:false"],
        "second click must uncheck it"
    );
}

#[gpui::test]
fn switch_keyboard_activates(cx: &mut TestAppContext) {
    let events: Events = Rc::new(RefCell::new(Vec::new()));
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        Switch::new("sw")
            .label("Wi-Fi")
            .on_change(move |checked, _, _| {
                events.borrow_mut().push(format!("change:{checked}"));
            })
            .into_any_element()
    });

    // Tab moves the focus from the harness root onto the track without firing
    // anything (a click would have toggled it once already). Space then
    // activates the focused track exactly once: gpui fires a focused element's
    // click listeners on Enter and Space, and a component that also bound its
    // own key handler would fire twice — the double-fire bug class this guards.
    //
    // The activation listens on key *up*, and `dispatch_keystroke` sends only
    // the down half, so the release is delivered explicitly.
    cx.simulate_keystrokes("tab space");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });

    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:true"],
        "one keystroke must toggle the switch exactly once"
    );
}

#[gpui::test]
fn select_click_selects_and_closes(cx: &mut TestAppContext) {
    let events: Events = Rc::new(RefCell::new(Vec::new()));
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let selection = events.clone();
        let opening = events.clone();
        Select::new(
            "sel",
            vec![PickerItem::new("rust", "Rust"), PickerItem::new("go", "Go")],
        )
        .on_selection_change(move |key, _, _| {
            let key = key
                .as_ref()
                .map_or_else(|| "none".to_owned(), ToString::to_string);
            selection.borrow_mut().push(format!("select:{key}"));
        })
        .on_open_change(move |open, _, _| opening.borrow_mut().push(format!("open:{open}")))
        .into_any_element()
    });

    // The trigger is a full-width row of `util::FIELD_HEIGHT` (36px) starting
    // at the origin; its centre is (60, 18).
    cx.simulate_click(point(px(60.), px(18.)), Modifiers::none());
    assert_eq!(recorded.borrow().as_slice(), ["open:true"]);

    // First option: `placed_field_panel` hangs the panel 6px below the trigger
    // (`top_full` + `mt(6px)` => y 42) with 6px vertical padding and
    // `min_h(FIELD_HEIGHT)` rows, so the row spans roughly y 48..84 and its
    // centre sits near y 66. The enter zoom animates only padding and radius,
    // never enough to move that band.
    cx.simulate_click(point(px(60.), px(66.)), Modifiers::none());
    assert_eq!(
        recorded.borrow().as_slice(),
        ["open:true", "open:false", "select:rust"],
        "clicking the first row must report the close and select its key"
    );

    // Closed proof by behaviour: the same spot is bare page below the trigger
    // now, so the press must reach nothing. Were the popover still open, the
    // row would record a second `select:rust` here. The callback proves it
    // too since the row reports its own dismissal, but the probe is what shows
    // the panel has actually left the tree.
    cx.simulate_click(point(px(60.), px(66.)), Modifiers::none());
    assert_eq!(
        recorded.borrow().as_slice(),
        ["open:true", "open:false", "select:rust"],
        "the popover must be closed after choosing an option"
    );
}
