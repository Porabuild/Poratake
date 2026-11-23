use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    canvas, div, fill, point, prelude::*, px, size, Animation, AnimationExt, App, Bounds,
    ClipboardItem, Entity, FocusHandle, Focusable, Font, Hsla, InputHandler, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    RenderOnce, ShapedLine, SharedString, Styled, TextAlign, TextRun, UTF16Selection,
    UnderlineStyle, Window,
};
use herogpui::gpui;
use herogpui::theme::ActiveTheme;

pub struct InlineTextState {
    value: String,
    cursor: usize,
    anchor: Option<usize>,
    marked: Option<Range<usize>>,
    focus_handle: FocusHandle,
    selecting: bool,
    caret_epoch: u64,
    last_bounds: Option<Bounds<Pixels>>,
    last_line: Option<ShapedLine>,
}

impl InlineTextState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            value: String::new(),
            cursor: 0,
            anchor: None,
            marked: None,
            focus_handle: cx.focus_handle(),
            selecting: false,
            caret_epoch: 0,
            last_bounds: None,
            last_line: None,
        }
    }

    pub fn with_value(cx: &mut App, value: impl Into<String>) -> Self {
        let mut state = Self::new(cx);
        let value = sanitize_single_line(&value.into());
        state.cursor = value.chars().count();
        state.value = value;
        state
    }

    fn bump_caret(&mut self) {
        self.caret_epoch = self.caret_epoch.wrapping_add(1);
    }

    #[cfg(test)]
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn is_selecting(&self) -> bool {
        self.selecting
    }

    pub fn end_selection(&mut self) {
        self.selecting = false;
    }

    pub fn update_drag_selection(&mut self, position: Point<Pixels>) -> bool {
        if !self.selecting {
            return false;
        }
        let Some(index) = self.char_index_for_point(position) else {
            return false;
        };
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        if self.cursor == index {
            return false;
        }
        self.cursor = index;
        true
    }

    pub fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        if anchor == self.cursor {
            return None;
        }
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    pub fn selected_text(&self) -> Option<String> {
        let (lo, hi) = self.selection()?;
        Some(self.value.chars().skip(lo).take(hi - lo).collect())
    }

    pub fn select_all(&mut self) {
        let len = self.value.chars().count();
        self.marked = None;
        if len == 0 {
            self.anchor = None;
            self.cursor = 0;
            return;
        }
        self.anchor = Some(0);
        self.cursor = len;
    }

    fn delete_selection(&mut self) -> bool {
        let Some((lo, hi)) = self.selection() else {
            return false;
        };
        let start = char_to_byte(&self.value, lo);
        let end = char_to_byte(&self.value, hi);
        self.value.replace_range(start..end, "");
        self.cursor = lo;
        self.anchor = None;
        self.marked = None;
        self.bump_caret();
        true
    }

    pub fn insert_str(&mut self, text: &str) -> bool {
        let clean = sanitize_single_line(text);
        let had_selection = self.delete_selection();
        if clean.is_empty() {
            return had_selection;
        }
        let byte = char_to_byte(&self.value, self.cursor);
        self.value.insert_str(byte, &clean);
        self.cursor += clean.chars().count();
        self.anchor = None;
        self.marked = None;
        self.bump_caret();
        true
    }

    fn replace_range_utf16(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        marked_selection: Option<Option<Range<usize>>>,
    ) -> bool {
        let old_value = self.value.clone();
        let clean = sanitize_single_line(text);
        let char_range = range
            .map(|r| {
                let start = utf16_to_char(&self.value, r.start);
                let end = utf16_to_char(&self.value, r.end);
                start.min(end)..start.max(end)
            })
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| {
                let (lo, hi) = self.selection().unwrap_or((self.cursor, self.cursor));
                lo..hi
            });
        let start_byte = char_to_byte(&self.value, char_range.start);
        let end_byte = char_to_byte(&self.value, char_range.end);
        self.value.replace_range(start_byte..end_byte, &clean);
        let inserted_chars = clean.chars().count();
        let end_char = char_range.start + inserted_chars;
        self.cursor = end_char;
        self.anchor = None;
        self.marked = marked_selection
            .as_ref()
            .and_then(|_| (end_char > char_range.start).then_some(char_range.start..end_char));
        if let Some(Some(sel)) = marked_selection {
            let start = char_range.start + utf16_to_char(&clean, sel.start);
            let end = char_range.start + utf16_to_char(&clean, sel.end);
            self.anchor = (start != end).then_some(start.min(end));
            self.cursor = start.max(end);
        }
        self.bump_caret();
        self.value != old_value
    }

    pub fn backspace(&mut self, word: bool, to_start: bool) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor == 0 {
            return false;
        }
        let target = if to_start {
            0
        } else if word {
            prev_word_boundary(&self.value, self.cursor)
        } else {
            self.cursor - 1
        };
        let start = char_to_byte(&self.value, target);
        let end = char_to_byte(&self.value, self.cursor);
        self.value.replace_range(start..end, "");
        self.cursor = target;
        self.anchor = None;
        self.marked = None;
        self.bump_caret();
        true
    }

    pub fn delete(&mut self, word: bool) -> bool {
        if self.delete_selection() {
            return true;
        }
        let len = self.value.chars().count();
        if self.cursor >= len {
            return false;
        }
        let target = if word {
            next_word_boundary(&self.value, self.cursor)
        } else {
            self.cursor + 1
        };
        let start = char_to_byte(&self.value, self.cursor);
        let end = char_to_byte(&self.value, target);
        self.value.replace_range(start..end, "");
        self.anchor = None;
        self.marked = None;
        self.bump_caret();
        true
    }

    fn set_cursor(&mut self, target: usize, shift: bool) {
        self.marked = None;
        if shift {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
        self.cursor = target;
        if self.anchor == Some(self.cursor) {
            self.anchor = None;
        }
        self.bump_caret();
    }

    pub fn move_left(&mut self, shift: bool, word: bool) {
        if !shift {
            if let Some((lo, _)) = self.selection() {
                self.set_cursor(lo, false);
                return;
            }
        }
        let target = if word {
            prev_word_boundary(&self.value, self.cursor)
        } else {
            self.cursor.saturating_sub(1)
        };
        self.set_cursor(target, shift);
    }

    pub fn move_right(&mut self, shift: bool, word: bool) {
        let len = self.value.chars().count();
        if !shift {
            if let Some((_, hi)) = self.selection() {
                self.set_cursor(hi, false);
                return;
            }
        }
        let target = if word {
            next_word_boundary(&self.value, self.cursor)
        } else {
            (self.cursor + 1).min(len)
        };
        self.set_cursor(target, shift);
    }

    pub fn move_home(&mut self, shift: bool) {
        self.set_cursor(0, shift);
    }

    pub fn move_end(&mut self, shift: bool) {
        let len = self.value.chars().count();
        self.set_cursor(len, shift);
    }

    pub fn select_word_at(&mut self, index: usize) {
        let chars: Vec<char> = self.value.chars().collect();
        if chars.is_empty() {
            self.anchor = None;
            self.cursor = 0;
            return;
        }
        let clamped = index.min(chars.len().saturating_sub(1));
        let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
        let target_kind = is_word(chars[clamped]);
        let mut start = clamped;
        while start > 0 && is_word(chars[start - 1]) == target_kind {
            start -= 1;
        }
        let mut end = clamped + 1;
        while end < chars.len() && is_word(chars[end]) == target_kind {
            end += 1;
        }
        self.anchor = Some(start);
        self.cursor = end;
        self.marked = None;
    }

    fn char_index_for_point(&self, position: Point<Pixels>) -> Option<usize> {
        let bounds = self.last_bounds?;
        let line = self.last_line.as_ref()?;
        let local_x = (position.x - bounds.left()).max(px(0.0));
        let mut byte = line.closest_index_for_x(local_x).min(self.value.len());
        while byte > 0 && !self.value.is_char_boundary(byte) {
            byte -= 1;
        }
        Some(self.value[..byte].chars().count())
    }
}

