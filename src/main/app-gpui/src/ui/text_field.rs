//! An editable single-line text field. The caret and selection are laid out
//! rather than measured: the value is split into before/selected/after spans
//! so the layout engine positions the caret exactly where the glyphs end.

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, AnimationExt, App, Context, FocusHandle, KeyDownEvent, Render,
    SharedString, Styled, Window,
};

use crate::theme::vars::active_theme;
use crate::ui::chrome;

pub type ChangeHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// The `size-4` leading glyph the search fields carry.
const ICON: f32 = 16.0;

pub struct TextField {
    value: String,
    placeholder: SharedString,
    cursor: usize,
    anchor: usize,
    secret: bool,
    full_width: bool,
    /// A leading glyph inside the field, e.g. the search magnifier.
    leading_icon: Option<&'static str>,
    /// Renders the field as a bare input: no surface, padding, radius or focus
    /// ring, so a caller can supply its own shell. The settings sidebar search
    /// is a styled `<label>` wrapping a `bg-transparent outline-none` input.
    bare: bool,
    /// `h-8` on the shortcut search, versus the default `min-h-9`.
    height: Option<gpui::Pixels>,
    /// `px-2.5` on the shortcut search, versus the default `px-3`.
    pad_x: Option<gpui::Pixels>,
    on_change: Option<ChangeHandler>,
    on_submit: Option<ChangeHandler>,
    on_cancel: Option<ChangeHandler>,
    focus_handle: FocusHandle,
}

impl TextField {
    pub fn new(value: impl Into<String>, cx: &mut Context<Self>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self {
            value,
            placeholder: SharedString::default(),
            cursor,
            anchor: cursor,
            secret: false,
            full_width: false,
            leading_icon: None,
            bare: false,
            height: None,
            pad_x: None,
            on_change: None,
            on_submit: None,
            on_cancel: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn secret(mut self, secret: bool) -> Self {
        self.secret = secret;
        self
    }

    pub fn full_width(mut self, full_width: bool) -> Self {
        self.full_width = full_width;
        self
    }

    pub fn leading_icon(mut self, icon: &'static str) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    /// See [`TextField::bare`].
    pub fn bare(mut self) -> Self {
        self.bare = true;
        self
    }

    pub fn height(mut self, height: gpui::Pixels) -> Self {
        self.height = Some(height);
        self
    }

    pub fn pad_x(mut self, pad: gpui::Pixels) -> Self {
        self.pad_x = Some(pad);
        self
    }

    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    pub fn on_submit(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_submit = Some(Rc::new(handler));
        self
    }

    pub fn on_cancel(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    /// Replaces the value when the owner's state changed underneath, keeping
    /// the caret inside the new text.
    pub fn set_value(&mut self, value: &str, cx: &mut Context<Self>) {
        if self.value == value {
            return;
        }
        self.value = value.to_string();
        self.cursor = clamp_boundary(&self.value, self.cursor);
        self.anchor = self.cursor;
        cx.notify();
    }

    fn selection(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    fn emit(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handler) = &self.on_change {
            handler(&self.value, window, cx);
        }
    }

    fn replace_selection(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (start, end) = self.selection();
        self.value.replace_range(start..end, text);
        self.cursor = start + text.len();
        self.anchor = self.cursor;
        self.emit(window, cx);
        cx.notify();
    }

    fn delete_backward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            self.replace_selection("", window, cx);
            return;
        }
        let Some(previous) = previous_boundary(&self.value, self.cursor) else {
            return;
        };
        self.value.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.anchor = previous;
        self.emit(window, cx);
        cx.notify();
    }

    fn delete_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            self.replace_selection("", window, cx);
            return;
        }
        let Some(next) = next_boundary(&self.value, self.cursor) else {
            return;
        };
        self.value.replace_range(self.cursor..next, "");
        self.emit(window, cx);
        cx.notify();
    }

    fn move_cursor(&mut self, to: usize, extend: bool, cx: &mut Context<Self>) {
        self.cursor = to.min(self.value.len());
        if !extend {
            self.anchor = self.cursor;
        }
        cx.notify();
    }

