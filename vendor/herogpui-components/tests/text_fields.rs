//! Behaviour tests for the TEXT components: TextField, TextArea, SearchField,
//! InputOTP, InputGroup, ColorField, DateField and Fieldset.
//!
//! Everything static about them is measured by the `.shots/*.py` audits; these
//! tests drive the controls with simulated keystrokes and clicks and assert on
//! recorded callbacks and state entities the test owns — never on appearance.
//!
//! Geometry is derived from the components' own constants, not guessed:
//!
//! - Every single-line field is `util::FIELD_HEIGHT` = 36px tall at the window
//!   origin and wrapped in `max_w(320)` unless full width (`Input::render`).
//!   Its `px(12)` padding puts the 20px `size-5` clear button at x 288..308,
//!   centre (298, 18) — the same maths as ComboBox's chevron in pickers.rs.
//!   `SearchField`'s 16px magnifier sits at x 12..28, so the text starts at
//!   12 + 16 + 8 (gap) = 36.
//! - `ColorField` in editable mode delegates to `Input` with a 16px
//!   `ColorSwatch` start content, so the text starts at the same 36.
//! - `TextArea` is the same `Input` in multi-line mode with
//!   `rows_height(3)` = 20·3 + 16 = 76px (`textarea.rs`), `py(8)` and the text
//!   at the top. An EMPTY textarea hugs its content: the wrapping row has no
//!   `whitespace_nowrap` to force a width, so the box measures only ~32px
//!   until text wraps inside it (measured by probing: a click at x = 40
//!   misses, x = 20 focuses). With text in it, each paragraph reports its own
//!   painted bounds, and a click is placed against the one it lands in.
//! - `InputOTP` cells are 38x40 with an 8px gap (`input_otp.rs`), so cell *i*
//!   spans x 46i..46i+38 and every cell centre is y 20; a second instance on
//!   the same page sits `gap(16)` below, cell 0 centre y = 40 + 16 + 20 = 76.
//! - `InputGroup` is `min-h-9` with no padding of its own: the `InputAddon`s
//!   carry `px(12)` each (`input_group.rs`). In a `w(px(400.))` wrapper the
//!   addons are flush against the wrapper's edges — a click at x = 6 lands in
//!   the prefix's left padding and one at x = 394 in the suffix's right
//!   padding for any glyph width. v3.2.4's `InputGroupRoot.handleClick`
//!   focuses the contained input when a click lands on the group outside it,
//!   so an addon click leaves the field focused — and it never touches the
//!   caret, which is `InputState`'s alone and is written only by the field's
//!   own mouse-down (`window.focus` moves the handle, nothing else).
//! - A `Fieldset` is `gap(24)` between children (`field.rs`); its legend is a
//!   24px line (`FieldsetLegend::render`), so a Group under it starts at y 48
//!   and its bare 36px field centres at (60, 66).
//!
//! Where the component reads the keyboard (the `track_focus`'d handle), Tab is
//! the entry point exactly as `fields.rs` drives `TimeField`; where a click is
//! what focuses, the pointer lands inside the field before typing begins.
//! Chords are dispatched with their modifiers parsed (`ctrl-a`), unlike the
//! screenshot driver's posted messages, which is what the select-all test
//! relies on.

use crate::harness;
use crate::source_scan;

use gpui::{
    point, prelude::*, px, Bounds, Entity, Focusable, Modifiers, MouseButton, Pixels,
    TestAppContext, VisualTestContext,
};
use herogpui_components::{
    Button, ColorField, Date, DateField, FieldGroup, Fieldset, FieldsetLegend, Input, InputAddon,
    InputGroup, InputOTP, InputState, InputType, NumberField, NumberState, OtpPattern, OtpState,
    PickerColor, SearchField, TextArea, TextField, Time, TimeField, TimeState, ValidationBehavior,
};
use herogpui_theme::ActiveTheme;

use harness::{click, events, open_host, press, probe as inset_probe};

/// One forced redraw, so `debug_bounds` sees the latest laid-out frame.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

const SEARCH_CLEAR_BOX: f32 = 20.;
const SEARCH_CLEAR_PRESS_SCALE: f32 = 0.93;

fn clear_probe(entity_id: u64) -> &'static str {
    Box::leak(format!("input-clear-{entity_id}").into_boxed_str())
}

fn clear_bounds(cx: &mut VisualTestContext, entity_id: u64) -> Bounds<Pixels> {
    cx.debug_bounds(clear_probe(entity_id))
        .unwrap_or_else(|| panic!("the SearchField clear button {entity_id} must paint"))
}

fn bounds_centre(bounds: Bounds<Pixels>) -> gpui::Point<Pixels> {
    point(
        bounds.origin.x + bounds.size.width / 2.,
        bounds.origin.y + bounds.size.height / 2.,
    )
}

fn near(value: Pixels, expected: f32) -> bool {
    (f32::from(value) - expected).abs() < 0.5
}

// ---------------------------------------------------------------------------
// TextField
// ---------------------------------------------------------------------------

#[gpui::test]
fn text_field_accepts_platform_text_without_a_printable_key(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "replace me"));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        TextField::new(state_for_view.clone())
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .into_any_element()
    });
    click(cx, 60., 18.);
    cx.simulate_keystrokes("ctrl-a");
    cx.update(|window, app| {
        window.dispatch_keystroke(
            gpui::Keystroke {
                key: "unidentified".into(),
                key_char: Some("東京😀".into()),
                modifiers: Modifiers::default(),
            },
            app,
        );
    });
    assert_eq!(
        state.read_with(cx, |state, _| state.value().to_owned()),
        "東京😀"
    );
    assert_eq!(recorded.borrow().as_slice(), ["東京😀"]);
}

#[gpui::test]
fn text_field_typing_reports_and_holds(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        TextField::new(state_for_view.clone())
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .into_any_element()
    });

    // Click into the field (36px tall, 320px wide at the origin), which
    // focuses it; the empty value puts the caret at 0.
    click(cx, 60., 18.);
    cx.simulate_input("hello");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["h", "he", "hel", "hell", "hello"],
        "typing must report the growing value on every keystroke"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "hello", "the InputState must hold what was typed");

    // Caret motion is behavioural: Left Left from the end puts the caret at
    // index 3 (`InputState` owns cursor/anchor), so the next char lands
    // mid-string instead of at the end.
    press(cx, "left");
    press(cx, "left");
    press(cx, "x");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["h", "he", "hel", "hell", "hello", "helxlo"],
        "a keystroke after Left Left must insert at cursor 3, not append"
    );
    let moved = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        moved, "helxlo",
        "the InputState must hold the mid-string insert"
    );
}

#[gpui::test]
fn text_field_backspace_and_select_all(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        TextField::new(state_for_view.clone())
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("hello");
    press(cx, "backspace");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["h", "he", "hel", "hell", "hello", "hell"],
        "Backspace must delete the char before the caret and report the loss"
    );

    // The test platform parses "ctrl-a" into a keystroke carrying the control
    // modifier (unlike the screenshot driver's posted messages), so the
    // field's select-all branch must run.
    press(cx, "ctrl-a");
    let selection = cx.update(|_, cx| state.read(cx).selection());
    assert_eq!(
        selection,
        Some((0, 4)),
        "Ctrl+A must select the whole value: anchor 0 to cursor 4"
    );

    press(cx, "x");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["h", "he", "hel", "hell", "hello", "hell", "x"],
        "typing over a selection must replace it outright"
    );
    let replaced = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(replaced, "x", "the InputState must hold the replaced value");
}

#[gpui::test]
fn input_max_length_blocks_rejected_edits_without_change(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Input::new(state_for_view.clone())
            .max_length(3)
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("abcd");
    press(cx, "space");
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("z".to_owned()));
    press(cx, "ctrl-v");
    press(cx, "delete");
    press(cx, "home");
    press(cx, "backspace");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a", "ab", "abc"],
        "rejected insertions and boundary deletions must not emit duplicate changes"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        value, "abc",
        "the owned InputState must stop at the configured maximum length"
    );
}

#[gpui::test]
fn password_input_copy_and_cut_never_reach_the_clipboard(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Input::new(state_for_view.clone())
            .input_type(InputType::Password)
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("hunter2");
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_owned()));
    press(cx, "ctrl-a");
    press(cx, "ctrl-c");
    press(cx, "cmd-c");
    press(cx, "ctrl-x");
    press(cx, "cmd-x");

    let clipboard = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(
        clipboard.as_deref(),
        Some("sentinel"),
        "copy and cut on a masked field must leave the clipboard untouched"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        value, "hunter2",
        "cut on a masked field must not delete the value"
    );
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("hunter2"),
        "cut on a masked field must not emit a change"
    );
}

#[gpui::test]
fn plain_input_copy_and_cut_still_use_the_clipboard(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Input::new(state_for_view.clone()).into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("hello");
    press(cx, "ctrl-a");
    press(cx, "ctrl-c");
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(
        copied.as_deref(),
        Some("hello"),
        "copy must write the selection"
    );

    cx.write_to_clipboard(gpui::ClipboardItem::new_string(String::new()));
    press(cx, "ctrl-a");
    press(cx, "ctrl-x");
    let cut = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(
        cut.as_deref(),
        Some("hello"),
        "cut must write the selection"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "", "cut must delete the selection");
}

// ---------------------------------------------------------------------------
// TextArea
// ---------------------------------------------------------------------------

#[gpui::test]
fn text_area_typing_and_newline(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        TextArea::new(state_for_view.clone())
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .into_any_element()
    });

    // The field is 76px tall (rows 3), but an EMPTY multi-line field hugs its
    // content: the row has no `whitespace_nowrap` to force a width, so the
    // field is only ~32px wide until text wraps inside it. Measured by
    // probing: clicks at x = 40 already miss it, x = 20 focus it. A click at
    // (10, 40) is safely inside the 76px box. The field is empty, so there is
    // no paragraph to land in and the click only focuses; see
    // `text_area_click_places_the_caret_inside_a_wrapped_line` for the case
    // where it does move the caret.
    click(cx, 10., 40.);
    cx.simulate_input("ab");
    // `WhiteSpace::Normal` wraps by default (AGENTS.md), so the only way a
    // newline gets in is Enter — which v3's TextArea takes as a newline, not
    // a submit.
    press(cx, "enter");
    cx.simulate_input("cd");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a", "ab", "ab\n", "ab\nc", "ab\ncd"],
        "Enter must insert a newline into the value, and typing must continue \
         after it"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        value, "ab\ncd",
        "the InputState must hold the multi-line value"
    );
}

#[gpui::test]
fn text_area_max_length_rejects_a_newline_without_change(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        TextArea::new(state_for_view.clone())
            .max_length(2)
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .into_any_element()
    });

    click(cx, 10., 40.);
    cx.simulate_input("ab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a", "ab"],
        "a rejected newline must not report an unchanged TextArea value"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "ab");
}

#[gpui::test]
fn text_area_value_builder_seeds_the_bound_state(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        TextArea::new(&state_for_view)
            .value("seeded")
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        seeded, "seeded",
        "TextArea::value must forward to the inner field's seed, without &mut App"
    );
}

// ---------------------------------------------------------------------------
// `value` — the pure builder seed
// ---------------------------------------------------------------------------

#[gpui::test]
fn input_value_builder_seeds_the_bound_state_at_first_render(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    // The borrowed handle is the point: a caller keeping its own handle clones
    // nothing at the call site, and the builder chain needs no `&mut App`.
    let cx = open_host(cx, move || {
        Input::new(&state_for_view)
            .value("Ada Lovelace")
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        seeded, "Ada Lovelace",
        "value must seed the bound InputState when the field first renders"
    );
}

#[gpui::test]
fn input_value_builder_keeps_the_last_call_and_outranks_default_value(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Input::new(&state_for_view)
            .default_value("uncontrolled")
            .value("first")
            .value("controlled")
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        seeded, "controlled",
        "a pure builder keeps the last call, and the controlled spelling outranks defaultValue"
    );
}

#[gpui::test]
fn input_value_builder_seeds_once_and_never_clobbers_edits(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Input::new(&state_for_view)
            .value("seed")
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(seeded, "seed");

    // The builder runs again on every refresh carrying the same `value`; the
    // seed must not rewrite the state the user has been editing. Tab seats the
    // caret where the seed left it — at the end — so the keystroke appends.
    press(cx, "tab");
    cx.simulate_input("!");
    flush_frame(cx);
    let typed = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        typed, "seed!",
        "a re-render must not re-apply the value seed over the user's edit"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["seed!"],
        "the seeded-then-edited value reports exactly once, from the keystroke"
    );
}

#[gpui::test]
fn text_field_value_builder_seeds_the_bound_state(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        TextField::new(&state_for_view)
            .value("labelled seed")
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        seeded, "labelled seed",
        "TextField::value must forward to the inner Input's seed"
    );
}

// ---------------------------------------------------------------------------
// SearchField
// ---------------------------------------------------------------------------

