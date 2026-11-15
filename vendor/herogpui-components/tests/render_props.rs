//! Behaviour tests for the *collection* render-prop closures — the inverted
//! render props AGENTS.md's "A render-prop argument is not an unportable prop"
//! table lists: `ListBox::item_content`, `Dropdown::item_content` and
//! `Dropdown::indicator_content`, `Menu::item_content`, `TagGroup::tag_content`,
//! `RadioGroup::option_content`, `Calendar::cell` and `RangeCalendar::cell`.
//!
//! `Button::content` — the seventh row of that table — panicked the instant it
//! rendered (two `on_hover` bindings, then an animated wrapper whose
//! generation-bearing id reset gpui's internal hover latch), and the gallery
//! never rendered it, so sixteen audits and 76 green pages said nothing. These
//! tests drive the collection ones the same way the button is now driven:
//! a closure-set component must render a frame without panicking, and the
//! closure must observe the state changes the pointer and the keyboard
//! produce. Every state the port computes for its own drawing is handed over
//! one frame late where gpui reports it to a handler, so the cycle is
//! event -> `flush_frame` -> read the closure's latest snapshot.
//!
//! Four of the eight closures are handed `util::InteractiveState`. TagGroup
//! wires its hover/press through `util::interaction` + `util::track_interaction`
//! (gated on the closure being set — see the no-closure test); ListBox, Menu
//! and Dropdown hand over hardcoded `false` for the hover but read the press
//! from the interaction slot, so their state-change tests assert what they do
//! deliver (selection and cursor focus) and separate
//! defect tests pin the press, which v3's `ListBox.Item` and `Dropdown.Item`
//! tables list as `isPressed` and which the port's own doc comments promise
//! "a frame behind the pointer".
//! RadioGroup instead hands its root closure `RadioOptionState`, matching v3's
//! field-level selected, disabled, read-only, invalid and required values.
//!
//! Geometry is derived from the components' own constants, never guessed:
//!
//! - ListBox: `.list-box` is `p-1` (4px) with a 4px gap between rows, each
//!   row `min-h-9` = `util::FIELD_HEIGHT` (36px), so row *i*'s centre is
//!   y = 4 + 36i + 4i + 18 = 22 + 40i. Rows stretch the window width (gpui
//!   divs are block), so x = 60 is inside every row.
//! - Menu: the panel is the component root with `p-1` (4px) and a 2px gap
//!   between `min-h-9` rows, so row *i*'s centre is y = 4 + 38i + 18 =
//!   22 + 38i; the panel's `min-w-55` is 220px, so x = 60 is inside every row.
//! - TagGroup: `.tag--md` is `px-2 py-1` (8px/4px) around the content the
//!   closure draws; the tests use a fixed 40x20 box, so a chip is
//!   8 + 40 + 8 = 56px wide and 4 + 20 + 4 = 28px tall, and the list gaps
//!   chips by 6px, so chip *i*'s centre is (28 + 62i, 14).
//! - RadioGroup: `.radio__control` is `size-4` (16px) with a 12px gap to the
//!   content; with the same 40x20 content box a row is 20px tall and its
//!   centre x = 16 + 12 + 20 = 48. Vertical orientation spaces rows by 16px,
//!   so row *i*'s centre y = 10 + 36i.
//! - Calendar: the derivation from `calendars_and_more.rs` is reused —
//!   `CALENDAR_WIDTH` (252) minus six 2px gaps over seven cells fixes the
//!   column centres, the first cell row sits at y = 74 (24px nav header +
//!   gap 8 + ~16px weekday line + gap 8 + half a 36px cell), rows step 38px
//!   (36 cell + 2 gap), and day *d* of a month with `lead` leading blanks
//!   sits at `idx = d + lead - 1`, row `idx / 7`, column `idx % 7`.
//! - RangeCalendar: cells are 38px with no column gaps (row `flex_row` with
//!   no gap), so column *c* centres at 19 + 38c; the first row centres at
//!   y = 75 and rows step 40px (38 cell + 2 gap).
//!
//! Every instance has its own element id: two components sharing an id share
//! their keyed state, which AGENTS.md records as a silent failure.

use crate::harness;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    point, prelude::*, px, Font, FontFeatures, FontStyle, FontWeight, KeyUpEvent, Keystroke,
    Modifiers, MouseButton, SharedString, TestAppContext, VisualTestContext,
};
use herogpui_components::{
    calendar::{Date, CALENDAR_WIDTH},
    Button, Calendar, CalendarCellState, CalendarState, DateConstraints, DateRangeState, Dropdown,
    InteractiveState, ListBox, ListBoxItem, Menu, MenuItem, RadioGroup, RadioOption,
    RadioOptionState, RangeCalendar, RangeCalendarCellState, SelectionMode, Tag, TagGroup,
    VisibleDuration, Weekday,
};

use harness::{click, events, open_host, press};

// ---------------------------------------------------------------------------
// Shared plumbing
// ---------------------------------------------------------------------------

/// Forces the frame that carries the state a handler just changed.
///
/// Every event below ends with the window dirty but not necessarily painted,
/// and events hit-test the *last rendered frame*. An explicit refresh makes
/// the frame under test the one whose render the closures recorded, which is
/// also how the hover/press one-frame lag becomes deterministic: the closure
/// can only read what the last frame stashed in the interaction slot, so the
/// refresh is what turns the handler's write into a visible value.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// The per-row snapshots a stateful closure wrote on the last frame, keyed by
/// the row's key so a multi-row component cannot hide a laggard behind the
/// last invocation (the closure runs once per row on every render).
type Interactives = Rc<RefCell<HashMap<String, InteractiveState>>>;

fn record_interactive(map: &Interactives, key: &SharedString, state: InteractiveState) {
    map.borrow_mut().insert(key.to_string(), state);
}

fn state_of(map: &Interactives, key: &str) -> InteractiveState {
    map.borrow().get(key).copied().unwrap_or_default()
}

type RadioStates = Rc<RefCell<HashMap<String, RadioOptionState>>>;

fn record_radio_state(map: &RadioStates, key: &SharedString, state: RadioOptionState) {
    map.borrow_mut().insert(key.to_string(), state);
}

fn radio_state_of(map: &RadioStates, key: &str) -> RadioOptionState {
    map.borrow().get(key).copied().unwrap_or_default()
}

/// The per-date snapshots a `cell` closure wrote on the last frame.
fn record_cell(map: &Rc<RefCell<HashMap<String, CalendarCellState>>>, state: &CalendarCellState) {
    map.borrow_mut()
        .insert(state.date.format_iso(), state.clone());
}

fn cell_of(map: &Rc<RefCell<HashMap<String, CalendarCellState>>>, key: &str) -> CalendarCellState {
    map.borrow()
        .get(key)
        .cloned()
        .unwrap_or_else(|| panic!("cell for {key} never rendered"))
}

fn record_range_cell(
    map: &Rc<RefCell<HashMap<String, RangeCalendarCellState>>>,
    state: &RangeCalendarCellState,
) {
    map.borrow_mut()
        .insert(state.date.format_iso(), state.clone());
}

fn range_cell_of(
    map: &Rc<RefCell<HashMap<String, RangeCalendarCellState>>>,
    key: &str,
) -> RangeCalendarCellState {
    map.borrow()
        .get(key)
        .cloned()
        .unwrap_or_else(|| panic!("cell for {key} never rendered"))
}

fn sorted_join(keys: &HashSet<SharedString>) -> String {
    let mut names: Vec<String> = keys.iter().map(ToString::to_string).collect();
    names.sort();
    names.join(",")
}

/// The advance width of `text` shaped the way the components shape it: gpui's
/// default `.SystemUIFont` stack at `size` px and `weight`. TagGroup labels
/// are 12px at the default weight; the window's own `WindowTextSystem` is the
/// renderer's, so the measurement is the render's.
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

// ---------------------------------------------------------------------------
// Calendar geometry — the `calendars_and_more.rs` derivation, reused
// ---------------------------------------------------------------------------

/// Column *c*'s centre in a bare Calendar: seven cells across `CALENDAR_WIDTH`
/// minus six 2px gaps.
fn cal_col_x(col: usize) -> f32 {
    let cell_w = (f32::from(CALENDAR_WIDTH) - 12.) / 7.;
    col as f32 * (cell_w + 2.) + cell_w / 2.
}

/// Row *r*'s centre in a bare Calendar: the first row at y = 74, then a
/// 36px cell plus a 2px gap per row.
fn cal_row_y(row: usize) -> f32 {
    74. + row as f32 * 38.
}

