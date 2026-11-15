//! NumberField — port of `@heroui/number-field` (v3).

use std::{sync::Arc, time::Duration};

use gpui::{
    prelude::*, px, App, Entity, IntoElement, MouseDownEvent, MouseUpEvent, RenderOnce,
    SharedString, Styled, Window,
};
use herogpui_core::{element_id, FieldVariant, NumberFormat};
use herogpui_theme::ActiveTheme;

use crate::a11y::A11y as _;

use crate::{icons, input::InputState};

/// State for a numeric input: text + parsed value.
pub struct NumberState {
    /// The text input entity that displays and edits the number.
    pub input: Entity<InputState>,
    value: f64,
    min: f64,
    max: f64,
    has_min: bool,
    has_max: bool,
    step: f64,
    /// `formatOptions`, written in by [`NumberField::format_options`]. The
    /// state owns it because the state is what turns a value into text.
    format: Option<NumberFormat>,
    /// React Stately's commit-gated validity snapshot for this field's
    /// `validate`. The inner `InputState`'s own display belongs to the inner
    /// `Input`, which rewrites it every frame, so the number field keeps its
    /// own — committed by blur-change, Enter, and every increment/decrement.
    validation_display: crate::validation::ValidationDisplay,
}

impl NumberState {
    /// Creates a state seeded with `initial`, with no bounds and a step of 1.
    pub fn new(cx: &mut App, initial: f64) -> Self {
        let input = cx.new(|cx| {
            let mut s = InputState::new(cx);
            s.set_value(format_number(initial));
            s
        });
        Self {
            input,
            value: initial,
            min: f64::MIN,
            max: f64::MAX,
            has_min: false,
            has_max: false,
            step: 1.0,
            format: None,
            validation_display: crate::validation::ValidationDisplay::default(),
        }
    }

    /// `defaultValue` — a state seeded with an initial number.
    ///
    /// The uncontrolled entry point; `new` already takes the value, so this is
    /// its documented alias.
    pub fn with_value(cx: &mut App, value: f64) -> Self {
        Self::new(cx, value)
    }

    /// The current numeric value.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// `FormField::number`'s submit commit: the pending gated validity becomes
    /// the displayed snapshot. Returns whether the display changed.
    pub(crate) fn commit_validation(&mut self) -> bool {
        self.validation_display.commit()
    }

    /// `FormField::number`'s reset: the display returns to valid and a queued
    /// commit is cancelled. Returns whether anything changed.
    pub(crate) fn reset_validation(&mut self) -> bool {
        self.validation_display.reset()
    }

    /// Writes a clamped value and syncs the text field.
    pub fn set_value(&mut self, v: f64, cx: &mut App) {
        self.value = v.clamp(self.min, self.max);
        let text = self.display_text();
        self.input.update(cx, |i, _| i.set_value(text));
    }

    /// The value as the field shows it: through `formatOptions` when one is
    /// set, otherwise the plain number.
    pub fn display_text(&self) -> String {
        match &self.format {
            Some(f) => f.format(self.value),
            None => format_number(self.value),
        }
    }

    /// Installs `formatOptions` and reformats the text to match.
    ///
    /// Typing is untouched — the raw text stays exactly as entered until
    /// something writes a value back, which is when v3 reformats too.
    pub fn set_format(&mut self, format: Option<NumberFormat>, cx: &mut App) {
        if self.format == format {
            return;
        }
        self.format = format;
        let text = self.display_text();
        self.input.update(cx, |i, _| i.set_value(text));
    }

    /// Sets the minimum and maximum bounds.
    pub fn set_range(&mut self, min: f64, max: f64) {
        self.min = min;
        self.max = max;
        self.has_min = true;
        self.has_max = true;
    }

    /// The current `(min, max)` bounds.
    pub fn range(&self) -> (f64, f64) {
        (self.min, self.max)
    }

    fn set_component_range(&mut self, min: Option<f64>, max: Option<f64>) {
        if let Some(min) = min {
            self.min = min;
            self.has_min = true;
        }
        if let Some(max) = max {
            self.max = max;
            self.has_max = true;
        }
    }

    fn bounds(&self) -> (Option<f64>, Option<f64>) {
        (
            self.has_min.then_some(self.min),
            self.has_max.then_some(self.max),
        )
    }

    /// The current step size.
    pub fn step_size(&self) -> f64 {
        self.step
    }

    /// Sets the step size; non-finite or non-positive values fall back to 1.
    pub fn set_step(&mut self, step: f64) {
        self.step = if step.is_finite() && step > 0.0 {
            step
        } else {
            1.0
        };
    }

    /// Re-reads the text field and updates the numeric value (or restores the
    /// previous formatted value when unparsable).
    ///
    /// The text may be what `formatOptions` produced (`$1,200.00`), so the
    /// group separators and affixes come off before parsing — the field would
    /// otherwise reject its own output.
    pub fn sync_from_input(&mut self, cx: &mut App) {
        if let Some(v) = parse_number(self.input.read(cx).value()) {
            self.value = v.clamp(self.min, self.max);
        } else {
            let text = self.display_text();
            self.input.update(cx, |i, _| i.set_value(text));
        }
    }

    fn bump(&mut self, dir: f64, cx: &mut App) -> Option<f64> {
        let min = self.has_min.then_some(self.min);
        let max = self.has_max.then_some(self.max);
        let snapped = snap_to_step(self.value, min, max, self.step);
        let next = if (dir > 0.0 && snapped > self.value) || (dir < 0.0 && snapped < self.value) {
            snapped
        } else {
            snap_to_step(self.value + dir * self.step, min, max, self.step)
        };
        if next.to_bits() == self.value.to_bits() {
            return None;
        }
        self.set_value(next, cx);
        Some(next)
    }

    fn snap(&self, value: f64) -> f64 {
        let snapped = snap_to_step(
            value,
            self.has_min.then_some(self.min),
            self.has_max.then_some(self.max),
            self.step,
        );
        if snapped.is_finite() {
            snapped
        } else {
            value.clamp(self.min, self.max)
        }
    }
}

fn snap_to_step(value: f64, min: Option<f64>, max: Option<f64>, step: f64) -> f64 {
    let anchor = min.unwrap_or(0.0);
    let mut snapped = ((value - anchor) / step).round() * step + anchor;
    if let Some(min) = min {
        snapped = snapped.max(min);
    }
    if let Some(max) = max {
        if snapped > max {
            snapped = anchor + ((max - anchor) / step).floor() * step;
        }
    }
    round_to_step_precision(snapped, step)
}

