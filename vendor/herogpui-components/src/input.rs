//! Input & InputState — port of `@heroui/input`.
//!
//! `InputState` is an entity holding the editable value; [`Input`] is the
//! styled element bound to it (controlled like HeroUI's controlled inputs).

use gpui::{
    prelude::*, px, App, Entity, FocusHandle, Focusable, IntoElement, KeyDownEvent, Pixels,
    RenderOnce, SharedString, Styled, Window,
};
use herogpui_core::{element_id, FieldVariant};
use herogpui_theme::ActiveTheme;

use crate::a11y::A11y as _;

/// Editable state of a single-line text input.
pub struct InputState {
    value: String,
    /// Cursor position in char indices (the focused end of the selection).
    cursor: usize,
    /// Selection anchor in char indices; `None` when the caret is collapsed.
    anchor: Option<usize>,
    marked: Option<std::ops::Range<usize>>,
    pub(crate) focus_handle: FocusHandle,
    /// Clear-button handle. Created without `tab_stop`, matching pinned
    /// `useSearchField` `excludeFromTabOrder: true`. Never a `tab_stop_handle`.
    pub(crate) clear_focus_handle: FocusHandle,
    /// `name` — what this field submits under, written in by the component's
    /// `name` builder. The state carries it because gpui gives a child no way
    /// to reach its `Form`; `FormField::text` reads it back out.
    name: Option<SharedString>,
    /// `validationBehavior` — travels with the name, for the same reason.
    validation_behavior: crate::form::ValidationBehavior,
    /// The resolved validity, written by `Input::render` so a `Form` can see
    /// whether this field blocks a native submission — the same reason `name`
    /// rides on the state.
    validity: crate::validation::Validity,
    /// Disabled native controls are not successful and therefore do not
    /// contribute an entry to FormData. Written by `Input::render` so the
    /// registered [`crate::form::FormField`] can read the live state.
    is_successful: bool,
    /// Read-only native controls stay successful and focusable, but HTML
    /// constraint validation bars them: neither a missing value nor a stored
    /// error on a read-only field may block a submission. Mirrored by
    /// `Input::render` like `is_successful`, so the form reads the rendered
    /// state rather than a builder snapshot — and `NumberField` inherits the
    /// same mirror through the inner `InputState` it forwards its own
    /// `isReadOnly` to.
    is_read_only: bool,
    /// Server messages routed to this field by its `Form`'s
    /// `validationErrors` record (`form.rs`). The form writes them on a new
    /// record, and *this field's own editing* clears them — v3:
    /// "displayed immediately and cleared when user modifies the field" —
    /// so a message never outlives the edit that answers it, and a sibling's
    /// message never disappears with someone else's edit. `Input::render`
    /// merges them into the resolved validity, ahead of the field's own
    /// `validationErrors` prop.
    routed_errors: Vec<SharedString>,
    /// The delivery receipt for `routed_errors`: the record revision these
    /// messages last came from, `0` before anything arrived. The form's
    /// delivery consults it — a record the receipt already names is a clone
    /// of one already delivered and re-arms nothing — so it moves with the
    /// messages in one write and survives edit suppression: clearing the
    /// messages must never rewind the receipt, or the next frame's clone
    /// would resurrect what the edit answered.
    routed_revision: u64,
    /// The commit-gated validity snapshot — React Stately's `nextValidation`
    /// (rewritten every render) and `currentValidity` (the last commit's
    /// copy). Under `ValidationBehavior::Native` the field paints the
    /// displayed half, not the live result: typing after a commit leaves the
    /// shown error untouched until the next commit (a change-blur or a
    /// submit). `Allow` ignores it and shows the live validity in real time.
    validation_display: crate::validation::ValidationDisplay,
}

impl InputState {
    /// Creates an empty state with a fresh focus handle.
    pub fn new(cx: &mut App) -> Self {
        Self {
            value: String::new(),
            cursor: 0,
            anchor: None,
            marked: None,
            // A field is a tab stop: the handle carries that, not the element.
            focus_handle: cx.focus_handle().tab_stop(true),
            // Plain handle: Tab never seats on the clear button.
            clear_focus_handle: cx.focus_handle(),
            name: None,
            validation_behavior: crate::form::ValidationBehavior::Native,
            validity: crate::validation::Validity::default(),
            is_successful: true,
            is_read_only: false,
            routed_errors: Vec::new(),
            routed_revision: 0,
            validation_display: crate::validation::ValidationDisplay::default(),
        }
    }

    /// `defaultValue` — a state seeded with initial text, with the caret at the
    /// end.
    ///
    /// This is the uncontrolled entry point: React needs a `defaultValue` prop
    /// because it has no state object to seed.
    pub fn with_value(cx: &mut App, value: impl Into<String>) -> Self {
        let mut state = Self::new(cx);
        state.set_value(value);
        state
    }

    /// The current text value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The `name` this field submits under, if one was set.
    pub fn name(&self) -> Option<SharedString> {
        self.name.clone()
    }

    /// Sets the submission name. Called by the component's `name` builder, not
    /// usually by hand.
    pub fn set_name(&mut self, name: Option<SharedString>) {
        self.name = name;
    }

    /// Whether this field's invalidity blocks form submission.
    pub fn validation_behavior(&self) -> crate::form::ValidationBehavior {
        self.validation_behavior
    }

    /// Set by the component's `validation_behavior` builder.
    pub fn set_validation_behavior(&mut self, behavior: crate::form::ValidationBehavior) {
        self.validation_behavior = behavior;
    }

    /// Records the resolved validity, written by `Input::render` so `Form`
    /// can see why a native submission is blocked. The write is guarded at
    /// the call site (only when the value differs), like `set_name`'s.
    pub(crate) fn set_validity(&mut self, validity: crate::validation::Validity) {
        self.validity = validity;
    }

    /// The resolved validity, as last written by `Input::render`.
    pub(crate) fn validity(&self) -> &crate::validation::Validity {
        &self.validity
    }

    /// Records the commit-gated validity, written by `Input::render` every
    /// frame; a commit copies it into the displayed snapshot.
    pub(crate) fn set_pending(&mut self, pending: crate::validation::Validity) {
        self.validation_display.sync(pending);
    }

    /// The commit-gated validity as last written by `Input::render`.
    pub(crate) fn pending(&self) -> &crate::validation::Validity {
        self.validation_display.pending()
    }

    /// The displayed validity snapshot — what `ValidationBehavior::Native`
    /// paints. Committed by a change-blur or a form submission.
    pub(crate) fn displayed(&self) -> &crate::validation::Validity {
        self.validation_display.displayed()
    }

    /// A commit: the pending gated validity becomes the displayed snapshot.
    /// Called by `Input::render`'s focus tracking and by `FormField::commit`.
    /// Returns whether the displayed half changed, so callers can notify.
    pub(crate) fn commit_display(&mut self) -> bool {
        self.validation_display.commit()
    }

    /// Queue a commit that lands on the next render's sync — a compound
    /// field's calendar pick or step commits the bound state's snapshot the
    /// same way its own event handlers do.
    pub(crate) fn queue_commit_display(&mut self) {
        self.validation_display.queue_commit();
    }

    /// `resetValidation` — the display returns to valid without committing;
    /// the next render's pending result waits for the next commit again.
    /// Returns whether the displayed half changed, so callers can notify.
    pub(crate) fn reset_display(&mut self) -> bool {
        self.validation_display.reset()
    }

    pub(crate) fn is_successful(&self) -> bool {
        self.is_successful
    }

    pub(crate) fn set_successful(&mut self, is_successful: bool) {
        self.is_successful = is_successful;
    }

    /// The rendered read-only state, as last written by `Input::render`.
    pub(crate) fn is_read_only(&self) -> bool {
        self.is_read_only
    }

    pub(crate) fn set_read_only(&mut self, is_read_only: bool) {
        self.is_read_only = is_read_only;
    }

    /// The server messages the form routed to this field, as last written by
    /// the form's `validationErrors` delivery — what this field's error slot
    /// renders, and what a native submit consults beside the stored
    /// validity. Empty once the user edited the field or a reset hid them.
    pub fn routed_errors(&self) -> &[SharedString] {
        &self.routed_errors
    }

    /// The delivery receipt beside [`Self::routed_errors`] — the record
    /// revision these messages last came from, `0` before any delivery.
    pub(crate) fn routed_revision(&self) -> u64 {
        self.routed_revision
    }

    /// Replaces the routed server messages *and* the receipt that names the
    /// record they came from, in one update. Guarded by the caller, which
    /// compares the receipt before writing so a render cannot notify-loop.
    pub(crate) fn set_routed(&mut self, messages: Vec<SharedString>, revision: u64) {
        self.routed_errors = messages;
        self.routed_revision = revision;
    }

    /// Suppresses the routed server messages: the user modified *this* field,
    /// so only its messages clear and its siblings keep theirs. The delivery
    /// receipt is deliberately untouched — the record that delivered already
    /// named this field, so a clone re-rendered on the next frame must not
    /// resurrect what the edit answered.
    pub(crate) fn clear_routed_errors(&mut self) {
        self.routed_errors.clear();
    }

    /// Replaces the value, clears any selection and composition, and moves the cursor to the end.
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.marked = None;
        self.anchor = None;
        self.cursor = self.value.chars().count();
    }

    /// Whether the value is empty.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// The clear affordance's focus handle. It is not a tab stop.
    pub fn clear_focus_handle(&self) -> FocusHandle {
        self.clear_focus_handle.clone()
    }

    /// Normalized `(start, end)` char range of the active selection.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        let c = self.cursor;
        if a == c {
            None
        } else {
            Some((a.min(c), a.max(c)))
        }
    }
}

impl Focusable for InputState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// -- char-index editing helpers -------------------------------------------

/// Checks a whole value against the v3 field constraints.
#[allow(clippy::too_many_arguments)]
fn validate_value(
    value: &str,
    input_type: InputType,
    min_length: Option<usize>,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    pattern: Option<&dyn Fn(&str) -> bool>,
) -> InputValidity {
    // An empty field is "not yet filled in", not invalid; `is_required` is the
    // prop that speaks to emptiness.
    if value.is_empty() {
        return InputValidity::Valid;
    }

    if let Some(f) = pattern {
        if !f(value) {
            return InputValidity::PatternMismatch;
        }
    }

    if min_length.is_some_and(|n| value.chars().count() < n) {
        return InputValidity::TooShort;
    }

    // The numeric bounds only mean anything for a numeric field with a
    // parsable value.
    if input_type == InputType::Number {
        if let Ok(n) = value.parse::<f64>() {
            if min.is_some_and(|lo| n < lo) {
                return InputValidity::BelowMin;
            }
            if max.is_some_and(|hi| n > hi) {
                return InputValidity::AboveMax;
            }
            if let Some(step) = step.filter(|s| *s > 0.0) {
                let base = min.unwrap_or(0.0);
                let steps = (n - base) / step;
                if (steps - steps.round()).abs() > 1e-6 {
                    return InputValidity::OffStep;
                }
            }
        }
    }

    InputValidity::Valid
}

/// Refreshes the stored validity mirror after an accepted edit. `render`
/// writes that mirror, so without this it is one frame old — and an
/// `on_change` may submit the enclosing `Form` synchronously (the auto-submit
/// v3's own docs invite) before any frame catches up, reading the routed
/// server error the edit just answered as still blocking. The routed slot is
/// left out — the edit suppressed it — so the mirror is resolved from the
/// builder's own sources exactly as the next render would resolve it with an
/// empty routed slot, HTML5 attribute check included.
fn refresh_stored_validity(
    state: &Entity<InputState>,
    is_invalid: bool,
    validation_errors: &[SharedString],
    validate: Option<&crate::validation::Validator<str>>,
    error_message: Option<SharedString>,
    native_valid: &dyn Fn(&str) -> bool,
    cx: &mut App,
) {
    let value = state.read(cx).value().to_owned();
    let mut validity = crate::validation::resolve(
        is_invalid,
        validation_errors,
        validate.and_then(|f| f(&value)),
        error_message,
    );
    if !native_valid(&value) {
        validity.is_invalid = true;
    }
    if state.read(cx).validity() != &validity {
        state.update(cx, |s, _| s.set_validity(validity));
    }
}

/// Whether `ch` may be inserted, honouring `type` and `maxLength`.
///
/// Takes the measured lengths rather than the whole state so it stays a pure
/// function (an `InputState` needs an `App` for its focus handle).
fn accepts_char(
    len_chars: usize,
    selected_chars: usize,
    ch: char,
    input_type: InputType,
    max_length: Option<usize>,
) -> bool {
    if !input_type.accepts(ch) {
        return false;
    }
    match max_length {
        // A selection is replaced by the keystroke, so it frees up room.
        Some(max) => len_chars.saturating_sub(selected_chars) < max,
        None => true,
    }
}

/// [`accepts_char`] for a live state.
fn state_accepts(
    state: &InputState,
    ch: char,
    input_type: InputType,
    max_length: Option<usize>,
) -> bool {
    let selected = state.selection().map_or(0, |(lo, hi)| hi - lo);
    accepts_char(
        state.value.chars().count(),
        selected,
        ch,
        input_type,
        max_length,
    )
}

fn insert_char(state: &mut InputState, ch: char) {
    delete_selection(state);
    // A collapsed anchor can remain after a multi-character platform edit
    // sets the range once and then inserts several characters. Treat the
    // first insertion as a normal caret edit so the next character does not
    // delete the character just inserted.
    state.anchor = None;
    let byte_idx = char_to_byte(&state.value, state.cursor);
    state.value.insert(byte_idx, ch);
    state.cursor += 1;
}

/// Removes the active selection (if any); returns true when it did.
fn delete_selection(state: &mut InputState) -> bool {
    if let Some((lo, hi)) = state.selection() {
        let lo_b = char_to_byte(&state.value, lo);
        let hi_b = char_to_byte(&state.value, hi);
        state.value.replace_range(lo_b..hi_b, "");
        state.cursor = lo;
        state.anchor = None;
        true
    } else {
        false
    }
}

/// The char index of the extended grapheme cluster boundary before `cursor`
/// (a char index), so caret motion and deletion never split an emoji, a flag,
/// a ZWJ sequence or a base letter from its combining marks.
fn prev_grapheme(value: &str, cursor: usize) -> usize {
    if cursor == 0 {
        return 0;
    }
    let byte = char_to_byte(value, cursor);
    let mut graphemes = unicode_segmentation::GraphemeCursor::new(byte, value.len(), true);
    match graphemes.prev_boundary(value, 0) {
        Ok(Some(prev)) => byte_to_char(value, prev),
        _ => cursor - 1,
    }
}

/// The char index of the extended grapheme cluster boundary after `cursor`;
/// see [`prev_grapheme`].
fn next_grapheme(value: &str, cursor: usize) -> usize {
    let byte = char_to_byte(value, cursor);
    if byte >= value.len() {
        return cursor;
    }
    let mut graphemes = unicode_segmentation::GraphemeCursor::new(byte, value.len(), true);
    match graphemes.next_boundary(value, 0) {
        Ok(Some(next)) => byte_to_char(value, next),
        _ => cursor + 1,
    }
}

fn backspace(state: &mut InputState) -> bool {
    if delete_selection(state) {
        return true;
    }
    if state.cursor == 0 {
        return false;
    }
    let byte_idx = char_to_byte(&state.value, state.cursor);
    let prev_char = prev_grapheme(&state.value, state.cursor);
    let prev = char_to_byte(&state.value, prev_char);
    state.value.replace_range(prev..byte_idx, "");
    state.cursor = prev_char;
    true
}

fn delete(state: &mut InputState) -> bool {
    if delete_selection(state) {
        return true;
    }
    let len = state.value.chars().count();
    if state.cursor >= len {
        return false;
    }
    let byte_idx = char_to_byte(&state.value, state.cursor);
    let next = char_to_byte(&state.value, next_grapheme(&state.value, state.cursor));
    state.value.replace_range(byte_idx..next, "");
    true
}

fn move_left(state: &mut InputState, extend: bool) {
    if !extend {
        state.anchor = None;
    } else if state.anchor.is_none() {
        state.anchor = Some(state.cursor);
    }
    state.cursor = prev_grapheme(&state.value, state.cursor);
}

fn move_right(state: &mut InputState, extend: bool) {
    if !extend {
        state.anchor = None;
    } else if state.anchor.is_none() {
        state.anchor = Some(state.cursor);
    }
    state.cursor = next_grapheme(&state.value, state.cursor);
}

fn move_home(state: &mut InputState, extend: bool) {
    if !extend {
        state.anchor = None;
    } else if state.anchor.is_none() {
        state.anchor = Some(state.cursor);
    }
    state.cursor = 0;
}

fn move_end(state: &mut InputState, extend: bool) {
    if !extend {
        state.anchor = None;
    } else if state.anchor.is_none() {
        state.anchor = Some(state.cursor);
    }
    state.cursor = state.value.chars().count();
}

fn select_all(state: &mut InputState) {
    state.anchor = Some(0);
    state.cursor = state.value.chars().count();
}

/// The text a char range covers, which is what the clipboard gets.
///
/// Pure, so the tests can reach it: building an `InputState` needs an `App` for
/// its focus handle, and none of the motion logic touches that.
fn slice_selection(value: &str, selection: Option<(usize, usize)>) -> Option<String> {
    let (lo, hi) = selection?;
    let lo_b = char_to_byte(value, lo);
    let hi_b = char_to_byte(value, hi);
    Some(value[lo_b..hi_b].to_owned())
}

