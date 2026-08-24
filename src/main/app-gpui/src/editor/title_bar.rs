//! Editor title bar — port of `renderer/components/title-bar.tsx`,
//! `editor/toolbar.tsx`, `editor/tool-options.tsx` and `editor/undo-redo.tsx`.
//!
//! Layout (Windows): tools | sep | crop·wallpaper·capture | sep |
//! undo·redo | copy·save·cloud·pin | sep | contextual options · color picker.

use gpui::{div, prelude::*, px, AnyElement, RenderOnce, SharedString, Styled};

use crate::config::shortcuts::EditorShortcuts;
use crate::editor::options::{EditorAction, EditorHandlers, EditorOption};
use crate::editor::tool_options::{self, ToolOptionsState};
use crate::system::accelerator;
use crate::theme::vars::active_theme;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::ui::color_picker;
use crate::ui::colors::Tool;
use crate::ui::menu::MenuHandle;

pub const TITLE_BAR_HEIGHT: f32 = chrome::TITLE_BAR_HEIGHT;

const TOOLS: [Tool; 10] = [
    Tool::Pen,
    Tool::Highlight,
    Tool::Rectangle,
    Tool::Circle,
    Tool::Line,
    Tool::Arrow,
    Tool::Text,
    Tool::Number,
    Tool::Redact,
    Tool::Select,
];

fn tool_shortcut(shortcuts: &EditorShortcuts, tool: Tool) -> &str {
    match tool {
        Tool::Pen => &shortcuts.pen,
        Tool::Highlight => &shortcuts.highlight,
        Tool::Rectangle => &shortcuts.rectangle,
        Tool::Circle => &shortcuts.circle,
        Tool::Line => &shortcuts.line,
        Tool::Arrow => &shortcuts.arrow,
        Tool::Text => &shortcuts.text,
        Tool::Number => &shortcuts.number,
        Tool::Redact => &shortcuts.redact,
        Tool::Select => &shortcuts.select,
        Tool::Crop => &shortcuts.crop,
        Tool::Wallpaper => &shortcuts.wallpaper,
    }
}

/// The editor's rules are plain `mx-1 h-[18px] w-px bg-border` divs, not the
/// HeroUI `<Separator>` component, so they take `--border` rather than
/// `--separator`.
fn separator(theme: &crate::theme::vars::ThemeVars) -> crate::ui::primitives::Separator {
    crate::ui::primitives::Separator::vertical(px(chrome::SEPARATOR_HEIGHT))
        .inset(px(chrome::SEPARATOR_INSET))
        .color(theme.border)
}

fn tool_button(tool: Tool, shortcut: &str, active: bool, handlers: &EditorHandlers) -> AnyElement {
    let name = tool.label();
    let select = handlers.option(EditorOption::Tool(tool));
    let tooltip = if shortcut.is_empty() {
        name.to_string()
    } else {
        format!("{name} ({})", shortcut.to_uppercase())
    };
    Button::new(tool.id())
        .variant(if active {
            ButtonVariant::Tertiary
        } else {
            ButtonVariant::Ghost
        })
        .size(ButtonSize::IconXs)
        .icon(tool.icon())
        // The renderer puts an explicit `size-4` on every tool glyph.
        .icon_size(px(chrome::TOOL_BUTTON_ICON))
        .tooltip(tooltip)
        .on_click(move |_event, window, cx| select(window, cx))
        .into_any_element()
}

fn action_button(
    id: &'static str,
    icon: &'static str,
    tooltip: String,
    action: EditorAction,
    handlers: &EditorHandlers,
    disabled: bool,
    size: ButtonSize,
) -> AnyElement {
    action_button_spinning(id, icon, tooltip, action, handlers, disabled, size, false)
}

