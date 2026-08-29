use gpui::{div, prelude::*, px, AnyElement, App, Hsla, SharedString, Styled};

use crate::editor::options::{
    self, EditorHandlers, EditorOption, ARROW_STYLES, FONT_FAMILIES, FONT_SIZES,
    HIGHLIGHT_OPACITIES, NUMBER_SIZES, NUMBER_START_VALUES, NUMBER_STYLES, REDACT_INTENSITIES,
    REDACT_STYLES, SHAPE_FILL_MODES, THICKNESS_OPTIONS,
};
use crate::theme::color::Srgba;
use crate::theme::vars::{active_theme, ThemeVars};
use crate::ui::chrome;
use crate::ui::colors::Tool;
use crate::ui::icon::{chevron_element, icon, icon_element};
use crate::ui::menu::{MenuBuilder, MenuEntry, MenuHandle, MenuItem, MenuPlacement};

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
fn number_badge(glyph: &'static str, size: f32, theme: &ThemeVars) -> AnyElement {
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

/// A `Popover.Trigger` option pill: `h-7 gap-2 rounded-3xl bg-default px-2`
/// with an in-flow `size-3.5` chevron that rotates while the popover is open.
fn trigger(
    id: &'static str,
    menu: &MenuHandle,
    theme: &ThemeVars,
    content: AnyElement,
    entries: Vec<MenuEntry>,
    window: &mut gpui::Window,
    cx: &mut App,
) -> AnyElement {
    let open = menu.is_open_for(id);
    trigger_base(id, menu, theme, entries, window, cx)
        .gap(px(chrome::TOOL_OPTION_GAP))
        .px(px(chrome::TOOL_OPTION_PAD_X))
        .child(content)
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child(chevron_element(px(chrome::TOOL_OPTION_CHEVRON), open)),
        )
        .child(menu.render_dropdown(id))
        .into_any_element()
}

/// A `Select.Trigger` option pill. HeroUI pads the content with `pe-7` and
/// absolutely positions a `size-4` indicator at `end-2`, tinted
/// `--field-placeholder`, so the trigger is wider than the popover-backed one
/// and the chevron sits closer to the edge.
fn select_trigger(
    id: &'static str,
    menu: &MenuHandle,
    theme: &ThemeVars,
    content: AnyElement,
    entries: Vec<MenuEntry>,
    window: &mut gpui::Window,
    cx: &mut App,
) -> AnyElement {
    let open = menu.is_open_for(id);
    trigger_base(id, menu, theme, entries, window, cx)
        .pl(px(chrome::TOOL_OPTION_PAD_X))
        .pr(px(chrome::SELECT_INDICATOR_PAD_END))
        .child(content)
        .child(
            div()
                .absolute()
                .right(px(chrome::SELECT_INDICATOR_INSET))
                .top_0()
                .bottom_0()
                .flex()
                .items_center()
                .text_color(theme.field_placeholder)
                .child(chevron_element(px(chrome::SELECT_INDICATOR_SIZE), open)),
        )
        .child(menu.render_dropdown(id))
        .into_any_element()
}

fn trigger_base(
    id: &'static str,
    menu: &MenuHandle,
    theme: &ThemeVars,
    entries: Vec<MenuEntry>,
    window: &mut gpui::Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let handle = menu.clone();
    // Gated hover flag instead of a `.hover()` style, which gpui paints
    // against the window's last mouse position and so survives the pointer
    // leaving the window.
    let (hover, hovered) = crate::ui::primitives::hover_flag(id, window, cx);
    div()
        .id(id)
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .h(px(chrome::TOOL_OPTION_HEIGHT))
        .rounded(px(chrome::TOOL_OPTION_RADIUS))
        .flex_shrink_0()
        .bg(if hovered {
            theme.default_hover
        } else {
            theme.default
        })
        .on_hover({
            let hover = hover.clone();
            move |over: &bool, _window, cx| {
                crate::ui::primitives::track_hover(&hover, *over, cx);
            }
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
            handle.toggle(MenuPlacement::below(id), entries.clone(), window, cx);
            cx.stop_propagation();
        })
}

