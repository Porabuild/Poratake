//! RadioGroup — port of `@heroui/radio`.

use std::{cell::RefCell, rc::Rc};

use gpui::{prelude::*, px, App, IntoElement, Pixels, RenderOnce, SharedString, Styled, Window};
use herogpui_core::{element_id, Color, FieldVariant, Orientation};
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};

/// One radio's visible label, submitted value, and local disabled state.
#[must_use = "builder methods return a new value; pass the option to its component"]
#[derive(Clone)]
pub struct RadioOption {
    label: SharedString,
    value: SharedString,
    is_disabled: bool,
    description: Option<SharedString>,
    error_message: Option<SharedString>,
}

impl RadioOption {
    /// Creates an option from a label; the submitted value defaults to the label.
    pub fn new(label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            value: label.clone(),
            label,
            is_disabled: false,
            description: None,
            error_message: None,
        }
    }

    /// `value` — submitted and reported independently of the visible label.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = value.into();
        self
    }

    /// `isDisabled` — disables this option only.
    pub fn is_disabled(mut self, value: bool) -> Self {
        self.is_disabled = value;
        self
    }

    /// `Description` — help text below this option's clickable content.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// `FieldError` — validation text below this option's clickable content.
    pub fn error_message(mut self, text: impl Into<SharedString>) -> Self {
        self.error_message = Some(text.into());
        self
    }
}

impl From<SharedString> for RadioOption {
    fn from(label: SharedString) -> Self {
        Self::new(label)
    }
}

impl From<String> for RadioOption {
    fn from(label: String) -> Self {
        Self::new(label)
    }
}

impl From<&str> for RadioOption {
    fn from(label: &str) -> Self {
        Self::new(label.to_owned())
    }
}

/// Field state handed to the root `Radio` and `Radio.Indicator` renderers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RadioOptionState {
    /// Whether this radio is selected.
    pub is_selected: bool,
    /// Whether this radio is disabled.
    pub is_disabled: bool,
    /// Whether this radio is read-only.
    pub is_read_only: bool,
    /// Whether this radio is invalid.
    pub is_invalid: bool,
    /// Whether this radio is required.
    pub is_required: bool,
}

/// HeroGPUI-only compact size for a [`RadioGroup`].
///
/// | Metric (pixels) | Sm | Md (pinned default) |
/// | --- | --- | --- |
/// | Control / selected dot / pressed dot | 14 / 5 / 8 | 16 / 6 / 8 |
/// | Label text / line height | 12 / 16 | 14 / 20 |
/// | Control-to-label gap / supporting-text indent | 10 / 24 | 12 / 28 |
///
/// Both steps have zero row padding and no extra minimum hitbox: each clickable
/// row includes its control and label and grows with caller content. Options
/// remain 16px apart in either orientation; horizontal groups wrap. Supporting
/// text remains 12/16 with a 4px vertical gap. Control and dot keep `key_radius`;
/// `radius` overrides only the control. Press scales the control by 0.95 when
/// motion is enabled; the selected dot's pressed size remains 8px in both steps.
///
/// v3.2.4 removed the field `size` prop, so this is additive: `Md` is
/// byte-identical to the pinned default and `Sm` is HeroGPUI's own 14px step.
/// Not a v3 prop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RadioSize {
    /// The small size.
    Sm,
    /// The medium size.
    #[default]
    Md,
}

impl RadioSize {
    /// Every size, in display order.
    pub const ALL: [RadioSize; 2] = [Self::Sm, Self::Md];

    /// `(control, dot, label text, row gap)` for this step.
    fn metrics(self) -> (Pixels, Pixels, Pixels, Pixels) {
        match self {
            Self::Sm => (px(14.), px(5.), px(12.), px(10.)),
            Self::Md => (px(16.), px(6.), px(14.), px(12.)),
        }
    }

    /// The display name of this size.
    pub fn label(self) -> &'static str {
        match self {
            Self::Sm => "Small",
            Self::Md => "Medium",
        }
    }
}