fn round_to_step_precision(value: f64, step: f64) -> f64 {
    let step_text = step.to_string().to_ascii_lowercase();
    let precision = if let Some((mantissa, exponent)) = step_text.split_once('e') {
        let exponent = exponent.parse::<i32>().unwrap_or(0);
        let fraction = mantissa
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len() as i32);
        (fraction - exponent).max(0)
    } else {
        step_text
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len() as i32)
    };
    if precision == 0 {
        return value;
    }
    let scale = 10_f64.powi(precision);
    if !scale.is_finite() {
        return value;
    }
    (value * scale).round() / scale
}

/// Reads a number back out of formatted text: `$1,200.00` -> `1200.0`,
/// `(€12.00)` -> `-12.0`, `42%` -> `42.0`.
///
/// Only the digits, sign and decimal point survive; a formatted value has to
/// round-trip or the field rejects what it just rendered.
pub fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let accounting = text.starts_with('(') && text.ends_with(')');
    let mut digits = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '0'..='9' | '.' => digits.push(c),
            '-' if digits.is_empty() => digits.push('-'),
            _ => {}
        }
    }
    let v: f64 = digits.parse().ok()?;
    Some(if accounting { -v } else { v })
}

fn format_number(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 && v.abs() < 1e15 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

type OnChange = Arc<dyn Fn(&f64, &mut Window, &mut App) + 'static>;

/// Values HeroUI supplies to a NumberField root render function.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct NumberFieldRenderState {
    /// Whether the field is disabled.
    pub is_disabled: bool,
    /// Whether the field is invalid.
    pub is_invalid: bool,
    /// Whether the field is read-only.
    pub is_read_only: bool,
    /// Whether the field is required.
    pub is_required: bool,
    /// Whether the field has focus.
    pub is_focused: bool,
    /// Whether focus is within the field.
    pub is_focus_within: bool,
    /// Whether focus is visible (keyboard focus).
    pub is_focus_visible: bool,
    /// The current value.
    pub value: f64,
    /// The minimum bound, if any.
    pub min_value: Option<f64>,
    /// The maximum bound, if any.
    pub max_value: Option<f64>,
    /// The step size.
    pub step: f64,
}

