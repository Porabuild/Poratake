//! Behaviour tests for the pickers' remaining props and the Drawer's
//! un-driven surface: `allowsEmptyCollection` on Autocomplete,
//! `allowsCustomValue` and `validate` on ComboBox, `shouldFocusWrap` /
//! `disabledKeys` on Select and Autocomplete, the clear affordance on
//! Autocomplete, and the Drawer's size.
//!
//! "The Drawer's size" needs a correction first, because v3 has no such prop:
//! `### Drawer.Content` documents only `placement` and `className`, and
//! `drawer.css` sizes the sheet by placement (`w-80 max-w-[85vw] sm:w-96` for
//! left/right, `max-h-[85vh]` for top/bottom). The size presets live on the
//! **Modal** (`Modal.Container size="xs"…"full"`), whose six values
//! `placement.rs` already drives. There is nothing in the port to test for a
//! Drawer size either (`drawer.rs` has `id`, `is_open`, `placement`, … and no
//! size), so this file pins the Drawer props that have never been driven
//! instead: the close trigger, `on_close`, `isKeyboardDismissDisabled`,
//! `isDismissible` and the footer slot.
//!
//! What is deliberately NOT duplicated here:
//!
//! - `combo_box_open.rs` drives `MenuTrigger::Manual` (typing does not open,
//!   the chevron does) along with the Focus and Input triggers.
//! - `pickers.rs` drives Autocomplete open/filter/select, escape, and the
//!   arrows-and-Enter pick; ComboBox chevron-open and click-select.
//! - `calendars_and_more.rs` drives Select typeahead, multiple mode and the
//!   section heading (for Select).
//! - `placement.rs` drives every Drawer placement and every ModalSize.
//!
//! Geometry is derived from the port's own constants, like the rest of the
//! suite:
//!
//! - Every trigger field is a 36px row (`util::FIELD_HEIGHT`) at the window
//!   origin, so its centre is (60, 18). Autocomplete panels are positioned
//!   from the measured trigger bounds with RAC's 8px popover gap: top = 44.
//!   ComboBox panels use the same measured 8px gap: top = 44. The Select
//!   panel uses the same 8px gap.
//! - Autocomplete: panel `pt(8)` + search wrapper `py(4)` + 36px field + list
//!   `p(6)` puts row *i* at y 102+36i; clicking y = 126+36i lands inside it in
//!   every phase of the entry zoom. A `section_before` heading rides above its
//!   item inside the same slot: `pt(6) pb(4)` around a 16px line, so for the
//!   slot starting at y S the heading occupies S..S+26 and the row
//!   S+26..S+62; the heading probe clicks
//!   S+14 and the option S+42.
//! - The Autocomplete clear button is the 20px (`size-5`) box in the trigger's
//!   flex row; the 320px trigger (`max_w(320)`) has `pr(28)`, so the button
//!   ends at x = 292 and centres at (282, 18), clear of the absolute chevron
//!   (`right(8)`, 16px, x 296..312).
//! - ComboBox: panel `p(4)` below the 8px gap puts row *i* at y 48+36i;
//!   clicking y = 64+36i covers it. The field caps at `max_w(320)`, so the
//!   chevron centre is (298, 18).
//! - Select: panel `py(6)` below the 8px gap puts option *i*'s centre at y
//!   68+36i.
//! - Drawer (window 1920x1080, 384px desktop side width): the Right panel is
//!   x 1536..1920, y 0..1080, `p-6` (24px). Inside it: the handle
//!   (bar 4px + `pb-2` 8px) at y 24..36, the 24px title line (the drag
//!   surface) at y 36..60, the close trigger (`absolute end-4 top-4` around
//!   the 24px CloseButton) centred at (1892, 28), the body at 24+12+24+8 = 68
//!   (probe centre (1580, 86)). The body is `flex_1`, so the footer stays at
//!   the bottom of the full-height side sheet; its 40x36 probe centres at
//!   (1876, 1038).
//!
//! The Drawer tests set reduced motion before the first frame (an entry slide
//! would sit at its t=0 off-window pose for the whole test otherwise) and
//! advance the clock past `EXITING_MS` before any closed-proof probe, exactly
//! as `placement.rs` does. The pickers need neither: their panels leave the
//! tree outright when closed.

use crate::harness;
use crate::source_scan;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    point, prelude::*, px, ElementId, Focusable, Modifiers, MouseButton, TestAppContext,
    VisualTestContext,
};
use harness::{click, events, open_host, press, probe as part_probe, Events};
use herogpui_components::{
    Autocomplete, Button, ComboBox, Drawer, DrawerCloseTrigger, DrawerHandle, DrawerPlacement,
    Form, FormData, Input, InputState, MenuTrigger, PickerItem, Select, SelectionMode,
};

/// An `InputState` entity for the search-field-backed controls, created before
/// the host opens so the test can keep its own handle to it.
fn search_state(cx: &mut TestAppContext) -> gpui::Entity<InputState> {
    cx.new(|cx| InputState::new(cx))
}

/// Items whose labels are unique, so the key can be the label itself. Tests
/// that need duplicate labels build explicit keys instead.
fn keyed(labels: &[&str]) -> Vec<PickerItem> {
    labels
        .iter()
        .map(|l| PickerItem::new(l.to_string(), l.to_string()))
        .collect()
}

/// A fixed-size pressable probe: records `label` on click. Every geometry
/// claim in the Drawer section is proven by placing one of these where the
/// component under test is computed to be and asserting its click records.
fn probe(id: impl Into<ElementId>, label: &'static str, recorded: Events) -> gpui::AnyElement {
    gpui::div()
        .id(id)
        .w(px(40.))
        .h(px(36.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_click(move |_, _, _| recorded.borrow_mut().push(label.to_owned()))
        .child(label)
        .into_any_element()
}

// -- Drawer helpers, as `placement.rs` uses them -----------------------------

/// Pins the layout by enabling reduced motion **before** the first frame:
/// without it the panel sits at its t=0 pose (fully off-window) for the whole
/// test, because entry animations run on the executor clock this suite
/// does not advance.
fn still() {
    harness::still();
}

/// Pushes the pending frame through. Mouse events hit-test the last rendered
/// frame, so a press whose effect the next event must see needs a redraw
/// first.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// Advances the test clock past `EXITING_MS` (100ms) plus slack and forces the
/// repaint the exit timer only scheduled. A closed-proof probe must not land
/// on the exiting, still-mounted panel.
fn let_exit_finish(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(300));
    flush_frame(cx);
}

/// One simulated drag: press at `from`, move to `to` with the button held,
/// release there. For the Drawer the press must land on the title row, where
/// the header's `on_mouse_down` writes the drag record.
fn drag(cx: &mut VisualTestContext, from: (f32, f32), to: (f32, f32)) {
    let modifiers = Modifiers::none();
    cx.simulate_mouse_down(point(px(from.0), px(from.1)), MouseButton::Left, modifiers);
    cx.simulate_mouse_move(point(px(to.0), px(to.1)), MouseButton::Left, modifiers);
    cx.simulate_mouse_up(point(px(to.0), px(to.1)), MouseButton::Left, modifiers);
}

// ---------------------------------------------------------------------------
// Autocomplete
// ---------------------------------------------------------------------------

#[gpui::test]
fn autocomplete_programmatic_focus_departure_closes_without_refocusing(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let search = search_state(cx);
    let next = search_state(cx);
    let search_for_view = search;
    let next_for_view = next.clone();

    let cx = open_host(cx, move || {
        let opens = opens.clone();
        gpui::div()
            .child(
                Autocomplete::new(search_for_view.clone(), keyed(&["Alpha", "Beta"]))
                    .default_open(true)
                    .on_open_change(move |open, _, _| {
                        opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    flush_frame(cx);
    cx.update(|window, cx| {
        let next = next.read(cx).focus_handle(cx);
        window.focus(&next, cx);
    });
    flush_frame(cx);

    assert_eq!(opened.borrow().as_slice(), ["open:false"]);
    assert!(cx.update(|window, cx| next.read(cx).focus_handle(cx).is_focused(window)));
}

/// `allowsEmptyCollection` keeps the popover mounted when a query matches
/// nothing: v3 documents it as *"Whether the autocomplete allows an empty
/// collection. When true, the autocomplete can function even with no items."*
/// and its example prose adds *"This is useful for scenarios where the list
/// might be empty initially or when all items are filtered out."* The empty
/// state has no handlers, so a press where a row would be records nothing and
/// does not close; only a press well outside the panel dismisses it.
#[gpui::test]
fn autocomplete_allows_empty_collection_keeps_the_panel_up_with_no_matches(
    cx: &mut TestAppContext,
) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Typst", "Rust", "Go"]))
            .allows_empty_collection(true)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // "zz" matches nothing. The empty state ("No results found") sits where
    // the rows would be; the press lands inside the panel, so it records
    // nothing and must not dismiss either.
    cx.simulate_input("zz");
    click(cx, 60., 124.);
    assert!(
        recorded.borrow().is_empty(),
        "the empty state must not select anything"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "a press inside the empty panel must not dismiss it"
    );

    // Only a press outside the mounted panel dismisses it — which proves the
    // panel was still up after the no-match query. Were the panel gone, this
    // outside press would reach nothing and record no dismissal.
    click(cx, 600., 300.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the empty panel must still answer its outside-press dismissal"
    );
    assert!(recorded.borrow().is_empty());
}

/// `allowsEmptyCollection` is **not** a close-on-filtered-empty flag. v3's
/// Autocomplete root is a React Aria `Select` and its filtering is a separate
/// layer (`Autocomplete.Filter` is RAC's `Autocomplete`), so a query that
/// prunes an open popover to zero changes nothing about the popover's open
/// state: the panel stays mounted and its composed "No results found" state
/// renders, with or without the prop. The prop only governs
/// whether a *collection with no items at all* may function at all.
#[gpui::test]
fn autocomplete_filtered_empty_without_the_prop_keeps_the_panel_mounted(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Typst", "Rust", "Go"]))
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // "zz" matches nothing. The empty state ("No results found") sits where
    // the rows would be; the press lands inside the still-mounted panel, so
    // it records nothing and must not dismiss it either.
    cx.simulate_input("zz");
    click(cx, 60., 124.);
    assert!(
        recorded.borrow().is_empty(),
        "the empty state must not select anything"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "a filtered-empty query must keep the popover mounted even without \
         the prop, so a press inside it cannot dismiss it"
    );

    // The panel is still mounted, so a press outside it dismisses it and
    // reports the close.
    click(cx, 600., 300.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the mounted empty panel must still answer its outside-press dismissal"
    );
    assert!(recorded.borrow().is_empty());
}

/// A collection with no items at all is a different gate, and this is what
/// `allowsEmptyCollection` is for. React Aria's `useSelectState.toggle` on
/// an empty collection early-returns ("Don't open if the collection is
/// empty."), so the trigger of a zero-item Autocomplete without the prop
/// neither opens nor reports `onOpenChange(true)` — and a second press must
/// not toggle either.
#[gpui::test]
fn autocomplete_zero_item_collection_without_the_prop_refuses_the_trigger(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, Vec::new())
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    // The trigger press is the `useSelectState.toggle()` act: with an empty
    // collection and no prop it must do nothing at all — no open, no report.
    click(cx, 60., 18.);
    assert!(
        opened.borrow().is_empty(),
        "a zero-item collection without allowsEmptyCollection must refuse \
         the trigger toggle and never report open"
    );

    // The refusal covers the toggle in both directions: a second press must
    // not open-and-close either (or report anything).
    click(cx, 60., 18.);
    assert!(
        opened.borrow().is_empty(),
        "a zero-item collection without allowsEmptyCollection must refuse \
         every trigger toggle"
    );
    assert!(recorded.borrow().is_empty());
}

/// v3 also uses `allowsEmptyCollection` for a collection that *starts* empty
/// ("the autocomplete can function even with no items"): with `items = []` the
/// trigger still opens, the empty state is still shown, and nothing can be
/// selected — the panel is dismissible by an outside press like any other.
#[gpui::test]
fn autocomplete_allows_empty_collection_with_no_items_opens_anyway(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, Vec::new())
            .allows_empty_collection(true)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "an empty collection must still open under the prop"
    );

    // The empty state occupies the popover; nothing there answers.
    click(cx, 60., 124.);
    assert!(recorded.borrow().is_empty());
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    click(cx, 600., 300.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the empty panel must still dismiss from an outside press"
    );
}

/// `disabledKeys` rows render but cannot be chosen: a pointer press on one
/// records nothing (the row has no handler and sits inside the panel, so the
/// press neither selects nor dismisses), and the arrows skip it — the cursor's
/// stops are the enabled indices, so Down from the first row lands on the
/// third.
#[gpui::test]
fn autocomplete_disabled_rows_are_unclickable_and_not_a_stop(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha", "Beta", "Gamma", "Delta"]))
            .disabled_keys(["Beta".into()])
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // The keyboard leg runs on a freshly opened panel: the popover's search
    // field takes the focus on the first frame, so Down lands on Alpha (0),
    // the next Down skips the disabled Beta and lands on Gamma (2), and Enter
    // takes the row the cursor is on. The flush renders the opened panel
    // before the keys arrive (mouse events hit-test the last rendered frame).
    flush_frame(cx);
    press(cx, "down");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Gamma"],
        "the arrows must skip the disabled row"
    );
    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);

    // Reopen for the pointer leg: flush the closed state so the next trigger
    // press is a genuine open (a press against the last drawn frame would
    // toggle that frame's `was_open` instead).
    flush_frame(cx);
    click(cx, 60., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true"]
    );
    flush_frame(cx);

    // Row 1 is Beta, y = 124 + 36. A press on it must record nothing and must
    // not dismiss the panel.
    click(cx, 60., 160.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Gamma"],
        "a disabled row must not answer the pointer"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true"],
        "a press on a disabled row must not dismiss the panel either"
    );
}

/// A `ListBox.Section` heading rides above the item it introduces and is not a
/// selectable thing of its own: a click on the heading records nothing and
/// keeps the panel open, and the arrows land on options — the heading never
/// consumes a stop, so three Downs from the top reach the item under it.
#[gpui::test]
fn autocomplete_section_heading_is_never_a_stop(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha", "Beta", "Gamma", "Delta"]))
            .section_before("Gamma", "Tropical")
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // Row 0 (Alpha) is a normal pick: y = 124.
    click(cx, 60., 124.);
    assert_eq!(recorded.borrow().as_slice(), ["Alpha"]);
    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);

    // The section's slot starts at y = 100 + 36*2 = 172: the heading spans
    // 172..198 (pt-6 + a 16px line + pb-4) and the option 36px below it.
    // A press at 172+14 = 186 hits the heading: no selection, no dismissal.
    click(cx, 60., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true"]
    );
    click(cx, 60., 186.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha"],
        "a section heading must not be selectable"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true"],
        "a press on the heading must not dismiss the panel"
    );

    // The option the heading announces is a normal pick: y = 172 + 42 = 214.
    click(cx, 60., 214.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha", "Gamma"],
        "the option under the heading must still be clickable"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true", "open:false"]
    );

    // Keyboard: the stops are the four item indices (the heading is a drawing,
    // not a row), so Down, Down, Down steps 0 -> 1 -> 2 and Enter takes the
    // item the heading introduces.
    click(cx, 60., 18.);
    press(cx, "down down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha", "Gamma", "Gamma"],
        "the arrows must step over the heading to the option under it"
    );
}

/// `max_items` caps the list after filtering. With five items and a cap of
/// two, the empty query shows exactly two rows: the third Down clamps on the
/// second stop instead of reaching the third item (an uncapped list would
/// land on "Beta"), and a query matching four items still caps to two — the
/// third Down clamps again, so "Alpine" comes out both times.
#[gpui::test]
fn autocomplete_max_items_caps_the_long_list(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(
            state,
            keyed(&["Alpha", "Alpine", "Beta", "Bravo", "Charlie"]),
        )
        .max_items(2)
        .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
        .into_any_element()
    });

    click(cx, 60., 18.);
    // Empty query: the first two items, so the stops are {0, 1}. The fourth
    // Down is the clamp: 0 -> 1 -> 1, and Enter reports "Alpine" — an
    // uncapped list would have stepped to "Beta".
    press(cx, "down down down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpine"],
        "down past the capped ends must clamp, not reach the third item"
    );

    // "a" matches Alpha, Alpine, Bravo and Charlie: filtered down to the same
    // two, so the keyboard behaves identically — the cap is applied after
    // filtering.
    click(cx, 60., 18.);
    cx.simulate_input("a");
    press(cx, "down down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpine", "Alpine"],
        "a filtering query must still cap to max_items rows"
    );
}

/// `on_clear` appears only when there is something to clear (v3's data-empty
/// state): with a seeded selection the 20px button at (282, 18) answers, and
/// after it fires the button is gone.
///
/// The clear button must not *also* open the popover. gpui dispatches a click
/// to the hit element and on up to its ancestors, and the clear button lives
/// *inside* the trigger, so its click would bubble into the trigger's own
/// `on_click`. v3 cannot do this: React Aria's trigger press is bound to
/// pointer events, so a bubbled DOM `click` is inert there. The button's own
/// handler stops that propagation.
#[gpui::test]
fn autocomplete_clear_button_fires_on_clear_and_vanishes(cx: &mut TestAppContext) {
    let clears = events();
    let cleared = clears.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let clears = clears.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha", "Beta", "Gamma"]))
            .default_value(["Alpha"])
            .on_clear(move |_, _| clears.borrow_mut().push("cleared".into()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    // The trigger holds the seeded selection, so the clear button is drawn at
    // x 292 - 10 (the 20px box ends at the 28px right padding of the 320px
    // trigger), centre (282, 18). A press there fires on_clear and must not
    // open anything.
    click(cx, 282., 18.);
    assert_eq!(
        cleared.borrow().as_slice(),
        ["cleared"],
        "the clear button must fire on_clear"
    );
    assert!(
        opened.borrow().is_empty(),
        "clearing is not an open gesture — the pressed clear button must not \
         bubble its click into the trigger"
    );
    flush_frame(cx);

    // The button is gone once nothing is selected, so a second press on the
    // same spot cannot clear again.
    click(cx, 282., 18.);
    assert_eq!(
        cleared.borrow().as_slice(),
        ["cleared"],
        "after the clear the button must be gone"
    );
}

#[gpui::test]
fn autocomplete_clear_button_works_without_an_on_clear_callback(cx: &mut TestAppContext) {
    let selections = events();
    let selected = selections.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha", "Beta"]))
            .default_value(["Alpha"])
            .on_selection_change_all(move |keys, _, _| {
                selections.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 282., 18.);
    assert_eq!(
        selected.borrow().as_slice(),
        [""],
        "ClearButton must report the empty selection even without on_clear"
    );
    assert!(
        opened.borrow().is_empty(),
        "clearing without on_clear must not bubble into the trigger"
    );
}

#[gpui::test]
fn autocomplete_controlled_clear_waits_for_the_owner(cx: &mut TestAppContext) {
    let selections = events();
    let selected = selections.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha", "Beta"]))
            .value(["Alpha"])
            .on_selection_change_all(move |keys, _, _| {
                selections.borrow_mut().push(keys.len().to_string());
            })
            .into_any_element()
    });

    click(cx, 282., 18.);
    flush_frame(cx);
    click(cx, 282., 18.);
    assert_eq!(
        selected.borrow().as_slice(),
        ["0", "0"],
        "controlled clear must report empty without mutating the owner's value"
    );
}

#[gpui::test]
fn autocomplete_clear_handles_a_controlled_key_missing_from_items(cx: &mut TestAppContext) {
    let selections = events();
    let selected = selections.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha", "Beta"]))
            .value(["loading-item"])
            .on_selection_change_all(move |keys, _, _| {
                selections.borrow_mut().push(keys.len().to_string());
            })
            .into_any_element()
    });

    click(cx, 282., 18.);
    assert_eq!(
        selected.borrow().as_slice(),
        ["0"],
        "a selected key remains clearable while its item is not loaded"
    );
}

#[gpui::test]
fn autocomplete_disabled_clear_is_inert(cx: &mut TestAppContext) {
    let selections = events();
    let selected = selections.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha"]))
            .default_value(["Alpha"])
            .is_disabled(true)
            .on_selection_change_all(move |_, _, _| {
                selections.borrow_mut().push("changed".into());
            })
            .into_any_element()
    });

    click(cx, 282., 18.);
    assert!(
        selected.borrow().is_empty(),
        "a disabled Autocomplete must not expose an active clear button"
    );
}

/// The clear drops the component's *owned* selection, proven through the
/// toggle in multiple mode: with {Alpha, Beta} seeded, picking Gamma reports
/// the three, and after the clear picks Alpha reports only {"Alpha"}. Were the
/// clear a no-op on the held set, the stored {Alpha, Beta, Gamma} would toggle
/// Alpha *off* and the report would be "Beta,Gamma".
#[gpui::test]
fn autocomplete_clear_empties_the_owned_selection_before_the_next_pick(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let clear_events = picks.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha", "Beta", "Gamma", "Delta"]))
            .selection_mode(SelectionMode::Multiple)
            .default_value(["Alpha", "Beta"])
            .on_clear(move |_, _| clear_events.borrow_mut().push("clear:cleared".into()))
            .on_selection_change_all(move |keys, _, _| {
                let joined = keys
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                picks.borrow_mut().push(joined);
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    // Row 2 (Gamma) joins the seeded pair: y = 124 + 2*36 = 196.
    click(cx, 60., 196.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha,Beta,Gamma"],
        "the pick must join the seeded selection"
    );

    // The clear button sits on the trigger, above the open panel, so it can
    // be pressed while the panel is up.
    click(cx, 282., 18.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha,Beta,Gamma", "", "clear:cleared"],
        "selection change must report before on_clear"
    );

    // Row 0 (Alpha) after the clear: {"Alpha"}. A stale held set would have
    // reported "Beta,Gamma".
    click(cx, 60., 124.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha,Beta,Gamma", "", "clear:cleared", "Alpha"],
        "the clear must empty the owned selection, so the next pick starts \
         from nothing"
    );
}

/// `shouldFocusWrap = false` holds at the ends: Down from the last row
/// clamps there (Enter then reports the last item, not a wrapped first), and
/// Up from the first row clamps at the top. The cursor persists across
/// reopenings, which is what makes the second leg deterministic.
#[gpui::test]
fn autocomplete_no_wrap_holds_at_both_ends(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha", "Beta", "Gamma"]))
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    // Four Downs from the top: 0 -> 1 -> 2 -> 2 (clamped). Enter reports the
    // last row — a wrapping list would have come back around to "Alpha".
    click(cx, 60., 18.);
    press(cx, "down down down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Gamma"],
        "down past the end must hold on the last row without wrap"
    );

    // Reopened, the cursor is still on Gamma (2); three Ups step 2 -> 1 -> 0
    // -> 0 (clamped) and Enter reports the first row.
    click(cx, 60., 18.);
    press(cx, "up up up");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Gamma", "Alpha"],
        "up past the start must hold on the first row without wrap"
    );
}

/// `shouldFocusWrap = true` joins the ends: Down from the last row wraps to
/// the first, and Up from the first wraps to the last.
#[gpui::test]
fn autocomplete_wrap_joins_both_ends(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha", "Beta", "Gamma"]))
            .should_focus_wrap(true)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    // Fourth Down wraps 2 -> 0; Enter reports "Alpha".
    click(cx, 60., 18.);
    press(cx, "down down down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha"],
        "down past the end must wrap to the first row"
    );

    // Reopened, the cursor is on Alpha (0); one Up wraps to Gamma (2).
    click(cx, 60., 18.);
    press(cx, "up");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha", "Gamma"],
        "up past the start must wrap to the last row"
    );
}

// -- Autocomplete keyed identity ----------------------------------------------
//
// Pinned v3.2.4 / React Aria Components 1.20.0 keep a stable `Key` separate
// from each item's `textValue`: the selection, `disabledKeys`, the callbacks
// and the form value address items by key, while filtering and the visible
// text use labels. The tests below use two items that share the label "Rust"
// with the distinct keys "rust" and "rusty" — under the old label-as-key
// identity every one of these expectations fails, because the duplicates
// aliased each other's selection, disabled state and row element id.

/// Two same-label items with distinct keys: the duplicate-labelled collection
/// the keyed-identity tests in this section drive.
fn duplicate_labels() -> Vec<PickerItem> {
    vec![
        PickerItem::new("rust", "Rust"),
        PickerItem::new("rusty", "Rust"),
        PickerItem::new("go", "Go"),
    ]
}

/// Picking the *second* of two same-label rows must report that row's own key
/// and render the label once — not alias onto the first item's key.
#[gpui::test]
fn autocomplete_duplicate_labels_pick_the_second_by_its_key(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let values = Rc::new(RefCell::new(Vec::new()));
    let seen_values = values.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let seen_values = seen_values.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, duplicate_labels())
            .value_content(move |v| {
                seen_values.borrow_mut().push(format!(
                    "text:{} keys:{} indices:{}",
                    v.selected_text,
                    v.selected_keys
                        .unwrap_or(&[])
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    v.selected_indices
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                ));
                v.default_children
            })
            .on_change(move |key, _, _| changes.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    // Row 1 is the second "Rust", key "rusty": y = 124 + 36.
    click(cx, 60., 18.);
    click(cx, 60., 160.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rusty"],
        "the second same-label row must report its own key, not the first \
         item's key"
    );
    flush_frame(cx);
    assert_eq!(
        values.borrow().last().map(String::as_str),
        Some("text:Rust keys:rusty indices:1"),
        "the trigger must render the selected key's label once, with the \
         selected key and collection index of the row that was picked"
    );

    // Reopening and picking the first "Rust" reports the *other* key — the
    // two rows are distinct selections.
    click(cx, 60., 18.);
    click(cx, 60., 124.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rusty", "rust"],
        "the first same-label row must keep its own key too"
    );
}