/// Starts the selection at the caret if `extend` and there is none yet, and
/// clears it otherwise. Every motion begins this way.
fn before_move(state: &mut InputState, extend: bool) {
    if !extend {
        state.anchor = None;
    } else if state.anchor.is_none() {
        state.anchor = Some(state.cursor);
    }
}

/// Whether a char counts as part of a word for `move_word`.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Where Ctrl+Left / Ctrl+Right lands: over any run of separators, then over
/// the word.
fn word_target(value: &str, cursor: usize, forward: bool) -> usize {
    let chars: Vec<char> = value.chars().collect();
    let mut i = cursor.min(chars.len());
    if forward {
        while i < chars.len() && !is_word(chars[i]) {
            i += 1;
        }
        while i < chars.len() && is_word(chars[i]) {
            i += 1;
        }
    } else {
        while i > 0 && !is_word(chars[i - 1]) {
            i -= 1;
        }
        while i > 0 && is_word(chars[i - 1]) {
            i -= 1;
        }
    }
    i
}

fn move_word(state: &mut InputState, forward: bool, extend: bool) {
    before_move(state, extend);
    state.cursor = word_target(&state.value, state.cursor, forward);
}

/// What the field draws: the value, or one bullet per char for a password.
///
/// The click maths runs on this rather than on the value, so a masked field maps
/// its own glyph widths.
fn displayed_value(state: &InputState, masks: bool) -> String {
    if masks {
        "\u{2022}".repeat(state.value.chars().count())
    } else {
        state.value.clone()
    }
}

/// Which char a pointer at `x` (relative to the text's left edge) is nearest.
///
/// A mouse listener is handed the pointer position and nothing else, so the
/// text's own origin has to be remembered from the previous frame -- see
/// `TEXT_ORIGIN` in `Input::render`. gpui shapes the line for us, and
/// `closest_index_for_x` answers in *bytes*, which the state counts in chars.
fn char_at_x(
    value: &str,
    x: Pixels,
    font: &gpui::Font,
    font_size: Pixels,
    window: &mut Window,
) -> usize {
    if value.is_empty() {
        return 0;
    }
    let run = gpui::TextRun {
        len: value.len(),
        font: font.clone(),
        // Shaping needs a colour and does not use it.
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window.text_system().shape_line(
        SharedString::from(value.to_owned()),
        font_size,
        &[run],
        None,
    );
    let byte = line.closest_index_for_x(x.max(px(0.)));
    value[..byte.min(value.len())].chars().count()
}

/// The char offset a click lands on inside a wrapped, multi-line body.
///
/// The paragraph is found by the bounds it was painted with, then gpui shapes
/// that paragraph at the width it was given and reports the closest character
/// boundary to the point -- `WrappedLine` derefs to the layout that answers it,
/// so a wrapped line does have a position after all. Returns `None` when no
/// paragraph has been laid out yet, which is any frame before the first paint.
fn char_at_point(
    value: &str,
    point: gpui::Point<Pixels>,
    paragraphs: &[gpui::Bounds<Pixels>],
    font: &gpui::Font,
    font_size: Pixels,
    line_height: Pixels,
    window: &mut Window,
) -> Option<usize> {
    let lines: Vec<&str> = value.split('\n').collect();
    let usable = paragraphs.len().min(lines.len());
    if usable == 0 {
        return None;
    }

    // The paragraph the click is inside, else the nearest one vertically: a
    // click in the padding below the last line belongs to the last line.
    let index = (0..usable)
        .find(|&i| {
            let b = paragraphs[i];
            point.y >= b.origin.y && point.y < b.origin.y + b.size.height.max(line_height)
        })
        .unwrap_or_else(|| {
            (0..usable)
                .min_by_key(|&i| {
                    let b = paragraphs[i];
                    let mid = b.origin.y + b.size.height.max(line_height) / 2.;
                    (f32::from(point.y - mid)).abs() as i64
                })
                .unwrap_or(0)
        });

    let offset: usize = lines[..index].iter().map(|l| l.chars().count() + 1).sum();
    let line = lines[index];
    if line.is_empty() {
        return Some(offset);
    }

    let bounds = paragraphs[index];
    let run = gpui::TextRun {
        len: line.len(),
        font: font.clone(),
        // Shaping needs a colour and does not use it.
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = window
        .text_system()
        .shape_text(
            SharedString::from(line.to_owned()),
            font_size,
            &[run],
            Some(bounds.size.width.max(px(1.))),
            None,
        )
        .ok()?;
    let wrapped = shaped.first()?;
    let local = gpui::point(point.x - bounds.origin.x, point.y - bounds.origin.y);
    let byte = match wrapped.closest_index_for_position(local, line_height) {
        Ok(byte) | Err(byte) => byte,
    };
    Some(offset + line[..byte.min(line.len())].chars().count())
}

/// The char indices bounding the line the caret is on, newlines excluded.
fn line_bounds(value: &str, cursor: usize) -> (usize, usize) {
    let chars: Vec<char> = value.chars().collect();
    let mut start = cursor.min(chars.len());
    while start > 0 && chars[start - 1] != '\n' {
        start -= 1;
    }
    let mut end = cursor.min(chars.len());
    while end < chars.len() && chars[end] != '\n' {
        end += 1;
    }
    (start, end)
}

/// Where Up / Down lands in a multiline field, keeping the column where it can.
///
/// The lines are the *logical* ones -- the ones `Enter` puts in. Up and Down
/// keeping to those rather than to visual rows is a deliberate difference from
/// the pointer, which lands on the visual row it was clicked on
/// (`char_at_point`); moving the keyboard caret to visual rows would need the
/// same shaping on every keystroke.
fn vertical_target(value: &str, cursor: usize, down: bool) -> usize {
    let (start, end) = line_bounds(value, cursor);
    let column = cursor - start;
    if down {
        let len = value.chars().count();
        if end >= len {
            return len;
        }
        let (next_start, next_end) = line_bounds(value, end + 1);
        (next_start + column).min(next_end)
    } else {
        if start == 0 {
            return 0;
        }
        let (prev_start, prev_end) = line_bounds(value, start - 1);
        (prev_start + column).min(prev_end)
    }
}

fn move_vertical(state: &mut InputState, down: bool, extend: bool) {
    before_move(state, extend);
    state.cursor = vertical_target(&state.value, state.cursor, down);
}

fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map_or(s.len(), |(b, _)| b)
}

/// The char index of a byte offset that sits on a char boundary.
fn byte_to_char(s: &str, byte: usize) -> usize {
    s[..byte.min(s.len())].chars().count()
}

/// A validation outcome for the current value.
///
/// v3 leaves validation to React Aria; here the caller reads this and passes
/// the result to `is_invalid` / `error_message`, so the rules stay declarative
/// without us owning a validation lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputValidity {
    /// The value satisfies every constraint.
    Valid,
    /// Shorter than `minLength`.
    TooShort,
    /// Below `min` (numeric types).
    BelowMin,
    /// Above `max` (numeric types).
    AboveMax,
    /// Not a multiple of `step` from `min`.
    OffStep,
    /// Does not satisfy `pattern`.
    PatternMismatch,
}

impl InputValidity {
    /// Whether this outcome is `Valid`.
    pub fn is_valid(self) -> bool {
        matches!(self, InputValidity::Valid)
    }
}

/// The `type` attribute. Only the variants that change rendering or input
/// handling in gpui are modelled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputType {
    /// Plain text.
    #[default]
    Text,
    /// Masks the value with bullets.
    Password,
    /// Email address; reports the email input role.
    Email,
    /// Restricts typing to digits, `-` and `.`.
    Number,
    /// Telephone number; reports the telephone input role.
    Tel,
    /// URL; reports the URL input role.
    Url,
    /// Search text; reports the search input role.
    Search,
}

impl InputType {
    /// The role a native `<input>` of this type reports.
    ///
    /// `useTextField` passes `type` straight through to the DOM input
    /// (`inputOnlyProps = { type, pattern }`), so the role is whatever the
    /// platform maps that input type to. AccessKit's role enum is modelled on
    /// exactly those input types, so the mapping is one-to-one.
    pub(crate) fn a11y_role(self) -> crate::a11y::Role {
        match self {
            InputType::Text => crate::a11y::Role::TextInput,
            InputType::Password => crate::a11y::Role::PasswordInput,
            InputType::Email => crate::a11y::Role::EmailInput,
            InputType::Number => crate::a11y::Role::NumberInput,
            InputType::Tel => crate::a11y::Role::PhoneNumberInput,
            InputType::Url => crate::a11y::Role::UrlInput,
            InputType::Search => crate::a11y::Role::SearchInput,
        }
    }

    /// Every input type, in declaration order.
    pub const ALL: [InputType; 7] = [
        InputType::Text,
        InputType::Password,
        InputType::Email,
        InputType::Number,
        InputType::Tel,
        InputType::Url,
        InputType::Search,
    ];

    /// The type's display name.
    pub fn label(self) -> &'static str {
        match self {
            InputType::Text => "Text",
            InputType::Password => "Password",
            InputType::Email => "Email",
            InputType::Number => "Number",
            InputType::Tel => "Tel",
            InputType::Url => "Url",
            InputType::Search => "Search",
        }
    }

    fn masks(self) -> bool {
        matches!(self, InputType::Password)
    }

    /// Whether `ch` may be typed into a field of this type.
    fn accepts(self, ch: char) -> bool {
        match self {
            InputType::Number => ch.is_ascii_digit() || ch == '-' || ch == '.',
            _ => true,
        }
    }
}

type TextCallback = std::sync::Arc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type ClearCallback = std::sync::Arc<dyn Fn(&mut Window, &mut App) + 'static>;

fn char_to_utf16(value: &str, offset: usize) -> usize {
    value.chars().take(offset).map(char::len_utf16).sum()
}

fn utf16_to_char(value: &str, offset: usize) -> usize {
    let mut units = 0;
    value
        .chars()
        .take_while(|ch| {
            units += ch.len_utf16();
            units <= offset
        })
        .count()
}

struct PlatformTextInput {
    state: Entity<InputState>,
    on_edit: TextCallback,
    input_type: InputType,
    max_length: Option<usize>,
    multiline: bool,
    bounds: gpui::Bounds<Pixels>,
    font: gpui::Font,
    paragraphs: Entity<Vec<gpui::Bounds<Pixels>>>,
}

impl PlatformTextInput {
    fn replace(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        text: &str,
        marked_selection: Option<Option<std::ops::Range<usize>>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let changed = self.state.update(cx, |state, cx| {
            if state.is_read_only || !state.is_successful {
                return false;
            }
            let range = range
                .map(|range| {
                    utf16_to_char(&state.value, range.start)..utf16_to_char(&state.value, range.end)
                })
                .or_else(|| state.marked.clone())
                .unwrap_or_else(|| {
                    let (start, end) = state.selection().unwrap_or((state.cursor, state.cursor));
                    start..end
                });
            let old_value = state.value.clone();
            let old_cursor = state.cursor;
            let old_anchor = state.anchor;
            state.anchor = Some(range.start);
            state.cursor = range.end;
            let mut inserted = false;
            for ch in text.chars() {
                if (ch == '\n' && self.multiline || !ch.is_control())
                    && state_accepts(state, ch, self.input_type, self.max_length)
                {
                    insert_char(state, ch);
                    inserted = true;
                }
            }
            if text.is_empty() {
                delete_selection(state);
            } else if !inserted {
                state.cursor = old_cursor;
                state.anchor = old_anchor;
                return false;
            }
            let end = state.cursor;
            state.anchor = None;
            state.marked = marked_selection
                .as_ref()
                .and_then(|_| (end > range.start).then_some(range.start..end));
            if let Some(Some(selection)) = marked_selection {
                let inserted: String = state
                    .value
                    .chars()
                    .skip(range.start)
                    .take(end - range.start)
                    .collect();
                state.anchor = Some(range.start + utf16_to_char(&inserted, selection.start));
                state.cursor = range.start + utf16_to_char(&inserted, selection.end);
            }
            cx.notify();
            state.value != old_value
        });
        if changed {
            let value = self.state.read(cx).value.clone();
            (self.on_edit)(&value, window, cx);
        }
    }
}

impl gpui::InputHandler for PlatformTextInput {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<gpui::UTF16Selection> {
        let state = self.state.read(cx);
        let (start, end) = state.selection().unwrap_or((state.cursor, state.cursor));
        Some(gpui::UTF16Selection {
            range: char_to_utf16(&state.value, start)..char_to_utf16(&state.value, end),
            reversed: state.anchor.is_some_and(|anchor| anchor > state.cursor),
        })
    }

    fn marked_text_range(
        &mut self,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<std::ops::Range<usize>> {
        let state = self.state.read(cx);
        state.marked.as_ref().map(|range| {
            char_to_utf16(&state.value, range.start)..char_to_utf16(&state.value, range.end)
        })
    }

    fn text_for_range(
        &mut self,
        range: std::ops::Range<usize>,
        actual: &mut Option<std::ops::Range<usize>>,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        let value = &self.state.read(cx).value;
        let start = utf16_to_char(value, range.start);
        let end = utf16_to_char(value, range.end);
        *actual = Some(char_to_utf16(value, start)..char_to_utf16(value, end));
        Some(
            value
                .chars()
                .skip(start)
                .take(end.saturating_sub(start))
                .collect(),
        )
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.replace(range, text, None, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        text: &str,
        selection: Option<std::ops::Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.replace(range, text, Some(selection), window, cx);
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            state.marked = None;
            cx.notify();
        });
    }

    fn set_selected_text_range(
        &mut self,
        range: std::ops::Range<usize>,
        _: &mut Window,
        cx: &mut App,
    ) {
        self.state.update(cx, |state, cx| {
            state.anchor = Some(utf16_to_char(&state.value, range.start));
            state.cursor = utf16_to_char(&state.value, range.end);
            cx.notify();
        });
    }

    fn text_length_utf16(&mut self, _: &mut Window, cx: &mut App) -> Option<usize> {
        Some(self.state.read(cx).value.encode_utf16().count())
    }

    fn accepts_text_input(&mut self, _: &mut Window, cx: &mut App) -> bool {
        let state = self.state.read(cx);
        !state.is_read_only && state.is_successful
    }

    fn prefers_ime_for_printable_keys(&mut self, _: &mut Window, _: &mut App) -> bool {
        true
    }

    fn element_bounds(&mut self, _: &mut Window, _: &mut App) -> Option<gpui::Bounds<Pixels>> {
        Some(self.bounds)
    }

