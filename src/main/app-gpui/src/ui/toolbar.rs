use gpui::{
    div, prelude::*, px, AnyElement, App, Div, ElementId, SharedString, Stateful, Styled, Window,
};
use herogpui::components::{Button, Size, Variant};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::icon_button;

pub fn surface(theme: &ThemeVars) -> Div {
    div()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(chrome::OVERLAY_SURFACE_GAP))
        .rounded(px(chrome::OVERLAY_SURFACE_RADIUS))
        .border_2()
        .border_color(theme.muted_foreground.opacity(0.35))
        .bg(theme.muted_background.opacity(0.95))
        .shadow_2xl()
        .p(px(chrome::OVERLAY_SURFACE_PADDING))
        .text_color(theme.foreground)
}

pub fn hairline(theme: &ThemeVars) -> AnyElement {
    div()
        .mx(px(chrome::OVERLAY_HAIRLINE_INSET))
        .h(px(chrome::OVERLAY_HAIRLINE_HEIGHT))
        .w(px(1.0))
        .flex_none()
        .bg(theme.border.opacity(0.7))
        .into_any_element()
}

pub fn button(id: impl Into<ElementId>) -> Button {
    Button::new(id)
        .variant(Variant::Ghost)
        .size(Size::Sm)
        .is_icon_only(true)
        .recipe("overlay")
}

pub fn desktop(button: Button) -> Button {
    let foreground = crate::ui::colors::white(0.85);
    button
        .sx(move |el| el.text_color(foreground))
        .hover_bg(crate::ui::colors::white(0.15))
}

pub fn desktop_selected(button: Button, selected: bool, theme: &ThemeVars) -> Button {
    let foreground = crate::ui::colors::white(0.85);
    let (surface, hover) = selected_surfaces(selected, theme.default, theme.default_hover);
    let resting = surface.unwrap_or_else(gpui::transparent_black);
    button
        .sx(move |el| el.bg(resting).text_color(foreground))
        .hover_bg(hover)
}

pub fn icon(id: impl Into<ElementId>, icon: &'static str) -> Button {
    desktop(button(id).child(icon_element(icon, px(chrome::TOOL_BUTTON_ICON))))
}

pub fn selected_icon(
    id: impl Into<ElementId>,
    icon: &'static str,
    selected: bool,
    theme: &ThemeVars,
) -> Button {
    desktop_selected(
        button(id).child(icon_element(icon, px(chrome::TOOL_BUTTON_ICON))),
        selected,
        theme,
    )
}

pub fn with_tooltip(tooltip: impl Into<SharedString>, child: impl IntoElement) -> AnyElement {
    icon_button::with_tooltip(tooltip, child)
}

pub fn selected_surfaces(
    selected: bool,
    selected_surface: gpui::Hsla,
    selected_hover: gpui::Hsla,
) -> (Option<gpui::Hsla>, gpui::Hsla) {
    if selected {
        return (Some(selected_surface), selected_hover);
    }
    (None, crate::ui::colors::white(0.15))
}

pub fn mode_tab(
    id: SharedString,
    icon: &'static str,
    active: bool,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let key = id.to_string();
    let focus = crate::ui::primitives::control_focus(&key, false, window, cx);
    let (hover, hovered) = crate::ui::primitives::hover_flag(&key, window, cx);
    let text = if hovered {
        theme.muted_foreground
    } else if active {
        theme.foreground
    } else {
        theme.muted_foreground.opacity(0.6)
    };
    div()
        .id(id)
        .track_focus(&focus)
        .focus(|style| style.shadow(crate::ui::primitives::focus_ring(theme, 2.0)))
        .size(px(chrome::OVERLAY_BUTTON_SIZE))
        .rounded(px(chrome::OVERLAY_BUTTON_RADIUS))
        .flex()
        .items_center()
        .justify_center()
        .when(active, |el| el.bg(theme.muted_foreground.opacity(0.25)))
        .text_color(text)
        .on_hover({
            let hover = hover.clone();
            move |over: &bool, _window, cx| {
                crate::ui::primitives::track_hover(&hover, *over, cx);
            }
        })
        .child(icon_element(icon, px(chrome::TOOL_BUTTON_ICON)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_toolbar_buttons_use_the_active_hover_surface() {
        let selected_surface = gpui::hsla(0.0, 0.0, 0.2, 1.0);
        let selected_hover = gpui::hsla(0.0, 0.0, 0.3, 1.0);
        assert_eq!(
            selected_surfaces(true, selected_surface, selected_hover),
            (Some(selected_surface), selected_hover)
        );
        assert_eq!(
            selected_surfaces(false, selected_surface, selected_hover),
            (None, crate::ui::colors::white(0.15))
        );
    }
}