/// With both same-label items selected, the trigger renders each selected
/// item once in the selection's own order — two picks with one label are two
/// labels, and the render props see the keys behind them.
#[gpui::test]
fn autocomplete_duplicate_labels_selected_together_render_each_once(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let values = Rc::new(RefCell::new(Vec::new()));
    let seen_values = values.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let seen_values = seen_values.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, duplicate_labels())
            .selection_mode(SelectionMode::Multiple)
            .default_value(["rust", "rusty"])
            .value_content(move |v| {
                seen_values.borrow_mut().push(format!(
                    "text:{} keys:{} indices:{}",
                    v.selected_text,
                    v.selected_keys
                        .unwrap_or(&[])
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    v.selected_indices
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                ));
                v.default_children
            })
            .on_selection_change_all(move |keys, _, _| {
                changes.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    flush_frame(cx);
    assert_eq!(
        values.borrow().last().map(String::as_str),
        Some("text:Rust, Rust keys:rust,rusty indices:0,1"),
        "both same-label selections must render, once each, in the \
         selection's own order — the keys prove they are two items, not one"
    );

    // Adding "Go" keeps every selected key distinct from its label, and the
    // callback reports the selection set's insertion order: the seeded pair
    // first, "go" appended last — not the collection's or a sorted order.
    click(cx, 60., 18.);
    click(cx, 60., 196.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rust,rusty,go"],
        "the complete selection must report all three keys in insertion \
         order — a sorted set would have reported go,rust,rusty"
    );
    flush_frame(cx);
    assert_eq!(
        values.borrow().last().map(String::as_str),
        Some("text:Rust, Rust, Go keys:rust,rusty,go indices:0,1,2"),
        "the trigger text must stay in the selection's own order with one \
         render per selected key"
    );
}

/// `disabledKeys` addresses one key: disabling the first "Rust" leaves the
/// same-label sibling enabled and clickable, and as a keyboard stop.
#[gpui::test]
fn autocomplete_duplicate_labels_disable_one_key_keeps_the_sibling_enabled(
    cx: &mut TestAppContext,
) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, duplicate_labels())
            .disabled_keys(["rust".into()])
            .on_change(move |key, _, _| changes.borrow_mut().push(key.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // Keyboard leg on the freshly opened panel (a pointer press inside the
    // panel blurs the autofocused search field, so it always comes first):
    // Down enters the first *enabled* row — the sibling "rusty" at index 1 —
    // so Enter commits "rusty". Disabling both same-label items (the
    // aliasing bug) would have made "go" the first enabled stop instead.
    flush_frame(cx);
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rusty"],
        "disabling one key must leave its same-label sibling enabled and \
         selectable"
    );
    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);

    // Pointer leg on a reopened panel: a press on the disabled key's row
    // records nothing and leaves the panel up, while the same-label sibling
    // row next to it still answers.
    flush_frame(cx);
    click(cx, 60., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true"]
    );
    flush_frame(cx);
    click(cx, 60., 124.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rusty"],
        "the disabled key's row must not answer the pointer"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true"],
        "a press on a disabled row must not dismiss the panel either"
    );
    click(cx, 60., 160.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rusty", "rusty"],
        "the same-label sibling row must answer the pointer"
    );
}

/// Two same-label rows are two interactive elements in the plain list path:
/// in multiple mode each row toggles only its own key.
#[gpui::test]
fn autocomplete_duplicate_labels_keep_distinct_rows_in_the_plain_path(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, duplicate_labels())
            .selection_mode(SelectionMode::Multiple)
            .on_selection_change_all(move |keys, _, _| {
                changes.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    // Row 0 at y = 124, row 1 at y = 160. The second click must ADD "rusty"
    // beside "rust" — one aliased row id would have made it toggle the first
    // pick off.
    click(cx, 60., 18.);
    click(cx, 60., 124.);
    click(cx, 60., 160.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rust", "rust,rusty"],
        "the two same-label rows must be distinct clickable rows"
    );
    // A third click on the first row toggles only "rust" back off.
    click(cx, 60., 124.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rust", "rust,rusty", "rusty"],
        "each row must toggle only its own key"
    );
}

/// The virtual (`row_height`) list builds its rows through `uniform_list`,
/// but the identity contract is the same: two same-label rows are two
/// distinct interactive rows that each report their own key.
#[gpui::test]
fn autocomplete_duplicate_labels_keep_distinct_rows_in_the_virtual_path(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, duplicate_labels())
            .selection_mode(SelectionMode::Multiple)
            .row_height(px(36.))
            .on_selection_change_all(move |keys, _, _| {
                changes.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    // Same row geometry as the plain path: row 0 at y = 124, row 1 at
    // y = 160.
    click(cx, 60., 18.);
    click(cx, 60., 124.);
    click(cx, 60., 160.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rust", "rust,rusty"],
        "the two same-label rows must stay distinct rows under virtualization"
    );
    click(cx, 60., 124.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rust", "rust,rusty", "rusty"],
        "each virtual row must toggle only its own key"
    );
}

/// A held selection is a set of keys: reordering the collection must not move
/// it onto another item, and the trigger keeps rendering the held key's label
/// from wherever that item now sits.
#[gpui::test]
fn autocomplete_selection_survives_an_item_reorder_by_key(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let values = Rc::new(RefCell::new(Vec::new()));
    let seen_values = values.clone();
    let state = search_state(cx);
    let state_for_view = state;
    let items = Rc::new(RefCell::new(duplicate_labels()));
    let items_for_view = items.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let seen_values = seen_values.clone();
        let state = state_for_view.clone();
        let items = items_for_view.borrow().clone();
        Autocomplete::new(state, items)
            .default_value(["rusty"])
            .value_content(move |v| {
                seen_values.borrow_mut().push(format!(
                    "text:{} keys:{}",
                    v.selected_text,
                    v.selected_keys
                        .unwrap_or(&[])
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                ));
                v.default_children
            })
            .on_change(move |key, _, _| changes.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    flush_frame(cx);
    assert_eq!(
        values.borrow().last().map(String::as_str),
        Some("text:Rust keys:rusty"),
        "the seeded key must resolve to its item's label"
    );

    // Reorder the collection: "rusty" moves from index 1 to the end.
    *items.borrow_mut() = vec![
        PickerItem::new("go", "Go"),
        PickerItem::new("rust", "Rust"),
        PickerItem::new("rusty", "Rust"),
    ];
    flush_frame(cx);
    assert_eq!(
        values.borrow().last().map(String::as_str),
        Some("text:Rust keys:rusty"),
        "the held selection must follow its key through the reorder, not \
         the row the label used to sit on"
    );

    // Picking the row that now sits at index 1 commits that row's own key.
    click(cx, 60., 18.);
    click(cx, 60., 160.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rust"],
        "the row's key must be reported from its new position"
    );
    flush_frame(cx);
    assert_eq!(
        values.borrow().last().map(String::as_str),
        Some("text:Rust keys:rust"),
        "the trigger must show the newly committed key's label"
    );
}

// -- Autocomplete selection order ---------------------------------------------
//
// Pinned react-stately 3.49.0's `useSelectState` keeps `selectedKeys` as a
// JavaScript `Set`, which iterates in insertion order: `selectedItems`,
// `selectedText`, the `onChange` slice and the form value all follow the
// order keys were picked (or the owner listed) in — never the collection's
// order and never a sorted order.

/// Nonlexicographic keys whose collection order differs from any pick order:
/// a sorted set would report Alpha,Mike,Zulu; the pick order below never is.
fn misordered() -> Vec<PickerItem> {
    keyed(&["Zulu", "Alpha", "Mike"])
}

/// Multiple picks must report and render in insertion order, whichever way
/// each pick was made: the keyboard leg toggles "Mike" on first, then pointer
/// picks append "Zulu" and "Alpha", and toggling "Zulu" off removes it in
/// place — re-adding appends it at the end.
#[gpui::test]
fn autocomplete_multiple_picks_follow_insertion_order(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, misordered())
            .selection_mode(SelectionMode::Multiple)
            .on_selection_change_all(move |keys, _, _| {
                changes.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    // Keyboard leg: three Downs walk 0 -> 1 -> 2 and Enter toggles "Mike" on.
    click(cx, 60., 18.);
    press(cx, "down down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Mike"],
        "the keyboard pick must report the toggled key"
    );

    // Pointer legs: "Zulu" then "Alpha" append in pick order.
    click(cx, 60., 124.);
    click(cx, 60., 160.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Mike", "Mike,Zulu", "Mike,Zulu,Alpha"],
        "pointer picks must append in pick order — a sorted set would have \
         reported Alpha,Mike,Zulu"
    );

    // Toggling "Zulu" off removes it in place — "Alpha" keeps its position
    // behind "Mike" — and toggling it back on appends it at the end.
    click(cx, 60., 124.);
    click(cx, 60., 124.);
    assert_eq!(
        recorded.borrow().as_slice(),
        [
            "Mike",
            "Mike,Zulu",
            "Mike,Zulu,Alpha",
            "Mike,Alpha",
            "Mike,Alpha,Zulu"
        ],
        "remove-in-place must keep the remaining order and re-adding must \
         append at the end"
    );
}

/// A controlled selection's order is the owner's order: the render props see
/// the keys exactly as `value` listed them, and a pick appends to that order
/// in the reported slice.
#[gpui::test]
fn autocomplete_controlled_value_keeps_the_owner_order(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let values = Rc::new(RefCell::new(Vec::new()));
    let seen_values = values.clone();
    let current = Rc::new(RefCell::new(vec![
        gpui::SharedString::from("k-mid"),
        gpui::SharedString::from("k-last"),
    ]));
    let state = search_state(cx);
    let state_for_view = state;
    let current_for_view = current;
    let items = vec![
        PickerItem::new("k-last", "One"),
        PickerItem::new("k-mid", "Two"),
        PickerItem::new("k-first", "Three"),
    ];

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let seen_values = seen_values.clone();
        let current = current_for_view.clone();
        let state = state_for_view.clone();
        let selected = current.borrow().clone();
        Autocomplete::new(state, items.clone())
            .selection_mode(SelectionMode::Multiple)
            .value(selected)
            .value_content(move |v| {
                seen_values.borrow_mut().push(format!(
                    "text:{} keys:{} indices:{}",
                    v.selected_text,
                    v.selected_keys
                        .unwrap_or(&[])
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    v.selected_indices
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                ));
                v.default_children
            })
            .on_selection_change_all(move |keys, _, _| {
                *current.borrow_mut() = keys.to_vec();
                changes.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    flush_frame(cx);
    assert_eq!(
        values.borrow().last().map(String::as_str),
        Some("text:Two, One keys:k-mid,k-last indices:1,0"),
        "the controlled render must follow the owner's listed order — keys, \
         resolved items and collection indices alike — not the collection's \
         or a sorted order"
    );

    // Picking "Three" appends to the owner's order; the owner accepts the
    // reported slice, so the next frame renders it back.
    click(cx, 60., 18.);
    click(cx, 60., 196.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["k-mid,k-last,k-first"],
        "the reported slice must append the pick to the owner's order"
    );
    flush_frame(cx);
    assert_eq!(
        values.borrow().last().map(String::as_str),
        Some("text:Two, One, Three keys:k-mid,k-last,k-first indices:1,0,2"),
        "the trigger render props must agree with the reported slice"
    );
}

/// The uncontrolled default order persists, and reordering the collection
/// must not reorder the selected keys' history: only the indices re-resolve
/// to wherever each key's item now sits.
#[gpui::test]
fn autocomplete_default_order_persists_through_a_collection_reorder(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let values = Rc::new(RefCell::new(Vec::new()));
    let seen_values = values.clone();
    let state = search_state(cx);
    let state_for_view = state;
    let items = Rc::new(RefCell::new(duplicate_labels()));
    let items_for_view = items.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let seen_values = seen_values.clone();
        let state = state_for_view.clone();
        let items = items_for_view.borrow().clone();
        Autocomplete::new(state, items)
            .selection_mode(SelectionMode::Multiple)
            .default_value(["rusty", "rust"])
            .value_content(move |v| {
                seen_values.borrow_mut().push(format!(
                    "text:{} keys:{} indices:{}",
                    v.selected_text,
                    v.selected_keys
                        .unwrap_or(&[])
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    v.selected_indices
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                ));
                v.default_children
            })
            .on_selection_change_all(move |keys, _, _| {
                changes.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    flush_frame(cx);
    assert_eq!(
        values.borrow().last().map(String::as_str),
        Some("text:Rust, Rust keys:rusty,rust indices:1,0"),
        "the default's listed order must persist — a sorted set would have \
         reported rust,rusty"
    );

    // Reorder the collection: "go" moves to the front. The key history must
    // not move; only the indices re-resolve.
    *items.borrow_mut() = vec![
        PickerItem::new("go", "Go"),
        PickerItem::new("rust", "Rust"),
        PickerItem::new("rusty", "Rust"),
    ];
    flush_frame(cx);
    assert_eq!(
        values.borrow().last().map(String::as_str),
        Some("text:Rust, Rust keys:rusty,rust indices:2,1"),
        "a collection reorder must not reorder the selected keys' history"
    );

    // The next pick appends to the preserved history, wherever its row sits.
    click(cx, 60., 18.);
    click(cx, 60., 124.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rusty,rust,go"],
        "the pick must append to the preserved selection order"
    );
}

/// The form value follows the same order: `FormData::get_all` must return the
/// picked keys in pick order, one value per key.
#[gpui::test]
fn autocomplete_form_get_all_follows_the_selection_order(cx: &mut TestAppContext) {
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let submitted_for_form = submitted.clone();
    let state = search_state(cx);
    let items = misordered();
    let field = Autocomplete::new(state.clone(), items.clone())
        .selection_mode(SelectionMode::Multiple)
        .name("lang")
        .form_field()
        .expect("named Autocomplete");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form.borrow_mut().push(data.get_all("lang"));
    });
    let submit = form.submit_handler();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        Autocomplete::new(state_for_view.clone(), items.clone())
            .selection_mode(SelectionMode::Multiple)
            .name("lang")
            .into_any_element()
    });

    click(cx, 60., 18.);
    click(cx, 60., 124.);
    click(cx, 60., 160.);
    flush_frame(cx);
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        [vec![
            gpui::SharedString::from("Zulu"),
            gpui::SharedString::from("Alpha")
        ]],
        "FormData::get_all must return the picked keys in pick order — a \
         sorted set would have returned Alpha,Zulu"
    );
}

/// Filtering matches labels, never keys: a query naming a key matches
/// nothing, while the same-label duplicates both answer a label query and
/// remain individually addressable.
#[gpui::test]
fn autocomplete_filter_matches_labels_not_keys(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    // Opaque keys that appear in no label: anything a "k" query reached would
    // be filtering on keys.
    let items = vec![
        PickerItem::new("k1", "Rust"),
        PickerItem::new("k2", "Rust"),
        PickerItem::new("k3", "Go"),
    ];

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        let items = items.clone();
        Autocomplete::new(state, items)
            .on_change(move |key, _, _| changes.borrow_mut().push(key.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // Typing "k2" — an existing item's key — must match nothing: the query
    // only ever reaches labels. The empty state sits where the rows would
    // be, so a press there records nothing and dismisses nothing.
    cx.simulate_input("k2");
    click(cx, 60., 124.);
    assert!(
        recorded.borrow().is_empty(),
        "a key-shaped query must not match any row: filtering runs on labels"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the filtered-empty panel must stay mounted"
    );

    // Reopen (that press blurred the search field, so the fresh popover
    // refocuses it) and filter by label instead: clear the key-shaped query,
    // type "rus", which matches both same-label duplicates, and pick the
    // second one — still its own row.
    click(cx, 60., 18.);
    click(cx, 60., 18.);
    flush_frame(cx);
    press(cx, "backspace backspace");
    cx.simulate_input("rus");
    click(cx, 60., 160.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["k2"],
        "a label query must surface both duplicates, each pickable by key"
    );
}

/// A closed idle frame draws no rows and must not call the filter at all;
/// opening, a query edit and a collection edit each re-run it for what
/// changed; the same query over the same collection runs it again on every
/// consuming frame, because an uncacheable closure is never assumed stable.
#[gpui::test]
fn autocomplete_filter_runs_only_while_the_panel_consumes_matches(cx: &mut TestAppContext) {
    let count = Rc::new(Cell::new(0));
    let count_for_view = count.clone();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_for_view = seen.clone();
    let state = search_state(cx);
    let state_for_view = state;
    // The collection lives behind a cell so the test can replace it between
    // frames, the way a caller rebuilding its builder each frame would.
    let collection = Rc::new(RefCell::new(keyed(&["Alpha", "Beta"])));
    let collection_for_view = collection.clone();

    let cx = open_host(cx, move || {
        let count = count_for_view.clone();
        let seen = seen_for_view.clone();
        let collection = collection_for_view.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, collection.borrow().clone())
            .filter(move |label, query| {
                count.set(count.get() + 1);
                seen.borrow_mut().push(format!("{query}·{label}"));
                label.to_lowercase().contains(&query.to_lowercase())
            })
            .into_any_element()
    });

    for _ in 0..3 {
        flush_frame(cx);
    }
    assert_eq!(
        count.get(),
        0,
        "a closed idle frame must not call the filter"
    );

    click(cx, 60., 18.);
    flush_frame(cx);
    let opened = count.get();
    assert!(
        opened >= 2,
        "the open frame must filter the whole collection"
    );
    assert!(
        seen.borrow()
            .iter()
            .take(2)
            .all(|call| call.starts_with('·')),
        "the open frame filters the unfiltered collection: {:?}",
        seen.borrow()
    );

    // The custom closure's configuration cannot join a cache key, so the
    // unchanged open frame re-runs it rather than assuming it stable.
    flush_frame(cx);
    assert!(
        count.get() > opened,
        "an unchanged open frame re-runs the uncacheable custom filter"
    );

    // A query change re-runs the filter for the new query.
    cx.simulate_input("al");
    flush_frame(cx);
    assert!(
        seen.borrow().iter().any(|call| call == "al·Alpha"),
        "the query edit must re-run the filter with the new query: {:?}",
        seen.borrow()
    );

    // A collection change re-runs the filter even though the query is
    // unchanged, and the new item is the one it reaches.
    collection
        .borrow_mut()
        .push(PickerItem::new("Gamma", "Gamma"));
    flush_frame(cx);
    assert!(
        seen.borrow().iter().any(|call| call == "al·Gamma"),
        "a collection change must re-run the filter over the new items: {:?}",
        seen.borrow()
    );
}

/// The ComboBox gates the same work, and skips one frame more: the
/// Focus-trigger open consumes nothing on its own frame (the panel has not
/// mounted yet) and the settled frame shows the full collection, which owns
/// the frame exactly as the pinned component skips `defaultFilter` while
/// showing everything. The full collection's rows are proven at runtime —
/// both rows are picked while the filter call count stays zero. The first
/// filter calls come from the first edit.
#[gpui::test]
fn combo_box_filter_skips_idle_frames_and_the_full_collection(cx: &mut TestAppContext) {
    let count = Rc::new(Cell::new(0));
    let count_for_view = count.clone();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_for_view = seen.clone();
    let picks = events();
    let picked = picks.clone();
    let state = search_state(cx);
    let state_for_view = state;
    let collection = Rc::new(RefCell::new(keyed(&["Alpha", "Beta"])));
    let collection_for_view = collection.clone();

    let cx = open_host(cx, move || {
        let count = count_for_view.clone();
        let seen = seen_for_view.clone();
        let picks = picks.clone();
        let collection = collection_for_view.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, collection.borrow().clone())
            .on_change(move |item, _, _| picks.borrow_mut().push(item.to_string()))
            .filter(move |label, query| {
                count.set(count.get() + 1);
                seen.borrow_mut().push(format!("{query}·{label}"));
                label.to_lowercase().contains(&query.to_lowercase())
            })
            .into_any_element()
    });

    for _ in 0..3 {
        flush_frame(cx);
    }
    assert_eq!(
        count.get(),
        0,
        "a closed idle frame must not call the filter"
    );

    // Focusing the field opens the panel (the Focus trigger): the opening
    // frame is gated off and the settled frame shows the full collection, so
    // no filter call happens anywhere in the open.
    click(cx, 60., 18.);
    flush_frame(cx);
    assert_eq!(
        count.get(),
        0,
        "the open and its settled full-collection frame must not call the filter"
    );

    // The full collection must actually be on screen while the filter is
    // skipped: interact with both rows, not just the last keyboard stop.
    // ComboBox row *i* centres at y = 64+36i.
    click(cx, 60., 64.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha"],
        "the first full-collection row must be pickable"
    );
    assert_eq!(
        count.get(),
        0,
        "picking the first full-collection row must never call the filter"
    );

    // The pick closed the panel. The chevron (x = 298) reopens the full
    // collection without depending on the Focus one-shot.
    flush_frame(cx);
    click(cx, 298., 18.);
    flush_frame(cx);
    assert_eq!(
        count.get(),
        0,
        "reopening the full collection must never call the filter"
    );
    click(cx, 60., 100.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Beta"],
        "the second full-collection row must be pickable after reopen"
    );
    assert_eq!(
        count.get(),
        0,
        "picking both full-collection rows must never call the filter"
    );

    // Pointer picks move focus onto the row; put it back on the field before
    // the query phase. The second pick left "Beta" in the field.
    flush_frame(cx);
    click(cx, 60., 18.);
    press(cx, "backspace backspace backspace backspace");

    // Editing switches the panel to filtered rows, which the filter runs for,
    // once per consuming frame and once per edit scan.
    cx.simulate_input("al");
    flush_frame(cx);
    let queried = count.get();
    assert!(
        seen.borrow().iter().any(|call| call == "al·Alpha"),
        "the query edit must re-run the filter with the new query: {:?}",
        seen.borrow()
    );

    // Replacing the collection re-runs the filter over the new items even
    // though the query is unchanged.
    collection
        .borrow_mut()
        .push(PickerItem::new("Gamma", "Gamma"));
    flush_frame(cx);
    assert!(count.get() > queried);
    assert!(
        seen.borrow().iter().any(|call| call == "al·Gamma"),
        "a collection change must re-run the filter over the new items: {:?}",
        seen.borrow()
    );
}

fn form_entry(data: &FormData, name: &str) -> String {
    data.get(name)
        .map_or_else(|| "omitted".to_owned(), |value| value.as_text().to_string())
}

/// Named Autocomplete must submit the live keyed selection, not the snapshot
/// `form_field()` saw when the Form was first told about the control.
#[gpui::test]
fn autocomplete_form_submits_live_uncontrolled_selection(cx: &mut TestAppContext) {
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let state = search_state(cx);
    let items = keyed(&["Alpha", "Beta", "Gamma"]);
    let field = Autocomplete::new(state.clone(), items.clone())
        .name("lang")
        .default_value(["Alpha"])
        .form_field()
        .expect("named Autocomplete");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(form_entry(data, "lang"));
    });
    let submit = form.submit_handler();
    let state_for_rebuild = state.clone();
    let items_for_rebuild = items.clone();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        Autocomplete::new(state_for_view.clone(), items.clone())
            .name("lang")
            .default_value(["Alpha"])
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["Alpha"]);

    click(cx, 60., 18.);
    click(cx, 60., 160.);
    flush_frame(cx);
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        ["Alpha", "Beta"],
        "FormData must follow the runtime selection after a pick"
    );
    cx.update(|_, cx| {
        let rebuilt = Autocomplete::new(state_for_rebuild.clone(), items_for_rebuild.clone())
            .name("lang")
            .default_value(["Alpha"]);
        let form = Form::new().field(rebuilt.form_field().expect("rebuilt named Autocomplete"));
        assert_eq!(
            form_entry(&form.data(cx), "lang"),
            "Beta",
            "rebuilding form_field must not overwrite a live pick with defaultValue"
        );
    });
}