#[allow(clippy::too_many_arguments)]
fn action_button_spinning(
    id: &'static str,
    icon: &'static str,
    tooltip: String,
    action: EditorAction,
    handlers: &EditorHandlers,
    disabled: bool,
    size: ButtonSize,
    spinning: bool,
) -> AnyElement {
    let run = handlers.action(action);
    Button::new(id)
        .variant(ButtonVariant::Ghost)
        .size(size)
        .icon(icon)
        .icon_size(px(chrome::TOOL_BUTTON_ICON))
        .icon_spinning(spinning)
        .tooltip(tooltip)
        .disabled(disabled)
        .on_click(move |_event, window, cx| run(window, cx))
        .into_any_element()
}

#[derive(IntoElement)]
pub struct TitleBar {
    pub options: ToolOptionsState,
    pub highlight_color: SharedString,
    pub shortcuts: EditorShortcuts,
    pub cloud_upload_shortcut: SharedString,
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_copied: bool,
    pub is_uploading: bool,
    pub is_upload_done: bool,
    pub is_capture_mode: bool,
    pub menu: MenuHandle,
    pub handlers: EditorHandlers,
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let theme = active_theme(cx);
        let handlers = self.handlers.clone();
        let state = self.options.clone();

        let mut tools = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(chrome::TITLE_BAR_GAP));

        for tool in TOOLS {
            tools = tools.child(tool_button(
                tool,
                tool_shortcut(&self.shortcuts, tool),
                state.tool == tool,
                &handlers,
            ));
        }

        tools = tools
            .child(separator(&theme))
            .child(tool_button(
                Tool::Crop,
                &self.shortcuts.crop,
                state.tool == Tool::Crop,
                &handlers,
            ))
            .child(tool_button(
                Tool::Wallpaper,
                &self.shortcuts.wallpaper,
                state.tool == Tool::Wallpaper,
                &handlers,
            ));

        {
            let capture = handlers.action(EditorAction::CaptureToggle);
            tools = tools.child(
                Button::new("tool-capture")
                    .variant(if self.is_capture_mode {
                        ButtonVariant::Tertiary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .size(ButtonSize::IconXs)
                    .icon("camera")
                    .icon_size(px(chrome::TOOL_BUTTON_ICON))
                    .tooltip(format!(
                        "Capture & Attach (hold {} for edge picker)",
                        accelerator::primary_modifier_label()
                    ))
                    .on_click(move |_event, window, cx| capture(window, cx))
                    .into_any_element(),
            );
        }

        tools = tools
            .child(separator(&theme))
            .child(action_button(
                "undo",
                "rotate-ccw",
                format!("Undo ({})", accelerator::display("CommandOrControl+Z")),
                EditorAction::Undo,
                &handlers,
                !self.can_undo,
                ButtonSize::IconXs,
            ))
            .child(action_button(
                "redo",
                "rotate-cw",
                format!(
                    "Redo ({})",
                    accelerator::display("CommandOrControl+Shift+Z")
                ),
                EditorAction::Redo,
                &handlers,
                !self.can_redo,
                ButtonSize::IconXs,
            ))
            .child(action_button(
                "action-copy",
                if self.is_copied { "check" } else { "copy" },
                format!("Copy ({})", accelerator::display("CommandOrControl+C")),
                EditorAction::Copy,
                &handlers,
                false,
                ButtonSize::IconSm,
            ))
            .child(action_button(
                "action-save",
                "save",
                format!("Save ({})", accelerator::display("CommandOrControl+S")),
                EditorAction::Save,
                &handlers,
                false,
                ButtonSize::IconSm,
            ))
            .child(action_button_spinning(
                "action-cloud",
                if self.is_uploading {
                    "loader-2"
                } else if self.is_upload_done {
                    "check"
                } else {
                    "cloud-upload"
                },
                match accelerator::display(&self.cloud_upload_shortcut) {
                    hint if hint.is_empty() => "Upload to Cloud".to_string(),
                    hint => format!("Upload to Cloud ({hint})"),
                },
                EditorAction::CloudUpload,
                &handlers,
                self.is_uploading,
                ButtonSize::IconSm,
                self.is_uploading,
            ))
            .child(action_button(
                "action-pin",
                "pin",
                "Pin Screenshot".to_string(),
                EditorAction::Pin,
                &handlers,
                false,
                ButtonSize::IconSm,
            ));

        let mut options = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(chrome::TITLE_BAR_GAP));
        for control in tool_options::render(&state, &self.menu, &handlers, cx) {
            options = options.child(control);
        }
        let is_highlight = state.tool == Tool::Highlight;
        let picker_color = if is_highlight {
            self.highlight_color.clone()
        } else {
            state.color_hex.clone()
        };
        let picker_opacity = if is_highlight {
            state.highlight_opacity as f32
        } else {
            1.0
        };
        options = options.child(color_trigger(
            picker_color,
            picker_opacity,
            is_highlight,
            &self.menu,
            &handlers,
            cx,
        ));

        let macos = chrome::is_macos();
        let cluster = if macos {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(chrome::TITLE_BAR_GAP))
                .child(options)
                .child(separator(&theme))
                .child(tools)
        } else {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(chrome::TITLE_BAR_GAP))
                .child(tools)
                .child(separator(&theme))
                .child(options)
        };

        let drag = div()
            .id("editor-title-drag")
            .flex_1()
            .h_full()
            .window_control_area(gpui::WindowControlArea::Drag);

        let mut bar = div()
            .id("editor-title-bar")
            .flex()
            .flex_row()
            .items_center()
            .h(px(TITLE_BAR_HEIGHT))
            .w_full()
            .flex_none()
            .overflow_x_hidden()
            .bg(theme.card)
            .px(px(chrome::TITLE_BAR_PADDING_X));
        if macos {
            bar = bar
                .child(crate::ui::window_controls::leading_inset())
                .child(drag)
                .child(cluster);
        } else {
            bar = bar
                .child(cluster)
                .child(drag)
                .child(crate::ui::window_controls::render(window, &theme));
        }
        bar
    }
}