/// HeroUI RadioGroup.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct RadioGroup {
    /// `name` — the name this control submits under; read back by
    /// [`Self::form_field`].
    name: Option<SharedString>,
    id: gpui::ElementId,
    options: Vec<RadioOption>,
    /// Mirrors the current value, validity, successful state, focus and reset
    /// behavior for a live [`crate::form::FormField`].
    form_state: Rc<RefCell<crate::form::LiveFormFieldState>>,
    /// `Radio`'s `children`-as-a-function: handed the option and its state.
    option_content:
        Option<std::sync::Arc<dyn Fn(&SharedString, RadioOptionState) -> gpui::AnyElement>>,
    /// `Radio.Indicator` children — replaces the built-in dot per option.
    indicator: Option<std::sync::Arc<dyn Fn(&SharedString, RadioOptionState) -> gpui::AnyElement>>,
    /// The `<Description>` v3 composes inside a `<Radio>`, per option and in the
    /// same order. `.radio` is `flex flex-col gap-1` around its content and this
    /// text, indented under the label by `ps-7`.
    descriptions: Vec<Option<SharedString>>,
    /// The group's own `<Label>`, `<Description>` and `<FieldError>`. v3
    /// composes all three inside `<RadioGroup>` -- every documented example
    /// opens with `<Label>Plan selection</Label>` -- and a monolithic group takes
    /// them as props, the way `CheckboxGroup` does.
    label: Option<SharedString>,
    description: Option<SharedString>,
    error_message: Option<SharedString>,
    selected: Option<usize>,
    /// Whether `value` was supplied. `Option<usize>` cannot distinguish
    /// "controlled, nothing selected" from "uncontrolled" on its own.
    is_controlled: bool,
    default_value: Option<usize>,
    orientation: Orientation,
    is_disabled: bool,
    variant: FieldVariant,
    is_invalid: bool,
    is_required: bool,
    is_read_only: bool,
    on_change: Option<std::sync::Arc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>>,
    /// The compact step; `Md` is the pinned default.
    size: RadioSize,
    /// The option labels' font size, in place of the size step's.
    text_size: Option<Pixels>,
    /// The control circle's corner radius, in place of the owning `key_radius`
    /// helper. The control's pressed box follows it; the selected dot inside
    /// keeps its own.
    radius: Option<Pixels>,
    /// Expands the root to the available width.
    full_width: bool,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl RadioGroup {
    /// Sets the field variant (v3 `variant`).
    pub fn variant(mut self, variant: FieldVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the invalid state (v3 `isInvalid`).
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// Sets the required state (v3 `isRequired`).
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// `isReadOnly` — the value is shown but cannot be changed.
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.is_read_only = v;
        self
    }

    /// `Radio`'s root render function — handed the option's label and v3's
    /// field state: selected, disabled, read-only, invalid and required.
    pub fn option_content(
        mut self,
        render: impl Fn(&SharedString, RadioOptionState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.option_content = Some(std::sync::Arc::new(render));
        self
    }

    /// `Radio.Indicator` — draws each option's indicator from its field state.
    pub fn indicator(
        mut self,
        render: impl Fn(&SharedString, RadioOptionState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.indicator = Some(std::sync::Arc::new(render));
        self
    }

    /// The per-option descriptions, in the order the options were given. v3
    /// writes one `<Description>` inside each `<Radio>`; a monolithic group
    /// takes the column instead.
    pub fn descriptions<T: Into<SharedString>>(
        mut self,
        text: impl IntoIterator<Item = Option<T>>,
    ) -> Self {
        self.descriptions = text.into_iter().map(|opt| opt.map(Into::into)).collect();
        self
    }

    /// Creates a radio group from an element id and its options.
    pub fn new(id: impl Into<gpui::ElementId>, options: Vec<RadioOption>) -> Self {
        Self {
            name: None,
            id: id.into(),
            options,
            form_state: Rc::new(RefCell::new(crate::form::LiveFormFieldState {
                commit_validation: None,
                reset_validation: None,
                value: crate::form::FormValue::Text(SharedString::default()),
                is_invalid: false,
                is_successful: true,
                focus: None,
                restore: None,
            })),
            option_content: None,
            indicator: None,
            descriptions: Vec::new(),
            label: None,
            description: None,
            error_message: None,
            selected: None,
            is_controlled: false,
            default_value: None,
            orientation: Orientation::Vertical,
            is_disabled: false,
            variant: FieldVariant::Primary,
            is_invalid: false,
            is_required: false,
            is_read_only: false,
            on_change: None,
            size: RadioSize::default(),
            text_size: None,
            radius: None,
            full_width: false,
            sx: None,
        }
    }

    /// The `<Label>` v3 composes inside the group.
    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// The `<Description>` v3 composes inside the group, below its options.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// The `<FieldError>` v3 composes inside the group; supplying it also marks
    /// the group invalid, as every other field in this port does.
    pub fn error_message(mut self, text: impl Into<SharedString>) -> Self {
        self.error_message = Some(text.into());
        self
    }

    /// `name` — the name this control submits under.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
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
    /// # use herogpui_components::{Form, RadioGroup, RadioOption};
    /// # struct Demo;
    /// # impl Render for Demo {
    /// #     fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    /// #         let form = Form::new();
    /// #         let control = RadioGroup::new("plan", vec![RadioOption::new("Pro")]).name("plan");
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
        let selected = if self.is_controlled {
            self.selected
        } else {
            self.default_value
        };
        {
            let mut state = self.form_state.borrow_mut();
            state.value = crate::form::FormValue::Text(
                selected
                    .and_then(|index| self.options.get(index))
                    .map(|option| option.value.clone())
                    .unwrap_or_default(),
            );
            state.is_invalid = self.is_invalid
                || self.error_message.is_some()
                || self
                    .options
                    .iter()
                    .any(|option| option.error_message.is_some());
            state.is_successful = !self.is_disabled;
        }
        Some(
            crate::form::FormField::live(name, self.form_state.clone())
                .is_required(self.is_required),
        )
    }

    /// `value` — the selected option's value. Supplying it makes the group controlled.
    pub fn value(mut self, value: impl AsRef<str>) -> Self {
        self.selected = self
            .options
            .iter()
            .position(|option| option.value == value.as_ref());
        self.form_state.borrow_mut().value = crate::form::FormValue::Text(
            self.selected
                .and_then(|index| self.options.get(index))
                .map(|option| option.value.clone())
                .unwrap_or_default(),
        );
        self.is_controlled = true;
        self
    }

    /// `defaultValue` — the uncontrolled initial selection.
    ///
    /// Only consulted when `value` is not supplied; the group then owns the
    /// selection and a press moves it.
    pub fn default_value(mut self, value: impl AsRef<str>) -> Self {
        self.default_value = self
            .options
            .iter()
            .position(|option| option.value == value.as_ref());
        if !self.is_controlled {
            self.form_state.borrow_mut().value = crate::form::FormValue::Text(
                self.default_value
                    .and_then(|index| self.options.get(index))
                    .map(|option| option.value.clone())
                    .unwrap_or_default(),
            );
        }
        self
    }

    /// Sets the layout direction of the options (v3 `orientation`).
    pub fn orientation(mut self, o: Orientation) -> Self {
        self.orientation = o;
        self
    }

    /// Sets the disabled state (v3 `isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets the handler called with the newly selected value (v3 `onChange`).
    pub fn on_change(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(std::sync::Arc::new(f));
        self
    }

    /// Sets the compact step. `Md` is the default and byte-identical to the
    /// pinned control; `Sm` is a 14px control with a 5px dot, 12px label text
    /// (16px leading) and a 10px row gap. Not a v3 prop.
    pub fn size(mut self, size: RadioSize) -> Self {
        self.size = size;
        self
    }

    /// The option labels' font size, in place of the size step's. A 12/14/16px size
    /// takes v3's leading pair (16/20/24); any other keeps the 20px leading.
    /// The control circle, its dot, the gap and the descriptions keep the size step, so the override changes the text and its line box only. Not a v3 prop: v3 sets it with a class on `Radio.Content`.
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// The radio control's corner radius, in place of the owning `key_radius`
    /// helper: the control circle takes it and its pressed box scales the same
    /// value instead of snapping back to the helper, while the selected dot
    /// inside is an inner part and keeps its own. Not a v3 prop; the removed
    /// v2 `radius` prop is prohibited and this is a per-component repository
    /// extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// `fullWidth` — expands the root to the available width without
    /// redistributing the children.
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the radio group's root element after every value the
    /// orientation and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl RenderOnce for RadioGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (selected, own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "value"),
            self.is_controlled.then_some(self.selected),
            self.default_value,
        );
        let is_invalid = self.is_invalid
            || self.error_message.is_some()
            || self
                .options
                .iter()
                .any(|option| option.error_message.is_some());
        let selected_value = selected
            .and_then(|index| self.options.get(index))
            .map(|option| option.value.clone())
            .unwrap_or_default();
        {
            let mut state = self.form_state.borrow_mut();
            state.value = crate::form::FormValue::Text(selected_value);
            state.is_invalid = is_invalid;
            state.is_successful = !self.is_disabled;
        }

        let reset_own = own.clone();
        let reset_state = Rc::downgrade(&self.form_state);
        let reset_change = self.is_controlled.then(|| self.on_change.clone()).flatten();
        let reset_index = self.default_value;
        let reset_value = reset_index
            .and_then(|index| self.options.get(index))
            .map(|option| option.value.clone())
            .unwrap_or_default();
        self.form_state.borrow_mut().restore = (reset_own.is_some() || reset_change.is_some())
            .then(|| {
                crate::util::shared(move |window: &mut Window, cx: &mut App| {
                    if let Some(state) = reset_state.upgrade() {
                        state.borrow_mut().value =
                            crate::form::FormValue::Text(reset_value.clone());
                    }
                    if let Some(held) = &reset_own {
                        held.update(cx, |selected, cx| {
                            *selected = reset_index;
                            cx.notify();
                        });
                    }
                    if let Some(on_change) = &reset_change {
                        on_change(&reset_value, window, cx);
                    }
                }) as std::sync::Arc<dyn Fn(&mut Window, &mut App)>
            });

        // *One* handle for the whole group, because a radio group is one tab
        // stop. Which row claims it is what moves: a roving tab stop cannot be
        // done by flipping a handle's `tab_stop`, since that is fixed where the
        // handle is made. `use_keyed_state` takes `cx` mutably, so it precedes
        // the theme.
        // Every per-option id hangs off the group id as structure, so a
        // per-option key costs an `Arc` clone rather than a fresh `String`.
        let id_prefix = self.id.clone();
        let group_focus =
            crate::util::tab_stop_handle(element_id::scoped(&id_prefix, "focus"), window, cx);
        self.form_state.borrow_mut().focus = Some(group_focus.clone());

        // A radio group is *one* tab stop: Tab moves past the whole group and
        // the arrows choose within it, which is the ARIA radio-group pattern
        // React Aria implements. The stop is the selected option, or the first
        // when nothing is selected yet.
        //
        // `Radio.isDisabled` options are left out of `stops` -- the list the
        // arrows and Home/End walk -- so the cursor never lands on one. The
        // tab stop skips them too: a stop resting on a disabled option has no
        // row to claim the group's handle (AGENTS.md's roving tab stop), which
        // would take the whole group out of the tab order. So with nothing
        // selected and the first option disabled, the group is still reachable
        // by Tab, on the first *enabled* option. With every option disabled
        // `stops` is empty, no row tracks the handle, and the group leaves the
        // tab order exactly as the group-wide `is_disabled` does.
        // `Arc` because every enabled option's key handler captures the whole
        // list: a plain clone per option was O(n^2) per frame.
        let stops: std::sync::Arc<Vec<usize>> = std::sync::Arc::new(
            (0..self.options.len())
                .filter(|i| !self.options[*i].is_disabled)
                .collect(),
        );
        let initial_focus_index = selected
            .filter(|i| stops.contains(i))
            .or_else(|| stops.first().copied())
            .unwrap_or(0);
        // Actual focus and selection diverge in a read-only group: React
        // Aria's arrow handler focuses the next input first, then its stately
        // setter rejects the value change. Keep that roving cursor separately
        // from `selected`, keyed by the component id so two groups cannot
        // share it.
        let cursor =
            window.use_keyed_state(element_id::scoped(&id_prefix, "cursor"), cx, move |_, _| {
                initial_focus_index
            });
        let held_cursor = *cursor.read(cx);
        let cursor_index = if group_focus.is_focused(window) && stops.contains(&held_cursor) {
            held_cursor
        } else {
            initial_focus_index
        };

        // One hover/press slot per option. The row's press grows the selected
        // indicator from 6px to 8px.
        let interaction: Vec<crate::util::Interaction> = (0..self.options.len())
            .map(|i| {
                crate::util::interaction(
                    element_id::scoped(&element_id::indexed(&id_prefix, "opt", i), "interaction"),
                    window,
                    cx,
                )
            })
            .collect();

        let sem = *cx.role(Color::Accent);
        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        // `.radio__control` is `size-4 rounded-lg` — a rounded square, not a
        // circle — and `.radio__indicator` fills it at `rounded-lg` too.
        // The selected dot is the indicator scaled to `0.4286` of the 16px
        // control, which v3's own comment rounds to 6px. (8px is its *pressed*
        // size, `scale: 0.5714`.)
        let (circle, dot, text, gap) = self.size.metrics();
        // The override reaches the option labels only; the control circle's
        // press box keeps the size step's metrics below.
        let label_text = self.text_size.unwrap_or(text);

        // `.radio-group` spaces its options with `mt-4` when vertical and
        // `gap-4` when horizontal — 16px either way.
        let mut group = match self.orientation {
            Orientation::Horizontal => gpui::div().flex().items_center().flex_wrap().gap(px(16.)),
            Orientation::Vertical => gpui::div().flex().flex_col().gap(px(16.)),
        };
        // `.radio-group--secondary` is not a panel: it only repaints the
        // *control* with `--default` and drops its shadow. This used to wrap the
        // whole group in a padded `surface_secondary` card, which v3 has no rule
        // for.
        let control_bg = match self.variant {
            FieldVariant::Primary => colors.field.background,
            FieldVariant::Secondary => colors.default.color,
        };
        let control_shadow = (self.variant == FieldVariant::Primary
            && !layout.field_shadow.is_empty())
        .then(|| layout.field_shadow.clone());

        // The control circle's radius, resolved once: the pressed box below
        // scales the same value instead of snapping back to the helper.
        let control_radius = self.radius.unwrap_or_else(|| crate::util::key_radius(cx));

        let option_values: std::sync::Arc<Vec<SharedString>> = std::sync::Arc::new(
            self.options
                .iter()
                .map(|option| option.value.clone())
                .collect(),
        );
        for (i, option) in self.options.into_iter().enumerate() {
            let label = option.label;
            let value = option.value;
            let description = option
                .description
                .or_else(|| self.descriptions.get(i).and_then(|text| text.clone()));
            let error_message = option.error_message;
            let is_selected = selected == Some(i);
            let option_invalid =
                self.is_invalid || self.error_message.is_some() || error_message.is_some();
            // `Radio.isDisabled` — the option's own switch, beside the
            // group-wide `is_disabled`: dimmed (`status-disabled`'s opacity,
            // v3's "reduced opacity, no pointer events"), no pointer
            // affordance, no click handler and no place in the tab order or
            // the arrow navigation.
            let row_disabled = self.is_disabled || option.is_disabled;
            let (is_hovered, is_pressed) = interaction
                .get(i)
                .map(|slot| *slot.read(cx))
                .unwrap_or_default();
            let option_state = RadioOptionState {
                is_selected,
                is_disabled: row_disabled,
                is_read_only: self.is_read_only,
                is_invalid: option_invalid,
                is_required: self.is_required,
            };
            // `.radio__control` uses the field border width and shadow from the
            // active theme. Unselected hover changes its fill; selected hover
            // keeps `bg-accent` and only changes the border, matching
            // `radio.css` where `bg-accent-hover` is reserved for press.
            let hover_bg = match self.variant {
                FieldVariant::Primary => colors.field.hover(),
                FieldVariant::Secondary => colors.default.hover(),
            };
            let mut circle_el = gpui::div()
                .id(element_id::scoped(
                    &element_id::indexed(&id_prefix, "opt", i),
                    "control",
                ))
                .flex()
                .items_center()
                .justify_center()
                .size(circle)
                .rounded(control_radius)
                .flex_shrink_0()
                .border(layout.field_border_width)
                // HeroUI removes the field border from the selected control;
                // the accent fill owns that edge. An enabled custom theme can
                // expose a nonzero field border, so keep the selected state
                // transparent instead of letting the base border show through.
                .border_color(if is_selected {
                    gpui::transparent_black()
                } else {
                    colors.field.border
                })
                .bg(if is_selected { sem.color } else { control_bg })
                .when(is_hovered && !row_disabled, |el| {
                    el.border_color(colors.field.border_hover())
                        .when(!is_selected, |el| el.bg(hover_bg))
                })
                .when_some(control_shadow.clone(), |el, shadows| el.shadow(shadows));

            if let Some(render) = &self.indicator {
                circle_el = circle_el.child(render(&label, option_state));
            } else if is_selected {
                circle_el = circle_el.child(
                    gpui::div()
                        .size(if is_pressed { px(8.) } else { dot })
                        .rounded(crate::util::key_radius(cx))
                        .bg(sem.foreground),
                );
            }
            // `status-invalid-field` is a 1px danger outline over whatever the
            // control already paints — it does not replace the fill, and v3
            // applies it whether or not the option is selected.
            if option_invalid {
                circle_el = circle_el.border_1().border_color(colors.danger.color);
            }

            // v3 focuses the radio and rings `.radio__control`: the row takes the
            // focus, the control shows it.
            let focused = i == cursor_index
                && group_focus.is_focused(window)
                && crate::util::focus_visible(cx);
            // The ring is an overlay child on the control itself, not on the
            // press skin `anim::pressed_with_background` builds below: the
            // control is the element that carries `control_radius` and the one
            // v3 rings, and the press refinement lands on that same element, so
            // the overlay scales with it. Concentric by construction, where the
            // spread shadow repeated the control's radius two pixels out.
            let circle_el = crate::util::with_focus_ring_overlay(
                circle_el,
                focused && !row_disabled,
                true,
                control_radius,
                control_shadow.clone().unwrap_or_default(),
                cx,
            );

            // `.radio__control[data-pressed]` is `scale-95`, and a checked one
            // also fills with `bg-accent-hover`. A disabled option cannot be
            // pressed, so it skips the animation like a read-only one.
            let circle_el = if row_disabled || self.is_read_only {
                circle_el
            } else {
                // The pressed fill rides inside the press refinement, which
                // owns the scale; a chained `.active` would replace it.
                let pressed_fill = sem.hover();
                if is_selected {
                    crate::anim::pressed_with_background(
                        circle_el,
                        crate::anim::PressBox {
                            height: circle,
                            padding_x: None,
                            width: Some(circle),
                            min_width: None,
                            text_size: text,
                            line_height: text,
                            gap: px(0.),
                            radius: control_radius,
                            shrink_x: true,
                            scale: crate::anim::PRESSED_SCALE_DEEP,
                        },
                        pressed_fill,
                        cx,
                    )
                } else {
                    crate::anim::pressed(
                        circle_el,
                        crate::anim::PressBox {
                            height: circle,
                            padding_x: None,
                            width: Some(circle),
                            min_width: None,
                            text_size: text,
                            line_height: text,
                            gap: px(0.),
                            radius: control_radius,
                            shrink_x: true,
                            scale: crate::anim::PRESSED_SCALE_DEEP,
                        },
                        cx,
                    )
                }
            };

            // Each option is a native `<input type="radio">` upstream, so its
            // role is `radio` and `aria-checked` follows the selection.
            let option_name = a11y::Name::field(
                Some(&label),
                description.as_ref(),
                &crate::validation::resolve(option_invalid, &[], None, error_message.clone()),
            );
            let mut row = gpui::div()
                .id(element_id::indexed(&id_prefix, "opt", i))
                .a11y_named(a11y::Role::RadioButton, &option_name)
                .a11y_checked(is_selected, false)
                .when(!row_disabled && i == cursor_index, |r| {
                    r.track_focus(&group_focus)
                })
                .flex()
                .items_center()
                .gap(gap)
                .text_size(label_text)
                .line_height(crate::util::leading_for(label_text).unwrap_or(px(20.)))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(colors.foreground)
                .when(!row_disabled && !self.is_read_only, |r| {
                    r.cursor(crate::util::interactive_cursor(cx))
                })
                .when(row_disabled, |r| r.opacity(layout.disabled_opacity))
                .child(circle_el)
                .child(match &self.option_content {
                    Some(render) => render(&label, option_state),
                    None => label.into_any_element(),
                });
            if !row_disabled && !self.is_read_only {
                if let Some(slot) = interaction.get(i) {
                    row = crate::util::track_interaction(row, slot);
                }
            }

            if !row_disabled {
                let on_change = self.on_change.clone();
                let own = own.clone();
                let read_only = self.is_read_only;
                // The arrows always take focus with them. In a mutable group
                // they also select; read-only keeps the cursor movement and
                // rejects only that second step, matching the pinned hooks.
                let key_change = on_change.clone();
                let key_own = own.clone();
                let key_stops = stops.clone();
                let key_cursor = cursor.clone();
                let key_values = option_values.clone();
                let key_form_state = self.form_state.clone();
                row = row.on_key_down(move |event, window, cx| {
                    let key = match event.keystroke.key.as_str() {
                        "down" | "right" => "down",
                        "up" | "left" => "up",
                        _ => return,
                    };
                    // `useRadioGroup` owns all four arrows, but has no Home or
                    // End shortcut. Leave those and every other key available
                    // to the enclosing surface.
                    cx.stop_propagation();
                    let crate::list_nav::Move::To(next) =
                        crate::list_nav::resolve(&key_stops, Some(i), key, true)
                    else {
                        return;
                    };
                    key_cursor.update(cx, |v, cx| {
                        *v = next;
                        cx.notify();
                    });
                    if !read_only {
                        if let Some(held) = &key_own {
                            key_form_state.borrow_mut().value =
                                crate::form::FormValue::Text(key_values[next].clone());
                            held.update(cx, |v, cx| {
                                *v = Some(next);
                                cx.notify();
                            });
                        }
                        if let Some(f) = &key_change {
                            f(&key_values[next], window, cx);
                        }
                    }
                });
                let click_cursor = cursor.clone();
                let click_focus = group_focus.clone();
                let click_form_state = self.form_state.clone();
                row = row.on_click(move |_, window, cx| {
                    window.focus(&click_focus, cx);
                    click_cursor.update(cx, |v, cx| {
                        *v = i;
                        cx.notify();
                    });
                    if !read_only {
                        // Uncontrolled: move our own selection, or pressing a
                        // radio would do nothing.
                        if let Some(held) = &own {
                            click_form_state.borrow_mut().value =
                                crate::form::FormValue::Text(value.clone());
                            held.update(cx, |v, cx| {
                                *v = Some(i);
                                cx.notify();
                            });
                        }
                        if let Some(f) = &on_change {
                            f(&value, window, cx);
                        }
                    }
                });
            }

            if !row_disabled && i == cursor_index {
                row = crate::util::record_focus_bounds(row, &group_focus, window, cx);
            }
            // `.radio` is `flex flex-col gap-1` around its content and the
            // description, which `ps-7` indents under the label -- the control
            // plus the content gap.
            match (error_message, description) {
                (Some(message), _) => {
                    group = group.child(
                        gpui::div().flex().flex_col().gap(px(4.)).child(row).child(
                            gpui::div()
                                .pl(circle + gap)
                                .child(crate::field::ErrorMessage::new(message)),
                        ),
                    );
                }
                (None, Some(text)) => {
                    group = group.child(
                        gpui::div().flex().flex_col().gap(px(4.)).child(row).child(
                            gpui::div()
                                .pl(circle + gap)
                                .child(crate::field::Description::new(text)),
                        ),
                    );
                }
                (None, None) => group = group.child(row),
            }
        }

        // `.radio` is `flex flex-col gap-1`, and the group's own label,
        // description and error are its siblings. v3 marks `isRequired` on the
        // Label rather than adding a line of its own, which is what
        // `field::Label` draws.
        // `useRadioGroup` is `role="radiogroup"` with `aria-orientation`
        // always present (it defaults to `vertical`), named and described
        // through `useField`.
        let group_name = a11y::Name::field(
            self.label.as_ref(),
            self.description.as_ref(),
            &crate::validation::resolve(is_invalid, &[], None, self.error_message.clone()),
        );
        let mut root = gpui::div()
            .id(self.id.clone())
            .a11y_named(a11y::Role::RadioGroup, &group_name)
            .a11y_orientation(self.orientation)
            .flex()
            .flex_col()
            .gap(px(4.));
        if self.full_width {
            root = root.w_full();
        }
        if let Some(label) = &self.label {
            root = root.child(
                crate::field::Label::new(label.clone())
                    .is_required(self.is_required)
                    .is_disabled(self.is_disabled)
                    .is_invalid(is_invalid),
            );
        }
        // v3's order, from its own examples: the group's `<Description>` sits
        // between the label and the options, and its `<FieldError>` after them.
        // (A *field*'s description is replaced by its error; `radio-group.css`
        // has no rule hiding this one, so both can show.)
        if let Some(description) = &self.description {
            root = root.child(crate::field::Description::new(description.clone()));
        }
        root = root.child(group);
        let error = is_invalid.then(|| self.error_message.clone()).flatten();
        if let Some(error) = crate::anim::field_error_panel(&self.id, error, window, cx) {
            root = root.child(error);
        }
        root = crate::util::apply_sx(root, &self.sx);
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Render wraps the walk list and the value list in `Arc` so every enabled
    /// option's key handler clones the pointer, not the Vec.
    #[test]
    fn option_key_handlers_share_walk_and_value_lists() {
        let source = include_str!("radio_group.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("let stops: std::sync::Arc<Vec<usize>> = std::sync::Arc::new("),
            "the walk list must be one Arc shared by every enabled option"
        );
        assert!(
            source.contains(
                "let option_values: std::sync::Arc<Vec<SharedString>> = std::sync::Arc::new("
            ),
            "the value list must be one Arc shared by every enabled option"
        );
        assert!(
            source.contains("let key_stops = stops.clone();"),
            "each enabled option must clone the shared walk list"
        );
        assert!(
            source.contains("let key_values = option_values.clone();"),
            "each enabled option must clone the shared value list"
        );

        // The clones above are `Arc::clone`: two handles to one allocation.
        let stops: std::sync::Arc<Vec<usize>> = std::sync::Arc::new(vec![0, 1, 2]);
        let values: std::sync::Arc<Vec<SharedString>> =
            std::sync::Arc::new(vec![SharedString::from("v0")]);
        let walk = std::sync::Arc::clone(&stops);
        let list = std::sync::Arc::clone(&values);
        assert!(
            std::sync::Arc::ptr_eq(&walk, &stops) && std::sync::Arc::ptr_eq(&list, &values),
            "Arc clones of the walk and value lists must share pointer identity"
        );
    }

    #[test]
    fn hover_background_respects_selection_and_variant() {
        let source = include_str!("radio_group.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(source.contains("let hover_bg = match self.variant"));
        assert!(source.contains("FieldVariant::Primary => colors.field.hover()"));
        assert!(source.contains("FieldVariant::Secondary => colors.default.hover()"));
        assert!(source.contains(".when(is_hovered && !row_disabled"));
        assert!(source.contains(".when(!is_selected, |el| el.bg(hover_bg))"));
        assert!(source.contains("colors.field.border_hover()"));
        assert!(source.contains("if is_selected {\n                    gpui::transparent_black()"));
    }
}

#[cfg(test)]
mod radio_size_tests {
    use super::*;

    #[test]
    fn md_is_the_pinned_geometry_and_sm_scales_together() {
        assert_eq!(RadioSize::default(), RadioSize::Md);
        assert_eq!(RadioSize::Md.metrics(), (px(16.), px(6.), px(14.), px(12.)));
        assert_eq!(RadioSize::Sm.metrics(), (px(14.), px(5.), px(12.), px(10.)));
        assert_eq!(
            crate::util::leading_for(RadioSize::Sm.metrics().2),
            Some(px(16.))
        );
    }
}

crate::util::impl_component_styled!(RadioGroup);
