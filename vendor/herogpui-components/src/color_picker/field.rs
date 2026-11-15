//! ColorField.

use super::*;

// ColorField
// ---------------------------------------------------------------------------

/// The complete state passed to [`ColorField::content`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ColorFieldRenderState {
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

/// ColorField — enters a color as text.
///
/// With no `channel` it edits the hex value; with one it edits that channel's
/// numeric value.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ColorField {
    /// See [`ColorField::content`].
    content: Option<Arc<dyn Fn(ColorFieldRenderState) -> gpui::AnyElement + 'static>>,
    /// `ColorField.Suffix` — the `me-3` slot after the value, in the
    /// placeholder colour. v3's own example fills the *prefix* with a swatch and
    /// leaves this to the caller (a channel unit, a lock icon).
    suffix: Option<gpui::AnyElement>,
    /// `validationBehavior` — carried on this control's form field.
    validation_behavior: crate::form::ValidationBehavior,
    /// `name` — the name this control submits under; read back by
    /// [`Self::form_field`].
    name: Option<SharedString>,
    /// `aria-label` — the explicit accessible name, which wins over the
    /// label-derived name exactly as the HTML name computation ranks it.
    aria_label: Option<SharedString>,
    /// `defaultValue` — set it to hand this component its own state. The
    /// outer option distinguishes an omitted prop from an explicit `null`
    /// seed, matching React Aria's `Color | null` contract.
    default_value: Option<Option<PickerColor>>,
    id: ElementId,
    /// The controlled value. `None` is the documented empty color state.
    value: Option<PickerColor>,
    channel: Option<ColorChannel>,
    /// `colorSpace` — how a `channel` value is interpreted.
    color_space: ColorSpace,
    /// `validate` — run by the component, not the caller.
    validate: Option<crate::validation::Validator<PickerColor>>,
    /// `validationErrors` — messages from a server round-trip.
    validation_errors: Vec<SharedString>,
    /// `isWheelDisabled` — stops the scroll wheel from stepping the channel.
    is_wheel_disabled: bool,
    /// `autoFocus` — take focus on the first render.
    auto_focus: bool,
    placeholder: Option<SharedString>,
    /// The value text's family; unset keeps the inherited family.
    font_family: Option<SharedString>,
    /// Supplying an `InputState` makes the field editable; without one it is a
    /// read-only display of `value`.
    state: Option<Entity<crate::input::InputState>>,
    on_change: Option<OnColorFieldChange>,
    /// `onBlur` — focus left the editable field. Fired just before the
    /// built-in revert-on-blur, the way React Aria chains the caller's
    /// `onBlur` ahead of its `commit` on the same event.
    on_blur: Option<Arc<dyn Fn(&mut Window, &mut App) + 'static>>,
    label: Option<SharedString>,
    description: Option<SharedString>,
    variant: FieldVariant,
    full_width: bool,
    is_disabled: bool,
    is_invalid: bool,
    is_read_only: bool,
    is_required: bool,
    /// Optional box geometry/chrome overrides; defaults are the stock box.
    field: util::FieldBox,
    /// The corner radius, in place of the owning `field_radius` helper.
    radius: Option<Pixels>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
    form_state: Rc<RefCell<crate::form::LiveFormFieldState>>,
}