#[gpui::test]
fn search_field_clear_button_press_scales_centered_to_pinned_value(cx: &mut TestAppContext) {
    let clears = events();
    let state = cx.new(|cx| InputState::with_value(cx, "rust"));
    let state_for_view = state.clone();
    let entity_id = state.entity_id().as_u64();
    let recorded_for_view = clears.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded_for_view.clone();
        SearchField::new(state_for_view.clone())
            .on_clear(move |_, _| recorded.borrow_mut().push("clear".to_owned()))
            .into_any_element()
    });

    flush_frame(cx);
    let at_rest = clear_bounds(cx, entity_id);
    assert!(
        near(at_rest.size.width, SEARCH_CLEAR_BOX) && near(at_rest.size.height, SEARCH_CLEAR_BOX),
        "the resting SearchField clear slot must be a 20px square, got {at_rest:?}"
    );

    let at = bounds_centre(at_rest);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    flush_frame(cx);
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);

    let pressed = clear_bounds(cx, entity_id);
    let scaled = SEARCH_CLEAR_BOX * SEARCH_CLEAR_PRESS_SCALE;
    let inset = (SEARCH_CLEAR_BOX - scaled) / 2.;
    assert!(
        near(pressed.size.width, scaled) && near(pressed.size.height, scaled),
        "a pressed SearchField clear button must scale to 0.93 ({scaled}px), got {pressed:?}"
    );
    assert!(
        near(pressed.origin.x, f32::from(at_rest.origin.x) + inset)
            && near(pressed.origin.y, f32::from(at_rest.origin.y) + inset),
        "the clear scale must stay centered in its original slot, got {pressed:?}"
    );

    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        clears.borrow().as_slice(),
        ["clear"],
        "the completed pressed clear must still activate"
    );
}

#[gpui::test]
fn search_field_clear_button_press_scale_is_instant_with_reduced_motion(cx: &mut TestAppContext) {
    harness::still();
    let state = cx.new(|cx| InputState::with_value(cx, "rust"));
    let state_for_view = state.clone();
    let entity_id = state.entity_id().as_u64();
    let cx = open_host(cx, move || {
        SearchField::new(state_for_view.clone()).into_any_element()
    });

    flush_frame(cx);
    let at_rest = clear_bounds(cx, entity_id);
    let at = bounds_centre(at_rest);
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);

    let pressed = clear_bounds(cx, entity_id);
    let scaled = SEARCH_CLEAR_BOX * SEARCH_CLEAR_PRESS_SCALE;
    let inset = (SEARCH_CLEAR_BOX - scaled) / 2.;
    assert!(
        near(pressed.size.width, scaled) && near(pressed.size.height, scaled),
        "reduced motion must apply the clear scale immediately ({scaled}px), got {pressed:?}"
    );
    assert!(
        near(pressed.origin.x, f32::from(at_rest.origin.x) + inset)
            && near(pressed.origin.y, f32::from(at_rest.origin.y) + inset),
        "reduced-motion clear scale must stay centered, got {pressed:?}"
    );

    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
}

#[gpui::test]
fn search_field_clear_button_is_inert_when_disabled_read_only_or_empty(cx: &mut TestAppContext) {
    let clears = events();
    let disabled = cx.new(|cx| InputState::with_value(cx, "disabled"));
    let read_only = cx.new(|cx| InputState::with_value(cx, "read-only"));
    let empty = cx.new(|cx| InputState::new(cx));
    let disabled_id = disabled.entity_id().as_u64();
    let read_only_id = read_only.entity_id().as_u64();
    let empty_id = empty.entity_id().as_u64();
    let clears_for_view = clears.clone();
    let cx = open_host(cx, move || {
        let disabled_clears = clears_for_view.clone();
        let read_only_clears = clears_for_view.clone();
        let empty_clears = clears_for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                SearchField::new(disabled.clone())
                    .is_disabled(true)
                    .on_clear(move |_, _| disabled_clears.borrow_mut().push("disabled".to_owned())),
            )
            .child(
                SearchField::new(read_only.clone())
                    .is_read_only(true)
                    .on_clear(move |_, _| {
                        read_only_clears.borrow_mut().push("read-only".to_owned());
                    }),
            )
            .child(
                SearchField::new(empty.clone())
                    .on_clear(move |_, _| empty_clears.borrow_mut().push("empty".to_owned())),
            )
            .into_any_element()
    });

    flush_frame(cx);
    for entity_id in [disabled_id, read_only_id, empty_id] {
        assert!(
            cx.debug_bounds(clear_probe(entity_id)).is_none(),
            "disabled, read-only, and empty SearchFields must not paint a clear button"
        );
    }

    for y in [18., 58., 98.] {
        let at = point(px(298.), px(y));
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        flush_frame(cx);
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        flush_frame(cx);
    }
    assert!(
        clears.borrow().is_empty(),
        "disabled, read-only, and invisible clear controls must never activate"
    );
}

#[gpui::test]
fn search_field_clear_and_submit(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let clears = events();
    let cleared = clears.clone();
    let submits = events();
    let submitted = submits.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let clears = clears.clone();
        let submits = submits.clone();
        SearchField::new(state_for_view.clone())
            .on_change(move |text, _, _| changes.borrow_mut().push(format!("change:{text}")))
            .on_clear(move |_, _| clears.borrow_mut().push("clear".to_owned()))
            .on_submit(move |text, _, _| submits.borrow_mut().push(text.to_owned()))
            .into_any_element()
    });

    // The magnifier occupies x 12..28, so the text area starts at 36; a click
    // at (60, 18) focuses the field with the empty value's caret at 0.
    click(cx, 60., 18.);
    cx.simulate_input("rust");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["change:r", "change:ru", "change:rus", "change:rust"],
        "typing must report the growing value"
    );

    // The clear button is the field's 20px end box, x 288..308 — v3's
    // `SearchField.ClearButton` — and it must empty both the state and the
    // report, notifying `on_clear` along the way.
    click(cx, 298., 18.);
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("change:"),
        "the clear button must report an empty value through on_change"
    );
    assert_eq!(
        cleared.borrow().as_slice(),
        ["clear"],
        "clearing must notify on_clear"
    );
    let empty = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(empty, "", "clearing must empty the InputState");

    let field_focus = cx.update(|_, cx| state.read(cx).focus_handle(cx));
    assert!(
        cx.update(|window, cx| {
            window
                .focused(cx)
                .is_some_and(|focused| focused == field_focus)
        }),
        "pinned `preventFocusOnPress: true`: a pointer clear must restore \
         field focus"
    );

    // The field keeps the focus through the clear click, so typing continues
    // and Enter submits the value.
    cx.simulate_input("abc");
    press(cx, "enter");
    assert_eq!(
        submitted.borrow().as_slice(),
        ["abc"],
        "Enter on the single-line search field must report on_submit with the \
         value"
    );
    let after = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(after, "abc", "the InputState must hold the typed value");
}

/// The pinned `useSearchField` hands the clear button `excludeFromTabOrder:
/// true` and `preventFocusOnPress: true`. The InputState-owned handle is not
/// a tab stop: Tab seats the input and wraps past the button. Enter/Space
/// activate only the *painted* button, via that same handle — a keyed probe
/// lives on a different element-id path and cannot prove this.
#[gpui::test]
fn search_field_clear_button_is_excluded_from_tab_order(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let clears = events();
    let cleared = clears.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "rust"));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let clears = clears.clone();
        SearchField::new(state_for_view.clone())
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .on_clear(move |_, _| clears.borrow_mut().push("clear".to_owned()))
            .into_any_element()
    });
    let field_focus = cx.update(|_, cx| state.read(cx).focus_handle(cx));
    let clear_handle = cx.update(|_, cx| state.read(cx).clear_focus_handle());

    // Tab seats the input. The clear button is not the next stop, so the next
    // Tab wraps straight back to the field without ever focusing the button.
    press(cx, "tab");
    assert!(
        cx.update(|window, cx| {
            window
                .focused(cx)
                .is_some_and(|focused| focused == field_focus)
        }),
        "Tab must seat the input"
    );
    for _ in 0..2 {
        press(cx, "tab");
        assert!(
            cx.update(|window, cx| {
                window
                    .focused(cx)
                    .is_some_and(|focused| focused == field_focus)
            }),
            "the clear button must stay out of the tab order (pinned \
             `excludeFromTabOrder: true`); Tab must wrap back to the field"
        );
    }

    // The InputState-owned handle is the one `track_focus` paints on the
    // button. Focusing it, then flushing so paint sees `is_focused`, is the
    // real GPUI Enter/Space click path.
    cx.update(|window, cx| window.focus(&clear_handle, cx));
    flush_frame(cx);
    assert!(
        cx.update(|window, _| clear_handle.is_focused(window)),
        "window.focus on the InputState handle must seat the painted clear \
         button"
    );
    press(cx, "enter");
    assert!(
        cx.update(|window, cx| {
            window
                .focused(cx)
                .is_some_and(|focused| focused == field_focus)
        }),
        "clear activation must return focus to the input"
    );

    state.update(cx, |state, cx| {
        state.set_value("rust");
        cx.notify();
    });
    flush_frame(cx);
    cx.update(|window, cx| window.focus(&clear_handle, cx));
    flush_frame(cx);
    assert!(
        cx.update(|window, _| clear_handle.is_focused(window)),
        "window.focus on the InputState handle must seat the painted clear \
         button"
    );
    press(cx, "space");
    assert!(
        cx.update(|window, cx| {
            window
                .focused(cx)
                .is_some_and(|focused| focused == field_focus)
        }),
        "clear activation must return focus to the input"
    );

    assert_eq!(
        recorded.borrow().as_slice(),
        ["", ""],
        "keyboard activation must report the clear through on_change"
    );
    assert_eq!(
        cleared.borrow().as_slice(),
        ["clear", "clear"],
        "Enter and Space on the focused clear button must report on_clear"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "", "keyboard activation must empty the InputState");
}

#[gpui::test]
fn search_field_custom_clear_icon_preserves_clear_behavior(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let clears = events();
    let cleared = clears.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let clears = clears.clone();
        SearchField::new(state_for_view.clone())
            .clear_icon(gpui::div().size(px(12.)).child("!"))
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .on_clear(move |_, _| clears.borrow_mut().push("clear".to_owned()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("rust");
    click(cx, 298., 18.);
    assert_eq!(recorded.borrow().last().map(String::as_str), Some(""));
    assert_eq!(cleared.borrow().as_slice(), ["clear"]);
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "");
}

// ---------------------------------------------------------------------------
// InputOTP
// ---------------------------------------------------------------------------

#[gpui::test]
fn input_otp_fills_and_advances(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let completes = events();
    let completed = completes.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let completes = completes.clone();
        InputOTP::new(state_for_view.clone())
            .on_change(move |code, _, _| changes.borrow_mut().push(code.to_owned()))
            .on_complete(move |code, _, _| completes.borrow_mut().push(code.to_owned()))
            .into_any_element()
    });

    // Cell 0 spans x 0..38, y 0..40 (input_otp.rs): clicking its centre (19,
    // 20) focuses the row, and each digit fills the next slot and reports the
    // assembled code.
    click(cx, 19., 20.);
    press(cx, "1");
    press(cx, "2");
    press(cx, "3");
    press(cx, "4");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1", "12", "123", "1234"],
        "typing digits must fill successive slots and report the growing code"
    );
    assert_eq!(
        completed.borrow().as_slice(),
        ["1234"],
        "filling the last slot must fire on_complete with the full code"
    );

    // Backspace clears the current slot first; a second press walks back a
    // slot and clears it (input_otp.rs: "if the cell holds a char, clear it;
    // else step back and clear").
    press(cx, "backspace");
    press(cx, "backspace");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1", "12", "123", "1234", "123", "12"],
        "Backspace must walk back through the filled slots"
    );
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        code, "12",
        "the OtpState must hold the remaining digits only"
    );
}

#[gpui::test]
fn input_otp_value_builder_seeds_the_bound_state_at_first_render(cx: &mut TestAppContext) {
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    // The builder chain needs no `&mut App`: the code travels on the struct
    // and lands in the state when the field first renders.
    let cx = open_host(cx, move || {
        InputOTP::new(state_for_view.clone())
            .value("12")
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        seeded, "12",
        "value must seed the bound OtpState when the field first renders"
    );
}

#[gpui::test]
fn input_otp_value_builder_keeps_the_last_call(cx: &mut TestAppContext) {
    // InputOTP has no `defaultValue` to outrank; the builder contract that
    // remains is the last call winning.
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        InputOTP::new(state_for_view.clone())
            .value("1")
            .value("23")
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        seeded, "23",
        "a pure builder keeps the last call, without &mut App"
    );
}

#[gpui::test]
fn input_otp_value_builder_seeds_once_and_never_clobbers_edits(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        InputOTP::new(state_for_view.clone())
            .value("12")
            .on_change(move |code, _, _| changes.borrow_mut().push(code.to_owned()))
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(seeded, "12");

    // The builder runs again on every refresh carrying the same `value`; the
    // seed must not rewrite the code the user has been editing. Seeding left
    // the cursor after "12" — slot 2 spans x 92..130, so its centre (111, 20)
    // seats the next digit there.
    click(cx, 111., 20.);
    press(cx, "9");
    flush_frame(cx);
    let typed = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        typed, "129",
        "a re-render must not re-apply the value seed over the user's edit"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["129"],
        "the seeded-then-edited code reports exactly once, from the keystroke"
    );
}

