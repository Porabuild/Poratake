use gpui::{div, prelude::*, px, AnyElement, App, Hsla, SharedString, Styled};
use herogpui::gpui;

use crate::editor::options::{
    self, EditorHandlers, EditorOption, ARROW_STYLES, HIGHLIGHT_OPACITIES, REDACT_STYLES,
    SHAPE_FILL_MODES, THICKNESS_OPTIONS,
};
use crate::editor::toolbar_select;
use crate::theme::color::Srgba;
use crate::theme::vars::{active_theme, ThemeVars};
use crate::ui::colors::Tool;
use crate::ui::icon::{icon, icon_element};
use crate::ui::menu::{MenuBuilder, MenuEntry, MenuItem};

pub const ARROW_PREVIEW_STANDARD: &str = "M 20 12 L 4 12 M4 12 L8 8 M4 12 L8 16";
pub const ARROW_PREVIEW_CURVED: &str = "M20 6 Q12 6 8 18 M8 18 L12 14 M8 18 L4 14";
pub const ARROW_PREVIEW_DOUBLE: &str =
    "M 20 12 L 4 12 M4 12 L8 8 M4 12 L8 16 M20 12 L16 8 M20 12 L16 16";
pub const ARROW_PREVIEW_DOUBLE_CURVED: &str = "M11 19H5v-6 M13 5h6v6 M19 5 5 19";

pub fn arrow_preview_path(style: &str) -> &'static str {
    match style {
        "curved" => ARROW_PREVIEW_CURVED,
        "double" => ARROW_PREVIEW_DOUBLE,
        "double-curved" => ARROW_PREVIEW_DOUBLE_CURVED,
        _ => ARROW_PREVIEW_STANDARD,
    }
}

#[derive(Clone)]
pub struct ToolOptionsState {
    pub tool: Tool,
    pub color_hex: SharedString,
    pub stroke_width: f64,
    pub arrow_style: SharedString,
    pub highlight_opacity: f64,
    pub number_style: SharedString,
    pub number_size: SharedString,
    pub number_start_value: f64,
    pub text_background: bool,
    pub text_font_size: f64,
    pub text_font_family: SharedString,
    pub redact_style: SharedString,
    pub redact_intensity: f64,
    pub shape_fill_mode: SharedString,
    #[allow(dead_code)]
    pub wallpaper: crate::editor::wallpaper::WallpaperSettings,
    #[allow(dead_code)]
    pub has_layers: bool,
}

fn thickness_bar(width: f32, height: f32, color: Hsla) -> AnyElement {
    div()
        .w(px(width))
        .h(px(height))
        .rounded_full()
        .bg(color)
        .into_any_element()
}

/// `NumberBadgePreview` in `number-options.tsx`: a `<circle r="10">` inside a
/// 24-unit viewBox scaled to `size`, so the disc is 5/6 of the box, with an
/// 11-unit bold glyph.
pub(super) fn number_badge(glyph: &'static str, size: f32, theme: &ThemeVars) -> AnyElement {
    div()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .child(
            div()
                .size(px(size * 20.0 / 24.0))
                .rounded_full()
                .bg(theme.foreground)
                .text_color(theme.background)
                .text_size(px(size * 11.0 / 24.0))
                .font_weight(gpui::FontWeight::BOLD)
                .flex()
                .items_center()
                .justify_center()
                .child(glyph),
        )
        .into_any_element()
}

fn shape_fill_preview(filled: bool, color: Hsla, size: f32) -> AnyElement {
    div()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .child(
            div()
                .size(px(size * 14.0 / 16.0))
                .rounded(px(size * 2.0 / 16.0))
                .border_2()
                .border_color(color)
                .when(filled, |el| el.bg(color)),
        )
        .into_any_element()
}

fn separator(theme: &ThemeVars) -> AnyElement {
    crate::ui::primitives::chrome_tick(theme).into_any_element()
}

fn thickness_entries(state: &ToolOptionsState, handlers: &EditorHandlers) -> Vec<MenuEntry> {
    let mut builder = MenuBuilder::new();
    for (width, height) in THICKNESS_OPTIONS {
        builder = builder.item(
            MenuItem::new(format!("{width}px"))
                .trailing_check((width - state.stroke_width).abs() < 0.01)
                .leading(move |cx| {
                    let theme = active_theme(cx);
                    div()
                        .w(px(32.0))
                        .h(px(16.0))
                        .flex()
                        .items_center()
                        .child(thickness_bar(32.0, height, theme.muted_foreground))
                        .into_any_element()
                })
                .on_select(handlers.option(EditorOption::StrokeWidth(width))),
        );
    }
    builder.build()
}