/// `<div className="mx-1 h-[18px] w-px bg-border" />` — `--border`, not
/// `--separator`.
fn separator(theme: &ThemeVars) -> AnyElement {
    crate::ui::primitives::Separator::vertical(px(chrome::SEPARATOR_HEIGHT))
        .inset(px(chrome::SEPARATOR_INSET))
        .color(theme.border)
        .into_any_element()
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

fn number_entries(state: &ToolOptionsState, handlers: &EditorHandlers) -> Vec<MenuEntry> {
    let mut builder = MenuBuilder::new();
    for (value, label) in NUMBER_STYLES {
        let glyph = options::number_preview_glyph(value);
        builder = builder.item(
            MenuItem::new(label)
                .trailing_check(state.number_style.as_ref() == value)
                .leading(move |cx| number_badge(glyph, 20.0, &active_theme(cx)))
                .on_select(handlers.option(EditorOption::NumberStyle(value.into()))),
        );
    }

    let mut start_values = MenuBuilder::new();
    for value in NUMBER_START_VALUES {
        start_values = start_values.item(
            MenuItem::new(options::number_display_value(value, &state.number_style))
                .trailing_check((value - state.number_start_value).abs() < 0.01)
                .on_select(handlers.option(EditorOption::NumberStartValue(value))),
        );
    }

    let mut sizes = MenuBuilder::new();
    for (value, label) in NUMBER_SIZES {
        sizes = sizes.item(
            MenuItem::new(label)
                .trailing_check(state.number_size.as_ref() == value)
                .on_select(handlers.option(EditorOption::NumberSize(value.into()))),
        );
    }

    builder
        .separator()
        .item(
            MenuItem::new(options::number_display_value(
                state.number_start_value,
                &state.number_style,
            ))
            .row("Starting:")
            .submenu(start_values.build()),
        )
        .separator()
        .item(
            MenuItem::new(options::label_for(&NUMBER_SIZES, &state.number_size))
                .row("Size:")
                .submenu(sizes.build()),
        )
        .build()
}

fn text_entries(state: &ToolOptionsState, handlers: &EditorHandlers) -> Vec<MenuEntry> {
    let mut builder = MenuBuilder::new();
    for (value, label) in FONT_FAMILIES {
        builder = builder.item(
            MenuItem::new(label)
                .trailing_check(state.text_font_family.as_ref() == value)
                .on_select(handlers.option(EditorOption::TextFontFamily(value.into()))),
        );
    }

    let mut sizes = MenuBuilder::new();
    for size in FONT_SIZES {
        sizes = sizes.item(
            MenuItem::new(format!("{}", size as i32))
                .trailing_check((size - state.text_font_size).abs() < 0.01)
                .on_select(handlers.option(EditorOption::TextFontSize(size))),
        );
    }

    let background = state.text_background;
    builder
        .separator()
        .item(
            MenuItem::new(format!("{}", state.text_font_size as i32))
                .row("Size:")
                .submenu(sizes.build()),
        )
        .separator()
        .item(
            MenuItem::new("")
                .row("Background:")
                .trailing_switch(background)
                .on_select(handlers.option(EditorOption::TextBackground(!background))),
        )
        .build()
}

fn redact_entries(state: &ToolOptionsState, handlers: &EditorHandlers) -> Vec<MenuEntry> {
    let mut builder = MenuBuilder::new();
    for (value, label, icon_name) in REDACT_STYLES {
        builder = builder.item(
            MenuItem::new(label)
                .icon(icon_name)
                .trailing_check(state.redact_style.as_ref() == value)
                .on_select(handlers.option(EditorOption::RedactStyle(value.into()))),
        );
    }

    if state.redact_style.as_ref() == "blackout" {
        return builder.build();
    }

    let mut intensities = MenuBuilder::new();
    for level in REDACT_INTENSITIES {
        intensities = intensities.item(
            MenuItem::new(format!("{}", level as i32))
                .trailing_check((level - state.redact_intensity).abs() < 0.01)
                .on_select(handlers.option(EditorOption::RedactIntensity(level))),
        );
    }

    builder
        .separator()
        .item(
            MenuItem::new(format!("{}", state.redact_intensity as i32))
                .row("Intensity:")
                .submenu(intensities.build()),
        )
        .build()
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
    menu: &MenuHandle,
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
        children.push(select_trigger(
            "tool-option-thickness",
            menu,
            &theme,
            thickness_bar(16.0, height, bar_color),
            thickness_entries(state, handlers),
            window,
            cx,
        ));
        children.push(separator(&theme));
    }

    if state.tool == Tool::Arrow {
        let path = arrow_preview_path(&state.arrow_style);
        children.push(select_trigger(
            "tool-option-arrow",
            menu,
            &theme,
            icon(path).size(px(20.0)).into_any_element(),
            arrow_entries(state, handlers),
            window,
            cx,
        ));
        children.push(separator(&theme));
    }

    if state.tool == Tool::Highlight {
        children.push(select_trigger(
            "tool-option-highlight",
            menu,
            &theme,
            icon_element("highlighter", px(16.0)),
            highlight_entries(state, handlers),
            window,
            cx,
        ));
    }

    if state.tool == Tool::Number {
        let glyph = options::number_preview_glyph(&state.number_style);
        children.push(trigger(
            "tool-option-number",
            menu,
            &theme,
            number_badge(glyph, 20.0, &theme),
            number_entries(state, handlers),
            window,
            cx,
        ));
        children.push(separator(&theme));
    }

    if state.tool == Tool::Text {
        children.push(trigger(
            "tool-option-text",
            menu,
            &theme,
            icon_element("type", px(16.0)),
            text_entries(state, handlers),
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
        children.push(trigger(
            "tool-option-redact",
            menu,
            &theme,
            icon_element(icon_name, px(16.0)),
            redact_entries(state, handlers),
            window,
            cx,
        ));
    }

    if matches!(state.tool, Tool::Rectangle | Tool::Circle) {
        let color = Srgba::parse(&state.color_hex).to_hsla();
        let filled = state.shape_fill_mode.as_ref() == "filled";
        children.push(select_trigger(
            "tool-option-shape",
            menu,
            &theme,
            shape_fill_preview(filled, color, 16.0),
            shape_entries(state, handlers),
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
