//! InputOTP — port of `@heroui/input-otp`.

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    prelude::*, px, Animation, AnimationExt, AnyElement, App, Entity, FocusHandle, Focusable,
    IntoElement, KeyDownEvent, Pixels, RenderOnce, SharedString, Styled, Window,
};
use herogpui_core::{element_id, FieldVariant};
use herogpui_theme::ActiveTheme;

use crate::a11y::A11y as _;

/// Editable state for an OTP field: one char per cell.
/// Which characters an OTP cell accepts (`pattern`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OtpPattern {
    /// `0-9` — the v3 default.
    #[default]
    Digits,
    /// `0-9A-Za-z`
    Alphanumeric,
    /// Any printable character.
    Any,
}

impl OtpPattern {
    /// Every pattern, in display order.
    pub const ALL: [OtpPattern; 3] = [
        OtpPattern::Digits,
        OtpPattern::Alphanumeric,
        OtpPattern::Any,
    ];

    /// Whether `ch` may be entered into a cell.
    pub fn accepts(self, ch: char) -> bool {
        match self {
            OtpPattern::Digits => ch.is_ascii_digit(),
            OtpPattern::Alphanumeric => ch.is_ascii_alphanumeric(),
            OtpPattern::Any => !ch.is_control(),
        }
    }

    /// The display name of this pattern.
    pub fn label(self) -> &'static str {
        match self {
            OtpPattern::Digits => "Digits",
            OtpPattern::Alphanumeric => "Alphanumeric",
            OtpPattern::Any => "Any",
        }
    }
}

/// State of an OTP input: the entered characters, the cursor and focus.
pub struct OtpState {
    cells: Vec<char>,
    cursor: usize,
    pub(crate) focus_handle: FocusHandle,
    /// Disabled native controls are omitted from FormData and native form validation.
    /// Written by `InputOTP::render` for the registered FormField to read.
    is_successful: bool,
    /// Resolved component validation, read by native Form submission.
    validity: crate::validation::Validity,
    /// Server messages routed to this field by its `Form`'s
    /// `validationErrors` record (`form.rs`). The form writes them on a new
    /// record; an accepted edit in any cell clears them — v3: "displayed
    /// immediately and cleared when user modifies the field".
    routed_errors: Vec<SharedString>,
    /// The delivery receipt for `routed_errors`: the record revision these
    /// messages last came from, `0` before anything arrived. The form's
    /// delivery consults it — a record the receipt already names is a clone
    /// of one already delivered and re-arms nothing — and an accepted edit
    /// clears the messages without rewinding it, so the next frame's clone
    /// cannot resurrect what the keystroke answered.
    routed_revision: u64,
}

impl OtpState {
    /// Fills the cells from `code`, padding with blanks and dropping any
    /// overflow.
    pub fn set_code(&mut self, code: &str) {
        let len = self.cells.len();
        let mut chars = code.chars();
        for i in 0..len {
            self.cells[i] = chars.next().unwrap_or(' ');
        }
        self.cursor = code.chars().count().min(len.saturating_sub(1));
    }

    /// `length` = number of cells (HeroUI default 4).
    pub fn with_length(cx: &mut App, length: usize) -> Self {
        Self {
            cells: vec![' '; length.max(1)],
            cursor: 0,
            // A field is a tab stop: the handle carries that, not the element.
            focus_handle: cx.focus_handle().tab_stop(true),
            is_successful: true,
            validity: crate::validation::Validity::default(),
            routed_errors: Vec::new(),
            routed_revision: 0,
        }
    }

    /// Returns the entered characters as a string, omitting empty cells.
    pub fn code(&self) -> String {
        self.cells.iter().filter(|c| **c != ' ').collect()
    }

    /// Returns whether every cell is filled.
    pub fn is_complete(&self) -> bool {
        self.cells.iter().all(|c| *c != ' ')
    }

    /// Empties every cell and resets the cursor.
    pub fn clear(&mut self) {
        self.cells.iter_mut().for_each(|c| *c = ' ');
        self.cursor = 0;
    }

    pub(crate) fn is_successful(&self) -> bool {
        self.is_successful
    }

    pub(crate) fn set_successful(&mut self, is_successful: bool) {
        self.is_successful = is_successful;
    }

    pub(crate) fn validity(&self) -> &crate::validation::Validity {
        &self.validity
    }

    pub(crate) fn set_validity(&mut self, validity: crate::validation::Validity) {
        self.validity = validity;
    }

    /// The server messages the form routed to this field, as last written by
    /// the form's `validationErrors` delivery — what this field's error slot
    /// renders. Empty once an accepted edit suppressed them or a reset hid
    /// them.
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

    /// Suppresses the routed server messages after an accepted edit. The
    /// delivery receipt is deliberately untouched — the record that delivered
    /// already named this field, so the next frame's clone must not resurrect
    /// what the keystroke answered.
    pub(crate) fn clear_routed_errors(&mut self) {
        self.routed_errors.clear();
    }
}

impl Focusable for OtpState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

type OnComplete = std::sync::Arc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