impl Focusable for InlineTextState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus_handle(cx)
    }
}

struct InlineTextInputHandler {
    state: Entity<InlineTextState>,
    on_change: Rc<dyn Fn(&str, &mut Window, &mut App)>,
    bounds: Bounds<Pixels>,
    font: Font,
    font_size: Pixels,
    line_height: Pixels,
}

impl InlineTextInputHandler {
    fn shape_value(&self, value: &str, window: &Window) -> ShapedLine {
        let run = plain_run(value.len(), &self.font, gpui::black());
        window.text_system().shape_line(
            SharedString::from(value.to_owned()),
            self.font_size,
            &[run],
            None,
        )
    }
}

impl InputHandler for InlineTextInputHandler {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection> {
        let state = self.state.read(cx);
        let (lo, hi) = state.selection().unwrap_or((state.cursor, state.cursor));
        Some(UTF16Selection {
            range: char_to_utf16(&state.value, lo)..char_to_utf16(&state.value, hi),
            reversed: state.anchor.is_some_and(|anchor| anchor > state.cursor),
        })
    }

    fn marked_text_range(&mut self, _window: &mut Window, cx: &mut App) -> Option<Range<usize>> {
        let state = self.state.read(cx);
        state.marked.as_ref().map(|range| {
            char_to_utf16(&state.value, range.start)..char_to_utf16(&state.value, range.end)
        })
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        let value = &self.state.read(cx).value;
        let start = utf16_to_char(value, range_utf16.start);
        let end = utf16_to_char(value, range_utf16.end);
        *adjusted_range = Some(char_to_utf16(value, start)..char_to_utf16(value, end));
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
        replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        let changed = self.state.update(cx, |state, cx| {
            let changed = state.replace_range_utf16(replacement_range, text, None);
            cx.notify();
            changed
        });
        if changed {
            let value = self.state.read(cx).value.clone();
            (self.on_change)(&value, window, cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let changed = self.state.update(cx, |state, cx| {
            let changed =
                state.replace_range_utf16(range_utf16, new_text, Some(new_selected_range));
            cx.notify();
            changed
        });
        if changed {
            let value = self.state.read(cx).value.clone();
            (self.on_change)(&value, window, cx);
        }
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            state.marked = None;
            cx.notify();
        });
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.state.update(cx, |state, cx| {
            let start = utf16_to_char(&state.value, range_utf16.start);
            let end = utf16_to_char(&state.value, range_utf16.end);
            state.anchor = (start != end).then_some(start);
            state.cursor = end;
            cx.notify();
        });
    }

