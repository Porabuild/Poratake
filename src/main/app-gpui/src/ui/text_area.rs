//! A multi-line editable text area, the counterpart of `ui::text_field` for
//! the JSON data editors. Lines are laid out as rows and the caret is placed
//! by splitting its line into before/after spans, so no glyph measurement is
//! needed.

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, KeyDownEvent, Render, ScrollHandle,
    SharedString, Styled, Window,
};

use crate::theme::vars::active_theme;

pub type ChangeHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;

pub struct TextArea {
    value: String,
    placeholder: SharedString,
    cursor: usize,
    anchor: usize,
    rows: usize,
    on_change: Option<ChangeHandler>,
    scroll: ScrollHandle,
    focus_handle: FocusHandle,
}

impl TextArea {
    pub fn new(value: impl Into<String>, cx: &mut Context<Self>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self {
            value,
            placeholder: SharedString::default(),
            cursor,
            anchor: cursor,
            rows: 14,
            on_change: None,
            scroll: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// How many lines the area is tall before it scrolls.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    #[allow(dead_code)]
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    #[allow(dead_code)]
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn value(&self) -> &str {
        &self.value
    }

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

    /// The byte offset each line starts at, plus a trailing sentinel so a line
    /// index always has a following bound.
    fn line_starts(&self) -> Vec<usize> {
        let mut starts = vec![0usize];
        for (index, byte) in self.value.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(index + 1);
            }
        }
        starts
    }

    fn line_at(&self, offset: usize) -> (usize, usize) {
        let starts = self.line_starts();
        let line = starts
            .iter()
            .rposition(|start| *start <= offset)
            .unwrap_or(0);
        (line, offset - starts[line])
    }

    fn offset_at(&self, line: usize, column: usize) -> usize {
        let starts = self.line_starts();
        let Some(start) = starts.get(line).copied() else {
            return self.value.len();
        };
        let end = starts
            .get(line + 1)
            .map(|next| next.saturating_sub(1))
            .unwrap_or(self.value.len());
        // A column past the end of the shorter line clamps to its end, which is
        // what a vertical move through ragged lines should do.
        (start + column).min(end)
    }

    fn move_cursor(&mut self, to: usize, extend: bool, cx: &mut Context<Self>) {
        self.cursor = to.min(self.value.len());
        if !extend {
            self.anchor = self.cursor;
        }
        cx.notify();
    }

    fn move_vertically(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let (line, column) = self.line_at(self.cursor);
        let target = line as isize + delta;
        if target < 0 {
            return self.move_cursor(0, extend, cx);
        }
        let starts = self.line_starts();
        if target as usize >= starts.len() {
            return self.move_cursor(self.value.len(), extend, cx);
        }
        let to = self.offset_at(target as usize, column);
        self.move_cursor(to, extend, cx);
    }