    fn copy_selection(&self) {
        let (start, end) = self.selection();
        if start == end || self.secret {
            return;
        }
        let _ = arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(self.value[start..end].to_string()));
    }

    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(text) = arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text())
        else {
            return;
        };
        let single_line: String = text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
        self.replace_selection(&single_line, window, cx);
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        let control = modifiers.control || modifiers.platform;
        let key = event.keystroke.key.as_str();
        if key != "tab" {
            cx.stop_propagation();
        }

        match key {
            "backspace" => return self.delete_backward(window, cx),
            "delete" => return self.delete_forward(window, cx),
            "left" => {
                let to = previous_boundary(&self.value, self.cursor).unwrap_or(0);
                return self.move_cursor(to, modifiers.shift, cx);
            }
            "right" => {
                let to = next_boundary(&self.value, self.cursor).unwrap_or(self.value.len());
                return self.move_cursor(to, modifiers.shift, cx);
            }
            "home" => return self.move_cursor(0, modifiers.shift, cx),
            "end" => return self.move_cursor(self.value.len(), modifiers.shift, cx),
            "enter" => {
                if let Some(handler) = &self.on_submit {
                    let handler = handler.clone();
                    handler(&self.value, window, cx);
                }
                return;
            }
            "escape" => {
                if let Some(handler) = &self.on_cancel {
                    let handler = handler.clone();
                    handler(&self.value, window, cx);
                }
                return;
            }
            _ => {}
        }

        if control {
            match key {
                "a" => {
                    self.anchor = 0;
                    self.cursor = self.value.len();
                    cx.notify();
                }
                "c" => self.copy_selection(),
                "x" => {
                    self.copy_selection();
                    self.replace_selection("", window, cx);
                }
                "v" => self.paste(window, cx),
                _ => {}
            }
            return;
        }

        let Some(text) = event.keystroke.key_char.as_ref() else {
            return;
        };
        if text.chars().any(|c| c.is_control()) {
            return;
        }
        self.replace_selection(text, window, cx);
    }
}

const SECRET_MASK: char = '\u{2022}';

fn clamp_boundary(value: &str, mut index: usize) -> usize {
    index = index.min(value.len());
    while index > 0 && !value.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn display_offset(value: &str, secret: bool, byte_index: usize) -> usize {
    let byte_index = clamp_boundary(value, byte_index);
    if secret {
        SECRET_MASK.len_utf8() * value[..byte_index].chars().count()
    } else {
        byte_index
    }
}

fn previous_boundary(value: &str, cursor: usize) -> Option<usize> {
    if cursor == 0 {
        return None;
    }
    Some(
        value[..cursor]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0),
    )
}

fn next_boundary(value: &str, cursor: usize) -> Option<usize> {
    if cursor >= value.len() {
        return None;
    }
    let mut chars = value[cursor..].char_indices();
    chars.next();
    Some(
        cursor
            + chars
                .next()
                .map(|(index, _)| index)
                .unwrap_or(value.len() - cursor),
    )
}