/// An accepted edit — a typed digit, a paste, a backspace that removed
/// something — suppresses the routed server messages (v3: server errors are
/// "cleared when user modifies the field"). A rejected keystroke is not an
/// edit and clears nothing.
fn suppress_routed_errors(state: &Entity<OtpState>, cx: &mut App) {
    if !state.read(cx).routed_errors().is_empty() {
        state.update(cx, |state, cx| {
            state.clear_routed_errors();
            cx.notify();
        });
    }
}

/// An accepted edit also refreshes the stored validity mirror. `render`
/// writes that mirror, so without this it is one frame old — and a
/// completion handler may submit the form synchronously (the one-time-code
/// auto-submit v3's `onComplete` invites) before any frame catches up. The
/// routed messages the edit just suppressed are left out, so the mirror
/// says what the field says *now*: its own `isInvalid`, `validationErrors`
/// and `validate` against the current code.
fn refresh_stored_validity(
    state: &Entity<OtpState>,
    is_invalid: bool,
    validation_errors: &[SharedString],
    validate: Option<&crate::validation::Validator<str>>,
    cx: &mut App,
) {
    let code: String = state.read(cx).code();
    let validity = crate::validation::resolve(
        is_invalid,
        validation_errors,
        validate.and_then(|f| f(code.as_str())),
        None,
    );
    if state.read(cx).validity() != &validity {
        state.update(cx, |s, _| s.set_validity(validity));
    }
}

type Slot = std::sync::Arc<dyn Fn(usize, Option<char>) -> AnyElement + 'static>;

/// HeroUI's `.input-otp__slot-value` enters over 250ms with the smooth curve.
/// GPUI's 0.3.3 public Div API does not expose a transform builder, so the
/// portable part of that endpoint is animated here through opacity. The slot
/// owner remains stable while the listener-free value child owns the animation,
/// so typing a new character cannot lose the field's input path.
const SLOT_VALUE_IN_MS: u64 = 250;

#[derive(Clone)]
struct SlotValueMotion {
    value: Option<char>,
    generation: usize,
    from: f32,
    opacity: Rc<Cell<f32>>,
}

struct SlotValueMotionFrame {
    base: gpui::ElementId,
    generation: usize,
    from: f32,
    to: f32,
    opacity: Rc<Cell<f32>>,
    animate: bool,
}

impl SlotValueMotionFrame {
    fn render(self, value: gpui::Div) -> AnyElement {
        if !self.animate {
            self.opacity.set(self.to);
            return value.opacity(self.to).into_any_element();
        }

        let opacity = self.opacity;
        let from = self.from;
        let to = self.to;
        value
            .with_animation(
                element_id::indexed(&self.base, "value-in", self.generation),
                Animation::new(Duration::from_millis(SLOT_VALUE_IN_MS))
                    .with_easing(|t| crate::anim::Curve::Smooth.at(t)),
                move |value, delta| {
                    let next = from + (to - from) * delta;
                    opacity.set(next);
                    value.opacity(next)
                },
            )
            .into_any_element()
    }
}

fn slot_value_motion(
    id: &gpui::ElementId,
    value: Option<char>,
    window: &mut Window,
    cx: &mut App,
) -> SlotValueMotionFrame {
    let state = window.use_keyed_state(element_id::scoped(id, "value-motion"), cx, |_, _| {
        let opacity = if value.is_some() { 1.0 } else { 0.0 };
        SlotValueMotion {
            value,
            generation: 0,
            from: opacity,
            opacity: Rc::new(Cell::new(opacity)),
        }
    });
    let mut current = state.read(cx).clone();
    let target = if value.is_some() { 1.0 } else { 0.0 };
    if current.value != value {
        current.value = value;
        current.generation = current.generation.wrapping_add(1);
        current.from = current.opacity.get();
        state.update(cx, |stored, _| *stored = current.clone());
    }
    if ActiveTheme::reduce_motion(cx) && (current.opacity.get() - target).abs() > f32::EPSILON {
        current.from = target;
        current.opacity.set(target);
        state.update(cx, |stored, _| *stored = current.clone());
    }
    SlotValueMotionFrame {
        base: id.clone(),
        generation: current.generation,
        from: current.from,
        to: target,
        opacity: current.opacity,
        animate: current.generation != 0
            && !ActiveTheme::reduce_motion(cx)
            && (current.from - target).abs() > f32::EPSILON,
    }
}

/// `textAlign` — where a digit sits inside its slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OtpTextAlign {
    /// Aligns text to the left.
    Left,
    /// Centers text.
    #[default]
    Center,
    /// Aligns text to the right.
    Right,
}

impl OtpTextAlign {
    /// Every alignment, in display order.
    pub const ALL: [OtpTextAlign; 3] = [
        OtpTextAlign::Left,
        OtpTextAlign::Center,
        OtpTextAlign::Right,
    ];

    /// The display name of this alignment.
    pub fn label(self) -> &'static str {
        match self {
            OtpTextAlign::Left => "Left",
            OtpTextAlign::Center => "Center",
            OtpTextAlign::Right => "Right",
        }
    }
}