/// The centre of the cell holding `day` of `(year, month)` in a bare
/// Calendar, derived from the month's leading blanks (Monday-start default,
/// the same `DateConstraints` the test's calendars use).
fn cal_day(year: i32, month: u32, day: u32) -> (f32, f32) {
    let lead = DateConstraints::new().lead_cells(year, month);
    let idx = day as usize + lead - 1;
    (cal_col_x(idx % 7), cal_row_y(idx / 7))
}

/// Column *c*'s centre in a bare RangeCalendar: 38px cells, no column gaps.
fn range_col_x(col: usize) -> f32 {
    19. + 38. * col as f32
}

/// Row *r*'s centre in a bare RangeCalendar: first row at y = 75, then a
/// 38px cell plus a 2px gap per row.
fn range_row_y(row: usize) -> f32 {
    75. + 40. * row as f32
}

/// The centre of the cell holding `day` of `(year, month)` in a bare
/// RangeCalendar.
fn range_day(year: i32, month: u32, day: u32) -> (f32, f32) {
    let lead = DateConstraints::new().lead_cells(year, month);
    let idx = day as usize + lead - 1;
    (range_col_x(idx % 7), range_row_y(idx / 7))
}

// ---------------------------------------------------------------------------
// ListBox::item_content
// ---------------------------------------------------------------------------

/// The closure runs once per row on the first frame — the `Button::content`
/// class of panic would have died here — and records every row's key with the
/// idle state: the cursor has not moved (no `is_focused`) and nothing is
/// selected.
#[gpui::test]
fn list_box_item_content_renders_at_all(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let _cx = open_host(cx, move || {
        let record = record.clone();
        ListBox::new(
            "rp-lb-render",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
                ListBoxItem::new("gamma", "Gamma"),
            ],
        )
        .item_content(move |key, state| {
            record_interactive(&record, key, state);
            // A fixed 40x20 box stands in for the label a caller would draw.
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .into_any_element()
    });

    for key in ["alpha", "beta", "gamma"] {
        let state = state_of(&recorded, key);
        assert!(
            !state.is_selected && !state.is_focused,
            "row {key} must have recorded the idle state on the first frame"
        );
    }
}

/// The state a caller's closure can observe on a ListBox row: the selection
/// (keyboard-driven, because Enter activates the cursor's row) and the cursor
/// the arrows move (`is_focused`). The hover is delivered as hardcoded false —
/// v3's `ListBox.Item` render-props table lists no `isHovered` — and the
/// press is pinned separately by the press test.
#[gpui::test]
fn list_box_item_content_tracks_selection_and_cursor(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let changes = events();
    let changed = changes.clone();
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let held = held.clone();
        let record = record.clone();
        let changes = changes.clone();
        // Read the set out of the guard first, or the borrow outlives this
        // statement and collides with the callback's write.
        let selected_now = held.borrow().clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(
                ListBox::new(
                    "rp-lb-state",
                    vec![
                        ListBoxItem::new("alpha", "Alpha"),
                        ListBoxItem::new("beta", "Beta"),
                    ],
                )
                .selection_mode(SelectionMode::Single)
                .selected_keys(selected_now)
                .item_content(move |key, state| {
                    record_interactive(&record, key, state);
                    gpui::div().w(px(40.)).h(px(20.)).into_any_element()
                })
                .on_selection_change(move |keys, window, _| {
                    changes.borrow_mut().push(sorted_join(keys));
                    *held.borrow_mut() = keys.clone();
                    // The selection is a prop: the closure can only see the next
                    // render's value, so the caller's copy must be rendered back in.
                    window.refresh();
                }),
            )
            .child(Button::new("after-rp-list-box").label("After"))
            .into_any_element()
    });
    cx.update(|window, _| window.activate_window());
    cx.update(|_, cx| herogpui_components::extend::set_focus_visible(true, cx));
    flush_frame(cx);

    // The list is the window's one tab stop: Tab focuses the first enabled row,
    // then Down advances to row 1 (y 44..80, centre 62), running the closure
    // with `is_focused` — and nothing selected.
    press(cx, "tab");
    press(cx, "down");
    flush_frame(cx);
    let beta = state_of(&recorded, "beta");
    assert!(
        beta.is_focused && beta.is_focus_visible && !beta.is_selected,
        "the frame after Down must focus and ring the second row"
    );
    assert!(
        !state_of(&recorded, "alpha").is_focused,
        "the first row must have lost the cursor"
    );

    // Enter activates the cursor's row: the selection change is reported and
    // fed back, so the closure sees the pick.
    press(cx, "enter");
    flush_frame(cx);
    assert!(
        state_of(&recorded, "beta").is_selected,
        "the frame after Enter must see row beta selected"
    );
    assert_eq!(changed.borrow().as_slice(), ["beta"]);

    // Up walks the cursor back to row 0 without changing the selection.
    press(cx, "up");
    flush_frame(cx);
    assert!(
        state_of(&recorded, "alpha").is_focused,
        "the cursor must have moved back to the first row"
    );
    assert!(
        !state_of(&recorded, "beta").is_focused,
        "the second row must have lost the cursor"
    );

    // Pointer contract as delivered: the row paints its own hover, but the
    // closure is handed the static value — v3's ListBox.Item render-props
    // table has no isHovered row, and the press is the press test's subject.
    cx.simulate_mouse_move(
        point(px(60.), px(22.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    assert!(
        !state_of(&recorded, "alpha").is_hovered,
        "ListBox does not hand the hover to the item_content closure"
    );

    press(cx, "tab");
    flush_frame(cx);
    assert!(
        ["alpha", "beta"].iter().all(|key| {
            let state = state_of(&recorded, key);
            !state.is_focused && !state.is_focus_visible
        }),
        "moving focus out of the ListBox must clear every render-prop focus and ring state"
    );
}

/// A pointer pick on a ListBox row rendered *with* `item_content` must select
/// exactly as the label path does: v3's `ListBox.Item` stays a row whether
/// its children are a node or a function. The content branch used to return
/// before the `on_click` the default label path attaches
/// (`list_box.rs` `row()`, "Label plus optional description stack"), which
/// made a pointer pick inert; the row now falls through to the click handler.
#[gpui::test]
fn list_box_item_content_pointer_selection_is_inert(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let changes = events();
    let changed = changes.clone();
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let held = held.clone();
        let record = record.clone();
        let changes = changes.clone();
        let selected_now = held.borrow().clone();
        ListBox::new(
            "rp-lb-click",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
            ],
        )
        .selection_mode(SelectionMode::Single)
        .selected_keys(selected_now)
        .item_content(move |key, state| {
            record_interactive(&record, key, state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .on_selection_change(move |keys, window, _| {
            changes.borrow_mut().push(sorted_join(keys));
            *held.borrow_mut() = keys.clone();
            window.refresh();
        })
        .into_any_element()
    });

    // Row 0 (y 4..40, centre y 22) spans the window width.
    click(cx, 60., 22.);
    flush_frame(cx);
    assert_eq!(
        changed.borrow().as_slice(),
        ["alpha"],
        "a click on the row must select it"
    );
    assert!(
        state_of(&recorded, "alpha").is_selected,
        "the item_content closure must see the pick"
    );
}

/// v3 documents `isPressed` on `ListBox.Item`'s render props, and the port's
/// own `item_content` doc says "the press is a frame behind the pointer,
/// because gpui reports it to a handler". The row hands over the press the
/// interaction slot recorded on the last frame, so the closure observes the
/// press the row is drawing.
#[gpui::test]
fn list_box_item_content_sees_the_press(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        ListBox::new(
            "rp-lb-press",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
            ],
        )
        .selection_mode(SelectionMode::Single)
        .item_content(move |key, state| {
            record_interactive(&record, key, state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .into_any_element()
    });

    let centre = point(px(60.), px(22.));
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(
        state_of(&recorded, "alpha").is_pressed,
        "the frame after the down must hand the press to the closure"
    );

    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    press(cx, "tab");
    cx.simulate_keystrokes("enter");
    flush_frame(cx);
    assert!(
        state_of(&recorded, "alpha").is_pressed,
        "the frame after Enter down must hand the focused row's keyboard press to the closure"
    );
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    flush_frame(cx);
    assert!(
        !state_of(&recorded, "alpha").is_pressed,
        "Enter up must release the focused row's keyboard press"
    );
}

/// A focused option can become disabled between renders. React Aria repairs
/// collection focus away from it and does not install press behavior on the
/// disabled row, so the stale cursor must not make Enter press or activate it.
#[gpui::test]
fn list_box_item_content_drops_press_when_focused_row_becomes_disabled(cx: &mut TestAppContext) {
    let disabled = Rc::new(RefCell::new(false));
    let disabled_for_view = disabled.clone();
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let actions = events();
    let acted = actions.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        let item = ListBoxItem::new("alpha", "Alpha").is_disabled(*disabled_for_view.borrow());
        ListBox::new("rp-lb-disable-press", vec![item])
            .item_content(move |key, state| {
                record_interactive(&record, key, state);
                gpui::div().w(px(40.)).h(px(20.)).into_any_element()
            })
            .on_action({
                let actions = actions.clone();
                move |key, _, _| actions.borrow_mut().push(key.to_string())
            })
            .into_any_element()
    });

    cx.update(|window, _| window.activate_window());
    flush_frame(cx);
    press(cx, "tab");
    flush_frame(cx);
    assert!(state_of(&recorded, "alpha").is_focused);

    *disabled.borrow_mut() = true;
    flush_frame(cx);
    cx.simulate_keystrokes("enter");
    flush_frame(cx);
    let alpha = state_of(&recorded, "alpha");
    assert!(alpha.is_disabled && !alpha.is_focused && !alpha.is_pressed);
    assert!(
        acted.borrow().is_empty(),
        "a stale cursor must not activate a newly disabled row"
    );
}