/// A controlled Autocomplete reports the pick but keeps submitting the owner's
/// value until that owner writes it back through `value`.
#[gpui::test]
fn autocomplete_form_waits_for_controlled_owner_acceptance(cx: &mut TestAppContext) {
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let changes = events();
    let recorded = changes.clone();
    let current = Rc::new(RefCell::new(vec![gpui::SharedString::from("Alpha")]));
    let state = search_state(cx);
    let items = keyed(&["Alpha", "Beta", "Gamma"]);
    let selected = current.borrow().clone();
    let field = Autocomplete::new(state.clone(), items.clone())
        .name("lang")
        .value(selected)
        .form_field()
        .expect("named Autocomplete");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(form_entry(data, "lang"));
    });
    let submit = form.submit_handler();
    let state_for_view = state;
    let current_for_view = current;
    let cx = open_host(cx, move || {
        let selected = current_for_view.borrow().clone();
        let current = current_for_view.clone();
        let changes = changes.clone();
        Autocomplete::new(state_for_view.clone(), items.clone())
            .name("lang")
            .value(selected)
            .on_selection_change_all(move |keys, _, _| {
                changes.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                *current.borrow_mut() = keys.to_vec();
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    click(cx, 60., 160.);
    flush_frame(cx);
    assert_eq!(recorded.borrow().as_slice(), ["Beta"]);
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        ["Beta"],
        "once the owner accepts the pick, FormData must follow that value"
    );
}

/// Controlled form data stays on the owner's value when the owner ignores the
/// change callback — the pick must not write through on its own.
#[gpui::test]
fn autocomplete_form_keeps_owner_value_when_controlled_pick_is_ignored(cx: &mut TestAppContext) {
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let changes = events();
    let recorded = changes.clone();
    let current = Rc::new(RefCell::new(vec![gpui::SharedString::from("Alpha")]));
    let state = search_state(cx);
    let items = keyed(&["Alpha", "Beta", "Gamma"]);
    let selected = current.borrow().clone();
    let field = Autocomplete::new(state.clone(), items.clone())
        .name("lang")
        .value(selected)
        .form_field()
        .expect("named Autocomplete");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(form_entry(data, "lang"));
    });
    let submit = form.submit_handler();
    let state_for_view = state;
    let current_for_view = current;
    let cx = open_host(cx, move || {
        let selected = current_for_view.borrow().clone();
        let changes = changes.clone();
        Autocomplete::new(state_for_view.clone(), items.clone())
            .name("lang")
            .value(selected)
            .on_selection_change_all(move |keys, _, _| {
                changes.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    click(cx, 60., 160.);
    flush_frame(cx);
    assert_eq!(recorded.borrow().as_slice(), ["Beta"]);
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        ["Alpha"],
        "a controlled pick that the owner does not accept must not change FormData"
    );
}

#[gpui::test]
fn autocomplete_disabled_form_field_is_omitted(cx: &mut TestAppContext) {
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let state = search_state(cx);
    let items = keyed(&["Alpha", "Beta"]);
    let field = Autocomplete::new(state.clone(), items.clone())
        .name("lang")
        .default_value(["Alpha"])
        .is_disabled(true)
        .form_field()
        .expect("named Autocomplete");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(form_entry(data, "lang"));
    });
    let submit = form.submit_handler();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        Autocomplete::new(state_for_view.clone(), items.clone())
            .name("lang")
            .default_value(["Alpha"])
            .is_disabled(true)
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["omitted"]);
}

#[gpui::test]
fn autocomplete_disabled_form_field_becomes_successful_after_rerender(cx: &mut TestAppContext) {
    let disabled = Rc::new(Cell::new(true));
    let disabled_for_view = disabled.clone();
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let state = search_state(cx);
    let items = keyed(&["Alpha", "Beta"]);
    let field = Autocomplete::new(state.clone(), items.clone())
        .name("lang")
        .default_value(["Alpha"])
        .form_field()
        .expect("named Autocomplete");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(form_entry(data, "lang"));
    });
    let submit = form.submit_handler();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        Autocomplete::new(state_for_view.clone(), items.clone())
            .name("lang")
            .default_value(["Alpha"])
            .is_disabled(disabled_for_view.get())
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["omitted"]);

    disabled.set(false);
    flush_frame(cx);
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["omitted", "Alpha"]);
}

#[gpui::test]
fn autocomplete_read_only_form_field_remains_successful(cx: &mut TestAppContext) {
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let state = search_state(cx);
    let items = keyed(&["Alpha", "Beta"]);
    let field = Autocomplete::new(state.clone(), items.clone())
        .name("lang")
        .default_value(["Alpha"])
        .is_read_only(true)
        .form_field()
        .expect("named Autocomplete");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(form_entry(data, "lang"));
    });
    let submit = form.submit_handler();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        Autocomplete::new(state_for_view.clone(), items.clone())
            .name("lang")
            .default_value(["Alpha"])
            .is_read_only(true)
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), ["Alpha"]);
}

#[gpui::test]
fn autocomplete_form_reset_restores_uncontrolled_default(cx: &mut TestAppContext) {
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let state = search_state(cx);
    let items = keyed(&["Alpha", "Beta", "Gamma"]);
    let field = Autocomplete::new(state.clone(), items.clone())
        .name("lang")
        .default_value(["Alpha"])
        .form_field()
        .expect("named Autocomplete");
    let form = Form::new().field(field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(form_entry(data, "lang"));
    });
    let submit = form.submit_handler();
    let reset = form.reset_handler();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        Autocomplete::new(state_for_view.clone(), items.clone())
            .name("lang")
            .default_value(["Alpha"])
            .into_any_element()
    });

    click(cx, 60., 18.);
    click(cx, 60., 160.);
    flush_frame(cx);
    cx.update(|window, cx| submit(window, cx));
    cx.update(|window, cx| reset(window, cx));
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        ["Beta", "Alpha"],
        "native reset must restore defaultValue in FormData without waiting for a repaint"
    );
}

