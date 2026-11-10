//! Behaviour tests for the three search-and-select components: Autocomplete,
//! ComboBox and Dropdown, plus the DatePicker's calendar grid.
//!
//! Everything static about them is measured by the `.shots/*.py` audits; these
//! tests drive the controls and assert on recorded callbacks and behavioural
//! probes only — never on appearance.
//!
//! Geometry is derived from the components' own constants, not guessed:
//!
//! - Picker trigger fields are 36px rows (`util::FIELD_HEIGHT`). Select,
//!   ComboBox and Autocomplete open from the field at (60, 18); DatePicker
//!   composes an editable DateField plus a separate 24px trigger whose centre
//!   is (124, 18).
//! - The pickers' panels hang from `placed_field_panel(BottomStart, 8px)`:
//!   top = trigger bottom + 8 = 44.
//! - Autocomplete: panel `pt(8)` + search wrapper `py(4)` + 36px field + list
//!   `p(6)` puts row *i* at y 100+36i, plus up to 6px of entry-zoom padding
//!   (`ZoomBox::panel(px(6))`); clicking y = 124+36i lands inside every phase
//!   of that animation.
//! - ComboBox: panel `p(4)` puts row *i* at y 46+36i with ≤4px of zoom
//!   padding; clicking y = 64+36i covers it.
//! - Dropdown: same panel shape as Select (row centres y 64+36i), but both
//!   menus here are driven by keyboard, so only their triggers are clicked.
//! - DatePicker: the cell band starts at 44 + 12 (`picker_panel` padding) +
//!   24 (nav header) + 8 + 8 (calendar gaps) + one text line of weekday
//!   header, and cells are 36px tall; the column centres come from
//!   `CALENDAR_WIDTH` split into seven equal columns. Only the weekday
//!   line height is a text metric rather than a constant, which the chosen y
//!   tolerates by ±14px either way.
//!
//! Reduce motion is deliberately **not** set for this process. Only the
//! Dropdown and DatePicker family retain their panels through
//! `util::overlay_phase` exits; no test here probes those surfaces after
//! dismissing them, while the Autocomplete and ComboBox panels leave the tree
//! outright when closed.

use crate::harness;

use std::{cell::RefCell, rc::Rc};

use gpui::{prelude::*, px, Context, Render, SharedString, TestAppContext, Window};
use herogpui_components::{
    calendar::{CalendarState, Date, CALENDAR_WIDTH},
    Autocomplete, Button, ComboBox, DateConstraints, DatePicker, Dropdown, InputState, MenuItem,
    MenuTrigger, PickerItem, SelectionMode,
};

use harness::{click, events, open_host, press};

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

#[gpui::test]
fn autocomplete_opens_filters_and_selects(cx: &mut TestAppContext) {
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
        // v3's Autocomplete is a trigger whose popover holds a SearchField;
        // typing filters, clicking a row selects.
        Autocomplete::new(state, keyed(&["Typst", "Rust", "Go"]))
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
        "the trigger must open"
    );

    // The popover autofocuses its search field, so typing goes straight into
    // it and filters the rows down to the one match.
    cx.simulate_input("ty");
    click(cx, 60., 124.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Typst"],
        "clicking the matching row must record its text"
    );
    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);

    // Closed proof by behaviour: the same spot is bare page below the trigger
    // now, so the press must reach nothing. Were the popover still open, the
    // row would record a second "Typst" here.
    click(cx, 60., 124.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Typst"],
        "the popover must be closed after choosing an item"
    );
}

#[gpui::test]
fn autocomplete_uncontrolled_selection_sticks(cx: &mut TestAppContext) {
    // The regression behind these tests: an Autocomplete with neither `value`
    // nor `defaultValue` used to hand its clicks back to a set nobody owned —
    // the callbacks fired, but nothing was remembered. The proof here never
    // reads private state: `on_selection_change_all` reports the component's
    // own held set, so the second report naming BOTH picks can only come from
    // state that stuck, and the third click *toggling Alpha back off* is only
    // reachable if the row knew it was already selected.
    let single = events();
    let picked = single.clone();
    let all = events();
    let reported = all.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let single = single.clone();
        let all = all.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .on_change(move |item, _, _| single.borrow_mut().push(item.to_string()))
            .on_selection_change_all(move |keys, _, _| {
                let joined = keys
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                all.borrow_mut().push(joined);
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    // Multiple mode keeps the panel open between picks: row i sits at
    // y = 124 + 36i.
    click(cx, 60., 124.);
    assert_eq!(reported.borrow().as_slice(), ["Alpha"]);

    click(cx, 60., 196.);
    assert_eq!(
        reported.borrow().as_slice(),
        ["Alpha", "Alpha,Gamma"],
        "the second report must still contain the first pick"
    );

    click(cx, 60., 124.);
    assert_eq!(
        reported.borrow().as_slice(),
        ["Alpha", "Alpha,Gamma", "Gamma"],
        "re-clicking a picked row must toggle it off, which requires \
         remembering it"
    );
    assert_eq!(picked.borrow().as_slice(), ["Alpha", "Gamma", "Alpha"]);
}

#[gpui::test]
fn autocomplete_escape_closes(cx: &mut TestAppContext) {
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
        Autocomplete::new(state, keyed(&["Alpha", "Beta", "Gamma"]))
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // Escape reaches the root handler through the focused search field; it
    // closes the popover and refocuses the trigger without selecting.
    press(cx, "escape");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "escape closes the popover"
    );
    assert!(recorded.borrow().is_empty(), "escape must not select");

    // Closed proof: where row one would have been, nothing answers.
    click(cx, 60., 124.);
    assert!(
        recorded.borrow().is_empty() && opened.borrow().len() == 2,
        "the popover must be gone after escape"
    );
}

#[gpui::test]
fn autocomplete_arrows_and_enter_select(cx: &mut TestAppContext) {
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
        Autocomplete::new(state, keyed(&["Rust", "Go", "Python"]))
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // Two Downs put the keyboard cursor on the second row; Enter takes it.
    // Enter must not ALSO reopen: the component deliberately does not refocus
    // the trigger inside this keystroke, because gpui activates a focused
    // element on Enter and the trigger's click listener would fire.
    press(cx, "down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Go"],
        "the second row must be selected exactly once"
    );
    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);
}