fn arrow_entries(state: &ToolOptionsState, handlers: &EditorHandlers) -> Vec<MenuEntry> {
    let mut builder = MenuBuilder::new();
    for (value, label) in ARROW_STYLES {
        let path = arrow_preview_path(value);
        builder = builder.item(
            MenuItem::new(label)
                .trailing_check(state.arrow_style.as_ref() == value)
                .leading(move |_cx| icon(path).size(px(24.0)).into_any_element())
                .on_select(handlers.option(EditorOption::ArrowStyle(value.into()))),
        );
    }
    builder.build()
}

fn highlight_entries(state: &ToolOptionsState, handlers: &EditorHandlers) -> Vec<MenuEntry> {
    let mut builder = MenuBuilder::new();
    for level in HIGHLIGHT_OPACITIES {
        builder = builder.item(
            MenuItem::new(format!("{}%", (level * 100.0).round() as i32))
                .trailing_check((level - state.highlight_opacity).abs() < 0.001)
                .on_select(handlers.option(EditorOption::HighlightOpacity(level))),
        );
    }
    builder.build()
}

fn shape_entries(state: &ToolOptionsState, handlers: &EditorHandlers) -> Vec<MenuEntry> {
    let color = Srgba::parse(&state.color_hex).to_hsla();
    let mut builder = MenuBuilder::new();
    for (value, label) in SHAPE_FILL_MODES {
        let filled = value == "filled";
        builder = builder.item(
            MenuItem::new(label)
                .trailing_check(state.shape_fill_mode.as_ref() == value)
                .leading(move |_cx| shape_fill_preview(filled, color, 16.0))
                .on_select(handlers.option(EditorOption::ShapeFillMode(value.into()))),
        );
    }
    builder.build()
}

/// Mirrors `ToolOptions.tsx`: thickness first, then the per-tool popover, each
/// followed by a hairline separator.
pub fn render(
    state: &ToolOptionsState,
    handlers: &EditorHandlers,
    window: &mut gpui::Window,
    cx: &mut App,
) -> Vec<AnyElement> {
    let theme = active_theme(cx);
    let mut children: Vec<AnyElement> = Vec::new();

    if matches!(
        state.tool,
        Tool::Pen | Tool::Rectangle | Tool::Circle | Tool::Line | Tool::Arrow
    ) {
        let height = options::thickness_bar_height(state.stroke_width);
        let bar_color = theme.muted_foreground;
        children.push(toolbar_select::render(
            "tool-option-thickness",
            "Line thickness",
            thickness_entries(state, handlers),
            move || thickness_bar(16.0, height, bar_color),
            false,
            window,
            cx,
        ));
        children.push(separator(&theme));
    }

    if state.tool == Tool::Arrow {
        let path = arrow_preview_path(&state.arrow_style);
        children.push(toolbar_select::render(
            "tool-option-arrow",
            "Arrow style",
            arrow_entries(state, handlers),
            move || icon(path).size(px(20.0)).into_any_element(),
            true,
            window,
            cx,
        ));
        children.push(separator(&theme));
    }

    if state.tool == Tool::Highlight {
        children.push(toolbar_select::render(
            "tool-option-highlight",
            "Highlight opacity",
            highlight_entries(state, handlers),
            || icon_element("highlighter", px(16.0)),
            true,
            window,
            cx,
        ));
    }

    if state.tool == Tool::Number {
        let glyph = options::number_preview_glyph(&state.number_style);
        children.push(super::tool_popover::render(
            "tool-option-number",
            state,
            handlers,
            number_badge(glyph, 20.0, &theme),
            window,
            cx,
        ));
        children.push(separator(&theme));
    }

    if state.tool == Tool::Text {
        children.push(super::tool_popover::render(
            "tool-option-text",
            state,
            handlers,
            icon_element("type", px(16.0)),
            window,
            cx,
        ));
        children.push(separator(&theme));
    }

    if state.tool == Tool::Redact {
        let icon_name = REDACT_STYLES
            .iter()
            .find(|(value, _, _)| *value == state.redact_style.as_ref())
            .map(|(_, _, icon_name)| *icon_name)
            .unwrap_or("grid-3x3");
        children.push(super::tool_popover::render(
            "tool-option-redact",
            state,
            handlers,
            icon_element(icon_name, px(16.0)),
            window,
            cx,
        ));
    }

    if matches!(state.tool, Tool::Rectangle | Tool::Circle) {
        let color = Srgba::parse(&state.color_hex).to_hsla();
        let filled = state.shape_fill_mode.as_ref() == "filled";
        children.push(toolbar_select::render(
            "tool-option-shape",
            "Shape fill",
            shape_entries(state, handlers),
            move || shape_fill_preview(filled, color, 16.0),
            true,
            window,
            cx,
        ));
        children.push(separator(&theme));
    }

    children
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::chrome;

    fn panels_for(tool: Tool) -> &'static [&'static str] {
        match tool {
            Tool::Pen | Tool::Line => &["thickness"],
            Tool::Rectangle | Tool::Circle => &["thickness", "shape"],
            Tool::Arrow => &["thickness", "arrow"],
            Tool::Highlight => &["highlight"],
            Tool::Number => &["number"],
            Tool::Text => &["text"],
            Tool::Redact => &["redact"],
            Tool::Select | Tool::Crop | Tool::Wallpaper => &[],
        }
    }

    fn trailing_separator(panel: &str) -> bool {
        !matches!(panel, "highlight" | "redact")
    }

    #[test]
    fn toolbar_panels_match_electron() {
        assert_eq!(chrome::TOOL_OPTION_HEIGHT, 28.0);
        assert_eq!(chrome::TOOL_OPTION_RADIUS, chrome::RADIUS_3XL);
        assert_eq!(chrome::TOOL_OPTION_PAD_X, 8.0);
        assert_eq!(chrome::TOOL_OPTION_GAP, 8.0);
        assert_eq!(chrome::TOOL_OPTION_CHEVRON, 14.0);
        assert_eq!(panels_for(Tool::Pen), &["thickness"]);
        assert_eq!(panels_for(Tool::Arrow), &["thickness", "arrow"]);
        assert_eq!(panels_for(Tool::Rectangle), &["thickness", "shape"]);
        assert_eq!(panels_for(Tool::Highlight), &["highlight"]);
        assert_eq!(panels_for(Tool::Number), &["number"]);
        assert_eq!(panels_for(Tool::Text), &["text"]);
        assert_eq!(panels_for(Tool::Redact), &["redact"]);
        assert_eq!(panels_for(Tool::Wallpaper), &[] as &[&str]);
        assert_eq!(panels_for(Tool::Select), &[] as &[&str]);
        assert!(trailing_separator("thickness"));
        assert!(trailing_separator("arrow"));
        assert!(trailing_separator("shape"));
        assert!(trailing_separator("number"));
        assert!(trailing_separator("text"));
        assert!(!trailing_separator("highlight"));
        assert!(!trailing_separator("redact"));
    }
}