#[gpui::test]
fn autocomplete_form_reset_reports_controlled_default_to_owner(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let picks = events();
    let recorded_picks = picks.clone();
    let current = Rc::new(RefCell::new(vec![gpui::SharedString::from("Beta")]));
    let state = search_state(cx);
    let items = keyed(&["Alpha", "Beta", "Gamma"]);
    let selected = current.borrow().clone();
    let field = Autocomplete::new(state.clone(), items.clone())
        .name("lang")
        .value(selected)
        .default_value(["Alpha"])
        .form_field()
        .expect("named Autocomplete");
    let form = Form::new().field(field);
    let reset = form.reset_handler();
    let state_for_view = state;
    let current_for_view = current;
    let cx = open_host(cx, move || {
        let selected = current_for_view.borrow().clone();
        let current = current_for_view.clone();
        let changes = changes.clone();
        let picks = picks.clone();
        Autocomplete::new(state_for_view.clone(), items.clone())
            .name("lang")
            .value(selected)
            .default_value(["Alpha"])
            .on_selection_change_all(move |keys, _, _| {
                *current.borrow_mut() = keys.to_vec();
                changes.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .on_change(move |key, _, _| picks.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    cx.update(|window, cx| reset(window, cx));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha"],
        "controlled reset must report defaultValue so the owner can update"
    );
    assert!(
        recorded_picks.borrow().is_empty(),
        "form reset must not invoke Autocomplete's pick-only scalar callback"
    );
}

#[gpui::test]
fn autocomplete_form_reset_reports_disabled_controlled_default_to_owner(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let current = Rc::new(RefCell::new(vec![gpui::SharedString::from("Beta")]));
    let state = search_state(cx);
    let items = keyed(&["Alpha", "Beta"]);
    let selected = current.borrow().clone();
    let field = Autocomplete::new(state.clone(), items.clone())
        .name("lang")
        .value(selected)
        .default_value(["Alpha"])
        .is_disabled(true)
        .form_field()
        .expect("named Autocomplete");
    let form = Form::new().field(field);
    let reset = form.reset_handler();
    let state_for_view = state;
    let current_for_view = current;
    let cx = open_host(cx, move || {
        let selected = current_for_view.borrow().clone();
        let current = current_for_view.clone();
        let changes = changes.clone();
        Autocomplete::new(state_for_view.clone(), items.clone())
            .name("lang")
            .value(selected)
            .default_value(["Alpha"])
            .is_disabled(true)
            .on_selection_change_all(move |keys, _, _| {
                *current.borrow_mut() = keys.to_vec();
                changes.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    cx.update(|window, cx| reset(window, cx));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha"],
        "disabled controlled Autocomplete still notifies its owner on reset"
    );
}

// ---------------------------------------------------------------------------
// ComboBox
// ---------------------------------------------------------------------------

#[gpui::test]
fn combo_box_blur_restores_selected_text_closes_and_keeps_destination_focus(
    cx: &mut TestAppContext,
) {
    let callbacks = events();
    let recorded = callbacks.clone();
    let combo = search_state(cx);
    let next = search_state(cx);
    let combo_for_view = combo.clone();
    let next_for_view = next.clone();

    let cx = open_host(cx, move || {
        let inputs = callbacks.clone();
        let opens = callbacks.clone();
        gpui::div()
            .child(
                ComboBox::new(combo_for_view.clone(), keyed(&["Alpha", "Beta"]))
                    .default_value(["Alpha"])
                    .default_input_value("mismatch")
                    .default_open(true)
                    .menu_trigger(MenuTrigger::Manual)
                    .on_input_change(move |value, _, _| {
                        inputs.borrow_mut().push(format!("input:{value}"));
                    })
                    .on_open_change(move |open, _, _| {
                        opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    cx.update(|window, cx| window.focus(&combo.read(cx).focus_handle(cx), cx));
    flush_frame(cx);
    cx.update(|window, cx| window.focus(&next.read(cx).focus_handle(cx), cx));
    flush_frame(cx);

    assert_eq!(
        combo.read_with(cx, |state, _| state.value().to_owned()),
        "Alpha"
    );
    assert_eq!(recorded.borrow().as_slice(), ["input:Alpha", "open:false"]);
    assert!(cx.update(|window, cx| next.read(cx).focus_handle(cx).is_focused(window)));
}

#[gpui::test]
fn combo_box_custom_single_blur_keeps_text_and_clears_selection(cx: &mut TestAppContext) {
    let selections = events();
    let selected = selections.clone();
    let inputs = events();
    let input_changes = inputs.clone();
    let opens = events();
    let opened = opens.clone();
    let combo = search_state(cx);
    let next = search_state(cx);
    let combo_for_view = combo.clone();
    let next_for_view = next.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let inputs = inputs.clone();
        let opens = opens.clone();
        gpui::div()
            .child(
                ComboBox::new(combo_for_view.clone(), keyed(&["Alpha", "Beta"]))
                    .default_value(["Alpha"])
                    .default_input_value("Custom")
                    .default_open(true)
                    .allows_custom_value(true)
                    .menu_trigger(MenuTrigger::Manual)
                    .on_selection_change_all(move |keys, _, _| {
                        selections.borrow_mut().push(keys.join(","));
                    })
                    .on_input_change(move |value, _, _| inputs.borrow_mut().push(value.to_owned()))
                    .on_open_change(move |open, _, _| {
                        opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    cx.update(|window, cx| window.focus(&combo.read(cx).focus_handle(cx), cx));
    flush_frame(cx);
    cx.update(|window, cx| window.focus(&next.read(cx).focus_handle(cx), cx));
    flush_frame(cx);

    assert_eq!(
        combo.read_with(cx, |state, _| state.value().to_owned()),
        "Custom"
    );
    assert_eq!(selected.borrow().as_slice(), [""]);
    assert!(input_changes.borrow().is_empty());
    assert_eq!(opened.borrow().as_slice(), ["open:false"]);
}

#[gpui::test]
fn combo_box_custom_multiple_blur_preserves_query_and_selection(cx: &mut TestAppContext) {
    let selections = events();
    let selected = selections.clone();
    let inputs = events();
    let input_changes = inputs.clone();
    let opens = events();
    let opened = opens.clone();
    let combo = search_state(cx);
    let next = search_state(cx);
    let combo_for_view = combo.clone();
    let next_for_view = next.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let inputs = inputs.clone();
        let opens = opens.clone();
        gpui::div()
            .child(
                ComboBox::new(combo_for_view.clone(), keyed(&["Alpha", "Beta"]))
                    .selection_mode(SelectionMode::Multiple)
                    .default_value(["Alpha"])
                    .default_input_value("Custom")
                    .default_open(true)
                    .allows_custom_value(true)
                    .menu_trigger(MenuTrigger::Manual)
                    .on_selection_change_all(move |keys, _, _| {
                        selections.borrow_mut().push(keys.join(","));
                    })
                    .on_input_change(move |value, _, _| inputs.borrow_mut().push(value.to_owned()))
                    .on_open_change(move |open, _, _| {
                        opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    cx.update(|window, cx| window.focus(&combo.read(cx).focus_handle(cx), cx));
    flush_frame(cx);
    cx.update(|window, cx| window.focus(&next.read(cx).focus_handle(cx), cx));
    flush_frame(cx);

    assert_eq!(
        combo.read_with(cx, |state, _| state.value().to_owned()),
        "Custom"
    );
    assert!(selected.borrow().is_empty());
    assert!(input_changes.borrow().is_empty());
    assert_eq!(opened.borrow().as_slice(), ["open:false"]);
}

#[gpui::test]
fn combo_box_noncustom_multiple_blur_clears_only_the_query(cx: &mut TestAppContext) {
    let selections = events();
    let selected = selections.clone();
    let inputs = events();
    let input_changes = inputs.clone();
    let combo = search_state(cx);
    let next = search_state(cx);
    let combo_for_view = combo.clone();
    let next_for_view = next.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let inputs = inputs.clone();
        gpui::div()
            .child(
                ComboBox::new(combo_for_view.clone(), keyed(&["Alpha", "Beta"]))
                    .selection_mode(SelectionMode::Multiple)
                    .default_value(["Alpha"])
                    .default_input_value("query")
                    .menu_trigger(MenuTrigger::Manual)
                    .on_selection_change_all(move |keys, _, _| {
                        selections.borrow_mut().push(keys.join(","));
                    })
                    .on_input_change(move |value, _, _| inputs.borrow_mut().push(value.to_owned())),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    cx.update(|window, cx| window.focus(&combo.read(cx).focus_handle(cx), cx));
    flush_frame(cx);
    cx.update(|window, cx| window.focus(&next.read(cx).focus_handle(cx), cx));
    flush_frame(cx);

    assert_eq!(combo.read_with(cx, |state, _| state.value().to_owned()), "");
    assert!(selected.borrow().is_empty());
    assert_eq!(input_changes.borrow().as_slice(), [""]);
}

#[gpui::test]
fn combo_box_click_away_commits_and_closes_exactly_once(cx: &mut TestAppContext) {
    let inputs = events();
    let input_changes = inputs.clone();
    let opens = events();
    let opened = opens.clone();
    let combo = search_state(cx);
    let next = search_state(cx);
    let combo_for_view = combo.clone();
    let next_for_view = next;

    let cx = open_host(cx, move || {
        let inputs = inputs.clone();
        let opens = opens.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(300.))
            .child(
                ComboBox::new(combo_for_view.clone(), keyed(&["Alpha", "Beta"]))
                    .default_value(["Alpha"])
                    .default_input_value("mismatch")
                    .default_open(true)
                    .menu_trigger(MenuTrigger::Manual)
                    .on_input_change(move |value, _, _| inputs.borrow_mut().push(value.to_owned()))
                    .on_open_change(move |open, _, _| {
                        opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    cx.update(|window, cx| window.focus(&combo.read(cx).focus_handle(cx), cx));
    flush_frame(cx);
    click(cx, 60., 354.);
    flush_frame(cx);

    assert_eq!(
        combo.read_with(cx, |state, _| state.value().to_owned()),
        "Alpha"
    );
    assert_eq!(input_changes.borrow().as_slice(), ["Alpha"]);
    assert_eq!(opened.borrow().as_slice(), ["open:false"]);
}

#[gpui::test]
fn controlled_combo_box_click_away_reports_one_close(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let combo = search_state(cx);
    let next = search_state(cx);
    let combo_for_view = combo.clone();
    let next_for_view = next;

    let cx = open_host(cx, move || {
        let opens = opens.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(300.))
            .child(
                ComboBox::new(combo_for_view.clone(), keyed(&["Alpha", "Beta"]))
                    .selected_keys(["Alpha".into()])
                    .default_input_value("mismatch")
                    .is_open(true)
                    .menu_trigger(MenuTrigger::Manual)
                    .on_open_change(move |open, _, _| {
                        opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    cx.update(|window, cx| window.focus(&combo.read(cx).focus_handle(cx), cx));
    flush_frame(cx);
    click(cx, 60., 354.);
    flush_frame(cx);
    flush_frame(cx);

    assert_eq!(opened.borrow().as_slice(), ["open:false"]);
}

#[gpui::test]
fn combo_box_tab_commits_the_highlight_then_moves_focus_on(cx: &mut TestAppContext) {
    let selections = events();
    let selected = selections.clone();
    let opens = events();
    let opened = opens.clone();
    let combo = search_state(cx);
    let next = search_state(cx);
    let combo_for_view = combo.clone();
    let next_for_view = next.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let opens = opens.clone();
        gpui::div()
            .child(
                ComboBox::new(combo_for_view.clone(), keyed(&["Alpha", "Beta"]))
                    .default_open(true)
                    .menu_trigger(MenuTrigger::Manual)
                    .on_change(move |value, _, _| selections.borrow_mut().push(value.to_string()))
                    .on_open_change(move |open, _, _| {
                        opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    cx.update(|window, cx| window.focus(&combo.read(cx).focus_handle(cx), cx));
    flush_frame(cx);
    press(cx, "down");
    press(cx, "tab");
    flush_frame(cx);

    assert_eq!(selected.borrow().as_slice(), ["Alpha"]);
    assert_eq!(
        combo.read_with(cx, |state, _| state.value().to_owned()),
        "Alpha"
    );
    assert_eq!(opened.borrow().as_slice(), ["open:false"]);
    assert!(cx.update(|window, cx| next.read(cx).focus_handle(cx).is_focused(window)));
}

#[gpui::test]
fn combo_box_multiple_tab_adds_the_highlight_then_moves_focus_on(cx: &mut TestAppContext) {
    let selections = events();
    let selected = selections.clone();
    let opens = events();
    let opened = opens.clone();
    let combo = search_state(cx);
    let next = search_state(cx);
    let combo_for_view = combo.clone();
    let next_for_view = next.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let opens = opens.clone();
        gpui::div()
            .child(
                ComboBox::new(combo_for_view.clone(), keyed(&["Alpha", "Beta"]))
                    .selection_mode(SelectionMode::Multiple)
                    .default_open(true)
                    .menu_trigger(MenuTrigger::Manual)
                    .on_selection_change_all(move |keys, _, _| {
                        selections.borrow_mut().push(keys.join(","));
                    })
                    .on_open_change(move |open, _, _| {
                        opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    cx.update(|window, cx| window.focus(&combo.read(cx).focus_handle(cx), cx));
    flush_frame(cx);
    press(cx, "down");
    press(cx, "tab");
    flush_frame(cx);

    assert_eq!(selected.borrow().as_slice(), ["Alpha"]);
    assert_eq!(combo.read_with(cx, |state, _| state.value().to_owned()), "");
    assert_eq!(opened.borrow().as_slice(), ["open:false"]);
    assert!(cx.update(|window, cx| next.read(cx).focus_handle(cx).is_focused(window)));
}

/// `allowsEmptyCollection` keeps the panel mounted when filtering removes
/// every row. The unmatched text remains in the field, the empty-state region
/// is inert, and an outside press still dismisses the panel.
#[gpui::test]
fn combo_box_allows_empty_collection_keeps_the_list_up_with_the_empty_state(
    cx: &mut TestAppContext,
) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .allows_empty_collection(true)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    // The default Focus trigger opens the list; allowsEmptyCollection keeps
    // it mounted when the edit filters every row away.
    cx.simulate_input("pl");
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "pl",
        "the unmatched text must be accepted into the field"
    );

    // Nothing matches "pl", but the list stays up: the press where a row
    // would be records nothing (the empty state has no handlers) and must not
    // dismiss the panel.
    click(cx, 60., 64.);
    assert!(recorded.borrow().is_empty());
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // The panel is still mounted, so an outside press dismisses it.
    click(cx, 600., 300.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the empty list must still answer its outside-press dismissal"
    );
}

/// v3 inherits React Aria for the ComboBox (its Accessibility section links
/// the RAC docs and lists "Support for custom values"). Pinned react-stately
/// 3.49.0's `commitCustomValue` keeps the typed text and sets the selected
/// key to `null` — it does not select the text. With no selection to change,
/// `onSelectionChange` fires nothing; the slice callback spells the `null`
/// when one did.
#[gpui::test]
fn combo_box_allows_custom_value_enter_commits_the_text(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let slices = events();
    let sliced = slices.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let slices = slices.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .allows_custom_value(true)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_selection_change_all(move |keys, _, _| {
                slices.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("pl");
    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);

    // The closed field still commits the custom value on Enter: the text
    // stays, but no key is selected, so the single-key callback — which
    // cannot spell `null` — and the slice callback both stay silent.
    press(cx, "enter");
    assert!(
        recorded.borrow().is_empty(),
        "a committed custom value carries a null selected key, so the \
         single-key callback must not report the typed text: {:?}",
        recorded.borrow().as_slice()
    );
    assert!(
        sliced.borrow().is_empty(),
        "with no selection to change, the null must not be reported"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the closed custom-value commit must not report another close"
    );
}

#[gpui::test]
fn combo_box_empty_filtered_collection_closes_and_reports(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Typst", "Rust", "Go"]))
            .menu_trigger(MenuTrigger::Input)
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("t");
    cx.simulate_input("z");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "an empty filtered collection must close the list and report the transition"
    );
}

/// `validate` is a function the *component* runs — that is the whole point of
/// the prop, and the half that is behaviour. The message it returns is drawn
/// by the field (appearance, the audits' business); what a caller can observe
/// is that the validator is invoked with the current value on every edit and
/// that the field stays fully interactive under it: typing still filters,
/// rows still open and pick. The validator itself records what it was asked.
#[gpui::test]
fn combo_box_validate_runs_on_every_edit_and_leaves_the_field_interactive(cx: &mut TestAppContext) {
    let seen = events();
    let asked = seen.clone();
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state.clone();

    let cx = open_host(cx, move || {
        let seen = seen.clone();
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Alpha", "Beta", "Gamma"]))
            .validate(move |value| {
                seen.borrow_mut().push(value.to_owned());
                if value.is_empty() {
                    None
                } else {
                    (value.chars().count() < 3).then(|| "Too short".into())
                }
            })
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    // `validate` runs during the render after an edit, so each keystroke is
    // flushed before the record is read. "a" then "l" keep the query matching
    // Alpha (a query that matched nothing would close the list).
    cx.simulate_input("a");
    flush_frame(cx);
    assert!(
        asked.borrow().contains(&"a".to_owned()),
        "the first edit must run the validator with its value"
    );

    cx.simulate_input("l");
    flush_frame(cx);
    assert!(
        asked.borrow().contains(&"al".to_owned()),
        "the second edit must re-run the validator with the whole value"
    );

    // The field never locked up under validation: typing continues to filter
    // (the list is up) and the keyboard still walks and picks the matching
    // row. (The pointer is deliberately not used here: a failing `validate`
    // draws its message under the field, which pushes the list down from its
    // no-message position, and the keyboard path is independent of that
    // geometry — the point of this test is that the field answers at all.)
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha"],
        "the field must stay fully interactive under a failing validator"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the pick must close the list as usual"
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "Alpha",
        "the pick must still write the chosen item into the field"
    );
}

/// The caret must survive the list opening. Under an explicit Input trigger
/// the first keystroke opens the list; the proof reads the caret from the
/// state entity the test owns via the selection anchor — pressing shift+Left
/// after "typ" selects the "p" (anchor 3, cursor 2) only if the caret is
/// where the typing left it. A list that slurped or reset the caret would
/// report a different range (or none). The keyboard path then still works, so
/// Down + Enter pick the only matching row.
#[gpui::test]
fn combo_box_caret_stays_at_the_end_when_the_list_opens(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = search_state(cx);
    let state_for_view = state.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .menu_trigger(MenuTrigger::Input)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    // Focus alone does not open an explicit Input trigger.
    assert!(opened.borrow().is_empty());

    cx.simulate_input("typ");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the first non-empty edit must open the list"
    );

    // Caret at the end of "typ" is char 3; shift+Left anchors at 3 and moves
    // the cursor to 2, so the state reports the selected "p" (2, 3).
    press(cx, "shift-left");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selection()),
        Some((2, 3)),
        "the caret must still sit at the end after the list opened — a reset \
         caret would select from position 0"
    );

    // The keyboard still reaches the list through the focused field: Down
    // highlights the only row matching "typ", Enter takes it.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Typst"],
        "the pick through the focused field must still work"
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "Typst",
        "the pick must write the chosen item into the field"
    );
    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);
}

// ---------------------------------------------------------------------------
// Select
// ---------------------------------------------------------------------------

#[gpui::test]
fn select_full_width_fills_its_host_and_keeps_the_trigger_clickable(cx: &mut TestAppContext) {
    for width in [200., 400., 640.] {
        for labeled in [false, true] {
            for full_width in [false, true] {
                let opened = events();
                let recorded = opened.clone();
                let measured = Rc::new(Cell::new(None));
                let observed = measured.clone();
                let cx = open_host(cx, move || {
                    let opened = opened.clone();
                    let measured = measured.clone();
                    let mut select = Select::new("full-width", keyed(&["Alpha"]))
                        .full_width(full_width)
                        .on_open_change(move |open, _, _| {
                            opened.borrow_mut().push(open.to_string());
                        });
                    if labeled {
                        select = select.label("Language");
                    }
                    gpui::div()
                        .w(px(width))
                        .child(select)
                        .on_children_prepainted(move |bounds, _, _| measured.set(Some(bounds[0])))
                        .into_any_element()
                });
                let bounds = observed.get().unwrap();
                let expected = if full_width { width } else { width.min(320.) };
                assert_eq!(
                    bounds.size.width,
                    px(expected),
                    "width={width}, labeled={labeled}, full_width={full_width}"
                );
                click(cx, expected - 12., f32::from(bounds.bottom()) - 18.);
                assert_eq!(recorded.borrow().as_slice(), ["true"]);
            }
        }
    }
}

/// v3's `Select.Value` render props include `isPlaceholder` ("Whether the
/// value is a placeholder"), and the port hands it over. Recording what the
/// closure is told each frame pins the placeholder ↔ value flip without
/// reading a pixel: the placeholder state until a pick, the chosen text
/// afterwards, and still after a reopening (the selection sticks).
#[gpui::test]
fn select_value_render_props_flip_placeholder_with_a_pick(cx: &mut TestAppContext) {
    let flags = Rc::new(RefCell::new(Vec::<(bool, String)>::new()));
    let seen_flags = flags.clone();
    let picked = events();
    let changes = picked.clone();

    let cx = open_host(cx, move || {
        let flags = flags.clone();
        let changes = changes.clone();
        Select::new("sel-ph", keyed(&["Alpha", "Beta", "Gamma"]))
            .value_content(move |value| {
                flags
                    .borrow_mut()
                    .push((value.is_placeholder, value.selected_text.to_owned()));
                value.default_children
            })
            .on_change(move |key, _, _| changes.borrow_mut().push(format!("{key:?}")))
            .into_any_element()
    });

    // The first render shows the placeholder state.
    assert_eq!(
        seen_flags.borrow().first(),
        Some(&(true, String::new())),
        "nothing selected must render the placeholder state"
    );

    // Pick row 2 (Gamma), centre y = 66 + 2*36 = 138.
    eprintln!(
        "DEBUG before {:?} {:?} {:?}",
        cx.debug_bounds("select-list-Name(\"sel-ph\")-opt-0"),
        cx.debug_bounds("select-list-Name(\"sel-ph\")-opt-1"),
        cx.debug_bounds("select-list-Name(\"sel-ph\")-opt-2")
    );
    click(cx, 60., 18.);
    eprintln!(
        "DEBUG after {:?} {:?} {:?}",
        cx.debug_bounds("select-list-Name(\"sel-ph\")-opt-0"),
        cx.debug_bounds("select-list-Name(\"sel-ph\")-opt-1"),
        cx.debug_bounds("select-list-Name(\"sel-ph\")-opt-2")
    );
    click(cx, 60., 138.);
    assert_eq!(picked.borrow().as_slice(), ["Some(\"Gamma\")"]);
    assert_eq!(
        seen_flags.borrow().last(),
        Some(&(false, "Gamma".to_owned())),
        "a pick must flip the value out of the placeholder state"
    );
    assert!(
        seen_flags.borrow().iter().any(|(_, text)| text == "Gamma"),
        "the chosen text must reach the value render props"
    );

    // Reopening renders the same chosen state: the uncontrolled selection
    // sticks across close and open.
    click(cx, 60., 18.);
    assert_eq!(
        seen_flags.borrow().last(),
        Some(&(false, "Gamma".to_owned())),
        "the selection must persist into a later render"
    );
}

/// `disabledKeys` options render but cannot be chosen: a pointer press on one
/// records nothing and leaves the panel open, and the arrows skip it — Down
/// from the first option lands on the third.
#[gpui::test]
fn select_disabled_rows_are_unclickable_and_not_a_stop(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        Select::new("sel-dis", keyed(&["Alpha", "Beta", "Gamma", "Delta"]))
            .disabled_keys(["Beta".into()])
            .on_change(move |key, _, _| changes.borrow_mut().push(format!("{key:?}")))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // The keyboard leg runs on a freshly opened panel: the trigger holds the
    // focus (its mouse-down claims it), so Down lands on Alpha (0), the next
    // Down skips the disabled Beta and lands on Gamma (2), and Enter takes
    // the highlighted option — closing through the trigger's own click
    // listener, which gpui fires on the same keystroke. The flush renders
    // the opened panel before the keys arrive.
    flush_frame(cx);
    press(cx, "down");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Some(\"Gamma\")"],
        "the arrows must skip the disabled option"
    );
    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);

    // Reopen for the pointer leg: flush the closed state so the next trigger
    // press is a genuine open.
    flush_frame(cx);
    click(cx, 60., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true"]
    );
    flush_frame(cx);

    // Row 1 (Beta) centres at y = 66 + 36 = 102. A press on it must record
    // nothing and must not dismiss the panel.
    click(cx, 60., 102.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Some(\"Gamma\")"],
        "a disabled option must not answer the pointer"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true"],
        "a press on a disabled option must not dismiss the panel"
    );
}

#[gpui::test]
fn select_keyboard_pick_does_not_reopen_when_owner_closes_on_change(cx: &mut TestAppContext) {
    for key in ["enter", "space"] {
        let open = Rc::new(Cell::new(false));
        let changes = events();
        let opens = events();
        let view_open = open.clone();
        let view_changes = changes.clone();
        let view_opens = opens.clone();
        let cx = open_host(cx, move || {
            let selection_open = view_open.clone();
            let trigger_open = view_open.clone();
            let changes = view_changes.clone();
            let opens = view_opens.clone();
            Select::new("sel-owner-close", keyed(&["Alpha", "Beta"]))
                .is_open(view_open.get())
                .on_change(move |value, window, _| {
                    changes.borrow_mut().push(format!("{value:?}"));
                    selection_open.set(false);
                    window.refresh();
                })
                .on_open_change(move |value, window, _| {
                    opens.borrow_mut().push(format!("{value}"));
                    trigger_open.set(*value);
                    window.refresh();
                })
                .into_any_element()
        });
        press(cx, "tab");
        // The keyboard open seats the cursor on the first row, Alpha; the
        // dispatched key then picks it without an extra Down.
        press(cx, key);
        let result = cx.update(|window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse(key).unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                }),
                cx,
            )
        });
        assert!(
            !result.propagate,
            "{key} selection must consume the browser default"
        );
        flush_frame(cx);
        let repeated = cx.update(|window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse(key).unwrap(),
                    is_held: true,
                    prefer_character_input: false,
                }),
                cx,
            )
        });
        assert!(
            !repeated.propagate,
            "held {key} must not synthesize another browser press"
        );
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
        assert_eq!(changes.borrow().as_slice(), ["Some(\"Alpha\")"], "{key}");
        assert!(
            !open.get(),
            "{key} must not reopen the owner's closed popup"
        );
        assert_eq!(opens.borrow().as_slice(), ["true"], "{key}");
        flush_frame(cx);
        click(cx, 60., 102.);
        assert_eq!(
            changes.borrow().as_slice(),
            ["Some(\"Alpha\")"],
            "{key} must leave the former Beta option inert"
        );
    }
}

/// A select whose whole collection is disabled opens but answers nothing: the
/// rows have no handlers and the arrow-key resolver has no stops, so neither
/// path can choose anything, and nothing is ever reported.
///
/// The trigger of an *open* select still closes it. v3's trigger is one press
/// that simply closes the list — React Aria's press handling never both
/// dismisses a popover and presses the element it belongs to, and its Button
/// press is pointer-bound, so nothing reopens it. The port's trigger press is
/// marked as "not outside" for the panel's outside-press dismissal, so the
/// trigger's own click owns the close and reports it exactly once.
#[gpui::test]
fn select_fully_disabled_collection_answers_nothing(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        Select::new("sel-all-off", keyed(&["Alpha", "Beta"]))
            .disabled_keys(["Alpha".into(), "Beta".into()])
            .on_change(move |key, _, _| changes.borrow_mut().push(format!("{key:?}")))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    // The trigger still opens: being disabled is a property of the options.
    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);
    flush_frame(cx);

    // Both rows are unclickable: y = 66 and 102.
    click(cx, 60., 66.);
    click(cx, 60., 102.);
    assert!(recorded.borrow().is_empty());

    // ..and the keyboard has no stops to walk (the trigger holds the focus).
    press(cx, "down");
    press(cx, "up");
    assert!(recorded.borrow().is_empty());

    // The trigger still closes; nothing was ever selected. (Enter is left out
    // of the keyboard leg on purpose: gpui fires a focused trigger's click on
    // Enter, which would toggle the panel on its own; this click is the
    // deterministic close.)
    click(cx, 60., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "a fully disabled collection must still close on its trigger"
    );
    assert!(
        recorded.borrow().is_empty(),
        "no interaction with an all-disabled collection may select anything"
    );
}

/// `shouldFocusWrap = false` holds at the ends: the fourth Down clamps on the
/// last option (Enter reports it, not a wrapped first) and three Ups from a
/// reopened cursor clamp on the first.
#[gpui::test]
fn select_no_wrap_holds_at_both_ends(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Select::new("sel-wrap-off", keyed(&["Alpha", "Beta", "Gamma"]))
            .on_change(move |key, _, _| changes.borrow_mut().push(format!("{key:?}")))
            .into_any_element()
    });

    // The first Down after opening starts the walk; 0 -> 1 -> 2 -> 2 (clamp).
    click(cx, 60., 18.);
    press(cx, "down down down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Some(\"Gamma\")"],
        "down past the end must hold on the last option without wrap"
    );

    // The cursor persisted on Gamma (2); 2 -> 1 -> 0 -> 0 (clamp).
    click(cx, 60., 18.);
    press(cx, "up up up");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Some(\"Gamma\")", "Some(\"Alpha\")"],
        "up past the start must hold on the first option without wrap"
    );
}

/// `shouldFocusWrap = true` joins the ends: Down from the last wraps to the
/// first, Up from the first wraps to the last.
#[gpui::test]
fn select_wrap_joins_both_ends(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Select::new("sel-wrap-on", keyed(&["Alpha", "Beta", "Gamma"]))
            .should_focus_wrap(true)
            .on_change(move |key, _, _| changes.borrow_mut().push(format!("{key:?}")))
            .into_any_element()
    });

    // Fourth Down wraps 2 -> 0.
    click(cx, 60., 18.);
    press(cx, "down down down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Some(\"Alpha\")"],
        "down past the end must wrap to the first option"
    );

    // The cursor is on Alpha (0); one Up wraps to Gamma (2).
    click(cx, 60., 18.);
    press(cx, "up");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Some(\"Alpha\")", "Some(\"Gamma\")"],
        "up past the start must wrap to the last option"
    );
}

/// Pinned React Aria `usePopover`: an open Select's list also closes when the
/// *focus* leaves it, and the keystroke that moved the focus is not swallowed.
/// Tab from the trigger lands on the next control while the list dismisses:
/// `onOpenChange(false)` reports exactly once, the rows are gone, and the next
/// input owns the focus. That direct probe catches a focus-stealing closer.
#[gpui::test]
fn select_tab_to_the_next_control_closes_and_keeps_the_focus_moving(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let next = search_state(cx);
    let next_for_view = next.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opened_open = opens.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(160.))
            .child(
                // Two options put Alpha's row centre at y 66 and Beta's at
                // y 102, both clear of the Input the gap parks below (y 196+).
                Select::new("sel-blur-out", keyed(&["Alpha", "Beta"]))
                    .on_change(move |key, _, _| changes.borrow_mut().push(format!("{key:?}")))
                    .on_open_change(move |open, _, _| {
                        opened_open.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    // Open by pointer, as every test above does: the mouse-down claims the
    // trigger's focus, so the focus starts inside the surface that closes on
    // its loss. The flush renders the armed, open state before any key lands.
    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);
    flush_frame(cx);

    press(cx, "tab");
    // One frame for the focus change to fire the blur closer...
    flush_frame(cx);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "Tabbing off an open select must report onOpenChange(false) exactly once"
    );
    // ...and one more for the panel to leave the tree outright, so the
    // closed-proof clicks hit-test the frame without it.
    flush_frame(cx);
    assert!(
        cx.update(|window, cx| next.read(cx).focus_handle(cx).is_focused(window)),
        "Tab must leave focus on the next control rather than return it to the Select"
    );

    // Where the rows were: two clicks must find nothing there any more.
    click(cx, 60., 66.);
    assert!(
        recorded.borrow().is_empty(),
        "a click where Alpha was must find no panel after the blur close"
    );
    click(cx, 60., 102.);
    assert!(recorded.borrow().is_empty());
}

// ---------------------------------------------------------------------------
// Drawer
// ---------------------------------------------------------------------------
//
// v3's Drawer has no size prop (see the header); what has never been driven
// is the rest of the Drawer's surface. Geometry (window 1920x1080, 384px
// desktop side width, `p-6` = 24px): the Right panel is x 1536..1920; the close trigger
// (`absolute end-4 top-4` around the 24px CloseButton) centres at (1892, 28)
// — x = 1920 - 16 - 12, y = 16 + 12; the title row (the drag surface) spans y
// 36..60; the body probe centre (1580, 86) comes from 24px padding + 12px
// handle + 24px title + `mt-2` 8px + 18px half-probe. The body is `flex_1`,
// so the footer stays at the bottom; its 40x36 probe centres at (1876, 1038).

/// Every dismissal path reports through `on_close` *and* `onOpenChange`, and
/// the composed default `DrawerCloseTrigger` is a path of its own. The
/// pointer press on the close trigger and the Escape key each fire both
/// callbacks exactly once; after the exit the sheet is gone, so the probe
/// spot records nothing.
#[gpui::test]
fn drawer_close_button_reports_on_close_and_open_change(cx: &mut TestAppContext) {
    still();
    let closes = events();
    let closed = closes.clone();
    let opens = events();
    let opened = opens.clone();
    let probed = events();
    let hits = probed.clone();
    let open = Rc::new(RefCell::new(true));
    let open_flag = open.clone();

    let cx = open_host(cx, move || {
        let closes = closes.clone();
        let opens = opens.clone();
        let hits = hits.clone();
        let is_open = *open_flag.borrow();
        Drawer::new()
            .id("pd-drawer-close")
            .is_open(is_open)
            .placement(DrawerPlacement::Right)
            .title("Drag me shut")
            .child(DrawerHandle::new())
            .child(DrawerCloseTrigger::new())
            .child(probe("pd-drawer-close-probe", "hit", hits))
            .on_close(move |reason, _, _| closes.borrow_mut().push(format!("{reason:?}")))
            .on_open_change({
                let open_flag = open_flag.clone();
                move |v, window, _| {
                    *open_flag.borrow_mut() = *v;
                    opens.borrow_mut().push(format!("open:{v}"));
                    window.refresh();
                }
            })
            .into_any_element()
    });

    // The close trigger answers the pointer and reports through both
    // callbacks exactly once.
    click(cx, 1892., 28.);
    assert_eq!(
        closed.borrow().as_slice(),
        ["CloseButton"],
        "the close button must fire on_close with DismissReason::CloseButton"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false"],
        "the close button must report the dismissal through onOpenChange"
    );
    let_exit_finish(cx);
    click(cx, 1580., 86.);
    assert!(
        probed.borrow().is_empty(),
        "the sheet must be gone after the exit"
    );

    // Escape takes the same shared path.
    *open.borrow_mut() = true;
    flush_frame(cx);
    press(cx, "escape");
    assert_eq!(
        closed.borrow().as_slice(),
        ["CloseButton", "Escape"],
        "escape must fire on_close too, with DismissReason::Escape"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false", "open:false"],
        "escape must report the dismissal through onOpenChange"
    );

    // A press on the dimmed region outside the sheet is the backdrop path.
    let_exit_finish(cx);
    *open.borrow_mut() = true;
    flush_frame(cx);
    click(cx, 100., 100.);
    assert_eq!(
        closed.borrow().as_slice(),
        ["CloseButton", "Escape", "Backdrop"],
        "an outside press must fire on_close with DismissReason::Backdrop"
    );
}

/// `isKeyboardDismissDisabled` silences Escape but nothing else: the
/// composed default `DrawerCloseTrigger` still dismisses, so the
/// keyboard-disabled drawer is not a stuck one.
#[gpui::test]
fn drawer_keyboard_dismiss_disabled_silences_escape_but_not_the_button(cx: &mut TestAppContext) {
    still();
    let opens = events();
    let opened = opens.clone();
    let open = Rc::new(RefCell::new(true));
    let open_flag = open;

    let cx = open_host(cx, move || {
        let opens = opens.clone();
        let is_open = *open_flag.borrow();
        Drawer::new()
            .id("pd-drawer-kbd")
            .is_open(is_open)
            .placement(DrawerPlacement::Right)
            .title("Drag me shut")
            .child(DrawerCloseTrigger::new())
            .is_keyboard_dismiss_disabled(true)
            .on_open_change({
                let open_flag = open_flag.clone();
                move |v, window, _| {
                    *open_flag.borrow_mut() = *v;
                    opens.borrow_mut().push(format!("open:{v}"));
                    window.refresh();
                }
            })
            .into_any_element()
    });

    press(cx, "escape");
    assert!(
        opened.borrow().is_empty(),
        "isKeyboardDismissDisabled must silence Escape"
    );

    click(cx, 1892., 28.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false"],
        "the close button must still dismiss a keyboard-disabled drawer"
    );
}

/// An omitted `DrawerCloseTrigger` leaves the `absolute end-4 top-4` spot as
/// bare panel padding — v3 has no `hideCloseButton` and no automatic
/// stand-in — so a press there records nothing, while the keyboard dismissal
/// stays intact.
#[gpui::test]
fn drawer_omitted_close_trigger_keeps_escape_dismissal(cx: &mut TestAppContext) {
    still();
    let opens = events();
    let opened = opens.clone();
    let open = Rc::new(RefCell::new(true));
    let open_flag = open;

    let cx = open_host(cx, move || {
        let opens = opens.clone();
        let is_open = *open_flag.borrow();
        Drawer::new()
            .id("pd-drawer-no-close")
            .is_open(is_open)
            .placement(DrawerPlacement::Right)
            .title("Drag me shut")
            .on_open_change({
                let open_flag = open_flag.clone();
                move |v, window, _| {
                    *open_flag.borrow_mut() = *v;
                    opens.borrow_mut().push(format!("open:{v}"));
                    window.refresh();
                }
            })
            .into_any_element()
    });

    // No trigger: the press lands on the panel's own padding, which is inside
    // the panel, so nothing records (the panel's outside-press dismissal only
    // fires outside it).
    click(cx, 1892., 28.);
    assert!(
        opened.borrow().is_empty(),
        "an omitted close trigger must leave its spot inert"
    );

    press(cx, "escape");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false"],
        "escape must still dismiss a drawer without a close trigger"
    );
}

/// A `DrawerCloseTrigger` with custom children still renders v3's wired
/// `CloseButton` — the children only replace its glyph — so pressing the
/// composed content reports the drawer's own `on_open_change(false)`, exactly
/// like the bare part.
#[gpui::test]
fn drawer_custom_close_trigger_children_replace_only_the_glyph(cx: &mut TestAppContext) {
    still();
    let opens = events();
    let opened = opens.clone();
    let open = Rc::new(RefCell::new(true));
    let open_flag = open;

    let cx = open_host(cx, move || {
        let opened = opens.clone();
        let is_open = *open_flag.borrow();
        // The owner records the close but deliberately never flips `is_open`,
        // so the drawer stays up for the keyboard half below.
        Drawer::new()
            .id("pd-drawer-custom-x")
            .is_open(is_open)
            .placement(DrawerPlacement::Right)
            .title("Drag me shut")
            .child(
                DrawerCloseTrigger::new().child(
                    gpui::div()
                        .size(px(12.))
                        .rounded(px(6.))
                        .bg(gpui::rgb(0xff3366)),
                ),
            )
            .on_open_change(move |v, _, _| {
                opened.borrow_mut().push(format!("open:{v}"));
            })
            .into_any_element()
    });

    // The close trigger is `absolute end-4 top-4`, a 24px button centred at
    // (1892, 28). The press reports the drawer's own close: the custom
    // children replaced the button's glyph, not its wiring.
    click(cx, 1892., 28.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false"],
        "the composed custom content must still report the drawer's close"
    );

    // Escape is a separate dismissal path and reports through the same
    // drawer-owned callbacks.
    press(cx, "escape");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false", "open:false"],
        "escape must still dismiss a drawer with a custom close trigger"
    );
}

/// With no dismissal callback to wire, a composed `DrawerCloseTrigger`
/// renders nothing at all — not even its custom children — while the
/// drawer's own body stays interactive.
#[gpui::test]
fn drawer_callback_less_close_trigger_renders_nothing(cx: &mut TestAppContext) {
    still();
    let rec = events();
    let probed = rec.clone();
    let body = events();
    let body_probed = body.clone();

    let cx = open_host(cx, move || {
        let probed = rec.clone();
        let body_probed = body.clone();
        // No `on_close` and no `on_open_change`: the trigger has nothing to
        // wire and must draw nothing — the probe child would answer if it
        // were rendered bare in the slot.
        Drawer::new()
            .id("pd-drawer-bare-x")
            .is_open(true)
            .placement(DrawerPlacement::Right)
            .title("Drag me shut")
            .child(DrawerHandle::new())
            .child(
                DrawerCloseTrigger::new().child(
                    gpui::div()
                        .id("pd-drawer-bare-x-slot")
                        .size(px(24.))
                        .on_click(move |_, _, _| probed.borrow_mut().push("slot".into())),
                ),
            )
            .child(probe("pd-drawer-bare-x-body", "body", body_probed))
            .into_any_element()
    });

    // The drawer is up and its body answers: the 40x36 probe centres at
    // (1580, 86).
    click(cx, 1580., 86.);
    assert_eq!(body_probed.borrow().as_slice(), ["body"]);

    // The slot press records nothing: the slot is `absolute end-4 top-4`, a
    // 24px box centred at (1892, 28).
    click(cx, 1892., 28.);
    assert!(
        probed.borrow().is_empty(),
        "a callback-less close trigger must render nothing: {:?}",
        probed.borrow()
    );
}

/// A `DrawerCloseTrigger` composed through `footer_child` is not swallowed by
/// the footer row: it is pulled into the `absolute end-4 top-4` slot and
/// wired like a part composed among the body children.
#[gpui::test]
fn drawer_footer_child_close_trigger_is_pulled_into_the_slot(cx: &mut TestAppContext) {
    still();
    let opens = events();
    let opened = opens.clone();
    let body = events();
    let body_probed = body.clone();
    let open = Rc::new(RefCell::new(true));
    let open_flag = open;

    let cx = open_host(cx, move || {
        let opened = opens.clone();
        let body_probed = body.clone();
        let is_open = *open_flag.borrow();
        Drawer::new()
            .id("pd-drawer-footer-x")
            .is_open(is_open)
            .placement(DrawerPlacement::Right)
            .title("Drag me shut")
            .child(DrawerHandle::new())
            .child(probe("pd-drawer-footer-x-body", "body", body_probed))
            .footer_child(DrawerCloseTrigger::new())
            .on_open_change({
                let open_flag = open_flag.clone();
                move |v, window, _| {
                    *open_flag.borrow_mut() = *v;
                    opened.borrow_mut().push(format!("open:{v}"));
                    window.refresh();
                }
            })
            .into_any_element()
    });

    // The drawer is up and its body answers.
    click(cx, 1580., 86.);
    assert_eq!(body_probed.borrow().as_slice(), ["body"]);

    // The trigger was the only footer child, so the footer row retires with
    // it; the slot press at (1892, 28) closes the drawer exactly once.
    click(cx, 1892., 28.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false"],
        "a footer-composed close trigger must render in the slot, wired"
    );
}

/// `isDismissible = false` gates the outside press and the drag, never the
/// composed close part: a non-dismissible drawer that composes the default
/// `DrawerCloseTrigger` must still close from it, while the scrim press
/// reports nothing.
#[gpui::test]
fn drawer_not_dismissible_still_closes_from_a_composed_close_trigger(cx: &mut TestAppContext) {
    still();
    let opens = events();
    let opened = opens.clone();
    let open = Rc::new(RefCell::new(true));
    let open_flag = open;

    let cx = open_host(cx, move || {
        let opened = opens.clone();
        let is_open = *open_flag.borrow();
        Drawer::new()
            .id("pd-drawer-nd-x")
            .is_open(is_open)
            .placement(DrawerPlacement::Right)
            .title("Drag me shut")
            .child(DrawerCloseTrigger::new())
            .is_dismissible(false)
            .on_open_change({
                let open_flag = open_flag.clone();
                move |v, window, _| {
                    *open_flag.borrow_mut() = *v;
                    opened.borrow_mut().push(format!("open:{v}"));
                    window.refresh();
                }
            })
            .into_any_element()
    });

    // The scrim press stays off the table for a non-dismissible drawer...
    click(cx, 100., 100.);
    assert!(
        opened.borrow().is_empty(),
        "a non-dismissible scrim press must not dismiss the drawer"
    );

    // ...but the composed close trigger is not the scrim. The trigger is
    // `absolute end-4 top-4`, a 24px button centred at (1892, 28), and its
    // press must close the drawer exactly once.
    click(cx, 1892., 28.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false"],
        "the composed close trigger must close a non-dismissible drawer"
    );
}

/// `isDismissible = false` takes the outside-press and the drag off the table
/// (the header's mouse-down that starts the drag record is not even attached)
/// while the panel itself still works — its body probe answers — and Escape,
/// the keyboard dismissal, still closes it. This is v3's Non-Dismissable
/// example: the scrim press and a 100px pull must both report nothing.
#[gpui::test]
fn drawer_not_dismissible_ignores_outside_press_and_drag(cx: &mut TestAppContext) {
    still();
    let closes = events();
    let closed = closes.clone();
    let opens = events();
    let opened = opens.clone();
    let probed = events();
    let hits = probed.clone();
    let open = Rc::new(RefCell::new(true));
    let open_flag = open;

    let cx = open_host(cx, move || {
        let closes = closes.clone();
        let opens = opens.clone();
        let hits = hits.clone();
        let is_open = *open_flag.borrow();
        Drawer::new()
            .id("pd-drawer-not-dismissible")
            .is_open(is_open)
            .placement(DrawerPlacement::Right)
            .title("Drag me shut")
            .is_dismissible(false)
            .child(DrawerHandle::new())
            .child(probe("pd-drawer-not-dismissible-probe", "hit", hits))
            .on_close(move |_, _, _| closes.borrow_mut().push("close".to_owned()))
            .on_open_change({
                let open_flag = open_flag.clone();
                move |v, window, _| {
                    *open_flag.borrow_mut() = *v;
                    opens.borrow_mut().push(format!("open:{v}"));
                    window.refresh();
                }
            })
            .into_any_element()
    });

    // The panel is still there and usable.
    click(cx, 1580., 86.);
    assert_eq!(
        probed.borrow().as_slice(),
        ["hit"],
        "a non-dismissible drawer must still render its body"
    );

    // A press on the dimmed region outside the panel..
    click(cx, 100., 100.);
    // ..and a 100px pull on the title row, over the drag threshold.
    drag(cx, (1760., 48.), (1860., 48.));
    assert!(
        closed.borrow().is_empty() && opened.borrow().is_empty(),
        "neither the outside press nor the drag may dismiss a \
         non-dismissible drawer"
    );

    // Keyboard dismissal is not the backdrop; it still works.
    press(cx, "escape");
    assert_eq!(
        closed.borrow().as_slice(),
        ["close"],
        "escape must still fire on_close"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false"],
        "escape must still dismiss a non-dismissible drawer"
    );
}

/// The footer slot renders as a real part of the sheet's column: with a title
/// and a body probe, the footer lands 20px (`mt-5`, v3's 1.25rem) after the
/// body, and the pressable it carries is reachable exactly there — proving
/// the slot exists in the layout and is not painted over by the body.
#[gpui::test]
fn drawer_footer_sits_after_the_body_and_both_answer(cx: &mut TestAppContext) {
    still();
    let probed = events();
    let hits = probed;
    let hits_for_view = hits.clone();
    let open = Rc::new(RefCell::new(true));
    let open_flag = open;

    let cx = open_host(cx, move || {
        let hits = hits_for_view.clone();
        let is_open = *open_flag.borrow();
        Drawer::new()
            .id("pd-drawer-footer")
            .is_open(is_open)
            .placement(DrawerPlacement::Right)
            .title("Drag me shut")
            .child(DrawerHandle::new())
            .child(probe("pd-drawer-footer-body", "body", hits.clone()))
            .footer_child(probe("pd-drawer-footer-foot", "footer", hits))
            .into_any_element()
    });

    // Body: 24 (p-6) + 12 (handle) + 24 (title) + 8 (mt-2) + 18 (half-probe).
    click(cx, 1580., 86.);
    assert_eq!(
        hits.borrow().as_slice(),
        ["body"],
        "the body probe must be reachable"
    );

    // The body is `flex_1`, so the footer consumes the bottom 36px of the
    // panel's 24px inset: y 1020..1056. It is `justify_end`, so its 40px probe
    // consumes x 1856..1896.
    click(cx, 1876., 1038.);
    assert_eq!(
        hits.borrow().as_slice(),
        ["body", "footer"],
        "the footer probe must be reachable where the footer's own extent \
         puts it"
    );
}

#[gpui::test]
fn picker_text_metrics_keep_sections_and_options_in_place(cx: &mut TestAppContext) {
    for kind in 0..3 {
        for leading in [None, Some(48.)] {
            harness::still();
            let state = search_state(cx);
            let changes = events();
            let recorded = changes.clone();
            let cx = open_host(cx, move || {
                let changes = changes.clone();
                let control = match kind {
                    0 => Select::new("leading-select", keyed(&["Alpha"]))
                        .default_open(true)
                        .section_before("Alpha", "First\nSecond")
                        .on_change(move |_, _, _| changes.borrow_mut().push("picked".into()))
                        .into_any_element(),
                    1 => Autocomplete::new(state.clone(), keyed(&["Alpha"]))
                        .default_open(true)
                        .section_before("Alpha", "First\nSecond")
                        .on_change(move |_, _, _| changes.borrow_mut().push("picked".into()))
                        .into_any_element(),
                    _ => ComboBox::new(state.clone(), keyed(&["Alpha"]))
                        .default_open(true)
                        .section_before("Alpha", "First\nSecond")
                        .on_change(move |_, _, _| changes.borrow_mut().push("picked".into()))
                        .into_any_element(),
                };
                gpui::div()
                    .w(px(320.))
                    .when_some(leading, |el, leading| el.line_height(px(leading)))
                    .child(
                        gpui::div()
                            .debug_selector(|| "picker-leading-trigger".into())
                            .child(control),
                    )
                    .into_any_element()
            });
            flush_frame(cx);
            assert_eq!(
                cx.debug_bounds("picker-leading-trigger")
                    .expect("trigger paints")
                    .size
                    .height,
                px(36.),
                "kind={kind}, host={leading:?}"
            );
            let option_center = match kind {
                0 => 108.,
                1 => 160.,
                _ => 106.,
            };
            click(cx, 60., option_center - 36.);
            assert!(
                recorded.borrow().is_empty(),
                "section is inert: kind={kind}, host={leading:?}"
            );
            click(cx, 60., option_center);
            assert_eq!(recorded.borrow().as_slice(), ["picked"], "36px option follows two 16px header lines plus 10px header padding: kind={kind}, host={leading:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// Select popup viewport visibility
// ---------------------------------------------------------------------------
//
// Upstream, `Select.Popover` is a React Aria Components `Popover` with only
// `placement` defaulted (`"bottom"`): no `offset`, `containerPadding`,
// `shouldFlip`, or `maxHeight` override. The inherited contract is therefore
// RAC's own — `offset` 8 (`Popover`'s `props.offset ?? 8`), `containerPadding`
// 12 and `shouldFlip: true` (`useOverlayPosition` defaults), and a computed
// `maxHeight` from `calculatePosition`'s `getMaxHeight`: the space from the
// placed edge to the viewport boundary minus padding, with the side flipped
// when the full overlay fits better on the other side (`overlaySize[size] >
// space` flips only toward the roomier side). The popover itself scrolls
// (`select.css`: `.select__popover` is `overflow-y-auto`, `min-w-(--trigger-width)`)
// while the ListBox inside is `overflow-clip` — there is no 280px cap
// anywhere upstream.
//
// The port previously hung the panel from `util::placed_field_panel` with a
// fixed `max_h(280)` (and a fixed `h(280)` virtual list): no flip, no
// viewport clamp, no available-height cap. A trigger near the bottom of the
// window pushed the list off-screen and the last rows stayed unreachable.
// These tests pin the upstream behavior with real bounds and real
// interaction: flip above the trigger, trigger-width alignment, an
// available-height cap with a reachable last row, resize tracking, and the
// keyboard paths — for both the plain and the virtual lists.

/// Twelve plain rows: natural panel height (~444px) fits neither below a
/// bottom trigger nor, in a 480px window, above it uncapped.
fn viewport_options(n: usize) -> Vec<PickerItem> {
    (0..n)
        .map(|i| {
            let label = format!("Option {i:02}");
            PickerItem::new(label.clone(), label)
        })
        .collect()
}

/// Resize plus frames for the measured position to settle; the positioner
/// measures and places in the same frame.
fn settle_select(cx: &mut VisualTestContext, width: f32, height: f32) {
    cx.simulate_resize(gpui::size(px(width), px(height)));
    for _ in 0..4 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

/// Layout rounds to whole pixels, and `float_cmp` is denied.
fn near_px(value: gpui::Pixels, expected: f32) -> bool {
    (f32::from(value) - expected).abs() < 1.5
}

/// A Select whose 320px trigger sits at (40, `top`) in a 640px window.
fn select_at(top: f32, select: Select) -> gpui::AnyElement {
    gpui::div()
        .size_full()
        .child(
            gpui::div()
                .absolute()
                .left(px(40.))
                .top(px(top))
                .w(px(320.))
                .child(select)
                .into_any_element(),
        )
        .into_any_element()
}

/// The same trigger inside a tracked scrollable page. The inner content is
/// 2000px tall so the page can scroll at both the 480px and 1100px test
/// viewports; with the page at offset zero the trigger sits at the same
/// window coordinates as `select_at`, while a wheel over the deferred popup
/// must not move the page (`overscroll-contain`).
fn select_in_page(top: f32, select: Select, page_scroll: gpui::ScrollHandle) -> gpui::AnyElement {
    gpui::div()
        .id("select-viewport-page")
        .size_full()
        .overflow_y_scroll()
        .track_scroll(&page_scroll)
        .child(
            gpui::div().relative().h(px(2000.)).child(
                gpui::div()
                    .absolute()
                    .left(px(40.))
                    .top(px(top))
                    .w(px(320.))
                    .child(select)
                    .into_any_element(),
            ),
        )
        .into_any_element()
}

#[gpui::test]
fn select_panel_flips_above_and_keeps_every_row_reachable(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let opened = events();
    let opens = opened.clone();
    let page_scroll = gpui::ScrollHandle::new();
    let page_for_view = page_scroll.clone();
    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        let opens = opened.clone();
        let page_scroll = page_for_view.clone();
        select_in_page(
            408.,
            Select::new("sel-vp", viewport_options(12))
                .full_width(true)
                .on_change(move |key, _, _| recorded.borrow_mut().push(format!("{key:?}")))
                .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("{open}"))),
            page_scroll,
        )
    });
    settle_select(cx, 640., 480.);
    click(cx, 200., 426.);
    settle_select(cx, 640., 480.);
    assert_eq!(opens.borrow().as_slice(), ["true"]);

    let trigger = cx
        .debug_bounds("select-trigger-Name(\"sel-vp\")")
        .expect("the trigger must be laid out");
    let panel = cx
        .debug_bounds("select-list-Name(\"sel-vp\")-panel")
        .expect("the open panel must be laid out");

    // The natural list (~444px) fits nowhere below the trigger, so the panel
    // flips above it with RAC's 8px gap, stays on the 12px viewport inset,
    // and matches the trigger width like `min-w-(--trigger-width)`.
    assert!(
        near_px(panel.bottom(), f32::from(trigger.top()) - 8.),
        "the panel must flip above a bottom trigger with an 8px gap: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.top()) >= 12. - 1.5,
        "the flipped panel must stay on the viewport inset, got {panel:?}"
    );
    assert!(
        f32::from(panel.bottom()) <= 480. - 12. + 1.5,
        "the panel must stay inside the window, got {panel:?}"
    );
    assert!(
        near_px(panel.left(), f32::from(trigger.left())),
        "the panel must align with the trigger: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        near_px(panel.size.width, f32::from(trigger.size.width)),
        "the panel must match the trigger width: panel={panel:?} trigger={trigger:?}"
    );

    // Neither side fits the natural height, so the panel caps at the
    // available height instead of the arbitrary 280px.
    assert!(
        f32::from(panel.size.height) > 280. + 1.5,
        "the capped panel must use the room above the trigger, got {panel:?}"
    );

    // The last row starts below the capped panel; scrolling the panel must
    // bring it into view and keep it clickable without moving the page.
    let last_selector = "select-list-Name(\"sel-vp\")-opt-11";
    let hidden = cx
        .debug_bounds(last_selector)
        .expect("rows must be laid out");
    assert!(
        f32::from(hidden.top()) >= f32::from(panel.bottom()),
        "the last row must start out of view in a capped panel: row={hidden:?} panel={panel:?}"
    );
    let trigger_before = cx
        .debug_bounds("select-trigger-Name(\"sel-vp\")")
        .expect("the trigger must be laid out");
    for _ in 0..3 {
        cx.simulate_event(gpui::ScrollEvent {
            position: panel.center(),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
            modifiers: Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let panel = cx
        .debug_bounds("select-list-Name(\"sel-vp\")-panel")
        .expect("the open panel must be laid out");
    let last = cx
        .debug_bounds(last_selector)
        .expect("rows must be laid out");
    assert!(
        f32::from(last.top()) >= f32::from(panel.top()) - 1.5
            && f32::from(last.bottom()) <= f32::from(panel.bottom()) + 1.5,
        "scrolling must reveal the last row: row={last:?} panel={panel:?}"
    );
    assert_eq!(
        page_scroll.offset().y,
        px(0.),
        "scrolling the popup must not move the page"
    );
    let trigger_after = cx
        .debug_bounds("select-trigger-Name(\"sel-vp\")")
        .expect("the trigger must be laid out");
    assert_eq!(
        trigger_after.origin, trigger_before.origin,
        "the page must not scroll under the open popup"
    );
    // Reaching the popup boundary must still contain the wheel.
    cx.simulate_event(gpui::ScrollEvent {
        position: panel.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
        modifiers: Modifiers::none(),
        touch_phase: gpui::TouchPhase::Moved,
        ..Default::default()
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(
        page_scroll.offset().y,
        px(0.),
        "scrolling at the popup boundary must not move the page"
    );
    let last = cx
        .debug_bounds(last_selector)
        .expect("rows must be laid out");
    click(cx, f32::from(last.center().x), f32::from(last.center().y));
    assert_eq!(recorded.borrow().as_slice(), ["Some(\"Option 11\")"]);
    assert_eq!(opens.borrow().as_slice(), ["true", "false"]);
}

#[gpui::test]
fn select_virtual_panel_flips_caps_and_tracks_resize(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let opened = events();
    let opens = opened.clone();
    let page_scroll = gpui::ScrollHandle::new();
    let page_for_view = page_scroll.clone();
    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        let opens = opened.clone();
        let page_scroll = page_for_view.clone();
        select_in_page(
            408.,
            Select::new("sel-vv", viewport_options(200))
                .full_width(true)
                .row_height(px(36.))
                .on_change(move |key, _, _| recorded.borrow_mut().push(format!("{key:?}")))
                .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("{open}"))),
            page_scroll,
        )
    });
    settle_select(cx, 640., 480.);
    click(cx, 200., 426.);
    settle_select(cx, 640., 480.);
    assert_eq!(opens.borrow().as_slice(), ["true"]);

    let trigger = cx
        .debug_bounds("select-trigger-Name(\"sel-vv\")")
        .expect("the trigger must be laid out");
    let panel = cx
        .debug_bounds("select-list-Name(\"sel-vv\")-panel")
        .expect("the open panel must be laid out");

    // A 200-row virtual list never fits uncapped: it flips above and caps at
    // the available height rather than the fixed 280px list viewport.
    assert!(
        near_px(panel.bottom(), f32::from(trigger.top()) - 8.),
        "the virtual panel must flip above a bottom trigger with an 8px gap: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.top()) >= 12. - 1.5 && f32::from(panel.bottom()) <= 480. - 12. + 1.5,
        "the capped virtual panel must stay inside the window, got {panel:?}"
    );
    assert!(
        near_px(panel.left(), f32::from(trigger.left()))
            && near_px(panel.size.width, f32::from(trigger.size.width)),
        "the virtual panel must match the trigger width: panel={panel:?} trigger={trigger:?}"
    );

    // Growing the window must reposition the still-open panel: below the
    // trigger now has the most room, so it flips back and stays inset.
    settle_select(cx, 640., 1100.);
    let trigger = cx
        .debug_bounds("select-trigger-Name(\"sel-vv\")")
        .expect("the trigger must be laid out");
    let panel = cx
        .debug_bounds("select-list-Name(\"sel-vv\")-panel")
        .expect("the open panel must be laid out");
    assert!(
        near_px(panel.top(), f32::from(trigger.bottom()) + 8.),
        "the panel must flip back below once the window grows with an 8px gap: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.bottom()) <= 1100. - 12. + 1.5,
        "the repositioned panel must stay inside the window, got {panel:?}"
    );

    // The pointer reaches the last virtual row through the wheel: scrolling
    // the popup must not move the page, and clicking the revealed row picks
    // it and closes the panel.
    let trigger_before = cx
        .debug_bounds("select-trigger-Name(\"sel-vv\")")
        .expect("the trigger must be laid out");
    for _ in 0..10 {
        cx.simulate_event(gpui::ScrollEvent {
            position: panel.center(),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
            modifiers: Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let panel = cx
        .debug_bounds("select-list-Name(\"sel-vv\")-panel")
        .expect("the open panel must be laid out");
    let last = cx
        .debug_bounds("select-list-Name(\"sel-vv\")-opt-199")
        .expect("rows must be laid out");
    assert!(
        f32::from(last.top()) >= f32::from(panel.top()) - 1.5
            && f32::from(last.bottom()) <= f32::from(panel.bottom()) + 1.5,
        "wheel must scroll the last virtual row into view: row={last:?} panel={panel:?}"
    );
    assert_eq!(
        page_scroll.offset().y,
        px(0.),
        "scrolling the virtual popup must not move the page"
    );
    let trigger_after = cx
        .debug_bounds("select-trigger-Name(\"sel-vv\")")
        .expect("the trigger must be laid out");
    assert_eq!(
        trigger_after.origin, trigger_before.origin,
        "the page must not scroll under the open virtual popup"
    );
    click(cx, f32::from(last.center().x), f32::from(last.center().y));
    assert_eq!(recorded.borrow().as_slice(), ["Some(\"Option 199\")"]);
    assert_eq!(opens.borrow().as_slice(), ["true", "false"]);

    // Reopen for the keyboard path: End still reaches the last virtual row
    // and Enter picks it, which closes the panel.
    click(cx, 200., 426.);
    settle_select(cx, 640., 1100.);
    flush_frame(cx);
    press(cx, "end");
    flush_frame(cx);
    let panel = cx
        .debug_bounds("select-list-Name(\"sel-vv\")-panel")
        .expect("the open panel must be laid out");
    let last = cx
        .debug_bounds("select-list-Name(\"sel-vv\")-opt-199")
        .expect("rows must be laid out");
    assert!(
        f32::from(last.top()) >= f32::from(panel.top()) - 1.5
            && f32::from(last.bottom()) <= f32::from(panel.bottom()) + 1.5,
        "End must scroll the last virtual row into view: row={last:?} panel={panel:?}"
    );
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Some(\"Option 199\")", "Some(\"Option 199\")"]
    );
    assert_eq!(
        opens.borrow().as_slice(),
        ["true", "false", "true", "false"]
    );
}

#[gpui::test]
fn select_panel_anchors_to_the_trigger_and_dismisses_with_escape(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let opened = events();
    let opens = opened.clone();
    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        let opens = opened.clone();
        select_at(
            40.,
            Select::new("sel-vl", viewport_options(3))
                .full_width(true)
                .label("Language")
                .description("Pick one")
                .on_change(move |key, _, _| recorded.borrow_mut().push(format!("{key:?}")))
                .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("{open}"))),
        )
    });
    settle_select(cx, 640., 480.);

    // The trigger sits below its label: the panel must anchor to the trigger
    // bounds, not to the label-to-description wrapper root. Three rows fit
    // below, so the preferred side stays below with RAC's 8px gap.
    let trigger = cx
        .debug_bounds("select-trigger-Name(\"sel-vl\")")
        .expect("the trigger must be laid out");
    click(cx, 200., f32::from(trigger.center().y));
    settle_select(cx, 640., 480.);
    assert_eq!(opens.borrow().as_slice(), ["true"]);

    let panel = cx
        .debug_bounds("select-list-Name(\"sel-vl\")-panel")
        .expect("the open panel must be laid out");
    assert!(
        near_px(panel.top(), f32::from(trigger.bottom()) + 8.),
        "a labeled panel must sit 8px below the trigger itself: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.bottom()) <= 480. - 12. + 1.5,
        "the labeled panel must stay inside the window, got {panel:?}"
    );

    // Escape dismisses; clicking where the last row was must then hit
    // nothing and record nothing.
    let last = cx
        .debug_bounds("select-list-Name(\"sel-vl\")-opt-2")
        .expect("rows must be laid out");
    let stale = (f32::from(last.center().x), f32::from(last.center().y));
    press(cx, "escape");
    assert_eq!(opens.borrow().as_slice(), ["true", "false"]);
    let_exit_finish(cx);
    click(cx, stale.0.min(639.), stale.1.min(479.));
    assert!(
        recorded.borrow().is_empty(),
        "a dismissed panel must not answer the pointer"
    );
}

// ComboBox popup viewport visibility
// ---------------------------------------------------------------------------
//
// Upstream, `ComboBox.Popover` is a React Aria Components `Popover` exactly
// like `Select.Popover`: no `offset`, `containerPadding`, `shouldFlip` or
// `maxHeight` override, so the inherited contract is RAC's own — `offset` 8
// (`Popover`'s `props.offset ?? 8`), `containerPadding` 12 and
// `shouldFlip: true` (`useOverlayPosition` defaults), with a computed
// `maxHeight` from `calculatePosition`'s `getMaxHeight`: the space from the
// placed edge to the viewport boundary minus padding, flipping toward the
// roomier side. The popover itself scrolls (`combo-box.css`:
// `.combo-box__popover` is `overflow-y-auto overscroll-contain,
// min-w-(--trigger-width)`) while the ListBox inside is `overflow-clip` —
// there is no 240px cap anywhere upstream. RAC also tracks the trigger width
// through a resize observer (`--trigger-width`), so the panel follows resizes.
//
// The port previously hung the panel from `util::placed_field_panel` with a
// fixed `max_h(240)` (and a fixed `h(240)` virtual list): no flip, no
// viewport clamp, no available-height cap, no wheel containment. A trigger
// near the bottom of the window pushed the list off-screen and the last rows
// stayed unreachable. These tests pin the upstream behavior with real bounds
// and real interaction, mirroring the Select viewport tests above: flip above
// the trigger, trigger-width alignment, an available-height cap with a
// reachable last row, resize tracking, and the keyboard path — for both the
// plain and the virtual lists.

/// `debug_bounds` wants a `&'static str`; dynamic selectors leak one short
/// string per probe.
fn combo_probe(name: String) -> &'static str {
    let leaked: &'static mut str = Box::leak(name.into_boxed_str());
    &*leaked
}

/// Twelve plain rows: natural panel height (~462px) fits neither below a
/// bottom trigger nor, in a 480px window, above it uncapped.
fn combo_options(n: usize) -> Vec<PickerItem> {
    (0..n)
        .map(|i| {
            let label = format!("Option {i:02}");
            PickerItem::new(label.clone(), label)
        })
        .collect()
}

/// A ComboBox whose 320px field sits at (40, `top`) in a 640px window.
fn combo_at(top: f32, combo: ComboBox) -> gpui::AnyElement {
    gpui::div()
        .size_full()
        .child(
            gpui::div()
                .absolute()
                .left(px(40.))
                .top(px(top))
                .w(px(320.))
                .child(combo)
                .into_any_element(),
        )
        .into_any_element()
}

/// The same field inside a tracked scrollable page. The inner content is
/// 2000px tall so the page can scroll at both the 480px and 1100px test
/// viewports; with the page at offset zero the field sits at the same window
/// coordinates as `combo_at`, while a wheel over the deferred popup must not
/// move the page (`overscroll-contain`).
fn combo_in_page(top: f32, combo: ComboBox, page_scroll: gpui::ScrollHandle) -> gpui::AnyElement {
    gpui::div()
        .id("combobox-viewport-page")
        .size_full()
        .overflow_y_scroll()
        .track_scroll(&page_scroll)
        .child(
            gpui::div().relative().h(px(2000.)).child(
                gpui::div()
                    .absolute()
                    .left(px(40.))
                    .top(px(top))
                    .w(px(320.))
                    .child(combo)
                    .into_any_element(),
            ),
        )
        .into_any_element()
}

#[gpui::test]
fn combo_box_panel_flips_above_and_keeps_every_row_reachable(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let opened = events();
    let opens = opened.clone();
    let page_scroll = gpui::ScrollHandle::new();
    let page_for_view = page_scroll.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        let opens = opened.clone();
        let page_scroll = page_for_view.clone();
        combo_in_page(
            408.,
            ComboBox::new(state_for_view.clone(), combo_options(12))
                .full_width(true)
                .max_items(12)
                .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
                .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("{open}"))),
            page_scroll,
        )
    });
    settle_select(cx, 640., 480.);
    click(cx, 200., 426.);
    settle_select(cx, 640., 480.);
    assert_eq!(opens.borrow().as_slice(), ["true"]);

    let trigger = cx
        .debug_bounds(combo_probe(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    let panel = cx
        .debug_bounds(combo_probe(format!("combobox-panel-{entity_id}")))
        .expect("the open panel must be laid out");

    // The natural list (~462px) fits nowhere below the field, so the panel
    // flips above it with RAC's 8px gap, stays on the 12px viewport inset,
    // and matches the field width like `min-w-(--trigger-width)`.
    assert!(
        near_px(panel.bottom(), f32::from(trigger.top()) - 8.),
        "the panel must flip above a bottom field with an 8px gap: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.top()) >= 12. - 1.5,
        "the flipped panel must stay on the viewport inset, got {panel:?}"
    );
    assert!(
        f32::from(panel.bottom()) <= 480. - 12. + 1.5,
        "the panel must stay inside the window, got {panel:?}"
    );
    assert!(
        near_px(panel.left(), f32::from(trigger.left())),
        "the panel must align with the field: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        near_px(panel.size.width, f32::from(trigger.size.width)),
        "the panel must match the field width: panel={panel:?} trigger={trigger:?}"
    );

    // Neither side fits the natural height, so the panel caps at the
    // available height instead of the arbitrary 240px.
    assert!(
        f32::from(panel.size.height) > 240. + 1.5,
        "the capped panel must use the room above the field, got {panel:?}"
    );

    // The last row starts below the capped panel; scrolling the panel must
    // bring it into view and keep it clickable without moving the page.
    let last_selector = combo_probe(format!("combobox-{entity_id}-item-Option 11"));
    let hidden = cx
        .debug_bounds(last_selector)
        .expect("rows must be laid out");
    assert!(
        f32::from(hidden.top()) >= f32::from(panel.bottom()),
        "the last row must start out of view in a capped panel: row={hidden:?} panel={panel:?}"
    );
    let trigger_before = cx
        .debug_bounds(combo_probe(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    for _ in 0..3 {
        cx.simulate_event(gpui::ScrollEvent {
            position: panel.center(),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
            modifiers: Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let panel = cx
        .debug_bounds(combo_probe(format!("combobox-panel-{entity_id}")))
        .expect("the open panel must be laid out");
    let last = cx
        .debug_bounds(last_selector)
        .expect("rows must be laid out");
    assert!(
        f32::from(last.top()) >= f32::from(panel.top()) - 1.5
            && f32::from(last.bottom()) <= f32::from(panel.bottom()) + 1.5,
        "scrolling must reveal the last row: row={last:?} panel={panel:?}"
    );
    assert_eq!(
        page_scroll.offset().y,
        px(0.),
        "scrolling the popup must not move the page"
    );
    let trigger_after = cx
        .debug_bounds(combo_probe(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    assert_eq!(
        trigger_after.origin, trigger_before.origin,
        "the page must not scroll under the open popup"
    );
    // Reaching the popup boundary must still contain the wheel.
    cx.simulate_event(gpui::ScrollEvent {
        position: panel.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
        modifiers: Modifiers::none(),
        touch_phase: gpui::TouchPhase::Moved,
        ..Default::default()
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(
        page_scroll.offset().y,
        px(0.),
        "scrolling at the popup boundary must not move the page"
    );
    let last = cx
        .debug_bounds(combo_probe(format!("combobox-{entity_id}-item-Option 11")))
        .expect("rows must be laid out");
    click(cx, f32::from(last.center().x), f32::from(last.center().y));
    assert_eq!(recorded.borrow().as_slice(), ["Option 11"]);
    assert_eq!(opens.borrow().as_slice(), ["true", "false"]);
}

#[gpui::test]
fn combo_box_virtual_panel_flips_caps_and_tracks_resize(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let opened = events();
    let opens = opened.clone();
    let page_scroll = gpui::ScrollHandle::new();
    let page_for_view = page_scroll.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        let opens = opened.clone();
        let page_scroll = page_for_view.clone();
        combo_in_page(
            408.,
            ComboBox::new(state_for_view.clone(), combo_options(200))
                .full_width(true)
                .max_items(200)
                .row_height(px(36.))
                .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
                .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("{open}"))),
            page_scroll,
        )
    });
    settle_select(cx, 640., 480.);
    click(cx, 200., 426.);
    settle_select(cx, 640., 480.);
    assert_eq!(opens.borrow().as_slice(), ["true"]);

    let trigger = cx
        .debug_bounds(combo_probe(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    let panel = cx
        .debug_bounds(combo_probe(format!("combobox-panel-{entity_id}")))
        .expect("the open panel must be laid out");

    // A 200-row virtual list never fits uncapped: it flips above and caps at
    // the available height rather than the fixed 240px list viewport.
    assert!(
        near_px(panel.bottom(), f32::from(trigger.top()) - 8.),
        "the virtual panel must flip above a bottom field with an 8px gap: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.top()) >= 12. - 1.5 && f32::from(panel.bottom()) <= 480. - 12. + 1.5,
        "the capped virtual panel must stay inside the window, got {panel:?}"
    );
    assert!(
        near_px(panel.left(), f32::from(trigger.left()))
            && near_px(panel.size.width, f32::from(trigger.size.width)),
        "the virtual panel must match the field width: panel={panel:?} trigger={trigger:?}"
    );

    // Growing the window must reposition the still-open panel: below the
    // field now has the most room, so it flips back and stays inset.
    settle_select(cx, 640., 1100.);
    let trigger = cx
        .debug_bounds(combo_probe(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    let panel = cx
        .debug_bounds(combo_probe(format!("combobox-panel-{entity_id}")))
        .expect("the open panel must be laid out");
    assert!(
        near_px(panel.top(), f32::from(trigger.bottom()) + 8.),
        "the panel must flip back below once the window grows with an 8px gap: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.bottom()) <= 1100. - 12. + 1.5,
        "the repositioned panel must stay inside the window, got {panel:?}"
    );

    // The pointer reaches the last virtual row through the wheel: scrolling
    // the popup must not move the page, and clicking the revealed row picks
    // it and closes the panel.
    let trigger_before = cx
        .debug_bounds(combo_probe(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    for _ in 0..10 {
        cx.simulate_event(gpui::ScrollEvent {
            position: panel.center(),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
            modifiers: Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let panel = cx
        .debug_bounds(combo_probe(format!("combobox-panel-{entity_id}")))
        .expect("the open panel must be laid out");
    let last = cx
        .debug_bounds(combo_probe(format!("combobox-{entity_id}-item-Option 199")))
        .expect("rows must be laid out");
    assert!(
        f32::from(last.top()) >= f32::from(panel.top()) - 1.5
            && f32::from(last.bottom()) <= f32::from(panel.bottom()) + 1.5,
        "wheel must scroll the last virtual row into view: row={last:?} panel={panel:?}"
    );
    assert_eq!(
        page_scroll.offset().y,
        px(0.),
        "scrolling the virtual popup must not move the page"
    );
    let trigger_after = cx
        .debug_bounds(combo_probe(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    assert_eq!(
        trigger_after.origin, trigger_before.origin,
        "the page must not scroll under the open virtual popup"
    );
    click(cx, f32::from(last.center().x), f32::from(last.center().y));
    assert_eq!(recorded.borrow().as_slice(), ["Option 199"]);
    assert_eq!(opens.borrow().as_slice(), ["true", "false"]);

    // Reopen for the keyboard path: End still reaches the last virtual row
    // and Enter picks it, which closes the panel.
    click(cx, 338., 426.);
    settle_select(cx, 640., 1100.);
    flush_frame(cx);
    press(cx, "end");
    flush_frame(cx);
    let panel = cx
        .debug_bounds(combo_probe(format!("combobox-panel-{entity_id}")))
        .expect("the open panel must be laid out");
    let last = cx
        .debug_bounds(combo_probe(format!("combobox-{entity_id}-item-Option 199")))
        .expect("rows must be laid out");
    assert!(
        f32::from(last.top()) >= f32::from(panel.top()) - 1.5
            && f32::from(last.bottom()) <= f32::from(panel.bottom()) + 1.5,
        "End must scroll the last virtual row into view: row={last:?} panel={panel:?}"
    );
    press(cx, "enter");
    assert_eq!(recorded.borrow().as_slice(), ["Option 199", "Option 199"]);
    assert_eq!(
        opens.borrow().as_slice(),
        ["true", "false", "true", "false"]
    );
}

#[gpui::test]
fn combo_box_panel_anchors_to_the_field_and_dismisses_with_escape(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let opened = events();
    let opens = opened.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        let opens = opened.clone();
        combo_at(
            40.,
            ComboBox::new(state_for_view.clone(), combo_options(3))
                .full_width(true)
                .label("Language")
                .description("Pick one")
                .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
                .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("{open}"))),
        )
    });
    settle_select(cx, 640., 480.);

    // The anchor is the Input's 36px field row — not the label-to-error
    // wrapper — the way RAC's `triggerRef` reads `groupRef.current ||
    // inputRef.current`. At top 40 the 20px label plus 4px gap puts the row
    // at 64..100; the 16px description starts at 104, so both live outside
    // the measured bounds. Three rows fit below, so the preferred side
    // stays below with RAC's 8px gap (panel 108, not the wrapper-based 128).
    let trigger = cx
        .debug_bounds(combo_probe(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    assert!(
        near_px(trigger.top(), 64.) && near_px(trigger.bottom(), 100.),
        "the anchor must be the 36px field row below the label: trigger={trigger:?}"
    );
    assert!(
        near_px(trigger.size.height, 36.),
        "the anchor must be exactly one field tall: trigger={trigger:?}"
    );
    click(cx, 200., f32::from(trigger.center().y));
    settle_select(cx, 640., 480.);
    assert_eq!(opens.borrow().as_slice(), ["true"]);

    let panel = cx
        .debug_bounds(combo_probe(format!("combobox-panel-{entity_id}")))
        .expect("the open panel must be laid out");
    assert!(
        near_px(panel.top(), 108.),
        "a labeled panel must sit 8px below the field row itself: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        near_px(panel.top(), f32::from(trigger.bottom()) + 8.),
        "a labeled panel must sit 8px below the field row: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.bottom()) <= 480. - 12. + 1.5,
        "the labeled panel must stay inside the window, got {panel:?}"
    );

    // Escape dismisses; clicking where the last row was must then hit
    // nothing and record nothing.
    let last = cx
        .debug_bounds(combo_probe(format!("combobox-{entity_id}-item-Option 02")))
        .expect("rows must be laid out");
    let stale = (f32::from(last.center().x), f32::from(last.center().y));
    press(cx, "escape");
    assert_eq!(opens.borrow().as_slice(), ["true", "false"]);
    click(cx, stale.0.min(639.), stale.1.min(479.));
    assert!(
        recorded.borrow().is_empty(),
        "a dismissed panel must not answer the pointer"
    );
}

#[gpui::test]
fn combo_box_panel_anchors_to_the_field_under_error_and_value(cx: &mut TestAppContext) {
    still();
    let opened = events();
    let opens = opened.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opened.clone();
        combo_at(
            40.,
            ComboBox::new(state_for_view.clone(), combo_options(3))
                .full_width(true)
                .label("Language")
                .is_invalid(true)
                .error_message("Required")
                .value_content(|v| v.default_children)
                .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("{open}"))),
        )
    });
    settle_select(cx, 640., 480.);

    // The error slot replaces the description at the same 104 offset and the
    // value row hangs below the field inside the root: both must stay outside
    // the 64..100 field anchor, so the panel still sits at 108.
    let trigger = cx
        .debug_bounds(combo_probe(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    assert!(
        near_px(trigger.top(), 64.) && near_px(trigger.bottom(), 100.),
        "the anchor must stay the field row under error and value: trigger={trigger:?}"
    );
    assert!(
        near_px(trigger.size.height, 36.),
        "the anchor must stay exactly one field tall: trigger={trigger:?}"
    );
    click(cx, 200., f32::from(trigger.center().y));
    settle_select(cx, 640., 480.);
    assert_eq!(opens.borrow().as_slice(), ["true"]);

    let panel = cx
        .debug_bounds(combo_probe(format!("combobox-panel-{entity_id}")))
        .expect("the open panel must be laid out");
    assert!(
        near_px(panel.top(), 108.)
            && near_px(panel.top(), f32::from(trigger.bottom()) + 8.),
        "the panel must sit 8px below the field row under error and value: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.bottom()) <= 480. - 12. + 1.5,
        "the error/value panel must stay inside the window, got {panel:?}"
    );
}

// ---------------------------------------------------------------------------
// Autocomplete popup viewport visibility
// ---------------------------------------------------------------------------
//
// Upstream, `Autocomplete.Popover` is `.autocomplete__popover`: `flex
// w-(--trigger-width)` with `overflow-hidden overscroll-contain` and `pt-2`,
// holding a fixed search header (`[data-slot="search-field"]` is `shrink-0
// px-3 py-1`) above the list itself (`[data-slot="list-box"]` is
// `max-h-[320px] min-h-0 overflow-y-auto p-1.5`). The list -- not an outer
// scroller -- owns the scrolling, and `min-h-0` in the flex column lets it
// shrink below 320px when the popover is capped (short viewport), keeping
// the last options reachable. Positioning is RAC's `useOverlayPosition`
// against the trigger rect: an 8px gap, flip toward the roomier side, and a
// cap at the available viewport height past a 12px inset.
//
// The port previously hung the panel from `util::placed_field_panel` with a
// 6px gap, measured nothing (so label/description/error rows pushed the
// panel down), clipped with no `min-h-0` inner list, and fixed the virtual
// list at `h(320)` with a 320px paging step: a trigger away from the origin
// pushed the list off-screen, the last rows stayed unreachable, and paging
// overshot every capped panel. These tests pin the upstream behavior with
// real bounds and real interaction, mirroring the Select/ComboBox viewport
// tests above: the trigger anchor excludes the label-to-error wrapper, the
// 8px gap, trigger-width alignment, the 12px inset with flip and cap, the
// fixed search header, reachable last rows for both lists, resize tracking,
// wheel containment over a scrollable page, label-based filtering, and
// virtual paging derived from the actual laid-out viewport height.

/// Choices whose key is the label itself.
fn auto_options(n: usize) -> Vec<PickerItem> {
    (0..n)
        .map(|i| {
            let label = format!("Choice {i:02}");
            PickerItem::new(label.clone(), label)
        })
        .collect()
}

/// An Autocomplete whose 320px field sits at (40, `top`) in a 640px window.
fn auto_at(top: f32, auto: Autocomplete) -> gpui::AnyElement {
    gpui::div()
        .size_full()
        .child(
            gpui::div()
                .absolute()
                .left(px(40.))
                .top(px(top))
                .w(px(320.))
                .child(auto)
                .into_any_element(),
        )
        .into_any_element()
}

/// The same field inside a tracked scrollable page. The inner content is
/// 2000px tall so the page can scroll at both the 480px and 1100px test
/// viewports; with the page at offset zero the field sits at the same window
/// coordinates as `auto_at`, while a wheel over the deferred popup must not
/// move the page (`overscroll-contain`).
fn auto_in_page(top: f32, auto: Autocomplete, page_scroll: gpui::ScrollHandle) -> gpui::AnyElement {
    gpui::div()
        .id("autocomplete-viewport-page")
        .size_full()
        .overflow_y_scroll()
        .track_scroll(&page_scroll)
        .child(
            gpui::div().relative().h(px(2000.)).child(
                gpui::div()
                    .absolute()
                    .left(px(40.))
                    .top(px(top))
                    .w(px(320.))
                    .child(auto)
                    .into_any_element(),
            ),
        )
        .into_any_element()
}

/// The trigger's headless probe: the clear-hover refinement makes the
/// selector carry its state, and an empty selection keeps it `false`.
fn auto_trigger(base: &str) -> &'static str {
    combo_probe(format!("{base}-trigger-suppressed-false"))
}

#[gpui::test]
fn autocomplete_panel_matches_trigger_width_with_8px_gap_and_320_list_cap(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let opened = events();
    let opens = opened.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let base = format!("autocomplete-{entity_id}");
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        let opens = opened.clone();
        auto_at(
            100.,
            Autocomplete::new(state_for_view.clone(), auto_options(12))
                .full_width(true)
                .max_items(12)
                .label("Language")
                .description("Pick one")
                .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
                .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("{open}"))),
        )
    });
    settle_select(cx, 640., 900.);

    // The anchor is the 36px trigger row itself -- not the
    // label-to-description wrapper. At top 100 the 20px label plus 4px gap
    // puts the row at 124..160; the description starts at 164, so both live
    // outside the measured bounds.
    let trigger = cx
        .debug_bounds(auto_trigger(&base))
        .expect("the trigger must be laid out");
    assert!(
        near_px(trigger.top(), 124.) && near_px(trigger.bottom(), 160.),
        "the anchor must be the 36px trigger row below the label: trigger={trigger:?}"
    );
    assert!(
        near_px(trigger.size.height, 36.),
        "the anchor must be exactly one row tall: trigger={trigger:?}"
    );
    click(cx, 200., f32::from(trigger.center().y));
    settle_select(cx, 640., 900.);
    assert_eq!(opens.borrow().as_slice(), ["true"]);

    // Twelve rows need ~444px, so the roomy-window list caps at the upstream
    // 320px maximum while the panel sits 8px below the trigger, matches its
    // width, and stays on the viewport inset.
    let trigger = cx
        .debug_bounds(auto_trigger(&base))
        .expect("the trigger must be laid out");
    let panel = cx
        .debug_bounds(combo_probe(format!("{base}-panel")))
        .expect("the open panel must be laid out");
    assert!(
        near_px(panel.top(), f32::from(trigger.bottom()) + 8.),
        "the panel must sit 8px below the trigger row itself: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        near_px(panel.left(), f32::from(trigger.left())),
        "the panel must align with the trigger: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        near_px(panel.size.width, f32::from(trigger.size.width)),
        "the panel must match the trigger width: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.bottom()) <= 900. - 12. + 1.5,
        "the panel must stay inside the window, got {panel:?}"
    );
    let list = cx
        .debug_bounds(combo_probe(format!("{base}-list-scroll")))
        .expect("the open list must be laid out");
    assert!(
        near_px(list.size.height, 320.),
        "the roomy-window list must keep the upstream 320px maximum, got {list:?}"
    );
    let search = cx
        .debug_bounds(combo_probe(format!("{base}-search")))
        .expect("the search header must be laid out");
    assert!(
        f32::from(search.top()) >= f32::from(panel.top()) - 1.5
            && f32::from(search.bottom()) <= f32::from(panel.bottom()) + 1.5,
        "the search header must stay visible inside the panel: search={search:?} panel={panel:?}"
    );

    // The last row starts below the capped list; scrolling the list must
    // bring it into view and keep it clickable.
    let last_selector = combo_probe(format!("{base}-list-Choice 11"));
    let hidden = cx
        .debug_bounds(last_selector)
        .expect("rows must be laid out");
    assert!(
        f32::from(hidden.top()) >= f32::from(list.bottom()),
        "the last row must start out of view in a capped list: row={hidden:?} list={list:?}"
    );
    for _ in 0..3 {
        cx.simulate_event(gpui::ScrollEvent {
            position: list.center(),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
            modifiers: Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let list = cx
        .debug_bounds(combo_probe(format!("{base}-list-scroll")))
        .expect("the open list must be laid out");
    let last = cx
        .debug_bounds(last_selector)
        .expect("rows must be laid out");
    assert!(
        f32::from(last.top()) >= f32::from(list.top()) - 1.5
            && f32::from(last.bottom()) <= f32::from(list.bottom()) + 1.5,
        "scrolling must reveal the last row: row={last:?} list={list:?}"
    );
    click(cx, f32::from(last.center().x), f32::from(last.center().y));
    assert_eq!(recorded.borrow().as_slice(), ["Choice 11"]);
    assert_eq!(opens.borrow().as_slice(), ["true", "false"]);
}

#[gpui::test]
fn autocomplete_trigger_anchor_excludes_error_and_escapes_cleanly(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let opened = events();
    let opens = opened.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let base = format!("autocomplete-{entity_id}");
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        let opens = opened.clone();
        auto_at(
            40.,
            Autocomplete::new(state_for_view.clone(), auto_options(3))
                .full_width(true)
                .max_items(3)
                .label("Language")
                .is_invalid(true)
                .error_message("Required")
                .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
                .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("{open}"))),
        )
    });
    settle_select(cx, 640., 480.);

    // The error slot replaces the description at the same offset below the
    // field: both must stay outside the 64..100 field anchor, so the panel
    // still sits at 108.
    let trigger = cx
        .debug_bounds(auto_trigger(&base))
        .expect("the trigger must be laid out");
    assert!(
        near_px(trigger.top(), 64.) && near_px(trigger.bottom(), 100.),
        "the anchor must stay the field row under error: trigger={trigger:?}"
    );
    assert!(
        near_px(trigger.size.height, 36.),
        "the anchor must stay exactly one row tall: trigger={trigger:?}"
    );
    click(cx, 200., f32::from(trigger.center().y));
    settle_select(cx, 640., 480.);
    assert_eq!(opens.borrow().as_slice(), ["true"]);

    let panel = cx
        .debug_bounds(combo_probe(format!("{base}-panel")))
        .expect("the open panel must be laid out");
    assert!(
        near_px(panel.top(), 108.)
            && near_px(panel.top(), f32::from(trigger.bottom()) + 8.),
        "the panel must sit 8px below the field row under error: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.bottom()) <= 480. - 12. + 1.5,
        "the error panel must stay inside the window, got {panel:?}"
    );

    // Escape dismisses; clicking where the last row was must then hit
    // nothing and record nothing.
    let last = cx
        .debug_bounds(combo_probe(format!("{base}-list-Choice 02")))
        .expect("rows must be laid out");
    let stale = (f32::from(last.center().x), f32::from(last.center().y));
    press(cx, "escape");
    assert_eq!(opens.borrow().as_slice(), ["true", "false"]);
    click(cx, stale.0.min(639.), stale.1.min(479.));
    assert!(
        recorded.borrow().is_empty(),
        "a dismissed panel must not answer the pointer"
    );
}

#[gpui::test]
fn autocomplete_short_panel_flips_caps_and_keeps_header_visible(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let opened = events();
    let opens = opened.clone();
    let page_scroll = gpui::ScrollHandle::new();
    let page_for_view = page_scroll.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let base = format!("autocomplete-{entity_id}");
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        let opens = opened.clone();
        let page_scroll = page_for_view.clone();
        auto_in_page(
            320.,
            Autocomplete::new(state_for_view.clone(), auto_options(12))
                .full_width(true)
                .max_items(12)
                .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
                .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("{open}"))),
            page_scroll,
        )
    });
    settle_select(cx, 640., 480.);
    click(cx, 200., 338.);
    settle_select(cx, 640., 480.);
    assert_eq!(opens.borrow().as_slice(), ["true"]);

    // Neither side fits the natural ~372px panel, so it flips above the
    // trigger with the 8px gap, caps at the available height, and the inner
    // list shrinks below the 320px maximum through `min-h-0`.
    let trigger = cx
        .debug_bounds(auto_trigger(&base))
        .expect("the field must be laid out");
    let panel = cx
        .debug_bounds(combo_probe(format!("{base}-panel")))
        .expect("the open panel must be laid out");
    assert!(
        near_px(panel.bottom(), f32::from(trigger.top()) - 8.),
        "the panel must flip above the trigger with an 8px gap: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.top()) >= 12. - 1.5 && f32::from(panel.bottom()) <= 480. - 12. + 1.5,
        "the capped panel must stay on the viewport inset, got {panel:?}"
    );
    assert!(
        near_px(panel.left(), f32::from(trigger.left()))
            && near_px(panel.size.width, f32::from(trigger.size.width)),
        "the flipped panel must match the trigger width: panel={panel:?} trigger={trigger:?}"
    );
    let list = cx
        .debug_bounds(combo_probe(format!("{base}-list-scroll")))
        .expect("the open list must be laid out");
    assert!(
        f32::from(list.size.height) < 320. - 1.5,
        "the capped list must shrink below the upstream maximum, got {list:?}"
    );

    // The search header is fixed above the list: wheeling the list must move
    // the rows without moving the header or the page behind the popup.
    let search_before = cx
        .debug_bounds(combo_probe(format!("{base}-search")))
        .expect("the search header must be laid out");
    for _ in 0..3 {
        cx.simulate_event(gpui::ScrollEvent {
            position: list.center(),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
            modifiers: Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let panel = cx
        .debug_bounds(combo_probe(format!("{base}-panel")))
        .expect("the open panel must be laid out");
    let search_after = cx
        .debug_bounds(combo_probe(format!("{base}-search")))
        .expect("the search header must be laid out");
    assert_eq!(
        search_after, search_before,
        "the search header must stay fixed while the inner list scrolls"
    );
    assert!(
        f32::from(search_after.bottom()) <= f32::from(panel.bottom()) + 1.5,
        "the fixed header must stay inside the capped panel: search={search_after:?} panel={panel:?}"
    );
    assert_eq!(
        page_scroll.offset().y,
        px(0.),
        "scrolling the popup must not move the page"
    );
    // Reaching the popup boundary must still contain the wheel.
    cx.simulate_event(gpui::ScrollEvent {
        position: panel.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
        modifiers: Modifiers::none(),
        touch_phase: gpui::TouchPhase::Moved,
        ..Default::default()
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(
        page_scroll.offset().y,
        px(0.),
        "scrolling at the popup boundary must not move the page"
    );

    // The wheel reveals the last row inside the shrunken list; clicking it
    // picks it and closes the panel.
    let list = cx
        .debug_bounds(combo_probe(format!("{base}-list-scroll")))
        .expect("the open list must be laid out");
    let last = cx
        .debug_bounds(combo_probe(format!("{base}-list-Choice 11")))
        .expect("rows must be laid out");
    assert!(
        f32::from(last.top()) >= f32::from(list.top()) - 1.5
            && f32::from(last.bottom()) <= f32::from(list.bottom()) + 1.5,
        "wheel must scroll the last row into the shrunken list: row={last:?} list={list:?}"
    );
    click(cx, f32::from(last.center().x), f32::from(last.center().y));
    assert_eq!(recorded.borrow().as_slice(), ["Choice 11"]);
    assert_eq!(opens.borrow().as_slice(), ["true", "false"]);
}

#[gpui::test]
fn autocomplete_virtual_panel_flips_caps_tracks_resize_and_answers_end(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let opened = events();
    let opens = opened.clone();
    let page_scroll = gpui::ScrollHandle::new();
    let page_for_view = page_scroll.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let base = format!("autocomplete-{entity_id}");
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        let opens = opened.clone();
        let page_scroll = page_for_view.clone();
        auto_in_page(
            320.,
            Autocomplete::new(state_for_view.clone(), auto_options(200))
                .full_width(true)
                .max_items(200)
                .row_height(px(36.))
                .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
                .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("{open}"))),
            page_scroll,
        )
    });
    settle_select(cx, 640., 480.);
    click(cx, 200., 338.);
    settle_select(cx, 640., 480.);
    assert_eq!(opens.borrow().as_slice(), ["true"]);

    // A 200-row virtual list never fits uncapped: it flips above and caps at
    // the available height instead of a fixed 320px viewport.
    let trigger = cx
        .debug_bounds(auto_trigger(&base))
        .expect("the field must be laid out");
    let panel = cx
        .debug_bounds(combo_probe(format!("{base}-panel")))
        .expect("the open panel must be laid out");
    assert!(
        near_px(panel.bottom(), f32::from(trigger.top()) - 8.),
        "the virtual panel must flip above with an 8px gap: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.top()) >= 12. - 1.5 && f32::from(panel.bottom()) <= 480. - 12. + 1.5,
        "the capped virtual panel must stay inside the window, got {panel:?}"
    );
    assert!(
        near_px(panel.left(), f32::from(trigger.left()))
            && near_px(panel.size.width, f32::from(trigger.size.width)),
        "the virtual panel must match the field width: panel={panel:?} trigger={trigger:?}"
    );

    // Growing the window must reposition the still-open panel: below the
    // field now has the most room, so it flips back and stays inset, and the
    // list returns to the upstream 320px maximum.
    settle_select(cx, 640., 1100.);
    let trigger = cx
        .debug_bounds(auto_trigger(&base))
        .expect("the field must be laid out");
    let panel = cx
        .debug_bounds(combo_probe(format!("{base}-panel")))
        .expect("the open panel must be laid out");
    assert!(
        near_px(panel.top(), f32::from(trigger.bottom()) + 8.),
        "the panel must flip back below once the window grows with an 8px gap: panel={panel:?} trigger={trigger:?}"
    );
    assert!(
        f32::from(panel.bottom()) <= 1100. - 12. + 1.5,
        "the repositioned panel must stay inside the window, got {panel:?}"
    );
    let list = cx
        .debug_bounds(combo_probe(format!("{base}-rows")))
        .expect("the virtual list must be laid out");
    assert!(
        near_px(list.size.height, 320.),
        "the roomy-window virtual list must keep the upstream 320px maximum, got {list:?}"
    );

    // The pointer reaches the last virtual row through the wheel: scrolling
    // the popup must not move the page, and clicking the revealed row picks
    // it and closes the panel.
    for _ in 0..10 {
        cx.simulate_event(gpui::ScrollEvent {
            position: panel.center(),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-1000.))),
            modifiers: Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let panel = cx
        .debug_bounds(combo_probe(format!("{base}-panel")))
        .expect("the open panel must be laid out");
    let last = cx
        .debug_bounds(combo_probe(format!("{base}-list-Choice 199")))
        .expect("rows must be laid out");
    assert!(
        f32::from(last.top()) >= f32::from(panel.top()) - 1.5
            && f32::from(last.bottom()) <= f32::from(panel.bottom()) + 1.5,
        "wheel must scroll the last virtual row into view: row={last:?} panel={panel:?}"
    );
    assert_eq!(
        page_scroll.offset().y,
        px(0.),
        "scrolling the virtual popup must not move the page"
    );
    click(cx, f32::from(last.center().x), f32::from(last.center().y));
    assert_eq!(recorded.borrow().as_slice(), ["Choice 199"]);
    assert_eq!(opens.borrow().as_slice(), ["true", "false"]);

    // Reopen for the keyboard path: End reaches the last virtual row and
    // Enter picks it, which closes the panel.
    click(cx, 200., 338.);
    settle_select(cx, 640., 1100.);
    flush_frame(cx);
    press(cx, "end");
    flush_frame(cx);
    let panel = cx
        .debug_bounds(combo_probe(format!("{base}-panel")))
        .expect("the open panel must be laid out");
    let last = cx
        .debug_bounds(combo_probe(format!("{base}-list-Choice 199")))
        .expect("rows must be laid out");
    assert!(
        f32::from(last.top()) >= f32::from(panel.top()) - 1.5
            && f32::from(last.bottom()) <= f32::from(panel.bottom()) + 1.5,
        "End must scroll the last virtual row into view: row={last:?} panel={panel:?}"
    );
    press(cx, "enter");
    assert_eq!(recorded.borrow().as_slice(), ["Choice 199", "Choice 199"]);
    assert_eq!(
        opens.borrow().as_slice(),
        ["true", "false", "true", "false"]
    );
}

#[gpui::test]
fn autocomplete_virtual_pagedown_derives_its_step_from_the_shown_viewport(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let base = format!("autocomplete-{entity_id}");
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        auto_at(
            320.,
            Autocomplete::new(state_for_view.clone(), auto_options(200))
                .full_width(true)
                .max_items(200)
                .row_height(px(36.))
                .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string())),
        )
    });

    // A capped panel shows fewer than 320px of rows, so one page from the
    // top is shorter than the roomy-window eight-row step.
    settle_select(cx, 640., 480.);
    click(cx, 200., 338.);
    settle_select(cx, 640., 480.);
    let list = cx
        .debug_bounds(combo_probe(format!("{base}-rows")))
        .expect("the virtual list must be laid out");
    let shown = f32::from(list.size.height);
    assert!(
        shown < 320. - 1.5,
        "the capped virtual list must shrink below the upstream maximum, got {list:?}"
    );
    let step = ((shown / 36.).ceil() as usize).saturating_sub(1);
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        [format!("Choice {step:02}")],
        "PageDown must page by the shown viewport, not the 320px maximum"
    );

    // Growing the window grows the viewport, so the same key pages further.
    // The cursor stands on the committed row, so Down steps one past it
    // before the second page.
    settle_select(cx, 640., 1100.);
    click(cx, 200., 338.);
    settle_select(cx, 640., 1100.);
    let list = cx
        .debug_bounds(combo_probe(format!("{base}-rows")))
        .expect("the virtual list must be laid out");
    let grown = f32::from(list.size.height);
    assert!(
        near_px(list.size.height, 320.),
        "the grown virtual list must return to the upstream maximum, got {list:?}"
    );
    assert!(
        grown > shown + 1.5,
        "resizing must grow the virtual viewport: {shown} -> {grown}"
    );
    let grown_step = ((grown / 36.).ceil() as usize).saturating_sub(1);
    assert!(
        grown_step > step,
        "a taller viewport must page further: {step} -> {grown_step}"
    );
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "enter");
    let second = (step + 1 + grown_step).min(199);
    assert_eq!(
        recorded.borrow().as_slice(),
        [format!("Choice {step:02}"), format!("Choice {second:02}")],
        "PageDown after resize must page by the grown viewport"
    );
}