const COLOR_PICKER_ID: &str = "editor-color-picker";

fn color_trigger(
    color: SharedString,
    opacity: f32,
    is_highlight: bool,
    menu: &MenuHandle,
    handlers: &EditorHandlers,
    cx: &mut gpui::App,
) -> AnyElement {
    let open = menu.is_open_for(COLOR_PICKER_ID);
    let handle = menu.clone();
    let palette: Vec<SharedString> = crate::ui::colors::palette_for_tool(if is_highlight {
        Tool::Highlight
    } else {
        Tool::Pen
    })
    .iter()
    .map(|value| SharedString::from(*value))
    .collect();
    let on_option = handlers.on_option.clone();
    let current = color.clone();

    color_picker::trigger(COLOR_PICKER_ID, &color, opacity, open, cx)
        .child(menu.render_dropdown(COLOR_PICKER_ID))
        .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
            let palette = palette.clone();
            let current = current.clone();
            let on_option = on_option.clone();
            handle.toggle_with(
                crate::ui::menu::MenuPlacement::below(COLOR_PICKER_ID),
                move |dismiss, cx| {
                    let handler: color_picker::ColorHandler = std::rc::Rc::new(
                        move |value: SharedString,
                              window: &mut gpui::Window,
                              cx: &mut gpui::App| {
                            let option = if is_highlight {
                                EditorOption::HighlightColor(value)
                            } else {
                                EditorOption::Color(value)
                            };
                            on_option(option, window, cx);
                        },
                    );
                    let view = cx.new(|cx| {
                        color_picker::ColorPickerPopover::new(
                            &current, palette, opacity, handler, dismiss, cx,
                        )
                    });
                    let focus = view.read(cx).focus_handle();
                    (view.into(), Some(focus))
                },
                window,
                cx,
            );
            cx.stop_propagation();
        })
        .into_any_element()
}
