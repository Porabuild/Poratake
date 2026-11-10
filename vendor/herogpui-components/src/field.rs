//! Form field primitives — port of `@heroui/label`, `@heroui/description`,
//! `@heroui/error-message`, `@heroui/field-error` and `@heroui/fieldset`.
//!
//! These are the composition-friendly slots that HeroUI v3 field components
//! assemble internally, exposed here so applications can build custom fields
//! with the same typography and states.
//!
//! `FieldsetRoot` in v3.2.4 also accepts the native `disabled` attribute and
//! forwards it to descendants through React Aria contexts. GPUI has no
//! ancestor context propagation and no DOM attribute graph, so this port
//! deliberately ships no disabled state on [`Fieldset`]: a field composed
//! inside one keeps every interaction
//! (`text_fields::fieldset_disabled_disables_its_children` proves it).

use gpui::{
    div, px, AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
};

use crate::a11y::{self, A11y as _};
use herogpui_theme::ActiveTheme;

/// HeroUI Label — `slot="label"`.
///
/// Mirrors the React API: `isRequired`, `isDisabled`, `isInvalid`. The
/// `htmlFor` prop has no gpui analogue (there is no DOM id graph), so labels
/// are associated by composition instead.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Label {
    text: SharedString,
    is_required: bool,
    is_disabled: bool,
    is_invalid: bool,
    /// `htmlFor` — the field this label names.
    label_for: Option<(ElementId, gpui::FocusHandle)>,
}

impl Label {
    /// Creates a label with the given text.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            is_required: false,
            is_disabled: false,
            is_invalid: false,
            label_for: None,
        }
    }

    /// `htmlFor` — associates the label with a field.
    ///
    /// In HTML this is an id reference; here it is the field's focus handle,
    /// which is what makes the association do the one thing it does visibly:
    /// clicking the label focuses the field. Pass a distinct `id` per label so
    /// the click target has one.
    pub fn label_for(mut self, id: impl Into<ElementId>, handle: gpui::FocusHandle) -> Self {
        self.label_for = Some((id.into(), handle));
        self
    }

    /// Sets the required state (v3 `isRequired`).
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// Sets the disabled state (v3 `isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets the invalid state (v3 `isInvalid`).
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }
}

impl RenderOnce for Label {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.colors();
        let mut el = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .text_size(px(14.))
            .line_height(px(20.))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(if self.is_invalid {
                colors.danger.color
            } else {
                colors.foreground
            })
            .child(self.text.to_string());

        if self.is_required {
            el = el.child(div().text_color(colors.danger.color).child("*".to_owned()));
        }

        if self.is_disabled {
            el = el.opacity(cx.layout().disabled_opacity);
        }

        // `htmlFor`: clicking the label focuses the field it names.
        match self.label_for {
            Some((id, handle)) if !self.is_disabled => el
                .id(id)
                .cursor(crate::util::interactive_cursor(cx))
                .on_click(move |_: &gpui::ClickEvent, window: &mut Window, cx| {
                    window.focus(&handle, cx);
                })
                .into_any_element(),
            _ => el.into_any_element(),
        }
    }
}

/// HeroUI Description — `slot="description"`. De-emphasised helper copy shown
/// beneath a field.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Description {
    text: SharedString,
}

impl Description {
    /// Creates a description with the given text.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for Description {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(cx.colors().muted)
            .child(self.text.to_string())
    }
}

/// HeroUI ErrorMessage — `slot="errorMessage"`. Always rendered when present.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ErrorMessage {
    text: SharedString,
}

impl ErrorMessage {
    /// Creates an error message with the given text.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for ErrorMessage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(cx.colors().danger.color)
            .child(self.text.to_string())
    }
}

/// HeroUI FieldError — validation-driven error text.
///
/// Unlike [`ErrorMessage`], a `FieldError` manages its own visibility from the
/// validation state: it renders nothing unless the field is invalid and a
/// message is present.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct FieldError {
    text: Option<SharedString>,
    is_invalid: bool,
    /// Optional stable identity used to retain and animate the row across
    /// validation updates. Without an id the standalone slot keeps its
    /// allocation-free static behavior for one-shot compositions.
    id: Option<ElementId>,
}

impl FieldError {
    /// Creates an empty field error.
    pub fn new() -> Self {
        Self {
            text: None,
            is_invalid: false,
            id: None,
        }
    }