/// PageUp/PageDown belong to the *open* list: pinned `useSelectableCollection`
/// binds them only while the collection is mounted, which a closed
/// Autocomplete never is. The proof is not vacuous: Escape closes onto the
/// field and leaves the cursor seated on the second row, so the page keys
/// land on the handler *with* a cursor -- exactly the state a page key would
/// move -- and a following Down still opens.
#[gpui::test]
fn autocomplete_page_keys_ignore_a_closed_field(cx: &mut TestAppContext) {
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
        Autocomplete::new(
            state,
            keyed(&["Alpha", "Beta", "Gamma", "Delta", "Epsilon"]),
        )
        .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
        .on_open_change(move |open, _, _| {
            opens.borrow_mut().push(format!("open:{open}"));
        })
        .into_any_element()
    });

    // Two Downs seat the cursor on the second row (Beta); Escape closes onto
    // the field and keeps the cursor.
    click(cx, 60., 18.);
    press(cx, "down down");
    press(cx, "escape");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the probe must begin from a list the Escape closed onto its field"
    );

    press(cx, "pagedown");
    press(cx, "pageup");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "a page key on the closed field must not open the list"
    );
    assert!(
        recorded.borrow().is_empty(),
        "a page key on the closed field must not commit anything"
    );

    // Down reopens without moving the retained cursor; a second Down steps it
    // to the third row, and Enter commits it. Had the page keys moved the
    // cursor at all, this commit would betray them.
    press(cx, "down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Gamma"],
        "the page keys must have left the closed field answering as before"
    );
}

/// Autocomplete is the one popup whose list really scrolls: pinned
/// `autocomplete.css` styles the composed `[data-slot="list-box"]` itself
/// `max-h-[320px] min-h-0 overflow-y-auto`, so pinned React Aria 3.51.0's
/// `ListKeyboardDelegate` pages by one visible rectangle of that 320px
/// viewport -- from the cursor row's rect, the first enabled row whose top
/// crosses `cursor top - row + 320` -- and takes the enabled end only when
/// the walk runs out. Those handlers still require `manager.focusedKey !=
/// null`, so a mouse-opened, selection-less Autocomplete is inert until a
/// Down seats the cursor. Rows are 36px (`util::FIELD_HEIGHT`) under a 6px
/// list padding, so one page from row *i* first crosses row *i + 8*
/// (320 - 36 = 284 = seven rows and 32px). Paging only moves the highlight
/// until Enter commits it.
#[gpui::test]
fn autocomplete_page_keys_move_one_visible_page_after_a_cursor_exists(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let options: Vec<PickerItem> = (0..20)
        .map(|i| {
            let label = format!("Option {i:02}");
            PickerItem::new(label.clone(), label)
        })
        .collect();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        // Option 09 sits exactly on the first page-down boundary from
        // Option 01, and the tail rows sit past every later boundary, so the
        // disabled set probes both the walk's skip and its end fallback.
        Autocomplete::new(state, options.clone())
            .disabled_keys(["Option 00".into(), "Option 09".into(), "Option 19".into()])
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    // Mouse-open with no selection: the cursor is null, so both page keys
    // must be inert. Down from a null cursor enters the first enabled row
    // (Option 01); had either page key created a cursor, Down would hold on
    // a later row or step to Option 02, and the commit would betray the
    // unconditional cursor creation.
    click(cx, 60., 18.);
    press(cx, "pagedown");
    press(cx, "pageup");
    assert!(
        recorded.borrow().is_empty(),
        "page keys on a mouse-opened, cursor-less list must commit nothing"
    );
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Option 01"],
        "page keys on a cursor-less list must be inert: Down must still \
         enter the first enabled row"
    );

    // Reopen; the cursor stands where the commit left it (Option 01). One
    // page down first crosses row 9 -- disabled, so the walk lands on the
    // next enabled row, Option 10. An enabled-end mapping would have taken
    // Option 18, and a boundary step blind to disabled rows would have
    // stopped on Option 09.
    click(cx, 60., 18.);
    press(cx, "pagedown");
    assert!(
        recorded.borrow().as_slice() == ["Option 01"],
        "PageDown must only move the highlight, never commit by itself"
    );
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Option 01", "Option 10"],
        "PageDown must move one visible page, skipping the disabled row \
         the page boundary lands on"
    );

    // Another page down from Option 10 crosses row 18 exactly (8 rows =
    // 288px), which is also the last enabled row -- never the disabled
    // Option 19 past it.
    click(cx, 60., 18.);
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Option 01", "Option 10", "Option 18"],
        "PageDown from the lower half must land on the last enabled row"
    );

    // PageUp walks the same geometry in reverse: from Option 18 one page up
    // crosses row 10, and from Option 10 it crosses row 2.
    click(cx, 60., 18.);
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Option 01", "Option 10", "Option 18", "Option 10"],
        "PageUp must reverse one visible page"
    );
    click(cx, 60., 18.);
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        [
            "Option 01",
            "Option 10",
            "Option 18",
            "Option 10",
            "Option 02"
        ],
        "PageUp must keep moving one visible page"
    );

    // One page up from Option 02 runs off the top of the list, and the walk
    // falls back to the first enabled row -- never the disabled Option 00.
    click(cx, 60., 18.);
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        [
            "Option 01",
            "Option 10",
            "Option 18",
            "Option 10",
            "Option 02",
            "Option 01"
        ],
        "PageUp off the top must fall back to the first enabled row"
    );
}

/// A `row_height` list is uniform, so it pages by the fixed ListBox shape:
/// whole-row steps across its fixed 320px viewport -- `ceil(320 / 36) - 1`
/// = 8 rows per page -- with the same enabled-end fallback when the step
/// runs past an end.
#[gpui::test]
fn autocomplete_row_height_pages_by_fixed_row_steps(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let options: Vec<PickerItem> = (0..20)
        .map(|i| {
            let label = format!("Option {i:02}");
            PickerItem::new(label.clone(), label)
        })
        .collect();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, options.clone())
            .row_height(px(36.))
            .disabled_keys(["Option 00".into(), "Option 19".into()])
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    // Down seats the cursor on Option 01; one page down runs to the row
    // eight places past it, Option 09.
    click(cx, 60., 18.);
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Option 09"],
        "PageDown in a uniform list must step eight rows"
    );

    // Reopen; the cursor stands on Option 09 and one page up reverses the
    // same step back to Option 01.
    click(cx, 60., 18.);
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Option 09", "Option 01"],
        "PageUp in a uniform list must reverse the eight-row step"
    );

    // Reopen; a Down seats the cursor on Option 02, and one page up from
    // there runs past the top -- the fallback takes the first enabled row,
    // never the disabled Option 00.
    click(cx, 60., 18.);
    press(cx, "down");
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Option 09", "Option 01", "Option 01"],
        "PageUp past the top must fall back to the first enabled row"
    );
}