/// HeroUI NumberField.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct NumberField {
    state: Entity<NumberState>,
    /// See [`NumberField::content`].
    content: Option<Arc<dyn Fn(NumberFieldRenderState) -> gpui::AnyElement + 'static>>,
    /// `Description` — v3 composes it as a sibling of `NumberField.Group`.
    description: Option<SharedString>,
    label: Option<SharedString>,
    hide_steppers: bool,
    increment_icon: Option<gpui::AnyElement>,
    decrement_icon: Option<gpui::AnyElement>,
    vertical_steppers: bool,
    is_disabled: bool,
    variant: FieldVariant,
    full_width: bool,
    /// Optional box geometry/chrome overrides; defaults are the stock group.
    field: crate::util::FieldBox,
    /// The group's hover endpoint, in place of the variant's token.
    group_hover_bg: Option<gpui::Hsla>,
    min_value: Option<f64>,
    max_value: Option<f64>,
    step: Option<f64>,
    /// `formatOptions` — installed into the state, which owns the text.
    format: Option<NumberFormat>,
    /// `name` — forwarded to the inner field's state.
    name: Option<SharedString>,
    /// `defaultValue` — seeds the state on the first render only.
    default_value: Option<f64>,
    /// `value` — the controlled spelling; seeds the state on the first render
    /// only, ahead of `default_value`. See [`NumberField::value`].
    value: Option<f64>,
    /// `validationBehavior` — written into the inner field's state on render.
    validation_behavior: Option<crate::form::ValidationBehavior>,
    /// `validate` — run by the component, not the caller.
    validate: Option<crate::validation::Validator<f64>>,
    /// `validationErrors` — messages from a server round-trip.
    validation_errors: Vec<SharedString>,
    is_invalid: bool,
    is_required: bool,
    is_read_only: bool,
    /// `isWheelDisabled` — suppress focused wheel stepping.
    is_wheel_disabled: bool,
    /// `autoFocus` — take focus on the first render.
    auto_focus: bool,
    /// The field text's family, forwarded to the inner `Input`.
    font_family: Option<SharedString>,
    on_change: Option<OnChange>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl NumberField {
    /// `minValue` — also settable on [`NumberState::set_range`].
    pub fn min_value(mut self, v: f64) -> Self {
        self.min_value = Some(v);
        self
    }

    /// `maxValue`
    pub fn max_value(mut self, v: f64) -> Self {
        self.max_value = Some(v);
        self
    }

    /// `step`
    pub fn step(mut self, v: f64) -> Self {
        self.step = Some(v);
        self
    }

    /// `name` — the name this field submits under.
    ///
    /// Forwarded to the inner text field's state, which is where
    /// `FormField::number` looks for it.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// `defaultValue` — the uncontrolled initial number.
    ///
    /// Written into the state on the first render only. `NumberState::with_value`
    /// does the same at construction; this is the prop spelling.
    pub fn default_value(mut self, value: f64) -> Self {
        self.default_value = Some(value);
        self
    }

    /// `value` — v3's controlled-value spelling, as a pure builder.
    ///
    /// Like [`NumberField::default_value`] this seeds the state on the first
    /// render only, and it wins when both are given; calling `.value(..)`
    /// twice keeps the last call, like every other builder here. A later
    /// value is an imperative update rather than a builder:
    /// `state.update(cx, |s, cx| s.set_value(v, cx))`.
    pub fn value(mut self, value: f64) -> Self {
        self.value = Some(value);
        self
    }

    /// `validationBehavior` — see [`crate::input::Input::validation_behavior`].
    pub fn validation_behavior(mut self, behavior: crate::form::ValidationBehavior) -> Self {
        self.validation_behavior = Some(behavior);
        self
    }

    /// `formatOptions` — how the value is written out.
    ///
    /// ```
    /// # use gpui::{prelude::*, Window};
    /// # use herogpui_components::{NumberField, NumberFormat, NumberState};
    /// # struct Demo;
    /// # impl Render for Demo {
    /// #     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    /// #         let state = cx.new(|cx| NumberState::new(cx, 42.));
    /// NumberField::new(state).format_options(NumberFormat::currency("USD"))
    /// #     }
    /// # }
    /// # let mut tcx = gpui::TestAppContext::single();
    /// # tcx.update(herogpui_theme::ThemeProvider::init);
    /// # let _ = tcx.add_window_view(|_, _| Demo);
    /// ```
    pub fn format_options(mut self, format: NumberFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Sets the field variant.
    pub fn variant(mut self, variant: FieldVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets whether the field fills the available width.
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self
    }

    /// Replaces the 36px group height. The inner field follows so its own box
    /// cannot defeat a smaller group.
    pub fn height(mut self, h: impl Into<gpui::Pixels>) -> Self {
        self.field.height = Some(h.into());
        self
    }

    /// Replaces the input's `px-3` horizontal padding. The steppers keep their
    /// own 40px slots at the ends of the group.
    pub fn padding_x(mut self, p: impl Into<gpui::Pixels>) -> Self {
        self.field.padding_x = Some(p.into());
        self
    }

    /// Renders the group with no background, border, field shadow, focus ring
    /// or hover fill, for a caller painting around it. The field stays
    /// editable and focusable.
    pub fn is_bare(mut self, v: bool) -> Self {
        self.field.is_bare = v;
        self.field.is_bare_is_set = true;
        self
    }

    /// The group's fill while hovered, in place of `--field-hover`
    /// (`--default-hover` on the secondary variant). The 150ms ease-smooth
    /// ramp and the border hover are unchanged; a focused, invalid, disabled
    /// or bare group does not hover, so the override never reaches those
    /// states. Not a v3 prop: v3 tints the group with a class.
    pub fn group_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.group_hover_bg = Some(color.into());
        self
    }

    /// The field text's font family, forwarded to the inner [`crate::Input`]
    /// so the value, placeholder and caret measurement all use it (see
    /// [`crate::Input::font_family`]), and set on the group so custom stepper
    /// content inherits it; unset keeps the inherited family. Not a v3 prop;
    /// v3 sets it with a class.
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// Shows or hides only the group's visual focus ring. The number field
    /// remains focusable, editable and stepper-accessible when set to `false`.
    pub fn focus_ring(mut self, v: bool) -> Self {
        self.field.focus_ring = Some(v);
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the field's root element — the column holding the label, the
    /// group and the message — after every value the variant and the active
    /// theme chose, so they win. The group's own chrome stays with the variant.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// `validate` — returns the message to show, or `None` when the number is fine.
    ///
    /// The component runs it and surfaces the result.
    pub fn validate(mut self, f: impl Fn(&f64) -> Option<SharedString> + 'static) -> Self {
        self.validate = Some(Arc::new(f));
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

    /// Sets whether the field is invalid (`isInvalid`).
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// Sets whether the field is required (`isRequired`).
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// `autoFocus` — take focus on the first render.
    pub fn auto_focus(mut self, v: bool) -> Self {
        self.auto_focus = v;
        self
    }

    /// Sets whether the field is read-only (`isReadOnly`).
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.is_read_only = v;
        self
    }

    /// `isWheelDisabled` — whether focused wheel input may step the value.
    pub fn is_wheel_disabled(mut self, v: bool) -> Self {
        self.is_wheel_disabled = v;
        self
    }

    /// v3's field `children`-as-a-function.
    pub fn content(
        mut self,
        render: impl Fn(NumberFieldRenderState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.content = Some(Arc::new(render));
        self
    }

    /// Builds the field over `state`, which may be borrowed —
    /// `NumberField::new(&handle)` — so a caller keeping its own handle clones
    /// nothing at the call site; an owned `Entity<NumberState>` still works,
    /// and the cheap handle clone happens once inside either way.
    pub fn new(state: impl std::borrow::Borrow<Entity<NumberState>>) -> Self {
        Self {
            content: None,
            state: state.borrow().clone(),
            description: None,
            label: None,
            hide_steppers: false,
            increment_icon: None,
            decrement_icon: None,
            vertical_steppers: false,
            is_disabled: false,
            variant: FieldVariant::Primary,
            full_width: false,
            min_value: None,
            max_value: None,
            step: None,
            format: None,
            name: None,
            default_value: None,
            value: None,
            validation_behavior: None,
            validate: None,
            validation_errors: Vec::new(),
            is_invalid: false,
            is_required: false,
            is_read_only: false,
            is_wheel_disabled: false,
            auto_focus: false,
            font_family: None,
            on_change: None,
            sx: None,
            field: crate::util::FieldBox::default(),
            group_hover_bg: None,
        }
    }

    /// `Description` — help text under the field.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Sets the label shown above the field.
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }

    /// Hides the -/+ steppers (`hideStepper`).
    pub fn hide_steppers(mut self, v: bool) -> Self {
        self.hide_steppers = v;
        self
    }

    /// `NumberField.IncrementButton` children.
    pub fn increment_icon(mut self, icon: impl IntoElement) -> Self {
        self.increment_icon = Some(icon.into_any_element());
        self
    }

    /// `NumberField.DecrementButton` children.
    pub fn decrement_icon(mut self, icon: impl IntoElement) -> Self {
        self.decrement_icon = Some(icon.into_any_element());
        self
    }

    /// The documented chevron composition: two 24px half-height steppers at
    /// the trailing edge instead of the default 40px side buttons.
    pub fn vertical_steppers(mut self, v: bool) -> Self {
        self.vertical_steppers = v;
        self
    }

    /// Sets whether the field is disabled (`isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets the handler called when the value changes (`onChange`).
    pub fn on_change(mut self, f: impl Fn(&f64, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Arc::new(f));
        self
    }
}

impl RenderOnce for NumberField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Component-level `minValue`/`maxValue`/`step` win over whatever the
        // state was seeded with. Bound presence is stored separately from its
        // number, because f64::MIN/MAX are also legitimate explicit bounds.
        if self.min_value.is_some() || self.max_value.is_some() {
            self.state.update(cx, |s, _| {
                s.set_component_range(self.min_value, self.max_value);
            });
        }
        if let Some(step) = self.step {
            if step.to_bits() != self.state.read(cx).step_size().to_bits() {
                self.state.update(cx, |s, _| s.set_step(step));
            }
        }

        // Every keyed slot hangs off the state entity: a `NumberField` takes
        // no id of its own, and the entity is the identity it does have.
        let base_id =
            gpui::ElementId::named_usize("number", self.state.entity_id().as_u64() as usize);

        // `value` / `defaultValue` seed the state once, before anything reads
        // it. `value` is v3's controlled spelling, so it outranks the
        // uncontrolled seed; the state owns the number afterwards, and
        // `NumberState::set_value` is the imperative update.
        if let Some(value) = self.value.or(self.default_value) {
            let state = self.state.clone();
            crate::util::seed_once(
                window,
                cx,
                element_id::scoped(&base_id, "default"),
                move |cx| {
                    state.update(cx, |s, cx| {
                        s.set_value(value, cx);
                        cx.notify();
                    });
                },
            );
        }

        // `formatOptions` lives in the state, which owns the text. `set_format`
        // is a no-op when it already matches, so this does not loop.
        if let Some(format) = self.format.clone() {
            self.state
                .update(cx, |s, cx| s.set_format(Some(format), cx));
        }

        let input_state = self.state.read(cx).input.clone();
        if input_state.read(cx).name() != self.name {
            let name = self.name.clone();
            input_state.update(cx, |state, _| state.set_name(name));
        }
        if let Some(behavior) = self.validation_behavior {
            if input_state.read(cx).validation_behavior() != behavior {
                input_state.update(cx, |state, _| state.set_validation_behavior(behavior));
            }
        }
        let is_successful = !self.is_disabled;
        if input_state.read(cx).is_successful() != is_successful {
            input_state.update(cx, |state, _| state.set_successful(is_successful));
        }

        // v3 order: the controlled flag, then server errors, then `validate`.
        // The server slot carries the messages the `Form`'s
        // `validationErrors` record routed into the inner state by name,
        // ahead of this field's own `validationErrors` prop.
        let value_now = self.state.read(cx).value();
        let mut server_errors = input_state.read(cx).routed_errors().to_vec();
        server_errors.extend(self.validation_errors.iter().cloned());
        let validate_result = self.validate.as_ref().and_then(|f| f(&value_now));
        let validity = crate::validation::resolve(
            self.is_invalid,
            &server_errors,
            validate_result.clone(),
            None,
        );
        if input_state.read(cx).validity() != &validity {
            let validity = validity.clone();
            input_state.update(cx, |state, _| state.set_validity(validity));
        }

        // React Stately's commit-gated display: `validate` alone is the
        // pending half, synced into the state's snapshot every render; the
        // commit triggers (blur-change, Enter, every increment/decrement)
        // queue the commit that the next render's sync lands.
        // `display_validity` composes what the field paints — the controlled
        // flag and server messages stay live, the gated half is the snapshot.
        let behavior = input_state.read(cx).validation_behavior();
        let pending = crate::validation::resolve(false, &[], validate_result, None);
        self.state
            .update(cx, |state, _| state.validation_display.sync(pending));

        let focus_handle = input_state.read(cx).focus_handle.clone();
        // v3 commits a field's validation on blur — but only when the text
        // changed since focus, mirroring the native `change` event. `left_focus`
        // returns the text the input held at focus on the one render focus has
        // just left; a difference commits the gated snapshot immediately. This
        // runs before `display_validity` reads `displayed`, so the committing
        // frame already paints the fresh snapshot.
        let inner_text = input_state.read(cx).value().to_owned();
        if let Some(arrival) = crate::validation::left_focus(
            element_id::scoped(&base_id, "left-focus"),
            focus_handle.is_focused(window),
            &inner_text,
            window,
            cx,
        ) {
            if arrival != inner_text {
                self.state.update(cx, |state, cx| {
                    if state.validation_display.commit() {
                        cx.notify();
                    }
                });
            }
        }

        let displayed = self.state.read(cx).validation_display.displayed().clone();
        let display = crate::validation::display_validity(
            behavior,
            &validity,
            self.is_invalid,
            &server_errors,
            &displayed,
            None,
        );

        if self.auto_focus {
            crate::util::focus_once(
                window,
                cx,
                element_id::scoped(&base_id, "autofocus"),
                &focus_handle,
            );
        }

        // v3's field children-as-a-function: the caller builds the parts from
        // the resolved field state, so only the built-in stack is skipped.
        if let Some(render) = self.content.clone() {
            let state = self.state.read(cx);
            let focused = focus_handle.is_focused(window);
            let (min_value, max_value) = state.bounds();
            return render(NumberFieldRenderState {
                is_disabled: self.is_disabled,
                is_invalid: display.is_invalid,
                is_read_only: self.is_read_only,
                is_required: self.is_required,
                is_focused: focused,
                is_focus_within: focus_handle.contains_focused(window, cx),
                is_focus_visible: focused && crate::util::focus_visible(cx),
                value: state.value(),
                min_value,
                max_value,
                step: state.step_size(),
            });
        }

        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        let field_box = self.field;

        // `.number-field__group` is `h-9`, the one height every v3 field has.
        let h = field_box.resolved_height();
        // `.number-field__increment-button` is `h-full w-10`: a 40px square-ish
        // slot at the end of the group, not the 26px one this used to draw.
        let btn_px = px(40.);

        // text field bound to the inner InputState
        let text_state = self.state.clone();
        let on_text_change = self.on_change.clone();
        let mut field = crate::input::Input::new(self.state.read(cx).input.clone())
            .when_some(self.name.clone(), |f, n| f.name(n))
            .when_some(self.validation_behavior, |f, b| f.validation_behavior(b))
            .variant(self.variant)
            .is_disabled(self.is_disabled)
            .is_read_only(self.is_read_only)
            .is_required(self.is_required)
            .auto_focus(self.auto_focus)
            .when_some(self.font_family.clone(), |f, family| f.font_family(family))
            // The live flag feeds the `InputState::validity` mirror a `Form`'s
            // blocked decision reads; the gated snapshot is what paints.
            .is_invalid(validity.is_invalid)
            .display_override(display.clone())
            .when_some(self.label.clone(), |f, label| f.a11y_label(label))
            .on_change(move |_text: &str, w, cx| {
                // The Input already wrote its own text; re-parse here.
                let before = text_state.read(cx).value();
                let after = text_state.update(cx, |s, sc| {
                    s.sync_from_input(sc);
                    sc.notify();
                    s.value()
                });
                if before.to_bits() != after.to_bits() {
                    if let Some(cb) = &on_text_change {
                        cb(&after, w, cx);
                    }
                }
            });

        // `.number-field__group` is one box -- `grid h-9 rounded-field bg-field
        // shadow-field overflow-hidden` -- with a 40px decrement button, the
        // input, and a 40px increment button inside it, each separated by a
        // hairline. The steppers used to sit *outside* the field as two loose
        // buttons, which is not a shape v3 has.
        let steppers = !self.hide_steppers;
        let group_radius = crate::util::field_radius(cx);
        // v3 rounds only a stepper cell's outer corners (`rounded-s-field` /
        // `rounded-e-field`): the press fill inside inherits the field
        // radius shrunk by the shell's border, the way a CSS radius insets.
        let stepper_corner =
            crate::util::inner_fill_radius(group_radius, layout.field_border_width);
        // NumberField.Input keeps `px-3` even beside either button. Passing
        // false on both sides removes the standalone chrome without borrowing
        // InputGroup's addon-padding behavior. The group owns the row and its
        // chrome; a padding override travels through Input's crate-internal
        // grouped seam so the public `Input::padding_x` contract is untouched.
        field = field.in_group(false, false);
        if let Some(padding_x) = field_box.padding_x {
            field = field.group_padding_x(padding_x);
        }
        if let Some(height) = field_box.height {
            field = field.height(height);
        }

        // `useNumberField` returns `groupProps` with `role: 'group'`; the
        // spin button role its `useSpinButton` produces is deleted again
        // (`role: null`) before it reaches the input, so nothing here is a
        // spin button.
        let a11y_name =
            crate::a11y::Name::field(self.label.as_ref(), self.description.as_ref(), &display);
        // The group's settled state ring, parked for the non-clipping carrier
        // at the tail of this render; see the chrome call below. Once the
        // ramp owns the ring it hands back the animated overlay element for
        // the same carrier instead.
        let mut settled_ring = None;
        let mut escaped_ring = None;
        let mut carries_ring = false;
        let mut group = gpui::div()
            .id(element_id::scoped(&base_id, "group"))
            .a11y_named(crate::a11y::Role::Group, &a11y_name)
            // The family also reaches caller stepper content (`increment_icon`
            // / `decrement_icon`); the inner field takes it explicitly above
            // for its caret measurement.
            .when_some(self.font_family.clone(), |group, family| group.font_family(family))
            .flex()
            .items_center()
            .h(h)
            .overflow_hidden()
            .text_size(crate::util::FIELD_TEXT)
            .line_height(px(20.));
        if !field_box.is_bare {
            // The settled group chrome comes from the same shared helper the
            // other fields take — variant fill and shadow, the invalid danger
            // outline or ring, the keyboard focus ring.
            let focused = focus_handle.is_focused(window);
            let show_focus_ring = field_box.focus_ring.unwrap_or(true);
            carries_ring = true;
            // `.number-field__group` is `overflow-hidden` around the input and
            // the steppers, so the settled ring cannot be one of its children:
            // the group takes the chrome without it and the ring hangs on
            // `util::field_ring_carrier` below, outside the clip.
            settled_ring =
                crate::util::field_ring_color(display.is_invalid, focused, show_focus_ring, cx);
            group = crate::util::apply_field_chrome_ringless(
                group,
                self.variant,
                display.is_invalid,
                focused,
                show_focus_ring,
                None,
                cx,
            );
            // `.number-field__group` transitions all three chrome properties
            // rather than swapping them (lines 39-43: `background-color 150ms
            // var(--ease-smooth), border-color 150ms var(--ease-smooth),
            // box-shadow 150ms var(--ease-out)`), so the same state table
            // resolves again as ramp endpoints and the shared chrome ramp
            // interpolates between them: focus-within fills `--field-focus`
            // and hover fills `--field-hover`/`--field-border-hover` only
            // while the group is neither focused nor invalid. The ramp owns
            // the group's border and ring past its first flip, so the
            // helper's instant ones stop being cast.
            let ringed = focused && show_focus_ring;
            // The shared helper's chain is an else-if: a ringed group carries
            // no border at all (the ring replaces the chrome), and an invalid
            // group that is not ringed carries the one-pixel danger outline.
            let outlined = display.is_invalid && !ringed;
            let idle = crate::anim::FieldChrome {
                bg: match self.variant {
                    // `--field-focus: var(--field-background)` — focus and
                    // invalid fill the group with the field token itself.
                    FieldVariant::Primary => colors.field.background,
                    FieldVariant::Secondary => colors.default.color,
                },
                border: if outlined {
                    colors.danger.color
                } else {
                    colors.field.border
                },
                border_width: if outlined {
                    layout.border_width.max(px(1.))
                } else if ringed {
                    px(0.)
                } else {
                    layout.field_border_width
                },
                ring: if ringed {
                    Some(if display.is_invalid {
                        crate::anim::danger_ring_endpoint(cx)
                    } else {
                        crate::anim::focus_ring_endpoint(cx)
                    })
                } else {
                    None
                },
            };
            let hovered = (!self.is_disabled && !display.is_invalid && !focused).then(|| {
                crate::anim::FieldChrome {
                    bg: self.group_hover_bg.unwrap_or(match self.variant {
                        FieldVariant::Primary => colors.field.hover(),
                        // `.number-field--secondary` hovers
                        // `--number-field-group-bg-hover: var(--default-hover)`.
                        FieldVariant::Secondary => colors.default.hover(),
                    }),
                    border: colors.field.border_hover(),
                    border_width: layout.field_border_width,
                    ring: None,
                }
            });
            let ramp_owns_ring;
            let ramp_ring;
            (group, ramp_owns_ring, ramp_ring) = crate::anim::field_chrome_ramp(
                group,
                &element_id::scoped(&base_id, "group"),
                idle,
                hovered,
                match self.variant {
                    FieldVariant::Primary => layout.field_shadow.clone(),
                    FieldVariant::Secondary => Vec::new(),
                },
                group_radius,
                // The group is `overflow-hidden` — no child, deferred or
                // not, can paint the outset ring inside its clip, so the
                // ramp hands the hollow overlay back for the carrier below.
                true,
                window,
                cx,
            );
            // Past its first flip the ramp's own layer is the state ring, and
            // painting the carrier's overlay as well would draw two.
            if ramp_owns_ring {
                settled_ring = None;
                carries_ring = false;
                escaped_ring = ramp_ring;
            }
        }
        if self.full_width {
            group = group.w_full();
        } else {
            group = group.w(px(220.));
        }

        // `border-field-placeholder/15` is the seam between a stepper and the
        // input; the buttons themselves are transparent.
        let seam = colors.field.placeholder.alpha(0.15);
        let decrement_icon = self.decrement_icon.unwrap_or_else(|| {
            gpui::svg()
                .size(crate::util::FIELD_ICON)
                .path(icons::MINUS)
                .text_color(colors.field.foreground)
                .into_any_element()
        });
        let increment_icon = self.increment_icon.unwrap_or_else(|| {
            gpui::svg()
                .size(crate::util::FIELD_ICON)
                .path(icons::PLUS)
                .text_color(colors.field.foreground)
                .into_any_element()
        });
        let field = gpui::div().flex_1().min_w_0().child(field);
        // A vertical stepper column stacks two half-height buttons; an
        // explicit group height shrinks them so both stay reachable inside
        // the overflow-hidden box.
        let vertical_stepper_h = field_box
            .height
            .map_or(px(18.), |height| px(f32::from(height) / 2.0));
        if !steppers {
            group = group.child(field);
        } else if self.vertical_steppers {
            group = group.child(field).child(
                gpui::div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .w(px(24.))
                    .h_full()
                    .border_l_1()
                    .border_color(seam)
                    .child(stepper_btn(
                        &self.state,
                        &self.on_change,
                        self.is_invalid,
                        self.validation_errors.clone(),
                        self.validate.clone(),
                        &colors,
                        layout.field_border_width,
                        stepper_corner,
                        StepperCorners::TopRight,
                        vertical_stepper_h,
                        px(24.),
                        increment_icon,
                        1.0,
                        self.is_disabled || self.is_read_only,
                        &a11y_name,
                        window,
                        cx,
                    ))
                    .child(stepper_btn(
                        &self.state,
                        &self.on_change,
                        self.is_invalid,
                        self.validation_errors.clone(),
                        self.validate.clone(),
                        &colors,
                        layout.field_border_width,
                        stepper_corner,
                        StepperCorners::BottomRight,
                        vertical_stepper_h,
                        px(24.),
                        decrement_icon,
                        -1.0,
                        self.is_disabled || self.is_read_only,
                        &a11y_name,
                        window,
                        cx,
                    )),
            );
        } else {
            group = group
                .child(
                    stepper_btn(
                        &self.state,
                        &self.on_change,
                        self.is_invalid,
                        self.validation_errors.clone(),
                        self.validate.clone(),
                        &colors,
                        layout.field_border_width,
                        stepper_corner,
                        StepperCorners::Left,
                        h,
                        btn_px,
                        decrement_icon,
                        -1.0,
                        self.is_disabled || self.is_read_only,
                        &a11y_name,
                        window,
                        cx,
                    )
                    .border_r_1()
                    .border_color(seam),
                )
                .child(field)
                .child(
                    stepper_btn(
                        &self.state,
                        &self.on_change,
                        self.is_invalid,
                        self.validation_errors.clone(),
                        self.validate.clone(),
                        &colors,
                        layout.field_border_width,
                        stepper_corner,
                        StepperCorners::Right,
                        h,
                        btn_px,
                        increment_icon,
                        1.0,
                        self.is_disabled || self.is_read_only,
                        &a11y_name,
                        window,
                        cx,
                    )
                    .border_l_1()
                    .border_color(seam),
                );
        }
        // React Aria drives a number field from `useSpinButton`: the arrows
        // step by `step`, Home and End run to the bounds, and Page Up/Down fall
        // back to a plain step because `NumberField` passes no page handlers.
        // The keys arrive at the focused input and bubble to here.
        if !self.is_disabled && !self.is_read_only {
            let key_state = self.state.clone();
            let key_change = self.on_change.clone();
            let key_is_invalid = self.is_invalid;
            let key_validation_errors = self.validation_errors.clone();
            let key_validate = self.validate.clone();
            let (min_value, max_value) = self.state.read(cx).bounds();
            group = group.on_key_down(move |ev: &gpui::KeyDownEvent, window, cx| {
                let dir = match ev.keystroke.key.as_str() {
                    // Enter commits the gated validity, then falls through —
                    // a `Form`'s implicit Enter submit must still see the key.
                    "enter" => {
                        key_state.update(cx, |state, cx| {
                            state.validation_display.queue_commit();
                            cx.notify();
                        });
                        return;
                    }
                    "up" | "pageup" => 1.0,
                    "down" | "pagedown" => -1.0,
                    "home" => {
                        let Some(min) = min_value else {
                            return;
                        };
                        // `incrementToMin`/`decrementToMin` commit even when the
                        // bound does not move the value.
                        key_state.update(cx, |state, cx| {
                            state.validation_display.queue_commit();
                            cx.notify();
                        });
                        if set_number_value(&key_state, min, cx) {
                            suppress_routed_server_errors(
                                &key_state,
                                key_is_invalid,
                                &key_validation_errors,
                                key_validate.as_ref(),
                                cx,
                            );
                            if let Some(cb) = &key_change {
                                cb(&key_state.read(cx).value(), window, cx);
                            }
                        }
                        return;
                    }
                    "end" => {
                        let Some(max) = max_value else {
                            return;
                        };
                        key_state.update(cx, |state, cx| {
                            state.validation_display.queue_commit();
                            cx.notify();
                        });
                        if set_number_value(&key_state, max, cx) {
                            suppress_routed_server_errors(
                                &key_state,
                                key_is_invalid,
                                &key_validation_errors,
                                key_validate.as_ref(),
                                cx,
                            );
                            if let Some(cb) = &key_change {
                                cb(&key_state.read(cx).value(), window, cx);
                            }
                        }
                        return;
                    }
                    _ => return,
                };
                report_bump(
                    &key_state,
                    dir,
                    &key_change,
                    key_is_invalid,
                    &key_validation_errors,
                    key_validate.as_ref(),
                    window,
                    cx,
                );
            });
        }
        if !self.is_disabled && !self.is_read_only && !self.is_wheel_disabled {
            let wheel_state = self.state.clone();
            let wheel_change = self.on_change.clone();
            let wheel_is_invalid = self.is_invalid;
            let wheel_validation_errors = self.validation_errors.clone();
            let wheel_validate = self.validate.clone();
            let wheel_focus = self.state.read(cx).input.read(cx).focus_handle.clone();
            group = group.on_scroll(move |event, window, cx| {
                if !wheel_focus.contains_focused(window, cx) {
                    return;
                }
                let (dx, dy) = match event.delta {
                    gpui::ScrollDelta::Pixels(point) => (f32::from(point.x), f32::from(point.y)),
                    gpui::ScrollDelta::Lines(point) => (point.x, point.y),
                };
                if event.modifiers.control || event.modifiers.platform {
                    return;
                }
                let direction = if dy != 0.0 { dy.signum() } else { dx.signum() };
                if direction != 0.0 {
                    report_bump(
                        &wheel_state,
                        f64::from(direction),
                        &wheel_change,
                        wheel_is_invalid,
                        &wheel_validation_errors,
                        wheel_validate.as_ref(),
                        window,
                        cx,
                    );
                }
                cx.stop_propagation();
            });
        }
        if self.is_disabled {
            group = group.opacity(layout.disabled_opacity);
        }

        // The label, description and error slot belong to the field, not to the
        // group: v3 composes them as siblings of `NumberField.Group`.
        let mut el = gpui::div().flex().flex_col().gap(px(4.));
        if self.full_width {
            el = el.w_full();
        }
        if let Some(label) = &self.label {
            el = el.child(
                crate::field::Label::new(label.clone())
                    .is_required(self.is_required)
                    .is_invalid(display.is_invalid)
                    .is_disabled(self.is_disabled),
            );
        }
        // The carrier must hug the group's box: it stretches with the field
        // column, and an overlay ring anchored to a wider carrier would span
        // the column instead of the group.
        el = el.child(if carries_ring {
            crate::util::field_ring_carrier(group, settled_ring, group_radius, cx)
                .when(!self.full_width, |carrier| carrier.w(px(220.)))
                .into_any_element()
        } else if let Some(overlay) = escaped_ring {
            // The ramp owns the ring past its first flip: its hollow overlay
            // hangs on the same non-clipping carrier the settled ring uses,
            // outside the group's `overflow-hidden`.
            crate::util::field_ring_carrier_overlay(group, overlay)
                .when(!self.full_width, |carrier| carrier.w(px(220.)))
                .into_any_element()
        } else {
            group.into_any_element()
        });
        // Every message, space-joined in upstream order — React Aria's
        // `FieldError` default — not just the first. The shared feedback row
        // retains the last message while its height and opacity settle out.
        let error = (!display.messages.is_empty()).then(|| display.joined().into());
        if let Some(error) = crate::anim::field_error_panel(&base_id, error, window, cx) {
            el = el.child(error);
        } else if let Some(description) = self.description.clone() {
            el = el.child(crate::field::Description::new(description));
        }
        el = crate::util::apply_sx(el, &self.sx);
        el.into_any_element()
    }
}

