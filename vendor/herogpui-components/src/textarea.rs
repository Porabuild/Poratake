//! TextArea — port of `@heroui/text-area` (v3).
//!
//! Reuses [`InputState`] in multi-line mode: gpui wraps text by default
//! (`WhiteSpace::Normal`), so the field lays one wrapping paragraph out per
//! newline, Enter inserts a newline instead of submitting, and the caret is
//! placed inside the line it falls in. `rows` sets the visible height.

use crate::input::{Input, InputState};
use gpui::{prelude::*, px, App, Entity, IntoElement, RenderOnce, SharedString, Styled, Window};

/// Multi-line text field.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct TextArea {
    inner: Input,
    /// `cols`, as a pixel width. `None` leaves the field's natural width.
    min_w: Option<gpui::Pixels>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

/// `rows` as a height: one 20px line each, over `.textarea`'s `py-2`, with
/// HeroUI's 38px minimum floor for even a one-row textarea.
fn rows_height(rows: u32) -> gpui::Pixels {
    px((rows.max(1) as f32 * 20.0 + 16.0).max(38.0))
}

impl TextArea {
    /// `value` — v3's controlled-value spelling, forwarded to the inner field;
    /// see [`crate::input::Input::value`]. A later value is an imperative
    /// update: `state.update(cx, |s, _| s.set_value(..))`.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.inner = self.inner.value(value);
        self
    }

    /// `maxLength` — refuses keystrokes past this many characters.
    pub fn max_length(mut self, n: usize) -> Self {
        self.inner = self.inner.max_length(n);
        self
    }

    /// `minLength` — reported by the inner field's `validity`.
    pub fn min_length(mut self, n: usize) -> Self {
        self.inner = self.inner.min_length(n);
        self
    }

    /// `cols` — visible width, in characters.
    ///
    /// gpui has no `ch` unit, so this is the column count times the size's
    /// character advance, the same approximation [`TextArea::rows`] makes for
    /// height.
    pub fn cols(mut self, cols: u32) -> Self {
        self.min_w = Some(px(cols.max(1) as f32 * 8.0));
        self
    }

    /// `rows` — the visible line count, at one line height per row plus the
    /// field's vertical padding.
    pub fn rows(mut self, rows: u32) -> Self {
        self.inner = self.inner.min_h(rows_height(rows));
        self
    }

    /// A pixel height floor, in place of the `rows`-derived one — the
    /// spelling for v3's `h-*`/`min-h-*` classes on the field box.
    ///
    /// The multi-line surface grows with its content and has no inner
    /// scroller, so this is a floor rather than the fixed `height` v3's
    /// `h-32` declares: an empty or short field shows exactly this height,
    /// and longer content still grows the box instead of clipping.
    pub fn height(mut self, h: impl Into<gpui::Pixels>) -> Self {
        self.inner = self.inner.min_h(h.into());
        self
    }

    /// `name` — see [`crate::input::Input::name`].
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.inner = self.inner.name(name);
        self
    }

    /// `defaultValue` — see [`crate::input::Input::default_value`].
    pub fn default_value(mut self, text: impl Into<SharedString>) -> Self {
        self.inner = self.inner.default_value(text);
        self
    }

    /// `validationBehavior` — see [`crate::input::Input::validation_behavior`].
    pub fn validation_behavior(mut self, behavior: crate::form::ValidationBehavior) -> Self {
        self.inner = self.inner.validation_behavior(behavior);
        self
    }

    /// Sets the field variant (v3 `variant`).
    pub fn variant(mut self, variant: herogpui_core::FieldVariant) -> Self {
        self.inner = self.inner.variant(variant);
        self
    }

    /// Fills the parent's width.
    pub fn full_width(mut self) -> Self {
        self.inner = self.inner.full_width();
        self
    }

    /// The box's horizontal padding — see [`crate::input::Input::padding_x`].
    ///
    /// The multi-line box keeps its own `py-2` and its `rows`-derived height,
    /// so only the horizontal padding is a knob here.
    pub fn padding_x(mut self, p: impl Into<gpui::Pixels>) -> Self {
        self.inner = self.inner.padding_x(p);
        self
    }

    /// Drops the field's chrome — see [`crate::input::Input::is_bare`].
    pub fn is_bare(mut self, v: bool) -> Self {
        self.inner = self.inner.is_bare(v);
        self
    }

    /// Shows or hides only the inner field's visual focus ring — see
    /// [`crate::input::Input::focus_ring`].
    pub fn focus_ring(mut self, v: bool) -> Self {
        self.inner = self.inner.focus_ring(v);
        self
    }

    /// The field text's font family — see
    /// [`crate::input::Input::font_family`].
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.inner = self.inner.font_family(family);
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the wrapper around the field after every value the variant
    /// and the active theme chose, so they win. The field paints its own
    /// chrome, so this reaches the box that chrome sits in, not the chrome.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// Sets the disabled state (v3 `isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.inner = self.inner.is_disabled(v);
        self
    }

    /// Sets the read-only state (v3 `isReadOnly`).
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.inner = self.inner.is_read_only(v);
        self
    }

    /// Sets the required state (v3 `isRequired`).
    pub fn is_required(mut self, v: bool) -> Self {
        self.inner = self.inner.is_required(v);
        self
    }

    /// Sets the invalid state (v3 `isInvalid`).
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.inner = self.inner.is_invalid(v);
        self
    }

    /// `validate` — see [`crate::input::Input::validate`].
    pub fn validate(mut self, f: impl Fn(&str) -> Option<SharedString> + 'static) -> Self {
        self.inner = self.inner.validate(f);
        self
    }

    /// `validationErrors` — see [`crate::input::Input::validation_errors`].
    pub fn validation_errors(
        mut self,
        errors: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.inner = self.inner.validation_errors(errors);
        self
    }

    /// Sets the error message text.
    pub fn error_message(mut self, text: impl Into<SharedString>) -> Self {
        self.inner = self.inner.error_message(text);
        self
    }

    /// Sets the handler called with the new text on change.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.inner = self.inner.on_change(handler);
        self
    }

    /// Builds the multi-line field over `state`, borrowed or owned — see
    /// [`crate::input::Input::new`].
    pub fn new(state: impl std::borrow::Borrow<Entity<InputState>>) -> Self {
        Self {
            // v3 documents `rows` as defaulting to 3.
            inner: Input::new(state).multiline(true).min_h(rows_height(3)),
            min_w: None,
            sx: None,
        }
    }

    /// Sets the label.
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.inner = self.inner.label(l);
        self
    }

    /// `aria-label` — see [`crate::input::Input::aria_label`].
    pub fn aria_label(mut self, l: impl Into<SharedString>) -> Self {
        self.inner = self.inner.aria_label(l);
        self
    }

    /// Sets the placeholder text.
    pub fn placeholder(mut self, p: impl Into<SharedString>) -> Self {
        self.inner = self.inner.placeholder(p);
        self
    }

    /// Sets the description.
    pub fn description(mut self, d: impl Into<SharedString>) -> Self {
        self.inner = self.inner.description(d);
        self
    }
}

impl TextArea {
    /// Hands the inner field to [`crate::input_group::InputGroup::text_area`].
    ///
    /// `InputGroup.TextArea` is the same multi-line field with the group's
    /// chrome instead of its own, so the wrapper this normally renders (which
    /// only carries `cols`) is dropped.
    pub(crate) fn into_group_input(self) -> Input {
        self.inner
    }
}

impl RenderOnce for TextArea {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // The field itself is multi-line; this wrapper only gives it the height
        // `rows` asks for and keeps the text at the top of it. The field paints
        // its own chrome (`util::apply_field_chrome`) -- the wrapper used to
        // repaint a `default.soft()` background at a hardcoded 10px radius,
        // neither of which is a v3 value.
        let el = gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .when_some(self.min_w, |e, w| e.min_w(w))
            .child(self.inner);
        crate::util::apply_sx(el, &self.sx)
    }
}

#[cfg(test)]
mod tests {
    use super::rows_height;
    use gpui::px;

    #[test]
    fn rows_keep_the_pinned_textarea_minimum() {
        assert_eq!(rows_height(0), px(38.));
        assert_eq!(rows_height(1), px(38.));
        assert_eq!(rows_height(3), px(76.));
    }
}

crate::util::impl_component_styled!(TextArea);