    fn bounds_for_range(
        &mut self,
        range: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<gpui::Bounds<Pixels>> {
        let state = self.state.read(cx);
        let start = utf16_to_char(&state.value, range.start);
        let end = utf16_to_char(&state.value, range.end);
        let shown = displayed_value(state, self.input_type.masks());
        let (line, offset, bounds) = if self.multiline {
            let prefix: String = shown.chars().take(start).collect();
            let paragraph = prefix.chars().filter(|&ch| ch == '\n').count();
            let offset = prefix.rsplit('\n').next()?.chars().count();
            (
                shown.split('\n').nth(paragraph)?,
                offset,
                *self.paragraphs.read(cx).get(paragraph)?,
            )
        } else {
            (shown.as_str(), start, self.bounds)
        };
        let size = crate::util::FIELD_TEXT;
        let height = px(20.);
        let run = gpui::TextRun {
            len: line.len(),
            font: self.font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = window
            .text_system()
            .shape_text(
                line.to_owned().into(),
                size,
                &[run],
                self.multiline.then_some(bounds.size.width.max(px(1.))),
                None,
            )
            .ok()?;
        let shaped = shaped.first()?;
        let left = shaped.position_for_index(char_to_byte(line, offset), height)?;
        let right = shaped
            .position_for_index(
                char_to_byte(line, offset + end.saturating_sub(start)),
                height,
            )
            .unwrap_or(left);
        let y = if self.multiline {
            bounds.top()
        } else {
            bounds.top() + (bounds.size.height - height) / 2.
        };
        Some(gpui::Bounds::new(
            gpui::point(bounds.left() + left.x, y + left.y),
            gpui::size(
                if left.y == right.y {
                    (right.x - left.x).max(px(1.))
                } else {
                    px(1.)
                },
                height,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize> {
        let state = self.state.read(cx);
        let shown = displayed_value(state, self.input_type.masks());
        let offset = if self.multiline {
            char_at_point(
                &shown,
                point,
                self.paragraphs.read(cx),
                &self.font,
                crate::util::FIELD_TEXT,
                px(20.),
                window,
            )?
        } else {
            char_at_x(
                &shown,
                point.x - self.bounds.left(),
                &self.font,
                crate::util::FIELD_TEXT,
                window,
            )
        };
        Some(char_to_utf16(&state.value, offset))
    }
}

/// The character Enter inserts in a multi-line field.
const NEWLINE: char = '\n';

/// Everything the multi-line body needs to draw itself.
struct MultilineBody<'a> {
    value: &'a str,
    cursor: usize,
    selection: Option<(usize, usize)>,
    focused: bool,
    /// The caret's element id, height and colour.
    caret: (gpui::ElementId, Pixels, gpui::Hsla),
    selection_bg: gpui::Hsla,
    /// Where each paragraph was painted, filled in during layout so a click can
    /// be measured against the text it actually landed on.
    paragraphs: Entity<Vec<gpui::Bounds<Pixels>>>,
}

/// One paragraph per newline, each wrapping, with the caret and selection
/// placed inside the paragraph they fall in.
///
/// gpui does wrap text — the default `WhiteSpace::Normal` — so a real
/// multi-line surface only needed the newlines split out and the caret located
/// within them. The single-line field opts out with `whitespace_nowrap`.
fn multiline_body(b: MultilineBody<'_>, cx: &App) -> gpui::AnyElement {
    let (caret_id, caret_h, caret_color) = b.caret;
    let mut col = gpui::div().flex().flex_col().items_start().w_full();
    // Char offset each line starts at, so the cursor and selection — which are
    // offsets into the whole value — can be mapped into it.
    let mut start = 0usize;
    let lines: Vec<&str> = b.value.split('\n').collect();
    let last = lines.len().saturating_sub(1);
    for (i, line) in lines.iter().enumerate() {
        let len = line.chars().count();
        let end = start + len;
        // `min_w_0` on the text spans is what lets each wrap inside the field
        // instead of pushing the row wider than its box.
        let mut para = gpui::div()
            .flex()
            .flex_wrap()
            .items_center()
            .w_full()
            .min_w_0();

        // A zero-size probe at the head of the paragraph: `canvas` is the only
        // element told its own bounds, and a click in a wrapped paragraph has
        // to be measured against where that paragraph was actually laid out.
        para = para.child(
            gpui::canvas(
                {
                    let sink = b.paragraphs.clone();
                    move |bounds: gpui::Bounds<Pixels>, _window, cx| {
                        sink.update(cx, |slots, _| {
                            if slots.len() <= i {
                                slots.resize(i + 1, gpui::Bounds::default());
                            }
                            slots[i] = bounds;
                        });
                        bounds
                    }
                },
                |_, _, _, _| {},
            )
            .w_full()
            .h(px(0.))
            .flex_shrink_0(),
        );

        // The selection, clipped to this line.
        let local_sel = b.selection.and_then(|(lo, hi)| {
            let lo = lo.max(start);
            let hi = hi.min(end);
            (lo < hi).then_some((lo - start, hi - start))
        });

        if let Some((lo, hi)) = local_sel {
            let before: String = line.chars().take(lo).collect();
            let selected: String = line.chars().skip(lo).take(hi - lo).collect();
            let after: String = line.chars().skip(hi).collect();
            para = para
                .child(gpui::div().min_w_0().child(before))
                .child(
                    gpui::div()
                        .min_w_0()
                        .px(px(1.))
                        .rounded(px(4.))
                        .bg(b.selection_bg)
                        .child(selected),
                )
                .child(gpui::div().min_w_0().child(after));
        } else if b.selection.is_none() && b.cursor >= start && b.cursor <= end {
            let at = b.cursor - start;
            let before: String = line.chars().take(at).collect();
            let after: String = line.chars().skip(at).collect();
            para = para.child(gpui::div().min_w_0().child(before));
            if b.focused {
                para = para.child(crate::anim::caret_blink(
                    gpui::div()
                        .w(px(1.5))
                        .h(caret_h)
                        .bg(caret_color)
                        .flex_shrink_0(),
                    caret_id.clone(),
                    cx,
                ));
            }
            para = para.child(gpui::div().min_w_0().child(after));
        } else {
            para = para.child(gpui::div().w_full().min_w_0().child(line.to_string()));
        }

        col = col.child(para);
        // +1 for the newline the split consumed, except after the last line.
        start = end + usize::from(i < last);
    }
    col.into_any_element()
}

struct InputFieldRenderState {
    focus: crate::util::FieldFocus,
    is_disabled: bool,
    is_invalid: bool,
    is_read_only: bool,
    is_required: bool,
    value: SharedString,
}

/// HeroUI Input.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Input {
    /// See [`Input::content`]: v3's field children-as-a-function.
    content: Option<std::sync::Arc<dyn Fn(crate::util::FieldFocus) -> gpui::AnyElement + 'static>>,
    field_content:
        Option<std::sync::Arc<dyn Fn(InputFieldRenderState) -> gpui::AnyElement + 'static>>,
    /// `validationBehavior` — written into the state on render.
    validation_behavior: Option<crate::form::ValidationBehavior>,
    state: Entity<InputState>,
    label: Option<SharedString>,
    /// An accessible name with no visible label; see [`Input::a11y_label`].
    a11y_label: Option<SharedString>,
    placeholder: Option<SharedString>,
    description: Option<SharedString>,
    error_message: Option<SharedString>,
    /// Crate-internal: a compound field (NumberField, the pickers' inner
    /// field) that owns its own commit-gated validity hands the result here
    /// so this input paints that snapshot instead of composing its own.
    /// `None` — every standalone input — keeps the computed display.
    display_override: Option<crate::validation::Validity>,
    /// `validate` — run by the component, not the caller.
    validate: Option<crate::validation::Validator<str>>,
    /// `validationErrors` — messages from a server round-trip.
    validation_errors: Vec<SharedString>,
    variant: FieldVariant,
    variant_is_set: bool,
    input_type: InputType,
    max_length: Option<usize>,
    min_length: Option<usize>,
    /// `min` / `max` / `step` for the numeric types.
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    /// `pattern` — a predicate over the whole value.
    pattern: Option<std::sync::Arc<dyn Fn(&str) -> bool + 'static>>,
    start_content: Option<gpui::AnyElement>,
    end_content: Option<gpui::AnyElement>,
    /// Stretch beyond the 320px default demo width.
    /// Multi-line only: the height `rows` asks for. `None` leaves v3's
    /// `min-height: 38px`.
    min_h: Option<Pixels>,
    /// [`Input::height`] — the single-line box height. `None` keeps
    /// `util::FIELD_HEIGHT`; the multi-line path ignores it (see the builder).
    height: Option<Pixels>,
    /// [`Input::padding_x`] — the standalone box's horizontal padding. `None`
    /// keeps v3's `px-3`. Ignored inside a group, whose addon rules own the
    /// sides.
    padding_x: Option<Pixels>,
    /// The group owner's padding override (`InputGroup::padding_x`,
    /// `NumberField::padding_x`). Crate-internal: it is not the public
    /// `Input::padding_x`, whose grouped behavior stays as documented.
    group_padding_x: Option<Pixels>,
    /// [`Input::is_bare`] — render the standalone box with no chrome at all,
    /// the way `InputGroup.Input` already does.
    is_bare: bool,
    is_bare_is_set: bool,
    /// Whether focused state paints the visual focus ring. `None` lets the
    /// active `TextFieldStyle` choose; unset everywhere keeps the stock ring.
    focus_ring: Option<bool>,
    /// [`Input::text_size`] — the field value and placeholder type size.
    text_size: Option<Pixels>,
    /// [`Input::font_family`] — the family the field text is drawn and
    /// measured with. `None` inherits the window's text style.
    font_family: Option<SharedString>,
    /// The corner radius, in place of the owning `field_radius` helper.
    radius: Option<Pixels>,
    /// Set by [`crate::input_group::InputGroup`]: `(has_prefix, has_suffix)`.
    /// `InputGroup.Input` has no chrome of its own -- the group paints it -- and
    /// drops the padding on whichever side touches an addon (`ps-0`/`pe-0`).
    in_group: Option<(bool, bool)>,
    /// Set by [`crate::input_group::InputGroup`]: the group box paints the one
    /// `status-disabled` dim over the whole row, so the field must not nest a
    /// second opacity inside it.
    group_dim: bool,
    full_width: bool,
    is_disabled: bool,
    is_read_only: bool,
    is_required: bool,
    is_invalid: bool,
    /// `autoFocus` — take focus on the first render.
    auto_focus: bool,
    /// `name` — the submission name, written into the state on render.
    name: Option<SharedString>,
    /// Set by `TextArea`: wrap the text, lay lines out top-down, and let Enter
    /// insert a newline instead of submitting.
    multiline: bool,
    /// `defaultValue` — seeds the state on the first render only.
    default_value: Option<SharedString>,
    /// `value` — the controlled spelling; seeds the state on the first render
    /// only, ahead of `default_value`. See [`Input::value`].
    value: Option<SharedString>,
    is_clearable: bool,
    clear_content: Option<gpui::AnyElement>,
    /// The fill the clear button takes on hover, in place of
    /// `--default-hover`.
    clear_hover_bg: Option<gpui::Hsla>,
    /// SearchField-only: Escape clears a non-empty query.
    clear_on_escape: bool,
    /// SearchField-only: the clear affordance and Escape report this action.
    on_clear: Option<ClearCallback>,
    on_change: Option<TextCallback>,
    on_submit: Option<TextCallback>,
    /// ComboBox popup anchor: when set, the field row (not the
    /// label-to-error wrapper) records its bounds into `anchor` and carries
    /// `selector` for headless probes — the way RAC's `triggerRef` reads
    /// `groupRef.current || inputRef.current` (pinned 1.20.0
    /// `dist/private/ComboBox.js`: "Position popover relative to group if
    /// available, otherwise input").
    field_anchor: Option<(
        std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<Pixels>>>>,
        SharedString,
    )>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
    recipes: Vec<SharedString>,
}

impl Input {
    /// The bound state, so wrappers can write through to it.
    pub fn state(&self) -> &Entity<InputState> {
        &self.state
    }

    /// The builder's `is_disabled` flag, read without a rendered frame.
    ///
    /// `InputGroup` needs it before [`Input::render`] has run: the state's
    /// success mirror is last frame's trace, and the first frame has none, so
    /// a field disabled while its group is enabled would take an addon click's
    /// focus once before the mirror caught up.
    pub(crate) fn builder_is_disabled(&self) -> bool {
        self.is_disabled
    }

    /// v3's field `children`-as-a-function, handed `{isFocused, isFocusWithin,
    /// isFocusVisible}`.
    ///
    /// v3's caller writes the parts themselves inside that function -- a
    /// `Label`, the group, a `Description` -- and this port exposes the same
    /// three as components, so a closure here replaces the field's own stack
    /// with whatever the caller builds from the state.
    pub fn content(
        mut self,
        render: impl Fn(crate::util::FieldFocus) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.content = Some(std::sync::Arc::new(render));
        self
    }

    fn field_content(
        mut self,
        render: impl Fn(InputFieldRenderState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.field_content = Some(std::sync::Arc::new(render));
        self
    }

    /// `value` — v3's controlled-value spelling, as a pure builder.
    ///
    /// The bound [`InputState`] owns the text once the field renders, so this
    /// seeds the state on the first render only, winning over
    /// [`Input::default_value`] the way v3's controlled prop outranks the
    /// uncontrolled seed; calling `.value(..)` twice keeps the last call, like
    /// every other builder here. A later value is an imperative update rather
    /// than a builder: `state.update(cx, |s, _| s.set_value(..))`.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Builds the field over `state`.
    ///
    /// The handle may be borrowed — `Input::new(&handle)` — so a caller that
    /// keeps its own handle clones nothing at the call site; an owned
    /// `Entity<InputState>` still works, and the cheap handle clone happens
    /// once inside either way.
    pub fn new(state: impl std::borrow::Borrow<Entity<InputState>>) -> Self {
        Self {
            content: None,
            field_content: None,
            validation_behavior: None,
            state: state.borrow().clone(),
            label: None,
            a11y_label: None,
            placeholder: None,
            description: None,
            error_message: None,
            display_override: None,
            validate: None,
            validation_errors: Vec::new(),
            variant: FieldVariant::Primary,
            variant_is_set: false,
            input_type: InputType::Text,
            max_length: None,
            min_length: None,
            min: None,
            max: None,
            step: None,
            pattern: None,
            start_content: None,
            end_content: None,
            min_h: None,
            height: None,
            padding_x: None,
            group_padding_x: None,
            is_bare: false,
            is_bare_is_set: false,
            focus_ring: None,
            text_size: None,
            font_family: None,
            radius: None,
            in_group: None,
            group_dim: false,
            full_width: false,
            is_disabled: false,
            is_read_only: false,
            is_required: false,
            is_invalid: false,
            auto_focus: false,
            name: None,
            default_value: None,
            value: None,
            multiline: false,
            is_clearable: false,
            clear_content: None,
            clear_hover_bg: None,
            clear_on_escape: false,
            on_clear: None,
            on_change: None,
            on_submit: None,
            field_anchor: None,
            sx: None,
            recipes: Vec::new(),
        }
    }

    /// `validationBehavior` — `Allow` shows the message without blocking form
    /// submission.
    ///
    /// Stored on the state beside `name`, because the form reads both from
    /// there.
    pub fn validation_behavior(mut self, behavior: crate::form::ValidationBehavior) -> Self {
        self.validation_behavior = Some(behavior);
        self
    }

    /// Turns this into the multi-line surface `TextArea` renders.
    ///
    /// Not a v3 prop: v3 has a separate `<textarea>`, and this is the flag that
    /// switches one implementation between the two.
    pub(crate) fn multiline(mut self, v: bool) -> Self {
        self.multiline = v;
        self
    }

    /// Multi-line only: the height `TextArea::rows` asks for.
    pub(crate) fn min_h(mut self, h: Pixels) -> Self {
        self.min_h = Some(h);
        self
    }

    /// Renders as v3's `InputGroup.Input`: transparent, unrounded, unshadowed,
    /// and flush against whichever addons surround it.
    pub(crate) fn in_group(mut self, has_prefix: bool, has_suffix: bool) -> Self {
        self.in_group = Some((has_prefix, has_suffix));
        self
    }

    /// The surrounding group box already carries the disabled dim; skip this
    /// field's own so the opacity does not nest twice.
    pub(crate) fn group_dim(mut self, v: bool) -> Self {
        self.group_dim = v;
        self
    }

    /// ComboBox popup anchor: records the 36px field row's bounds (not the
    /// label-to-error wrapper) and tags that row with `selector` for headless
    /// probes — the way RAC's `triggerRef` reads `groupRef.current ||
    /// inputRef.current`.
    pub(crate) fn field_anchor(
        mut self,
        anchor: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<Pixels>>>>,
        selector: impl Into<SharedString>,
    ) -> Self {
        self.field_anchor = Some((anchor, selector.into()));
        self
    }

    /// `name` — the name this field submits under.
    ///
    /// Stored on the [`InputState`], because gpui gives a child no way to reach
    /// its `Form`; `FormField::text(state)` reads it back out, so the name is
    /// written once here and not repeated at the form.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// `defaultValue` — the uncontrolled initial text.
    ///
    /// Written into the state on the first render only, so it seeds the field
    /// without overwriting what the user types. `InputState::with_value` does
    /// the same at construction; this is the prop spelling, for a state the
    /// caller made without one.
    pub fn default_value(mut self, text: impl Into<SharedString>) -> Self {
        self.default_value = Some(text.into());
        self
    }

    /// Sets the visible label.
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }

    /// The accessible name for a field whose visible label belongs to a
    /// composite owner.
    ///
    /// `NumberField` renders `field::Label` itself, as v3 composes it as a
    /// sibling of `NumberField.Group`, so handing the inner `Input` the label
    /// through [`Self::label`] would draw it twice. Upstream still passes
    /// `label` down to `useFormattedTextField`, which is where the input's
    /// accessible name comes from, so this carries the name without the box.
    pub(crate) fn a11y_label(mut self, l: impl Into<SharedString>) -> Self {
        self.a11y_label = Some(l.into());
        self
    }

    /// `aria-label` — the explicit accessible name, which wins over the
    /// label-derived name exactly as the HTML name computation ranks it.
    pub fn aria_label(mut self, l: impl Into<SharedString>) -> Self {
        self.a11y_label = Some(l.into());
        self
    }

    /// Sets the placeholder shown while the value is empty.
    pub fn placeholder(mut self, p: impl Into<SharedString>) -> Self {
        self.placeholder = Some(p.into());
        self
    }

    /// Sets the description text.
    pub fn description(mut self, d: impl Into<SharedString>) -> Self {
        self.description = Some(d.into());
        self
    }

    /// `autoFocus` — take focus on the first render.
    pub fn auto_focus(mut self, v: bool) -> Self {
        self.auto_focus = v;
        self
    }

    /// `validate` — returns the message to show, or `None` when the text is fine.
    ///
    /// The component runs it and surfaces the result, so a caller does not have
    /// to mirror the logic into `is_invalid` / `error_message`.
    pub fn validate(mut self, f: impl Fn(&str) -> Option<SharedString> + 'static) -> Self {
        self.validate = Some(std::sync::Arc::new(f));
        self
    }

    /// `validationErrors` — messages produced elsewhere, shown ahead of
    /// whatever `validate` returns.
    pub fn validation_errors(
        mut self,
        errors: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.validation_errors = errors.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the error message shown when the field is invalid.
    pub fn error_message(mut self, e: impl Into<SharedString>) -> Self {
        self.error_message = Some(e.into());
        self
    }

    /// The `type` attribute — `password` masks, `number` filters keystrokes.
    pub fn input_type(mut self, input_type: InputType) -> Self {
        self.input_type = input_type;
        self
    }

    /// `maxLength` — refuses keystrokes past this many characters.
    pub fn max_length(mut self, n: usize) -> Self {
        self.max_length = Some(n);
        self
    }

    /// `minLength` — reported by [`Input::validity`], not enforced while typing.
    pub fn min_length(mut self, n: usize) -> Self {
        self.min_length = Some(n);
        self
    }

    /// `min` — the lowest accepted value for the numeric types.
    pub fn min(mut self, v: f64) -> Self {
        self.min = Some(v);
        self
    }

    /// `max` — the highest accepted value for the numeric types.
    pub fn max(mut self, v: f64) -> Self {
        self.max = Some(v);
        self
    }

    /// `step` — the numeric granularity, measured from `min` (or 0).
    pub fn step(mut self, v: f64) -> Self {
        self.step = Some(v);
        self
    }

    /// `pattern` — a predicate the whole value must satisfy.
    pub fn pattern(mut self, f: impl Fn(&str) -> bool + 'static) -> Self {
        self.pattern = Some(std::sync::Arc::new(f));
        self
    }

    /// Checks `value` against this field's constraints.
    ///
    /// Callers own their validation lifecycle, so this is a pure query — pass
    /// the outcome back through `is_invalid` / `error_message`.
    pub fn validity(&self, value: &str) -> InputValidity {
        validate_value(
            value,
            self.input_type,
            self.min_length,
            self.min,
            self.max,
            self.step,
            self.pattern.as_deref(),
        )
    }

    /// Sets the field variant.
    pub fn variant(mut self, v: FieldVariant) -> Self {
        self.variant = v;
        self.variant_is_set = true;
        self
    }

    /// Named theme overlay from [`herogpui_theme::ComponentThemes::text_field`].
    /// Stackable; a missing name adds no override.
    pub fn recipe(mut self, name: impl Into<SharedString>) -> Self {
        self.recipes.push(name.into());
        self
    }

    pub(crate) fn with_recipes(mut self, recipes: Vec<SharedString>) -> Self {
        self.recipes = recipes;
        self
    }

    /// The height of the single-line field box, replacing v3's 36px
    /// (`util::FIELD_HEIGHT`).
    ///
    /// Only the row height changes: the text keeps its 14px size and 20px
    /// line height and stays vertically centred, so a shorter box is a
    /// tighter box rather than smaller type. The multi-line surface
    /// (`TextArea`) ignores this — its height is content-driven with a
    /// `rows`-derived floor, and a fixed height there would either clip the
    /// text or fight `rows`; use `TextArea::rows` for that.
    pub fn height(mut self, h: impl Into<Pixels>) -> Self {
        self.height = Some(h.into());
        self
    }

    /// The horizontal padding of the standalone field box, replacing v3's
    /// `px-3` (12px).
    ///
    /// Inside an [`crate::input_group::InputGroup`] the padding is the group's
    /// rule — the side that touches an addon carries none — so this is ignored
    /// there.
    pub fn padding_x(mut self, p: impl Into<Pixels>) -> Self {
        self.padding_x = Some(p.into());
        self
    }

    /// The group owner's padding override, applied to every side without an
    /// addon. Crate-internal: [`Self::padding_x`] is the public, standalone
    /// spelling, and it remains ignored inside a group.
    pub(crate) fn group_padding_x(mut self, padding_x: impl Into<Pixels>) -> Self {
        self.group_padding_x = Some(padding_x.into());
        self
    }

    /// A compound field's already commit-gated validity, painted in place of
    /// the display this input would compose for itself. The live `validity`
    /// and `pending` writes are untouched, so a `Form`'s blocked decision
    /// still reads the real value.
    pub(crate) fn display_override(mut self, validity: crate::validation::Validity) -> Self {
        self.display_override = Some(validity);
        self
    }

    /// Applies a standalone [`crate::util::FieldBox`] to this field: explicit
    /// height and padding, plus the chrome-less bare mode. Crate-internal for
    /// the components that forward their box seam to a held `Input`.
    /// Hands a wrapper's captured `sx` refinement to this field, so it lands on
    /// the same root [`Input::sx`] would (a `ColorField` composing this field
    /// has no element of its own to refine).
    pub(crate) fn with_sx_refinement(mut self, sx: Option<Box<gpui::StyleRefinement>>) -> Self {
        if sx.is_some() {
            self.sx = sx;
        }
        self
    }

    pub(crate) fn with_field_box(mut self, field: crate::util::FieldBox) -> Self {
        if let Some(height) = field.height {
            self = self.height(height);
        }
        if let Some(padding_x) = field.padding_x {
            self = self.padding_x(padding_x);
        }
        if field.is_bare_is_set {
            self = self.is_bare(field.is_bare);
        }
        if let Some(focus_ring) = field.focus_ring {
            self = self.focus_ring(focus_ring);
        }
        self
    }

    /// Drops the field's own chrome: no background, no border, no field
    /// shadow, and no focus or invalid ring.
    ///
    /// This is exactly the treatment `InputGroup.Input` already gets (v3's
    /// `.input-group__input` is `border-0 bg-transparent shadow-none`), for a
    /// field the caller paints around — a toolbar, a table cell, an editable
    /// label. Everything else, including focus itself and the invalid state
    /// the error line reports, is unchanged.
    pub fn is_bare(mut self, v: bool) -> Self {
        self.is_bare = v;
        self.is_bare_is_set = true;
        self
    }

    /// Shows or hides only the field's visual focus ring.
    ///
    /// `false` keeps the control focusable and editable and keeps invalid
    /// feedback, but suppresses the accent ring painted while it is focused.
    /// Use [`Input::is_bare`] when the surrounding caller should own all field
    /// chrome instead. The default is `true`, or the value supplied by the
    /// active [`herogpui_theme::TextFieldStyle`].
    pub fn focus_ring(mut self, v: bool) -> Self {
        self.focus_ring = Some(v);
        self
    }

    /// The field value and placeholder type size, replacing v3's `text-sm`.
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// The font family the field's value, placeholder and caret measurement
    /// use — a monospace face for a code or token field.
    ///
    /// The field's own text layout captures the font during `render`, before
    /// any element's text style is pushed onto the window's stack, so the
    /// family is threaded into that captured font here as well as set on the
    /// box for the text gpui itself shapes.
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// The corner radius, in place of the owning `field_radius` helper. Not a
    /// v3 prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    ///
    /// The box and shared field chrome use the same resolved radius. Bare
    /// and grouped fields retain it without painting their own chrome.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// Sets the content rendered before the text.
    pub fn start_content(mut self, el: impl IntoElement) -> Self {
        self.start_content = Some(el.into_any_element());
        self
    }

    /// Sets the content rendered after the text.
    pub fn end_content(mut self, el: impl IntoElement) -> Self {
        self.end_content = Some(el.into_any_element());
        self
    }

    /// Fills the parent instead of the 320px default demo width.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// Sets whether the field is disabled.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets whether the field is read-only.
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.is_read_only = v;
        self
    }

    /// Sets whether the field is required.
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// Sets whether the field is invalid.
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// The fill the clear button takes on hover, in place of
    /// `--default-hover`.
    pub fn clear_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.clear_hover_bg = Some(color.into());
        self
    }

    /// Shows a clear button when there is a value.
    pub fn is_clearable(mut self, v: bool) -> Self {
        self.is_clearable = v;
        self
    }

    fn clear_content(mut self, content: impl IntoElement) -> Self {
        self.clear_content = Some(content.into_any_element());
        self
    }

    /// SearchField's Escape shortcut, kept internal because plain Input has no
    /// clear-on-Escape contract.
    pub(crate) fn clear_on_escape(mut self, v: bool) -> Self {
        self.clear_on_escape = v;
        self
    }

    /// SearchField's dedicated clear action, separate from an edit that merely
    /// produces an empty value.
    pub(crate) fn on_clear(mut self, f: ClearCallback) -> Self {
        self.on_clear = Some(f);
        self
    }

    /// Sets the handler called with the new value whenever it changes.
    pub fn on_change(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(std::sync::Arc::new(f));
        self
    }

    /// Sets the handler called with the current value when Enter is pressed.
    pub fn on_submit(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_submit = Some(std::sync::Arc::new(f));
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the input's root element after every value the variant and
    /// the active theme chose, so they win. The root is the label-to-error
    /// column a standalone field returns; inside an `InputGroup` it is the
    /// field row itself, which is all the group leaves the field.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Input {
    /// The focus handle of the state this field is bound to.
    ///
    /// `InputGroup` rings on `focus-within`, and the only thing inside it that
    /// can hold a focus is this field.
    pub(crate) fn state_focus(&self, cx: &App) -> FocusHandle {
        self.state.read(cx).focus_handle.clone()
    }

    /// The captured [`Input::sx`] refinement, handed through by wrappers that
    /// hold their `sx` apart from the field — [`SearchField`] builds its
    /// `Input` at render time, so its slot travels as the refinement itself.
    pub(crate) fn sx_refinement(mut self, sx: Option<Box<gpui::StyleRefinement>>) -> Self {
        self.sx = sx;
        self
    }
}

impl RenderOnce for Input {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // One structured id per rendered field, and every part of the field
        // hangs off it. The state entity is what makes it unique, and
        // `named_usize` keeps it as structure, so a part's name can never
        // fold into the base the way a formatted `"input-{n}-part"` string
        // can.
        let base_id =
            gpui::ElementId::named_usize("input", self.state.entity_id().as_u64() as usize);
        // `validationBehavior` travels with the name, on the state.
        if let Some(behavior) = self.validation_behavior {
            let state = self.state.clone();
            if state.read(cx).validation_behavior() != behavior {
                state.update(cx, |s, _| s.set_validation_behavior(behavior));
            }
        }
        // `focus_once` takes `cx` mutably, so it has to run before the theme
        // tokens are borrowed.
        // `value` / `defaultValue` seed the state once, before anything reads
        // it. `value` is v3's controlled spelling, so it outranks the
        // uncontrolled seed; the state owns the text afterwards, and
        // `InputState::set_value` is the imperative update.
        let seed = self.value.clone().or(self.default_value.clone());
        if let Some(text) = seed {
            let state = self.state.clone();
            crate::util::seed_once(
                window,
                cx,
                element_id::scoped(&base_id, "default"),
                move |cx| {
                    state.update(cx, |s, cx| {
                        s.set_value(text.to_string());
                        cx.notify();
                    });
                },
            );
        }
        // `name` lives on the state so the form can find it. Only write when
        // it differs, so this does not loop through `notify`.
        if self.state.read(cx).name() != self.name {
            let name = self.name.clone();
            self.state.update(cx, |s, _| s.set_name(name));
        }
        let is_successful = !self.is_disabled;
        if self.state.read(cx).is_successful() != is_successful {
            self.state
                .update(cx, |s, _| s.set_successful(is_successful));
        }
        // The read-only mirror: the form's constraint-validation bar reads
        // the rendered flag, the way it reads `name` and the resolved
        // validity off this state.
        if self.state.read(cx).is_read_only() != self.is_read_only {
            let read_only = self.is_read_only;
            self.state.update(cx, |s, _| s.set_read_only(read_only));
        }
        // v3 order: the controlled flag, then server errors, then `validate`,
        // with `errorMessage` as the fallback. Resolved here, ahead of the
        // theme borrow, because the write below needs `&mut cx`. The server
        // slot is two sources: the messages the `Form`'s `validationErrors`
        // record routed into this state by name, then the field's own
        // `validationErrors` prop.
        let value_now = self.state.read(cx).value().to_owned();
        let mut server_errors = self.state.read(cx).routed_errors().to_vec();
        server_errors.extend(self.validation_errors.iter().cloned());
        let validate_result = self.validate.as_ref().and_then(|f| f(&value_now));
        let mut validity = crate::validation::resolve(
            self.is_invalid,
            &server_errors,
            validate_result.clone(),
            self.error_message.clone(),
        );
        // The commit-gated half of the same resolution: `validate` and the
        // HTML5 attributes only, with the controlled flag, server messages
        // and `errorMessage` left out — those stay live in every mode.
        let mut pending = crate::validation::resolve(false, &[], validate_result, None);
        // v3's native validation enforces the HTML5 attribute constraints too:
        // a `minLength`/`pattern`/`min`/`max`/`step` violation is a field error
        // even when the controlled flags and `validate` say nothing, and a
        // native form must block on it. The merge touches only the flag — the
        // message slot stays untouched, so no error line appears.
        if !self.validity(&value_now).is_valid() {
            validity.is_invalid = true;
            pending.is_invalid = true;
        }
        // The form reads this back through `FormField::text`; written only
        // when it differs, so the write cannot notify-re-render forever
        // (`set_name` guards its state write the same way).
        let validity_state = self.state.clone();
        if validity_state.read(cx).validity() != &validity {
            validity_state.update(cx, |s, _| s.set_validity(validity.clone()));
        }
        if validity_state.read(cx).pending() != &pending {
            validity_state.update(cx, |s, _| s.set_pending(pending.clone()));
        }
        let focus_handle = self.state.read(cx).focus_handle.clone();
        if self.auto_focus {
            crate::util::focus_once(
                window,
                cx,
                element_id::scoped(&base_id, "autofocus"),
                &focus_handle,
            );
        }
        // Where the text starts, remembered from the last frame: a `canvas` is
        // the only element that is told its own bounds, and a click has to be
        // measured against something. `use_keyed_state` takes `cx` mutably, so
        // it precedes the theme.
        let text_origin =
            window.use_keyed_state(element_id::scoped(&base_id, "text-origin"), cx, |_, _| {
                None::<Pixels>
            });
        // The same trick for a wrapped body, one entry per paragraph.
        let paragraph_bounds =
            window.use_keyed_state(element_id::scoped(&base_id, "paragraphs"), cx, |_, _| {
                Vec::<gpui::Bounds<Pixels>>::new()
            });
        // The font the field draws with, captured here: at event time the text
        // style stack is empty and the shaping would use the wrong face.
        // A `font_family` refinement on the box below is pushed onto the
        // window's text-style stack only while that element prepaints, which
        // is after this `render` has already built the tree — verified against
        // pinned gpui-pre 0.3.3 (`Window::text_style` folds
        // `text_style_stack`, pushed in `with_text_style`). So the family is
        // threaded into the captured font explicitly.
        let mut text_font = window.text_style().font();
        if let Some(family) = &self.font_family {
            text_font.family = family.clone();
        }
        let disabled_opacity = cx.layout().disabled_opacity;
        let focused = focus_handle.is_focused(window);
        // v3 commits a field's validation on blur — but only when the value
        // changed since focus, mirroring the native `change` event — and on
        // submit, which `FormField::commit` reaches for every field. Until a
        // commit the gated half stays internal: `pending` still feeds a
        // `Form`'s blocked decision through `validity`, but the field paints
        // only the live sources plus the last committed snapshot. `left_focus`
        // returns the value the field held at focus on the one render focus
        // has just left.
        if let Some(arrival) = crate::validation::left_focus(
            element_id::scoped(&base_id, "left-focus"),
            focused,
            &value_now,
            window,
            cx,
        ) {
            if arrival != value_now {
                self.state.update(cx, |state, _| state.commit_display());
            }
        }
        // What the field paints. `Allow` (= React Aria's `aria` behavior)
        // shows every source live, validate included, with no commit at
        // all. `Native` keeps the controlled flag, the server messages and
        // `errorMessage` live, but the validate/HTML5 half is the committed
        // `displayed` snapshot — edits after a commit leave it untouched
        // until the next commit.
        let display_validity = match &self.display_override {
            Some(overridden) => overridden.clone(),
            None => crate::validation::display_validity(
                self.state.read(cx).validation_behavior(),
                &validity,
                self.is_invalid,
                &server_errors,
                self.state.read(cx).displayed(),
                self.error_message.clone(),
            ),
        };
        if !focused && self.state.read(cx).marked.is_some() {
            self.state.update(cx, |state, _| state.marked = None);
        }
        // The shared hover tween below borrows the app mutably; own the theme
        // snapshot so the remaining field labels and clear affordance can use
        // the same resolved tokens after the animation is installed.
        let colors = cx.colors().clone();
        let field_theme = cx.theme().components.text_field.resolve(&self.recipes);
        if !self.variant_is_set {
            if let Some(variant) = field_theme.variant {
                self.variant = variant;
            }
        }
        if !self.is_bare_is_set {
            if let Some(is_bare) = field_theme.is_bare {
                self.is_bare = is_bare;
            }
        }
        if self.focus_ring.is_none() {
            self.focus_ring = field_theme.focus_ring;
        }
        self.height = self.height.or(field_theme.height);
        self.padding_x = self.padding_x.or(field_theme.padding_x);
        self.radius = self.radius.or(field_theme.radius);
        let theme_background = field_theme.background.map(|color| color.resolve(&colors));
        let theme_foreground = field_theme.foreground.map(|color| color.resolve(&colors));
        let theme_placeholder = field_theme
            .placeholder
            .map_or(colors.muted, |color| color.resolve(&colors));
        let accent = colors.accent;
        let field_focus = crate::util::FieldFocus {
            is_focused: focused,
            is_focus_within: focus_handle.contains_focused(window, cx),
            is_focus_visible: focused && crate::util::focus_visible(cx),
        };

        // v3's field children-as-a-function: the caller builds the parts from the
        // focus state, so the field's own stack is skipped entirely.
        if let Some(render) = self.field_content.clone() {
            return render(InputFieldRenderState {
                focus: field_focus,
                is_disabled: self.is_disabled,
                is_invalid: display_validity.is_invalid,
                is_read_only: self.is_read_only,
                is_required: self.is_required,
                value: value_now.into(),
            });
        }
        if let Some(render) = self.content.clone() {
            return render(field_focus);
        }

        // Every v3 field is one box: `.input` is `px-3 py-2 text-sm`, which is
        // 36px tall, and its siblings say so outright (`.input-group` and
        // `.search-field__group` are `min-h-9`, `.number-field__group` is `h-9`).
        // This was 40, so every field in the port stood 4px taller than v3's.
        // `Input::height` replaces the row height and nothing else: the text
        // keeps `FIELD_TEXT` and its 20px line, centred in whatever box the
        // caller asked for.
        let (h, text) = (crate::util::FIELD_HEIGHT, crate::util::FIELD_TEXT);
        let h = self.height.unwrap_or(h);
        let text = self.text_size.or(field_theme.text_size).unwrap_or(text);

        let is_invalid = display_validity.is_invalid;
        let show_focus_ring = self.focus_ring.unwrap_or(true);
        let _border_color = if is_invalid {
            colors.danger.color
        } else if focused {
            colors.focus
        } else {
            colors.border
        };
        let multiline = self.multiline;
        // `.textarea` is `py-2` over a `min-height: 38px`; `rows` raises
        // that floor. Without this the field ignored `rows`, so every
        // TextArea came out one line tall inside a taller wrapper.
        let multiline_h = self.min_h.unwrap_or(px(38.));
        // The field box is the `<input>` (or `<textarea>`) upstream renders:
        // its role follows `type`, except that a multi-line field is a
        // `<textarea>`, whose role is the multiline one. The name and the
        // description come from the field anatomy, joined the way
        // `useField` joins the description and error ids.
        let a11y_role = if multiline {
            crate::a11y::Role::MultilineTextInput
        } else {
            self.input_type.a11y_role()
        };
        let a11y_name = crate::a11y::Name::field(
            self.a11y_label.as_ref().or(self.label.as_ref()),
            self.description.as_ref(),
            &display_validity,
        );
        // The box and shared field chrome use the same resolved radius.
        let radius = self.radius.unwrap_or_else(|| crate::util::field_radius(cx));
        let mut field = gpui::div()
            .id(base_id.clone())
            .a11y_named(a11y_role, &a11y_name)
            .a11y_text(&value_now, self.placeholder.as_ref())
            .flex()
            // Multi-line: the text starts at the top, the box grows downward
            // with the content, and the width is fixed so lines can wrap.
            .map(|f| {
                if multiline {
                    f.items_start()
                        .min_h(multiline_h)
                        .py(px(8.))
                        .w_full()
                        .overflow_hidden()
                } else {
                    f.items_center().h(h)
                }
            })
            .gap(px(8.))
            // `.input-group__input` keeps `px-3` except on a side that touches
            // an addon, which carries the padding instead.
            .map(|f| {
                // Only the group owner's override reaches a grouped field;
                // the public `Input::padding_x` keeps its documented
                // grouped-ignored behavior, and unset means `px-3`.
                let padding_x = self.group_padding_x.unwrap_or(px(12.));
                match self.in_group {
                    None => f.px(px(12.)),
                    Some((prefix, suffix)) => f
                        .flex_1()
                        .pl(if prefix { px(0.) } else { padding_x })
                        .pr(if suffix { px(0.) } else { padding_x }),
                }
            })
            // `Input::padding_x` replaces the standalone `px-3`; a grouped
            // field keeps the addon rules above.
            .when_some(
                self.padding_x.filter(|_| self.in_group.is_none()),
                |f, padding_x| f.px(padding_x),
            )
            .text_size(text)
            .line_height(px(20.))
            .when_some(self.font_family.clone(), |f, family| f.font_family(family))
            // A grouped input is the transparent middle of the shared
            // `.input-group` shell. HeroUI's `.input-group__input` is
            // `rounded-none`; letting the inner field keep its field radius
            // makes custom fills and focus overlays expose a second set of
            // corners inside the group's single rounded outline.
            .when(self.in_group.is_none(), |f| f.rounded(radius))
            .when(!self.is_disabled, |e| {
                e.cursor(gpui::CursorStyle::IBeam)
                    .track_focus(&focus_handle)
                    .key_context("Input")
                    // A click puts the caret where it landed, and a drag with
                    // the button down selects -- what a text field does, and
                    // what this one did not: the caret stayed wherever the
                    // value had left it, so the middle of a word was
                    // unreachable with the mouse.
                    .on_mouse_down(gpui::MouseButton::Left, {
                        let fh = focus_handle.clone();
                        let st = self.state.clone();
                        let origin = text_origin.clone();
                        let font = text_font.clone();
                        let masks = self.input_type.masks();
                        let multiline = self.multiline;
                        let paras = paragraph_bounds.clone();
                        // Hit testing follows the text row advance, not the caret glyph height.
                        let line_h = px(20.);
                        let size = text;
                        move |ev: &gpui::MouseDownEvent, window, cx| {
                            window.focus(&fh, cx);
                            let shown = displayed_value(st.read(cx), masks);
                            let at = if multiline {
                                let boxes = paras.read(cx).clone();
                                let Some(at) = char_at_point(
                                    &shown,
                                    ev.position,
                                    &boxes,
                                    &font,
                                    size,
                                    line_h,
                                    window,
                                ) else {
                                    return;
                                };
                                at
                            } else {
                                let Some(left) = *origin.read(cx) else {
                                    return;
                                };
                                char_at_x(&shown, ev.position.x - left, &font, size, window)
                            };
                            st.update(cx, |s, cx| {
                                s.marked = None;
                                s.cursor = at;
                                s.anchor = None;
                                // A double click takes the word under the
                                // pointer and a triple click the lot, the way
                                // every other text field does.
                                match ev.click_count {
                                    2 => {
                                        s.anchor = Some(word_target(&s.value, at, false));
                                        s.cursor = word_target(&s.value, at, true);
                                    }
                                    n if n >= 3 => select_all(s),
                                    _ => {}
                                }
                                cx.notify();
                            });
                        }
                    })
                    .on_mouse_move({
                        let st = self.state.clone();
                        let origin = text_origin.clone();
                        let font = text_font.clone();
                        let masks = self.input_type.masks();
                        let multiline = self.multiline;
                        let paras = paragraph_bounds.clone();
                        // The caret is drawn at 1.3x the font size; the rows a wrapped
                        // paragraph is measured in are the same height.
                        let line_h = text * 1.3;
                        let size = text;
                        move |ev: &gpui::MouseMoveEvent, window, cx| {
                            if ev.pressed_button != Some(gpui::MouseButton::Left) {
                                return;
                            }
                            let shown = displayed_value(st.read(cx), masks);
                            let at = if multiline {
                                let boxes = paras.read(cx).clone();
                                let Some(at) = char_at_point(
                                    &shown,
                                    ev.position,
                                    &boxes,
                                    &font,
                                    size,
                                    line_h,
                                    window,
                                ) else {
                                    return;
                                };
                                at
                            } else {
                                let Some(left) = *origin.read(cx) else {
                                    return;
                                };
                                char_at_x(&shown, ev.position.x - left, &font, size, window)
                            };
                            st.update(cx, |s, cx| {
                                if s.anchor.is_none() {
                                    s.anchor = Some(s.cursor);
                                }
                                s.marked = None;
                                s.cursor = at;
                                cx.notify();
                            });
                        }
                    })
            })
            // `status-disabled` is `--disabled-opacity`, which the theme owns.
            // Inside a group that dims its own box the field skips the second
            // coat; on its own (or a field disabled beside an enabled group)
            // it still dims itself.
            .when(self.is_disabled && !self.group_dim, |e| e.opacity(disabled_opacity));

        // The multi-line field's ring, parked until its content is in place;
        // see the chrome call below.
        let mut carrier_ring = None;

        // Inside an `InputGroup` the group is the field: v3's
        // `.input-group__input` is `rounded-none border-0 bg-transparent
        // shadow-none`.
        // `is_bare` takes the same exit: one chrome call site, skipped by
        // either reason.
        if self.in_group.is_none() && !self.is_bare {
            // Both spellings of the field take the same overlay ring. The
            // single-line box does not clip, so the ring is its own child;
            // the multi-line box clips its wrapped text (`overflow_hidden`
            // above) and would cut a child ring, so its ring hangs on the
            // non-clipping carrier added once the content is in place.
            let ring = crate::util::field_ring_color(is_invalid, focused, show_focus_ring, cx);
            field = crate::util::apply_field_chrome_ringless(
                field,
                self.variant,
                is_invalid,
                focused,
                show_focus_ring,
                Some(radius),
                cx,
            );
            if multiline {
                carrier_ring = Some(ring);
            } else {
                field = crate::util::with_field_ring_overlay(field, ring, radius, cx);
            }

            if !self.is_disabled && !is_invalid && !focused && theme_background.is_none() {
                let idle_bg = match self.variant {
                    FieldVariant::Primary => colors.field.background,
                    FieldVariant::Secondary => colors.default.color,
                };
                let hover_bg = match self.variant {
                    FieldVariant::Primary => colors.field.hover(),
                    // `.input--secondary` uses the default-hover endpoint.
                    FieldVariant::Secondary => colors.default.hover(),
                };
                let hover_border = colors.field.border_hover();
                // Keep the editable Input as the stable behavior owner while
                // the listener-free visual fill follows HeroUI's 150ms
                // ease-smooth transition. The border endpoint is immediate,
                // and custom theme backgrounds retain their caller-owned fill.
                field = crate::anim::hover_fade_with_duration_and_easing(
                    field,
                    element_id::scoped(&base_id, "hover-fade"),
                    (idle_bg, hover_bg),
                    None,
                    Some(hover_border),
                    |fill| fill.rounded(radius),
                    Some(150),
                    crate::anim::HoverFadeEasing::EaseSmooth,
                    window,
                    cx,
                );
            }
        }
        if let Some(background) = theme_background {
            field = field.bg(background);
        }
        if let Some(foreground) = theme_foreground {
            field = field.text_color(foreground);
        }

        // -- text content -----------------------------------------------------
        let st = self.state.read(cx);
        // `password` renders bullets while keeping the real value in state, so
        // cursor and selection maths stay in char units either way.
        let value = if self.input_type.masks() {
            "\u{2022}".repeat(st.value.chars().count())
        } else {
            st.value.clone()
        };
        let cursor = st.cursor;
        let is_empty = value.is_empty();
        // IME marked text lives inside `value`, so a composition already
        // makes the field non-empty; the explicit check keeps the placeholder
        // from ever painting under marked text.
        let empty_for_display = is_empty && st.marked.is_none();
        let selection = st.selection();

        // A multi-line field lays its lines out top-down and lets each one
        // wrap; a single-line one is a centred, non-wrapping row.
        let mut row = if self.multiline {
            gpui::div()
                .flex()
                .flex_col()
                .items_start()
                .w_full()
                .min_w_0()
                .flex_1()
        } else {
            let origin = text_origin;
            gpui::div()
                .flex()
                .items_center()
                .min_w_0()
                .flex_1()
                // A zero-width item at the head of the row: its bounds are the
                // text's left edge, which is what a click is measured against.
                // Only `canvas` is told its own bounds.
                .child(
                    gpui::canvas(
                        move |bounds, _window, cx| {
                            let left = bounds.origin.x;
                            if *origin.read(cx) != Some(left) {
                                origin.update(cx, |v, _| *v = Some(left));
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .w(px(0.))
                    .h(px(0.))
                    .flex_shrink_0(),
                )
        };

        if empty_for_display && focused && self.placeholder.is_some() {
            // HTML keeps the placeholder painted while the empty field holds
            // focus, the caret blinking at the start before the first glyph.
            // The caret overlays the text's left edge instead of joining the
            // flow, so the placeholder sits at the same x focused or not —
            // and it is the same caret element the text path uses, so the
            // blink continues seamlessly once typing replaces it.
            row = row.child(
                gpui::div()
                    .relative()
                    .w_full()
                    .min_w_0()
                    .child(
                        gpui::div()
                            .w_full()
                            .min_w_0()
                            // Headless probe: the placeholder's own bounds,
                            // so a test can prove it holds its x when focus
                            // toggles and vanishes once typing starts.
                            .debug_selector(|| "input-placeholder".to_owned())
                            .text_color(theme_placeholder)
                            .truncate()
                            .child(self.placeholder.clone().unwrap().to_string()),
                    )
                    .child(
                        gpui::div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(crate::anim::caret_blink(
                                gpui::div()
                                    .w(px(1.5))
                                    .h(text * 1.3)
                                    .bg(accent.color)
                                    .flex_shrink_0(),
                                element_id::scoped(&base_id, "caret"),
                                cx,
                            )),
                    ),
            );
        } else if self.multiline && !(empty_for_display && !focused && self.placeholder.is_some()) {
            // One wrapping paragraph per newline, with the caret and any
            // selection placed inside the line they fall in.
            let caret_id = element_id::scoped(&base_id, "caret");
            row = row.child(multiline_body(
                MultilineBody {
                    paragraphs: paragraph_bounds.clone(),
                    value: &value,
                    cursor,
                    selection,
                    focused,
                    caret: (caret_id, text * 1.3, accent.color),
                    selection_bg: accent.with_alpha(0.24),
                },
                cx,
            ));
        } else if empty_for_display && !focused && self.placeholder.is_some() {
            row = row.child(
                gpui::div()
                    // Headless probe: see the focused branch above.
                    .debug_selector(|| "input-placeholder".to_owned())
                    .text_color(theme_placeholder)
                    .truncate()
                    .child(self.placeholder.clone().unwrap().to_string()),
            );
        } else if let Some((lo, hi)) = selection {
            // Selected range — three spans.
            let before: String = value.chars().take(lo).collect();
            let selected: String = value.chars().skip(lo).take(hi - lo).collect();
            let after: String = value.chars().skip(hi).collect();
            let sel_bg = accent.with_alpha(0.24);
            row = row.child(
                gpui::div()
                    .flex()
                    .items_center()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(before)
                    .child(
                        gpui::div()
                            .px(px(1.))
                            .py(px(1.))
                            .rounded(px(4.))
                            .bg(sel_bg)
                            .child(selected),
                    )
                    .child(after),
            );
        } else {
            let before: String = value.chars().take(cursor).collect();
            let after: String = value.chars().skip(cursor).collect();
            row = row.child(
                gpui::div()
                    .flex()
                    .items_center()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(before)
                    .when(focused, |r| {
                        // v3's `@keyframes caret-blink`.
                        r.child(crate::anim::caret_blink(
                            gpui::div()
                                .w(px(1.5))
                                .h(text * 1.3)
                                .bg(accent.color)
                                .flex_shrink_0(),
                            element_id::scoped(&base_id, "caret"),
                            cx,
                        ))
                    })
                    .child(after),
            );
        }

        // The builder inputs an accepted edit re-resolves the stored validity
        // from (see `refresh_stored_validity`): this field's own sources,
        // never the routed slot the edit suppresses, plus the HTML5 attribute
        // check render merges into the flag.
        let edit_is_invalid = self.is_invalid;
        let edit_validation_errors = self.validation_errors.clone();
        let edit_validate = self.validate.clone();
        let edit_error_message = self.error_message.clone();
        let native_validity = {
            let input_type = self.input_type;
            let min_length = self.min_length;
            let min = self.min;
            let max = self.max;
            let step = self.step;
            let pattern = self.pattern.clone();
            move |value: &str| {
                validate_value(
                    value,
                    input_type,
                    min_length,
                    min,
                    max,
                    step,
                    pattern.as_deref(),
                )
                .is_valid()
            }
        };

        let edit_state = self.state.clone();
        let on_change = self.on_change.clone();
        let platform_native_validity = native_validity.clone();
        let platform_error_message = edit_error_message.clone();
        let platform_validate = edit_validate.clone();
        let platform_validation_errors = edit_validation_errors.clone();
        let edit_callback: TextCallback = crate::util::shared(move |value, window, cx| {
            edit_state.update(cx, |s, _| s.clear_routed_errors());
            refresh_stored_validity(
                &edit_state,
                edit_is_invalid,
                &platform_validation_errors,
                platform_validate.as_ref(),
                platform_error_message.clone(),
                &platform_native_validity,
                cx,
            );
            if let Some(callback) = &on_change {
                callback(value, window, cx);
            }
        });
        let platform_state = self.state.clone();
        let platform_edit = edit_callback.clone();
        let platform_focus = focus_handle.clone();
        let platform_font = text_font;
        let platform_paragraphs = paragraph_bounds;
        let platform_type = self.input_type;
        let platform_max = self.max_length;
        let platform_multiline = self.multiline;
        let platform_disabled = self.is_disabled;
        row = row.relative().child(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    if !platform_disabled {
                        window.handle_input(
                            &platform_focus,
                            PlatformTextInput {
                                state: platform_state.clone(),
                                on_edit: platform_edit.clone(),
                                input_type: platform_type,
                                max_length: platform_max,
                                multiline: platform_multiline,
                                bounds,
                                font: platform_font.clone(),
                                paragraphs: platform_paragraphs.clone(),
                            },
                            cx,
                        );
                    }
                },
            )
            .absolute()
            .size_full(),
        );

        field = field.children(self.start_content);
        field = field.child(row);
        // isClearable — show X when has value and not disabled/readonly
        if self.is_clearable && !is_empty && !self.is_disabled && !self.is_read_only {
            let clear_focus_handle = self.state.read(cx).clear_focus_handle.clone();
            let clear_content = match self.clear_content {
                Some(content) => content,
                None => gpui::svg()
                    .size(px(12.))
                    .path(crate::icons::CLOSE)
                    .text_color(colors.muted)
                    .into_any_element(),
            };
            let clear_state = self.state.clone();
            let clear_on_change = self.on_change.clone();
            let on_clear = self.on_clear.clone();
            let clear_validation_errors = edit_validation_errors;
            let clear_validate = edit_validate.clone();
            let clear_error_message = edit_error_message;
            let clear_native = native_validity.clone();
            // `.search-field__clear-button` *is* a `CloseButton`, and
            // `.close-button:hover` fills `bg-default-hover` -- not a
            // hand-mixed wash.
            let clear_hover_bg = self.clear_hover_bg.unwrap_or(colors.default.hover());
            let clear_box = px(20.);
            let clear_radius = crate::util::small_radius(cx);
            let clear_selector = format!("input-clear-{}", self.state.entity_id().as_u64());
            let input_focus_after_clear = focus_handle.clone();
            let input_focus_handle = focus_handle.clone();
            let mut clear = gpui::div()
                .id(element_id::scoped(&base_id, "clear"))
                .debug_selector(move || clear_selector)
                .flex()
                .items_center()
                .justify_center()
                .size(clear_box)
                .p(px(4.))
                .rounded(clear_radius)
                .cursor(crate::util::interactive_cursor(cx))
                .text_color(colors.muted)
                .hover(move |s| s.bg(clear_hover_bg))
                .active({
                    // GPUI 0.2.2 has no div transform; use the same centred
                    // geometry as CloseButton's scale(0.93). The active style
                    // is an instant swap, including under reduced motion.
                    const PRESS_SCALE: f32 = 0.93;
                    let inset = px(f32::from(clear_box) * (1.0 - PRESS_SCALE) / 2.0);
                    let pressed = px(f32::from(clear_box) * PRESS_SCALE);
                    let radius = px(f32::from(clear_radius) * PRESS_SCALE);
                    move |s| {
                        s.h(pressed)
                            .w(pressed)
                            .mt(inset)
                            .mb(inset)
                            .ml(inset)
                            .mr(inset)
                            .rounded(radius)
                    }
                })
                // Pinned `useSearchField` `excludeFromTabOrder: true` (the
                // InputState-owned handle is not a tab stop) and
                // `preventFocusOnPress: true`. Pointer presses keep focus on
                // the input. Keys on the button must not bubble into the
                // field except Tab, which still walks to the next stop.
                .track_focus(&clear_focus_handle)
                .on_key_down(|event, _, cx| {
                    if event.keystroke.key != "tab" {
                        cx.stop_propagation();
                    }
                })
                .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    window.focus(&input_focus_handle, cx);
                    window.prevent_default();
                })
                .on_click(move |_, window, cx| {
                    window.focus(&input_focus_after_clear, cx);
                    clear_state.update(cx, |s, cx| {
                        s.value.clear();
                        s.marked = None;
                        s.cursor = 0;
                        s.anchor = None;
                        // The clear affordance is a user modification:
                        // it suppresses the routed server errors too.
                        s.clear_routed_errors();
                        cx.notify();
                    });
                    // The emptied value is the mirror's too, before any
                    // callback can observe the field: a synchronous
                    // submit inside `on_change` must not be blocked by
                    // the routed error this click just answered.
                    refresh_stored_validity(
                        &clear_state,
                        edit_is_invalid,
                        &clear_validation_errors,
                        clear_validate.as_ref(),
                        clear_error_message.clone(),
                        &clear_native,
                        cx,
                    );
                    if let Some(cb) = &clear_on_change {
                        cb("", window, cx);
                    }
                    if let Some(cb) = &on_clear {
                        cb(window, cx);
                    }
                })
                .child(clear_content);
            // The clear affordance both carries `clear_radius` and takes the
            // focus, so the overlay ring goes straight on it and tracks the
            // same corner. Only the button migrates: the field around it
            // clips its children in the multi-line case and stays on shadows.
            clear = crate::util::ring_overlay_if_focused(
                clear,
                &clear_focus_handle,
                true,
                clear_radius,
                Vec::new(),
                window,
                cx,
            );
            field = field.child(clear);
        }
        field = field.children(self.end_content);

        // -- editing ----------------------------------------------------------
        let state_entity = self.state.clone();
        let on_submit = self.on_submit.clone();
        let on_clear = self.on_clear.clone();
        let clear_on_escape = self.clear_on_escape;
        let is_read_only = self.is_read_only;
        let is_disabled = self.is_disabled;
        field = field.on_key_down(move |ev: &KeyDownEvent, window, cx| {
            if is_disabled {
                return;
            }
            let key: &str = &ev.keystroke.key;
            if crate::util::key_enables_focus_visible(key) {
                crate::util::set_focus_visible(true, cx);
            }
            let mods = ev.keystroke.modifiers;
            if state_entity.read(cx).marked.is_some() && !matches!(key, "escape" | "tab") {
                cx.stop_propagation();
                return;
            }
            let input_type = self.input_type;
            let max_length = self.max_length;
            let multiline = self.multiline;
            let owns_key = (!mods.control
                && !mods.alt
                && !mods.platform
                && (matches!(key, "backspace" | "delete" | "left" | "right")
                    || multiline && matches!(key, "up" | "down" | "home" | "end" | "enter")))
                || (mods.control || mods.platform)
                    && !mods.alt
                    && matches!(key, "a" | "left" | "right");
            if owns_key {
                cx.stop_propagation();
            }
            if owns_key || key == "escape" {
                state_entity.update(cx, |state, _| state.marked = None);
            }
            let mut changed = false;
            let mut cleared = false;
            let mut submit = false;

            if key == "a" && (mods.control || mods.platform) {
                // Ctrl+A / Cmd+A : select all
                if !mods.alt {
                    state_entity.update(cx, |s, cx| {
                        select_all(s);
                        cx.notify();
                    });
                }
            } else if mods.control || mods.alt || mods.platform {
                let cmd = mods.control || mods.platform;
                if !is_read_only && cmd && key == "v" {
                    if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                        changed = state_entity.update(cx, |s, cx| {
                            let mut inserted = false;
                            for ch in text.chars() {
                                if state_accepts(s, ch, input_type, max_length) {
                                    insert_char(s, ch);
                                    inserted = true;
                                }
                            }
                            if inserted {
                                cx.notify();
                            }
                            inserted
                        });
                    }
                } else if cmd && (key == "c" || key == "x") {
                    // Paste was here without a copy: a field you can paste into
                    // and not copy out of is half a clipboard. A masked field
                    // is the exception: browsers refuse copy and cut on
                    // `type="password"` (the value never reaches the clipboard
                    // and cut deletes nothing), and so does this.
                    if input_type.masks() {
                        return;
                    }
                    let selected = {
                        let st = state_entity.read(cx);
                        slice_selection(&st.value, st.selection())
                    };
                    if let Some(text) = selected {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                        if key == "x" && !is_read_only {
                            state_entity.update(cx, |s, cx| {
                                delete_selection(s);
                                cx.notify();
                            });
                            changed = true;
                        }
                    }
                } else if cmd && (key == "left" || key == "right") {
                    // Ctrl+arrow is word-wise motion; a password field is the
                    // one place it would leak the shape of the value, and it
                    // does not there either, since the glyphs are dots.
                    state_entity.update(cx, |s, cx| {
                        move_word(s, key == "right", mods.shift);
                        cx.notify();
                    });
                }
            } else {
                let shift = mods.shift;
                match key {
                    "backspace" => {
                        if is_read_only {
                            return;
                        }
                        changed = state_entity.update(cx, |s, cx| {
                            let deleted = backspace(s);
                            if deleted {
                                cx.notify();
                            }
                            deleted
                        });
                    }
                    "delete" => {
                        if is_read_only {
                            return;
                        }
                        changed = state_entity.update(cx, |s, cx| {
                            let deleted = delete(s);
                            if deleted {
                                cx.notify();
                            }
                            deleted
                        });
                    }
                    "left" => state_entity.update(cx, |s, cx| {
                        move_left(s, shift);
                        cx.notify();
                    }),
                    "right" => state_entity.update(cx, |s, cx| {
                        move_right(s, shift);
                        cx.notify();
                    }),
                    // Home and End are the *line's* ends in a multi-line field,
                    // the way a `<textarea>` has it; only a single-line field
                    // has one line to run to.
                    "home" => state_entity.update(cx, |s, cx| {
                        if multiline {
                            before_move(s, shift);
                            s.cursor = line_bounds(&s.value, s.cursor).0;
                        } else {
                            move_home(s, shift);
                        }
                        cx.notify();
                    }),
                    "end" => state_entity.update(cx, |s, cx| {
                        if multiline {
                            before_move(s, shift);
                            s.cursor = line_bounds(&s.value, s.cursor).1;
                        } else {
                            move_end(s, shift);
                        }
                        cx.notify();
                    }),
                    // Vertical motion only means something with lines to move
                    // between; a single-line field leaves up and down to
                    // whatever is around it (a combo box reads them).
                    "up" | "down" if multiline => state_entity.update(cx, |s, cx| {
                        move_vertical(s, key == "down", shift);
                        cx.notify();
                    }),
                    // A multi-line field takes Enter as a newline, the way a
                    // `<textarea>` does; a single-line one submits.
                    "enter" if multiline => {
                        cx.stop_propagation();
                        if is_read_only {
                            return;
                        }
                        changed = state_entity.update(cx, |s, cx| {
                            let inserted = state_accepts(s, NEWLINE, input_type, max_length);
                            if inserted {
                                insert_char(s, NEWLINE);
                                cx.notify();
                            }
                            inserted
                        });
                    }
                    "enter" => submit = true,
                    "escape" => {
                        let had_value = !state_entity.read(cx).is_empty();
                        state_entity.update(cx, |s, cx| {
                            s.anchor = None;
                            if clear_on_escape && !is_read_only {
                                s.value.clear();
                                s.cursor = 0;
                            }
                            cx.notify();
                        });
                        if clear_on_escape && !is_read_only && had_value {
                            changed = true;
                            cleared = true;
                        }
                    }
                    _ => {}
                }
            }

            if changed {
                if mods.control || mods.platform {
                    cx.stop_propagation();
                }
                state_entity.update(cx, |state, _| state.marked = None);
                let value = state_entity.read(cx).value().to_owned();
                edit_callback(&value, window, cx);
            }
            if cleared {
                if let Some(cb) = &on_clear {
                    cb(window, cx);
                }
            }
            if submit {
                if let Some(cb) = &on_submit {
                    {
                        let v = state_entity.read(cx).value().to_owned();
                        cb(&v, window, cx);
                    }
                    // A field with its own `onSubmit` owns Enter, the way a
                    // native input whose keydown handler handles the key
                    // stops the form's implicit submission: the keystroke
                    // must not also bubble into the enclosing `Form`. A
                    // field without `onSubmit` swallows nothing — its Enter
                    // bubbling is exactly how a plain field submits its
                    // form.
                    cx.stop_propagation();
                }
            }
        });

        // The `sx` slot refines whatever root this render returns. Inside a
        // group the field row *is* the whole component (the wrapper below is
        // skipped), so it lands there; standalone it lands on the
        // label-to-error column at the tail, and the row keeps its own paint.
        let field = if self.in_group.is_some() {
            crate::util::apply_sx(field, &self.sx)
        } else {
            field
        };
        let field = if self.is_disabled {
            field
        } else {
            crate::util::record_focus_bounds(field, &focus_handle, window, cx)
        };

        // The anchor is the 36px field row itself — not the
        // label-to-error wrapper below — the way RAC's `triggerRef` reads
        // `groupRef.current || inputRef.current`. Wrapping here keeps the
        // label, description, error and value rows outside the measured
        // bounds while preserving their existing rendering order.
        let field_element: gpui::AnyElement = if let Some((anchor, selector)) = self.field_anchor {
            let selector_text = selector.to_string();
            crate::popover::PopoverTriggerMeasure::new(
                field.debug_selector(move || selector_text),
                anchor,
            )
            .into_any_element()
        } else {
            field.into_any_element()
        };

        // The multi-line box clips its wrapped text, so its focus ring rides
        // `util::field_ring_carrier`. The measured anchor bounds stay on the
        // field row inside it.
        let field_element: gpui::AnyElement = match carrier_ring {
            Some(ring) => {
                crate::util::field_ring_carrier(field_element, ring, radius, cx).into_any_element()
            }
            None => field_element,
        };

        // Inside a group the surrounding component owns the label, the
        // description and the error slot -- v3's `InputGroup.Input` is the input
        // and nothing else. Returning the wrapper here would drop a whole
        // labelled column into the group's row.
        if self.in_group.is_some() {
            return field_element;
        }

        // -- wrapper with label / description / error --------------------------
        let mut el = gpui::div().flex().flex_col().gap(px(4.));
        if self.full_width {
            el = el.w_full();
        } else {
            el = el.max_w(px(320.));
        }
        if let Some(label) = self.label {
            let mut label_row = gpui::div()
                .flex()
                .items_center()
                .gap(px(4.))
                .text_size(px(14.))
                .line_height(px(20.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(colors.foreground)
                .child(label.to_string());
            if self.is_required {
                label_row = label_row.child(
                    gpui::div()
                        .text_color(colors.danger.color)
                        .child("*".to_owned()),
                );
            }
            el = el.child(label_row);
        }
        el = el.child(field_element);
        // Every message, space-joined in upstream order — React Aria's
        // `FieldError` default — not just the first. The shared helper keeps
        // the last message mounted while the invalid row animates out, so a
        // description cannot replace it halfway through the height tween.
        let error =
            (!display_validity.messages.is_empty()).then(|| display_validity.joined().into());
        if let Some(error) = crate::anim::field_error_panel(&base_id, error, window, cx) {
            el = el.child(error);
        } else if let Some(desc) = self.description {
            el = el.child(
                gpui::div()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(colors.muted)
                    .child(desc.to_string()),
            );
        }

        el = crate::util::apply_sx(el, &self.sx);
        el.into_any_element()
    }
}

// ---------------------------------------------------------------------------
// TextField / SearchField
// ---------------------------------------------------------------------------

/// The complete state passed to [`TextField::content`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct TextFieldRenderState {
    /// The field cannot receive focus or input.
    pub is_disabled: bool,
    /// Controlled, server, or custom validation currently fails.
    pub is_invalid: bool,
    /// The value can be selected but not changed.
    pub is_read_only: bool,
    /// The field must contain a value before native form submission.
    pub is_required: bool,
    /// The input itself owns keyboard focus.
    pub is_focused: bool,
    /// The input or another composed child owns keyboard focus.
    pub is_focus_within: bool,
    /// Focus was reached through keyboard navigation.
    pub is_focus_visible: bool,
}

/// TextField — port of `@heroui/text-field` (v3).
///
/// The composition-friendly field: a label, an [`Input`], and a description or
/// validation message. `Input` is the bare control; `TextField` is the labelled
/// wrapper most applications reach for.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct TextField {
    inner: Input,
}

impl TextField {
    /// Builds the labelled field over `state`, borrowed or owned — see
    /// [`Input::new`].
    pub fn new(state: impl std::borrow::Borrow<Entity<InputState>>) -> Self {
        Self {
            inner: Input::new(state),
        }
    }