#[gpui::test]
fn autocomplete_filter_reads_labels_and_end_commits_the_last_match(cx: &mut TestAppContext) {
    still();
    let picked = events();
    let recorded = picked.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let base = format!("autocomplete-{entity_id}");
    let state_for_view = state;
    let items = vec![
        PickerItem::new("a", "Alpha"),
        PickerItem::new("b", "Alpine"),
        PickerItem::new("c", "Beta"),
        PickerItem::new("d", "Alphabet"),
    ];

    let cx = open_host(cx, move || {
        let recorded = picked.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, items.clone())
            .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    // The keys carry none of the query text, so only label matching can
    // answer it: "alp" is a substring of Alpha, Alpine and Alphabet, never
    // of Beta.
    click(cx, 60., 18.);
    cx.simulate_input("alp");
    flush_frame(cx);
    assert!(
        cx.debug_bounds(combo_probe(format!("{base}-list-c")))
            .is_none(),
        "a label the query does not match must leave the list"
    );
    assert!(
        cx.debug_bounds(combo_probe(format!("{base}-list-a")))
            .is_some(),
        "a label matching the query must stay in the list"
    );
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a"],
        "forward typing must focus the first label match so Enter commits its key"
    );

    // Reopen on the retained query and take the other end instead: End seats
    // the cursor on the last filtered row and Enter commits its key.
    click(cx, 60., 18.);
    flush_frame(cx);
    press(cx, "end");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a", "d"],
        "End must reach the last filtered row and Enter must commit its key"
    );
}