    fn text_length_utf16(&mut self, _window: &mut Window, cx: &mut App) -> Option<usize> {
        Some(self.state.read(cx).value.encode_utf16().count())
    }

    fn accepts_text_input(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        true
    }

    fn prefers_ime_for_printable_keys(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        true
    }

    fn element_bounds(&mut self, _window: &mut Window, _cx: &mut App) -> Option<Bounds<Pixels>> {
        Some(self.bounds)
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        let value = self.state.read(cx).value.clone();
        let start_byte = char_to_byte(&value, utf16_to_char(&value, range_utf16.start));
        let end_byte = char_to_byte(&value, utf16_to_char(&value, range_utf16.end));
        let shaped = self.shape_value(&value, window);
        let left = shaped.x_for_index(start_byte);
        let right = shaped.x_for_index(end_byte);
        Some(Bounds::new(
            point(self.bounds.left() + left, self.bounds.top()),
            size((right - left).max(px(1.0)), self.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize> {
        let value = self.state.read(cx).value.clone();
        let shaped = self.shape_value(&value, window);
        let local_x = (position.x - self.bounds.left()).max(px(0.0));
        let byte = shaped.closest_index_for_x(local_x).min(value.len());
        let char_idx = value[..byte].chars().count();
        Some(char_to_utf16(&value, char_idx))
    }
}

#[derive(IntoElement)]
pub struct InlineTextInput {
    pub state: Entity<InlineTextState>,
    pub x: f64,
    pub y: f64,
    pub zoom: f32,
    pub font_size: f64,
    pub font_family: &'static str,
    pub text_color: Hsla,
    pub background_color: Option<Hsla>,
    pub padding_x: f32,
    pub padding_y: f32,
    pub radius: f32,
    pub border_color: Hsla,
    pub selection_color: Hsla,
    pub on_change: Rc<dyn Fn(&str, &mut Window, &mut App)>,
    pub on_commit: Rc<dyn Fn(&mut Window, &mut App)>,
    pub on_cancel: Rc<dyn Fn(&mut Window, &mut App)>,
}

impl RenderOnce for InlineTextInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let scaled_font_size_f32 = (self.font_size as f32 * self.zoom).max(1.0);
        let scaled_font_size = px(scaled_font_size_f32);
        let font = gpui::font(self.font_family);

        let (value, cursor, selection, marked, focus_handle, caret_epoch) = {
            let state = self.state.read(cx);
            (
                state.value.clone(),
                state.cursor,
                state.selection(),
                state.marked.clone(),
                state.focus_handle.clone(),
                state.caret_epoch,
            )
        };
        let focused = focus_handle.is_focused(window);

        let runs = build_text_runs(&value, &font, self.text_color, marked.as_ref());
        let shaped = window.text_system().shape_line(
            SharedString::from(value.clone()),
            scaled_font_size,
            &runs,
            None,
        );

        let line_height = if value.is_empty() {
            let probe_run = TextRun {
                len: 1,
                font: font.clone(),
                color: self.text_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let probe = window.text_system().shape_line(
                SharedString::from("M"),
                scaled_font_size,
                &[probe_run],
                None,
            );
            (probe.ascent + probe.descent).max(scaled_font_size)
        } else {
            (shaped.ascent + shaped.descent).max(scaled_font_size)
        };

        let caret_width = px((scaled_font_size_f32 * 0.06).clamp(1.5, 3.0));
        let min_width = px((scaled_font_size_f32 * 0.35).clamp(12.0, 24.0));
        let content_width = shaped.width().max(min_width) + caret_width + px(2.0);
        let border_width = px(1.5);
        let left = px(self.x as f32 * self.zoom) - px(self.padding_x) - border_width;
        let top = px(self.y as f32 * self.zoom) - px(self.padding_y) - border_width;

        let caret_x = shaped.x_for_index(char_to_byte(&value, cursor));
        let selection_x = selection.map(|(lo, hi)| {
            (
                shaped.x_for_index(char_to_byte(&value, lo)),
                shaped.x_for_index(char_to_byte(&value, hi)),
            )
        });

        let paint_state = self.state.clone();
        let handler_state = self.state.clone();
        let paint_line = shaped.clone();
        let paint_focus = focus_handle.clone();
        let paint_change = self.on_change.clone();
        let paint_font = font.clone();
        let paint_selection_color = self.selection_color;
        let paint_is_empty = value.is_empty();

        let text_canvas = canvas(
            move |bounds, _window, cx| {
                let line = paint_line.clone();
                paint_state.update(cx, |state, _| {
                    state.last_bounds = Some(bounds);
                    state.last_line = Some(line);
                });
                (bounds, paint_line)
            },
            move |_bounds, (bounds, line), window, cx| {
                window.handle_input(
                    &paint_focus,
                    InlineTextInputHandler {
                        state: handler_state.clone(),
                        on_change: paint_change.clone(),
                        bounds,
                        font: paint_font.clone(),
                        font_size: scaled_font_size,
                        line_height,
                    },
                    cx,
                );
                if let Some((x0, x1)) = selection_x {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + x0, bounds.top()),
                            size((x1 - x0).max(px(2.0)), line_height),
                        ),
                        paint_selection_color,
                    ));
                }
                if !paint_is_empty {
                    let _ = line.paint(
                        bounds.origin,
                        line_height,
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
            },
        )
        .w(content_width)
        .h(line_height);

        let mut content = div()
            .relative()
            .w(content_width)
            .h(line_height)
            .child(text_canvas);

        if focused && selection.is_none() {
            let caret = div()
                .w(caret_width)
                .h(line_height)
                .bg(self.text_color)
                .rounded(px(0.5));
            let caret = if ActiveTheme::reduce_motion(cx) {
                caret.into_any_element()
            } else {
                caret
                    .with_animation(
                        ("inline-text-caret", caret_epoch),
                        Animation::new(Duration::from_millis(1000)).repeat(),
                        |el, delta| el.opacity(caret_opacity(delta)),
                    )
                    .into_any_element()
            };
            content = content.child(div().absolute().left(caret_x).top_0().child(caret));
        }

        let down_state = self.state.clone();
        let down_focus = focus_handle.clone();
        let move_state = self.state.clone();
        let up_state = self.state.clone();
        let key_state = self.state.clone();
        let key_focus = focus_handle.clone();
        let on_change = self.on_change.clone();
        let on_commit = self.on_commit.clone();
        let on_cancel = self.on_cancel.clone();

        div()
            .id("inline-text-editor")
            .debug_selector(|| "inline-text-editor".to_owned())
            .absolute()
            .left(left)
            .top(top)
            .border(border_width)
            .border_color(self.border_color)
            .when_some(self.background_color, |el, bg| el.bg(bg))
            .rounded(px(self.radius))
            .px(px(self.padding_x))
            .py(px(self.padding_y))
            .cursor(gpui::CursorStyle::IBeam)
            .track_focus(&focus_handle)
            .key_context("InlineTextEditor")
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    window.prevent_default();
                    window.focus(&down_focus, cx);
                    down_state.update(cx, |state, cx| {
                        let index = state
                            .char_index_for_point(event.position)
                            .unwrap_or_else(|| state.value.chars().count());
                        state.marked = None;
                        match event.click_count {
                            0 | 1 => {
                                state.set_cursor(index, event.modifiers.shift);
                                state.selecting = true;
                            }
                            2 => {
                                state.select_word_at(index);
                                state.selecting = false;
                            }
                            _ => {
                                state.select_all();
                                state.selecting = false;
                            }
                        }
                        cx.notify();
                    });
                },
            )
            .on_mouse_down(MouseButton::Right, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_mouse_move(move |event: &MouseMoveEvent, _window, cx| {
                if !move_state.read(cx).selecting {
                    return;
                }
                cx.stop_propagation();
                move_state.update(cx, |state, cx| {
                    if state.update_drag_selection(event.position) {
                        cx.notify();
                    }
                });
            })
            .on_mouse_up(MouseButton::Left, move |_: &MouseUpEvent, _window, cx| {
                cx.stop_propagation();
                up_state.update(cx, |state, _| {
                    state.selecting = false;
                });
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let mods = event.keystroke.modifiers;
                if key_state.read(cx).marked.is_some() && key != "escape" {
                    cx.stop_propagation();
                    return;
                }

                let secondary = mods.secondary(cx);
                let word_mod = if cfg!(target_os = "macos") {
                    mods.alt
                } else {
                    mods.control
                };
                let line_mod = cfg!(target_os = "macos") && mods.platform;

                let mut changed = false;
                match key {
                    "escape" => {
                        cx.stop_propagation();
                        on_cancel(window, cx);
                        return;
                    }
                    "enter" => {
                        cx.stop_propagation();
                        on_commit(window, cx);
                        return;
                    }
                    "backspace" => {
                        cx.stop_propagation();
                        changed = key_state.update(cx, |state, cx| {
                            let edited = state.backspace(word_mod, line_mod);
                            if edited {
                                cx.notify();
                            }
                            edited
                        });
                    }
                    "delete" => {
                        cx.stop_propagation();
                        changed = key_state.update(cx, |state, cx| {
                            let edited = state.delete(word_mod);
                            if edited {
                                cx.notify();
                            }
                            edited
                        });
                    }
                    "left" => {
                        cx.stop_propagation();
                        key_state.update(cx, |state, cx| {
                            if line_mod {
                                state.move_home(mods.shift);
                            } else {
                                state.move_left(mods.shift, word_mod);
                            }
                            cx.notify();
                        });
                    }
                    "right" => {
                        cx.stop_propagation();
                        key_state.update(cx, |state, cx| {
                            if line_mod {
                                state.move_end(mods.shift);
                            } else {
                                state.move_right(mods.shift, word_mod);
                            }
                            cx.notify();
                        });
                    }
                    "home" => {
                        cx.stop_propagation();
                        key_state.update(cx, |state, cx| {
                            state.move_home(mods.shift);
                            cx.notify();
                        });
                    }
                    "end" => {
                        cx.stop_propagation();
                        key_state.update(cx, |state, cx| {
                            state.move_end(mods.shift);
                            cx.notify();
                        });
                    }
                    "a" if secondary && !mods.alt => {
                        cx.stop_propagation();
                        key_state.update(cx, |state, cx| {
                            state.select_all();
                            cx.notify();
                        });
                    }
                    "c" if secondary && !mods.alt => {
                        cx.stop_propagation();
                        if let Some(selected) = key_state.read(cx).selected_text() {
                            cx.write_to_clipboard(ClipboardItem::new_string(selected));
                        }
                    }
                    "x" if secondary && !mods.alt => {
                        cx.stop_propagation();
                        if let Some(selected) = key_state.read(cx).selected_text() {
                            cx.write_to_clipboard(ClipboardItem::new_string(selected));
                            changed = key_state.update(cx, |state, cx| {
                                let edited = state.delete_selection();
                                if edited {
                                    cx.notify();
                                }
                                edited
                            });
                        }
                    }
                    "v" if secondary && !mods.alt => {
                        cx.stop_propagation();
                        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                            changed = key_state.update(cx, |state, cx| {
                                let edited = state.insert_str(&text);
                                if edited {
                                    cx.notify();
                                }
                                edited
                            });
                        }
                    }
                    _ => {}
                }

                if changed {
                    window.notify_text_input_changed(&key_focus);
                    let value = key_state.read(cx).value.clone();
                    on_change(&value, window, cx);
                }
            })
            .child(content)
    }
}