    fn delete_backward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            return self.replace_selection("", window, cx);
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
            return self.replace_selection("", window, cx);
        }
        let Some(next) = next_boundary(&self.value, self.cursor) else {
            return;
        };
        self.value.replace_range(self.cursor..next, "");
        self.emit(window, cx);
        cx.notify();
    }

    fn copy_selection(&self) {
        let (start, end) = self.selection();
        if start == end {
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
        // Newlines are kept; carriage returns are not, so a Windows clipboard
        // does not leave stray characters in the JSON.
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        self.replace_selection(&normalized, window, cx);
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let modifiers = event.keystroke.modifiers;
        let control = modifiers.control || modifiers.platform;
        let key = event.keystroke.key.as_str();

        match key {
            "backspace" => return self.delete_backward(window, cx),
            "delete" => return self.delete_forward(window, cx),
            "enter" => return self.replace_selection("\n", window, cx),
            "tab" => return self.replace_selection("  ", window, cx),
            "left" => {
                let to = previous_boundary(&self.value, self.cursor).unwrap_or(0);
                return self.move_cursor(to, modifiers.shift, cx);
            }
            "right" => {
                let to = next_boundary(&self.value, self.cursor).unwrap_or(self.value.len());
                return self.move_cursor(to, modifiers.shift, cx);
            }
            "up" => return self.move_vertically(-1, modifiers.shift, cx),
            "down" => return self.move_vertically(1, modifiers.shift, cx),
            "home" => {
                let (line, _) = self.line_at(self.cursor);
                let to = self.offset_at(line, 0);
                return self.move_cursor(to, modifiers.shift, cx);
            }
            "end" => {
                let (line, _) = self.line_at(self.cursor);
                let to = self.offset_at(line, usize::MAX);
                return self.move_cursor(to, modifiers.shift, cx);
            }
            "escape" => {
                self.anchor = self.cursor;
                cx.notify();
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
        if text.chars().any(|character| character.is_control()) {
            return;
        }
        self.replace_selection(text, window, cx);
    }
}

fn clamp_boundary(value: &str, mut index: usize) -> usize {
    index = index.min(value.len());
    while index > 0 && !value.is_char_boundary(index) {
        index -= 1;
    }
    index
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
    value[cursor..]
        .char_indices()
        .nth(1)
        .map(|(index, _)| cursor + index)
        .or(Some(value.len()))
}

const LINE_HEIGHT: f32 = 18.0;

impl Render for TextArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let focused = self.focus_handle.is_focused(window);
        let (selection_start, selection_end) = self.selection();
        let (cursor_line, _) = self.line_at(self.cursor);
        let starts = self.line_starts();

        let mut lines = div().flex().flex_col();
        if self.value.is_empty() && !self.placeholder.is_empty() {
            lines = lines.child(
                div()
                    .h(px(LINE_HEIGHT))
                    .text_color(theme.muted_foreground)
                    .child(self.placeholder.clone()),
            );
        }

        for (index, start) in starts.iter().copied().enumerate() {
            let end = starts
                .get(index + 1)
                .map(|next| next.saturating_sub(1))
                .unwrap_or(self.value.len());
            let text = &self.value[start..end];

            // Each line is split into the part before the selection, the
            // selected part and the rest, so the caret and highlight land
            // between the right glyphs without measuring them.
            let highlight_start = selection_start.clamp(start, end) - start;
            let highlight_end = selection_end.clamp(start, end) - start;
            let caret_at =
                (focused && cursor_line == index).then(|| self.cursor.clamp(start, end) - start);

            let mut row = div()
                .flex()
                .flex_row()
                .h(px(LINE_HEIGHT))
                .child(SharedString::from(text[..highlight_start].to_string()));

            if highlight_end > highlight_start {
                row = row.child(
                    div()
                        .bg(theme.accent.opacity(0.35))
                        .child(SharedString::from(
                            text[highlight_start..highlight_end].to_string(),
                        )),
                );
            }
            if let Some(caret) = caret_at.filter(|caret| *caret >= highlight_end) {
                row = row
                    .child(SharedString::from(text[highlight_end..caret].to_string()))
                    .child(div().w(px(1.0)).h(px(LINE_HEIGHT)).bg(theme.foreground))
                    .child(SharedString::from(text[caret..].to_string()));
            } else {
                row = row.child(SharedString::from(text[highlight_end..].to_string()));
            }

            lines = lines.child(row);
        }

        div()
            .id("text-area")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _event, window, cx| {
                    window.focus(&this.focus_handle);
                    cx.notify();
                }),
            )
            .track_scroll(&self.scroll)
            .overflow_y_scroll()
            .w_full()
            .h(px(self.rows as f32 * LINE_HEIGHT + 16.0))
            // `ui/textarea.tsx`: `rounded-field border-0 bg-field px-3 py-2`,
            // with the field focus ring instead of a border.
            .rounded(px(crate::ui::chrome::FIELD_RADIUS))
            .bg(theme.field_background)
            .when(focused, |el| {
                el.shadow(crate::ui::primitives::focus_ring(&theme, 0.0))
            })
            .px(px(crate::ui::chrome::FIELD_PAD_X))
            .py(px(crate::ui::chrome::FIELD_PAD_Y))
            .font_family("Consolas")
            .text_size(px(12.0))
            .text_color(theme.foreground)
            .child(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries_step_over_whole_characters() {
        let value = "a\u{00e9}b";
        assert_eq!(previous_boundary(value, 0), None);
        assert_eq!(next_boundary(value, value.len()), None);
        assert_eq!(next_boundary(value, 0), Some(1));
        // The two-byte character is skipped in one step.
        assert_eq!(next_boundary(value, 1), Some(3));
        assert_eq!(previous_boundary(value, 3), Some(1));
    }

    #[test]
    fn clamp_boundary_backs_up_to_a_char_start() {
        let value = "é";
        assert_eq!(clamp_boundary(value, 1), 0);
        assert_eq!(clamp_boundary(value, 2), 2);
    }
}