// ---------------------------------------------------------------------------
// Select row_height
// ---------------------------------------------------------------------------

/// `.list-box-item[data-selected]` is indicator-only in v3: selection does
/// not recolour the option, so a probe in the selected row's indicator slot
/// inherits the same text colour as one in an unselected row. The keyboard
/// cursor rings its row with the shared offset overlay (its `ring-offset`
/// band is a painted quad), and moving the cursor changes no row geometry — a
/// border treatment would. (Long labels wrapping in the trigger and in
/// natural-height rows is `select_long_values_wrap_the_trigger_and_natural_option_row`.)
#[gpui::test]
fn select_option_visuals_follow_the_pinned_indicator_only_contract(cx: &mut TestAppContext) {
    still();
    let selected = harness::style_sink();
    let unselected = harness::style_sink();
    let (sel, other) = (selected.clone(), unselected.clone());
    let cx = open_host(cx, move || {
        let (sel, other) = (sel.clone(), other.clone());
        Select::new("sel-visuals", keyed(&["Alpha", "Beta"]))
            .default_value(Some("Alpha".into()))
            .indicator(move |is_sel| harness::style_probe(if is_sel { &sel } else { &other }))
            .into_any_element()
    });
    click(cx, 60., 18.);
    harness::settle(cx);
    let (sel_color, other_color) = (
        harness::seen_style(&selected, "selected row").color,
        harness::seen_style(&unselected, "unselected row").color,
    );
    assert_eq!(
        sel_color, other_color,
        "a selected Select row must keep the normal row foreground"
    );

    let row = "select-list-Name(\"sel-visuals\")-opt-1";
    let before = cx
        .debug_bounds(row)
        .expect("the second row must be laid out");
    let (bg, gap) = cx.update(|_, cx| {
        use herogpui_theme::ActiveTheme;
        (
            cx.colors().background,
            f32::from(cx.layout().ring_offset_width),
        )
    });
    let idle = harness::painted(cx);
    let idle_rings = idle.ring_gaps(bg, gap).len();
    press(cx, "down");
    press(cx, "down");
    let scene = harness::painted(cx);
    let after = cx
        .debug_bounds(row)
        .expect("the second row must stay laid out");
    assert_eq!(
        before, after,
        "the cursor must not change the row's geometry"
    );
    let rings = scene.ring_gaps(bg, gap);
    let around_row = rings.iter().any(|q| {
        let b = scene.bounds(q);
        (f32::from(b.origin.y) - (f32::from(after.origin.y) - gap)).abs() < 0.5
            && (f32::from(b.size.height) - (f32::from(after.size.height) + 2. * gap)).abs() < 0.5
    });
    assert!(
        around_row && rings.len() > idle_rings,
        "the cursor row must take the shared offset overlay ring\n{}",
        scene.describe()
    );
}
/// Remaining source-text check. The option label is a bare text box with no
/// caller slot inside it, and the headless scene exposes no glyph runs, so
/// whether a fixed-height (virtual) row's label wraps or ellipsizes is not
/// observable; the natural-height wrap is proven on layout by
/// `select_long_values_wrap_the_trigger_and_natural_option_row`.
#[test]
fn picker_option_labels_never_add_a_virtual_row_ellipsis() {
    for (name, file) in [
        ("Select", "select.rs"),
        ("Autocomplete", "autocomplete.rs"),
        ("ComboBox", "combo_box.rs"),
    ] {
        let source = source_scan::component_src(file);
        assert!(
            source.contains(".flex_1().min_w_0().whitespace_normal()"),
            "{name} option labels must keep HeroUI's normal wrapping in every row mode"
        );
        assert!(
            !source.contains("Some(_) => label.truncate()"),
            "{name} must not invent an ellipsis for fixed virtual rows"
        );
    }
}
/// Remaining source-text check. The chevron is an `svg()` glyph rotated by
/// a paint transformation: the test platform ships no asset source (every
/// svg draws nothing) and the scene exposes no sprites or transforms, so
/// which path is drawn and how it turns is not observable. The indicator
/// *slot* geometry is measured in
/// `picker_indicators_sit_in_the_pinned_absolute_end_slot`.
#[test]
fn picker_trigger_chevrons_use_one_rotating_down_svg() {
    let select = source_scan::component_src("select.rs");
    let autocomplete = source_scan::component_src("autocomplete.rs");
    let combo_box = source_scan::component_src("combo_box.rs");
    for (name, source) in [
        ("Select", &select),
        ("Autocomplete", &autocomplete),
        ("ComboBox", &combo_box),
    ] {
        assert!(
            source.contains("rotating_indicator_with_duration")
                && source.contains(".path(icons::CHEVRON_DOWN)"),
            "{name}'s trigger chevron must use the shared rotation helper"
        );
    }
    assert!(
        !select.contains(".path(if is_open") && !autocomplete.contains(".path(if open"),
        "picker triggers must not swap up/down paths as an immediate state change"
    );
}
#[gpui::test]
fn select_custom_trigger_indicator_receives_open_state(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(None));
    let recorded = seen.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        Select::new("sel-trigger-indicator", keyed(&["Alpha", "Beta"]))
            .trigger_indicator(move |is_open| {
                *seen.borrow_mut() = Some(is_open);
                gpui::div().into_any_element()
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(
        *recorded.borrow(),
        Some(true),
        "the custom Select trigger indicator must observe the live open state"
    );
}

/// v3's `ListBox.ItemIndicator` and `Select.Indicator` sit in an absolute
/// 16px slot 8px from the inline end, vertically centred, so a long label
/// can never push them. A 10px probe drawn as the indicator lands centred in
/// that slot: its left edge is `right - 8 - 16 + 3`, whatever the label.
#[gpui::test]
fn picker_indicators_sit_in_the_pinned_absolute_end_slot(cx: &mut TestAppContext) {
    const LONG: &str = "A label long enough to fill the row and wrap onto a second line";
    fn assert_slot(
        what: &str,
        host: gpui::Bounds<gpui::Pixels>,
        probe: gpui::Bounds<gpui::Pixels>,
    ) {
        let want_x = f32::from(host.origin.x + host.size.width) - 8. - 16. + 3.;
        let (host_mid, probe_mid) = (
            f32::from(host.origin.y + host.size.height / 2.),
            f32::from(probe.origin.y + probe.size.height / 2.),
        );
        assert!(
            (f32::from(probe.origin.x) - want_x).abs() < 0.5 && (host_mid - probe_mid).abs() < 0.5,
            "{what}: the indicator must sit centred in the 16px slot 8px from \
             the end, host {host:?}, probe {probe:?}"
        );
    }

    still();
    let cx = open_host(cx, || {
        Select::new("sel-slot", keyed(&["Alpha", LONG]))
            .default_value(Some(LONG.into()))
            .trigger_indicator(|_| part_probe("sel-trigger-ind"))
            .indicator(|is_sel| {
                if is_sel {
                    part_probe("sel-row-ind")
                } else {
                    gpui::div().into_any_element()
                }
            })
            .into_any_element()
    });
    settle_select(cx, 640., 900.);
    let trigger = cx
        .debug_bounds("select-trigger-Name(\"sel-slot\")")
        .expect("the trigger must be laid out");
    assert_slot(
        "Select trigger",
        trigger,
        cx.debug_bounds("sel-trigger-ind").unwrap(),
    );
    click(cx, 60., 18.);
    settle_select(cx, 640., 900.);
    let row = cx
        .debug_bounds("select-list-Name(\"sel-slot\")-opt-1")
        .expect("the long row must be laid out");
    assert_slot("Select row", row, cx.debug_bounds("sel-row-ind").unwrap());

    let state = search_state(cx);
    let id = state.entity_id().as_u64();
    let cx = open_host(cx, move || {
        combo_at(
            40.,
            ComboBox::new(state.clone(), keyed(&["Alpha", LONG]))
                .default_value([LONG])
                .default_open(true)
                .indicator(|is_sel| {
                    if is_sel {
                        part_probe("combo-row-ind")
                    } else {
                        gpui::div().into_any_element()
                    }
                }),
        )
    });
    settle_select(cx, 640., 900.);
    let row = cx
        .debug_bounds(combo_probe(format!("combobox-{id}-item-{LONG}")))
        .expect("the ComboBox row must be laid out");
    assert_slot(
        "ComboBox row",
        row,
        cx.debug_bounds("combo-row-ind").unwrap(),
    );

    let state = search_state(cx);
    let id = state.entity_id().as_u64();
    let cx = open_host(cx, move || {
        auto_at(
            40.,
            Autocomplete::new(state.clone(), keyed(&["Alpha", LONG]))
                .default_value([LONG])
                .default_open(true)
                .item_indicator(|is_sel| {
                    if is_sel {
                        part_probe("auto-row-ind")
                    } else {
                        gpui::div().into_any_element()
                    }
                }),
        )
    });
    settle_select(cx, 640., 900.);
    let row = cx
        .debug_bounds(combo_probe(format!("autocomplete-{id}-list-{LONG}")))
        .expect("the Autocomplete row must be laid out");
    assert_slot(
        "Autocomplete row",
        row,
        cx.debug_bounds("auto-row-ind").unwrap(),
    );
}
/// HeroUI's Select value and natural-height list rows use `wrap-break-word`.
/// Keep a long label visible in both places instead of silently clipping it
/// with the one-line trigger/row ellipsis used by the virtualized path.
#[gpui::test]
fn select_long_values_wrap_the_trigger_and_natural_option_row(cx: &mut TestAppContext) {
    still();
    let long =
        "A project name that is intentionally long enough to wrap across multiple trigger lines";
    let cx = open_host(cx, move || {
        Select::new(
            "sel-wrap",
            vec![
                PickerItem::new("long", long),
                PickerItem::new("short", "Short option"),
            ],
        )
        .label("Project")
        .default_value(Some("long".into()))
        .default_open(true)
        .into_any_element()
    });
    flush_frame(cx);

    let trigger = cx
        .debug_bounds("select-trigger-Name(\"sel-wrap\")")
        .expect("the selected value trigger must be laid out");
    assert!(
        f32::from(trigger.size.height) > f32::from(herogpui_components::extend::FIELD_HEIGHT) + 8.,
        "a long selected value must wrap and grow the trigger, got {trigger:?}"
    );

    let option = cx
        .debug_bounds("select-list-Name(\"sel-wrap\")-opt-0")
        .expect("the long option must be laid out");
    assert!(
        f32::from(option.size.height) > f32::from(herogpui_components::extend::FIELD_HEIGHT) + 8.,
        "a natural-height long option must wrap instead of truncating, got {option:?}"
    );
}

/// HeroUI's Autocomplete trigger value uses `wrap-break-word`, so a selected
/// label grows the field instead of being clipped by a one-line ellipsis.
#[gpui::test]
fn autocomplete_long_selected_value_wraps_the_trigger(cx: &mut TestAppContext) {
    still();
    let long = "A project name that is intentionally long enough to wrap";
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let base = format!("autocomplete-{entity_id}");
    let state_for_view = state;
    let cx = open_host(cx, move || {
        auto_at(
            100.,
            Autocomplete::new(
                state_for_view.clone(),
                vec![
                    PickerItem::new("long", long),
                    PickerItem::new("short", "Short option"),
                ],
            )
            .default_value(["long"])
            .label("Project")
            .placeholder("Choose one"),
        )
    });
    settle_select(cx, 640., 900.);

    let trigger = cx
        .debug_bounds(auto_trigger(&base))
        .expect("the Autocomplete trigger must be laid out");
    assert!(
        f32::from(trigger.size.height) > f32::from(herogpui_components::extend::FIELD_HEIGHT) + 8.,
        "a long selected Autocomplete value must wrap and grow the trigger, got {trigger:?}"
    );
}

/// HeroUI's optional ComboBox.Value slot uses `wrap-break-word`; the default
/// render-prop children must grow the value row instead of clipping it.
#[gpui::test]
fn combo_box_long_value_content_wraps_the_value_row(cx: &mut TestAppContext) {
    still();
    let long = "A project name that is intentionally long enough to wrap";
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        combo_at(
            100.,
            ComboBox::new(
                state_for_view.clone(),
                vec![
                    PickerItem::new("long", long),
                    PickerItem::new("short", "Short option"),
                ],
            )
            .default_value(["long"])
            .label("Project")
            .placeholder("Choose one")
            .value_content(|value| value.default_children),
        )
    });
    settle_select(cx, 640., 900.);

    let value = cx
        .debug_bounds(combo_probe(format!("combobox-value-{entity_id}")))
        .expect("the ComboBox.Value slot must be laid out");
    assert!(
        f32::from(value.size.height) > 20.,
        "a long ComboBox value must wrap and grow the value row, got {value:?}"
    );
}