#[gpui::test]
fn input_otp_rejects_wrong_length_and_non_digits(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let completes = events();
    let completed = completes.clone();
    let state = cx.new(|cx| OtpState::with_length(cx, 4));
    let state_for_view = state.clone();
    let alpha_changes = events();
    let alpha_recorded = alpha_changes.clone();
    let alpha_state = cx.new(|cx| OtpState::with_length(cx, 4));
    let alpha_for_view = alpha_state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let completes = completes.clone();
        let alpha_changes = alpha_changes.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                InputOTP::new(state_for_view.clone())
                    .on_change(move |code, _, _| changes.borrow_mut().push(code.to_owned()))
                    .on_complete(move |code, _, _| completes.borrow_mut().push(code.to_owned())),
            )
            // v3's `pattern` prop overrides the digit-only default; this port
            // spells it `OtpPattern::Alphanumeric`, matching the exported
            // `REGEXP_ONLY_DIGITS_AND_CHARS`.
            .child(
                InputOTP::new(alpha_for_view.clone())
                    .pattern(OtpPattern::Alphanumeric)
                    .on_change(move |code, _, _| alpha_changes.borrow_mut().push(code.to_owned())),
            )
            .into_any_element()
    });

    // Length is structural: a 4-slot field cannot hold 5 digits. The fifth
    // keystroke overwrites the last slot (the cursor never leaves it once the
    // code is full), so the code stays 4 chars instead of growing to 5.
    click(cx, 19., 20.);
    press(cx, "1");
    press(cx, "2");
    press(cx, "3");
    press(cx, "4");
    press(cx, "5");
    let code = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        code, "1235",
        "a fifth digit must overwrite the last slot: the code cannot grow past \
         the slot count"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1", "12", "123", "1234", "1235"],
        "every accepted keystroke must be reported"
    );
    assert_eq!(
        completed.borrow().as_slice(),
        ["1234", "1235"],
        "an overwrite that leaves the code full must fire on_complete again"
    );

    // `OtpPattern::Digits` is the default, and v3's default is digits-only
    // (`inputMode: 'numeric'`; a `pattern` prop opts into more). A letter
    // must be refused: no report, no state change, and the cursor must not
    // swallow it so the next digit still lands.
    let before = recorded.borrow().len();
    press(cx, "a");
    assert_eq!(
        recorded.borrow().len(),
        before,
        "a letter in the default digit mode must be refused silently"
    );
    let after_letter = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(after_letter, "1235", "a refused letter must change nothing");
    press(cx, "6");
    let after_digit = cx.update(|_, cx| state.read(cx).code());
    assert_eq!(
        after_digit, "1236",
        "a digit typed after a refused letter must still land"
    );

    // With the alphanumeric pattern, letters are accepted — this instance
    // sits at y 56..96 (40px OTP + 16px gap), cell 0 centre (19, 76).
    click(cx, 19., 76.);
    press(cx, "a");
    press(cx, "1");
    assert_eq!(
        alpha_recorded.borrow().as_slice(),
        ["a", "a1"],
        "the Alphanumeric pattern must accept letters and then digits"
    );
    let alpha_code = cx.update(|_, cx| alpha_state.read(cx).code());
    assert_eq!(
        alpha_code, "a1",
        "the OtpState must hold the alphanumeric code"
    );
}

// ---------------------------------------------------------------------------
// InputGroup
// ---------------------------------------------------------------------------

#[gpui::test]
fn input_group_prefix_and_suffix_do_not_steal_the_caret(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        // A `w(px(400.))` wrapper pins the group's edges so the addons, which
        // carry the group's padding (`px(12)`, `InputAddon::render`), sit at
        // known coordinates: prefix 0..~32, suffix ~351..400.
        gpui::div()
            .w(px(400.))
            .child(
                InputGroup::new()
                    .prefix(InputAddon::new("$"))
                    .input(
                        Input::new(state_for_view.clone()).on_change(move |text, _, _| {
                            changes.borrow_mut().push(text.to_owned());
                        }),
                    )
                    .suffix(InputAddon::new(".com")),
            )
            .into_any_element()
    });

    // Click the field (flex_1 between the addons), which focuses it and puts
    // the caret at 0 of the empty value.
    click(cx, 200., 18.);
    cx.simulate_input("ab");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a", "ab"],
        "typing into the group's field must work"
    );

    // v3.2.4's `InputGroupRoot.handleClick` focuses the contained input when
    // a click lands on the group outside it, so the addon clicks below must
    // leave the field focused and typing must keep reaching it. Focusing
    // never moves the caret — it is `InputState`'s, written only by the
    // field's own mouse-down — and Left put it at 1, so the text must grow
    // mid-string: after the prefix click "cd" turns "ab" into "acdb" ("a|b"
    // -> "a|cdb" -> "acd|b"), not "abcd" (caret dropped to the end) nor
    // "cdab" (caret dropped to 0); the cursor now rests at 3, so the suffix
    // click must not move it and "x" lands between d and b: "acdxb".
    press(cx, "left");
    click(cx, 6., 18.);
    cx.simulate_input("cd");
    click(cx, 394., 18.);
    cx.simulate_input("x");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a", "ab", "acb", "acdb", "acdxb"],
        "an addon click must focus the group's field and leave the caret \
         exactly where it was: typing continues mid-string"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        value, "acdxb",
        "the InputState must hold the text typed through the addon clicks"
    );
}

/// v3's `.input-group__prefix`/`__suffix` slots carry `px-3` themselves, so a
/// bare icon child insets exactly like `InputAddon` text — which keeps its
/// old position now that the padding lives on the slot.
#[gpui::test]
fn input_group_slot_padding_insets_icons_and_keeps_addon_text(cx: &mut TestAppContext) {
    let icon_state = cx.new(|cx| InputState::new(cx));
    let text_state = cx.new(|cx| InputState::new(cx));
    let icon_id = icon_state.entity_id().as_u64();
    let text_id = text_state.entity_id().as_u64();
    let icon_for_view = icon_state;
    let text_for_view = text_state;
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .items_start()
            .w(px(400.))
            .child(
                InputGroup::new()
                    .prefix(
                        gpui::div()
                            .debug_selector(|| "icon-addon".to_owned())
                            .child("I"),
                    )
                    .input(Input::new(icon_for_view.clone())),
            )
            .child(
                InputGroup::new()
                    .prefix(
                        gpui::div()
                            .debug_selector(|| "text-addon".to_owned())
                            .child(InputAddon::new("$")),
                    )
                    .input(Input::new(text_for_view.clone())),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let slot_probe = |entity: u64| -> &'static str {
        Box::leak(format!("input-group-{entity}-prefix").into_boxed_str())
    };
    for (slot, probe) in [
        (slot_probe(icon_id), "icon-addon"),
        (slot_probe(text_id), "text-addon"),
    ] {
        let slot_bounds = cx.debug_bounds(slot).unwrap();
        let probe_bounds = cx.debug_bounds(probe).unwrap();
        let inset = probe_bounds.origin.x - slot_bounds.origin.x;
        assert!(
            (inset - px(12.)).abs() < px(0.5),
            "{probe} must sit 12px inside its slot, got {inset:?}"
        );
    }
}

#[gpui::test]
fn input_group_disabled_propagates_to_the_field(cx: &mut TestAppContext) {
    // v3 puts `isDisabled` on the TextField around the group, and the browser
    // then holds a *disabled* `<input>`: it takes no caret, keeps no focus and
    // answers no key. The group's own flag must reach the field it contains —
    // dimming alone would leave a control that looks off and still types.
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        gpui::div()
            .w(px(400.))
            .child(
                InputGroup::new()
                    .is_disabled(true)
                    .prefix(InputAddon::new("$"))
                    .input(
                        Input::new(state_for_view.clone()).on_change(move |text, _, _| {
                            changes.borrow_mut().push(text.to_owned());
                        }),
                    )
                    .suffix(InputAddon::new(".com")),
            )
            .into_any_element()
    });
    flush_frame(cx);

    // The field spans between the addons (x ~32..346, y 0..36): a direct
    // click must not focus it, and neither keys typed after the click nor
    // keys typed blind may reach the disabled state.
    click(cx, 200., 18.);
    cx.simulate_input("x");
    press(cx, "a");
    // v3.2.4's `InputGroupRoot.handleClick` runs `input.focus()` on a click
    // outside the field — a browser no-ops that on a disabled input, so the
    // addon click must leave nothing to type into either.
    click(cx, 6., 18.);
    cx.simulate_input("y");
    assert!(
        recorded.borrow().is_empty(),
        "a group-disabled field must record no typing at all, got {:?}",
        recorded.borrow()
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        value, "",
        "a group-disabled field must hold no text: is_disabled propagates to \
         the inner Input"
    );
}

#[gpui::test]
fn input_group_textarea_click_exception_and_direct_focus(cx: &mut TestAppContext) {
    // Pinned v3.2.4 `InputGroupRoot.handleClick` looks the field up with
    // `querySelector("input")` — a `<textarea>` is not an `input`, so a group
    // holding only a TextArea gets NO click-to-focus: the addon click below
    // must leave the textarea unfocused, while a click inside the textarea
    // itself still focuses it.
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        gpui::div()
            .w(px(400.))
            .child(
                InputGroup::new().prefix(InputAddon::new("Note")).text_area(
                    TextArea::new(state_for_view.clone())
                        .rows(3)
                        .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned())),
                ),
            )
            .into_any_element()
    });
    flush_frame(cx);

    // The addon slot is top-aligned with the pinned 8px top padding, so the
    // addon's box spans y 8..28 inside the prefix's x 0..~40: a click there
    // is a click on the group *outside* the textarea, which v3.2.4 ignores
    // for a textarea-only group.
    click(cx, 6., 13.);
    cx.simulate_input("x");
    assert!(
        recorded.borrow().is_empty(),
        "a click on a textarea-only group's addon must not focus the textarea \
         (querySelector(\"input\") finds nothing), got {:?}",
        recorded.borrow()
    );

    // Positive control: the textarea itself is focusable — a click inside it
    // (76px tall for rows 3, wide in the 400px group) must focus it, so the
    // empty recording above is the pinned exception and not a broken field.
    click(cx, 200., 38.);
    cx.simulate_input("ab");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a", "ab"],
        "a click inside the textarea must focus it: the exception only covers \
         clicks outside the field"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "ab", "the InputState must hold the typed text");
}

#[gpui::test]
fn input_group_button_suffix_presses_then_hands_focus_to_the_field(cx: &mut TestAppContext) {
    // v3.2.4 `handleClick` focuses the contained input on any click outside
    // it — a focusable suffix button included. The button keeps its own press
    // (gpui decides clicks by hover, not focus, so the focus hand-over on
    // mouse-down does not cancel the mouse-up click) and the field ends up
    // holding the focus, so the next keystrokes type into it.
    let changes = events();
    let recorded = changes.clone();
    let presses = events();
    let pressed = presses.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let presses = presses.clone();
        gpui::div()
            .w(px(400.))
            .child(
                InputGroup::new()
                    .prefix(InputAddon::new("$"))
                    .input(
                        Input::new(state_for_view.clone()).on_change(move |text, _, _| {
                            changes.borrow_mut().push(text.to_owned());
                        }),
                    )
                    .suffix(
                        Button::new("ig-go")
                            .label("Go")
                            .on_press(move |_, _, _| presses.borrow_mut().push("press".into())),
                    ),
            )
            .into_any_element()
    });
    flush_frame(cx);

    // The suffix slot's own `px-3` insets the Button ("Go", md padding) 12px
    // off the row's right edge in the 400px wrapper: the click lands in its
    // right padding at (384, 18) — on the group, outside the field between
    // the addons.
    click(cx, 384., 18.);
    assert_eq!(
        pressed.borrow().as_slice(),
        ["press"],
        "the focusable suffix must keep its own action"
    );
    let field_handle = cx.update(|_, cx| state.read(cx).focus_handle(cx));
    let focused_on_field =
        cx.update(|window, cx| window.focused(cx).is_some_and(|held| held == field_handle));
    assert!(
        focused_on_field,
        "the click-to-focus must take the focus back from the pressed \
         suffix button and give it to the field"
    );
    cx.simulate_input("x");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["x"],
        "typing after the suffix click must reach the field"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        value, "x",
        "the InputState must hold the text typed through the suffix button"
    );
}

