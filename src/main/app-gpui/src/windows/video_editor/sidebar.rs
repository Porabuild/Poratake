use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled, Window};

use crate::config::shortcuts::VideoEditorSidebarShortcuts;
use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::windows::video_editor::VideoEditorWindow;

pub const TAB_RAIL_WIDTH: f32 = chrome::VIDEO_TAB_RAIL_WIDTH;
#[allow(dead_code)]
pub const PANEL_WIDTH: f32 = chrome::VIDEO_SIDEBAR_WIDTH;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SidebarTab {
    Cursor,
    Zoom,
    Drawing,
    Camera,
    Audio,
    Wallpaper,
    Keyboard,
    Subtitle,
    FirstFrame,
    Export,
}

impl SidebarTab {
    pub const ALL: [SidebarTab; 10] = [
        Self::Cursor,
        Self::Zoom,
        Self::Drawing,
        Self::Camera,
        Self::Audio,
        Self::Wallpaper,
        Self::Keyboard,
        Self::Subtitle,
        Self::FirstFrame,
        Self::Export,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::Zoom => "zoom",
            Self::Drawing => "drawing",
            Self::Camera => "camera",
            Self::Audio => "audio",
            Self::Wallpaper => "wallpaper",
            Self::Keyboard => "keyboard",
            Self::Subtitle => "subtitle",
            Self::FirstFrame => "first-frame",
            Self::Export => "export",
        }
    }

    pub fn parse(value: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|tab| tab.id() == value)
            .unwrap_or(Self::Cursor)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor",
            Self::Zoom => "Zoom",
            Self::Drawing => "Drawing",
            Self::Camera => "Camera",
            Self::Audio => "Audio",
            Self::Wallpaper => "Wallpaper",
            Self::Keyboard => "Keyboard",
            Self::Subtitle => "Subtitles",
            Self::FirstFrame => "First Frame",
            Self::Export => "Export",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Cursor => "mouse-pointer-2",
            Self::Zoom => "zoom-in",
            Self::Drawing => "pen-line",
            Self::Camera => "camera",
            Self::Audio => "volume-2",
            Self::Wallpaper => "wallpaper",
            Self::Keyboard => "keyboard",
            Self::Subtitle => "subtitles",
            Self::FirstFrame => "frame",
            Self::Export => "download",
        }
    }

    pub fn shortcut(self, shortcuts: &VideoEditorSidebarShortcuts) -> &str {
        match self {
            Self::Cursor => &shortcuts.cursor,
            Self::Zoom => &shortcuts.zoom,
            Self::Drawing => &shortcuts.drawing,
            Self::Camera => &shortcuts.camera,
            Self::Audio => &shortcuts.audio,
            Self::Wallpaper => &shortcuts.wallpaper,
            Self::Keyboard => &shortcuts.keyboard,
            Self::Subtitle => &shortcuts.subtitle,
            Self::FirstFrame => &shortcuts.first_frame,
            Self::Export => &shortcuts.export,
        }
    }
}

pub fn resize_handle(
    resizing: bool,
    theme: &ThemeVars,
    _window: &mut Window,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    div()
        .id("video-sidebar-resize")
        .w(px(chrome::VIDEO_SIDEBAR_RESIZE))
        .h_full()
        .flex_shrink_0()
        .flex()
        .justify_center()
        .cursor_ew_resize()
        .child(div().my_auto().h_full().w(px(1.0)).bg(if resizing {
            theme.accent
        } else {
            theme.border
        }))
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|this, event: &gpui::MouseDownEvent, _window, cx| {
                this.begin_sidebar_resize(f32::from(event.position.x), cx);
                cx.stop_propagation();
            }),
        )
        .into_any_element()
}

pub fn tab_rail(
    active: Option<SidebarTab>,
    shortcuts: &VideoEditorSidebarShortcuts,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let mut rail = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(chrome::TITLE_BAR_GAP))
        .h_full()
        .w(px(TAB_RAIL_WIDTH))
        .flex_shrink_0()
        .border_l_1()
        .border_color(theme.border)
        .bg(theme.card)
        .py(px(chrome::TITLE_BAR_PADDING_X));

    for tab in SidebarTab::ALL {
        let shortcut = tab.shortcut(shortcuts);
        let tooltip = if shortcut.is_empty() {
            tab.label().to_string()
        } else {
            format!("{} ({})", tab.label(), shortcut.to_uppercase())
        };
        rail = rail.child(
            Button::new(SharedString::from(format!("video-tab-{}", tab.id())))
                .variant(if active == Some(tab) {
                    ButtonVariant::Tertiary
                } else {
                    ButtonVariant::Ghost
                })
                .size(ButtonSize::IconSm)
                .icon(tab.icon())
                .tooltip(tooltip)
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.select_tab(tab, cx);
                })),
        );
    }

    rail.into_any_element()
}