impl ColorField {
    /// Sets whether the field is read-only (`isReadOnly`).
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.is_read_only = v;
        self
    }

    /// Sets whether the field is required (`isRequired`).
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// v3's field `children`-as-a-function, handed the complete resolved field
    /// state.
    pub fn content(
        mut self,
        render: impl Fn(ColorFieldRenderState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.content = Some(Arc::new(render));
        self
    }

    /// `ColorField.Suffix` — the slot after the value.
    pub fn suffix(mut self, el: impl IntoElement) -> Self {
        self.suffix = Some(el.into_any_element());
        self
    }

    /// The family the value text is drawn with, on both paths: the editable
    /// field forwards it to the [`crate::Input`] it composes, so the caret
    /// measurement uses it too (see [`crate::Input::font_family`]), and the
    /// static display sets it on its box. Unset keeps the inherited family.
    /// Not a v3 prop; v3 sets it with a class.
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the field's root element — the column holding the label,
    /// the box and the description or error — after every value the variant
    /// and the active theme chose, so they win. Both paths land on that
    /// column: the editable field hands the slot to the [`crate::Input`] it
    /// composes, whose standalone root is the same column, and the static
    /// display refines its own. The box's chrome stays with the variant.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }

    /// Creates a color field with the id `id`, showing `value` (or empty for `None`).
    pub fn new(id: impl Into<ElementId>, value: impl Into<Option<PickerColor>>) -> Self {
        Self {
            content: None,
            suffix: None,
            validation_behavior: crate::form::ValidationBehavior::Native,
            name: None,
            aria_label: None,
            default_value: None,
            id: id.into(),
            value: value.into(),
            channel: None,
            color_space: ColorSpace::default(),
            validate: None,
            validation_errors: Vec::new(),
            is_wheel_disabled: false,
            auto_focus: false,
            placeholder: None,
            font_family: None,
            state: None,
            on_change: None,
            on_blur: None,
            label: None,
            description: None,
            variant: FieldVariant::Primary,
            full_width: false,
            is_disabled: false,
            is_invalid: false,
            is_read_only: false,
            is_required: false,
            field: util::FieldBox::default(),
            radius: None,
            sx: None,
            form_state: live_color_form_state(
                crate::form::FormValue::Text(SharedString::default()),
            ),
        }
    }

    /// `validationBehavior` — `Allow` shows the message without blocking form
    /// submission. Carried on the [`Self::form_field`] this control produces.
    pub fn validation_behavior(mut self, behavior: crate::form::ValidationBehavior) -> Self {
        self.validation_behavior = behavior;
        self
    }

    /// `name` — the name this control submits under.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// `aria-label` — the explicit accessible name, which wins over the
    /// label-derived name exactly as the HTML name computation ranks it.
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
    /// # use herogpui_components::{ColorField, Form, PickerColor};
    /// # struct Demo;
    /// # impl Render for Demo {
    /// #     fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    /// #         let form = Form::new();
    /// #         let color = PickerColor::hsb(210., 0.8, 0.9);
    /// #         let control = ColorField::new("brand", color).name("brand");
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
        let value = self.default_value.unwrap_or(self.value);
        let validity = crate::validation::resolve(
            self.is_invalid,
            &self.validation_errors,
            self.validate
                .as_ref()
                .and_then(|f| value.as_ref().and_then(|value| f(value))),
            None,
        );
        sync_color_form_state(
            &self.form_state,
            color_field_form_value(value, self.channel, self.color_space),
            !self.is_disabled,
            validity.is_invalid,
        );
        Some(
            crate::form::FormField::live(name, self.form_state.clone())
                .is_required(self.is_required)
                .validation_behavior(self.validation_behavior),
        )
    }

    /// `defaultValue` — the uncontrolled initial colour. Passing `None`
    /// explicitly seeds the field empty; omitting this builder keeps the
    /// component controlled by the value supplied to [`Self::new`].
    ///
    /// Supplying it hands the component its own state: the constructor's
    /// `value` becomes the seed, and a change moves the component's copy.
    pub fn default_value(mut self, value: impl Into<Option<PickerColor>>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    /// `colorSpace` — the space a `channel` value is read in.
    pub fn color_space(mut self, space: ColorSpace) -> Self {
        self.color_space = space;
        self
    }

    /// `validate` — returns the message to show, or `None` when the colour is
    /// fine. The component runs it and surfaces the result.
    pub fn validate(mut self, f: impl Fn(&PickerColor) -> Option<SharedString> + 'static) -> Self {
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

    /// `autoFocus` — take focus on the first render. Only meaningful in the
    /// editable mode; see [`ColorField::state`].
    pub fn auto_focus(mut self, v: bool) -> Self {
        self.auto_focus = v;
        self
    }

    /// `isWheelDisabled` — stops the wheel from stepping the channel.
    ///
    /// Only a single-channel field steps: there is no sensible increment for a
    /// hex value.
    pub fn is_wheel_disabled(mut self, v: bool) -> Self {
        self.is_wheel_disabled = v;
        self
    }

    /// `placeholder` on `ColorField.Input`.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Makes the field editable, backed by this text state.
    pub fn state(mut self, state: Entity<crate::input::InputState>) -> Self {
        self.state = Some(state);
        self
    }

    /// `onChange` — the parsed colour, or `None` when the text is not one.
    ///
    /// Only fires in the editable mode; see [`ColorField::state`].
    pub fn on_change(
        mut self,
        f: impl Fn(&Option<PickerColor>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(f));
        self
    }

    /// `onBlur` — the editable field lost focus.
    ///
    /// React Aria chains the caller's `onBlur` with its own blur `commit` on
    /// one event, caller first, so this hook runs ahead of the built-in
    /// revert-on-blur and still sees the raw text. The revert itself is built
    /// in and needs no hook. Only meaningful in the editable mode; see
    /// [`ColorField::state`].
    pub fn on_blur(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_blur = Some(Arc::new(f));
        self
    }

    /// Edit one channel instead of the hex value.
    pub fn channel(mut self, channel: ColorChannel) -> Self {
        self.channel = Some(channel);
        self
    }

    /// Sets the label shown above the field.
    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// Sets the description shown below the field.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
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

    /// Sets whether the field is disabled (`isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets whether the field is invalid (`isInvalid`).
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// Replaces the 36px box height. The editable path forwards it to the
    /// inner Input; the static display box changes its own height.
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
    /// for a caller painting around it. The field stays editable and focusable.
    pub fn is_bare(mut self, v: bool) -> Self {
        self.field.is_bare = v;
        self.field.is_bare_is_set = true;
        self
    }

    /// Shows or hides only the field's visual focus ring. The editable color
    /// field remains focusable and the static display keeps its normal chrome.
    pub fn focus_ring(mut self, v: bool) -> Self {
        self.field.focus_ring = Some(v);
        self
    }

    /// The corner radius, in place of the owning `field_radius` helper. Not a
    /// v3 prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    ///
    /// The static box and shared field chrome use the same resolved radius;
    /// a bare box retains it without painting chrome. The editable path
    /// forwards the override to its inner field alongside `height` and
    /// `padding_x`.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }
}

impl ColorField {
    /// The text form of the current value, honouring `channel` and
    /// `colorSpace`.
    fn display_text(&self) -> String {
        color_field_display_text(self.value, self.channel, self.color_space)
    }
}

pub(super) fn parse_color_field(
    value: PickerColor,
    channel: Option<ColorChannel>,
    color_space: ColorSpace,
    text: &str,
) -> Option<PickerColor> {
    match channel {
        None => PickerColor::from_hex(text),
        Some(channel) => {
            let text = text.trim();
            let mut number: f32 = text.trim_end_matches('%').trim().parse().ok()?;
            if is_normalized_channel(channel) {
                number /= 100.0;
            }
            let (min, max) = channel.range();
            if number < min || number > max {
                return None;
            }
            Some(value.with_channel_in(channel, color_space, number))
        }
    }
}

pub(super) fn step_color_channel(
    value: PickerColor,
    channel: ColorChannel,
    color_space: ColorSpace,
    direction: f32,
) -> PickerColor {
    let (min, max) = channel.range();
    let step = color_channel_step(channel);
    let current = value.channel_in(channel, color_space);
    let next = snap_color_channel(channel, (current + direction * step).clamp(min, max));
    value.with_channel_in(channel, color_space, next)
}

pub(super) fn color_channel_step(channel: ColorChannel) -> f32 {
    let (min, max) = channel.range();
    if max - min > 2.0 {
        1.0
    } else {
        0.01
    }
}

pub(super) fn snap_color_channel(channel: ColorChannel, value: f32) -> f32 {
    let (min, max) = channel.range();
    let step = color_channel_step(channel);
    (((value - min) / step).round() * step + min).clamp(min, max)
}

pub(super) fn is_normalized_channel(channel: ColorChannel) -> bool {
    matches!(
        channel,
        ColorChannel::Saturation
            | ColorChannel::Brightness
            | ColorChannel::Lightness
            | ColorChannel::Alpha
    )
}

pub(super) fn format_color_channel_value(
    value: PickerColor,
    channel: ColorChannel,
    color_space: ColorSpace,
) -> String {
    let value = value.channel_in(channel, color_space);
    if is_normalized_channel(channel) {
        format!("{}%", (value * 100.0).round())
    } else {
        format!("{}", value.round())
    }
}

#[allow(clippy::too_many_arguments)] // mirrors the state, callback and field channels in one event
pub(super) fn report_color_field_change(
    next: PickerColor,
    channel: ColorChannel,
    color_space: ColorSpace,
    state: &Entity<crate::input::InputState>,
    own: &Option<Entity<Option<PickerColor>>>,
    on_change: &Option<OnColorFieldChange>,
    window: &mut Window,
    cx: &mut App,
) {
    let text = format_color_channel_value(next, channel, color_space);
    state.update(cx, |state, cx| {
        state.set_value(text);
        cx.notify();
    });
    if let Some(held) = own {
        held.update(cx, |value, cx| {
            *value = Some(next);
            cx.notify();
        });
    }
    if let Some(callback) = on_change {
        callback(&Some(next), window, cx);
    }
}

impl RenderOnce for ColorField {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `defaultValue` opts into the component holding its own colour;
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (resolved, own) = util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "field-value"),
            self.default_value.is_none().then_some(self.value),
            self.default_value.unwrap_or(self.value),
        );
        self.value = resolved;
        // v3 order: the controlled flag, then server errors, then `validate`.
        let validate_result = self
            .validate
            .as_ref()
            .and_then(|f| self.value.as_ref().and_then(|value| f(value)));
        let validity = crate::validation::resolve(
            self.is_invalid,
            &self.validation_errors,
            validate_result.clone(),
            None,
        );
        // React Stately's commit-gated display: `validate` alone is the
        // pending half, synced into a keyed snapshot every render; blur,
        // Enter and every channel step commit it. `display_validity` composes
        // what the field paints — the controlled flag and server messages
        // stay live, the gated half is the snapshot.
        let pending = crate::validation::resolve(false, &[], validate_result, None);
        let validation_display =
            window.use_keyed_state(element_id::scoped(&self.id, "validation"), cx, |_, _| {
                crate::validation::ValidationDisplay::default()
            });
        validation_display.update(cx, |d, _| d.sync(pending));
        let displayed = validation_display.read(cx).displayed().clone();
        let display = crate::validation::display_validity(
            self.validation_behavior,
            &validity,
            self.is_invalid,
            &self.validation_errors,
            &displayed,
            None,
        );
        if let Some(render) = self.content.clone() {
            // v3's field children-as-a-function: the caller builds the parts.
            let focused = self
                .state
                .as_ref()
                .is_some_and(|s| s.read(cx).focus_handle.is_focused(window));
            let within = self
                .state
                .as_ref()
                .is_some_and(|s| s.read(cx).focus_handle.contains_focused(window, cx));
            return render(ColorFieldRenderState {
                is_disabled: self.is_disabled,
                is_invalid: display.is_invalid,
                is_read_only: self.is_read_only,
                is_required: self.is_required,
                is_focused: focused,
                is_focus_within: within,
                is_focus_visible: focused && util::focus_visible(cx),
            })
            .into_any_element();
        }
        let form_default = window.use_keyed_state(
            element_id::scoped(&self.id, "field-form-default"),
            cx,
            |_, _| None::<Option<PickerColor>>,
        );
        if form_default.read(cx).is_none() {
            let initial = self.value;
            form_default.update(cx, |slot, cx| {
                *slot = Some(initial);
                cx.notify();
            });
        }
        let restore_default = form_default.read(cx).unwrap_or(self.value);
        // Submit the resolved colour. Uncontrolled keyed state is current after
        // a parsed change; a controlled owner must accept it first.
        sync_color_form_state(
            &self.form_state,
            color_field_form_value(self.value, self.channel, self.color_space),
            !self.is_disabled,
            validity.is_invalid,
        );
        // The form's submit/reset reach the snapshot through these closures;
        // `commit`/`reset` notify only when they changed the displayed half.
        {
            let (commit, reset) = crate::validation::form_hooks(&validation_display);
            let mut form = self.form_state.borrow_mut();
            form.commit_validation = Some(commit);
            form.reset_validation = Some(reset);
        }
        let restore_own = own.clone();
        let restore_on_change = self.on_change.clone();
        // The field stores this callback; owning it back would retain every
        // rendered InputState after the field is removed.
        let restore_form_state = Rc::downgrade(&self.form_state);
        let restore_input = self.state.clone();
        let restore_channel = self.channel;
        let restore_space = self.color_space;
        let restore_is_disabled = self.is_disabled;
        let restore: Arc<dyn Fn(&mut Window, &mut App)> = util::shared(move |window, cx| {
            if let Some(own) = &restore_own {
                own.update(cx, |current, cx| {
                    *current = restore_default;
                    cx.notify();
                });
            }
            if let Some(state) = &restore_input {
                let text =
                    color_field_display_text(restore_default, restore_channel, restore_space);
                state.update(cx, |state, cx| {
                    state.set_value(text);
                    cx.notify();
                });
            }
            if let Some(callback) = &restore_on_change {
                callback(&restore_default, window, cx);
            }
            if let Some(state) = restore_form_state.upgrade() {
                sync_color_form_state(
                    &state,
                    color_field_form_value(restore_default, restore_channel, restore_space),
                    !restore_is_disabled,
                    false,
                );
            }
        });
        self.form_state.borrow_mut().restore = Some(restore);
        if let Some(state) = &self.state {
            self.form_state.borrow_mut().focus = Some(state.read(cx).focus_handle.clone());
        }
        // The shared hover tween borrows the window/app mutably while the
        // final field chrome still needs theme tokens below, so copy the
        // active snapshots before constructing the element tree.
        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        let text = self.display_text();

        // Editable mode: delegate the text handling to Input and parse on every
        // keystroke, so `onChange` reports exactly what v3's does.
        if let Some(state) = self.state.clone() {
            // React Aria also commits on blur: `useColorField` merges
            // `onBlur: commit` into the input's props, and a channel field
            // inherits NumberField's blur commit. `commit` restores text that
            // no longer parses to the formatted last-committed value, leaves
            // valid text alone, and keeps an emptied field empty (its
            // committed null). The revert itself runs in the blur path —
            // event handlers may mutate keyed state; render must not — so the
            // only render-time work below is observing that a blur happened.
            // GPUI blanks focus-event paths for inactive windows, including
            // its headless test platform, so the observation has two legs,
            // the same split `util::on_focus_leave` makes: a
            // `Window::on_focus_out` on the input's own focus handle serves
            // active windows, and a render-time edge catches the focus move
            // to another tab stop everywhere else and defers the same revert
            // out of render. Both consume the same flag, so one blur reverts
            // exactly once.
            let blur_focus = state.read(cx).focus_handle.clone();
            let blur_seen = window.use_keyed_state(
                element_id::scoped(&self.id, "field-blur-seen"),
                cx,
                |_, _| false,
            );
            let blur_subscription = window.use_keyed_state(
                element_id::scoped(&self.id, "field-blur-subscription"),
                cx,
                |_, _| None::<gpui::Subscription>,
            );
            // The text the field held when focus arrived; the blur commit
            // below compares it to the departing text, as the native `change`
            // event does. `Some` doubles as "a focus session is open".
            let focus_text = window.use_keyed_state(
                element_id::scoped(&self.id, "field-focus-text"),
                cx,
                |_, _| None::<String>,
            );
            let revert: Rc<dyn Fn(&mut Window, &mut App)> = {
                let hook = self.on_blur.clone();
                let seen = blur_seen.clone();
                let focus_entry = focus_text.clone();
                let commit_display = validation_display.clone();
                let input = state.downgrade();
                let committed = self.value;
                let channel = self.channel;
                let space = self.color_space;
                Rc::new(move |window, cx| {
                    seen.update(cx, |seen, _| *seen = false);
                    // React Aria chains the caller's `onBlur` ahead of its
                    // `commit`, so the hook still sees the raw text.
                    if let Some(hook) = &hook {
                        hook(window, cx);
                    }
                    let Some(input) = input.upgrade() else {
                        return;
                    };
                    let text = input.read(cx).value().to_owned();
                    // Commit-gated display: the blur commits the snapshot when
                    // the text moved from its focus-entry value, or when it is
                    // non-empty and parses — React Aria's `commit` on blur.
                    // This runs ahead of the revert so it sees the raw text,
                    // and consumes the entry so one blur commits once.
                    let entry = focus_entry.read(cx).clone();
                    focus_entry.update(cx, |slot, _| *slot = None);
                    let changed = entry.is_some_and(|arrival| arrival != text);
                    let parses = !text.is_empty()
                        && parse_color_field(committed.unwrap_or_default(), channel, space, &text)
                            .is_some();
                    if changed || parses {
                        commit_display.update(cx, |d, cx| {
                            if d.commit() {
                                cx.notify();
                            }
                        });
                    }
                    // An emptied field is React Aria's committed null: the
                    // text stays empty instead of restoring the old colour.
                    if text.is_empty()
                        || parse_color_field(committed.unwrap_or_default(), channel, space, &text)
                            .is_some()
                    {
                        return;
                    }
                    let restored = color_field_display_text(committed, channel, space);
                    input.update(cx, |state, cx| {
                        state.set_value(restored);
                        cx.notify();
                    });
                })
            };
            if !self.is_disabled {
                // The event listener is frame-scoped: every render re-arms it
                // with the `revert` built from this frame's committed value.
                // Arming once would pin the first render's seed onto every
                // later blur while the keyed colour advances per keystroke;
                // storing the fresh subscription drops the stale one. The
                // listener also clears the slot when it fires — the same
                // one-shot transition `util::on_focus_leave` draws — so a
                // blur owns its revert exactly once and the next frame
                // re-arms.
                let disarmer = blur_subscription.downgrade();
                let listener = window.on_focus_out(&blur_focus, cx, {
                    let revert = Rc::clone(&revert);
                    move |_, window, cx| {
                        if let Some(disarmer) = disarmer.upgrade() {
                            disarmer.update(cx, |slot, _| *slot = None);
                        }
                        revert(window, cx);
                    }
                });
                blur_subscription.update(cx, |slot, _| *slot = Some(listener));
                if blur_focus.is_focused(window) {
                    blur_seen.update(cx, |seen, _| *seen = true);
                    // Remember the focus-entry text once per focus session;
                    // the blur commit compares the departing text against it.
                    if focus_text.read(cx).is_none() {
                        let entry = state.read(cx).value().to_owned();
                        focus_text.update(cx, |slot, _| *slot = Some(entry));
                    }
                } else if *blur_seen.read(cx)
                    && window.focused(cx).is_some_and(|focused| focused.tab_stop)
                {
                    // The inactive-window leg: the armed event never fires
                    // there, so this frame's render observed the departure.
                    // Consuming the flag here is what keeps exactly one
                    // deferred revert per blur, and the revert itself runs
                    // out of render, the way the event leg above does.
                    blur_seen.update(cx, |seen, _| *seen = false);
                    window.defer(cx, {
                        let revert = Rc::clone(&revert);
                        move |window, cx| revert(window, cx)
                    });
                }
            } else if blur_subscription.read(cx).is_some() {
                blur_subscription.update(cx, |slot, _| *slot = None);
            }
            let mut input = Input::new(state.clone())
                .variant(self.variant)
                .is_disabled(self.is_disabled)
                .is_read_only(self.is_read_only)
                .is_required(self.is_required)
                .is_invalid(display.is_invalid)
                .display_override(display.clone())
                .validation_errors(self.validation_errors.clone())
                .auto_focus(self.auto_focus);
            if let Some(value) = self.value {
                input = input.start_content(ColorSwatch::new(value).size(SizeXl::Xs));
            }
            input = input
                .with_field_box(self.field)
                .with_sx_refinement(self.sx.take());
            if let Some(family) = self.font_family.clone() {
                input = input.font_family(family);
            }
            // The editable box is the inner field's own, so the radius rides
            // along with the field box, the way its `height` and `padding_x`
            // do; the static box below paints its own.
            input = match self.radius {
                Some(radius) => input.radius(radius),
                None => input,
            };
            if let Some(message) = display.first() {
                input = input.error_message(message);
            }
            if let Some(ph) = self.placeholder.clone() {
                input = input.placeholder(ph);
            } else {
                input = input.placeholder(text);
            }
            if let Some(label) = self.label.clone() {
                input = input.label(label);
            }
            if let Some(description) = self.description.clone() {
                input = input.description(description);
            }
            if let Some(suffix) = self.suffix.take() {
                input = input.end_content(
                    div()
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .text_color(colors.field.placeholder)
                        .child(suffix),
                );
            }
            if self.full_width {
                input = input.full_width();
            }
            if self.on_change.is_some() || own.is_some() {
                let cb = self.on_change.clone();
                let own = own.clone();
                let parse_value = self.value.unwrap_or_default();
                let parse_channel = self.channel;
                let parse_space = self.color_space;
                input = input.on_change(move |text, window, cx| {
                    let next = parse_color_field(parse_value, parse_channel, parse_space, text);
                    // Uncontrolled: keep what was typed, or the swatch would
                    // never follow the text.
                    if let (Some(held), Some(c)) = (&own, next) {
                        held.update(cx, |v, cx| {
                            *v = Some(c);
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &cb {
                        cb(&next, window, cx);
                    }
                });
            }
            let rendered = input.render(window, cx).into_any_element();
            if self.is_disabled || self.is_read_only {
                return rendered;
            }
            let Some(channel) = self.channel else {
                // A hex field has no channel to step, but Enter still commits
                // the gated display; the key is not swallowed, so a form's
                // implicit submit still sees it.
                let enter_commit = validation_display.clone();
                return div()
                    .id(element_id::scoped(&self.id, "hex-events"))
                    .child(rendered)
                    .when(self.full_width, |el| el.w_full())
                    .on_key_down(move |event, _window, cx| {
                        if event.keystroke.key == "enter" {
                            crate::validation::queue_commit(&enter_commit, cx);
                        }
                    })
                    .into_any_element();
            };

            let mut field = div()
                .id(element_id::scoped(&self.id, "channel-events"))
                .child(rendered);
            if self.on_change.is_some() || own.is_some() {
                let key_value = self.value.unwrap_or_default();
                let key_space = self.color_space;
                let key_state = state.clone();
                let key_own = own.clone();
                let key_change = self.on_change.clone();
                let key_commit = validation_display.clone();
                field = field.on_key_down(move |event, window, cx| {
                    // Enter commits the gated display but is not swallowed, so
                    // a surrounding form's implicit submit still sees it.
                    if event.keystroke.key == "enter" {
                        crate::validation::queue_commit(&key_commit, cx);
                        return;
                    }
                    let direction = match event.keystroke.key.as_str() {
                        "up" => 1.0,
                        "down" => -1.0,
                        _ => return,
                    };
                    let next = step_color_channel(key_value, channel, key_space, direction);
                    report_color_field_change(
                        next,
                        channel,
                        key_space,
                        &key_state,
                        &key_own,
                        &key_change,
                        window,
                        cx,
                    );
                    crate::validation::queue_commit(&key_commit, cx);
                    cx.stop_propagation();
                });

                if !self.is_wheel_disabled {
                    let wheel_value = self.value.unwrap_or_default();
                    let wheel_space = self.color_space;
                    let wheel_state = state;
                    let wheel_own = own;
                    let wheel_change = self.on_change;
                    let wheel_commit = validation_display.clone();
                    field = field.on_scroll(move |event, window, cx| {
                        if !wheel_state
                            .read(cx)
                            .focus_handle
                            .contains_focused(window, cx)
                        {
                            return;
                        }
                        let (dx, dy) = match event.delta {
                            gpui::ScrollDelta::Pixels(point) => {
                                (f32::from(point.x), f32::from(point.y))
                            }
                            gpui::ScrollDelta::Lines(point) => (point.x, point.y),
                        };
                        if dy == 0.0 || dy.abs() <= dx.abs() {
                            return;
                        }
                        let next =
                            step_color_channel(wheel_value, channel, wheel_space, dy.signum());
                        report_color_field_change(
                            next,
                            channel,
                            wheel_space,
                            &wheel_state,
                            &wheel_own,
                            &wheel_change,
                            window,
                            cx,
                        );
                        crate::validation::queue_commit(&wheel_commit, cx);
                        cx.stop_propagation();
                    });
                }
            }
            return field.into_any_element();
        }

        let field_box = self.field;
        // The box and shared field chrome use the same resolved radius.
        let radius = self.radius.unwrap_or_else(|| util::field_radius(cx));
        let mut field = div()
            .id(self.id.clone())
            .when_some(self.font_family.clone(), |field, family| {
                field.font_family(family)
            })
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(field_box.resolved_padding_x())
            .h(field_box.resolved_height())
            .rounded(radius)
            .text_size(util::FIELD_TEXT)
            .line_height(px(20.))
            .text_color(colors.field.foreground);
        if !field_box.is_bare {
            let focused = self
                .state
                .as_ref()
                .is_some_and(|s| s.read(cx).focus_handle.is_focused(window));
            // The colour field box does not clip its children, so its focus
            // and invalid rings are painted as concentric overlay children
            // rather than as blurred spread shadows.
            field = util::apply_field_chrome_overlay(
                field,
                self.variant,
                self.is_invalid,
                focused,
                field_box.focus_ring.unwrap_or(true),
                Some(radius),
                cx,
            );

            // HeroUI's invalid color-input group keeps the danger chrome and
            // swaps the surface to the field-focus endpoint. Secondary uses
            // its neutral default focus endpoint, just like InputGroup.
            if display.is_invalid {
                field = field.bg(match self.variant {
                    FieldVariant::Primary => colors.field.focus(),
                    FieldVariant::Secondary => colors.default.color,
                });
            }

            if !self.is_disabled && !self.is_invalid && !focused {
                let idle_bg = match self.variant {
                    FieldVariant::Primary => colors.field.background,
                    FieldVariant::Secondary => colors.default.color,
                };
                let hover_bg = match self.variant {
                    FieldVariant::Primary => colors.field.hover(),
                    // `.color-input-group--secondary` uses the default-hover
                    // endpoint rather than the field-hover token.
                    FieldVariant::Secondary => colors.default.hover(),
                };
                let hover_border = colors.field.border_hover();
                // Keep the group identity and focus/scroll listeners stable;
                // only the visual fill interpolates over HeroUI's 150ms
                // ease-smooth hover transition. The border endpoint remains
                // an immediate hover refinement, matching the field family.
                field = crate::anim::hover_fade_with_duration_and_easing(
                    field,
                    element_id::scoped(&self.id, "field-hover-fade"),
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

        // The content joins the skin after the fade's fill — the fill is
        // appended and would paint over a child already present.
        // `.color-input-group__prefix` is `shrink-0 ms-3` in the placeholder
        // colour. An empty ColorField omits the swatch, matching
        // `ColorSwatch color={value ?? undefined}` in the pinned example.
        if let Some(value) = self.value {
            field = field.child(ColorSwatch::new(value).size(SizeXl::Xs));
        }
        field = field.child(div().flex_1().child(text));
        // `.color-input-group__suffix` is `shrink-0 text-field-placeholder
        // me-3 flex items-center`; the trailing inset is the field box's
        // resolved padding, since this port renders the value itself.
        field = field.children(self.suffix.map(|el| {
            div()
                .flex()
                .items_center()
                .flex_shrink_0()
                .text_color(colors.field.placeholder)
                .child(el)
        }));

        // v3's ColorField steps its channel on scroll; `isWheelDisabled` turns
        // that off. There is no sensible increment for a hex value, so only a
        // single-channel field responds.
        if let (Some(channel), false, Some(cb)) = (
            self.channel,
            self.is_wheel_disabled || self.is_disabled || self.is_read_only,
            self.on_change.clone(),
        ) {
            let value = self.value.unwrap_or_default();
            let space = self.color_space;
            let scroll_commit = validation_display.clone();
            field = field.on_scroll(move |ev: &gpui::ScrollEvent, window, cx| {
                let dy = match ev.delta {
                    gpui::ScrollDelta::Pixels(p) => f32::from(p.y),
                    gpui::ScrollDelta::Lines(p) => p.y,
                };
                if dy == 0.0 {
                    return;
                }
                let (min, max) = channel.range();
                // One notch is a percent of the channel's range, so hue moves
                // in degrees and an 8-bit channel in whole steps.
                let step = ((max - min) / 100.0).max(1.0);
                let next = (value.channel_in(channel, space) + step * dy.signum()).clamp(min, max);
                cb(
                    &Some(value.with_channel_in(channel, space, next)),
                    window,
                    cx,
                );
                crate::validation::queue_commit(&scroll_commit, cx);
            });
        }

        if display.is_invalid && !field_box.is_bare {
            field = field.border_1().border_color(colors.danger.color);
        }
        // A read-only field is legible but not interactive, so it reads the
        // same as disabled here (there is no editing affordance to remove).
        if self.is_disabled || self.is_read_only {
            field = field.opacity(layout.disabled_opacity);
        }
        if self.full_width {
            field = field.w_full();
        } else {
            field = field.w(px(200.));
        }
        // `useColorField` is `role: 'textbox'` on the input. The hex path
        // that composes `Input` lets that field carry the node instead.
        field = field.a11y_named(
            a11y::Role::TextInput,
            &a11y::Name::field(
                self.aria_label
                    .as_ref()
                    .or(self.label.as_ref())
                    .or(self.name.as_ref()),
                self.description.as_ref(),
                &display,
            ),
        );

        // `.color-field` is `flex flex-col gap-1`.
        let mut root = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .when(self.full_width, |root| root.w_full());
        if let Some(label) = self.label {
            root = root.child(
                crate::field::Label::new(label)
                    .is_required(self.is_required)
                    .is_disabled(self.is_disabled)
                    .is_invalid(self.is_invalid),
            );
        }
        root = root.child(field);
        if let Some(description) = self.description {
            root = root.child(crate::field::Description::new(description));
        }
        util::apply_sx(root, &self.sx).into_any_element()
    }
}

// ---------------------------------------------------------------------------

crate::util::impl_component_styled!(ColorField);