/// HeroUI InputOTP.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct InputOTP {
    /// `children` on `InputOTP.Slot` — v3's render prop, handed the slot's
    /// `index` and its character.
    slot: Option<Slot>,
    /// `name` — the name this control submits under; read back by
    /// [`Self::form_field`].
    name: Option<SharedString>,
    /// `aria-label` — the explicit accessible name, which wins over the
    /// name-derived name exactly as the HTML name computation ranks it.
    aria_label: Option<SharedString>,
    variant: FieldVariant,
    /// `validate` — run by the component, not the caller.
    validate: Option<crate::validation::Validator<str>>,
    /// `validationErrors` — messages from a server round-trip.
    validation_errors: Vec<SharedString>,
    /// `textAlign` — where the digit sits inside its slot.
    text_align: OtpTextAlign,
    /// `autoFocus` — take focus on the first render.
    auto_focus: bool,
    /// `pasteTransformer` — rewrites pasted text before the slots take it.
    paste_transformer: Option<std::sync::Arc<dyn Fn(&str) -> String + 'static>>,
    is_invalid: bool,
    placeholder: Option<SharedString>,
    pattern: OtpPattern,
    on_change: Option<std::sync::Arc<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
    state: Entity<OtpState>,
    is_disabled: bool,
    separator: bool,
    on_complete: Option<OnComplete>,
    /// `value` — the controlled code, stored for the first render only.
    value: Option<String>,
    /// The fill a hovered slot takes, in place of `--default-hover`.
    slot_hover_bg: Option<gpui::Hsla>,
    /// The corner radius of each slot, in place of the owning `field_radius`
    /// helper.
    radius: Option<Pixels>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl InputOTP {
    /// `value` — v3's controlled-code spelling, as a pure builder.
    ///
    /// The bound [`OtpState`] owns the code once the field renders, so this
    /// seeds the state on the first render only — one char per cell — winning
    /// over nothing else here (InputOTP has no `defaultValue`); calling
    /// `.value(..)` twice keeps the last call, like every other builder here.
    /// A later code is an imperative update rather than a builder:
    /// `state.update(cx, |s, _| s.set_code(..))`.
    pub fn value(mut self, code: impl Into<String>) -> Self {
        self.value = Some(code.into());
        self
    }

    /// The fill a hovered slot takes, in place of `--default-hover`.
    pub fn slot_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.slot_hover_bg = Some(color.into());
        self
    }

    /// The corner radius of every slot, in place of the owning `field_radius`
    /// helper. Not a v3 prop; the removed v2 `radius` prop is prohibited and
    /// this is a per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// Creates an OTP input backed by the given state.
    pub fn new(state: Entity<OtpState>) -> Self {
        Self {
            slot: None,
            name: None,
            aria_label: None,
            variant: FieldVariant::Primary,
            validate: None,
            validation_errors: Vec::new(),
            text_align: OtpTextAlign::Center,
            auto_focus: false,
            paste_transformer: None,
            is_invalid: false,
            placeholder: None,
            pattern: OtpPattern::Digits,
            on_change: None,
            state,
            is_disabled: false,
            separator: false,
            on_complete: None,
            value: None,
            slot_hover_bg: None,
            radius: None,
            sx: None,
        }
    }

    /// `children` on `InputOTP.Slot` — replaces a slot's contents.
    ///
    /// The closure receives the slot's `index` and its character (`None` when
    /// empty), the values v3 passes into the same render prop.
    pub fn slot(mut self, render: impl Fn(usize, Option<char>) -> AnyElement + 'static) -> Self {
        self.slot = Some(std::sync::Arc::new(render));
        self
    }

    /// `name` — the name this control submits under.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// `aria-label` — the explicit accessible name, which wins over the
    /// name-derived name exactly as the HTML name computation ranks it.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// The `Form` field this control submits, when it has a `name`.
    ///
    /// v3 discovers a field through the DOM; gpui gives a child no way to reach
    /// its ancestor, so the control hands the pair over instead. Borrows, so the
    /// control is still yours to place:
    ///
    /// ```
    /// # use gpui::{prelude::*, Window};
    /// # use herogpui_components::{Form, InputOTP, OtpState};
    /// # struct Demo;
    /// # impl Render for Demo {
    /// #     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    /// #         let form = Form::new();
    /// #         let state = cx.new(|cx| OtpState::with_length(cx, 4));
    /// #         let control = InputOTP::new(state).name("code");
    /// let field = control.form_field();
    /// form.field(field.unwrap()).child(control)
    /// #     }
    /// # }
    /// # let mut tcx = gpui::TestAppContext::single();
    /// # tcx.update(herogpui_theme::ThemeProvider::init);
    /// # let _ = tcx.add_window_view(|_, _| Demo);
    /// ```
    pub fn form_field(&self) -> Option<crate::form::FormField> {
        let name = self.name.clone()?;
        let state = self.state.clone();
        Some(crate::form::FormField::code(name, state).is_required(false))
    }

    /// Sets the field variant (v3 `variant`).
    pub fn variant(mut self, variant: FieldVariant) -> Self {
        self.variant = variant;
        self
    }

    /// `validate` — returns the message to show, or `None` when the code is fine.
    ///
    /// The component runs it and surfaces the result.
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

    /// `autoFocus` — take focus on the first render.
    pub fn auto_focus(mut self, v: bool) -> Self {
        self.auto_focus = v;
        self
    }

    /// `pasteTransformer` — rewrites pasted text before it fills the slots.
    ///
    /// Useful for stripping separators from a code the user copied out of an
    /// email, e.g. `|t| t.replace('-', "")`.
    pub fn paste_transformer(mut self, f: impl Fn(&str) -> String + 'static) -> Self {
        self.paste_transformer = Some(std::sync::Arc::new(f));
        self
    }

    /// `textAlign` — where each digit sits inside its slot.
    ///
    /// v3 documents `left` as the default; a single character in a square slot
    /// reads better centred, which is what the slots render, so `Center` is the
    /// default here and the other two are available.
    pub fn text_align(mut self, align: OtpTextAlign) -> Self {
        self.text_align = align;
        self
    }

    /// Sets the invalid state (v3 `isInvalid`).
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// `placeholder` — the text shown in an empty cell.
    ///
    /// v3 documents no default: `input-otp.css` gives an empty slot nothing to
    /// draw. This port used to default it to `'-'`, which is what the docs
    /// table prints in its *Default* column to mean "none" — so every unfilled
    /// cell showed a dash upstream leaves blank.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// `pattern` — the characters a cell accepts. Defaults to digits.
    pub fn pattern(mut self, pattern: OtpPattern) -> Self {
        self.pattern = pattern;
        self
    }

    /// Fires on every cell change, not just completion (`onChange`).
    pub fn on_change(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(std::sync::Arc::new(f));
        self
    }

    /// Sets the disabled state (v3 `isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// `InputOTP.Separator` — the dash between cell groups.
    ///
    /// It takes no content in v3: `.input-otp__separator` is `h-[2px] w-[6px]
    /// rounded-sm bg-separator`, a bar rather than a glyph, so this is a flag
    /// and not the string it used to accept.
    pub fn separator(mut self) -> Self {
        self.separator = true;
        self
    }

    /// Sets the handler called with the code once every cell is filled (v3 `onComplete`).
    pub fn on_complete(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_complete = Some(std::sync::Arc::new(f));
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the field's root element after every value the variant and
    /// the active theme chose, so they win. The root is the row of cells —
    /// or the column that also holds the error message, when one shows.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl RenderOnce for InputOTP {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_successful = !self.is_disabled;
        if self.state.read(cx).is_successful() != is_successful {
            self.state
                .update(cx, |state, _| state.set_successful(is_successful));
        }
        // The mount-time autofocus decision runs before the tokens. A disabled
        // field consumes the one-shot without focusing, just like a disabled
        // native input whose `autofocus` attribute does not rerun if enabled.
        // Every part of the field hangs off the state entity: an `InputOTP`
        // takes no id of its own, and the entity is the identity it does have.
        let base_id = gpui::ElementId::named_usize("otp", self.state.entity_id().as_u64() as usize);
        // `value` seeds the code once, before anything reads it. The state
        // owns the cells afterwards, and `OtpState::set_code` is the
        // imperative update. `seed_once` takes `cx` mutably, so it runs
        // before the theme tokens, like `focus_once` below.
        if let Some(code) = self.value.clone() {
            let state = self.state.clone();
            crate::util::seed_once(
                window,
                cx,
                element_id::scoped(&base_id, "default"),
                move |cx| {
                    state.update(cx, |s, cx| {
                        s.set_code(&code);
                        cx.notify();
                    });
                },
            );
        }
        let focused_handle = self.state.read(cx).focus_handle.clone();
        if self.auto_focus {
            let done =
                window.use_keyed_state(element_id::scoped(&base_id, "autofocus"), cx, |_, _| false);
            if !*done.read(cx) {
                if !self.is_disabled {
                    window.focus(&focused_handle, cx);
                }
                done.update(cx, |done, _| *done = true);
            }
        }

        // Where the row's text starts, remembered from the last frame: a
        // `canvas` is the only element that is told its own bounds, and a
        // click has to be measured against something. `use_keyed_state` takes
        // `cx` mutably, so it runs before the theme tokens — the same reason
        // `focus_once` above runs before them.
        let row_origin =
            window.use_keyed_state(element_id::scoped(&base_id, "origin"), cx, |_, _| {
                None::<Pixels>
            });

        // `.input-otp__slot` is `h-10 w-9.5` with `text-sm`, and the row and
        // group are both `gap-2`.
        let (cell_w, cell_h, text, slot_gap) = (px(38.), px(40.), px(14.), px(8.));

        let focused = focused_handle.is_focused(window);
        let (cells_snapshot, cursor) = {
            let st = self.state.read(cx);
            (st.cells.clone(), st.cursor)
        };
        let _length = cells_snapshot.len();
        let disabled = self.is_disabled;
        // Every slot in the row below paints the same corner, so it resolves
        // once here.
        let radius = self.radius.unwrap_or_else(|| crate::util::field_radius(cx));

        // v3 order: the controlled flag, then server errors, then `validate`.
        // The server slot carries the messages the `Form`'s
        // `validationErrors` record routed into this state by name, ahead of
        // this field's own `validationErrors` prop.
        let code_now = self.state.read(cx).code();
        let mut server_errors = self.state.read(cx).routed_errors().to_vec();
        server_errors.extend(self.validation_errors.iter().cloned());
        let validity = crate::validation::resolve(
            self.is_invalid,
            &server_errors,
            self.validate.as_ref().and_then(|f| f(code_now.as_str())),
            None,
        );
        if self.state.read(cx).validity() != &validity {
            self.state
                .update(cx, |state, _| state.set_validity(validity.clone()));
        }
        let invalid = validity.is_invalid;
        // Clone the small token records before per-slot keyed animation state
        // borrows `cx` mutably. This keeps the render loop free to update a
        // character's motion without holding an immutable theme borrow over it.
        let colors = cx.colors().clone();
        let layout = cx.layout().clone();

        // v3's `InputOTP` wraps the `input-otp` package, which renders one
        // real `<input>` behind the slots — `aria-placeholder`,
        // `autocomplete="one-time-code"`, no role of its own, so a text box.
        // The slots themselves are presentational divs and stay out of the
        // tree. The row here is that input.
        let name = crate::a11y::Name::field(
            self.aria_label.as_ref().or(self.name.as_ref()),
            None,
            &validity,
        );
        let mut row = gpui::div()
            .id(base_id.clone())
            .a11y_named(crate::a11y::Role::TextInput, &name)
            .a11y_text(&code_now, self.placeholder.as_ref())
            .flex()
            .items_center()
            .gap(slot_gap)
            .cursor(if disabled {
                gpui::CursorStyle::Arrow
            } else {
                gpui::CursorStyle::IBeam
            })
            // A disabled field is not a tab stop.
            .when(!self.is_disabled, |el| el.track_focus(&focused_handle))
            .key_context("InputOTP")
            .on_mouse_down(gpui::MouseButton::Left, {
                let fh = focused_handle.clone();
                let origin = row_origin.clone();
                let st = self.state.clone();
                let disabled = self.is_disabled;
                move |ev: &gpui::MouseDownEvent, window, cx| {
                    if disabled {
                        return;
                    }
                    // The click that *grants* the focus must not disturb a
                    // caret the value placed (a seeded code parks it after
                    // the last filled slot); only a click on an already
                    // focused row re-homes it, which is what v3's input-otp
                    // does when a slot is clicked.
                    let was_focused = fh.is_focused(window);
                    window.focus(&fh, cx);
                    if !was_focused {
                        return;
                    }
                    // The caret lands on the slot the click hit, measured
                    // from the row's remembered left edge. The gap after the
                    // zero-width origin item shifts each cell 8px in;
                    // flooring the pitch-scaled position still names the
                    // right cell.
                    let Some(left) = *origin.read(cx) else {
                        return;
                    };
                    let pitch = f32::from(cell_w + slot_gap);
                    let x = f32::from(ev.position.x) - f32::from(left);
                    let len = st.read(cx).cells.len();
                    let cell = (x / pitch).floor() as i32;
                    st.update(cx, |s, cx| {
                        s.cursor = cell.clamp(0, len as i32 - 1) as usize;
                        cx.notify();
                    });
                }
            });

        if disabled {
            row = row.opacity(layout.disabled_opacity);
        }

        // A zero-width item at the row's head: its bounds give the row's left
        // edge, which the click handler above measures against (it took its
        // own clone). A row starts with this, then its cells, so cell *i*
        // spans `i*46 + 8` px in.
        row = row.child(
            gpui::canvas(
                move |bounds, _window, cx| {
                    let left = bounds.origin.x;
                    if *row_origin.read(cx) != Some(left) {
                        row_origin.update(cx, |v, _| *v = Some(left));
                    }
                },
                |_, _, _, _| {},
            )
            .w(px(0.))
            .h(px(0.))
            .flex_shrink_0(),
        );

        for (i, cell_ch) in cells_snapshot.iter().enumerate() {
            // group separator every 3 cells
            if i > 0 && i % 3 == 0 && self.separator {
                row = row.child(
                    gpui::div()
                        .flex_shrink_0()
                        .w(px(6.))
                        .h(px(2.))
                        .rounded(crate::util::hairline_radius(cx))
                        .bg(colors.separator),
                );
            }

            let ch = *cell_ch;
            let is_cursor_cell = focused && i == cursor && !disabled;
            let filled = ch != ' ';
            // The slot carries its own id because its hover listener needs
            // element state: gpui wires hover listeners only for elements the
            // tree can name, and the chrome ramp below tracks hover through
            // one. The id derives from the row's, so instances never share a
            // timeline.
            let slot_id = element_id::indexed(&base_id, "slot", i);

            let mut cell = gpui::div()
                .id(slot_id.clone())
                .flex()
                .items_center()
                .relative()
                // `textAlign` positions the digit inside its slot.
                .map(|c| match self.text_align {
                    OtpTextAlign::Left => c.justify_start().pl(px(6.)),
                    OtpTextAlign::Center => c.justify_center(),
                    OtpTextAlign::Right => c.justify_end().pr(px(6.)),
                })
                .w(cell_w)
                .h(cell_h)
                .rounded(radius)
                .text_size(text)
                .line_height(px(20.))
                .font_weight(gpui::FontWeight::SEMIBOLD);

            // Every slot is filled and shadowed, empty or not. The pinned CSS
            // gives the slot the theme field border width/color, then changes
            // its background by variant and active/filled state — and
            // transitions all three chrome properties rather than swapping
            // them, so the endpoints resolve here and the shared chrome ramp
            // (`.input-otp__slot`, lines 28-32: `background-color 150ms
            // var(--ease-smooth), border-color 150ms var(--ease-smooth),
            // box-shadow 150ms var(--ease-out)`) interpolates between them.
            let slot_bg = match self.variant {
                FieldVariant::Primary => colors.field.background,
                FieldVariant::Secondary => colors.default.color,
            };
            let active_bg = match self.variant {
                FieldVariant::Primary => colors.field.focus(),
                FieldVariant::Secondary => colors.default.color,
            };
            // HeroUI's invalid rule comes after active and filled rules:
            // every invalid slot keeps the focus background and receives
            // the danger outline, including the keyboard-active slot — which
            // is why the ring below is skipped once `invalid` holds.
            // v3's `[data-active]` slot rings on pointer focus too: `isActive`
            // is "input focused && caret on this slot" on any modality, the
            // same focus-within rule NumberField and Input already follow —
            // `focus_visible` would wrongly gate it to keyboard.
            let focus_ring =
                (!invalid && is_cursor_cell).then(|| crate::anim::focus_ring_endpoint(cx));
            let idle = crate::anim::FieldChrome {
                bg: if invalid {
                    colors.field.focus()
                } else if is_cursor_cell || filled {
                    active_bg
                } else {
                    slot_bg
                },
                border: if invalid {
                    colors.danger.color
                } else {
                    colors.field.border
                },
                border_width: if invalid {
                    layout.border_width.max(px(1.))
                } else {
                    layout.field_border_width
                },
                ring: focus_ring,
            };
            // `--input-otp-slot-bg-hover` is `--default-hover` for the
            // secondary variant; primary slots use the field hover token.
            // Active and filled slots keep their focus background while their
            // hover border still follows the shared field token.
            let hover_bg = self.slot_hover_bg.unwrap_or(match self.variant {
                FieldVariant::Primary => colors.field.hover(),
                FieldVariant::Secondary => colors.default.hover(),
            });
            let hovered_bg = if is_cursor_cell || filled {
                idle.bg
            } else {
                hover_bg
            };
            let hovered = (!self.is_disabled).then(|| crate::anim::FieldChrome {
                bg: hovered_bg,
                border: colors.field.border_hover(),
                border_width: idle.border_width,
                ring: focus_ring,
            });
            // The settled slot chrome comes from the endpoints themselves, and
            // the ring rides `status-focused-field` through the shared painter
            // on top of the slot's constant field shadow — the same paint the
            // other field parts take.
            let base_shadows = match self.variant {
                FieldVariant::Primary => layout.field_shadow.clone(),
                FieldVariant::Secondary => Vec::new(),
            };
            cell = cell
                .bg(idle.bg)
                .border(idle.border_width)
                .border_color(idle.border);
            // The instant ring the first frame casts is an overlay child,
            // concentric with the slot's own `radius`; the flush geometry takes
            // no offset gap, so the band sits straight on the slot edge.
            cell = crate::util::with_focus_ring_overlay(
                cell,
                focus_ring.is_some(),
                false,
                radius,
                base_shadows.clone(),
                cx,
            );
            // The ramp owns the slot's border and ring past its first flip,
            // so the instant ones it painted above stop being cast there.
            // The cell is not `overflow-hidden`, so neither flag the ramp
            // returns (whether it is painting the state ring, and the
            // clip-escaping overlay it never builds here) has a reader.
            (cell, _, _) = crate::anim::field_chrome_ramp(
                cell,
                &slot_id,
                idle,
                hovered,
                base_shadows,
                radius,
                false,
                window,
                cx,
            );
            cell = cell.text_color(colors.foreground);

            // `slot` is v3's render prop on `InputOTP.Slot`: it receives the
            // slot's `index` and its character, so a caller can draw the cell's
            // contents without re-deriving either.
            if let Some(render) = &self.slot {
                cell = cell.child(render(i, if ch == ' ' { None } else { Some(ch) }));
            } else if ch != ' ' {
                // `.input-otp__slot-value` is `text-lg leading-6`: the digit is
                // a step larger than the slot's own `text-sm`. The shared
                // motion frame covers the portable opacity part of
                // `slot-value-in`; GPUI has no public scale/translate builder.
                let value_motion = slot_value_motion(
                    &element_id::indexed(&base_id, "slot-value", i),
                    Some(ch),
                    window,
                    cx,
                );
                cell = cell.child(
                    value_motion.render(
                        gpui::div()
                            .text_size(px(18.))
                            .line_height(px(24.))
                            .child(ch.to_string()),
                    ),
                );
            } else if is_cursor_cell {
                // v3's `@keyframes caret-blink`.
                cell = cell.child(crate::anim::caret_blink(
                    // `.input-otp__caret` is `h-4 w-[2px] rounded-sm
                    // bg-field-placeholder`.
                    gpui::div()
                        .absolute()
                        .left(match self.text_align {
                            OtpTextAlign::Left => px(6.),
                            OtpTextAlign::Center => px(18.),
                            OtpTextAlign::Right => px(30.),
                        })
                        .top(px(12.))
                        .w(px(2.))
                        .h(px(16.))
                        .rounded(crate::util::hairline_radius(cx))
                        .bg(colors.field.placeholder),
                    element_id::indexed(&base_id, "caret", i),
                    cx,
                ));
            } else if let Some(placeholder) = &self.placeholder {
                // `placeholder` fills the empty, unfocused cells.
                cell = cell.text_color(colors.muted).child(placeholder.clone());
            }

            row = row.child(cell);
        }

        // editing
        let state_entity = self.state.clone();
        let on_complete = self.on_complete.clone();
        let on_change = self.on_change.clone();
        let pattern = self.pattern;
        let paste_transformer = self.paste_transformer.clone();
        // The sources an accepted edit re-resolves the stored validity from
        // (see `refresh_stored_validity`): the field's own, never the
        // routed slot the edit suppresses.
        let edit_is_invalid = self.is_invalid;
        let edit_validation_errors = self.validation_errors.clone();
        let edit_validate = self.validate.clone();
        row = row.on_key_down(move |ev: &KeyDownEvent, window, cx| {
            if disabled {
                return;
            }
            let key: &str = &ev.keystroke.key;

            // Ctrl/Cmd+V fills the slots from the clipboard. `Cmd` matters on
            // macOS; checking only `control` would make paste dead there.
            let paste_chord =
                (ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform) && key == "v";
            if paste_chord {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    let text = match &paste_transformer {
                        Some(f) => f(&text),
                        None => text,
                    };
                    let was_complete = state_entity.read(cx).is_complete();
                    let accepted = state_entity.update(cx, |s, cx| {
                        let mut accepted = false;
                        // A paste replaces the code from the cursor onward.
                        // Every pasted char goes through the same `pattern`
                        // gate a keystroke does (the typing branch calls
                        // `pattern.accepts`); a digits field used to take
                        // letters simply because they were alphanumeric.
                        for ch in text.chars() {
                            if s.cursor >= s.cells.len() {
                                break;
                            }
                            if !pattern.accepts(ch) {
                                continue;
                            }
                            accepted = true;
                            s.cells[s.cursor] = ch.to_ascii_uppercase();
                            s.cursor += 1;
                        }
                        // Leave the cursor on the last filled slot so the next
                        // keystroke overwrites rather than falling off the end.
                        if s.cursor >= s.cells.len() {
                            s.cursor = s.cells.len() - 1;
                        }
                        cx.notify();
                        accepted
                    });
                    let code: String = state_entity.read(cx).code();
                    if accepted {
                        suppress_routed_errors(&state_entity, cx);
                        refresh_stored_validity(
                            &state_entity,
                            edit_is_invalid,
                            &edit_validation_errors,
                            edit_validate.as_ref(),
                            cx,
                        );
                        if let Some(cb) = &on_change {
                            cb(&code, window, cx);
                        }
                    }
                    if !was_complete && state_entity.read(cx).is_complete() {
                        if let Some(cb) = &on_complete {
                            cb(&code, window, cx);
                        }
                    }
                }
                return;
            }

            match key {
                "backspace" => {
                    let changed = state_entity.update(cx, |s, cx| {
                        let changed = if s.cells[s.cursor] != ' ' {
                            s.cells[s.cursor] = ' ';
                            true
                        } else if s.cursor > 0 {
                            s.cursor -= 1;
                            s.cells[s.cursor] = ' ';
                            true
                        } else {
                            false
                        };
                        if changed {
                            cx.notify();
                        }
                        changed
                    });
                    // A backspace that clears nothing is not a change:
                    // `onChange` reports what changed, never what it was told.
                    if changed {
                        suppress_routed_errors(&state_entity, cx);
                        refresh_stored_validity(
                            &state_entity,
                            edit_is_invalid,
                            &edit_validation_errors,
                            edit_validate.as_ref(),
                            cx,
                        );
                        if let Some(cb) = &on_change {
                            let code = state_entity.read(cx).code();
                            cb(&code, window, cx);
                        }
                    }
                }
                "left" => state_entity.update(cx, |s, cx| {
                    s.cursor = s.cursor.saturating_sub(1);
                    cx.notify();
                }),
                "right" => state_entity.update(cx, |s, cx| {
                    if s.cursor + 1 < s.cells.len() {
                        s.cursor += 1;
                    }
                    cx.notify();
                }),
                single if single.chars().count() == 1 && !single.is_empty() => {
                    // `key` is the key cap; `key_char` is what was typed, which
                    // is what a capital or a shifted symbol needs.
                    let typed = ev.keystroke.key_char.as_deref().unwrap_or(single);
                    let mut chars = typed.chars();
                    let (Some(c), None) = (chars.next(), chars.next()) else {
                        return;
                    };
                    let accepted = pattern.accepts(c);
                    if accepted {
                        let completed = state_entity.update(cx, |s, cx| {
                            s.cells[s.cursor] = c;
                            if s.cursor + 1 < s.cells.len() {
                                s.cursor += 1;
                            }
                            let done = s.is_complete();
                            cx.notify();
                            done
                        });
                        // The accepted mutation above *is* the user
                        // modification, so the routed server errors must be
                        // gone — and the stored validity mirror must agree —
                        // before any callback can observe the field: a
                        // completion handler that submits the form
                        // synchronously (the one-time-code auto-submit v3's
                        // `onComplete` invites) must never be blocked by the
                        // very error this keystroke answers.
                        suppress_routed_errors(&state_entity, cx);
                        refresh_stored_validity(
                            &state_entity,
                            edit_is_invalid,
                            &edit_validation_errors,
                            edit_validate.as_ref(),
                            cx,
                        );
                        let code = state_entity.read(cx).code();
                        if let Some(cb) = &on_change {
                            cb(&code, window, cx);
                        }
                        if completed {
                            if let Some(cb) = &on_complete {
                                cb(&code, window, cx);
                            }
                        }
                    }
                }
                _ => {}
            }
        });
        if !self.is_disabled {
            row = crate::util::record_focus_bounds(row, &focused_handle, window, cx);
        }

        // A field that can be invalid has to be able to say why — every
        // message, space-joined in upstream order (React Aria's `FieldError`
        // default), not just the first.
        let error = (!validity.messages.is_empty()).then(|| validity.joined().into());
        let error_panel = crate::anim::field_error_panel(&base_id, error, window, cx);
        if let Some(error_panel) = error_panel {
            let el = gpui::div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(row)
                .child(error_panel);
            crate::util::apply_sx(el, &self.sx).into_any_element()
        } else {
            crate::util::apply_sx(row, &self.sx).into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_pattern_is_the_default() {
        assert_eq!(OtpPattern::default(), OtpPattern::Digits);
    }

    #[test]
    fn digits_rejects_letters_and_symbols() {
        assert!(OtpPattern::Digits.accepts('7'));
        assert!(!OtpPattern::Digits.accepts('a'));
        assert!(!OtpPattern::Digits.accepts('-'));
    }

    #[test]
    fn alphanumeric_accepts_both_cases() {
        assert!(OtpPattern::Alphanumeric.accepts('7'));
        assert!(OtpPattern::Alphanumeric.accepts('a'));
        assert!(OtpPattern::Alphanumeric.accepts('Z'));
        assert!(!OtpPattern::Alphanumeric.accepts('-'));
    }

    #[test]
    fn any_accepts_printables_but_not_controls() {
        assert!(OtpPattern::Any.accepts('-'));
        assert!(OtpPattern::Any.accepts(' '));
        assert!(!OtpPattern::Any.accepts('\n'));
        assert!(!OtpPattern::Any.accepts('\t'));
    }

    #[test]
    fn slot_visual_contract_keeps_pinned_state_precedence() {
        let source = include_str!("input_otp.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(source.contains("const SLOT_VALUE_IN_MS: u64 = 250"));
        // The chrome endpoints resolve in pinned precedence — invalid over
        // active/filled over resting — and the shared ramp interpolates them:
        // `.input-otp__slot` transitions `background-color 150ms
        // var(--ease-smooth), border-color 150ms var(--ease-smooth),
        // box-shadow 150ms var(--ease-out)`, snapped by
        // `motion-reduce:transition-none`.
        assert!(source.contains("crate::anim::field_chrome_ramp("));
        assert!(source.contains("colors.field.focus()"));
        assert!(source.contains("colors.danger.color"));
        assert!(source.contains("layout.border_width.max(px(1.))"));
        assert!(source.contains("layout.field_border_width"));
        assert!(source.contains("colors.field.border_hover()"));
        assert!(source.contains("crate::anim::focus_ring_endpoint(cx)"));
        assert!(source.contains(".absolute()"));
        assert!(source.contains(".top(px(12.))"));
        assert!(source.contains("value-in"));
    }

    #[test]
    fn cursor_slot_ring_is_modality_free() {
        let source = include_str!("input_otp.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        // v3 `.input-otp__slot[data-active="true"]` shows
        // `status-focused-field` for pointer focus too: `isActive` is
        // focused-input && caret-on-slot on any modality — the same
        // focus-within rule NumberField and Input follow. Gating the ring on
        // `focus_visible` would make it keyboard-only.
        let ring_expr = source
            .split("let focus_ring =")
            .nth(1)
            .expect("the slot focus ring is always computed");
        assert!(ring_expr.contains("is_cursor_cell"));
        assert!(
            !ring_expr.contains("focus_visible"),
            "the OTP cursor ring must not gate on keyboard modality"
        );
    }
}

crate::util::impl_component_styled!(InputOTP);