/// Which of the group's rounded corners a stepper cell's press fill owns:
/// v3 rounds the outer edge (`rounded-s-field` / `rounded-e-field`) while
/// the seam side and the interior edges stay square.
#[derive(Clone, Copy)]
enum StepperCorners {
    /// Horizontal decrement: the group's left edge.
    Left,
    /// Horizontal increment: the group's right edge.
    Right,
    /// Vertical increment: the top of the right-hand column.
    TopRight,
    /// Vertical decrement: the bottom of the right-hand column.
    BottomRight,
}

/// One stepper cell: `flex h-full w-10 items-center justify-center
/// rounded-none bg-transparent`, pressed at `bg-field-foreground/10`.
#[allow(clippy::too_many_arguments)]
fn stepper_btn(
    state: &Entity<NumberState>,
    on_change: &Option<OnChange>,
    is_invalid: bool,
    validation_errors: Vec<SharedString>,
    validate: Option<crate::validation::Validator<f64>>,
    colors: &herogpui_theme::ThemeColors,
    field_border_width: gpui::Pixels,
    corner_radius: gpui::Pixels,
    outer_corners: StepperCorners,
    h: gpui::Pixels,
    btn_px: gpui::Pixels,
    icon: gpui::AnyElement,
    dir: f64,
    is_disabled: bool,
    field_name: &crate::a11y::Name,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let st = state.clone();
    let on_change = on_change.clone();
    let edit_validation_errors = validation_errors;
    let edit_validate = validate.clone();
    // `dir` is the +1/-1 step, so the part is named rather than formatted:
    // `-1` would otherwise put a second hyphen in the middle of the key.
    let id = element_id::scoped(
        &gpui::ElementId::named_usize("num", state.entity_id().as_u64() as usize),
        if dir >= 0.0 { "increment" } else { "decrement" },
    );
    let press = window.use_keyed_state(element_id::scoped(&id, "press"), cx, |_, _| {
        StepperPress::default()
    });
    let focus_handle = state.read(cx).input.read(cx).focus_handle.clone();
    // `useNumberField` names each stepper "Increase {label}" / "Decrease
    // {label}", spelling the field's name into the button's own `aria-label`
    // rather than pointing at the label element; the template (and its word
    // order) comes from the locale's catalogue.
    let stepper_key = if dir >= 0.0 {
        crate::i18n::UiString::Increase
    } else {
        crate::i18n::UiString::Decrease
    };
    let stepper_name = crate::a11y::Name::labelled(crate::i18n::ui_string_with(
        stepper_key,
        field_name.label().map_or("", |label| label.as_ref()),
        cx,
    ));
    let mut b = gpui::div()
        .id(id)
        .a11y_named(crate::a11y::Role::Button, &stepper_name)
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .w(btn_px)
        .h(h)
        // HeroUI gives each spin button the field border token. The default
        // theme keeps this at zero, while custom themes can opt into a visible
        // field border; carrying it here prevents the stepper cells from
        // losing the shared shell's border treatment in that configuration.
        .border(field_border_width)
        .border_color(colors.field.border);
    // The icon joins the skin before the press wrap: children added after
    // `pressed` land on the slot and fight the skin for width.
    b = b.text_color(colors.field.foreground).child(icon);
    if !is_disabled {
        let release = press.clone();
        b = b.child(
            gpui::canvas(
                |bounds, _, _| bounds,
                move |_, _, window, _| {
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == gpui::DispatchPhase::Capture
                            && event.button == gpui::MouseButton::Left
                        {
                            release.update(cx, |press, _| press.active = false);
                        }
                    });
                },
            )
            .absolute()
            .inset_0(),
        );
        let pressed_bg = colors.field.foreground.alpha(0.1);
        b = crate::util::cursor_interactive(b, cx).on_mouse_down(
            gpui::MouseButton::Left,
            move |_: &MouseDownEvent, window, cx| {
                window.focus(&focus_handle, cx);
                // gpui's mouse-down default then focuses the nearest
                // focusable ancestor and steals the handle back in the same
                // dispatch. RAC's `useNumberField` keeps the input focused
                // while stepping (`onButtonPressStart`), which is what makes
                // the group focus-within and paints the state ring.
                window.prevent_default();
                let generation = press.update(cx, |press, _| {
                    press.active = true;
                    press.generation = press.generation.wrapping_add(1);
                    press.generation
                });
                report_bump(
                    &st,
                    dir,
                    &on_change,
                    is_invalid,
                    &edit_validation_errors,
                    edit_validate.as_ref(),
                    window,
                    cx,
                );

                let repeat_press = press.downgrade();
                let repeat_state = st.clone();
                let repeat_change = on_change.clone();
                let repeat_errors = edit_validation_errors.clone();
                let repeat_validate = edit_validate.clone();
                window
                    .spawn(cx, async move |cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(400))
                            .await;
                        loop {
                            let keep_repeating = cx
                                .update(|window, cx| {
                                    let Some(press) = repeat_press.upgrade() else {
                                        return false;
                                    };
                                    let active = {
                                        let press = press.read(cx);
                                        press.active && press.generation == generation
                                    };
                                    if !active {
                                        return false;
                                    }
                                    let changed = report_bump(
                                        &repeat_state,
                                        dir,
                                        &repeat_change,
                                        is_invalid,
                                        &repeat_errors,
                                        repeat_validate.as_ref(),
                                        window,
                                        cx,
                                    );
                                    if !changed {
                                        if let Some(press) = repeat_press.upgrade() {
                                            press.update(cx, |press, _| press.active = false);
                                        }
                                    }
                                    changed
                                })
                                .unwrap_or(false);
                            if !keep_repeating {
                                break;
                            }
                            cx.background_executor()
                                .timer(Duration::from_millis(60))
                                .await;
                        }
                    })
                    .detach();
            },
        );
        // The press fill rounds only the cell's outer corners; the seam side
        // and interior edges stay square. `pressed_corners` reads these off
        // the skin and scales them with the press.
        match outer_corners {
            StepperCorners::Left => {
                b = b.rounded_tl(corner_radius).rounded_bl(corner_radius);
            }
            StepperCorners::Right => {
                b = b.rounded_tr(corner_radius).rounded_br(corner_radius);
            }
            StepperCorners::TopRight => {
                b = b.rounded_tr(corner_radius);
            }
            StepperCorners::BottomRight => {
                b = b.rounded_br(corner_radius);
            }
        }
        b = crate::anim::pressed_with_background(
            b,
            crate::anim::PressBox {
                height: h,
                padding_x: None,
                width: Some(btn_px),
                min_width: None,
                text_size: crate::util::FIELD_TEXT,
                line_height: px(20.),
                gap: px(0.),
                radius: px(0.),
                scale: crate::anim::PRESSED_SCALE,
                shrink_x: true,
            },
            pressed_bg,
            cx,
        );
    }
    b
}