#[gpui::test]
fn input_group_field_disabled_refuses_addon_focus_from_the_first_frame(cx: &mut TestAppContext) {
    // A field disabled while the group stays enabled must refuse the addon
    // click-to-focus from the FIRST frame. The group used to read the
    // disabled state through the mirror `Input::render` writes into the
    // shared state — a trace of the last rendered frame, and untouched
    // before the first one — so the very first addon click ran
    // `input.focus()` on a disabled field, which a browser refuses.
    //
    // The focusable suffix is the witness. A press on it must leave the
    // BUTTON holding the focus (its own transfer ran deeper, and its handle
    // lives in the focus tree, so the app root does not reclaim it): Enter
    // then activates it a second time. A wrongly focused disabled field,
    // by contrast, is not in the focus tree, so the app root takes the
    // focus back on the next paint and Enter activates nothing.
    let changes = events();
    let recorded = changes.clone();
    let presses = events();
    let pressed = presses.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let presses = presses.clone();
        gpui::div()
            .w(px(400.))
            .child(
                InputGroup::new()
                    .prefix(InputAddon::new("$"))
                    .input(
                        Input::new(state_for_view.clone())
                            .is_disabled(true)
                            .on_change(move |text, _, _| {
                                changes.borrow_mut().push(text.to_owned());
                            }),
                    )
                    .suffix(
                        Button::new("ig-disabled-go")
                            .label("Go")
                            .on_press(move |_, _, _| presses.borrow_mut().push("press".into())),
                    ),
            )
            .into_any_element()
    });
    let field_handle = cx.update(|_, cx| state.read(cx).focus_handle(cx));

    // The click below runs against the frame `open_host` already painted —
    // the FIRST frame, whose listener decision the mirror read made before
    // any `Input::render` had ever run. It is deliberately the first
    // interaction: a click repaints, and every later frame reads a mirror
    // the previous frame already corrected.
    //
    // The suffix button keeps its press, and the focus must stay with the
    // button — proven by Enter activating it again, which only works while
    // the button holds the focus. A wrongly focused disabled field is not
    // in the focus tree, so the app root takes the focus back on the next
    // paint and Enter activates nothing.
    click(cx, 384., 18.);
    assert_eq!(
        pressed.borrow().as_slice(),
        ["press"],
        "the suffix button must keep its own press beside a disabled field"
    );
    let focused_on_field = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| window.focused(cx).is_some_and(|held| held == field_handle))
    };
    assert!(
        !focused_on_field(cx),
        "a field disabled inside an enabled group must refuse the addon \
         click-to-focus on the first frame"
    );
    flush_frame(cx);
    press(cx, "enter");
    assert_eq!(
        pressed.borrow().as_slice(),
        ["press", "press"],
        "the button must still hold the focus after the click: a disabled \
         field must not have taken it from the first frame on"
    );

    // And the plain addons refuse the click-to-focus on the later frames too.
    click(cx, 6., 18.);
    assert!(
        !focused_on_field(cx),
        "the addon click must never focus a disabled field"
    );
    cx.simulate_input("y");

    cx.simulate_input("z");
    assert!(
        recorded.borrow().is_empty(),
        "no keystroke may reach the disabled field, got {:?}",
        recorded.borrow()
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        value, "",
        "the disabled field must hold no text: the group's click-to-focus \
         reads the field's own flag, not a rendered mirror"
    );
}

#[gpui::test]
fn input_group_disabled_group_propagates_to_the_textarea(cx: &mut TestAppContext) {
    // The group's flag reaches the converted multi-line field the same way —
    // `text_area` becomes the one held `Input` slot — so a click inside the
    // box focuses nothing and no key reaches the state.
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        gpui::div()
            .w(px(400.))
            .child(
                InputGroup::new().is_disabled(true).text_area(
                    TextArea::new(state_for_view.clone())
                        .rows(3)
                        .on_change(move |text, _, _| {
                            changes.borrow_mut().push(text.to_owned());
                        }),
                ),
            )
            .into_any_element()
    });
    flush_frame(cx);

    // The textarea spans the full row (no addons), 76px tall for rows 3.
    click(cx, 200., 38.);
    cx.simulate_input("x");
    let field_handle = cx.update(|_, cx| state.read(cx).focus_handle(cx));
    let focused_on_field =
        cx.update(|window, cx| window.focused(cx).is_some_and(|held| held == field_handle));
    assert!(
        !focused_on_field,
        "a group-disabled textarea must not take the focus"
    );
    assert!(
        recorded.borrow().is_empty(),
        "a group-disabled textarea must record no typing, got {:?}",
        recorded.borrow()
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        value, "",
        "the propagation must reach the converted multi-line field"
    );
}

#[gpui::test]
// The geometry assertions below compare whole-pixel layout results — a 400px
// request laid out by taffy is exactly 400.0, with no fractional step to
// tolerate — so the exact comparisons are on purpose.
#[allow(clippy::float_cmp)]
fn input_group_full_width_stretches_the_outer_wrapper(cx: &mut TestAppContext) {
    // `.input-group--full-width` is `w-full`, and in this port that width has
    // to reach the OUTER wrapper too: a `w_full` child resolves against a
    // content-sized parent and stretches nothing. Measured in an
    // items-start column, where an unstretched group hugs its content.
    let changes = events();
    let state = cx.new(|cx| InputState::new(cx));
    let entity = state.entity_id().as_u64();
    let plain = cx.new(|cx| InputState::new(cx));
    let plain_entity = plain.entity_id().as_u64();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        gpui::div()
            .w(px(400.))
            .flex()
            .flex_col()
            .items_start()
            .gap(px(16.))
            .child(
                InputGroup::new()
                    .full_width(true)
                    .prefix(InputAddon::new("$"))
                    .input(Input::new(state.clone()).on_change(move |text, _, _| {
                        changes.borrow_mut().push(text.to_owned());
                    }))
                    .suffix(InputAddon::new("USD")),
            )
            .child(
                InputGroup::new()
                    .prefix(InputAddon::new("$"))
                    .input(Input::new(plain.clone()))
                    .suffix(InputAddon::new("USD")),
            )
            .into_any_element()
    });
    flush_frame(cx);

    // `debug_bounds` takes a `&'static str` and the probe keys carry the
    // input's entity id, so the formatted keys are leaked for the test's
    // lifetime.
    let full = cx
        .debug_bounds(input_group_probe(entity, "group"))
        .expect("the full-width group box must be measurable");
    let plain_bounds = cx
        .debug_bounds(input_group_probe(plain_entity, "group"))
        .expect("the plain group box must be measurable");
    assert_eq!(
        f32::from(full.size.width),
        400.0,
        "full_width must stretch the group to its container's width, got {}",
        f32::from(full.size.width)
    );
    assert!(
        f32::from(plain_bounds.size.width) < 400.0,
        "without full_width the group must hug its content, got {}",
        f32::from(plain_bounds.size.width)
    );
}

#[gpui::test]
// The geometry assertions below compare whole-pixel sums — 20px lines and the
// pinned 8px slot padding laid out by taffy — so the exact comparisons are on
// purpose.
#[allow(clippy::float_cmp)]
fn input_group_textarea_auto_height_and_top_aligned_addons(cx: &mut TestAppContext) {
    // `:has([data-slot="input-group-textarea"])` switches the group to
    // `items-start` with `height: auto`, and gives each addon
    // `padding-top: 0.5rem` — the 8px that used to be faked by hand in the
    // gallery demo. Probed with fixed-height slots: the prefix wrapper must
    // start at the group's top and stand 8px taller than its 20px probe.
    let state = cx.new(|cx| InputState::new(cx));
    let entity = state.entity_id().as_u64();
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(400.))
            .child(
                InputGroup::new()
                    .prefix(
                        // A fixed 20px probe: any glyph metrics stay out of the
                        // arithmetic.
                        gpui::div().w(px(16.)).h(px(20.)),
                    )
                    .suffix(gpui::div().w(px(16.)).h(px(20.)))
                    .text_area(TextArea::new(state.clone()).rows(3)),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let group = cx
        .debug_bounds(input_group_probe(entity, "group"))
        .expect("the textarea group box must be measurable");
    assert_eq!(
        f32::from(group.size.height),
        76.0,
        "rows(3) is 20*3 + 16 = 76px and `height: auto` must let the group \
         grow to it, got {}",
        f32::from(group.size.height)
    );
    let prefix = cx
        .debug_bounds(input_group_probe(entity, "prefix"))
        .expect("the textarea group's addon slot must be measurable");
    assert_eq!(
        f32::from(prefix.origin.y),
        f32::from(group.origin.y),
        "the addon slot must be top-aligned with the textarea (items-start), \
         not centred in the taller group"
    );
    assert_eq!(
        f32::from(prefix.size.height),
        28.0,
        "the addon slot carries the pinned 8px top padding over its 20px \
         probe, got {}",
        f32::from(prefix.size.height)
    );
    let suffix = cx
        .debug_bounds(input_group_probe(entity, "suffix"))
        .expect("the textarea group's suffix slot must be measurable");
    assert_eq!(
        f32::from(suffix.origin.y),
        f32::from(group.origin.y),
        "both addon slots must top-align"
    );
}

// ---------------------------------------------------------------------------
// ColorField
// ---------------------------------------------------------------------------

#[gpui::test]
fn color_field_typing_reports_a_colour(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        ColorField::new("cf-hex", PickerColor::from_hex("#336699").unwrap())
            .state(state_for_view.clone())
            .on_change(move |colour, _, _| {
                changes.borrow_mut().push(match colour {
                    Some(c) => c.to_hex(),
                    None => "none".to_owned(),
                });
            })
            .into_any_element()
    });

    // Editable mode delegates to `Input` with a 16px swatch start content, so
    // the text starts at 12 + 16 + 8 = 36 and (60, 18) focuses the field.
    // The state starts empty — the initial colour is the placeholder — so the
    // typed text IS the value.
    click(cx, 60., 18.);
    cx.simulate_input("ff0000");
    assert_eq!(
        recorded.borrow().len(),
        6,
        "every keystroke must be parsed and reported"
    );
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("#FF0000"),
        "the final report must be the colour the hex text parses to"
    );
    let text = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(text, "ff0000", "the InputState must hold the typed hex");

    // A partial hex is not a colour: one more keystroke past the end must not
    // turn "ff0000" into a different valid value silently — "0" appended is
    // length 7, which `from_hex` rejects, so the report flips to none.
    cx.simulate_input("0");
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("none"),
        "a 7-char hex must not parse; on_change must report None for it"
    );
}

#[gpui::test]
fn color_field_accepts_an_uncontrolled_null_default(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        ColorField::new("cf-null-default", None::<PickerColor>)
            .default_value(None::<PickerColor>)
            .state(state_for_view.clone())
            .placeholder("#000000")
            .on_change(move |colour, _, _| {
                changes.borrow_mut().push(match colour {
                    Some(c) => c.to_hex(),
                    None => "none".to_owned(),
                });
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("00ff00");
    assert_eq!(
        recorded.borrow().last().map(String::as_str),
        Some("#00FF00"),
        "an uncontrolled null field must accept its first complete color"
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "00ff00",
        "the empty field's text input remains editable"
    );
}

// ---------------------------------------------------------------------------
// DateField
// ---------------------------------------------------------------------------

#[gpui::test]
fn date_field_segments_answer_arrows_and_digits(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "2025-01-15"));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        // Start from a complete value so this test isolates segment arrows,
        // digits and bounds. `date_field_picker_deep` separately proves that
        // an empty field fills only its active segment and defers onChange.
        DateField::new(state_for_view.clone())
            .min_value(Date::new(2025, 1, 10))
            .max_value(Date::new(2025, 1, 20))
            .on_change(move |date, _, _| {
                changes.borrow_mut().push(match date {
                    Some(d) => d.format_iso(),
                    None => "none".to_owned(),
                });
            })
            .into_any_element()
    });

    // The field's handle is the page's only tab stop (`InputState` is
    // `tab_stop(true)`), so Tab focuses it with the Month segment focused —
    // no coordinates involved, exactly how fields.rs drives TimeField.
    press(cx, "tab");
    press(cx, "up");
    press(cx, "down");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["none", "2025-01-15"],
        "stepping the month past the bounds must report None, and stepping \
         back must report the in-range date again"
    );

    // Right walks to the Day segment; digits type into it. `1` alone is
    // below `minValue`, so it reports None; the second digit completes `15`,
    // an in-range day.
    press(cx, "right");
    press(cx, "1");
    press(cx, "5");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["none", "2025-01-15", "none", "2025-01-15"],
        "digits must type into the focused day segment and report each step's \
         date, filtering any out-of-range value"
    );

    // A full segment hands the caret on (the Year now), so Left returns to
    // Day; `25` clamps to nothing outside `maxValue` the same way, and the
    // raw segment math still lands in the bound state.
    press(cx, "left");
    press(cx, "2");
    press(cx, "5");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["none", "2025-01-15", "none", "2025-01-15", "none", "none"],
        "an over-`maxValue` day must report None, never the out-of-range date"
    );
    let text = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        text, "2025-01-25",
        "the bound InputState must hold the ISO text the segments produced"
    );
}

// ---------------------------------------------------------------------------
// Fieldset
// ---------------------------------------------------------------------------

#[gpui::test]
fn fieldset_disabled_disables_its_children(cx: &mut TestAppContext) {
    // The claim this test is named for has no mechanism to satisfy — on
    // either side. v3's Fieldset API table lists `className`, `children` and
    // `nativeProps` only: there is no `isDisabled` prop, and the native
    // `<fieldset disabled>` behaviour would have to arrive through
    // `nativeProps`, which gpui has no analogue for (no DOM attribute graph).
    // This port's `Fieldset` is exactly the v3 surface: a layout container
    // (flex column, `gap(24)`) with no disabled state of its own. So the
    // honest assertion is the inverse of the claim: a field composed inside a
    // Fieldset keeps every interaction, because nothing in the container can
    // take it away. There is no `.is_disabled` builder on `Fieldset` to call.
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Fieldset::new()
            .child(FieldsetLegend::new("Billing address"))
            .child(
                FieldGroup::new().child(
                    Input::new(state_for_view.clone())
                        .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned())),
                ),
            )
            .into_any_element()
    });

    // The legend is a 24px line and the Fieldset gaps children by 24, so the
    // group's bare 36px field spans y 48..84 — click (60, 66) to focus it.
    click(cx, 60., 66.);
    cx.simulate_input("abc");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["a", "ab", "abc"],
        "a field inside a Fieldset must keep accepting typing: the container \
         imposes no disabling"
    );
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "abc", "the InputState must hold the typed value");
}

