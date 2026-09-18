//! Port of `cursor-data-editor-dialog.tsx` and `subtitle-data-editor-dialog.tsx`
//! — a JSON editor over one of the project's sidecars, with the same template
//! and example the renderer offers and the same validation before saving.

use std::path::PathBuf;

use gpui::{div, prelude::*, px, AnyElement, Context, Entity, SharedString, Styled};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;
use crate::ui::icon::icon_element;
use crate::video::project;
use crate::video::sidecars::{CursorData, SubtitleData};
use crate::windows::video_editor::VideoEditorWindow;
use herogpui::components::{Button, InputState, Size, TextArea, Variant};

/// Which sidecar an open editor is editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataKind {
    Cursor,
    Subtitle,
}

impl DataKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Cursor => "Edit Cursor Data",
            Self::Subtitle => "Edit Subtitle Data",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Cursor => "Manually enter or modify cursor movement data in JSON format.",
            Self::Subtitle => "Manually enter or modify subtitle data in JSON format.",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor Data (JSON)",
            Self::Subtitle => "Subtitle Data (JSON)",
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            Self::Cursor => "Enter cursor data JSON...",
            Self::Subtitle => "Enter subtitle data JSON...",
        }
    }

    pub fn field_documentation(self) -> &'static str {
        match self {
            Self::Cursor => concat!(
                "recordingArea: Video dimensions in pixels\n",
                "events: Array of cursor events with:\n",
                "  timestamp: Time in seconds\n",
                "  x, y: Position (0-1, normalized)\n",
                "  type: move, down, up, or scroll\n",
                "  button: left, right, middle (optional)\n",
                "  cursor: arrow, pointingHand, iBeam, etc. (optional)\n",
                "meta: Recording metadata",
            ),
            Self::Subtitle => concat!(
                "segments: Array of subtitle segments with:\n",
                "  start: Start time in seconds\n",
                "  end: End time in seconds\n",
                "  text: Subtitle text content\n",
                "  words: Word-level timing (optional)\n",
                "meta: Metadata including language and model",
            ),
        }
    }

    pub fn path(self, project_or_video: &std::path::Path) -> PathBuf {
        match self {
            Self::Cursor => project::cursor_path(project_or_video),
            Self::Subtitle => project::subtitle_path(project_or_video),
        }
    }

    /// `EXAMPLE_CURSOR_DATA` / the subtitle dialog's example.
    pub fn example(self) -> String {
        match self {
            Self::Cursor => serde_json::to_string_pretty(&serde_json::json!({
                "recordingArea": { "width": 1920, "height": 1080 },
                "events": [
                    { "timestamp": 0.0, "x": 0.5, "y": 0.5, "type": "move", "cursor": "arrow" },
                    { "timestamp": 1.0, "x": 0.6, "y": 0.4, "type": "move" },
                    { "timestamp": 2.0, "x": 0.7, "y": 0.3, "type": "down", "button": "left" },
                    { "timestamp": 2.1, "x": 0.7, "y": 0.3, "type": "up", "button": "left" }
                ],
                "meta": {
                    "startTime": "2024-01-01T00:00:00.000Z",
                    "duration": 10,
                    "sampleRate": 60
                }
            }))
            .unwrap_or_default(),
            Self::Subtitle => serde_json::to_string_pretty(&serde_json::json!({
                "segments": [
                    { "start": 0.0, "end": 2.5, "text": "Hello, welcome to this video." },
                    { "start": 2.5, "end": 5.0, "text": "Today we will learn about subtitles." },
                    { "start": 5.0, "end": 8.0, "text": "Each segment has a start and end time." }
                ],
                "meta": {
                    "generatedAt": "2024-01-01T00:00:00.000Z",
                    "language": "en",
                    "model": "manual"
                }
            }))
            .unwrap_or_default(),
        }
    }

    /// `generateTemplate` — an empty document sized to this recording.
    pub fn template(self, width: f64, height: f64, duration: f64) -> String {
        match self {
            Self::Cursor => serde_json::to_string_pretty(&serde_json::json!({
                "recordingArea": { "width": width, "height": height },
                "events": [
                    { "timestamp": 0.0, "x": 0.5, "y": 0.5, "type": "move", "cursor": "arrow" }
                ],
                "meta": {
                    "startTime": chrono::Utc::now().to_rfc3339(),
                    "duration": duration,
                    "sampleRate": 60
                }
            }))
            .unwrap_or_default(),
            Self::Subtitle => serde_json::to_string_pretty(&serde_json::json!({
                "segments": [
                    { "start": 0.0, "end": duration.min(3.0), "text": "Your subtitle text here" }
                ],
                "meta": {
                    "generatedAt": chrono::Utc::now().to_rfc3339(),
                    "language": "en",
                    "model": "manual"
                }
            }))
            .unwrap_or_default(),
        }
    }

    /// `validateCursorData` / `validateSubtitleData`, returning the normalized
    /// document the renderer would have written.
    pub fn validate(self, value: &str) -> Result<String, String> {
        match self {
            Self::Cursor => {
                let parsed: CursorData = serde_json::from_str(value)
                    .map_err(|error| format!("Invalid cursor data: {error}"))?;
                if parsed.events.is_empty() {
                    return Err("Invalid cursor data: no events".to_string());
                }
                serde_json::to_string_pretty(&parsed)
                    .map_err(|error| format!("Invalid cursor data: {error}"))
            }
            Self::Subtitle => {
                let parsed: SubtitleData = serde_json::from_str(value)
                    .map_err(|error| format!("Invalid subtitle data: {error}"))?;
                if parsed.segments.is_empty() {
                    return Err("Invalid subtitle data: no segments".to_string());
                }
                serde_json::to_string_pretty(&parsed)
                    .map_err(|error| format!("Invalid subtitle data: {error}"))
            }
        }
    }
}