fn plain_run(len: usize, font: &Font, color: Hsla) -> TextRun {
    TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

fn build_text_runs(
    value: &str,
    font: &Font,
    color: Hsla,
    marked: Option<&Range<usize>>,
) -> Vec<TextRun> {
    if value.is_empty() {
        return Vec::new();
    }
    let Some(range) = marked.filter(|r| r.start < r.end) else {
        return vec![plain_run(value.len(), font, color)];
    };
    let start = char_to_byte(value, range.start);
    let end = char_to_byte(value, range.end);
    let mut runs = Vec::with_capacity(3);
    if start > 0 {
        runs.push(plain_run(start, font, color));
    }
    if end > start {
        let mut marked_run = plain_run(end - start, font, color);
        marked_run.underline = Some(UnderlineStyle {
            thickness: px(1.0),
            color: Some(color),
            wavy: false,
        });
        runs.push(marked_run);
    }
    if end < value.len() {
        runs.push(plain_run(value.len() - end, font, color));
    }
    runs
}

fn caret_opacity(delta: f32) -> f32 {
    if delta < 0.20 {
        1.0 - (delta / 0.20)
    } else if delta < 0.50 {
        0.0
    } else if delta < 0.70 {
        (delta - 0.50) / 0.20
    } else {
        1.0
    }
}

fn sanitize_single_line(text: &str) -> String {
    text.chars()
        .filter(|ch| *ch != '\r')
        .map(|ch| if ch == '\n' { ' ' } else { ch })
        .collect()
}

fn char_to_byte(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map_or(value.len(), |(byte, _)| byte)
}

fn char_to_utf16(value: &str, char_index: usize) -> usize {
    value.chars().take(char_index).map(char::len_utf16).sum()
}

fn utf16_to_char(value: &str, utf16_offset: usize) -> usize {
    let mut units = 0;
    value
        .chars()
        .take_while(|ch| {
            units += ch.len_utf16();
            units <= utf16_offset
        })
        .count()
}

fn prev_word_boundary(value: &str, cursor: usize) -> usize {
    let chars: Vec<char> = value.chars().collect();
    let mut idx = cursor.min(chars.len());
    while idx > 0 && chars[idx - 1].is_whitespace() {
        idx -= 1;
    }
    while idx > 0 && !chars[idx - 1].is_whitespace() {
        idx -= 1;
    }
    idx
}

fn next_word_boundary(value: &str, cursor: usize) -> usize {
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    let mut idx = cursor.min(len);
    while idx < len && !chars[idx].is_whitespace() {
        idx += 1;
    }
    while idx < len && chars[idx].is_whitespace() {
        idx += 1;
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[herogpui::test]
    fn inline_text_state_handles_editing_selection_word_ops_and_utf16(
        cx: &mut gpui::TestAppContext,
    ) {
        let state = cx.update(|cx| cx.new(|cx| InlineTextState::with_value(cx, "hello world")));
        cx.update(|cx| {
            state.update(cx, |s, _| {
                assert_eq!(s.value(), "hello world");
                s.move_left(true, true);
                assert_eq!(s.selected_text().as_deref(), Some("world"));
                assert!(s.insert_str("gpui\nfast"));
                assert_eq!(s.value(), "hello gpui fast");
                assert!(s.backspace(true, false));
                assert_eq!(s.value(), "hello gpui ");
                s.move_home(false);
                assert!(s.delete(true));
                assert_eq!(s.value(), "gpui ");
                s.select_all();
                assert_eq!(s.selected_text().as_deref(), Some("gpui "));
                assert!(s.replace_range_utf16(None, "héllo 😀", Some(None)));
                assert_eq!(s.value(), "héllo 😀");
                assert_eq!(s.marked, Some(0..7));
                let utf16_len = char_to_utf16(s.value(), 7);
                assert_eq!(utf16_len, 8);
                assert_eq!(utf16_to_char(s.value(), utf16_len), 7);
                assert!(s.replace_range_utf16(None, "done", None));
                assert_eq!(s.value(), "done");
                assert_eq!(s.marked, None);
            });
        });
    }
}