/// A pointer click inside a WRAPPED paragraph places the caret where it landed.
///
/// This was recorded as impossible (`behaviour_audit`'s
/// `no-wrapped-line-metrics`) on the grounds that gpui reports no position for
/// a wrapped line. It does: `shape_text` answers `WrappedLine`, which derefs to
/// the layout that owns `closest_index_for_position`. The caret is asserted
/// through the next keystroke, which is where it actually matters.
#[gpui::test]
fn text_area_click_places_the_caret_inside_a_wrapped_line(cx: &mut TestAppContext) {
    // One long word-wrapped paragraph, no newlines: every visual line break
    // here is one gpui chose, which is exactly the case that was unreachable.
    let state = cx.new(|cx| InputState::with_value(cx, "aaaa bbbb cccc dddd eeee ffff gggg hhhh"));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        // 200px forces the text to wrap into several visual lines inside one
        // paragraph; the TextArea is `rows(3)` = 76px tall at the origin.
        gpui::div()
            .w(px(200.))
            .child(TextArea::new(state_for_view.clone()))
            .into_any_element()
    });

    let end = cx.update(|_, cx| state.read(cx).value().chars().count());

    // Click near the start of the first visual line, well before the end. A
    // plain click leaves no anchor, so the caret is not observable directly --
    // the keystroke below is what reports where it went.
    click(cx, 16., 14.);
    cx.simulate_input("X");
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert!(
        !value.ends_with('X'),
        "typing after a click in a wrapped paragraph must insert at the caret, \
         not append; got {value:?}"
    );
    assert_eq!(
        value.chars().count(),
        end + 1,
        "the click must move the caret, not select and replace"
    );
}

#[gpui::test]
fn field_addons_and_custom_slots_keep_twenty_pixel_lines(cx: &mut TestAppContext) {
    for kind in 0..7 {
        for leading in [None, Some(12.), Some(48.)] {
            let state = cx.new(|cx| InputState::new(cx));
            let otp = cx.new(|cx| OtpState::with_length(cx, 1));
            let time = cx.new(|cx| TimeState::new(cx));
            let number = cx.new(|cx| NumberState::new(cx, 4.));
            let cx = open_host(cx, move || {
                let probe = || {
                    gpui::div()
                        .debug_selector(|| "input-leading-text".into())
                        .child("$")
                };
                let control = match kind {
                    0 => Input::new(state.clone())
                        .start_content(probe())
                        .into_any_element(),
                    1 => InputGroup::new()
                        .prefix(
                            gpui::div()
                                .debug_selector(|| "input-leading-text".into())
                                .child(InputAddon::new("$")),
                        )
                        .input(Input::new(state.clone()))
                        .into_any_element(),
                    2 => InputOTP::new(otp.clone())
                        .slot(move |_, _| probe().into_any_element())
                        .into_any_element(),
                    3 => DateField::new(state.clone())
                        .prefix(probe())
                        .into_any_element(),
                    4 => TimeField::new(time.clone())
                        .prefix(probe())
                        .into_any_element(),
                    _ => NumberField::new(number.clone())
                        .increment_icon(probe())
                        .is_disabled(kind == 6)
                        .into_any_element(),
                };
                gpui::div()
                    .w(px(300.))
                    .when_some(leading, |el, leading| el.line_height(px(leading)))
                    .child(control)
                    .into_any_element()
            });
            assert_eq!(
                cx.debug_bounds("input-leading-text")
                    .expect("slot text paints")
                    .size
                    .height,
                px(20.),
                "kind={kind}, host={leading:?}"
            );
            if kind == 5 {
                let at = bounds_centre(cx.debug_bounds("input-leading-text").unwrap());
                cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
                flush_frame(cx);
                let pressed = cx
                    .debug_bounds("input-leading-text")
                    .expect("pressed slot paints");
                assert_eq!(
                    pressed.size.height,
                    px(20.),
                    "stepper press leaves the inner 20px line alone: {pressed:?}"
                );
                cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
                flush_frame(cx);
                assert_eq!(
                    cx.debug_bounds("input-leading-text").unwrap().size.height,
                    px(20.)
                );
            }
        }
    }
}

#[gpui::test]
fn color_and_time_full_width_reaches_the_parent_edge(cx: &mut TestAppContext) {
    for kind in 0..3 {
        for width in [200., 400., 640.] {
            for labeled in [false, true] {
                let measured = std::rc::Rc::new(std::cell::Cell::new(None));
                let recorded = measured.clone();
                let input = cx.new(|cx| InputState::new(cx));
                let time = cx.new(|cx| TimeState::new(cx));
                let time_for_view = time.clone();
                let input_for_view = input.clone();
                let cx = open_host(cx, move || {
                    let control = if kind == 2 {
                        TimeField::new(time_for_view.clone())
                            .default_value(Time::new(12, 30))
                            .full_width(true)
                            .when(labeled, |field| field.label("Time"))
                            .into_any_element()
                    } else {
                        ColorField::new("width-color", PickerColor::hsb(0., 1., 1.))
                            .full_width(true)
                            .when(kind == 1, |field| field.state(input_for_view.clone()))
                            .when(labeled, |field| field.label("Color"))
                            .into_any_element()
                    };
                    let recorded = recorded.clone();
                    gpui::div()
                        .w(px(width))
                        .flex()
                        .flex_col()
                        .items_start()
                        .on_children_prepainted(move |bounds, _, _| recorded.set(Some(bounds[0])))
                        .child(control)
                        .into_any_element()
                });
                flush_frame(cx);
                assert_eq!(
                    measured.get().expect("field paints").size.width,
                    px(width),
                    "kind={kind}, labeled={labeled}"
                );
                if kind != 0 {
                    click(cx, width - 8., if labeled { 42. } else { 18. });
                    if kind == 1 {
                        cx.update(|window, cx| {
                            assert!(
                                input.focus_handle(cx).is_focused(window),
                                "expanded input edge accepts focus"
                            );
                        });
                    } else {
                        press(cx, "up");
                        cx.update(|_, cx| {
                            assert_ne!(
                                time.read(cx).value,
                                Some(Time::new(12, 30)),
                                "expanded time field edge accepts keyboard editing"
                            );
                        });
                    }
                }
            }
        }
    }
}

/// The `Input` wrapper's label, description, and error slots keep v3.2.4's
/// pinned line boxes even under a hostile inherited leading.
///
/// v3.2.4's `.label` is `text-sm` (14px over a 20px line) and both
/// `.description` and `.error-message` are `text-xs` (12px over a 16px line);
/// `field.rs`'s `Label`/`Description`/`ErrorMessage` already pin 20/16, and
/// the field box itself pins 14/20 (`input.rs`). The wrapper must pin the
/// same three lines, so a labelled field with helper copy is
/// 20 (label) + 4 + 36 (field) + 4 + 16 (description/error) = 80px at any
/// host leading. The error case also carries a description: the error
/// replaces it (still 80px, never 100px), and the required star rides in the
/// 20px label row without growing it.
#[gpui::test]
fn input_label_description_and_error_keep_pinned_line_heights(cx: &mut TestAppContext) {
    for leading in [None, Some(12.), Some(48.)] {
        let desc_state = cx.new(|cx| InputState::new(cx));
        let err_state = cx.new(|cx| InputState::new(cx));
        let cx = open_host(cx, move || {
            let mut root = gpui::div().flex().flex_col().gap(px(16.)).items_start();
            if let Some(leading) = leading {
                root = root.text_size(px(32.)).line_height(px(leading));
            }
            root.child(
                gpui::div()
                    .debug_selector(|| "input-desc".to_owned())
                    .child(
                        Input::new(desc_state.clone())
                            .label("Name")
                            .description("Help"),
                    ),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "input-error".to_owned())
                    .child(
                        Input::new(err_state.clone())
                            .label("Name")
                            .is_required(true)
                            .description("Help")
                            .validation_errors(["Oops"]),
                    ),
            )
            .into_any_element()
        });
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("input-desc").unwrap().size.height,
            px(80.),
            "label + description must stay 20 + 4 + 36 + 4 + 16, host={leading:?}"
        );
        assert_eq!(
            cx.debug_bounds("input-error").unwrap().size.height,
            px(80.),
            "required label + error must stay 80px and the error must replace \
             the description rather than stack under it, host={leading:?}"
        );
    }
}

/// v3's `validate` is commit-gated the way React Stately's native behavior
/// gates it: the displayed validity is a *snapshot* taken at commit, not the
/// live result. A focus→blur with no edit commits nothing (the native
/// `change` event needs a changed value); an edit followed by blur commits
/// the snapshot, and typing afterwards leaves the shown error untouched —
/// even when the live value turns valid — until the next commit.
#[gpui::test]
fn native_validation_shows_a_committed_snapshot_not_the_live_value(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let other = cx.new(|cx| InputState::new(cx));
    let validated = state.clone();
    let plain = other.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .items_start()
            .child(
                gpui::div().debug_selector(|| "validated".to_owned()).child(
                    Input::new(validated.clone())
                        .label("Name")
                        .is_required(true)
                        .validate(|value: &str| {
                            (value.len() < 3).then(|| "Name must be at least 3 characters".into())
                        }),
                ),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "plain".to_owned())
                    .child(Input::new(plain.clone()).label("Other")),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let plain_height = cx.debug_bounds("plain").unwrap().size.height;
    let rest_height = cx.debug_bounds("validated").unwrap().size.height;
    assert_eq!(
        rest_height, plain_height,
        "an untouched field failing `validate` must not paint its error at rest"
    );
    let focus = |cx: &mut VisualTestContext, which: &Entity<InputState>| {
        cx.update(|window, cx| {
            let handle = which.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        });
        cx.run_until_parked();
    };
    // Focus then blur with no edit: no `change`, so no commit — still no error.
    focus(cx, &state);
    focus(cx, &other);
    assert_eq!(
        cx.debug_bounds("validated").unwrap().size.height,
        rest_height,
        "a blur with no edit since focus must not commit the validation"
    );
    // Type an invalid value and blur: the change commits the snapshot. The
    // error row animates its height in, so let the motion settle first.
    focus(cx, &state);
    cx.simulate_input("ab");
    focus(cx, &other);
    harness::wait_ms(cx, 800);
    let committed_height = cx.debug_bounds("validated").unwrap().size.height;
    assert!(
        committed_height > rest_height,
        "an edit followed by blur must commit and show the error"
    );
    // Type the value valid *without* blurring: the displayed snapshot must
    // not track the live result — the stale error stays painted.
    focus(cx, &state);
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("name");
    let typed = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert!(
        typed.len() >= 3,
        "the valid edit must have landed for this check to mean anything, got {typed:?}"
    );
    assert_eq!(
        cx.debug_bounds("validated").unwrap().size.height,
        committed_height,
        "typing after a commit must not change the displayed error"
    );
    // The next change-blur commits the now-valid pending result: it clears,
    // and the error row animates its height back out.
    focus(cx, &other);
    harness::wait_ms(cx, 800);
    assert_eq!(
        cx.debug_bounds("validated").unwrap().size.height,
        rest_height,
        "the commit after the valid edit must clear the displayed error"
    );
}