    /// `type` — the HTML input type, which v3 sets on the field and this port
    /// forwards to the inner [`Input`].
    pub fn input_type(mut self, input_type: InputType) -> Self {
        self.inner = self.inner.input_type(input_type);
        self
    }

    /// Sets the visible label.
    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.inner = self.inner.label(text);
        self
    }

    /// `aria-label` — see [`Input::aria_label`].
    pub fn aria_label(mut self, text: impl Into<SharedString>) -> Self {
        self.inner = self.inner.aria_label(text);
        self
    }

    /// v3's field `children`-as-a-function, handed the complete resolved field
    /// state.
    pub fn content(
        mut self,
        render: impl Fn(TextFieldRenderState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.inner = self.inner.field_content(move |state| {
            render(TextFieldRenderState {
                is_disabled: state.is_disabled,
                is_invalid: state.is_invalid,
                is_read_only: state.is_read_only,
                is_required: state.is_required,
                is_focused: state.focus.is_focused,
                is_focus_within: state.focus.is_focus_within,
                is_focus_visible: state.focus.is_focus_visible,
            })
        });
        self
    }

    /// Sets the placeholder shown while the value is empty.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.inner = self.inner.placeholder(text);
        self
    }

    /// Sets the description text.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.inner = self.inner.description(text);
        self
    }

    /// `autoFocus` — see [`Input::auto_focus`].
    pub fn auto_focus(mut self, v: bool) -> Self {
        self.inner = self.inner.auto_focus(v);
        self
    }

    /// `name` — see [`Input::name`].
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.inner = self.inner.name(name);
        self
    }

    /// `defaultValue` — see [`Input::default_value`].
    pub fn default_value(mut self, text: impl Into<SharedString>) -> Self {
        self.inner = self.inner.default_value(text);
        self
    }

    /// `value` — see [`Input::value`].
    pub fn value(mut self, text: impl Into<SharedString>) -> Self {
        self.inner = self.inner.value(text);
        self
    }

    /// `validationBehavior` — see [`crate::input::Input::validation_behavior`].
    pub fn validation_behavior(mut self, behavior: crate::form::ValidationBehavior) -> Self {
        self.inner = self.inner.validation_behavior(behavior);
        self
    }

    /// `validate` — see [`Input::validate`].
    pub fn validate(mut self, f: impl Fn(&str) -> Option<SharedString> + 'static) -> Self {
        self.inner = self.inner.validate(f);
        self
    }

    /// `validationErrors` — see [`Input::validation_errors`].
    pub fn validation_errors(
        mut self,
        errors: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.inner = self.inner.validation_errors(errors);
        self
    }

    /// Sets the error message shown when the field is invalid.
    pub fn error_message(mut self, text: impl Into<SharedString>) -> Self {
        self.inner = self.inner.error_message(text);
        self
    }

    /// Sets the field variant.
    pub fn variant(mut self, variant: FieldVariant) -> Self {
        self.inner = self.inner.variant(variant);
        self
    }

    /// Makes the field fill its parent's width.
    pub fn full_width(mut self) -> Self {
        self.inner = self.inner.full_width();
        self
    }

    /// The single-line box height — see [`Input::height`].
    pub fn height(mut self, h: impl Into<Pixels>) -> Self {
        self.inner = self.inner.height(h);
        self
    }

    /// The box's horizontal padding — see [`Input::padding_x`].
    pub fn padding_x(mut self, p: impl Into<Pixels>) -> Self {
        self.inner = self.inner.padding_x(p);
        self
    }

    /// Named theme recipe — see [`Input::recipe`].
    pub fn recipe(mut self, name: impl Into<SharedString>) -> Self {
        self.inner = self.inner.recipe(name);
        self
    }

    /// The field text size — see [`Input::text_size`].
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.inner = self.inner.text_size(size);
        self
    }

    /// The box's corner radius — see [`Input::radius`].
    pub fn radius(mut self, r: impl Into<Pixels>) -> Self {
        self.inner = self.inner.radius(r);
        self
    }

    /// Drops the field's chrome — see [`Input::is_bare`].
    pub fn is_bare(mut self, v: bool) -> Self {
        self.inner = self.inner.is_bare(v);
        self
    }

    /// Shows or hides only the inner field's visual focus ring — see
    /// [`Input::focus_ring`].
    pub fn focus_ring(mut self, v: bool) -> Self {
        self.inner = self.inner.focus_ring(v);
        self
    }

    /// The field text's font family — see [`Input::font_family`].
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.inner = self.inner.font_family(family);
        self
    }

    /// Leading content inside the box — see [`Input::start_content`].
    pub fn start_content(mut self, el: impl IntoElement) -> Self {
        self.inner = self.inner.start_content(el);
        self
    }

    /// Sets whether the field is disabled.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.inner = self.inner.is_disabled(v);
        self
    }

    /// Sets whether the field is read-only.
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.inner = self.inner.is_read_only(v);
        self
    }

    /// Sets whether the field is required.
    pub fn is_required(mut self, v: bool) -> Self {
        self.inner = self.inner.is_required(v);
        self
    }

    /// Sets whether the field is invalid.
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.inner = self.inner.is_invalid(v);
        self
    }

    /// Sets the handler called with the new value whenever it changes.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.inner = self.inner.on_change(handler);
        self
    }

    /// Sets the handler called with the current value when Enter is pressed.
    pub fn on_submit(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.inner = self.inner.on_submit(handler);
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the field's root element after every value the variant and
    /// the active theme chose, so they win. The wrapper renders no element of
    /// its own, so the slot rides the inner [`Input`] and lands on that root.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        self.inner = self.inner.sx(style);
        self
    }
}