#[cfg(test)]
mod interaction_tests {
    use herogpui::gpui;
    use herogpui::test::TestWindowExt;
    use std::sync::Arc;

    #[herogpui::test]
    fn toolbar_select_commits_and_keeps_editor_shortcuts_outside_the_open_list(
        cx: &mut gpui::TestAppContext,
    ) {
        let scratch = tempfile::tempdir().unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(scratch.path().join("config.json")).unwrap(),
        );
        store.update(|config| config.editor.last_tool = "pen".into());
        cx.update(|cx| {
            crate::state::set_test_state(cx, store);
            crate::editor::actions::init_bindings(cx);
        });
        let path = scratch.path().join("capture.png");
        image::RgbaImage::from_pixel(600, 400, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .unwrap();
        let (editor, cx) = cx.add_window_view(|window, cx| {
            crate::editor::window::EditorWindow::from_file(path.to_str().unwrap(), window, cx)
        });
        cx.settle();
        let trigger = "select-trigger-Name(\"tool-option-thickness\")";
        cx.click(trigger);
        let content = cx.expect("tool-option-thickness-content-1");
        let indicator = cx.expect("tool-option-thickness-selected-indicator");
        assert!(
            content.right() + gpui::px(4.) <= indicator.left(),
            "{content:?} overlaps {indicator:?}"
        );
        cx.press("down enter");
        assert_eq!(editor.read_with(cx, |editor, _| editor.stroke_width), 8.0);
        cx.click(trigger);
        cx.press("h");
        assert_eq!(
            editor.read_with(cx, |editor, _| editor.tool),
            crate::ui::colors::Tool::Pen
        );
        cx.press("escape");
        let focus = editor.read_with(cx, |editor, _| editor.focus_handle.clone());
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.press("h");
        assert_eq!(
            editor.read_with(cx, |editor, _| editor.tool),
            crate::ui::colors::Tool::Highlight
        );
    }
}