    /// The validation message. Setting a message also marks the field invalid,
    /// matching React Aria's `ValidationResult` behaviour.
    pub fn message(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self.is_invalid = true;
        self
    }

    /// Overrides visibility independently of the message.
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// Supplies the stable element identity needed for the v3 height/opacity
    /// transition when a standalone FieldError is toggled across renders.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }
}

impl Default for FieldError {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for FieldError {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let message = self.text.clone().filter(|_| self.is_invalid);
        if let Some(id) = self.id {
            return crate::anim::field_error_panel(&id, message, window, cx)
                .unwrap_or_else(|| div().into_any_element());
        }
        match (self.is_invalid, self.text) {
            // `.field-error` is `px-1`, which `.error-message` is not.
            (true, Some(text)) => div()
                .px(px(4.))
                .child(ErrorMessage::new(text))
                .into_any_element(),
            _ => div().into_any_element(),
        }
    }
}

/// HeroUI Fieldset — groups related form controls under a legend.
///
/// Compose with [`FieldsetLegend`], [`FieldGroup`] and [`FieldsetActions`],
/// mirroring `Fieldset.Legend` / `.Group` / `.Actions` in React.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Fieldset {
    gap: Pixels,
    children: Vec<AnyElement>,
    id: Option<ElementId>,
}

impl Fieldset {
    /// Creates an empty fieldset.
    pub fn new() -> Self {
        Self {
            gap: px(24.),
            children: Vec::new(),
            id: None,
        }
    }

    /// Names this fieldset so it can report `role="group"`. Unnamed fieldsets
    /// produce no AccessKit node.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the gap between children.
    pub fn gap(mut self, gap: impl Into<Pixels>) -> Self {
        self.gap = gap.into();
        self
    }
}

impl Default for Fieldset {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Fieldset {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Fieldset {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let el = div()
            .flex()
            .flex_col()
            .gap(self.gap)
            // `.fieldset` also carries `shrink grow basis-0` so it fills a flex
            // parent like any other section.
            .flex_shrink(1.)
            .flex_grow(1.)
            .flex_basis(px(0.))
            .text_color(cx.colors().foreground)
            .children(self.children);
        match self.id {
            Some(id) => el.id(id).a11y(a11y::Role::Group).into_any_element(),
            None => el.into_any_element(),
        }
    }
}

/// `Fieldset.Legend` — the group's caption.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct FieldsetLegend {
    text: SharedString,
}

impl FieldsetLegend {
    /// Creates a legend with the given text.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for FieldsetLegend {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .text_size(px(16.))
            .line_height(px(24.))
            // `.fieldset__legend` is `text-base font-medium`, not semibold.
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(cx.colors().foreground)
            .child(self.text.to_string())
    }
}

/// `Fieldset.Group` — layout wrapper for the grouped controls.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct FieldGroup {
    gap: Pixels,
    children: Vec<AnyElement>,
}

impl FieldGroup {
    /// Creates an empty field group.
    pub fn new() -> Self {
        Self {
            gap: px(16.),
            children: Vec::new(),
        }
    }

    /// Sets the gap between children.
    pub fn gap(mut self, gap: impl Into<Pixels>) -> Self {
        self.gap = gap.into();
        self
    }
}

impl Default for FieldGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for FieldGroup {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// `.fieldset__field-group` is `w-full space-y-4`: the stack of fields, 16px
/// apart (`--spacing` × 4).
impl RenderOnce for FieldGroup {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(self.gap)
            .children(self.children)
    }
}

/// `Fieldset.Actions` — trailing row for submit/cancel controls.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct FieldsetActions {
    gap: Pixels,
    children: Vec<AnyElement>,
}

impl FieldsetActions {
    /// Creates an empty actions row.
    pub fn new() -> Self {
        Self {
            gap: px(8.),
            children: Vec::new(),
        }
    }

    /// Sets the gap between children.
    pub fn gap(mut self, gap: impl Into<Pixels>) -> Self {
        self.gap = gap.into();
        self
    }
}

impl Default for FieldsetActions {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for FieldsetActions {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for FieldsetActions {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            // `.fieldset__actions` is `gap-2 pt-1`.
            .pt(px(4.))
            .gap(self.gap)
            .children(self.children)
    }
}
