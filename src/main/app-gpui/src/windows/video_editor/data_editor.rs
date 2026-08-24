//! Port of `cursor-data-editor-dialog.tsx` and `subtitle-data-editor-dialog.tsx`
//! — a JSON editor over one of the project's sidecars, with the same template
//! and example the renderer offers and the same validation before saving.

use std::path::PathBuf;

use gpui::{div, prelude::*, px, AnyElement, Context, Entity, SharedString, Styled};

use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::icon::icon_element;
use crate::ui::text_area::TextArea;
use crate::video::project;
use crate::video::sidecars::{CursorData, SubtitleData};
use crate::windows::video_editor::VideoEditorWindow;

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
                    {
                        "start": 0.0,
                        "end": 2.0,
                        "text": "Hello there",
                        "words": [
                            { "text": "Hello", "start": 0.0, "end": 1.0 },
                            { "text": "there", "start": 1.0, "end": 2.0 }
                        ]
                    }
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
                    { "start": 0.0, "end": duration.min(2.0), "text": "" }
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
    pub field: Entity<TextArea>,
    pub error: Option<SharedString>,
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
        let field = cx.new(|cx| {
            TextArea::new(initial, cx)
                .rows(16)
                .placeholder("Enter data as JSON\u{2026}")
        });
        Self {
            kind,
            field,
            error: None,
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

    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(crate::ui::colors::black(0.5))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .w(px(640.0))
                .max_h(px(620.0))
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
                        .child(div().text_size(px(12.0)).child(kind.label()))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(4.0))
                                .child(
                                    Button::new("data-editor-template")
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Xs)
                                        .label("Load Template")
                                        .on_click(cx.listener(|this, _event, _window, cx| {
                                            this.load_data_editor_template(cx)
                                        })),
                                )
                                .child(
                                    Button::new("data-editor-example")
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Xs)
                                        .label("Load Example")
                                        .on_click(cx.listener(|this, _event, _window, cx| {
                                            this.load_data_editor_example(cx)
                                        })),
                                ),
                        ),
                )
                .child(editor.field.clone())
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
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .label("Cancel")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.close_data_editor(cx)
                                })),
                        )
                        .child(
                            Button::new("data-editor-save")
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Sm)
                                .label("Save")
                                .on_click(cx.listener(|this, _event, _window, cx| {
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