/// `Select::row_height` is the virtualized row box, and the option inside it
/// must be that tall too. It carried `min_h(util::FIELD_HEIGHT)` regardless,
/// so a row shorter than 36px was a 36px option overflowing a 28px row.
///
/// Measured, not asserted against itself: the two rows' own painted bounds
/// must step by the requested height and neither may exceed it.
#[gpui::test]
fn select_row_height_sizes_the_option_inside_the_row(cx: &mut TestAppContext) {
    still();
    const ROW: f32 = 28.;
    let cx = open_host(cx, move || {
        Select::new("sel-rh", keyed(&["One", "Two", "Three"]))
            .row_height(px(ROW))
            .into_any_element()
    });
    click(cx, 60., 18.);
    flush_frame(cx);

    let first = cx
        .debug_bounds("select-list-Name(\"sel-rh\")-opt-0")
        .expect("the first option must be laid out");
    let second = cx
        .debug_bounds("select-list-Name(\"sel-rh\")-opt-1")
        .expect("the second option must be laid out");
    assert!(
        near_px(second.origin.y - first.origin.y, ROW),
        "the virtual rows must step by the requested row height: first={first:?} second={second:?}"
    );
    for (name, row) in [("first", first), ("second", second)] {
        assert!(
            f32::from(row.size.height) <= ROW + 0.5,
            "the {name} option must not be taller than its {ROW}px row, got {row:?}"
        );
    }
}

/// The default (no `row_height`) list is unchanged: its options keep the 36px
/// `.list-box-item` floor.
#[gpui::test]
fn select_without_row_height_keeps_the_thirty_six_pixel_option_floor(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        Select::new("sel-rh-default", keyed(&["One", "Two"])).into_any_element()
    });
    click(cx, 60., 18.);
    flush_frame(cx);

    let first = cx
        .debug_bounds("select-list-Name(\"sel-rh-default\")-opt-0")
        .expect("the first option must be laid out");
    assert!(
        near_px(
            first.size.height,
            f32::from(herogpui_components::extend::FIELD_HEIGHT)
        ),
        "a plain option must keep the field-height floor, got {first:?}"
    );
}

// ---------------------------------------------------------------------------
// Collection trigger box geometry and row padding
// ---------------------------------------------------------------------------

/// `Select::height` replaces the trigger's 36px `min-h-9` box.
#[gpui::test]
fn select_trigger_height_replaces_the_min_height(cx: &mut TestAppContext) {
    still();
    const SHORT: f32 = 28.;
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                gpui::div()
                    .debug_selector(|| "sel-h-default".to_owned())
                    .child(Select::new("sel-h-default", keyed(&["One", "Two"]))),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "sel-h-short".to_owned())
                    .child(Select::new("sel-h-short", keyed(&["One", "Two"])).height(px(SHORT))),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let default = cx
        .debug_bounds("select-trigger-Name(\"sel-h-default\")")
        .expect("the default trigger must be laid out");
    let short = cx
        .debug_bounds("select-trigger-Name(\"sel-h-short\")")
        .expect("the short trigger must be laid out");
    assert!(
        near_px(
            default.size.height,
            f32::from(herogpui_components::extend::FIELD_HEIGHT)
        ),
        "an untouched trigger must keep the 36px floor, got {default:?}"
    );
    assert!(
        near_px(short.size.height, SHORT),
        "`height` must replace the trigger's floor, got {short:?}"
    );
}

/// `Select::row_padding_y` replaces the rows' `py-1.5`: with the 20px line,
/// a 12px vertical padding makes rows 44px instead of the 36px floor.
#[gpui::test]
fn select_row_padding_y_sizes_the_option_box(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        Select::new("sel-pad", keyed(&["One", "Two"]))
            .row_padding_y(px(12.))
            .into_any_element()
    });
    click(cx, 60., 18.);
    flush_frame(cx);

    let first = cx
        .debug_bounds("select-list-Name(\"sel-pad\")-opt-0")
        .expect("the first option must be laid out");
    assert!(
        near_px(first.size.height, 44.),
        "a 12px row padding must grow the 20px line to 44px, got {first:?}"
    );
}