impl RenderOnce for TextField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.inner.render(window, cx)
    }
}

/// The complete state passed to [`SearchField::content`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SearchFieldRenderState {
    /// The field cannot receive focus or input.
    pub is_disabled: bool,
    /// Controlled, server, or custom validation currently fails.
    pub is_invalid: bool,
    /// The value can be selected but not changed.
    pub is_read_only: bool,
    /// The field must contain a value before native form submission.
    pub is_required: bool,
    /// The input itself owns keyboard focus.
    pub is_focused: bool,
    /// The input or another composed child owns keyboard focus.
    pub is_focus_within: bool,
    /// Focus was reached through keyboard navigation.
    pub is_focus_visible: bool,
    /// The current search query.
    pub value: SharedString,
    /// The current search query is empty.
    pub is_empty: bool,
}

/// SearchField — port of `@heroui/search-field` (v3).
///
/// An [`Input`] specialised for search: a leading magnifier icon and a clear
/// button that appears once there is a value. `onSubmit` fires on Enter and
/// `onClear` when the value is cleared.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct SearchField {
    state: Entity<InputState>,
    /// See [`SearchField::content`]: v3's field children-as-a-function.
    content: Option<std::sync::Arc<dyn Fn(SearchFieldRenderState) -> gpui::AnyElement + 'static>>,
    /// `name` — the submission name, forwarded to the inner `Input`.
    name: Option<SharedString>,
    /// `defaultValue` — forwarded to the inner `Input`.
    default_value: Option<SharedString>,
    /// `value` — forwarded to the inner `Input`.
    value: Option<SharedString>,
    /// `validationBehavior` — forwarded to the inner `Input`.
    validation_behavior: Option<crate::form::ValidationBehavior>,
    label: Option<SharedString>,
    placeholder: SharedString,
    /// Whether `placeholder` came from the caller; otherwise render uses
    /// the localized default (`i18n::UiString::Search`).
    placeholder_is_set: bool,
    description: Option<SharedString>,
    variant: FieldVariant,
    variant_is_set: bool,
    recipes: Vec<SharedString>,
    text_size: Option<Pixels>,
    /// The field text's family, forwarded to the inner `Input`.
    font_family: Option<SharedString>,
    radius: Option<Pixels>,
    full_width: bool,
    /// Optional box geometry/chrome overrides forwarded to the inner `Input`.
    field: crate::util::FieldBox,
    is_disabled: bool,
    is_read_only: bool,
    is_required: bool,
    is_invalid: bool,
    /// `autoFocus` — take focus on the first render.
    auto_focus: bool,
    /// `validate` — run by the component, not the caller.
    validate: Option<crate::validation::Validator<str>>,
    /// `validationErrors` — messages from a server round-trip.
    validation_errors: Vec<SharedString>,
    on_change: Option<TextCallback>,
    on_submit: Option<TextCallback>,
    on_clear: Option<std::sync::Arc<dyn Fn(&mut Window, &mut App) + 'static>>,
    /// `SearchField.SearchIcon` — the leading glyph. `None` draws the magnifier.
    search_icon: Option<gpui::AnyElement>,
    /// `SearchField.ClearButton` children. `None` draws the close glyph.
    clear_icon: Option<gpui::AnyElement>,
    /// Trailing content inside the field, before the clear button. v3 composes
    /// it (a `Kbd` with the shortcut, in its "With Keyboard Shortcut" example).
    end_content: Option<gpui::AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl SearchField {
    /// v3's field `children`-as-a-function, handed the complete resolved field
    /// and query state.
    pub fn content(
        mut self,
        render: impl Fn(SearchFieldRenderState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.content = Some(std::sync::Arc::new(render));
        self
    }

    /// Builds the search field over `state`, borrowed or owned — see
    /// [`Input::new`].
    pub fn new(state: impl std::borrow::Borrow<Entity<InputState>>) -> Self {
        Self {
            content: None,
            state: state.borrow().clone(),
            name: None,
            default_value: None,
            value: None,
            validation_behavior: None,
            label: None,
            placeholder: "Search".into(),
            placeholder_is_set: false,
            description: None,
            variant: FieldVariant::Primary,
            variant_is_set: false,
            recipes: Vec::new(),
            text_size: None,
            font_family: None,
            radius: None,
            full_width: false,
            field: crate::util::FieldBox::default(),
            is_disabled: false,
            is_read_only: false,
            is_required: false,
            is_invalid: false,
            auto_focus: false,
            validate: None,
            validation_errors: Vec::new(),
            on_change: None,
            on_submit: None,
            on_clear: None,
            search_icon: None,
            clear_icon: None,
            end_content: None,
            sx: None,
        }
    }

    /// Sets the visible label.
    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// Sets the placeholder shown while the value is empty.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self.placeholder_is_set = true;
        self
    }

    /// Sets the description text.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Sets the field variant.
    pub fn variant(mut self, variant: FieldVariant) -> Self {
        self.variant = variant;
        self.variant_is_set = true;
        self
    }

    /// Named recipe from `Theme.components.text_field.recipes`. Stackable.
    pub fn recipe(mut self, name: impl Into<SharedString>) -> Self {
        self.recipes.push(name.into());
        self
    }

    /// The field text size — forwarded to the inner [`Input`].
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// The field text's font family — forwarded to the inner [`Input`], so
    /// the query, placeholder and caret measurement all use it (see
    /// [`Input::font_family`]). Not a v3 prop; v3 sets it with a class.
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// The box's corner radius — forwarded to the inner [`Input`].
    pub fn radius(mut self, r: impl Into<Pixels>) -> Self {
        self.radius = Some(r.into());
        self
    }

    /// Makes the field fill its parent's width.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// Replaces the 36px box height of the inner field.
    pub fn height(mut self, h: impl Into<Pixels>) -> Self {
        self.field.height = Some(h.into());
        self
    }

    /// Replaces the box's `px-3` horizontal padding.
    pub fn padding_x(mut self, p: impl Into<Pixels>) -> Self {
        self.field.padding_x = Some(p.into());
        self
    }

    /// Renders the box with no background, border, field shadow or focus ring,
    /// for a caller painting around it.
    pub fn is_bare(mut self, v: bool) -> Self {
        self.field.is_bare = v;
        self.field.is_bare_is_set = true;
        self
    }

    /// Shows or hides only the search field's visual focus ring. The field
    /// remains focusable, editable and accessible when set to `false`.
    pub fn focus_ring(mut self, v: bool) -> Self {
        self.field.focus_ring = Some(v);
        self
    }

    /// Sets whether the field is disabled.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets the handler called with the new value whenever it changes.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(std::sync::Arc::new(handler));
        self
    }

    /// Sets the handler called with the current value when Enter is pressed.
    pub fn on_submit(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_submit = Some(std::sync::Arc::new(handler));
        self
    }

    /// `name` — see [`Input::name`].
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// `defaultValue` — see [`Input::default_value`].
    pub fn default_value(mut self, text: impl Into<SharedString>) -> Self {
        self.default_value = Some(text.into());
        self
    }

    /// `value` — see [`Input::value`].
    pub fn value(mut self, text: impl Into<SharedString>) -> Self {
        self.value = Some(text.into());
        self
    }

    /// `SearchField.SearchIcon` — replaces the leading magnifier.
    pub fn search_icon(mut self, el: impl IntoElement) -> Self {
        self.search_icon = Some(el.into_any_element());
        self
    }

    /// Replaces the glyph inside `SearchField.ClearButton`.
    pub fn clear_icon(mut self, el: impl IntoElement) -> Self {
        self.clear_icon = Some(el.into_any_element());
        self
    }

    /// Trailing content inside the field — v3 composes a `Kbd` here.
    pub fn end_content(mut self, el: impl IntoElement) -> Self {
        self.end_content = Some(el.into_any_element());
        self
    }

    /// `validationBehavior` — see [`Input::validation_behavior`].
    pub fn validation_behavior(mut self, behavior: crate::form::ValidationBehavior) -> Self {
        self.validation_behavior = Some(behavior);
        self
    }

    /// `validate` — see [`Input::validate`].
    pub fn validate(mut self, f: impl Fn(&str) -> Option<SharedString> + 'static) -> Self {
        self.validate = Some(std::sync::Arc::new(f));
        self
    }

    /// `validationErrors` — see [`Input::validation_errors`].
    pub fn validation_errors(
        mut self,
        errors: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.validation_errors = errors.into_iter().map(Into::into).collect();
        self
    }

    /// `autoFocus` — take focus on the first render.
    pub fn auto_focus(mut self, v: bool) -> Self {
        self.auto_focus = v;
        self
    }

    /// `isReadOnly` — the field shows its value but cannot be edited.
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.is_read_only = v;
        self
    }

    /// `isRequired` — marks the label.
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// `isInvalid` — applies the danger treatment.
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// Called after the clear button empties the field.
    pub fn on_clear(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(std::sync::Arc::new(handler));
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the search field's root element after every value the
    /// variant and the active theme chose, so they win. Held here and handed
    /// to the [`Input`] this field builds at render time, whose render
    /// applies it.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl RenderOnce for SearchField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.colors();
        let validate = self.validate.clone();
        // `useSearchField` defaults `type` to `'search'`, so the rendered
        // input is `<input type="search">` and its role is the search one.
        let mut input = Input::new(self.state)
            .input_type(InputType::Search)
            .when_some(self.content, |i, render| {
                i.field_content(move |state| {
                    let is_empty = state.value.is_empty();
                    render(SearchFieldRenderState {
                        is_disabled: state.is_disabled,
                        is_invalid: state.is_invalid,
                        is_read_only: state.is_read_only,
                        is_required: state.is_required,
                        is_focused: state.focus.is_focused,
                        is_focus_within: state.focus.is_focus_within,
                        is_focus_visible: state.focus.is_focus_visible,
                        value: state.value,
                        is_empty,
                    })
                })
            })
            .placeholder(if self.placeholder_is_set {
                self.placeholder
            } else {
                crate::i18n::ui_string(crate::i18n::UiString::Search, cx)
            })
            .when_some(self.name, |i, n| i.name(n))
            .when_some(self.value, |i, v| i.value(v))
            .when_some(self.default_value, |i, v| i.default_value(v))
            .when_some(self.validation_behavior, |i, b| i.validation_behavior(b))
            .with_recipes(self.recipes)
            .is_disabled(self.is_disabled)
            .is_read_only(self.is_read_only)
            .is_required(self.is_required)
            .is_invalid(self.is_invalid)
            .validation_errors(self.validation_errors.clone())
            .auto_focus(self.auto_focus)
            .when_some(validate, |i, f| i.validate(move |v| f(v)))
            .is_clearable(true)
            .start_content(match self.search_icon {
                Some(icon) => icon,
                None => gpui::svg()
                    .size(crate::util::FIELD_ICON)
                    .path(crate::icons::SEARCH)
                    .flex_shrink_0()
                    .text_color(colors.muted)
                    .into_any_element(),
            });
        if self.variant_is_set {
            input = input.variant(self.variant);
        }
        if let Some(size) = self.text_size {
            input = input.text_size(size);
        }
        if let Some(family) = self.font_family {
            input = input.font_family(family);
        }
        if let Some(r) = self.radius {
            input = input.radius(r);
        }
        input = input.with_field_box(self.field);
        if let Some(icon) = self.clear_icon {
            input = input.clear_content(icon);
        }
        if let Some(end) = self.end_content {
            input = input.end_content(end);
        }

        if self.full_width {
            input = input.full_width();
        }
        if let Some(label) = self.label {
            input = input.label(label);
        }
        if let Some(description) = self.description {
            input = input.description(description);
        }

        // React Aria reports `onClear` only for the clear affordance or its
        // Escape shortcut, never because ordinary editing reached "".
        input = input
            .clear_on_escape(true)
            .when_some(self.on_clear, |input, on_clear| input.on_clear(on_clear));
        if let Some(on_change) = self.on_change {
            input = input.on_change(move |text, window, cx| on_change(text, window, cx));
        }

        if let Some(on_submit) = self.on_submit {
            input = input.on_submit(move |text, window, cx| on_submit(text, window, cx));
        }

        // The `sx` slot rides the inner field, whose render applies it to the
        // root it returns — the same place a plain `Input`'s slot lands.
        input = input.sx_refinement(self.sx);

        input.render(window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Backspace, Delete and the arrow keys step by extended grapheme
    /// cluster: an emoji with a skin-tone modifier, a ZWJ family, a flag and
    /// a decomposed accent are each one caret stop and one deletion.
    #[gpui::test]
    fn editing_steps_by_grapheme_cluster(cx: &mut gpui::TestAppContext) {
        // "e" + COMBINING ACUTE, thumbs-up + skin tone, family ZWJ sequence,
        // the Ukrainian flag (two regional indicators).
        let value = "ae\u{301}b\u{1F44D}\u{1F3FD}\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{1F1FA}\u{1F1E6}";
        let state = cx.new(|cx| InputState::with_value(cx, value));
        cx.update(|cx| {
            state.update(cx, |s, _| {
                let len = s.value.chars().count();
                s.cursor = len;
                // One Backspace removes the whole flag.
                assert!(backspace(s));
                assert_eq!(s.value, value.trim_end_matches(['\u{1F1FA}', '\u{1F1E6}']));
                // One Left skips the whole ZWJ family; Delete removes it.
                move_left(s, false);
                assert!(delete(s));
                assert_eq!(s.value, "ae\u{301}b\u{1F44D}\u{1F3FD}");
                // Backspace takes the thumbs-up and its modifier together.
                move_right(s, false);
                assert!(backspace(s));
                assert_eq!(s.value, "ae\u{301}b");
                // Left from the end lands before "b", then before "é" as one
                // stop; Right returns past the combining mark, never inside.
                move_left(s, false);
                move_left(s, false);
                assert_eq!(s.cursor, 1);
                move_right(s, false);
                assert_eq!(s.cursor, 3);
                // Backspace removes the base letter and its accent at once.
                assert!(backspace(s));
                assert_eq!(s.value, "ab");
                assert_eq!(s.cursor, 1);
                // Shift+Right extends the selection by one cluster.
                move_right(s, true);
                assert_eq!(s.selection(), Some((1, 2)));
            });
        });
    }

    #[gpui::test]
    fn platform_multi_character_insert_keeps_all_characters(cx: &mut gpui::TestAppContext) {
        use gpui::InputHandler;
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let state = cx.new(|cx| InputState::new(cx));
            let mut handler = PlatformTextInput {
                state: state.clone(),
                on_edit: crate::util::shared(|_, _, _| {}),
                input_type: InputType::Text,
                max_length: None,
                multiline: false,
                bounds: gpui::Bounds::default(),
                font: window.text_style().font(),
                paragraphs: cx.new(|_| Vec::new()),
            };
            handler.replace_text_in_range(None, "Go", window, cx);
            assert_eq!(state.read(cx).value(), "Go");
            assert_eq!(
                handler
                    .selected_text_range(false, window, cx)
                    .unwrap()
                    .range,
                2..2
            );
        });
    }

    #[gpui::test]
    fn platform_composition_replaces_marked_utf16_ranges(cx: &mut gpui::TestAppContext) {
        use gpui::InputHandler;
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let state = cx.new(|cx| InputState::with_value(cx, "a😀z"));
            let mut handler = PlatformTextInput {
                state: state.clone(),
                on_edit: crate::util::shared(|_, _, _| {}),
                input_type: InputType::Text,
                max_length: None,
                multiline: false,
                bounds: gpui::Bounds::default(),
                font: window.text_style().font(),
                paragraphs: cx.new(|_| Vec::new()),
            };
            handler.set_selected_text_range(1..3, window, cx);
            handler.replace_and_mark_text_in_range(None, "に", Some(1..1), window, cx);
            assert_eq!(state.read(cx).value(), "aにz");
            assert_eq!(handler.marked_text_range(window, cx), Some(1..2));
            handler.replace_and_mark_text_in_range(None, "日本😀", Some(2..4), window, cx);
            assert_eq!(state.read(cx).value(), "a日本😀z");
            assert_eq!(handler.marked_text_range(window, cx), Some(1..5));
            assert_eq!(
                handler
                    .selected_text_range(false, window, cx)
                    .unwrap()
                    .range,
                3..5
            );
            let mut actual = None;
            assert_eq!(
                handler
                    .text_for_range(3..5, &mut actual, window, cx)
                    .as_deref(),
                Some("😀")
            );
            assert_eq!(actual, Some(3..5));
            handler.replace_text_in_range(None, "東京", window, cx);
            assert_eq!(state.read(cx).value(), "a東京z");
            assert_eq!(handler.marked_text_range(window, cx), None);
            assert_eq!(
                handler
                    .selected_text_range(false, window, cx)
                    .unwrap()
                    .range,
                3..3
            );
            handler.replace_text_in_range(Some(1..3), "", window, cx);
            assert_eq!(state.read(cx).value(), "az");
        });
    }

    #[gpui::test]
    fn platform_edits_preserve_filters_and_read_only_state(cx: &mut gpui::TestAppContext) {
        use gpui::InputHandler;
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let state = cx.new(|cx| InputState::with_value(cx, "12"));
            let changes = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let recorded = changes.clone();
            let mut handler = PlatformTextInput {
                state: state.clone(),
                on_edit: crate::util::shared(move |value, _, _| {
                    recorded.borrow_mut().push(value.to_owned());
                }),
                input_type: InputType::Number,
                max_length: Some(3),
                multiline: false,
                bounds: gpui::Bounds::default(),
                font: window.text_style().font(),
                paragraphs: cx.new(|_| Vec::new()),
            };
            handler.set_selected_text_range(0..2, window, cx);
            handler.replace_text_in_range(None, "abc", window, cx);
            assert_eq!(state.read(cx).value(), "12");
            assert_eq!(
                handler
                    .selected_text_range(false, window, cx)
                    .unwrap()
                    .range,
                0..2
            );
            handler.replace_text_in_range(None, "3x456", window, cx);
            assert_eq!(state.read(cx).value(), "345");
            assert_eq!(changes.borrow().as_slice(), ["345"]);
            state.update(cx, |state, _| state.set_read_only(true));
            handler.replace_text_in_range(Some(0..3), "7", window, cx);
            assert_eq!(state.read(cx).value(), "345");
            assert_eq!(changes.borrow().as_slice(), ["345"]);
            assert!(!handler.accepts_text_input(window, cx));
        });
    }

    #[gpui::test]
    fn platform_geometry_uses_rendered_row_spacing_and_password_offsets(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::InputHandler;
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let state = cx.new(|cx| InputState::with_value(cx, "😀secret"));
            let origin = gpui::point(px(30.), px(70.));
            let bounds = gpui::Bounds::new(origin, gpui::size(px(80.), px(20.)));
            let mut handler = PlatformTextInput {
                state: state.clone(),
                on_edit: crate::util::shared(|_, _, _| {}),
                input_type: InputType::Password,
                max_length: None,
                multiline: false,
                bounds,
                font: window.text_style().font(),
                paragraphs: cx.new(|_| vec![bounds]),
            };
            let caret = handler.bounds_for_range(2..2, window, cx).unwrap();
            assert!(caret.left() > origin.x);
            assert_eq!(caret.top(), origin.y);
            assert_eq!(
                handler.character_index_for_point(caret.origin, window, cx),
                Some(2)
            );
            state.update(cx, |state, _| state.set_value(""));
            assert_eq!(
                handler.bounds_for_range(0..0, window, cx).unwrap().origin,
                origin
            );
            state.update(cx, |state, _| {
                state.set_value("one two three four five six seven eight nine ten");
            });
            handler.input_type = InputType::Text;
            handler.multiline = true;
            let text = state.read(cx).value.clone();
            let run = gpui::TextRun {
                len: text.len(),
                font: handler.font.clone(),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let shaped = window
                .text_system()
                .shape_text(
                    text.clone().into(),
                    crate::util::FIELD_TEXT,
                    &[run],
                    Some(bounds.size.width),
                    None,
                )
                .unwrap();
            let third_row = (0..text.len())
                .find(|&index| {
                    shaped[0]
                        .position_for_index(index, px(20.))
                        .is_some_and(|point| point.y == px(40.))
                })
                .expect("fixture must wrap into at least three rows");
            let caret = handler
                .bounds_for_range(third_row..third_row, window, cx)
                .unwrap();
            assert_eq!(caret.top(), origin.y + px(40.));
            assert_eq!(caret.size.height, px(20.));
        });
    }

    #[gpui::test]
    fn composition_enter_does_not_submit_or_insert_a_newline(cx: &mut gpui::TestAppContext) {
        struct View {
            state: Entity<InputState>,
            submitted: std::rc::Rc<std::cell::Cell<usize>>,
            multiline: bool,
        }
        impl Render for View {
            fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
                let submitted = self.submitted.clone();
                Input::new(self.state.clone())
                    .multiline(self.multiline)
                    .on_submit(move |_, _, _| {
                        submitted.set(submitted.get() + 1);
                    })
            }
        }
        cx.update(herogpui_theme::ThemeProvider::init);
        for multiline in [false, true] {
            let state = cx.new(|cx| InputState::with_value(cx, "ab日本z"));
            let submitted = std::rc::Rc::new(std::cell::Cell::new(0));
            let window = cx.add_window(|window, cx| {
                let focus = state.read(cx).focus_handle.clone();
                window.focus(&focus, cx);
                View {
                    state: state.clone(),
                    submitted: submitted.clone(),
                    multiline,
                }
            });
            let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
            visual.update(|window, cx| {
                state.update(cx, |state, cx| {
                    state.marked = Some(2..4);
                    state.cursor = 4;
                    cx.notify();
                });
                window.refresh();
            });
            visual.simulate_keystrokes("enter");
            assert_eq!(submitted.get(), 0);
            assert_eq!(
                state.read_with(&visual, |state, _| state.value.clone()),
                "ab日本z"
            );
            visual.simulate_keystrokes("escape");
            assert_eq!(
                state.read_with(&visual, |state, _| state.marked.clone()),
                None
            );
        }
    }

    fn check(
        value: &str,
        ty: InputType,
        min_length: Option<usize>,
        min: Option<f64>,
        max: Option<f64>,
        step: Option<f64>,
    ) -> InputValidity {
        validate_value(value, ty, min_length, min, max, step, None)
    }

    #[test]
    fn empty_is_valid_regardless_of_bounds() {
        // Emptiness is `is_required`'s business, not the bounds'.
        assert_eq!(
            check(
                "",
                InputType::Number,
                Some(5),
                Some(10.0),
                Some(20.0),
                Some(2.0)
            ),
            InputValidity::Valid
        );
    }

    #[test]
    fn min_length_counts_chars_not_bytes() {
        assert_eq!(
            check("ab", InputType::Text, Some(3), None, None, None),
            InputValidity::TooShort
        );
        assert_eq!(
            check("abc", InputType::Text, Some(3), None, None, None),
            InputValidity::Valid
        );
        // Two-byte chars still count as one each.
        assert_eq!(
            check("éé", InputType::Text, Some(3), None, None, None),
            InputValidity::TooShort
        );
    }

    #[test]
    fn numeric_bounds_are_inclusive() {
        let n = InputType::Number;
        assert_eq!(
            check("9", n, None, Some(10.0), None, None),
            InputValidity::BelowMin
        );
        assert_eq!(
            check("10", n, None, Some(10.0), None, None),
            InputValidity::Valid
        );
        assert_eq!(
            check("20", n, None, None, Some(20.0), None),
            InputValidity::Valid
        );
        assert_eq!(
            check("21", n, None, None, Some(20.0), None),
            InputValidity::AboveMax
        );
    }

    #[test]
    fn bounds_are_ignored_for_non_numeric_types() {
        // A text field holding "5" is not subject to `min`.
        assert_eq!(
            check("5", InputType::Text, None, Some(10.0), None, None),
            InputValidity::Valid
        );
    }

    #[test]
    fn unparsable_numeric_input_is_not_a_bounds_error() {
        assert_eq!(
            check("-", InputType::Number, None, Some(0.0), Some(9.0), None),
            InputValidity::Valid
        );
    }

    #[test]
    fn step_is_measured_from_min() {
        let n = InputType::Number;
        // Steps of 3 from 1: 1, 4, 7 ...
        assert_eq!(
            check("4", n, None, Some(1.0), None, Some(3.0)),
            InputValidity::Valid
        );
        assert_eq!(
            check("5", n, None, Some(1.0), None, Some(3.0)),
            InputValidity::OffStep
        );
        // With no min, the base is 0.
        assert_eq!(
            check("6", n, None, None, None, Some(3.0)),
            InputValidity::Valid
        );
        assert_eq!(
            check("5", n, None, None, None, Some(3.0)),
            InputValidity::OffStep
        );
    }

    #[test]
    fn fractional_steps_tolerate_float_error() {
        assert_eq!(
            check("0.3", InputType::Number, None, None, None, Some(0.1)),
            InputValidity::Valid
        );
    }

    #[test]
    fn zero_step_is_ignored_rather_than_dividing_by_zero() {
        assert_eq!(
            check("5", InputType::Number, None, None, None, Some(0.0)),
            InputValidity::Valid
        );
    }

    #[test]
    fn pattern_is_checked_before_the_other_rules() {
        let deny = |_: &str| false;
        assert_eq!(
            validate_value(
                "ab",
                InputType::Text,
                Some(10),
                None,
                None,
                None,
                Some(&deny)
            ),
            InputValidity::PatternMismatch
        );
        let allow = |v: &str| v.starts_with('a');
        assert_eq!(
            validate_value("abc", InputType::Text, None, None, None, None, Some(&allow)),
            InputValidity::Valid
        );
    }

    #[test]
    fn validity_helper_reports_valid() {
        assert!(InputValidity::Valid.is_valid());
        assert!(!InputValidity::TooShort.is_valid());
    }

    #[test]
    fn number_type_rejects_letters() {
        assert!(accepts_char(2, 0, '3', InputType::Number, None));
        assert!(accepts_char(2, 0, '-', InputType::Number, None));
        assert!(accepts_char(2, 0, '.', InputType::Number, None));
        assert!(!accepts_char(2, 0, 'a', InputType::Number, None));
    }

    #[test]
    fn text_type_accepts_anything() {
        assert!(accepts_char(2, 0, 'c', InputType::Text, None));
        assert!(accepts_char(2, 0, '!', InputType::Text, None));
    }

    #[test]
    fn no_max_length_never_blocks() {
        assert!(accepts_char(9999, 0, 'x', InputType::Text, None));
    }

    #[test]
    fn max_length_blocks_at_the_limit() {
        assert!(accepts_char(2, 0, 'c', InputType::Text, Some(3)));
        assert!(!accepts_char(3, 0, 'd', InputType::Text, Some(3)));
        // Already over the limit (e.g. set_value bypassed the gate).
        assert!(!accepts_char(4, 0, 'd', InputType::Text, Some(3)));
    }

    #[test]
    fn max_length_counts_a_replaced_selection_as_free() {
        // 3 chars, all selected: the keystroke replaces them, so it fits.
        assert!(accepts_char(3, 3, 'd', InputType::Text, Some(3)));
        // Only one selected: 2 survive, still room for one more.
        assert!(accepts_char(3, 1, 'd', InputType::Text, Some(3)));
    }

    #[test]
    fn type_filter_wins_over_available_room() {
        assert!(!accepts_char(0, 0, 'a', InputType::Number, Some(10)));
    }

    #[test]
    fn password_masking_preserves_char_count() {
        let value = "sécret";
        let masked = "•".repeat(value.chars().count());
        assert_eq!(masked.chars().count(), value.chars().count());
    }

    #[test]
    fn word_motion_crosses_the_separators_then_the_word() {
        let v = "hello world, again";
        assert_eq!(word_target(v, 0, true), 5); // end of "hello"
        assert_eq!(word_target(v, 5, true), 11); // end of "world"
        assert_eq!(word_target(v, 11, false), 6); // start of "world"
        assert_eq!(word_target(v, 0, false), 0); // nowhere left to go
        assert_eq!(word_target(v, 18, true), 18);
    }

    #[test]
    fn line_bounds_exclude_the_newlines() {
        let value = "one
two
three";
        assert_eq!(line_bounds(value, 0), (0, 3));
        assert_eq!(line_bounds(value, 5), (4, 7));
        assert_eq!(line_bounds(value, 13), (8, 13));
    }

    #[test]
    fn vertical_motion_keeps_the_column_where_the_line_is_long_enough() {
        let v = "hello
hi
world";
        assert_eq!(vertical_target(v, 3, true), 8); // "hi" is shorter: its end
        assert_eq!(vertical_target(v, 8, true), 11); // column 2 of "world"
        assert_eq!(vertical_target(v, 11, false), 8);
    }

    #[test]
    fn vertical_motion_runs_to_the_ends_at_the_edges() {
        let v = "one
two";
        assert_eq!(vertical_target(v, 1, false), 0);
        assert_eq!(vertical_target(v, 5, true), 7);
    }

    #[test]
    fn a_selection_slices_on_char_boundaries() {
        assert_eq!(
            slice_selection("hello world", Some((6, 11))).as_deref(),
            Some("world")
        );
        // Multi-byte: the indices are chars, not bytes.
        assert_eq!(
            slice_selection("sécret", Some((0, 2))).as_deref(),
            Some("sé")
        );
        assert_eq!(slice_selection("hello", None), None);
    }
}