/// `ValidationBehavior::Allow` is React Aria's `aria` behavior: `validate`
/// and built-in results display in real time — no blur, no commit.
#[gpui::test]
fn allow_validation_displays_in_real_time_without_a_commit(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let field = state.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .items_start()
            .child(
                gpui::div().debug_selector(|| "allow".to_owned()).child(
                    Input::new(field.clone())
                        .label("Name")
                        .validation_behavior(ValidationBehavior::Allow)
                        .validate(|value: &str| {
                            (value == "bad").then(|| "That name is taken".into())
                        }),
                ),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let rest_height = cx.debug_bounds("allow").unwrap().size.height;
    // Type while focused and never blur: the error must already paint.
    cx.update(|window, cx| {
        let handle = state.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    cx.run_until_parked();
    cx.simulate_input("bad");
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "bad",
        "the typed value must have landed for this check to mean anything"
    );
    assert!(
        cx.debug_bounds("allow").unwrap().size.height > rest_height,
        "Allow must display the validate error in real time, before any blur"
    );
}

/// The caret probe shared by the placeholder tests: a ~1.5px-wide solid
/// accent bar inside `field`'s bounds. `still()` keeps the blink opaque.
fn caret_quad(scene: &harness::Painted, field: Bounds<Pixels>, accent: gpui::Hsla) -> bool {
    scene.quads.iter().any(|q| {
        q.background
            .as_solid()
            .is_some_and(|c| harness::same_color(c, accent))
            && {
                let b = scene.bounds(q);
                f32::from(b.size.width) <= 2.
                    && f32::from(b.size.height) > 10.
                    && harness::contains(field, b)
            }
    })
}

/// HTML keeps `placeholder` painted while the empty field is focused — the
/// caret blinks at the start and the first typed character swaps the hint
/// for the value. The focused placeholder must sit at the same x as the
/// unfocused one, and the caret must be the same element the text path
/// uses.
#[gpui::test]
fn focused_empty_input_keeps_its_placeholder_and_caret(cx: &mut TestAppContext) {
    harness::still();
    let state = cx.new(|cx| InputState::new(cx));
    let other = cx.new(|cx| InputState::new(cx));
    let field = state.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .items_start()
            .child(
                gpui::div().debug_selector(|| "ph-field".to_owned()).child(
                    Input::new(field.clone())
                        .label("Name")
                        .placeholder("Type your name"),
                ),
            )
            .child(Input::new(other.clone()).label("Other"))
            .into_any_element()
    });
    cx.run_until_parked();
    let unfocused = cx
        .debug_bounds("input-placeholder")
        .expect("an unfocused empty field must paint its placeholder");
    cx.update(|window, cx| {
        let handle = state.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    cx.run_until_parked();
    let focused = cx
        .debug_bounds("input-placeholder")
        .expect("a focused empty field must keep its placeholder");
    assert_eq!(
        focused.origin.x, unfocused.origin.x,
        "the placeholder must not shift horizontally when focus toggles"
    );
    let field_bounds = cx.debug_bounds("ph-field").unwrap();
    let accent = cx.update(|_, cx| cx.colors().accent.color);
    let scene = harness::painted(cx);
    assert!(
        caret_quad(&scene, field_bounds, accent),
        "the focused empty field must paint its caret at the start"
    );
    cx.simulate_input("a");
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("input-placeholder").is_none(),
        "typing must replace the placeholder with the value"
    );
}

/// The same contract inside a multi-line field: the placeholder stays
/// painted under the blinking caret while the TextArea is focused and
/// empty.
#[gpui::test]
fn focused_empty_text_area_keeps_its_placeholder_and_caret(cx: &mut TestAppContext) {
    harness::still();
    let state = cx.new(|cx| InputState::new(cx));
    let field = state.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .items_start()
            .child(
                gpui::div().debug_selector(|| "ph-area".to_owned()).child(
                    TextArea::new(field.clone())
                        .label("Bio")
                        .placeholder("Tell us about yourself..."),
                ),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let unfocused = cx
        .debug_bounds("input-placeholder")
        .expect("an unfocused empty TextArea must paint its placeholder");
    cx.update(|window, cx| {
        let handle = state.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    cx.run_until_parked();
    let focused = cx
        .debug_bounds("input-placeholder")
        .expect("a focused empty TextArea must keep its placeholder");
    assert_eq!(
        focused.origin.x, unfocused.origin.x,
        "the TextArea placeholder must not shift horizontally on focus"
    );
    let field_bounds = cx.debug_bounds("ph-area").unwrap();
    let accent = cx.update(|_, cx| cx.colors().accent.color);
    let scene = harness::painted(cx);
    assert!(
        caret_quad(&scene, field_bounds, accent),
        "the focused empty TextArea must paint its caret at the start"
    );
    cx.simulate_input("a");
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("input-placeholder").is_none(),
        "typing must replace the TextArea placeholder with the value"
    );
}

// ---------------------------------------------------------------------------
// Input box geometry: `height`, `padding_x`, `is_bare`, `start_content`
// ---------------------------------------------------------------------------

// The field probes are the shared `harness::probe` (a 10px zero-behaviour
// marker with a debug selector), imported above as `inset_probe`: a slot
// child's origin is the box origin plus whatever inset the box carries.

/// The bounds of the element carrying the `name` debug selector. With no
/// label, description or error the field row is the wrapper column's only
/// child, so the wrapper's height *is* the box height.
fn box_bounds(cx: &mut VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("the `{name}` probe must paint"))
}

/// `debug_bounds` takes a `&'static str` and the InputGroup probes carry the
/// input's entity id, so the formatted keys are leaked for the test's
/// lifetime.
fn input_group_probe(entity: u64, suffix: &str) -> &'static str {
    Box::leak(format!("input-group-{entity}-{suffix}").into_boxed_str())
}

/// The default box is v3's 36px `.input`, and `height` replaces that one
/// value. Both boxes are measured; the assertion is that the default equals
/// the component's own constant and that the override differs from it by the
/// requested amount.
#[gpui::test]
fn input_height_replaces_the_field_height_and_defaults_to_it(cx: &mut TestAppContext) {
    const SHORT: f32 = 28.;
    let default_state = cx.new(|cx| InputState::new(cx));
    let short_state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                gpui::div()
                    .debug_selector(|| "h-default".to_owned())
                    .child(Input::new(default_state.clone())),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "h-short".to_owned())
                    .child(Input::new(short_state.clone()).height(px(SHORT))),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let default_box = box_bounds(cx, "h-default");
    let short_box = box_bounds(cx, "h-short");
    assert!(
        near(
            default_box.size.height,
            f32::from(herogpui_components::extend::FIELD_HEIGHT)
        ),
        "an untouched field must stay `util::FIELD_HEIGHT` tall, got {default_box:?}"
    );
    assert!(
        near(short_box.size.height, SHORT),
        "`height` must replace the box height, got {short_box:?}"
    );
    assert!(
        default_box.size.height - short_box.size.height
            == herogpui_components::extend::FIELD_HEIGHT - px(SHORT),
        "the only difference between the two boxes must be the requested \
         height delta: default={default_box:?} short={short_box:?}"
    );
}

/// `padding_x` replaces v3's `px-3`: the field's leading content moves by the
/// difference, and by nothing else.
#[gpui::test]
fn input_padding_x_moves_the_content_origin_by_the_requested_delta(cx: &mut TestAppContext) {
    const WIDE: f32 = 24.;
    let default_state = cx.new(|cx| InputState::new(cx));
    let wide_state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(Input::new(default_state.clone()).start_content(inset_probe("pad-default")))
            .child(
                Input::new(wide_state.clone())
                    .padding_x(px(WIDE))
                    .start_content(inset_probe("pad-wide")),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let default_probe = box_bounds(cx, "pad-default");
    let wide_probe = box_bounds(cx, "pad-wide");
    let delta = f32::from(wide_probe.origin.x - default_probe.origin.x);
    assert!(
        (delta - (WIDE - 12.)).abs() < 0.5,
        "`padding_x` must move the content origin by the delta over the \
         default 12px inset, moved {delta}: default={default_probe:?} \
         wide={wide_probe:?}"
    );
}

/// `is_bare(true)` drops the chrome the variant would paint. The measurable
/// trace of that chrome is the invalid state's 1px border, which insets the
/// content: a bare field carries none of it, and `is_bare(false)` keeps it.
#[gpui::test]
fn is_bare_drops_the_chrome_the_variant_would_paint(cx: &mut TestAppContext) {
    let bare_state = cx.new(|cx| InputState::new(cx));
    let chromed_state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                Input::new(bare_state.clone())
                    .is_bare(true)
                    .is_invalid(true)
                    .start_content(inset_probe("bare-probe")),
            )
            .child(
                Input::new(chromed_state.clone())
                    .is_bare(false)
                    .is_invalid(true)
                    .start_content(inset_probe("chromed-probe")),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let bare = box_bounds(cx, "bare-probe");
    let chromed = box_bounds(cx, "chromed-probe");
    assert!(
        chromed.origin.x > bare.origin.x,
        "the chromed invalid field must paint the danger border that insets \
         its content; the bare one must paint no chrome at all: \
         bare={bare:?} chromed={chromed:?}"
    );
    assert!(
        near(bare.origin.x, 12.),
        "a bare field must keep only its own padding, got {bare:?}"
    );
}

/// A bare field is still a field: it takes focus from a click and accepts
/// typing, because `is_bare` reuses the same chrome-less branch
/// `InputGroup.Input` takes rather than changing any behaviour.
#[gpui::test]
fn is_bare_keeps_the_field_focusable_and_editable(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Input::new(state_for_view.clone())
            .is_bare(true)
            .into_any_element()
    });
    click(cx, 40., 18.);
    press(cx, "b a r e");
    cx.update(|_, cx| {
        assert_eq!(
            state.read(cx).value(),
            "bare",
            "a bare field must still take focus and typing"
        );
    });
}

/// `focus_ring(false)` is narrower than `is_bare(true)`: the field keeps its
/// normal chrome and remains a live text control while the focus-ring paint is
/// opted out.
#[gpui::test]
fn focus_ring_opt_out_keeps_the_field_focusable_and_editable(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Input::new(state_for_view.clone())
            .focus_ring(false)
            .into_any_element()
    });
    click(cx, 40., 18.);
    press(cx, "r i n g");
    cx.update(|_, cx| {
        assert_eq!(
            state.read(cx).value(),
            "ring",
            "disabling the visual ring must not disable focus or editing"
        );
    });
}

/// The field chrome — `bg-field` and its border — painted as quads: the
/// fills of `colors.field.background` on a box at least `min_h` tall.
fn chrome_boxes(cx: &mut VisualTestContext, min_h: f32) -> usize {
    let scene = harness::painted(cx);
    let field = cx.update(|_, cx| {
        use herogpui_theme::ActiveTheme;
        cx.colors().field.background
    });
    scene
        .filled(field)
        .into_iter()
        .filter(|q| f32::from(scene.bounds(q).size.height) >= min_h)
        .count()
}

/// `Input` has one chrome call site, gated on `in_group` and `is_bare`: a
/// stock or ring-less field paints its chrome once, a bare one paints none,
/// and a grouped one leaves it to the group — so a group paints exactly one
/// chrome box around its held field. `focus_ring(false)` removes only the
/// ring (behaviour pinned by the test above), never the chrome.
#[gpui::test]
fn input_chrome_is_one_call_site_gated_on_bare_and_group(cx: &mut TestAppContext) {
    type Build = fn(Entity<InputState>) -> gpui::AnyElement;
    let cases: [(&str, Build, usize); 4] = [
        ("stock", |s| Input::new(s).into_any_element(), 1),
        (
            "focus_ring(false)",
            |s| Input::new(s).focus_ring(false).into_any_element(),
            1,
        ),
        (
            "is_bare",
            |s| Input::new(s).is_bare(true).into_any_element(),
            0,
        ),
        (
            "grouped",
            |s| InputGroup::new().input(Input::new(s)).into_any_element(),
            1,
        ),
    ];
    for (name, build, want) in cases {
        let state = cx.new(|cx| InputState::new(cx));
        harness::still();
        let cx = open_host(cx, move || {
            gpui::div()
                .w(px(320.))
                .child(build(state.clone()))
                .into_any_element()
        });
        assert_eq!(chrome_boxes(cx, 30.), want, "{name}: chrome boxes painted");
    }
}

/// `TextField` forwards `start_content`, so the labelled wrapper can carry a
/// leading icon without dropping to the bare `Input`.
#[gpui::test]
fn text_field_start_content_renders_inside_the_box(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        TextField::new(state.clone())
            .placeholder("Search")
            .start_content(inset_probe("tf-start"))
            .into_any_element()
    });
    flush_frame(cx);

    let probe = box_bounds(cx, "tf-start");
    assert!(
        near(probe.origin.x, 12.) && near(probe.size.width, 10.),
        "the leading element must render at the box's 12px inset, got {probe:?}"
    );
}