#[derive(Default)]
struct StepperPress {
    active: bool,
    generation: u64,
}

fn set_number_value(state: &Entity<NumberState>, value: f64, cx: &mut App) -> bool {
    state.update(cx, |state, cx| {
        let before = state.value();
        let value = state.snap(value);
        state.set_value(value, cx);
        let changed = before.to_bits() != state.value().to_bits();
        if changed {
            cx.notify();
        }
        changed
    })
}

/// A user-driven value change — the steppers, the arrows, Home/End, the wheel
/// — suppresses the routed server messages the way an edit does (v3: server
/// errors are "cleared when user modifies the field") and refreshes the inner
/// state's stored validity in the same stroke. `NumberField::render` writes
/// that mirror, so without the refresh it is one frame old — and `on_change`
/// may submit the enclosing `Form` synchronously before any frame catches up,
/// reading the routed error the change just answered as still blocking. The
/// routed slot is left out — the change suppressed it — so the inner mirror
/// is resolved from the builder's own sources exactly as the next render
/// would resolve it with an empty routed slot. Typing flows through the inner
/// `Input`'s own edit path, which clears the same slot and refreshes the same
/// mirror.
fn suppress_routed_server_errors(
    state: &Entity<NumberState>,
    is_invalid: bool,
    validation_errors: &[SharedString],
    validate: Option<&crate::validation::Validator<f64>>,
    cx: &mut App,
) {
    let input = state.read(cx).input.clone();
    if !input.read(cx).routed_errors().is_empty() {
        input.update(cx, |input, cx| {
            input.clear_routed_errors();
            cx.notify();
        });
    }
    let value = state.read(cx).value();
    let validity = crate::validation::resolve(
        is_invalid,
        validation_errors,
        validate.and_then(|f| f(&value)),
        None,
    );
    if input.read(cx).validity() != &validity {
        input.update(cx, |input, _| input.set_validity(validity));
    }
}

