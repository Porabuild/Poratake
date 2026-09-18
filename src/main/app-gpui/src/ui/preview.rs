use gpui::{div, prelude::*, px, AnyElement, App, SharedString, Window};
use herogpui::components::{Button, Variant};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::icon::{icon_element, Icon};
use crate::ui::icon_button;

pub const FINISHED_BADGE: f32 = 40.0;
pub const FINISHED_CHECK: f32 = 20.0;
pub const FINISHED_CHECK_STROKE: f32 = 3.0;
pub const FINISHED_ZOOM_FROM: f32 = 0.5;
pub const FINISHED_ENTER_MS: u64 = 300;
pub const PROGRESS_INSET: f32 = 8.0;
pub const PROGRESS_BOTTOM: f32 = 40.0;
pub const PROGRESS_HEIGHT: f32 = 6.0;

pub fn finished_zoom(progress: f32) -> f32 {
    FINISHED_ZOOM_FROM + (1.0 - FINISHED_ZOOM_FROM) * progress.clamp(0.0, 1.0)
}

pub fn finished_badge(progress: f32, theme: &ThemeVars) -> AnyElement {
    let scale = finished_zoom(progress);
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.5))
        .child(
            div()
                .size(px(FINISHED_BADGE * scale))
                .rounded_full()
                .bg(theme.foreground)
                .text_color(theme.background)
                .flex()
                .items_center()
                .justify_center()
                .children(
                    Icon::with_size("check", px(FINISHED_CHECK * scale))
                        .map(|icon| icon.stroke_width(FINISHED_CHECK_STROKE)),
                ),
        )
        .into_any_element()
}

pub fn progress_bar(fraction: f32, theme: &ThemeVars) -> AnyElement {
    div()
        .absolute()
        .left(px(PROGRESS_INSET))
        .right(px(PROGRESS_INSET))
        .bottom(px(PROGRESS_BOTTOM))
        .child(
            div()
                .h(px(PROGRESS_HEIGHT))
                .w_full()
                .overflow_hidden()
                .rounded_full()
                .bg(theme.background.opacity(0.3))
                .child(
                    div()
                        .h_full()
                        .w(gpui::relative(fraction.clamp(0.0, 1.0)))
                        .rounded_full()
                        .bg(theme.primary),
                ),
        )
        .into_any_element()
}

pub fn circle(
    id: impl Into<gpui::ElementId>,
    icon: &'static str,
    busy: bool,
    tooltip: impl Into<SharedString>,
    theme: &ThemeVars,
    hover_bg: gpui::Hsla,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    chip(
        id,
        "preview",
        true,
        theme,
        hover_bg,
        busy,
        tooltip,
        move |button| {
            if busy {
                button.child(crate::ui::icon::spinner_element(
                    gpui::ElementId::Name(format!("{icon}-spinner").into()),
                    px(chrome::BUTTON_XS_ICON),
                ))
            } else {
                button.child(icon_element(icon, px(chrome::BUTTON_XS_ICON)))
            }
        },
        on_click,
    )
}

pub fn pill(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    tooltip: impl Into<SharedString>,
    theme: &ThemeVars,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    chip(
        id,
        "preview-pill",
        false,
        theme,
        theme.primary,
        false,
        tooltip,
        move |button| button.label(label),
        on_click,
    )
}

fn chip(
    id: impl Into<gpui::ElementId>,
    recipe: &'static str,
    icon_only: bool,
    theme: &ThemeVars,
    hover_bg: gpui::Hsla,
    disabled: bool,
    tooltip: impl Into<SharedString>,
    content: impl FnOnce(Button) -> Button + 'static,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let surface = theme.background.opacity(0.8);
    let foreground = theme.foreground;
    let button = Button::new(id)
        .variant(Variant::Ghost)
        .is_icon_only(icon_only)
        .is_disabled(disabled)
        .recipe(recipe)
        .sx(move |el| {
            let el = el.bg(surface).text_color(foreground);
            match disabled {
                true => el.cursor(gpui::CursorStyle::OperationNotAllowed),
                false => el,
            }
        })
        .hover_bg(hover_bg)
        .on_press(move |_event, window, cx| {
            on_click(window, cx);
            cx.stop_propagation();
        });
    icon_button::with_tooltip(tooltip, content(button))
}