/// `height`, `padding_x` and `is_bare` are forwarded by `TextField` to the
/// same box.
#[gpui::test]
fn text_field_forwards_the_box_builders(cx: &mut TestAppContext) {
    const SHORT: f32 = 28.;
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .debug_selector(|| "tf-box".to_owned())
            .child(
                TextField::new(state.clone())
                    .height(px(SHORT))
                    .padding_x(px(4.))
                    .is_bare(true)
                    .font_family("monospace")
                    .start_content(inset_probe("tf-box-start")),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let wrapper = box_bounds(cx, "tf-box");
    let probe = box_bounds(cx, "tf-box-start");
    assert!(
        near(wrapper.size.height, SHORT),
        "the forwarded height must be the whole unlabelled column's height, \
         got {wrapper:?}"
    );
    assert!(
        near(probe.origin.x, 4.),
        "the forwarded padding must inset the leading element, got {probe:?}"
    );
}

// ---------------------------------------------------------------------------
// Field-family box geometry: TimeField, DateField, NumberField, ColorField,
// SearchField use the shared field-box seam
// ---------------------------------------------------------------------------

/// `TimeField` answers the same box builders as `Input`: the default is the
/// pinned 36px/12px box, `height` and `padding_x` move exactly those values,
/// and `is_bare` drops the invalid border that insets the leading prefix.
#[gpui::test]
fn time_field_box_builders_match_the_input_contract(cx: &mut TestAppContext) {
    const SHORT: f32 = 28.;
    const WIDE: f32 = 24.;
    let default_state = cx.new(|cx| TimeState::new(cx));
    let short_state = cx.new(|cx| TimeState::new(cx));
    let wide_state = cx.new(|cx| TimeState::new(cx));
    let bare_state = cx.new(|cx| TimeState::new(cx));
    let chromed_state = cx.new(|cx| TimeState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                gpui::div()
                    .debug_selector(|| "tf-default".to_owned())
                    .child(
                        TimeField::new(default_state.clone()).prefix(inset_probe("tf-pad-default")),
                    ),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "tf-short".to_owned())
                    .child(TimeField::new(short_state.clone()).height(px(SHORT))),
            )
            .child(
                TimeField::new(wide_state.clone())
                    .padding_x(px(WIDE))
                    .prefix(inset_probe("tf-pad-wide")),
            )
            .child(
                TimeField::new(bare_state.clone())
                    .is_bare(true)
                    .is_invalid(true)
                    .prefix(inset_probe("tf-bare")),
            )
            .child(
                TimeField::new(chromed_state.clone())
                    .is_invalid(true)
                    .prefix(inset_probe("tf-chromed")),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let default_box = box_bounds(cx, "tf-default");
    let short_box = box_bounds(cx, "tf-short");
    assert!(
        near(
            default_box.size.height,
            f32::from(herogpui_components::extend::FIELD_HEIGHT)
        ),
        "an untouched TimeField must stay `util::FIELD_HEIGHT` tall, got {default_box:?}"
    );
    assert!(
        near(short_box.size.height, SHORT),
        "`height` must replace the box height, got {short_box:?}"
    );

    let pad_default = box_bounds(cx, "tf-pad-default");
    let pad_wide = box_bounds(cx, "tf-pad-wide");
    let delta = f32::from(pad_wide.origin.x - pad_default.origin.x);
    assert!(
        (delta - (WIDE - 12.)).abs() < 0.5,
        "`padding_x` must move the prefix by the delta over the default 12px \
         inset, moved {delta}: default={pad_default:?} wide={pad_wide:?}"
    );

    let bare = box_bounds(cx, "tf-bare");
    let chromed = box_bounds(cx, "tf-chromed");
    assert!(
        chromed.origin.x > bare.origin.x,
        "the chromed invalid field must paint the danger border that insets \
         its prefix; the bare one must paint no chrome: bare={bare:?} \
         chromed={chromed:?}"
    );
    assert!(
        near(bare.origin.x, 12.),
        "a bare TimeField must keep only its own padding, got {bare:?}"
    );
}

/// `DateField` resolves the same shared box seam as `TimeField`.
#[gpui::test]
fn date_field_box_builders_match_the_input_contract(cx: &mut TestAppContext) {
    const SHORT: f32 = 28.;
    const WIDE: f32 = 24.;
    let default_state = cx.new(|cx| InputState::new(cx));
    let short_state = cx.new(|cx| InputState::new(cx));
    let wide_state = cx.new(|cx| InputState::new(cx));
    let bare_state = cx.new(|cx| InputState::new(cx));
    let chromed_state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                gpui::div()
                    .debug_selector(|| "df-default".to_owned())
                    .child(
                        DateField::new(default_state.clone()).prefix(inset_probe("df-box-default")),
                    ),
            )
            .child(
                gpui::div().debug_selector(|| "df-short".to_owned()).child(
                    DateField::new(short_state.clone())
                        .height(px(SHORT))
                        .prefix(inset_probe("df-box-short")),
                ),
            )
            .child(
                DateField::new(wide_state.clone())
                    .padding_x(px(WIDE))
                    .prefix(inset_probe("df-pad-wide")),
            )
            .child(
                DateField::new(bare_state.clone())
                    .is_bare(true)
                    .is_invalid(true)
                    .prefix(inset_probe("df-bare")),
            )
            .child(
                DateField::new(chromed_state.clone())
                    .is_invalid(true)
                    .prefix(inset_probe("df-chromed")),
            )
            .into_any_element()
    });
    flush_frame(cx);

    // DateField always renders a 16px format-hint description under the box
    // (4px gap), so the box is measured through its vertically centred
    // prefix: a 10px probe sits at (box - 10) / 2 from the wrapper top.
    let default_probe = box_bounds(cx, "df-box-default");
    let short_probe = box_bounds(cx, "df-box-short");
    let default_offset = f32::from(default_probe.origin.y - box_bounds(cx, "df-default").origin.y);
    let short_offset = f32::from(short_probe.origin.y - box_bounds(cx, "df-short").origin.y);
    assert!(
        near(
            px(default_offset),
            (f32::from(herogpui_components::extend::FIELD_HEIGHT) - 10.) / 2.
        ),
        "an untouched DateField must stay `util::FIELD_HEIGHT` tall, got {default_probe:?}"
    );
    assert!(
        near(px(short_offset), (SHORT - 10.) / 2.),
        "`height` must replace the box height, got {short_probe:?}"
    );

    let pad_default = box_bounds(cx, "df-box-default");
    let pad_wide = box_bounds(cx, "df-pad-wide");
    let delta = f32::from(pad_wide.origin.x - pad_default.origin.x);
    assert!(
        (delta - (WIDE - 12.)).abs() < 0.5,
        "`padding_x` must move the prefix by the delta over the default 12px \
         inset, moved {delta}: default={pad_default:?} wide={pad_wide:?}"
    );

    let bare = box_bounds(cx, "df-bare");
    let chromed = box_bounds(cx, "df-chromed");
    assert!(
        chromed.origin.x > bare.origin.x,
        "the chromed invalid field must paint the danger border that insets \
         its prefix; the bare one must paint no chrome: bare={bare:?} \
         chromed={chromed:?}"
    );
    assert!(
        near(bare.origin.x, 12.),
        "a bare DateField must keep only its own padding, got {bare:?}"
    );
}

/// `NumberField.height` replaces the group's 36px box and follows through to
/// the inner input so the smaller group is not defeated by the input's own
/// box.
#[gpui::test]
fn number_field_height_replaces_the_group_and_inner_field(cx: &mut TestAppContext) {
    const SHORT: f32 = 28.;
    let default_state = cx.new(|cx| NumberState::new(cx, 0.));
    let short_state = cx.new(|cx| NumberState::new(cx, 0.));
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                gpui::div()
                    .debug_selector(|| "nf-default".to_owned())
                    .child(NumberField::new(default_state.clone())),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "nf-short".to_owned())
                    .child(NumberField::new(short_state.clone()).height(px(SHORT))),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let default_box = box_bounds(cx, "nf-default");
    let short_box = box_bounds(cx, "nf-short");
    assert!(
        near(
            default_box.size.height,
            f32::from(herogpui_components::extend::FIELD_HEIGHT)
        ),
        "an untouched NumberField must stay `util::FIELD_HEIGHT` tall, got {default_box:?}"
    );
    assert!(
        near(short_box.size.height, SHORT),
        "`height` must replace the group height, got {short_box:?}"
    );
    assert!(
        default_box.size.height - short_box.size.height
            == herogpui_components::extend::FIELD_HEIGHT - px(SHORT),
        "the only difference between the two groups must be the requested \
         height delta: default={default_box:?} short={short_box:?}"
    );
}

/// `ColorField` answers the same seam in both render paths: the static box
/// (its suffix inset moves with `padding_x`) and the editable field (which
/// forwards the builders to its inner `Input`).
#[gpui::test]
fn color_field_box_builders_cover_both_render_paths(cx: &mut TestAppContext) {
    const SHORT: f32 = 28.;
    const WIDE: f32 = 24.;
    let state = cx.new(|cx| InputState::new(cx));
    let default_state = cx.new(|cx| InputState::new(cx));
    let short_state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                gpui::div().debug_selector(|| "cf-static".to_owned()).child(
                    ColorField::new("cf-static", PickerColor::hsb(180., 1., 1.))
                        .suffix(inset_probe("cf-suffix-default")),
                ),
            )
            .child(
                ColorField::new("cf-static-wide", PickerColor::hsb(180., 1., 1.))
                    .padding_x(px(WIDE))
                    .suffix(inset_probe("cf-suffix-wide")),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "cf-edit-default".to_owned())
                    .child(
                        ColorField::new("cf-edit", PickerColor::hsb(180., 1., 1.))
                            .state(default_state.clone()),
                    ),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "cf-edit-short".to_owned())
                    .child(
                        ColorField::new("cf-edit-short", PickerColor::hsb(180., 1., 1.))
                            .state(short_state.clone())
                            .height(px(SHORT)),
                    ),
            )
            .child(
                ColorField::new("cf-edit-h", PickerColor::hsb(180., 1., 1.))
                    .is_bare(true)
                    .state(state.clone()),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let static_box = box_bounds(cx, "cf-static");
    assert!(
        near(
            static_box.size.height,
            f32::from(herogpui_components::extend::FIELD_HEIGHT)
        ),
        "an untouched static ColorField must stay `util::FIELD_HEIGHT` tall, \
         got {static_box:?}"
    );

    let suffix_default = box_bounds(cx, "cf-suffix-default");
    let suffix_wide = box_bounds(cx, "cf-suffix-wide");
    let delta = f32::from(
        (suffix_default.origin.x + suffix_default.size.width)
            - (suffix_wide.origin.x + suffix_wide.size.width),
    );
    assert!(
        (delta - (WIDE - 12.)).abs() < 0.5,
        "`padding_x` must move the trailing suffix inward by the delta over \
         the default 12px inset, moved {delta}: default={suffix_default:?} \
         wide={suffix_wide:?}"
    );

    let edit_default = box_bounds(cx, "cf-edit-default");
    let edit_short = box_bounds(cx, "cf-edit-short");
    assert!(
        near(
            edit_default.size.height,
            f32::from(herogpui_components::extend::FIELD_HEIGHT)
        ),
        "the editable ColorField must inherit the stock Input height, got \
         {edit_default:?}"
    );
    assert!(
        near(edit_short.size.height, SHORT),
        "the editable ColorField must forward `height` to the inner Input, \
         got {edit_short:?}"
    );
}

/// `SearchField.height` and `padding_x` reach the inner field; the trailing
/// content moves inward by the padding delta.
#[gpui::test]
fn search_field_box_builders_reach_the_inner_field(cx: &mut TestAppContext) {
    const SHORT: f32 = 28.;
    const WIDE: f32 = 24.;
    let default_state = cx.new(|cx| InputState::new(cx));
    let short_state = cx.new(|cx| InputState::new(cx));
    let wide_state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                gpui::div()
                    .w(px(320.))
                    .debug_selector(|| "sf-default".to_owned())
                    .child(
                        SearchField::new(default_state.clone())
                            .full_width()
                            .end_content(inset_probe("sf-end-default")),
                    ),
            )
            .child(
                gpui::div()
                    .w(px(320.))
                    .debug_selector(|| "sf-short".to_owned())
                    .child(
                        SearchField::new(short_state.clone())
                            .full_width()
                            .height(px(SHORT)),
                    ),
            )
            .child(
                gpui::div().w(px(320.)).child(
                    SearchField::new(wide_state.clone())
                        .full_width()
                        .padding_x(px(WIDE))
                        .end_content(inset_probe("sf-end-wide")),
                ),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let default_box = box_bounds(cx, "sf-default");
    let short_box = box_bounds(cx, "sf-short");
    assert!(
        near(
            default_box.size.height,
            f32::from(herogpui_components::extend::FIELD_HEIGHT)
        ),
        "an untouched SearchField must stay `util::FIELD_HEIGHT` tall, got {default_box:?}"
    );
    assert!(
        near(short_box.size.height, SHORT),
        "`height` must reach the inner field, got {short_box:?}"
    );

    let end_default = box_bounds(cx, "sf-end-default");
    let end_wide = box_bounds(cx, "sf-end-wide");
    let delta = f32::from(
        (end_default.origin.x + end_default.size.width) - (end_wide.origin.x + end_wide.size.width),
    );
    assert!(
        (delta - (WIDE - 12.)).abs() < 0.5,
        "`padding_x` must move the trailing content inward by the delta over \
         the default 12px inset, moved {delta}: default={end_default:?} \
         wide={end_wide:?}"
    );
}

/// The bare gate is wired at each owner's one chrome call site: the stock
/// field paints its chrome once, `is_bare(true)` paints none.
#[gpui::test]
fn the_field_family_gates_its_chrome_on_one_bare_flag(cx: &mut TestAppContext) {
    for bare in [false, true] {
        let time = cx.new(|cx| TimeState::new(cx));
        let date = cx.new(|cx| InputState::new(cx));
        let number = cx.new(|cx| NumberState::new(cx, 0.));
        type Build = Box<dyn Fn() -> gpui::AnyElement>;
        let fields: [(&str, Build); 4] = [
            (
                "TimeField",
                Box::new(move || {
                    TimeField::new(time.clone())
                        .is_bare(bare)
                        .into_any_element()
                }),
            ),
            (
                "DateField",
                Box::new(move || {
                    DateField::new(date.clone())
                        .is_bare(bare)
                        .into_any_element()
                }),
            ),
            (
                "ColorField",
                Box::new(move || {
                    ColorField::new("cf-bare", PickerColor::from_hex("#FF0000"))
                        .is_bare(bare)
                        .into_any_element()
                }),
            ),
            (
                "NumberField",
                Box::new(move || {
                    NumberField::new(number.clone())
                        .is_bare(bare)
                        .into_any_element()
                }),
            ),
        ];
        for (name, build) in fields {
            harness::still();
            let cx = open_host(cx, move || {
                gpui::div().w(px(320.)).child(build()).into_any_element()
            });
            assert_eq!(
                chrome_boxes(cx, 30.),
                usize::from(!bare),
                "{name} bare={bare}: chrome boxes painted"
            );
        }
    }
}