/// The open editor's state, owned by the video editor window.
pub struct DataEditor {
    pub kind: DataKind,
    pub field: Entity<InputState>,
    pub error: Option<SharedString>,
    pub saving: bool,
}

impl DataEditor {
    pub fn open(
        kind: DataKind,
        project: &std::path::Path,
        width: f64,
        height: f64,
        duration: f64,
        cx: &mut Context<VideoEditorWindow>,
    ) -> Self {
        let existing = std::fs::read_to_string(kind.path(project)).ok();
        let initial = existing.unwrap_or_else(|| kind.template(width, height, duration));
        let field = cx.new(|cx| InputState::with_value(cx, initial));
        Self {
            kind,
            field,
            error: None,
            saving: false,
        }
    }
}

/// The dialog surface, rendered above the editor.
pub fn render(
    editor: &DataEditor,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let kind = editor.kind;
    let error = editor.error.clone();
    let saving = editor.saving;

    div()
        .id("data-editor-backdrop")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(crate::ui::colors::black(0.5))
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|this, _event, _window, cx| this.close_data_editor(cx)),
        )
        .child(
            div()
                .id("data-editor-card")
                .flex()
                .flex_col()
                .gap(px(12.0))
                .w(px(672.0))
                .max_h(gpui::relative(0.9))
                .on_mouse_down(gpui::MouseButton::Left, |_event, _window, cx| {
                    cx.stop_propagation();
                })
                .rounded(px(10.0))
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover)
                .text_color(theme.popover_foreground)
                .shadow_lg()
                .p(px(16.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(kind.title()),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme.muted_foreground)
                                .child(kind.description()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.0))
                                .child(div().text_size(px(12.0)).child(kind.label()))
                                .child(
                                    herogpui::components::Tooltip::new(kind.field_documentation())
                                        .child(
                                            div()
                                                .text_color(theme.muted_foreground)
                                                .child(icon_element("help-circle", px(14.0))),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(4.0))
                                .child(
                                    Button::new("data-editor-template")
                                        .variant(Variant::Ghost)
                                        .recipe("compact")
                                        .label("Load Template")
                                        .on_press(cx.listener(|this, _event, _window, cx| {
                                            this.load_data_editor_template(cx)
                                        })),
                                )
                                .child(
                                    Button::new("data-editor-example")
                                        .variant(Variant::Ghost)
                                        .recipe("compact")
                                        .label("Load Example")
                                        .on_press(cx.listener(|this, _event, _window, cx| {
                                            this.load_data_editor_example(cx)
                                        })),
                                ),
                        ),
                )
                .child(
                    div().flex_1().min_h_0().child(
                        TextArea::new(editor.field.clone())
                            .rows(16)
                            .placeholder(kind.placeholder())
                            .font_family(crate::ui::colors::MONO_FONT),
                    ),
                )
                .when_some(error, |el, error| {
                    el.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .rounded(px(6.0))
                            .bg(theme.destructive.opacity(0.12))
                            .text_color(theme.destructive)
                            .p(px(10.0))
                            .text_size(px(12.0))
                            .child(icon_element("alert-circle", px(14.0)))
                            .child(error),
                    )
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            Button::new("data-editor-cancel")
                                .variant(Variant::Tertiary)
                                .size(Size::Sm)
                                .label("Cancel")
                                .on_press(cx.listener(|this, _event, _window, cx| {
                                    this.close_data_editor(cx)
                                })),
                        )
                        .child(
                            Button::new("data-editor-save")
                                .variant(Variant::Primary)
                                .size(Size::Sm)
                                .label(if saving { "Saving..." } else { "Save" })
                                .is_disabled(saving)
                                .on_press(cx.listener(|this, _event, _window, cx| {
                                    this.save_data_editor(cx)
                                })),
                        ),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_example_validates() {
        let normalized = DataKind::Cursor
            .validate(&DataKind::Cursor.example())
            .expect("example");
        assert!(normalized.contains("recordingArea"));
    }

    #[test]
    fn the_subtitle_example_validates() {
        let normalized = DataKind::Subtitle
            .validate(&DataKind::Subtitle.example())
            .expect("example");
        assert!(normalized.contains("segments"));
    }

    #[test]
    fn templates_are_sized_to_the_recording() {
        let template = DataKind::Cursor.template(1280.0, 720.0, 5.5);
        assert!(template.contains("1280"));
        assert!(template.contains("5.5"));
        assert!(DataKind::Cursor.validate(&template).is_ok());
    }

    #[test]
    fn an_empty_document_is_refused() {
        let error = DataKind::Cursor
            .validate(r#"{"recordingArea":{"width":1,"height":1},"events":[],"meta":{}}"#)
            .unwrap_err();
        assert!(error.contains("no events"), "{error}");

        let error = DataKind::Subtitle
            .validate(r#"{"segments":[],"meta":{}}"#)
            .unwrap_err();
        assert!(error.contains("no segments"), "{error}");
    }

    #[test]
    fn malformed_json_reports_where_it_broke() {
        let error = DataKind::Subtitle.validate("{ not json").unwrap_err();
        assert!(error.starts_with("Invalid subtitle data"), "{error}");
    }

    #[test]
    fn each_kind_edits_its_own_sidecar() {
        let project = std::path::Path::new("/tmp/Take.poratake");
        assert!(DataKind::Cursor.path(project).ends_with("cursor.json"));
        assert!(DataKind::Subtitle.path(project).ends_with("subtitle.json"));
    }
}