#[gpui::test]
fn autocomplete_forward_typing_focuses_the_first_match_for_enter(cx: &mut TestAppContext) {
    // react-aria 3.51.0's `useAutocomplete.onChange` treats a forward
    // insertion as a request to focus the wrapped collection's first item, so
    // a query can be typed into the search field and Enter commits the top
    // match without an arrow key first. The focus target is
    // `ListKeyboardDelegate.getFirstKey`, which operates on the *filtered*
    // collection: "Alpha" is index 0 of the unfiltered list and must not
    // catch the autofocus — "ru" filters to Rust and Rusty, and the cursor
    // lands on Rust.
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
        Autocomplete::new(state, keyed(&["Alpha", "Rust", "Rusty"]))
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // Typing forward must move the keyboard cursor onto the first filtered
    // row, so a bare Enter commits it and closes the popover exactly once.
    cx.simulate_input("ru");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Rust"],
        "typing forward must focus the first filtered match so Enter commits it"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the Enter pick must close the popover exactly once"
    );
}

#[gpui::test]
fn autocomplete_forward_typing_focuses_the_first_enabled_match(cx: &mut TestAppContext) {
    // `ListKeyboardDelegate.getFirstKey` is `findNextNonDisabled` — the first
    // *enabled* item, with `disabledKeys` walked past. A literal first match
    // would put the cursor on the disabled "Rust", and Enter would commit a
    // row that cannot be chosen; the autofocus must land on "Rusty" instead.
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
        Autocomplete::new(state, keyed(&["Rust", "Rusty", "Go"]))
            .disabled_keys(["Rust".into()])
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    cx.simulate_input("ru");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Rusty"],
        "a disabled first match must not catch the forward-typing autofocus"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the Enter pick must close the popover exactly once"
    );
}

#[gpui::test]
fn autocomplete_space_resets_focus_to_the_first_match(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_assert = state.clone();
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Rust book", "Rust belt"]))
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("rust");
    press(cx, "down");
    press(cx, "space");
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "rust "
    );
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Rust book"],
        "a typed space is a forward insertion and must refocus the first match"
    );
}

#[gpui::test]
fn autocomplete_backspace_clears_virtual_focus(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Rust", "Rusty"]))
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("ru");
    press(cx, "down");
    press(cx, "backspace");
    press(cx, "enter");
    assert!(
        recorded.borrow().is_empty(),
        "deleting text must clear virtual focus instead of committing a stale row"
    );
}

#[gpui::test]
fn autocomplete_paste_clears_virtual_focus(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Rust", "Rusty"]))
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("r");
    press(cx, "down");
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("u".to_owned()));
    press(cx, "ctrl-v");
    press(cx, "enter");
    assert!(
        recorded.borrow().is_empty(),
        "pasting text must clear virtual focus instead of committing a stale row"
    );
}

#[gpui::test]
fn autocomplete_modified_space_does_not_activate_a_match(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Rust", "Rusty"]))
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("ru");
    press(cx, "ctrl-space");
    assert!(
        recorded.borrow().is_empty(),
        "modified Space must not activate the virtually focused row"
    );
}

#[gpui::test]
fn autocomplete_controlled_query_update_does_not_focus_a_match(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let query = Rc::new(RefCell::new(String::new()));
    let query_for_view = query.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        Autocomplete::new(state, keyed(&["Alpha", "Rust", "Rusty"]))
            .default_open(true)
            .input_value(query_for_view.borrow().clone())
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    // React Aria focuses the first match from its input-event handler. An
    // owner replacing controlled `inputValue` is not such an event, so a bare
    // Enter after this repaint must still have no collection cursor to commit.
    *query.borrow_mut() = "ru".to_owned();
    cx.update(|window, _| window.refresh());
    press(cx, "enter");
    assert!(
        recorded.borrow().is_empty(),
        "a controlled query prop update must not masquerade as forward typing"
    );
}

/// Pinned React Stately 3.49.0 backs `inputValue` with controlled state: an
/// edit reports the proposed text, but the visible input keeps the owner's
/// value until that owner accepts it. The bound `InputState` is the visible
/// search field in this port, so it must return to the prop after the edit.
#[gpui::test]
fn autocomplete_controlled_query_waits_for_owner_acceptance(cx: &mut TestAppContext) {
    let inputs = events();
    let recorded = inputs.clone();
    let state = search_state(cx);
    let state_for_view = state.clone();

    let cx = open_host(cx, move || {
        let inputs = inputs.clone();
        Autocomplete::new(state_for_view.clone(), keyed(&["Alpha", "Rust", "Rusty"]))
            .default_open(true)
            .input_value("ru")
            .on_input_change(move |value, _, _| inputs.borrow_mut().push(value.to_owned()))
            .into_any_element()
    });

    cx.simulate_input("s");
    assert_eq!(
        (
            recorded.borrow().clone(),
            state.read_with(cx, |state, _| state.value().to_owned())
        ),
        (vec!["rus".to_owned()], "ru".to_owned()),
        "the edit must be reported without replacing the controlled query"
    );
}