/// The distinct fills painted exactly over `target`.
fn fills_over(scene: &harness::Painted, target: Bounds<Pixels>) -> Vec<gpui::Hsla> {
    let mut fills: Vec<gpui::Hsla> = Vec::new();
    for q in &scene.quads {
        let b = scene.bounds(q);
        if harness::contains(b, target) && harness::contains(target, b) {
            if let Some(c) = q.background.as_solid().filter(|c| c.a > 0.) {
                if !harness::has_color(&fills, c) {
                    fills.push(c);
                }
            }
        }
    }
    fills
}

/// ColorField's static group eases its fill to the variant's hover endpoint
/// (`field.hover()` on Primary, `default.hover()` on Secondary) over its
/// stylesheet's own 150ms — not the
/// theme's shared duration, which is stretched to a minute here, so after
/// 400ms the fade must have settled — and a disabled or invalid group keeps
/// its chrome under the pointer. (The border's hover token paints nothing in
/// the stock theme, whose field border is zero wide.)
#[gpui::test]
fn color_field_static_group_uses_the_pinned_hover_transition(cx: &mut TestAppContext) {
    use gpui::Modifiers;
    use herogpui_components::FieldVariant;
    use herogpui_theme::{set_theme, ActiveTheme, Theme};

    fn red() -> PickerColor {
        PickerColor::from_hex("#FF0000").expect("red")
    }
    type Build = fn() -> gpui::AnyElement;
    let cases: [(&str, Build, bool); 4] = [
        (
            "Primary",
            || ColorField::new("cf-h", red()).into_any_element(),
            true,
        ),
        (
            "Secondary",
            || {
                ColorField::new("cf-h", red())
                    .variant(FieldVariant::Secondary)
                    .into_any_element()
            },
            true,
        ),
        (
            "disabled",
            || {
                ColorField::new("cf-h", red())
                    .is_disabled(true)
                    .into_any_element()
            },
            false,
        ),
        (
            "invalid",
            || {
                ColorField::new("cf-h", red())
                    .is_invalid(true)
                    .into_any_element()
            },
            false,
        ),
    ];
    for (name, build, hovers) in cases {
        let cx = open_host(cx, move || {
            gpui::div()
                .p(px(24.))
                .w(px(320.))
                .child(build())
                .into_any_element()
        });
        cx.update(|window, cx| {
            set_theme(
                Theme::builder("slow", Theme::light())
                    .hover_fade_ms(60_000)
                    .build(),
                cx,
            );
            window.refresh();
        });
        let resting = harness::painted(cx);
        let group = resting
            .quads
            .iter()
            .filter(|q| q.background.as_solid().is_some_and(|c| c.a > 0.))
            .filter(|q| f32::from(resting.bounds(q).size.height) > 30.)
            .map(|q| resting.bounds(q))
            .next()
            .unwrap_or_else(|| panic!("{name}: the group must paint\n{}", resting.describe()));
        let before = fills_over(&resting, group);
        cx.simulate_mouse_move(group.center(), None, Modifiers::none());
        harness::wait_ms(cx, 400);
        let scene = harness::painted(cx);
        let after = fills_over(&scene, group);
        let endpoint = cx.update(|_, cx| {
            let colors = cx.colors();
            if name == "Secondary" {
                colors.default.hover()
            } else {
                colors.field.hover()
            }
        });
        if hovers {
            assert!(
                after.len() == 1 && harness::same_color(after[0], endpoint),
                "{name}: after 400ms the group's own 150ms fade must have settled \
                 on {endpoint:?}, painted {after:?}"
            );
        } else {
            assert_eq!(
                after, before,
                "{name}: the group keeps its chrome under the pointer"
            );
        }
    }
}

/// `InputGroup.height` is the group's box and reaches the held Input so the
/// inner 36px field cannot defeat the smaller group.
#[gpui::test]
fn input_group_height_replaces_the_group_and_inner_field(cx: &mut TestAppContext) {
    const SHORT: f32 = 28.;
    let default_state = cx.new(|cx| InputState::new(cx));
    let short_state = cx.new(|cx| InputState::new(cx));
    let default_entity = default_state.entity_id().as_u64();
    let short_entity = short_state.entity_id().as_u64();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(16.))
            .child(InputGroup::new().input(Input::new(default_state.clone())))
            .child(
                InputGroup::new()
                    .height(px(SHORT))
                    .input(Input::new(short_state.clone())),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let default = cx
        .debug_bounds(input_group_probe(default_entity, "group"))
        .expect("the default group must be laid out");
    let short = cx
        .debug_bounds(input_group_probe(short_entity, "group"))
        .expect("the short group must be laid out");
    assert!(
        near(
            default.size.height,
            f32::from(herogpui_components::extend::FIELD_HEIGHT)
        ),
        "an untouched group must keep the 36px floor, got {default:?}"
    );
    assert!(
        near(short.size.height, SHORT),
        "`height` must replace the group's floor, got {short:?}"
    );
}

/// `InputGroup.padding_x` replaces the inner field's `px-3` on the sides with
/// no addon: an addon-less group grows by exactly twice the delta.
#[gpui::test]
fn input_group_padding_x_reaches_the_inner_field(cx: &mut TestAppContext) {
    const WIDE: f32 = 24.;
    let default_state = cx.new(|cx| InputState::new(cx));
    let wide_state = cx.new(|cx| InputState::new(cx));
    let default_entity = default_state.entity_id().as_u64();
    let wide_entity = wide_state.entity_id().as_u64();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(16.))
            .child(InputGroup::new().input(Input::new(default_state.clone())))
            .child(
                InputGroup::new()
                    .padding_x(px(WIDE))
                    .input(Input::new(wide_state.clone())),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let default = cx
        .debug_bounds(input_group_probe(default_entity, "group"))
        .expect("the default group must be laid out");
    let wide = cx
        .debug_bounds(input_group_probe(wide_entity, "group"))
        .expect("the wide group must be laid out");
    let delta = f32::from(wide.size.width) - f32::from(default.size.width);
    assert!(
        (delta - (WIDE - 12.) * 2.).abs() < 0.5,
        "`padding_x` must inset both exposed sides by the delta over px-3, \
         moved {delta}: default={default:?} wide={wide:?}"
    );
}

/// The group gates its one chrome call site on the bare flag and forwards
/// its seam to the held field. (The `padding_x` forward is measured by
/// `input_group_padding_x_reaches_the_inner_field`.)
#[gpui::test]
fn input_group_gates_chrome_and_forwards_the_seam(cx: &mut TestAppContext) {
    for bare in [false, true] {
        let state = cx.new(|cx| InputState::new(cx));
        harness::still();
        let cx = open_host(cx, move || {
            gpui::div()
                .w(px(320.))
                .child(
                    InputGroup::new()
                        .is_bare(bare)
                        .input(Input::new(state.clone())),
                )
                .into_any_element()
        });
        assert_eq!(
            chrome_boxes(cx, 30.),
            usize::from(!bare),
            "bare={bare}: the group paints its chrome unless bare"
        );
    }

    // Remaining source-text check: the held field's own box has no selector,
    // and its content centres in the group whether or not it took the
    // forwarded height, so the forward itself leaves no measurable trace.
    assert!(
        source_scan::component_src("input_group.rs")
            .contains("Some(height) => input.height(height),"),
        "the group must forward the explicit height to the held field"
    );
}

/// `padding_x` must not switch NumberField's inner field to the standalone
/// `Input` wrapper: that wrapper caps at 320px, which would leave the rest of
/// a wide group inert. A click far past 320px must still focus and type.
#[gpui::test]
fn number_field_padding_override_keeps_the_wide_editable_area(cx: &mut TestAppContext) {
    let state = cx.new(|cx| NumberState::new(cx, 0.));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(600.))
            .child(
                NumberField::new(state_for_view.clone())
                    .full_width(true)
                    .hide_steppers(true)
                    .padding_x(px(12.)),
            )
            .into_any_element()
    });
    flush_frame(cx);

    click(cx, 520., 18.);
    press(cx, "5");
    cx.update(|_, cx| {
        assert!(
            (state.read(cx).value() - 5.).abs() < f64::EPSILON,
            "a click past the standalone Input's 320px cap must still focus \
             and type, got {}",
            state.read(cx).value()
        );
    });
}

/// An explicit group height shrinks the vertical stepper column's two buttons
/// so both stay inside the overflow-hidden box.
#[gpui::test]
fn number_field_short_height_keeps_vertical_steppers_reachable(cx: &mut TestAppContext) {
    const SHORT: f32 = 28.;
    let state = cx.new(|cx| NumberState::new(cx, 5.));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(220.))
            .child(
                NumberField::new(state_for_view.clone())
                    .vertical_steppers(true)
                    .height(px(SHORT)),
            )
            .into_any_element()
    });
    flush_frame(cx);

    // The vertical column occupies the right 24px: increment on top,
    // decrement below; at a 28px group each button is 14px tall. y=16 is
    // inside the corrected decrement (14..28) but still inside the original
    // 18px increment, so the click discriminates the fix.
    click(cx, 208., 16.);
    cx.update(|_, cx| {
        assert!(
            (state.read(cx).value() - 4.).abs() < f64::EPSILON,
            "the lower stepper must stay reachable at an explicit height, got {}",
            state.read(cx).value()
        );
    });
}

/// The group's padding override moves the field's exposed edge; a side that
/// touches an addon keeps the addon's own inset. The public
/// `Input::padding_x` remains ignored inside a group, as documented.
#[gpui::test]
fn input_group_padding_override_respects_addon_sides(cx: &mut TestAppContext) {
    const WIDE: f32 = 24.;
    let plain = cx.new(|cx| InputState::new(cx));
    let plain_wide = cx.new(|cx| InputState::new(cx));
    let prefixed = cx.new(|cx| InputState::new(cx));
    let prefixed_wide = cx.new(|cx| InputState::new(cx));
    let direct = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(16.))
            .child(
                InputGroup::new()
                    .padding_x(px(4.))
                    .input(Input::new(plain.clone()).start_content(inset_probe("ig-pad-4"))),
            )
            .child(
                InputGroup::new()
                    .padding_x(px(WIDE))
                    .input(Input::new(plain_wide.clone()).start_content(inset_probe("ig-pad-24"))),
            )
            .child(
                InputGroup::new()
                    .padding_x(px(4.))
                    .prefix(InputAddon::new("$"))
                    .input(Input::new(prefixed.clone()).start_content(inset_probe("ig-prefix-4"))),
            )
            .child(
                InputGroup::new()
                    .padding_x(px(WIDE))
                    .prefix(InputAddon::new("$"))
                    .input(
                        Input::new(prefixed_wide.clone())
                            .start_content(inset_probe("ig-prefix-24")),
                    ),
            )
            .child(
                InputGroup::new().input(
                    Input::new(direct.clone())
                        .padding_x(px(30.))
                        .start_content(inset_probe("ig-direct")),
                ),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let pad4 = box_bounds(cx, "ig-pad-4");
    let pad24 = box_bounds(cx, "ig-pad-24");
    assert!(
        near(pad4.origin.x, 4.),
        "the group override must set the exposed edge, got {pad4:?}"
    );
    assert!(
        near(pad24.origin.x, WIDE),
        "the wide group override must set the exposed edge, got {pad24:?}"
    );

    let prefix4 = box_bounds(cx, "ig-prefix-4");
    let prefix24 = box_bounds(cx, "ig-prefix-24");
    assert!(
        near(prefix4.origin.x, f32::from(prefix24.origin.x)),
        "a side with an addon keeps the addon's inset regardless of the \
         group override: 4={prefix4:?} 24={prefix24:?}"
    );

    let direct = box_bounds(cx, "ig-direct");
    assert!(
        near(direct.origin.x, 12.),
        "Input::padding_x must stay ignored inside a group, got {direct:?}"
    );
}

/// The static ColorField's `is_bare` drops the invalid border that insets the
/// trailing suffix.
#[gpui::test]
fn color_field_static_bare_drops_the_invalid_border(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(16.))
            .child(
                ColorField::new("cf-bare-static", PickerColor::hsb(180., 1., 1.))
                    .is_bare(true)
                    .is_invalid(true)
                    .suffix(inset_probe("cf-bare-suffix")),
            )
            .child(
                ColorField::new("cf-chromed-static", PickerColor::hsb(180., 1., 1.))
                    .is_invalid(true)
                    .suffix(inset_probe("cf-chromed-suffix")),
            )
            .child(
                ColorField::new("cf-valid-static", PickerColor::hsb(180., 1., 1.))
                    .suffix(inset_probe("cf-valid-suffix")),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let bare_invalid = box_bounds(cx, "cf-bare-suffix");
    let chromed_invalid = box_bounds(cx, "cf-chromed-suffix");
    let valid = box_bounds(cx, "cf-valid-suffix");
    assert!(
        near(bare_invalid.origin.x, f32::from(valid.origin.x)),
        "a bare static ColorField must paint no border even when invalid: \
         bare={bare_invalid:?} valid={valid:?}"
    );
    assert!(
        chromed_invalid.origin.x < valid.origin.x,
        "the chromed invalid field must paint the danger border that insets \
         its suffix: chromed={chromed_invalid:?} valid={valid:?}"
    );
}