#[allow(clippy::too_many_arguments)] // one user change: state, direction, callback and the three validity sources
fn report_bump(
    state: &Entity<NumberState>,
    dir: f64,
    on_change: &Option<OnChange>,
    is_invalid: bool,
    validation_errors: &[SharedString],
    validate: Option<&crate::validation::Validator<f64>>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let next = state.update(cx, |state, cx| {
        // Upstream commits on every increment/decrement attempt — even one
        // the bounds clamp to no movement — so the queue lands unconditionally.
        state.validation_display.queue_commit();
        let next = state.bump(dir, cx);
        cx.notify();
        next
    });
    let Some(next) = next else { return false };
    suppress_routed_server_errors(state, is_invalid, validation_errors, validate, cx);
    if let Some(callback) = on_change {
        callback(&next, window, cx);
    }
    true
}

// The pinned `.number-field--secondary` hover fill is
// `--number-field-group-bg-hover: var(--default-hover)`, which
// `RoleColor::hover()` computes; `soft_hover()` is a different, lighter
// token. The two accessors differ by one word and the wrong one still looks
// plausible on screen, so the check is mechanical.
#[cfg(test)]
mod hover_tokens {
    #[test]
    fn secondary_group_uses_the_pinned_default_hover_token() {
        // Scan the implementation only.
        let source = include_str!("number_field.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("FieldVariant::Secondary => colors.default.hover()"),
            "the secondary group hover must read `colors.default.hover()` \
             (pinned `--number-field-group-bg-hover: var(--default-hover)`)"
        );
    }

    #[test]
    fn stepper_cells_carry_the_shared_field_border_token() {
        let source = include_str!("number_field.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains(".border(field_border_width)")
                && source.contains(".border_color(colors.field.border)"),
            "spin buttons must preserve the configured field border token"
        );
    }

    #[test]
    fn group_chrome_uses_the_pinned_shell_transition() {
        let source = include_str!("number_field.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("crate::anim::field_chrome_ramp(")
                && source.contains("crate::anim::focus_ring_endpoint(cx)")
                && source.contains("crate::anim::danger_ring_endpoint(cx)"),
            "the group must move focus, invalid and hover chrome through the \
             shared keyed ramp, not one-frame endpoint swaps"
        );
    }
}

crate::util::impl_component_styled!(NumberField);