/// With neither selection nor an action callback, React Aria does not make a
/// ListBox option pressable. Its render closure must stay idle for pointer and
/// keyboard input even though the collection itself can receive focus.
#[gpui::test]
fn list_box_item_content_is_not_pressed_when_row_has_no_behavior(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        ListBox::new(
            "rp-lb-inert-press",
            vec![ListBoxItem::new("alpha", "Alpha")],
        )
        .selection_mode(SelectionMode::None)
        .item_content(move |key, state| {
            record_interactive(&record, key, state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .into_any_element()
    });

    let centre = point(px(60.), px(22.));
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(!state_of(&recorded, "alpha").is_pressed);
    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    press(cx, "tab");
    cx.simulate_keystrokes("space");
    flush_frame(cx);
    assert!(!state_of(&recorded, "alpha").is_pressed);
}

// ---------------------------------------------------------------------------
// Menu::item_content (Dropdown.Item children)
// ---------------------------------------------------------------------------

/// `Dropdown` composes the `Menu` that owns `Dropdown.Item` render state, so
/// the root builder must reach that live menu rather than existing only on a
/// bare `Menu` instance.
#[gpui::test]
fn dropdown_forwards_item_content_to_its_composed_menu(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        Dropdown::new(
            "rp-dropdown-item",
            Button::new("rp-dropdown-item-trigger").label("Open"),
            vec![MenuItem::new("one", "One"), MenuItem::new("two", "Two")],
            true,
        )
        .selection_mode(SelectionMode::Multiple)
        .default_selected_keys([SharedString::from("one")])
        .disabled_keys([SharedString::from("two")])
        .item_content(move |key, state| {
            record_interactive(&record, key, state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .into_any_element()
    });

    flush_frame(cx);
    let one = state_of(&recorded, "one");
    let two = state_of(&recorded, "two");
    assert!(
        one.is_selected && !one.is_disabled && !two.is_selected && two.is_disabled,
        "the composed menu must hand each item closure its selection and disabled state"
    );
}

/// `Dropdown.ItemIndicator` has its own render function. It receives selected
/// state independently of `Dropdown.Item` children. The pinned HeroUI source
/// forwards raw React Aria item state, where `isIndeterminate` is absent and
/// therefore always falsy.
#[gpui::test]
fn dropdown_forwards_indicator_content_to_its_composed_menu(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::<String, (bool, bool)>::new()));
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        Dropdown::new(
            "rp-dropdown-indicator",
            Button::new("rp-dropdown-indicator-trigger").label("Open"),
            vec![MenuItem::new("one", "One"), MenuItem::new("two", "Two")],
            true,
        )
        .selection_mode(SelectionMode::Multiple)
        .default_selected_keys([SharedString::from("one")])
        .indicator_content(move |key, is_selected, is_indeterminate| {
            record
                .borrow_mut()
                .insert(key.to_string(), (is_selected, is_indeterminate));
            gpui::div().size(px(16.)).into_any_element()
        })
        .into_any_element()
    });

    flush_frame(cx);
    assert_eq!(
        recorded.borrow().clone(),
        HashMap::from([
            ("one".to_owned(), (true, false)),
            ("two".to_owned(), (false, false)),
        ]),
        "indicator render state must reach every composed dropdown item"
    );
    press(cx, "down");
    press(cx, "down");
    press(cx, "enter");
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().clone(),
        HashMap::from([
            ("one".to_owned(), (true, false)),
            ("two".to_owned(), (true, false)),
        ]),
        "the composed indicator closure must observe live uncontrolled selection"
    );
}