/// `ComboBox::height` reaches the inner Input, which is the trigger box.
#[gpui::test]
fn combo_box_height_reaches_the_inner_field(cx: &mut TestAppContext) {
    still();
    const SHORT: f32 = 28.;
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        combo_at(
            40.,
            ComboBox::new(state_for_view.clone(), combo_options(3)).height(px(SHORT)),
        )
    });
    flush_frame(cx);

    let field = cx
        .debug_bounds(combo_probe(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    assert!(
        near_px(field.size.height, SHORT),
        "`height` must reach the inner field, got {field:?}"
    );
}

/// `Autocomplete::height` replaces the trigger's 36px `min-h-9` box; the
/// label-to-description wrapper above and below it does not move.
#[gpui::test]
fn autocomplete_trigger_height_replaces_the_min_height(cx: &mut TestAppContext) {
    still();
    const SHORT: f32 = 28.;
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let base = format!("autocomplete-{entity_id}");
    let state_for_view = state;
    let cx = open_host(cx, move || {
        auto_at(
            100.,
            Autocomplete::new(state_for_view.clone(), auto_options(3))
                .height(px(SHORT))
                .label("Language")
                .description("Pick one"),
        )
    });
    settle_select(cx, 640., 900.);

    let trigger = cx
        .debug_bounds(auto_trigger(&base))
        .expect("the trigger must be laid out");
    assert!(
        near_px(trigger.size.height, SHORT),
        "`height` must replace the trigger's floor, got {trigger:?}"
    );
}

/// The collection triggers either gate their one chrome call site on the bare
/// flag (Select, Autocomplete) or forward it to the inner Input (ComboBox).
/// The collection triggers either gate their one chrome call site on the bare
/// flag (Select, Autocomplete) or forward it to the inner Input (ComboBox):
/// a bare trigger paints no field fill, the stock one does.
#[gpui::test]
fn collection_triggers_gate_or_forward_the_bare_flag(cx: &mut TestAppContext) {
    for bare in [false, true] {
        let combo = search_state(cx);
        let auto = search_state(cx);
        still();
        let cx = open_host(cx, move || {
            gpui::div()
                .flex()
                .flex_col()
                .gap(px(40.))
                .w(px(320.))
                .child(Select::new("sel-bare", keyed(&["Alpha"])).is_bare(bare))
                .child(ComboBox::new(combo.clone(), keyed(&["Alpha"])).is_bare(bare))
                .child(Autocomplete::new(auto.clone(), keyed(&["Alpha"])).is_bare(bare))
                .into_any_element()
        });
        let scene = harness::painted(cx);
        let field = cx.update(|_, cx| {
            use herogpui_theme::ActiveTheme;
            cx.colors().field.background
        });
        let boxes = scene
            .filled(field)
            .into_iter()
            .filter(|q| (f32::from(scene.bounds(q).size.width) - 320.).abs() < 0.5)
            .count();
        assert_eq!(
            boxes,
            if bare { 0 } else { 3 },
            "bare={bare}: each trigger paints its field chrome unless bare\n{}",
            scene.describe()
        );
    }
}
/// `value_content` lands inside the trigger, so a probe there measures the
/// trigger's leading padding: `padding_x` must move it by the requested delta.
#[gpui::test]
fn collection_trigger_padding_x_moves_the_value(cx: &mut TestAppContext) {
    still();
    const WIDE: f32 = 24.;
    let auto_default = search_state(cx);
    let auto_wide = search_state(cx);
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(8.))
            .child(
                Select::new("sel-trig-pad-default", keyed(&["One"]))
                    .value_content(|_| part_probe("sel-trig-pad-default-probe")),
            )
            .child(
                Select::new("sel-trig-pad-wide", keyed(&["One"]))
                    .padding_x(px(WIDE))
                    .value_content(|_| part_probe("sel-trig-pad-wide-probe")),
            )
            .child(
                Autocomplete::new(auto_default.clone(), keyed(&["One"]))
                    .value_content(|_| part_probe("auto-trig-pad-default-probe")),
            )
            .child(
                Autocomplete::new(auto_wide.clone(), keyed(&["One"]))
                    .padding_x(px(WIDE))
                    .value_content(|_| part_probe("auto-trig-pad-wide-probe")),
            )
            .into_any_element()
    });
    flush_frame(cx);

    for (default, wide) in [
        ("sel-trig-pad-default-probe", "sel-trig-pad-wide-probe"),
        ("auto-trig-pad-default-probe", "auto-trig-pad-wide-probe"),
    ] {
        let default = cx
            .debug_bounds(default)
            .expect("the default value must paint");
        let wide = cx.debug_bounds(wide).expect("the wide value must paint");
        let delta = f32::from(wide.origin.x - default.origin.x);
        assert!(
            (delta - (WIDE - 12.)).abs() < 0.5,
            "`padding_x` must move the value by the delta over the default \
             12px inset, moved {delta}: default={default:?} wide={wide:?}"
        );
    }
}

/// The pinned item indicator is absolutely anchored eight pixels from the row
/// edge, so changing row `padding_x` changes the label inset without moving
/// the marker on either the plain or virtual path.
#[gpui::test]
fn collection_row_padding_x_moves_the_indicator(cx: &mut TestAppContext) {
    still();
    const WIDE: f32 = 24.;
    let combo_default = search_state(cx);
    let combo_wide = search_state(cx);
    let auto_default = search_state(cx);
    let auto_wide = search_state(cx);
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(8.))
            .child(
                Select::new("sel-row-pad-default", keyed(&["One"]))
                    .default_open(true)
                    .indicator(|_| part_probe("sel-row-pad-default-tick")),
            )
            .child(
                Select::new("sel-row-pad-wide", keyed(&["One"]))
                    .default_open(true)
                    .row_padding_x(px(WIDE))
                    .indicator(|_| part_probe("sel-row-pad-wide-tick")),
            )
            .child(
                ComboBox::new(combo_default.clone(), keyed(&["One"]))
                    .default_open(true)
                    .indicator(|_| part_probe("combo-row-pad-default-tick")),
            )
            .child(
                ComboBox::new(combo_wide.clone(), keyed(&["One"]))
                    .default_open(true)
                    .row_padding_x(px(WIDE))
                    .indicator(|_| part_probe("combo-row-pad-wide-tick")),
            )
            .child(
                Autocomplete::new(auto_default.clone(), keyed(&["One"]))
                    .default_open(true)
                    .item_indicator(|_| part_probe("auto-row-pad-default-tick")),
            )
            .child(
                Autocomplete::new(auto_wide.clone(), keyed(&["One"]))
                    .default_open(true)
                    .row_padding_x(px(WIDE))
                    .item_indicator(|_| part_probe("auto-row-pad-wide-tick")),
            )
            .into_any_element()
    });
    settle_select(cx, 640., 900.);

    for (default, wide) in [
        ("sel-row-pad-default-tick", "sel-row-pad-wide-tick"),
        ("combo-row-pad-default-tick", "combo-row-pad-wide-tick"),
        ("auto-row-pad-default-tick", "auto-row-pad-wide-tick"),
    ] {
        let default = cx
            .debug_bounds(default)
            .expect("the default row indicator must paint");
        let wide = cx
            .debug_bounds(wide)
            .expect("the wide row indicator must paint");
        let moved = f32::from(default.origin.x - wide.origin.x);
        assert!(
            moved.abs() < 0.5,
            "the absolute indicator must stay at the row edge when `row_padding_x` \
             changes, moved {moved}: default={default:?} wide={wide:?}"
        );
    }
}

/// The virtual paths (`row_height` set) apply the same row padding; the plain
/// fixtures above omit `row_height`, so this pins the uniform/list paths.
#[gpui::test]
fn collection_virtual_row_padding_x_moves_the_indicator(cx: &mut TestAppContext) {
    still();
    const WIDE: f32 = 24.;
    let combo_default = search_state(cx);
    let combo_wide = search_state(cx);
    let auto_default = search_state(cx);
    let auto_wide = search_state(cx);
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(8.))
            .child(
                Select::new("sel-vrow-pad-default", keyed(&["One"]))
                    .default_open(true)
                    .row_height(px(40.))
                    .indicator(|_| part_probe("sel-vrow-pad-default-tick")),
            )
            .child(
                Select::new("sel-vrow-pad-wide", keyed(&["One"]))
                    .default_open(true)
                    .row_height(px(40.))
                    .row_padding_x(px(WIDE))
                    .indicator(|_| part_probe("sel-vrow-pad-wide-tick")),
            )
            .child(
                ComboBox::new(combo_default.clone(), keyed(&["One"]))
                    .default_open(true)
                    .row_height(px(40.))
                    .indicator(|_| part_probe("combo-vrow-pad-default-tick")),
            )
            .child(
                ComboBox::new(combo_wide.clone(), keyed(&["One"]))
                    .default_open(true)
                    .row_height(px(40.))
                    .row_padding_x(px(WIDE))
                    .indicator(|_| part_probe("combo-vrow-pad-wide-tick")),
            )
            .child(
                Autocomplete::new(auto_default.clone(), keyed(&["One"]))
                    .default_open(true)
                    .row_height(px(40.))
                    .item_indicator(|_| part_probe("auto-vrow-pad-default-tick")),
            )
            .child(
                Autocomplete::new(auto_wide.clone(), keyed(&["One"]))
                    .default_open(true)
                    .row_height(px(40.))
                    .row_padding_x(px(WIDE))
                    .item_indicator(|_| part_probe("auto-vrow-pad-wide-tick")),
            )
            .into_any_element()
    });
    settle_select(cx, 640., 900.);

    for (default, wide) in [
        ("sel-vrow-pad-default-tick", "sel-vrow-pad-wide-tick"),
        ("combo-vrow-pad-default-tick", "combo-vrow-pad-wide-tick"),
        ("auto-vrow-pad-default-tick", "auto-vrow-pad-wide-tick"),
    ] {
        let default = cx
            .debug_bounds(default)
            .expect("the default virtual row indicator must paint");
        let wide = cx
            .debug_bounds(wide)
            .expect("the wide virtual row indicator must paint");
        let moved = f32::from(default.origin.x - wide.origin.x);
        assert!(
            moved.abs() < 0.5,
            "the absolute virtual indicator must stay at the row edge when \
             `row_padding_x` changes, moved {moved}: default={default:?} \
             wide={wide:?}"
        );
    }
}

/// `row_padding_y` grows plain rows on the two remaining picker owners: the
/// 20px line plus twice the padding, with the defaults keeping the 36px floor.
#[gpui::test]
fn combo_box_row_padding_y_sizes_the_rows(cx: &mut TestAppContext) {
    still();
    const PAD: f32 = 12.;
    let default_state = search_state(cx);
    let padded_state = search_state(cx);
    let default_id = default_state.entity_id().as_u64();
    let padded_id = padded_state.entity_id().as_u64();
    let cx = open_host(cx, move || {
        gpui::div()
            .size_full()
            .child(combo_at(
                40.,
                ComboBox::new(default_state.clone(), keyed(&["One"])).default_open(true),
            ))
            .child(combo_at(
                120.,
                ComboBox::new(padded_state.clone(), keyed(&["One"]))
                    .default_open(true)
                    .row_padding_y(px(PAD)),
            ))
            .into_any_element()
    });
    settle_select(cx, 640., 900.);

    let default = cx
        .debug_bounds(combo_probe(format!("combobox-{default_id}-item-One")))
        .expect("the default row must be laid out");
    let padded = cx
        .debug_bounds(combo_probe(format!("combobox-{padded_id}-item-One")))
        .expect("the padded row must be laid out");
    assert!(
        near_px(default.size.height, 36.),
        "the default row must keep the 36px floor, got {default:?}"
    );
    assert!(
        near_px(padded.size.height, 44.),
        "a 12px row padding must grow the 20px line to 44px, got {padded:?}"
    );
}

/// `row_padding_y` grows an Autocomplete suggestion the same way; the single
/// control avoids two open popovers competing for the focus.
#[gpui::test]
fn autocomplete_row_padding_y_sizes_the_suggestion(cx: &mut TestAppContext) {
    still();
    for (pad, expected) in [(None, 36.), (Some(12.), 44.)] {
        let state = search_state(cx);
        let entity_id = state.entity_id().as_u64();
        let base = format!("autocomplete-{entity_id}");
        let state_for_view = state;
        let cx = open_host(cx, move || {
            let mut auto = Autocomplete::new(state_for_view.clone(), auto_options(3))
                .full_width(true)
                .label("Language");
            if let Some(pad) = pad {
                auto = auto.row_padding_y(px(pad));
            }
            auto_at(100., auto)
        });
        settle_select(cx, 640., 900.);

        let trigger = cx
            .debug_bounds(auto_trigger(&base))
            .expect("the trigger must be laid out");
        click(cx, 200., f32::from(trigger.center().y));
        settle_select(cx, 640., 900.);
        let row = cx
            .debug_bounds(combo_probe(format!("{base}-list-Choice 00")))
            .expect("the suggestion must be laid out");
        let message = format!("row padding {pad:?} must size the suggestion");
        assert!(near_px(row.size.height, expected), "{message}, got {row:?}");
    }
}

/// `useSelect`/`useOption` run under `shouldFocusOnHover`: a pointer resting
/// on a row seats the virtual cursor the next arrow continues from. Real
/// focus stays on the trigger — only the cursor moves — and Enter still
/// commits the row it lands on.
#[gpui::test]
fn select_hover_seats_the_cursor_for_the_next_arrow(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Select::new(
            "sel-hover-cursor",
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .on_change(move |key, _, _| changes.borrow_mut().push(format!("{key:?}")))
        .into_any_element()
    });

    click(cx, 60., 18.);
    harness::settle(cx);
    let row = cx
        .debug_bounds(r#"select-list-Name("sel-hover-cursor")-opt-2"#)
        .expect("the third option row must be laid out");
    cx.simulate_mouse_move(row.center(), None, Modifiers::none());
    // Down walks from the hovered row to the fourth; Enter reports Delta.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Some(\"Delta\")"],
        "hover must seat the cursor so the next arrow continues from it"
    );
}

/// Once a key has run (`focus_visible`), the same hover must not steal the
/// cursor: `useOption.onHoverStart`'s guard keeps it where the arrows left
/// it, so the next Down continues from the keyboard position.
#[gpui::test]
fn select_hover_keeps_a_keyboard_seated_cursor(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Select::new(
            "sel-hover-keyboard",
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .on_change(move |key, _, _| changes.borrow_mut().push(format!("{key:?}")))
        .into_any_element()
    });

    click(cx, 60., 18.);
    harness::settle(cx);
    // One Down seats the cursor on Alpha (row 0) and turns on focus-visible.
    press(cx, "down");
    // Resting the pointer on Gamma must not move it.
    let row = cx
        .debug_bounds(r#"select-list-Name("sel-hover-keyboard")-opt-2"#)
        .expect("the third option row must be laid out");
    cx.simulate_mouse_move(row.center(), None, Modifiers::none());
    // Down continues from Alpha to Beta; Enter reports Beta — had the hover
    // seated Gamma, the same keys would have reported Delta.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Some(\"Beta\")"],
        "a keyboard session's cursor must survive pointer hover"
    );
}

/// Select's popover is modal in React Aria: it renders a full-window
/// underlay, so the outside press that closes the list is consumed and must
/// not reach the control underneath. The sibling button stays unclicked
/// while the open select dismisses.
#[gpui::test]
fn select_outside_press_does_not_reach_the_sibling(cx: &mut TestAppContext) {
    let clicks = events();
    let clicked = clicks.clone();
    let cx = open_host(cx, move || {
        let clicks = clicks.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                gpui::div()
                    .id("sel-outside-btn")
                    .w(px(140.))
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .debug_selector(|| "sel-outside-btn".to_owned())
                    .child(
                        Button::new("sel-outside-inner")
                            .label("Outside")
                            .on_press(move |_, _, _| clicks.borrow_mut().push("click".into())),
                    )
                    .into_any_element(),
            )
            .child(
                Select::new("sel-modal", keyed(&["Alpha", "Beta"]))
                    .default_open(true)
                    .into_any_element(),
            )
            .into_any_element()
    });
    harness::settle(cx);

    assert!(
        cx.debug_bounds(r#"select-list-Name("sel-modal")-opt-0"#)
            .is_some(),
        "the select's panel must start open"
    );

    let button = cx
        .debug_bounds("sel-outside-btn")
        .expect("the sibling button must be laid out");
    cx.simulate_click(button.center(), Modifiers::none());
    harness::settle(cx);
    // The dismissed panel keeps its exit frame for the close motion; let it
    // finish before asserting it is gone.
    harness::wait_ms(cx, 200);

    assert!(
        clicked.borrow().is_empty(),
        "the modal underlay must consume the press: the button never sees it"
    );
    assert!(
        cx.debug_bounds(r#"select-list-Name("sel-modal")-opt-0"#)
            .is_none(),
        "the same press still dismisses the open select"
    );
}

/// The gallery's controlled Select: the parent owns `open` and feeds it back
/// through `is_open`/`on_open_change`. A pointer click on the trigger must
/// run the whole path -- trigger click, open callback, re-render, panel
/// layout -- not just flip an internal flag.
#[gpui::test]
fn select_controlled_trigger_click_opens_the_panel(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let open_flag = Rc::new(RefCell::new(false));

    let cx = open_host(cx, move || {
        let opens = opens.clone();
        let is_open = *open_flag.borrow();
        Select::new("sel-controlled-open", keyed(&["Alpha", "Beta", "Gamma"]))
            .is_open(is_open)
            .on_open_change({
                let open_flag = open_flag.clone();
                move |v, window, _| {
                    *open_flag.borrow_mut() = *v;
                    opens.borrow_mut().push(format!("open:{v}"));
                    window.refresh();
                }
            })
            .into_any_element()
    });
    harness::settle(cx);

    let trigger = cx
        .debug_bounds(r#"select-trigger-Name("sel-controlled-open")"#)
        .expect("the trigger must be laid out");
    cx.simulate_click(trigger.center(), Modifiers::none());
    harness::settle(cx);

    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the trigger click must report the open through on_open_change"
    );
    assert!(
        cx.debug_bounds(r#"select-list-Name("sel-controlled-open")-opt-0"#)
            .is_some(),
        "the controlled panel must lay out a row after the click"
    );
}

/// `useComboBox`/`useOption` run under `shouldFocusOnHover` as well: pointer
/// hover seats the virtual cursor for the next arrow, while the input keeps
/// real focus and Enter commits the row it lands on.
#[gpui::test]
fn combo_box_hover_seats_the_cursor_for_the_next_arrow(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        ComboBox::new(
            state_for_view.clone(),
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
        .into_any_element()
    });

    click(cx, 60., 18.);
    harness::settle(cx);
    let row = cx
        .debug_bounds(combo_probe(format!("combobox-{entity_id}-item-Gamma")))
        .expect("the Gamma row must be laid out");
    cx.simulate_mouse_move(row.center(), None, Modifiers::none());
    // Down walks from the hovered row to Delta; Enter reports it.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Delta"],
        "hover must seat the cursor so the next arrow continues from it"
    );
}

/// The ComboBox guard mirrors Select's: a keyboard-established cursor is not
/// moved by pointer hover, so the next arrow continues from the keyboard row.
#[gpui::test]
fn combo_box_hover_keeps_a_keyboard_seated_cursor(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        ComboBox::new(
            state_for_view.clone(),
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
        .into_any_element()
    });

    click(cx, 60., 18.);
    harness::settle(cx);
    // One Down seats the cursor on Alpha and turns on focus-visible.
    press(cx, "down");
    // Resting the pointer on Gamma must not move it.
    let row = cx
        .debug_bounds(combo_probe(format!("combobox-{entity_id}-item-Gamma")))
        .expect("the Gamma row must be laid out");
    cx.simulate_mouse_move(row.center(), None, Modifiers::none());
    // Down continues from Alpha to Beta; Enter reports Beta — had the hover
    // seated Gamma, the same keys would have reported Delta.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Beta"],
        "a keyboard session's cursor must survive pointer hover"
    );
}

/// The Autocomplete guard mirrors ComboBox's and Select's: a pointer hover
/// over an enabled row seats the list cursor, so the next arrow continues from
/// the hovered row rather than from wherever the cursor last sat.
#[gpui::test]
fn autocomplete_hover_seats_the_cursor_for_the_next_arrow(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Autocomplete::new(
            state_for_view.clone(),
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
        .into_any_element()
    });

    click(cx, 60., 18.);
    flush_frame(cx);
    let row = cx
        .debug_bounds(combo_probe(format!("autocomplete-{entity_id}-list-Gamma")))
        .expect("the Gamma row must be laid out");
    cx.simulate_mouse_move(row.center(), None, Modifiers::none());
    // Down walks from the hovered row to Delta; Enter reports it.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Delta"],
        "hover must seat the cursor so the next arrow continues from it"
    );
}

/// The Autocomplete guard mirrors Select's and ComboBox's: a keyboard-
/// established cursor is not moved by pointer hover, so the next arrow
/// continues from the keyboard row.
#[gpui::test]
fn autocomplete_hover_keeps_a_keyboard_seated_cursor(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Autocomplete::new(
            state_for_view.clone(),
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
        .into_any_element()
    });

    click(cx, 60., 18.);
    flush_frame(cx);
    // One Down seats the cursor on Alpha and turns on focus-visible.
    press(cx, "down");
    // Resting the pointer on Gamma must not move it.
    let row = cx
        .debug_bounds(combo_probe(format!("autocomplete-{entity_id}-list-Gamma")))
        .expect("the Gamma row must be laid out");
    cx.simulate_mouse_move(row.center(), None, Modifiers::none());
    // Down continues from Alpha to Beta; Enter reports Beta — had the hover
    // seated Gamma, the same keys would have reported Delta.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Beta"],
        "a keyboard session's cursor must survive pointer hover"
    );
}

/// A typeahead match in an open Select must scroll its row into view, the way
/// React Aria keeps the focused key visible. The list here is taller than the
/// capped panel, so the matched last row starts below the fold; one letter
/// finds it by its unique initial and the reveal must bring it on screen.
#[gpui::test]
fn select_typeahead_scrolls_the_found_row_into_view(cx: &mut TestAppContext) {
    still();
    let page_scroll = gpui::ScrollHandle::new();
    let page_for_view = page_scroll;
    let cx = open_host(cx, move || {
        let page_scroll = page_for_view.clone();
        // Thirteen plain rows plus a last row with a unique initial, so a
        // single letter resolves to it unambiguously and it starts below the
        // capped panel.
        let mut options: Vec<PickerItem> = (0..13)
            .map(|i| {
                let label = format!("Row {i:02}");
                PickerItem::new(label.clone(), label)
            })
            .collect();
        let last = "Zebra Final".to_owned();
        options.push(PickerItem::new(last.clone(), last));
        select_in_page(408., Select::new("sel-ta-scroll", options), page_scroll)
    });
    settle_select(cx, 640., 480.);
    click(cx, 200., 426.);
    settle_select(cx, 640., 480.);

    let panel = cx
        .debug_bounds("select-list-Name(\"sel-ta-scroll\")-panel")
        .expect("the open panel must be laid out");
    let hidden = cx
        .debug_bounds("select-list-Name(\"sel-ta-scroll\")-opt-13")
        .expect("the last row must be laid out");
    assert!(
        f32::from(hidden.top()) >= f32::from(panel.bottom()),
        "the last row must start out of view in a capped panel: row={hidden:?} panel={panel:?}"
    );

    // "z" typeaheads to the last row; the reveal must scroll it into view.
    press(cx, "z");
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let revealed = cx
        .debug_bounds("select-list-Name(\"sel-ta-scroll\")-opt-13")
        .expect("the last row must still be laid out");
    assert!(
        f32::from(revealed.top()) < f32::from(panel.bottom())
            && f32::from(revealed.bottom()) > f32::from(panel.top()),
        "typeahead must scroll the found row into the panel: row={revealed:?} panel={panel:?}"
    );
}

/// A Shift+letter typeahead is `useTypeSelect`'s `setFocusedKey`: it moves the
/// cursor but must not extend a multiple selection the way Shift+Arrow does.
/// Selecting Alpha first seats the anchor; Shift+D must move the cursor to
/// Delta without reporting a range, and the next Enter then toggles Delta on
/// its own — proof the cursor really sat on Delta.
#[gpui::test]
fn select_multiple_shift_letter_typeahead_moves_without_extending(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new("sel-shift-ta", keyed(&["Alpha", "Beta", "Gamma", "Delta"]))
            .selection_mode(SelectionMode::Multiple)
            .default_open(true)
            .on_selection_change_all(move |keys, _, _| {
                picks.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    // Tab reaches the trigger, Down seats the cursor on Alpha, and Enter
    // toggles it into the selection and leaves the anchor there.
    press(cx, "tab down enter");
    assert_eq!(picked.borrow().as_slice(), ["Alpha"]);

    // Shift+D typeaheads to Delta. A real arrow extension would report the
    // whole Alpha..Delta range; a typeahead move must report nothing.
    press(cx, "shift-d");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha"],
        "Shift+letter typeahead must move the cursor without extending the selection"
    );

    // Enter toggles the row the cursor landed on: Delta joins Alpha alone, not
    // the range a Shift extension would have selected.
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Alpha,Delta"],
        "the typeahead cursor must sit on Delta for the next Enter"
    );
}