// The pinned `.search-field__clear-button` composes `.close-button`, whose
// hover fills `bg-default-hover`; a hand-mixed wash looks plausible on
// screen, so the check is mechanical.
#[cfg(test)]
mod hover_tokens {
    #[test]
    fn the_field_uses_the_pinned_hover_transition() {
        let source = include_str!("input.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("colors.field.hover()")
                && source.contains("colors.default.hover()")
                && source.contains("colors.field.border_hover()")
        );
        assert!(
            source.contains("hover_fade_with_duration_and_easing")
                && source.contains("HoverFadeEasing::EaseSmooth")
                && source.contains("Some(150)")
        );
    }

    #[test]
    fn the_clear_button_hovers_the_role_hover_token() {
        // Scan the implementation only; this test's own text names the
        // forbidden accessor.
        let source = include_str!("input.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains(".hover(move |s| s.bg(clear_hover_bg))")
                && source.contains(
                    "let clear_hover_bg = self.clear_hover_bg.unwrap_or(colors.default.hover());"
                ),
            "the clear button must hover `bg-default-hover` \
             (pinned `.close-button:hover`)"
        );
        assert!(
            !source.contains("with_alpha(0.15)"),
            "the clear button must not hover a hand-mixed wash"
        );
    }
}

crate::util::impl_component_styled!(Input, SearchField);