/// `Autocomplete.Indicator` is the chevron in the trigger. HeroUI gives that
/// part the open state through its data attribute; the local closure receives
/// the equivalent boolean so custom content can follow the same state.
#[gpui::test]
fn autocomplete_custom_indicator_receives_trigger_open_state(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(None));
    let recorded = seen.clone();
    let state = search_state(cx);

    let cx = open_host(cx, move || {
        let seen = seen.clone();
        Autocomplete::new(state.clone(), keyed(&["Alpha", "Beta"]))
            .indicator(move |is_open| {
                *seen.borrow_mut() = Some(is_open);
                gpui::div().into_any_element()
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(
        *recorded.borrow(),
        Some(true),
        "the custom trigger indicator must observe the open state"
    );
}

/// The composed `ListBox.ItemIndicator` remains a separate row-level seam: it
/// receives selection state rather than the trigger's open state.
#[gpui::test]
fn autocomplete_item_indicator_receives_row_selection_state(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let recorded = seen.clone();
    let state = search_state(cx);

    let _cx = open_host(cx, move || {
        let seen = seen.clone();
        Autocomplete::new(state.clone(), keyed(&["Alpha", "Beta"]))
            .default_value(["Alpha"])
            .default_open(true)
            .item_indicator(move |is_selected| {
                seen.borrow_mut().push(is_selected);
                gpui::div().into_any_element()
            })
            .into_any_element()
    });

    // The host builder is allowed to render more than once while the
    // placement state settles. A render prop is therefore called once per
    // row per frame; assert the first complete row pass and reject any state
    // value outside the two row selections instead of coupling the contract
    // to a frame count.
    let recorded = recorded.borrow();
    assert!(
        recorded.len() >= 2
            && recorded
                .as_chunks::<2>()
                .0
                .iter()
                .all(|states| states == &[true, false]),
        "the composed item indicator must receive each row's selection state"
    );
}

/// Exit frames are visual only. Once Escape closes the logical popover, the
/// retained shrinking surface must no longer own outside-dismiss interaction
/// or report the same close a second time.
#[gpui::test]
fn autocomplete_exiting_panel_does_not_repeat_outside_dismissal(cx: &mut TestAppContext) {
    let opens = events();
    let recorded = opens.clone();
    let state = search_state(cx);

    let cx = open_host(cx, move || {
        let opens = opens.clone();
        Autocomplete::new(state.clone(), keyed(&["Alpha", "Beta"]))
            .default_open(true)
            .on_open_change(move |is_open, _, _| {
                opens.borrow_mut().push(format!("open:{is_open}"));
            })
            .into_any_element()
    });

    press(cx, "escape");
    click(cx, 600., 300.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["open:false"],
        "an exiting panel must not retain its dismissal listeners"
    );
}

#[gpui::test]
fn combo_box_custom_enter_preserves_multiple_selection(cx: &mut TestAppContext) {
    let singular = events();
    let singular_for_view = singular.clone();
    let plural = events();
    let plural_for_view = plural.clone();
    let opens = events();
    let opens_for_view = opens.clone();
    let values = events();
    let values_for_view = values.clone();
    let state = search_state(cx);
    let state_for_assert = state.clone();
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let singular = singular_for_view.clone();
        let plural = plural_for_view.clone();
        let opens = opens_for_view.clone();
        let values = values_for_view.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .default_value(["Alpha", "Beta"])
            .allows_custom_value(true)
            .allows_empty_collection(true)
            .on_change(move |item, _, _| singular.borrow_mut().push(item.to_string()))
            .on_selection_change_all(move |items, _, _| {
                plural.borrow_mut().push(
                    items
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .value_content(move |value| {
                values.borrow_mut().push(value.selected_text.to_owned());
                value.default_children
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "down");
    cx.simulate_input("zig");
    press(cx, "enter");
    press(cx, "down enter");
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "zig",
        "custom text must remain in the input"
    );
    assert!(
        singular.borrow().is_empty(),
        "multiple-mode custom text must not report a singular selection"
    );
    assert!(
        plural.borrow().is_empty(),
        "multiple-mode custom text must not report a plural selection change"
    );
    assert_eq!(
        values.borrow().last().map(String::as_str),
        Some("Alpha, Beta"),
        "custom text must not replace the existing multiple selection"
    );
    assert_eq!(
        opens.borrow().as_slice(),
        ["open:true", "open:false"],
        "custom Enter must close the still-open empty list exactly once"
    );
}

#[gpui::test]
fn combo_box_multiple_enter_toggles_the_focused_item(cx: &mut TestAppContext) {
    let singular = events();
    let singular_for_view = singular.clone();
    let plural = events();
    let plural_for_view = plural.clone();
    let opens = events();
    let opens_for_view = opens.clone();
    let inputs = events();
    let inputs_for_view = inputs.clone();
    let state = search_state(cx);
    let state_for_assert = state.clone();
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let singular = singular_for_view.clone();
        let plural = plural_for_view.clone();
        let opens = opens_for_view.clone();
        let inputs = inputs_for_view.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .default_value(["Alpha", "Beta"])
            .on_change(move |item, _, _| singular.borrow_mut().push(item.to_string()))
            .on_selection_change_all(move |items, _, _| {
                plural.borrow_mut().push(
                    items
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .on_input_change(move |value, _, _| inputs.borrow_mut().push(value.to_owned()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("a");
    press(cx, "down enter enter");
    cx.simulate_input("g");
    press(cx, "down enter");
    assert!(
        singular.borrow().is_empty(),
        "multiple-mode keyboard picks must not use the singular callback"
    );
    assert_eq!(
        plural.borrow().as_slice(),
        // The selection keeps insertion order, the way pinned react-stately
        // 3.49.0's `Set` iterates: the re-added Alpha appends after Beta.
        ["Beta", "Beta,Alpha", "Beta,Alpha,Gamma"],
        "the focused key must survive query reset by identity so Enter toggles the same item again"
    );
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "",
        "multiple-mode row activation must reset the input text"
    );
    assert_eq!(
        opens.borrow().as_slice(),
        ["open:true"],
        "multiple-mode row activation must leave the list open"
    );
    assert_eq!(
        inputs.borrow().as_slice(),
        ["a", "", "g", ""],
        "typing and selection-driven query resets must both report input changes"
    );
}

#[gpui::test]
fn combo_box_controlled_multiple_enter_waits_for_the_owner(cx: &mut TestAppContext) {
    let singular = events();
    let singular_for_view = singular.clone();
    let plural = events();
    let plural_for_view = plural.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let singular = singular_for_view.clone();
        let plural = plural_for_view.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(["Alpha".into(), "Beta".into()])
            .on_change(move |item, _, _| singular.borrow_mut().push(item.to_string()))
            .on_selection_change_all(move |items, window, _| {
                plural.borrow_mut().push(
                    items
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                // Re-render the unchanged controlled prop, as an owner declining
                // the proposed selection would.
                window.refresh();
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "down enter enter");
    assert!(
        singular.borrow().is_empty(),
        "controlled multiple picks must not use the singular callback"
    );
    assert_eq!(
        plural.borrow().as_slice(),
        ["Beta", "Beta"],
        "a controlled ComboBox must keep proposing from the owner's unchanged selection"
    );
}

#[gpui::test]
fn combo_box_multiple_pointer_resets_query_and_stays_open(cx: &mut TestAppContext) {
    let singular = events();
    let singular_for_view = singular.clone();
    let plural = events();
    let plural_for_view = plural.clone();
    let opens = events();
    let opens_for_view = opens.clone();
    let inputs = events();
    let inputs_for_view = inputs.clone();
    let state = search_state(cx);
    let state_for_assert = state.clone();
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let singular = singular_for_view.clone();
        let plural = plural_for_view.clone();
        let opens = opens_for_view.clone();
        let inputs = inputs_for_view.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .default_value(["Alpha", "Beta"])
            .on_change(move |item, _, _| singular.borrow_mut().push(item.to_string()))
            .on_selection_change_all(move |items, _, _| {
                plural.borrow_mut().push(
                    items
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .on_input_change(move |value, _, _| inputs.borrow_mut().push(value.to_owned()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("g");
    press(cx, "down");
    click(cx, 60., 64.);
    press(cx, "enter");
    click(cx, 60., 64.);
    assert!(
        singular.borrow().is_empty(),
        "multiple-mode pointer picks must not use the singular callback"
    );
    assert_eq!(
        plural.borrow().as_slice(),
        ["Alpha,Beta,Gamma", "Alpha,Beta", "Beta"],
        "the first pick must add filtered Gamma, retained Enter must remove Gamma, then the reset list must let the same point remove Alpha"
    );
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "",
        "multiple-mode pointer selection must reset the input query"
    );
    assert_eq!(
        opens.borrow().as_slice(),
        ["open:true"],
        "multiple-mode pointer selection must leave the list open"
    );
    assert_eq!(
        inputs.borrow().as_slice(),
        ["g", ""],
        "the pointer-driven query reset must report the cleared input exactly once"
    );
}

#[gpui::test]
fn combo_box_multiple_focus_survives_a_capped_collection_reset(cx: &mut TestAppContext) {
    let plural = events();
    let plural_for_view = plural.clone();
    let inputs = events();
    let inputs_for_view = inputs.clone();
    let state = search_state(cx);
    let state_for_assert = state.clone();
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let plural = plural_for_view.clone();
        let inputs = inputs_for_view.clone();
        ComboBox::new(
            state_for_view.clone(),
            keyed(&[
                "Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta", "Needle",
            ]),
        )
        .selection_mode(SelectionMode::Multiple)
        .max_items(2)
        .on_selection_change_all(move |items, _, _| {
            plural.borrow_mut().push(
                items
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .on_input_change(move |value, _, _| inputs.borrow_mut().push(value.to_owned()))
        .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("needle");
    press(cx, "down enter enter");
    assert_eq!(
        plural.borrow().as_slice(),
        ["Needle", ""],
        "the focused key must survive even when the expanded cap does not render that item"
    );
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "",
        "the first selection must still clear the filtered query"
    );
    assert_eq!(
        inputs.borrow().as_slice(),
        ["n", "ne", "nee", "need", "needl", "needle", ""]
    );
}

#[gpui::test]
fn combo_box_multiple_external_query_drops_a_stale_focused_key(cx: &mut TestAppContext) {
    let plural = events();
    let plural_for_view = plural.clone();
    let inputs = events();
    let inputs_for_view = inputs.clone();
    let opens = events();
    let opens_for_view = opens.clone();
    let state = search_state(cx);
    let state_for_assert = state.clone();
    let state_for_update = state.clone();
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let plural = plural_for_view.clone();
        let inputs = inputs_for_view.clone();
        let opens = opens_for_view.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .on_selection_change_all(move |items, _, _| {
                plural.borrow_mut().push(
                    items
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .on_input_change(move |value, _, _| inputs.borrow_mut().push(value.to_owned()))
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "down");
    cx.update(|window, cx| {
        state_for_update.update(cx, |state, cx| {
            state.set_value("g");
            cx.notify();
        });
        window.refresh();
    });
    press(cx, "enter");
    assert!(
        plural.borrow().is_empty(),
        "a caller-driven query must not let Enter select the stale key: {:?}",
        plural.borrow().as_slice()
    );
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "",
        "Enter without a live focused key must reset the input"
    );
    assert_eq!(inputs.borrow().as_slice(), [""]);
    assert_eq!(opens.borrow().as_slice(), ["open:true", "open:false"]);
}

#[gpui::test]
fn combo_box_single_enter_without_focus_restores_the_selected_label(cx: &mut TestAppContext) {
    let selections = events();
    let selections_for_view = selections.clone();
    let inputs = events();
    let inputs_for_view = inputs.clone();
    let opens = events();
    let opens_for_view = opens.clone();
    let state = search_state(cx);
    let state_for_assert = state.clone();
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let selections = selections_for_view.clone();
        let inputs = inputs_for_view.clone();
        let opens = opens_for_view.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta", "Gamma"]))
            .default_value(["Beta"])
            .on_change(move |item, _, _| selections.borrow_mut().push(item.to_string()))
            .on_input_change(move |value, _, _| inputs.borrow_mut().push(value.to_owned()))
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("g");
    press(cx, "enter");
    assert!(
        selections.borrow().is_empty(),
        "Enter without a focused row must not report a new selection"
    );
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "Beta",
        "single mode must restore the selected item's label"
    );
    assert_eq!(inputs.borrow().as_slice(), ["g", "Beta"]);
    assert_eq!(opens.borrow().as_slice(), ["open:true", "open:false"]);
}

#[gpui::test]
fn combo_box_multiple_show_all_commits_a_row_outside_the_query(cx: &mut TestAppContext) {
    let plural = events();
    let plural_for_view = plural.clone();
    let opens = events();
    let opens_for_view = opens.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let plural = plural_for_view.clone();
        let opens = opens_for_view.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .on_selection_change_all(move |items, _, _| {
                plural.borrow_mut().push(
                    items
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("g");
    click(cx, 298., 18.);
    click(cx, 298., 18.);
    press(cx, "down enter");
    assert_eq!(
        plural.borrow().as_slice(),
        ["Alpha"],
        "show-all navigation must commit the focused visible row even when it does not match the query"
    );
    assert_eq!(
        opens.borrow().as_slice(),
        ["open:true", "open:false", "open:true"],
        "multiple selection must leave the explicitly reopened full list open"
    );
}

/// Two items share the label "Same" but carry distinct keys, so they stay
/// separate stops, rows, and selection members.
fn duplicate_labels() -> Vec<PickerItem> {
    vec![
        PickerItem::new("same-1", "Same"),
        PickerItem::new("same-2", "Same"),
        PickerItem::new("other", "Other"),
    ]
}

#[gpui::test]
fn combo_box_duplicate_labels_keep_distinct_keyboard_stops(cx: &mut TestAppContext) {
    let plural = events();
    let plural_for_view = plural.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let plural = plural_for_view.clone();
        ComboBox::new(state_for_view.clone(), duplicate_labels())
            .selection_mode(SelectionMode::Multiple)
            .on_selection_change_all(move |items, _, _| {
                plural.borrow_mut().push(
                    items
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "down down down enter");
    assert_eq!(
        plural.borrow().as_slice(),
        ["other"],
        "the second equal label must remain a real stop before navigation reaches the third row"
    );
}

#[gpui::test]
fn combo_box_typing_filters_and_click_selects(cx: &mut TestAppContext) {
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
        // ComboBox is input-shaped: the query is typed into the field itself.
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .placeholder("Search")
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    // The chevron button sits at the right end of the 320px-wide field:
    // 320 - 12px padding - half its 20px box. The chevron is used here to
    // open deliberately without focusing the input first. The default focus
    // trigger is covered in combo_box_open.rs; this test's point is the
    // chevron and pick geometry.
    click(cx, 298., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the chevron must open the list"
    );

    // Typing into the field filters the suggestion rows down to one.
    cx.simulate_input("ty");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "ty",
        "the chevron press must leave the ComboBox input focused for typing"
    );
    click(cx, 60., 64.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Typst"],
        "clicking the matching suggestion must select it"
    );

    // Taking a suggestion fills the field and closes the list — read back
    // through the state entity the test itself owns.
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "Typst", "the input must hold the chosen item");
    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);

    click(cx, 60., 64.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Typst"],
        "a pointer selection must clear uncontrolled open state"
    );
    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);
}

#[gpui::test]
fn dropdown_arrows_and_enter_activate(cx: &mut TestAppContext) {
    let actions = events();
    let fired = actions.clone();
    let opens = events();
    let opened = opens.clone();

    let cx = open_host(cx, move || {
        let actions = actions.clone();
        let skip_actions = actions.clone();
        let opens = opens.clone();
        // Two dropdowns stacked 320px apart, so the first menu (which ends
        // near y 170) never overlaps the second trigger.
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(320.))
            .child(
                Dropdown::uncontrolled(
                    "dd-open",
                    Button::new("dd-open-trigger").label("Actions"),
                    vec![
                        MenuItem::new("open", "Open"),
                        MenuItem::new("close", "Close"),
                    ],
                )
                .id("dd-open")
                .on_action(move |key, _, _| actions.borrow_mut().push(key.to_string()))
                .on_open_change(move |open, _, _| {
                    opens.borrow_mut().push(format!("open:{open}"));
                }),
            )
            .child(
                Dropdown::uncontrolled(
                    "dd-skip",
                    Button::new("dd-skip-trigger").label("Skipper"),
                    vec![
                        MenuItem::new("cut", "Cut"),
                        MenuItem::new("del", "Delete"),
                        MenuItem::new("zoom", "Zoom"),
                    ],
                )
                .id("dd-skip")
                .disabled_keys(["del"])
                .on_action(move |key, _, _| skip_actions.borrow_mut().push(key.to_string())),
            )
            .into_any_element()
    });

    // First menu: the trigger button is 36px tall at the origin, centre
    // (40, 18). Opening moves the focus into the panel, so the arrows work
    // without a click first. An Enter choice closes the menu, as v3's does.
    click(cx, 40., 18.);
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        fired.borrow().as_slice(),
        ["open"],
        "Down then Enter must activate the first enabled item exactly once"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the Enter pick must dismiss the menu"
    );

    // Second menu: its root starts at 36px + the 320px gap, so its trigger
    // centre is (40, 374). The cut pick closed the menu, so reopen it first;
    // then Down twice must step Cut -> Zoom, skipping the disabled Delete
    // between them.
    click(cx, 40., 374.);
    press(cx, "down");
    press(cx, "enter");
    click(cx, 40., 374.);
    press(cx, "down");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        fired.borrow().as_slice(),
        ["open", "cut", "zoom"],
        "the arrows must skip disabledKeys: activating the second stop \
         records 'zoom', not 'del'"
    );
}

#[gpui::test]
fn date_picker_opens_and_picks_a_day(cx: &mut TestAppContext) {
    let picks = events();
    let recorded = picks.clone();
    let state = cx.new(|cx| CalendarState::new(cx));

    // The visible month is the state's view month, seeded from today. Column
    // c of the first week holds day `c - lead + 1` once the lead blanks are
    // past, so the last column always holds day `7 - lead` — a real day of
    // this month whatever the month is.
    let today = Date::today();
    let lead = DateConstraints::new().lead_cells(today.year, today.month);
    let expected = Date::new(today.year, today.month, (7 - lead) as u32);

    // The coordinate probe opts into bottom-start so the panel origin stays
    // at the host's left edge; the public default is centered bottom.
    // x: the calendar column is CALENDAR_WIDTH wide (252px = seven cells),
    // each slot is 252/7 wide and the last
    // column's centre sits at 12 + 6*w + w/2 from the panel origin.
    // y: panel top 44 + padding 12 + header 40 + weekday 24 + body offset 4
    // + half a 36px cell = 142.
    let cell_w = f32::from(CALENDAR_WIDTH) / 7.;
    let day_x = 12. + 6. * cell_w + cell_w / 2.;
    let day_y = 142.;

    let state_for_view = state;
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        DatePicker::new(state_for_view.clone())
            .placement(herogpui_core::Placement::BottomStart)
            .on_change(move |date, _, _| {
                let iso = date.map(|d| d.format_iso()).unwrap_or_default();
                picks.borrow_mut().push(iso);
            })
            .into_any_element()
    });

    click(cx, 295., 18.);
    click(cx, day_x, day_y);

    assert_eq!(
        recorded.borrow().as_slice(),
        [expected.format_iso()],
        "clicking the last cell of the first week must pick day {}",
        7 - lead
    );
}

/// Picking the second of two same-label rows must report the row's *key* —
/// the labels are identical, so only the key tells the picks apart — exactly
/// once, and fill the input with the shared label.
#[gpui::test]
fn combo_box_duplicate_label_pick_reports_the_key_once(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state.clone();
    let state_for_assert = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        ComboBox::new(state_for_view.clone(), duplicate_labels())
            .menu_trigger(MenuTrigger::Manual)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    click(cx, 298., 18.);
    click(cx, 60., 100.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["same-2"],
        "the second same-label row must report its own key, exactly once"
    );
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "Same",
        "the input must show the row's label, not its key"
    );
}

/// `disabledKeys` addresses one key: disabling the first same-label row must
/// leave its sibling enabled, both for the pointer (no pick from the disabled
/// row) and for the keyboard (the stops skip only that key).
#[gpui::test]
fn combo_box_disabling_one_key_leaves_its_label_sibling_enabled(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state.clone();
    let state_for_assert = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        ComboBox::new(state_for_view.clone(), duplicate_labels())
            .menu_trigger(MenuTrigger::Manual)
            .disabled_keys(["same-1".into()])
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    click(cx, 298., 18.);
    // The keyboard stops skip only that key: the first Down from the null
    // cursor lands on the first enabled row — the sibling — and Enter picks
    // it.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["same-2"],
        "the enabled sibling of a disabled same-label row must stay choosable"
    );

    // Reopen and click the disabled "same-1": its click must do nothing —
    // no pick, no input change (the click blurs the field, as a browser
    // mousedown on a non-focusable row does, so the panel closes behind it).
    click(cx, 298., 18.);
    click(cx, 60., 64.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["same-2"],
        "clicking the disabled key's row must not pick anything"
    );
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "Same",
        "the disabled row must not change the input"
    );
}

/// The cursor rides the item's key, so it survives a caller-driven reorder of
/// the collection — even between two items that share a label.
#[gpui::test]
fn combo_box_cursor_survives_a_reorder_by_key(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;
    let items: Rc<RefCell<Vec<PickerItem>>> = Rc::new(RefCell::new(duplicate_labels()));
    let items_for_view = items.clone();

    let cx = open_host(cx, move || {
        let items = items_for_view.borrow().clone();
        let changes = changes.clone();
        ComboBox::new(state_for_view.clone(), items)
            .menu_trigger(MenuTrigger::Manual)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    click(cx, 298., 18.);
    press(cx, "down down");

    // The caller reorders the collection: the focused item moves from row 1
    // to row 2, and the key must move with the item, not the row.
    cx.update(|window, _cx| {
        items.borrow_mut().reverse();
        window.refresh();
    });
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["same-2"],
        "the cursor must follow the item's key through a reorder, not its row"
    );
}

/// The virtual path shares the plain path's row builder, so duplicate labels
/// stay distinct keyboard stops with `row_height` set too.
#[gpui::test]
fn combo_box_virtual_rows_stay_distinct_under_duplicate_labels(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        ComboBox::new(state_for_view.clone(), duplicate_labels())
            .menu_trigger(MenuTrigger::Manual)
            .row_height(px(36.))
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    click(cx, 298., 18.);
    press(cx, "down enter");
    // The pick closed the list and cleared the cursor; the chevron reopens it
    // and seats the cursor on the selected same-1, so a single Down reaches
    // the second same-label row as its own stop.
    click(cx, 298., 18.);
    press(cx, "down enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["same-1", "same-2"],
        "the two same-label virtual rows must be separate stops picking their own keys"
    );
}

/// The selection keeps the owner's key order, the way pinned react-stately
/// 3.49.0's `Set` iterates: `ComboBox.Value` reports `Gamma, Alpha` exactly as
/// listed, not sorted.
#[gpui::test]
fn combo_box_multiple_keeps_the_owner_s_key_order(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let record = seen.clone();
    let state = search_state(cx);
    let state_for_view = state;

    let _cx = open_host(cx, move || {
        let record = record.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .default_value(["Gamma", "Alpha"])
            .menu_trigger(MenuTrigger::Manual)
            .value_content(move |value| {
                record.borrow_mut().push(value.selected_text.to_owned());
                value.default_children
            })
            .into_any_element()
    });

    assert_eq!(
        seen.borrow().last().map(String::as_str),
        Some("Gamma, Alpha"),
        "the default selection must keep the owner's key order everywhere it is read"
    );
}

/// A custom commit over an existing selection reports the pinned `null`
/// selected key — the empty slice — exactly once, keeps the typed text, and
/// stays silent on the single-key callback that cannot spell `null`.
#[gpui::test]
fn combo_box_custom_commit_over_a_selection_reports_the_null_key(cx: &mut TestAppContext) {
    let slices = events();
    let sliced = slices.clone();
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state.clone();
    let state_for_assert = state;

    let cx = open_host(cx, move || {
        let slices = slices.clone();
        let changes = changes.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta"]))
            .default_value(["Alpha"])
            .allows_custom_value(true)
            .menu_trigger(MenuTrigger::Manual)
            .on_selection_change_all(move |keys, _, _| {
                slices.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("Zig");
    press(cx, "enter");
    assert_eq!(
        sliced.borrow().as_slice(),
        [""],
        "committing a custom value over a selection must report the null key"
    );
    assert!(
        recorded.borrow().is_empty(),
        "the single-key callback cannot spell null and must stay silent"
    );
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "Zig",
        "the typed text must survive the custom commit"
    );
}

/// Enter on a field whose text still matches the selected item's label is not
/// a custom commit: pinned `commitValue` re-runs `commitSelection` there, so
/// the selection stands and the callbacks stay silent. Closed field.
#[gpui::test]
fn combo_box_enter_matching_the_selected_label_preserves_the_selection(cx: &mut TestAppContext) {
    let slices = events();
    let sliced = slices.clone();
    let values = events();
    let values_recorded = values.clone();
    let state = search_state(cx);
    let state_for_view = state.clone();
    let state_for_assert = state;

    let cx = open_host(cx, move || {
        let slices = slices.clone();
        let values = values.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta"]))
            .default_value(["Alpha"])
            .default_input_value("Alpha")
            .allows_custom_value(true)
            .menu_trigger(MenuTrigger::Manual)
            .on_selection_change_all(move |keys, _, _| {
                slices.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .value_content(move |v| {
                values.borrow_mut().push(v.selected_text.to_owned());
                v.default_children
            })
            .into_any_element()
    });

    // Focus the field first: Manual keeps the panel closed on the click, and
    // the Enter below is the field's own keydown.
    click(cx, 60., 18.);
    press(cx, "enter");
    assert!(
        sliced.borrow().is_empty(),
        "text matching the selected label must not commit the null key"
    );
    assert_eq!(
        values_recorded.borrow().last().map(String::as_str),
        Some("Alpha"),
        "the selection must stand when the text still matches its label"
    );
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "Alpha",
        "the matching text must survive the Enter"
    );
}

/// The same re-affirmation on an open field with no cursor row: Enter closes
/// the panel, keeps the selection and reports nothing.
#[gpui::test]
fn combo_box_enter_matching_the_selected_label_keeps_it_open_without_a_cursor(
    cx: &mut TestAppContext,
) {
    let slices = events();
    let sliced = slices.clone();
    let opens = events();
    let opened = opens.clone();
    let values = events();
    let values_recorded = values.clone();
    let state = search_state(cx);
    let state_for_view = state.clone();
    let state_for_assert = state;

    let cx = open_host(cx, move || {
        let slices = slices.clone();
        let opens = opens.clone();
        let values = values.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta"]))
            .default_value(["Alpha"])
            .default_input_value("Alpha")
            .allows_custom_value(true)
            .menu_trigger(MenuTrigger::Manual)
            .default_open(true)
            .on_selection_change_all(move |keys, _, _| {
                slices.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .on_open_change(move |open, _, _| opens.borrow_mut().push(format!("open:{open}")))
            .value_content(move |v| {
                values.borrow_mut().push(v.selected_text.to_owned());
                v.default_children
            })
            .into_any_element()
    });

    // Focus the field: Manual trigger keeps the default-open panel up through
    // the click, and the Enter below is the field's own keydown.
    click(cx, 60., 18.);
    press(cx, "enter");
    assert!(
        sliced.borrow().is_empty(),
        "an open field without a cursor must not commit the null key when the \
         text matches the selected label"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false"],
        "the Enter must still close the open panel, exactly once"
    );
    assert_eq!(
        values_recorded.borrow().last().map(String::as_str),
        Some("Alpha"),
        "the selection must stand"
    );
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "Alpha",
        "the matching text must survive the Enter"
    );
}

/// The `selected_key` owner view rebuilds the combo every frame with the key
/// it currently holds — the shape of a controlled gallery owner.
struct SelectedKeyOwner {
    state: gpui::Entity<InputState>,
    key: SharedString,
    default_input: Option<SharedString>,
    picks: Rc<RefCell<Vec<String>>>,
}

impl Render for SelectedKeyOwner {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let key = self.key.clone();
        let picks = self.picks.clone();
        let mut combo = ComboBox::new(
            self.state.clone(),
            vec![
                PickerItem::new("alpha", "Alpha"),
                PickerItem::new("beta", "Beta"),
            ],
        )
        .selected_key(key)
        .on_selection_change(move |item, _, _| picks.borrow_mut().push(item.to_string()));
        if let Some(text) = self.default_input.clone() {
            combo = combo.default_input_value(text);
        }
        combo.into_any_element()
    }
}

/// `selected_key(key)` writes the key's label into the input when the
/// owner's key changes, never on the owner's other re-renders — so typing
/// survives a rerender — and the empty string clears. The pick callback still
/// reports the key of whatever row the user takes.
#[gpui::test]
fn combo_box_selected_key_syncs_the_label_only_when_the_key_changes(cx: &mut TestAppContext) {
    let picks = events();
    let picks_for_view = picks.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_assert = state.clone();
    // The owner view replaces `open_host`'s anonymous host; the theme global
    // still has to exist before the window draws.
    cx.update(herogpui_theme::ThemeProvider::init);
    let (_view, cx) = cx.add_window_view(|_, _| SelectedKeyOwner {
        state: state.clone(),
        key: "beta".into(),
        default_input: None,
        picks: picks_for_view.clone(),
    });

    // First frame: the controlled key "beta" writes its label into the input.
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "Beta",
        "the controlled key's label must be the input's text"
    );

    // Typing, then an owner rerender with the same key: the text must not be
    // overwritten by the label.
    click(cx, 60., 18.);
    press(cx, "ctrl-a");
    press(cx, "backspace");
    cx.simulate_input("Al");
    cx.update(|window, _| window.refresh());
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "Al",
        "an owner rerender with an unchanged key must leave the typed text alone"
    );

    // An owner key change does move the label in.
    cx.update(|_, cx| _view.update(cx, |owner, _| owner.key = "alpha".into()));
    cx.update(|window, _| window.refresh());
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "Alpha",
        "the owner's key change must write the new key's label into the input"
    );

    // The empty string is v3's `null`: it clears the input.
    cx.update(|_, cx| _view.update(cx, |owner, _| owner.key = "".into()));
    cx.update(|window, _| window.refresh());
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "",
        "the null key must clear the input"
    );

    // Picking a row still reports the key, not the label.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        picks.borrow().as_slice(),
        ["alpha"],
        "the pick must report the row's key"
    );
}

/// Pinned `getDefaultInputValue` derives the first input text from the
/// selected key only when no `defaultInputValue` was given: an explicit
/// default must beat the controlled key's label on first render, and later
/// owner key changes must still move their labels in.
#[gpui::test]
fn combo_box_default_input_value_wins_over_the_first_selected_key(cx: &mut TestAppContext) {
    let picks = events();
    let picks_for_view = picks;
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_assert = state.clone();
    cx.update(herogpui_theme::ThemeProvider::init);
    let (view, cx) = cx.add_window_view(|_, _| SelectedKeyOwner {
        state: state.clone(),
        key: "beta".into(),
        default_input: Some("custom".into()),
        picks: picks_for_view.clone(),
    });

    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "custom",
        "the explicit default input must beat the controlled key's label on \
         first render"
    );

    // A later owner key change still syncs the new key's label.
    cx.update(|_, cx| view.update(cx, |owner, _| owner.key = "alpha".into()));
    cx.update(|window, _| window.refresh());
    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "Alpha",
        "a later owner key change must still write the new key's label"
    );
}

/// The empty `selected_key` is v3's `null`; with an explicit
/// `defaultInputValue` it must not clear the seeded text either.
#[gpui::test]
fn combo_box_default_input_value_wins_over_the_null_selected_key(cx: &mut TestAppContext) {
    let picks = events();
    let picks_for_view = picks;
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_assert = state.clone();
    cx.update(herogpui_theme::ThemeProvider::init);
    let (_view, cx) = cx.add_window_view(|_, _| SelectedKeyOwner {
        state: state.clone(),
        key: "".into(),
        default_input: Some("custom".into()),
        picks: picks_for_view.clone(),
    });

    assert_eq!(
        cx.update(|_, cx| state_for_assert.read(cx).value().to_owned()),
        "custom",
        "the null key must not clear the seeded default input on first render"
    );
}

// ---------------------------------------------------------------------------
// `value` — the pure builder seed on ComboBox
// ---------------------------------------------------------------------------

#[gpui::test]
fn combo_box_value_builder_seeds_the_bound_state_at_first_render(cx: &mut TestAppContext) {
    let state = search_state(cx);
    let state_for_view = state.clone();
    // The builder chain needs no `&mut App`: the text travels on the struct
    // and lands in the state when the field first renders.
    let cx = open_host(cx, move || {
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta"]))
            .value("Al")
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        seeded, "Al",
        "value must seed the bound InputState when the ComboBox first renders"
    );
}

#[gpui::test]
fn combo_box_value_builder_keeps_the_last_call_and_outranks_default_input_value(
    cx: &mut TestAppContext,
) {
    let state = search_state(cx);
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta"]))
            .default_input_value("Beta")
            .value("first")
            .value("second")
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        seeded, "second",
        "a pure builder keeps the last call, and the controlled spelling outranks defaultInputValue"
    );
}

#[gpui::test]
fn combo_box_value_builder_seeds_once_and_never_clobbers_edits(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta"]))
            .value("al")
            .on_input_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(seeded, "al");

    // The builder runs again on every refresh carrying the same `value`; the
    // seed must not rewrite the text the user has been typing.
    click(cx, 60., 18.);
    cx.simulate_input("p");
    cx.update(|window, _| window.refresh());
    let typed = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        typed, "alp",
        "a re-render must not re-apply the value seed over the user's typing"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alp"],
        "the seeded-then-edited text reports exactly once, from the keystroke"
    );
}

/// Removing the highlighted suggestion while the list is open repairs the
/// cursor to the nearest enabled neighbour: the highlighted row is dropped
/// from `items`, and the next render walks forward onto the row that slid
/// into its place. Enter then commits the repaired row.
#[gpui::test]
fn autocomplete_removing_the_highlighted_row_repairs_to_the_next(cx: &mut TestAppContext) {
    let items = Rc::new(RefCell::new(vec![
        ("alpha", "Alpha"),
        ("beta", "Beta"),
        ("gamma", "Gamma"),
        ("delta", "Delta"),
    ]));
    let changes = events();
    let recorded = changes.clone();
    let state = search_state(cx);
    let state_for_view = state;
    let items_for_view = items.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        let items = items_for_view.borrow();
        let its: Vec<PickerItem> = items
            .iter()
            .map(|(key, label)| PickerItem::new(*key, *label))
            .collect();
        Autocomplete::new(state, its)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    // Click opens the full list; three Downs walk the cursor to "gamma"
    // (index 2 of 4).
    click(cx, 60., 18.);
    press(cx, "down down down");

    // The caller drops "gamma" while the list stays open; the next render
    // must move the highlight on to "delta".
    items.borrow_mut().remove(2);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();

    press(cx, "enter");
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("delta"),
        "removing the highlighted row must repair the cursor to the next row"
    );
}