/// A submenu is still part of the same Dropdown collection composition, so
/// its items receive both root render closures too.
#[gpui::test]
fn dropdown_forwards_render_content_into_submenus(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let indicators = Rc::new(RefCell::new(HashMap::<String, (bool, bool)>::new()));
    let record = recorded.clone();
    let indicator_record = indicators.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        let indicator_record = indicator_record.clone();
        Dropdown::new(
            "rp-dropdown-submenu",
            Button::new("rp-dropdown-submenu-trigger").label("Open"),
            vec![MenuItem::new("parent", "Parent").submenu(vec![MenuItem::new("child", "Child")])],
            true,
        )
        .item_content(move |key, state| {
            record_interactive(&record, key, state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .indicator_content(move |key, is_selected, is_indeterminate| {
            indicator_record
                .borrow_mut()
                .insert(key.to_string(), (is_selected, is_indeterminate));
            gpui::div().size(px(16.)).into_any_element()
        })
        .into_any_element()
    });

    click(cx, 40., 64.);
    flush_frame(cx);
    assert!(
        recorded.borrow().contains_key("child"),
        "the root item closure must reach a submenu item"
    );
    assert_eq!(
        indicators.borrow().get("child"),
        Some(&(false, false)),
        "the root indicator closure must reach the submenu item state"
    );
}

/// The builder behind v3's `Dropdown.Item` children also works on a bare Menu.
#[gpui::test]
fn menu_item_content_renders_at_all(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let _cx = open_host(cx, move || {
        let record = record.clone();
        Menu::new(
            "render-props-menu-press",
            vec![MenuItem::new("one", "One"), MenuItem::new("two", "Two")],
        )
        .id("rp-menu-render")
        .item_content(move |key, state| {
            record_interactive(&record, key, state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .into_any_element()
    });

    for key in ["one", "two"] {
        let state = state_of(&recorded, key);
        assert!(
            !state.is_selected && !state.is_focused,
            "row {key} must have recorded the idle state on the first frame"
        );
    }
}

/// In multiple mode the menu stays open between picks, so one test can watch
/// the closure see the whole selection lifecycle: the cursor the arrows move
/// (`is_focused`), the tick a pick adds (`is_selected`), the "some but not
/// all" rows a partial selection makes (`is_indeterminate`), and the static
/// pointer state the panel actually delivers.
#[gpui::test]
fn menu_item_content_tracks_selection_and_cursor(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let changes = events();
    let changed = changes.clone();
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let held = held.clone();
        let record = record.clone();
        let changes = changes.clone();
        let selected_now = held.borrow().clone();
        Menu::new(
            "render-props-menu-selection",
            vec![
                MenuItem::new("one", "One"),
                MenuItem::new("two", "Two"),
                MenuItem::new("three", "Three"),
            ],
        )
        .id("rp-menu-state")
        .selection_mode(SelectionMode::Multiple)
        .selected_keys(selected_now)
        .item_content(move |key, state| {
            record_interactive(&record, key, state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .on_selection_change(move |keys, window, _| {
            let joined = keys
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            changes.borrow_mut().push(joined);
            *held.borrow_mut() = keys.to_vec();
            window.refresh();
        })
        .into_any_element()
    });

    // The menu claims the panel focus on its first frame (`util::focus_once`),
    // so the arrows work without a click: Down lands the cursor on row 0
    // (y 4..40, centre 22).
    press(cx, "down");
    flush_frame(cx);
    assert!(
        state_of(&recorded, "one").is_focused,
        "the frame after Down must see the cursor on the first row"
    );

    // Enter activates the cursor's row (multiple mode: no dismissal), and the
    // fed-back selection reaches the closure: one ticked, the others idle.
    press(cx, "enter");
    flush_frame(cx);
    let one = state_of(&recorded, "one");
    assert!(one.is_selected, "the picked row must report selected");
    for key in ["two", "three"] {
        let state = state_of(&recorded, key);
        assert!(
            !state.is_selected && !state.is_indeterminate,
            "{key} must stay idle rather than inventing indeterminate item state"
        );
    }
    assert_eq!(changed.borrow().as_slice(), ["one"]);

    // A pointer pick on row 1 (y 42..78, centre 60) joins it: the menu stays
    // open in multiple mode, so the row is still there to click.
    click(cx, 60., 60.);
    flush_frame(cx);
    assert!(
        state_of(&recorded, "two").is_selected,
        "the clicked row must report selected"
    );
    assert!(
        state_of(&recorded, "one").is_selected,
        "the earlier pick must survive the second one"
    );
    assert_eq!(
        changed.borrow().as_slice(),
        ["one", "one,two"],
        "the second report must still contain the first pick"
    );

    // Two Downs walk the cursor to row 2 (y 80..116, centre 98); the closure
    // sees the cursor leave "one".
    press(cx, "down");
    press(cx, "down");
    flush_frame(cx);
    assert!(
        state_of(&recorded, "three").is_focused,
        "the cursor must reach the third row"
    );
    assert!(
        !state_of(&recorded, "one").is_focused,
        "the cursor must have left the first row"
    );

    // Pointer contract as delivered: v3's Dropdown.Item render-props table
    // lists no isHovered, and the press is the press test's subject.
    cx.simulate_mouse_move(
        point(px(60.), px(98.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    assert!(
        !state_of(&recorded, "three").is_hovered,
        "Menu does not hand the hover to the item_content closure"
    );
}

/// v3 documents `isPressed` on `Dropdown.Item`'s render props, and the port's
/// own `item_content` doc says "the press is a frame behind the pointer,
/// because gpui reports it to a handler". `dropdown.rs` hands over the press
/// the interaction slot recorded on the last frame, so the closure sees the
/// press a menu row is animating through `anim::pressed`.
#[gpui::test]
fn menu_item_content_sees_the_press(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        Menu::new(
            "render-props-menu-disabled",
            vec![MenuItem::new("one", "One"), MenuItem::new("two", "Two")],
        )
        .id("rp-menu-press")
        .disabled_keys([SharedString::from("two")])
        .item_content(move |key, state| {
            record_interactive(&record, key, state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .into_any_element()
    });

    let centre = point(px(60.), px(22.));
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(
        state_of(&recorded, "one").is_pressed,
        "the frame after the down must hand the press to the closure"
    );

    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    press(cx, "home");
    cx.simulate_keystrokes("space");
    flush_frame(cx);
    assert!(
        state_of(&recorded, "one").is_pressed,
        "the frame after Space down must hand the focused menu row's keyboard press to the closure"
    );
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });
    flush_frame(cx);
    assert!(
        !state_of(&recorded, "one").is_pressed,
        "Space up must release the focused menu row's keyboard press"
    );

    let disabled = point(px(60.), px(60.));
    cx.simulate_mouse_down(disabled, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(
        !state_of(&recorded, "two").is_pressed,
        "a disabled menu row must not report a pointer press"
    );
}

/// Pinned `useMenuItem` moves the collection's focused key to the row under a
/// pointer hover, then reports both `isPressed` and `isFocused` after mouse
/// down. The render closure must receive both transitions.
#[gpui::test]
fn menu_item_content_pointer_press_moves_focus(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        Menu::new(
            "render-props-menu-pointer-focus",
            vec![MenuItem::new("one", "One"), MenuItem::new("two", "Two")],
        )
        .id("rp-menu-pointer-focus")
        .item_content(move |key, state| {
            record_interactive(&record, key, state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .into_any_element()
    });

    let centre = point(px(60.), px(60.));
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    flush_frame(cx);
    assert!(
        state_of(&recorded, "two").is_focused,
        "pointer hover must move the menu's focused key to its row"
    );
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    let two = state_of(&recorded, "two");
    assert!(
        two.is_pressed,
        "the press must reach the second row's closure"
    );
    assert!(
        two.is_focused,
        "a pointer press must move the menu's focused key to its row"
    );
}

// ---------------------------------------------------------------------------
// TagGroup::tag_content
// ---------------------------------------------------------------------------

/// TagGroup is the one collection closure that wires the interaction slot
/// (`util::interaction` + `util::track_interaction`, both gated on the
/// closure being set), so this is also the button's contract on a second
/// component: render the closure-set tag without dying on the first frame.
#[gpui::test]
fn tag_group_tag_content_renders_at_all(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let _cx = open_host(cx, move || {
        let record = record.clone();
        TagGroup::new(
            "rp-tg-render",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .tag_content(move |tag, state| {
            record_interactive(&record, tag.key(), state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .into_any_element()
    });

    // The group's roving cursor starts on the first enabled tag, but the first
    // frame has no window focus yet, so every render-prop state is idle.
    let alpha = state_of(&recorded, "alpha");
    assert!(!alpha.is_focused, "a cursor alone is not window focus");
    assert!(!alpha.is_selected, "nothing is selected on the first frame");
    for key in ["beta", "gamma"] {
        let state = state_of(&recorded, key);
        assert!(
            !state.is_focused && !state.is_selected,
            "{key} must be idle on the first frame"
        );
    }
}

#[gpui::test]
fn disabling_tag_group_tag_content_clears_hover_and_press(cx: &mut TestAppContext) {
    let disabled = Rc::new(RefCell::new(false));
    let disabled_for_view = disabled.clone();
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        TagGroup::new(
            "rp-tg-disabled-state",
            vec![Tag::new("alpha", "Alpha").is_disabled(*disabled_for_view.borrow())],
        )
        .selection_mode(SelectionMode::Single)
        .tag_content(move |tag, state| {
            record_interactive(&record, tag.key(), state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .into_any_element()
    });

    let centre = point(px(28.), px(14.));
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    let active = state_of(&recorded, "alpha");
    assert!(active.is_hovered && active.is_pressed && !active.is_disabled);

    *disabled.borrow_mut() = true;
    flush_frame(cx);
    let alpha = state_of(&recorded, "alpha");
    assert!(
        alpha.is_disabled && !alpha.is_hovered && !alpha.is_pressed,
        "a disabled tag must expose disabled state without pointer interaction"
    );

    cx.simulate_mouse_move(
        point(px(200.), px(100.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    *disabled.borrow_mut() = false;
    flush_frame(cx);
    let reenabled = state_of(&recorded, "alpha");
    assert!(
        !reenabled.is_disabled && !reenabled.is_hovered && !reenabled.is_pressed,
        "re-enabling a tag must not restore interaction recorded before it was disabled"
    );
}

#[gpui::test]
fn inert_tag_group_tag_content_never_reports_hover_or_press(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        TagGroup::new("rp-tg-inert-state", vec![Tag::new("alpha", "Alpha")])
            .selection_mode(SelectionMode::None)
            .tag_content(move |tag, state| {
                record_interactive(&record, tag.key(), state);
                gpui::div().w(px(40.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let centre = point(px(28.), px(14.));
    cx.simulate_mouse_move(centre, None::<MouseButton>, Modifiers::none());
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    let alpha = state_of(&recorded, "alpha");
    assert!(
        !alpha.is_hovered && !alpha.is_pressed,
        "a tag without selection or an item action must stay pointer-inert"
    );
}

/// The full pointer cycle on a TagGroup chip, one frame behind the pointer
/// the way the button's test sequences it — and the per-tag keying of the
/// interaction slots: each chip has its own `{id}-tag-{index}-interaction`
/// slot, so hovering chip 0 must not smear a hover onto chip 1 (the
/// `inert_audit.py` "bare literal key" class of bug).
#[gpui::test]
fn tag_group_tag_content_tracks_hover_press_and_selection(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let held = held.clone();
        let record = record.clone();
        let selected_now = held.borrow().clone();
        // Two chips at the origin: chip 0 spans x 0..56 (y 0..28), chip 1
        // starts at x = 56 + 6 (the list's 6px gap); their centres are
        // (28, 14) and (90, 14).
        gpui::div()
            .flex()
            .flex_col()
            .child(
                TagGroup::new(
                    "rp-tg-state",
                    vec![Tag::new("alpha", "Alpha"), Tag::new("beta", "Beta")],
                )
                .selection_mode(SelectionMode::Single)
                .selected_keys(selected_now)
                .tag_content(move |tag, state| {
                    record_interactive(&record, tag.key(), state);
                    gpui::div().w(px(40.)).h(px(20.)).into_any_element()
                })
                .on_selection_change(move |keys, window, _| {
                    *held.borrow_mut() = keys.clone();
                    window.refresh();
                }),
            )
            .child(Button::new("after-rp-tag-group").label("After"))
            .into_any_element()
    });
    cx.update(|window, _| window.activate_window());
    cx.update(|_, cx| herogpui_components::extend::set_focus_visible(true, cx));
    flush_frame(cx);

    // Move the pointer onto chip 0: the slot hears the hover, and the forced
    // frame hands it to the closure — chip 1 stays clean, which only per-chip
    // slot keys can arrange.
    cx.simulate_mouse_move(
        point(px(28.), px(14.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    assert!(
        state_of(&recorded, "alpha").is_hovered,
        "the frame after the move must see the hover on the first chip"
    );
    assert!(
        !state_of(&recorded, "beta").is_hovered,
        "a hover on one chip must not smear onto its neighbour"
    );

    // Press down: hover and press together in the frame after the down.
    cx.simulate_mouse_down(
        point(px(28.), px(14.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    flush_frame(cx);
    let alpha = state_of(&recorded, "alpha");
    assert!(
        alpha.is_hovered && alpha.is_pressed,
        "the frame after the down must see the press with the hover"
    );

    // Release: the press lifts, and the click the up completes selects the
    // chip through the fed-back selection — the up's frame reports all three.
    cx.simulate_mouse_up(
        point(px(28.), px(14.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    flush_frame(cx);
    let alpha = state_of(&recorded, "alpha");
    assert!(
        alpha.is_hovered && !alpha.is_pressed && alpha.is_selected,
        "the up must lift the press and select the chip"
    );

    // Leave chip 0 for chip 1: the first hover clears, the second arrives.
    cx.simulate_mouse_move(
        point(px(90.), px(14.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    assert!(
        !state_of(&recorded, "alpha").is_hovered && state_of(&recorded, "beta").is_hovered,
        "the hover must follow the pointer between chips"
    );

    // A click on chip 1 moves the single selection to it: Single collapses to
    // the clicked key, so the closure sees the first chip lose its tick — and
    // the click itself seats the group's focus and roving cursor on Beta.
    // Pointer focus: `is_focused` without the ring, because the app root
    // cleared the focus-visible flag on the mouse-down.
    click(cx, 90., 14.);
    flush_frame(cx);
    assert!(
        state_of(&recorded, "beta").is_selected,
        "the clicked chip must report selected"
    );
    assert!(
        !state_of(&recorded, "alpha").is_selected,
        "the single selection must have moved off the first chip"
    );
    let beta = state_of(&recorded, "beta");
    assert!(
        beta.is_focused && !beta.is_focus_visible,
        "the click's pointer seat must focus the clicked chip without a focus-visible ring"
    );
    assert!(
        !state_of(&recorded, "alpha").is_focused,
        "the pointer seat must carry the cursor off the first chip"
    );

    // Keyboard re-entry: Tab leaves the pointer-focused group for the button
    // after it, clearing every render-prop focus and ring state, and the next
    // Tab wraps back in — where the ring shows, because this focus came from
    // the keyboard.
    press(cx, "tab");
    flush_frame(cx);
    assert!(
        ["alpha", "beta"].iter().all(|key| {
            let state = state_of(&recorded, key);
            !state.is_focused && !state.is_focus_visible
        }),
        "moving focus out of the TagGroup must clear every render-prop focus and ring state"
    );
    press(cx, "tab");
    flush_frame(cx);
    let beta = state_of(&recorded, "beta");
    assert!(
        beta.is_focused && beta.is_focus_visible,
        "keyboard re-entry must ring the TagGroup's roving stop"
    );
    assert!(
        !state_of(&recorded, "alpha").is_focused,
        "keyboard re-entry must land on the seated cursor's chip"
    );
}

/// The interaction slot and its `track_interaction` handlers are attached
/// only when a `tag_content` closure is set (`tag_group.rs` gates both on
/// `tag_content.is_some()`), so a plain group must behave exactly as the
/// closure twin does — the gating cannot regress the default path. The
/// per-frame cost of the slot itself (keyed state + three mouse handlers) is
/// not observable from a behaviour test; what is observable, and asserted
/// here, is that the plain group selects, removes and roves identically.
#[gpui::test]
fn tag_group_without_content_needs_no_slot(cx: &mut TestAppContext) {
    let selected = events();
    let selections = selected.clone();
    let removed = events();
    let removals = removed.clone();
    // A TagGroup's selection is caller-owned (`selected_keys` is a prop and
    // `on_selection_change` hands back the next set), so the toggle only
    // survives a re-render if the caller renders its copy back in — the same
    // loop the closure-set tests above run.
    let held = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
    let cx = open_host(cx, move || {
        let selected = selected.clone();
        let removed = removed.clone();
        let held = held.clone();
        let selected_now = held.borrow().clone();
        TagGroup::new(
            "rp-tg-plain",
            vec![Tag::new("alpha", "Alpha"), Tag::new("beta", "Beta")],
        )
        .selection_mode(SelectionMode::Single)
        .selected_keys(selected_now)
        .on_selection_change(move |keys, window, _| {
            selected.borrow_mut().push(sorted_join(keys));
            *held.borrow_mut() = keys.clone();
            window.refresh();
        })
        .on_remove(move |keys, _, _| removed.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    // Keyboard, zero geometry: Tab enters the group on the first tag, Enter
    // toggles it (gpui activates a focused element's click on key up), and
    // Delete reports the focused tag — the plain group roves exactly as the
    // closure twin in `collections.rs::tag_group_remove_reports_the_key`.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        selections.borrow().as_slice(),
        ["alpha"],
        "the plain group must answer Enter like the closure twin"
    );
    press(cx, "enter");
    assert_eq!(
        selections.borrow().as_slice(),
        ["alpha", ""],
        "Enter again must toggle the single selection off"
    );

    // Pointer, measured: the label ("Alpha", 12px default weight) is shaped
    // by the same text system the renderer uses, so chip 0's label centre is
    // x = px-2 (8) + w/2 at the vertical middle of the 28px chip (y = 14),
    // and the click selects exactly as the closure-set chips did.
    let w =
        cx.update(|window, _| text_width(window.text_system(), "Alpha", 12.0, FontWeight::NORMAL));
    click(cx, 8. + w / 2., 14.);
    assert_eq!(
        selections.borrow().as_slice(),
        ["alpha", "", "alpha"],
        "the plain group must answer the pointer like the closure twin"
    );

    press(cx, "delete");
    assert_eq!(
        removals.borrow().as_slice(),
        ["alpha"],
        "Delete must remove the focused tag of the plain group"
    );
}

// ---------------------------------------------------------------------------
// RadioGroup::option_content
// ---------------------------------------------------------------------------

#[gpui::test]
fn radio_group_option_content_hands_v3_field_state(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(Vec::new()));
    let record = recorded.clone();
    let _cx = open_host(cx, move || {
        let record = record.clone();
        RadioGroup::new("rp-rg-field-state", vec!["One".into()])
            .is_disabled(true)
            .is_invalid(true)
            .is_required(true)
            .is_read_only(true)
            .option_content(move |_, state| {
                record.borrow_mut().push((
                    state.is_selected,
                    state.is_disabled,
                    state.is_read_only,
                    state.is_invalid,
                    state.is_required,
                ));
                gpui::div().into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        recorded.borrow().as_slice(),
        [(false, true, true, true, true)],
        "the Radio root render prop must receive every pinned field-level state"
    );
}

#[gpui::test]
fn radio_group_option_content_keeps_per_option_state_local(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let _cx = open_host(cx, move || {
        let record = record.clone();
        RadioGroup::new(
            "rp-rg-local-field-state",
            vec![
                RadioOption::new("One")
                    .is_disabled(true)
                    .error_message("Unavailable"),
                RadioOption::new("Two"),
            ],
        )
        .option_content(move |label, state| {
            record_radio_state(&record, label, state);
            gpui::div().into_any_element()
        })
        .into_any_element()
    });

    let one = radio_state_of(&recorded, "One");
    let two = radio_state_of(&recorded, "Two");
    assert!(one.is_disabled && one.is_invalid);
    assert!(!two.is_disabled && !two.is_invalid);
}

/// The closure runs once per option on the first frame — a panic here is the
/// button's defect class on a fourth component — and records the option's
/// label and field state before any interaction occurs.
#[gpui::test]
fn radio_group_option_content_renders_at_all(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let _cx = open_host(cx, move || {
        let record = record.clone();
        RadioGroup::new(
            "rp-rg-render",
            vec!["One".into(), "Two".into(), "Three".into()],
        )
        .option_content(move |label, state| {
            record_radio_state(&record, label, state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .into_any_element()
    });

    let one = radio_state_of(&recorded, "One");
    assert!(
        !one.is_selected
            && !one.is_disabled
            && !one.is_read_only
            && !one.is_invalid
            && !one.is_required,
        "the first option must receive an idle field state"
    );
    for label in ["Two", "Three"] {
        let state = radio_state_of(&recorded, label);
        assert!(
            !state.is_selected
                && !state.is_disabled
                && !state.is_read_only
                && !state.is_invalid
                && !state.is_required,
            "{label} must receive an idle field state"
        );
    }
}

/// Selection in the root field state follows both pointer and arrow changes.
#[gpui::test]
fn radio_group_option_content_tracks_selection(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let picks = events();
    let picked = picks.clone();
    let record = recorded.clone();
    let cx = open_host(cx, move || {
        let record = record.clone();
        let picks = picks.clone();
        RadioGroup::new(
            "rp-rg-state",
            vec!["One".into(), "Two".into(), "Three".into()],
        )
        .default_value("One")
        .option_content(move |label, state| {
            record_radio_state(&record, label, state);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .on_change(move |value, _, _| picks.borrow_mut().push(value.to_string()))
        .into_any_element()
    });

    // Seeded selection: row 0 is set, and the roving stop sits on it.
    assert!(
        radio_state_of(&recorded, "One").is_selected,
        "the default selection must reach the closure"
    );

    // A pointer pick on row 1: the row (16px control + 12px gap + 40px
    // content = 68px wide, 20px tall) centres at (48, 46), the next row
    // stepping 20 + 16px. The selection moves and the stop follows.
    click(cx, 48., 46.);
    flush_frame(cx);
    assert!(
        radio_state_of(&recorded, "Two").is_selected,
        "the clicked option must report selected"
    );
    assert!(
        !radio_state_of(&recorded, "One").is_selected,
        "the earlier selection must be replaced"
    );
    assert_eq!(picked.borrow().as_slice(), ["Two"]);

    // Tab enters the group on the selected row; Down roves to the third
    // option, whose claim on the group's handle moves with the selection.
    press(cx, "tab");
    press(cx, "down");
    flush_frame(cx);
    let three = radio_state_of(&recorded, "Three");
    assert!(three.is_selected, "the arrow must select the third option");
    assert_eq!(picked.borrow().as_slice(), ["Two", "Three"]);
}

// ---------------------------------------------------------------------------
// Calendar::cell
// ---------------------------------------------------------------------------

/// The cell closure ran for every visible cell on the first frame — a panic
/// is the button's defect class on a fifth component — and recorded the state
/// v3 hands `Calendar.Cell`: the formatted day label and the flags, for all
/// 31 in-month days and both adjacent months' spill cells. The recorder keys
/// each invocation by date so distinct cells cannot merge; the identity test
/// below pins every adjacent-month date and label.
#[gpui::test]
fn calendar_cell_renders_at_all(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let _cx = open_host(cx, move || {
        let record = record.clone();
        // `defaultValue` seeds the selection and the view month (August
        // 2026), so the grid and the click arithmetic are deterministic.
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .first_day_of_week(Weekday::Mon)
            .cell(move |state| {
                record_cell(&record, state);
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    // The seeded pick reached the closure with its day label.
    let selected = cell_of(&recorded, "2026-08-15");
    assert_eq!(selected.formatted_date, "15");
    assert!(selected.is_selected && !selected.is_outside_month && !selected.is_disabled);

    // August 2026 starts on a Saturday, so its six rows contain five July
    // cells, 31 August cells, and six September cells. React Aria invokes the
    // renderer for all 42 real dates; outside-month cells are disabled and
    // cannot be selected, but their other state still comes from the date.
    let constraints = DateConstraints {
        first_day_of_week: Weekday::Mon,
        ..Default::default()
    };
    let lead = constraints.lead_cells(2026, 8);
    assert_eq!(lead, 5, "the month's own derivation must give a lead");
    let rows = constraints.rows(2026, 8);
    let cells = recorded.borrow();
    let in_month = cells.values().filter(|cell| !cell.is_outside_month).count();
    assert_eq!(
        in_month, 31,
        "every in-month day must render with the closure"
    );
    let spills = cells.values().filter(|cell| cell.is_outside_month).count();
    assert_eq!(
        spills,
        rows * 7 - 31,
        "both adjacent months' cells must reach the render closure"
    );
    for cell in cells.values() {
        if cell.is_outside_month {
            assert!(cell.is_disabled, "an outside-month cell must be disabled");
            assert!(
                !cell.is_unavailable,
                "the unconfigured unavailable predicate is false"
            );
            assert!(
                !cell.is_selected,
                "an outside-month copy cannot report selected"
            );
        } else {
            assert!(
                !cell.is_disabled,
                "an unconstrained in-month cell must not report disabled"
            );
        }
    }
}

/// v3 hands a spill cell the *actual* next-month date, and the closure is the
/// only identity a caller has — this port added the `date` field precisely so
/// `state.date` can key a custom cell. `calendar.rs` `month_grid` used to pass
/// `Date::new(y, m, 1)` for every spill cell, so the six September cells were
/// all "2026-08-01": a caller drawing from `state.date` rendered six identical
/// cells and the real next-month days were unreachable. Each spill cell now
/// carries its own next-month date.
#[gpui::test]
fn calendar_cell_spill_dates_preserve_their_identity(cx: &mut TestAppContext) {
    let invocations = Rc::new(RefCell::new(Vec::<(String, String, bool)>::new()));
    let record = invocations.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let _cx = open_host(cx, move || {
        let record = record.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .first_day_of_week(Weekday::Mon)
            .cell(move |state| {
                record.borrow_mut().push((
                    state.date.format_iso(),
                    state.formatted_date.to_string(),
                    state.is_outside_month,
                ));
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    // The five July cells and six September cells each carry their own date
    // and formatted day label. A missing edge or duplicated identity changes
    // this exact map.
    let spills: HashMap<String, String> = invocations
        .borrow()
        .iter()
        .filter(|(_, _, outside)| *outside)
        .map(|(date, label, _)| (date.clone(), label.clone()))
        .collect();
    assert_eq!(
        spills,
        HashMap::from([
            ("2026-07-27".into(), "27".into()),
            ("2026-07-28".into(), "28".into()),
            ("2026-07-29".into(), "29".into()),
            ("2026-07-30".into(), "30".into()),
            ("2026-07-31".into(), "31".into()),
            ("2026-09-01".into(), "1".into()),
            ("2026-09-02".into(), "2".into()),
            ("2026-09-03".into(), "3".into()),
            ("2026-09-04".into(), "4".into()),
            ("2026-09-05".into(), "5".into()),
            ("2026-09-06".into(), "6".into()),
        ]),
        "both spill edges must hand the closure their real dates and labels"
    );
}

/// Outside-month cells are disabled copies, but React Aria still evaluates
/// `isDateUnavailable` for their real adjacent dates before handing render
/// props to the child closure.
#[gpui::test]
fn calendar_outside_cells_keep_date_derived_unavailable_state(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let _cx = open_host(cx, move || {
        let record = record.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .is_date_unavailable(|date| {
                date == Date::new(2026, 7, 31) || date == Date::new(2026, 9, 1)
            })
            .cell(move |state| {
                record_cell(&record, state);
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    for date in ["2026-07-31", "2026-09-01"] {
        let cell = cell_of(&recorded, date);
        assert!(
            cell.is_outside_month && cell.is_disabled && cell.is_unavailable && !cell.is_selected,
            "outside cell {date} must retain its predicate-derived unavailable state"
        );
    }
}

/// `isUnavailable`, `isDisabled`, and read-only interaction are independent
/// React Aria states. Read-only blocks presses without making every cell
/// disabled, and an unavailable in-month date remains focusable.
#[gpui::test]
fn calendar_cell_distinguishes_read_only_unavailable_and_disabled(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        let record = record.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .is_read_only(true)
            .is_date_unavailable(|date| date == Date::new(2026, 8, 16))
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".into(), |date| date.format_iso()));
            })
            .cell(move |state| {
                record_cell(&record, state);
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let selected = cell_of(&recorded, "2026-08-15");
    assert!(selected.is_selected && !selected.is_unavailable && !selected.is_disabled);
    let unavailable = cell_of(&recorded, "2026-08-16");
    assert!(unavailable.is_unavailable && !unavailable.is_selected && !unavailable.is_disabled);
    press(cx, "tab");
    press(cx, "right");
    let (x17, y17) = cal_day(2026, 8, 17);
    click(cx, x17, y17);
    let (x16, y16) = cal_day(2026, 8, 16);
    click(cx, x16, y16);
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-16"],
        "read-only and unavailable cells remain keyboard-focusable but must be pointer-inert"
    );
    assert!(
        changed.borrow().is_empty(),
        "read-only cells must never report a selection change"
    );
}

/// A controlled value can become unavailable or fall outside new bounds.
/// React Aria keeps the value but does not expose that disabled cell as
/// selected through Calendar.Cell render props.
#[gpui::test]
fn calendar_cell_suppresses_selection_for_unavailable_and_disabled_values(cx: &mut TestAppContext) {
    let disabled_cells = Rc::new(RefCell::new(HashMap::new()));
    let disabled_record = disabled_cells.clone();
    let unavailable_cells = Rc::new(RefCell::new(HashMap::new()));
    let unavailable_record = unavailable_cells.clone();
    let disabled_state = cx.new(|cx| CalendarState::new(cx));
    let unavailable_state = cx.new(|cx| CalendarState::new(cx));
    let _cx = open_host(cx, move || {
        let disabled_record = disabled_record.clone();
        let unavailable_record = unavailable_record.clone();
        gpui::div()
            .child(
                Calendar::new(disabled_state.clone())
                    .default_value(Date::new(2026, 8, 14))
                    .min_value(Date::new(2026, 8, 15))
                    .cell(move |state| {
                        record_cell(&disabled_record, state);
                        gpui::div().into_any_element()
                    }),
            )
            .child(
                Calendar::new(unavailable_state.clone())
                    .default_value(Date::new(2026, 8, 16))
                    .is_date_unavailable(|date| date == Date::new(2026, 8, 16))
                    .cell(move |state| {
                        record_cell(&unavailable_record, state);
                        gpui::div().into_any_element()
                    }),
            )
            .into_any_element()
    });

    let disabled = cell_of(&disabled_cells, "2026-08-14");
    assert!(disabled.is_disabled && !disabled.is_selected);
    let unavailable = cell_of(&unavailable_cells, "2026-08-16");
    assert!(unavailable.is_unavailable && !unavailable.is_disabled && !unavailable.is_selected);
}

/// Multiple-mode deselection must leave the keyed cursor on the pressed date.
/// Enter then toggles that same date back on instead of acting on the last
/// remaining selection.
#[gpui::test]
fn calendar_multiple_deselection_keeps_pointer_focus(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let focuses = events();
    let focused = focuses.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let focuses = focuses.clone();
        Calendar::new(state_for_view.clone())
            .selection_mode(SelectionMode::Multiple)
            .default_values([Date::new(2026, 8, 15), Date::new(2026, 8, 16)])
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change_all(move |dates, _, _| {
                changes.borrow_mut().push(
                    dates
                        .iter()
                        .map(Date::format_iso)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    let (x16, y16) = cal_day(2026, 8, 16);
    click(cx, x16, y16);
    press(cx, "enter");
    assert_eq!(focused.borrow().as_slice(), ["2026-08-16"]);
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-15", "2026-08-15,2026-08-16"],
        "Enter after deselection must toggle the pointer-focused date back on"
    );
}

/// An adjacent-month copy can represent a date selected in another visible
/// month, but React Aria reports the outside copy unselected and inert.
#[gpui::test]
fn calendar_outside_selected_copy_is_unselected_and_inert(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    state.update(cx, |state, _| {
        state.toggle(Date::new(2026, 9, 1), SelectionMode::Single);
        state.set_anchor(Date::new(2026, 8, 15));
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let record = record.clone();
        Calendar::new(state_for_view.clone())
            .first_day_of_week(Weekday::Mon)
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".into(), |date| date.format_iso()));
            })
            .cell(move |state| {
                record_cell(&record, state);
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let outside = cell_of(&recorded, "2026-09-01");
    assert!(outside.is_outside_month && outside.is_disabled && !outside.is_selected);
    click(cx, cal_col_x(0), cal_row_y(0));
    click(cx, cal_col_x(1), cal_row_y(5));
    assert!(
        changed.borrow().is_empty(),
        "neither outside-month edge may report a selection"
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected()),
        Some(Date::new(2026, 9, 1)),
        "outside presses must preserve the selected adjacent-month date"
    );
}

/// Multi-month layouts render the boundary date once in each month grid. The
/// two copies must remain distinct: one is an inert outside-month cell and the
/// other is the real in-month cell.
#[gpui::test]
fn calendar_multimonth_boundary_cells_keep_independent_state(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(Vec::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let _cx = open_host(cx, move || {
        let record = record.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .visible_duration(VisibleDuration::Months(2))
            .cell(move |state| {
                if matches!(
                    state.date.format_iso().as_str(),
                    "2026-08-31" | "2026-09-01"
                ) {
                    record
                        .borrow_mut()
                        .push((state.date.format_iso(), state.is_outside_month));
                }
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let mut boundary = recorded.borrow().clone();
    boundary.sort();
    assert_eq!(
        boundary,
        [
            ("2026-08-31".into(), false),
            ("2026-08-31".into(), true),
            ("2026-09-01".into(), false),
            ("2026-09-01".into(), true),
        ],
        "each grid must keep its in-month and outside-month copy independent"
    );
}

/// A pick moves the selection the closure sees: clicking day 16 replaces the
/// seeded 15th, both rows of the state changing in the same frame — a
/// selection the closure only pretends to draw would sit still.
#[gpui::test]
fn calendar_cell_tracks_selection(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let record = record.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .cell(move |state| {
                record_cell(&record, state);
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    assert!(
        cell_of(&recorded, "2026-08-15").is_selected,
        "the seeded pick must reach the closure"
    );

    // Day 16 sits one slot to the right of day 15 in the same row; a click
    // toggles the single selection onto it.
    let (x, y) = cal_day(2026, 8, 16);
    click(cx, x, y);
    flush_frame(cx);
    assert!(
        cell_of(&recorded, "2026-08-16").is_selected,
        "the picked day must report selected"
    );
    assert!(
        !cell_of(&recorded, "2026-08-15").is_selected,
        "the single selection must have left the seeded day"
    );
}

// ---------------------------------------------------------------------------
// RangeCalendar::cell
// ---------------------------------------------------------------------------

/// The closure ran for every real date in the pinned six-week grid on the
/// first frame, including both adjacent months. With no anchor or end, every
/// cell is idle and only the outside-month copies are disabled.
#[gpui::test]
fn range_calendar_cell_renders_at_all(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    // Pin the visible month so the closure's cells are deterministic,
    // whatever day the suite runs on.
    state.update(cx, |s, _| {
        s.view_year = 2026;
        s.view_month = 8;
        s.view_day = 1;
        s.user_navigated = true;
    });
    let state_for_view = state;
    let _cx = open_host(cx, move || {
        let record = record.clone();
        RangeCalendar::new(state_for_view.clone())
            .cell(move |state| {
                record_range_cell(&record, state);
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let five = range_cell_of(&recorded, "2026-08-05");
    assert!(
        !five.is_selected && !five.is_selection_start && !five.is_selection_end,
        "every cell must record the idle range state on the first frame"
    );
    assert!(
        !five.is_disabled && !five.is_outside_month,
        "an in-month day of an enabled calendar must report neither"
    );

    let cells = recorded.borrow();
    assert_eq!(
        cells.len(),
        42,
        "the six-week grid must expose 42 real dates"
    );
    assert_eq!(
        cells.values().filter(|cell| !cell.is_outside_month).count(),
        31,
        "all August dates must remain in-month"
    );
    assert_eq!(
        cells.values().filter(|cell| cell.is_outside_month).count(),
        11,
        "the five July and six September copies must reach the closure"
    );
    for cell in cells.values().filter(|cell| cell.is_outside_month) {
        assert!(cell.is_disabled && !cell.is_selected);
        assert!(!cell.is_selection_start && !cell.is_selection_end);
    }
}

/// Both spill edges preserve their real dates and labels. Blank leading slots
/// or a repeated synthetic date would make this exact map differ.
#[gpui::test]
fn range_calendar_spill_dates_preserve_their_identity(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let _cx = open_host(cx, move || {
        let record = record.clone();
        RangeCalendar::new(state.clone())
            .first_day_of_week(Weekday::Mon)
            .cell(move |cell| {
                record_range_cell(&record, cell);
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let spills: HashMap<String, String> = recorded
        .borrow()
        .values()
        .filter(|cell| cell.is_outside_month)
        .map(|cell| (cell.date.format_iso(), cell.formatted_date.to_string()))
        .collect();
    assert_eq!(
        spills,
        HashMap::from([
            ("2026-07-27".into(), "27".into()),
            ("2026-07-28".into(), "28".into()),
            ("2026-07-29".into(), "29".into()),
            ("2026-07-30".into(), "30".into()),
            ("2026-07-31".into(), "31".into()),
            ("2026-09-01".into(), "1".into()),
            ("2026-09-02".into(), "2".into()),
            ("2026-09-03".into(), "3".into()),
            ("2026-09-04".into(), "4".into()),
            ("2026-09-05".into(), "5".into()),
            ("2026-09-06".into(), "6".into()),
        ])
    );
}

/// `weeksInMonth` is an exact nonzero row count in pinned React Stately, even
/// above the usual six rows. The extra row must contain real spill dates too.
#[gpui::test]
fn range_calendar_weeks_in_month_keeps_exact_row_count(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let _cx = open_host(cx, move || {
        let record = record.clone();
        RangeCalendar::new(state.clone())
            .first_day_of_week(Weekday::Mon)
            .weeks_in_month(7)
            .cell(move |cell| {
                record_range_cell(&record, cell);
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(recorded.borrow().len(), 49);
    let final_cell = range_cell_of(&recorded, "2026-09-13");
    assert!(final_cell.is_outside_month && final_cell.is_disabled);
}

/// Outside-month copies are disabled and unselected, but their unavailable
/// state still comes from the predicate evaluated against the real date.
#[gpui::test]
fn range_calendar_outside_cells_keep_date_derived_state(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.start = Some(Date::new(2026, 9, 1));
        state.end = Some(Date::new(2026, 9, 2));
        state.user_navigated = true;
    });
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let record = record.clone();
        RangeCalendar::new(state.clone())
            .is_date_unavailable(|date, _| date == Date::new(2026, 7, 31))
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}..{}", start.format_iso(), end.format_iso()));
            })
            .cell(move |cell| {
                record_range_cell(&record, cell);
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let july = range_cell_of(&recorded, "2026-07-31");
    assert!(july.is_outside_month && july.is_disabled && july.is_unavailable);
    assert!(!july.is_selected && !july.is_selection_start && !july.is_selection_end);
    let september = range_cell_of(&recorded, "2026-09-01");
    assert!(september.is_outside_month && september.is_disabled && !september.is_selected);
    assert!(
        september.is_selection_start,
        "RAC preserves endpoint identity even though the outside copy is not selected"
    );

    click(cx, range_col_x(0), range_row_y(0));
    click(cx, range_col_x(1), range_row_y(5));
    assert!(
        changed.borrow().is_empty(),
        "neither outside-month edge may report a range change"
    );
}

/// Read-only blocks pointer selection without turning cells disabled, while an
/// unavailable date reports its own state independently of read-only.
#[gpui::test]
fn range_calendar_read_only_and_unavailable_states_stay_independent(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.start = Some(Date::new(2026, 8, 15));
        state.end = Some(Date::new(2026, 8, 17));
        state.user_navigated = true;
    });
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let record = record.clone();
        RangeCalendar::new(state.clone())
            .is_read_only(true)
            .is_date_unavailable(|date, _| date == Date::new(2026, 8, 16))
            .on_change(move |_, _, _, _| changes.borrow_mut().push("changed".into()))
            .cell(move |cell| {
                record_range_cell(&record, cell);
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let start = range_cell_of(&recorded, "2026-08-15");
    assert!(start.is_selected && start.is_selection_start && !start.is_disabled);
    let unavailable = range_cell_of(&recorded, "2026-08-16");
    assert!(unavailable.is_unavailable && !unavailable.is_disabled && !unavailable.is_selected);
    let (start_x, start_y) = range_day(2026, 8, 15);
    click(cx, start_x, start_y);
    let (unavailable_x, unavailable_y) = range_day(2026, 8, 16);
    click(cx, unavailable_x, unavailable_y);
    assert!(changed.borrow().is_empty());
}

/// Adjacent grids may render the same boundary date twice. The in-month copy
/// carries selection while the outside copy keeps endpoint identity but not
/// selected state.
#[gpui::test]
fn range_calendar_multimonth_boundary_copies_keep_independent_state(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(Vec::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.start = Some(Date::new(2026, 8, 31));
        state.end = Some(Date::new(2026, 9, 2));
        state.user_navigated = true;
    });
    let _cx = open_host(cx, move || {
        let record = record.clone();
        RangeCalendar::new(state.clone())
            .visible_duration(VisibleDuration::Months(2))
            .cell(move |cell| {
                if matches!(cell.date.format_iso().as_str(), "2026-08-31" | "2026-09-01") {
                    record.borrow_mut().push((
                        cell.date.format_iso(),
                        cell.is_outside_month,
                        cell.is_selected,
                        cell.is_selection_start,
                    ));
                }
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let mut boundary = recorded.borrow().clone();
    boundary.sort();
    assert_eq!(
        boundary,
        [
            ("2026-08-31".into(), false, true, true),
            ("2026-08-31".into(), true, false, true),
            ("2026-09-01".into(), false, true, false),
            ("2026-09-01".into(), true, false, false),
        ]
    );
}

/// A completed pointer range leaves the keyed cursor on its end. Enter then
/// starts the next range at that end rather than jumping back to the old start.
#[gpui::test]
fn range_calendar_pointer_end_hands_focus_to_keyboard(cx: &mut TestAppContext) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        RangeCalendar::new(state.clone())
            .on_change(move |start, end, _, _| {
                changes.borrow_mut().push((*start, *end));
            })
            .into_any_element()
    });

    let (five_x, five_y) = range_day(2026, 8, 5);
    click(cx, five_x, five_y);
    let (eight_x, eight_y) = range_day(2026, 8, 8);
    click(cx, eight_x, eight_y);
    press(cx, "enter");
    assert_eq!(
        changed.borrow().as_slice(),
        [(Date::new(2026, 8, 5), Date::new(2026, 8, 8))]
    );
}

/// The range lifecycle, watched through the cell closure: the first click
/// anchors (isSelectionStart), the hover preview extends the half-open range
/// (the hovered day becomes isSelectionEnd and the days between turn
/// isSelected), and the second click completes it (the end is fixed at the
/// clicked day).
#[gpui::test]
fn range_calendar_cell_tracks_anchor_preview_and_range(cx: &mut TestAppContext) {
    let recorded = Rc::new(RefCell::new(HashMap::new()));
    let record = recorded.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |s, _| {
        s.view_year = 2026;
        s.view_month = 8;
        s.view_day = 1;
        s.user_navigated = true;
    });
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let record = record.clone();
        RangeCalendar::new(state_for_view.clone())
            .cell(move |state| {
                record_range_cell(&record, state);
                gpui::div().w(px(20.)).h(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    // First click: the anchor. The cell reports the half-open start.
    let (five_x, five_y) = range_day(2026, 8, 5);
    click(cx, five_x, five_y);
    flush_frame(cx);
    let five = range_cell_of(&recorded, "2026-08-05");
    assert!(
        five.is_selection_start && five.is_selected,
        "the anchor cell must report the open-ended start"
    );
    assert!(
        five.is_selection_end,
        "the pointer over the anchor previews a one-day range"
    );

    // Hover day 8: the preview paints the open range — the hovered day is
    // the moving end, the days between turn selected.
    let (eight_x, eight_y) = range_day(2026, 8, 8);
    cx.simulate_mouse_move(
        point(px(eight_x), px(eight_y)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    flush_frame(cx);
    let eight = range_cell_of(&recorded, "2026-08-08");
    assert!(
        eight.is_selection_end && eight.is_selected,
        "the hovered day must report the preview end"
    );
    assert!(
        range_cell_of(&recorded, "2026-08-07").is_selected,
        "a day between the anchor and the preview must report selected"
    );
    assert!(
        !range_cell_of(&recorded, "2026-08-10").is_selected,
        "a day past the preview must stay unselected"
    );

    // Second click on day 12: the range completes. The anchor keeps its
    // start, the clicked day takes the end, and the whole run is selected.
    let (twelve_x, twelve_y) = range_day(2026, 8, 12);
    click(cx, twelve_x, twelve_y);
    flush_frame(cx);
    let twelve = range_cell_of(&recorded, "2026-08-12");
    assert!(
        twelve.is_selection_end && twelve.is_selected,
        "the picked end must report the closed range"
    );
    assert!(
        range_cell_of(&recorded, "2026-08-05").is_selection_start,
        "the anchor must keep its start"
    );
    assert!(
        range_cell_of(&recorded, "2026-08-10").is_selected,
        "a day inside the range must report selected"
    );
    assert!(
        !range_cell_of(&recorded, "2026-08-14").is_selected,
        "a day outside the range must stay unselected"
    );
}