fn display(value: &str, secret: bool) -> String {
    if secret {
        SECRET_MASK.to_string().repeat(value.chars().count())
    } else {
        value.to_string()
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let focused = self.focus_handle.is_focused(window);
        let (start, end) = self.selection();
        let text = display(&self.value, self.secret);
        let start = display_offset(&self.value, self.secret, start);
        let end = display_offset(&self.value, self.secret, end);
        let cursor = display_offset(&self.value, self.secret, self.cursor);

        let caret = || {
            div()
                .w(px(1.0))
                .h(px(16.0))
                .bg(theme.foreground)
                .flex_shrink_0()
        };

        let mut content = div()
            .flex()
            .flex_row()
            .items_center()
            .flex_1()
            .min_w_0()
            .overflow_hidden();

        if text.is_empty() {
            // A focused-but-empty field still shows its placeholder in the
            // DOM; only the caret is added.
            if focused {
                content = content.child(caret());
            }
            content = content.child(
                div()
                    .text_color(theme.field_placeholder)
                    .child(self.placeholder.clone()),
            );
        } else {
            content = content.child(div().child(text[..start].to_string()));
            if focused && cursor == start {
                content = content.child(caret());
            }
            if start != end {
                content = content.child(div().bg(theme.ring).child(text[start..end].to_string()));
            }
            if focused && cursor == end && start != end {
                content = content.child(caret());
            }
            content = content.child(div().child(text[end..].to_string()));
        }

        let mut field = div()
            .id("text-field")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _event, window, cx| {
                    window.focus(&this.focus_handle);
                    cx.notify();
                }),
            )
            .flex()
            .items_center()
            .gap(px(8.0))
            .text_size(px(chrome::FIELD_TEXT))
            .text_color(theme.field_foreground)
            .when(self.full_width, |el| el.w_full());

        if !self.bare {
            field = field
                .rounded(px(chrome::FIELD_RADIUS))
                .px(self.pad_x.unwrap_or(px(chrome::FIELD_PAD_X)))
                .py(px(chrome::FIELD_PAD_Y))
                .min_h(self.height.unwrap_or(px(chrome::FIELD_MIN_HEIGHT)))
                .bg(theme.field_background)
                // `.input:focus { status-focused-field }` — a flush 2px accent
                // ring, not a border, so focusing never shifts the layout.
                .when(focused, |el| {
                    el.shadow(crate::ui::primitives::focus_ring(&theme, 0.0))
                });
        }

        if let Some(icon) = self.leading_icon {
            field = field.child(
                div()
                    .flex_shrink_0()
                    .child(crate::ui::icon::icon_element(icon, px(ICON))),
            );
        }

        if self.bare || focused {
            return field.child(content).into_any_element();
        }

        // `.input:hover:not(:focus) { bg-field-hover }` over
        // `background-color 150ms var(--ease-smooth)`.
        let (hover, hovered, _) = crate::ui::primitives::hover_fade("text-field", window, cx);
        let (from, to) = hover
            .read(cx)
            .range(theme.field_background, theme.field_hover);
        field
            .child(content)
            .on_hover(cx.listener(move |_, over: &bool, _window, cx| {
                crate::ui::primitives::track_hover(&hover, *over, cx);
            }))
            .with_animation(
                gpui::ElementId::Name(format!("text-field-hover-{hovered}").into()),
                gpui::Animation::new(std::time::Duration::from_millis(chrome::FIELD_HOVER_MS))
                    .with_easing(crate::ui::primitives::ease_smooth()),
                move |field, delta| field.bg(crate::theme::color::lerp_srgb(from, to, delta)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_character_boundaries_in_both_directions() {
        let value = "a\u{00e9}b";
        assert_eq!(previous_boundary(value, 0), None);
        assert_eq!(previous_boundary(value, 1), Some(0));
        assert_eq!(previous_boundary(value, 3), Some(1));
        assert_eq!(next_boundary(value, 0), Some(1));
        assert_eq!(next_boundary(value, 1), Some(3));
        assert_eq!(next_boundary(value, value.len()), None);
    }

    #[test]
    fn masks_secret_values_by_character_count() {
        assert_eq!(
            display("secret", true),
            "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"
        );
        assert_eq!(display("secret", false), "secret");
    }

    #[test]
    fn secret_offsets_land_on_display_char_boundaries() {
        let value = "é";
        let text = display(value, true);
        let start = display_offset(value, true, value.len());
        assert!(text.is_char_boundary(start));
        assert_eq!(&text[..start], "\u{2022}");
        let empty = display_offset(value, true, 1);
        assert!(text.is_char_boundary(empty));
    }

    #[test]
    fn clamp_boundary_backs_up_to_a_char_start() {
        let value = "é";
        assert_eq!(clamp_boundary(value, 1), 0);
        assert_eq!(clamp_boundary(value, 2), 2);
        assert_eq!(clamp_boundary(value, 99), 2);
    }
}
