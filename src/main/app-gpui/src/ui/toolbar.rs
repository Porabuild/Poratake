use gpui::{
    div, prelude::*, px, AnyElement, App, Context, Div, ElementId, SharedString, Styled, Window,
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

pub fn tooltip_button<V: 'static>(
    button: Button,
    tooltip: impl Into<SharedString>,
    cx: &mut Context<V>,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> AnyElement {
    icon_button::with_tooltip(
        tooltip,
        button.on_press(cx.listener(move |this, _event, window, cx| {
            on_click(this, window, cx);
        })),
    )
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

pub const FILLED_GLYPH_SIZE: f32 = 14.0;

pub fn filled_glyph(color: gpui::Hsla, round: bool) -> AnyElement {
    let glyph = div().size(px(FILLED_GLYPH_SIZE)).bg(color);
    if round {
        return glyph.rounded_full().into_any_element();
    }
    glyph.rounded(px(chrome::RADIUS_SM)).into_any_element()
}

pub fn filled_play(color: gpui::Hsla) -> AnyElement {
    gpui::canvas(
        |_, _, _| {},
        move |bounds: gpui::Bounds<gpui::Pixels>, _: (), window: &mut Window, _cx: &mut App| {
            let at = |x: f32, y: f32| {
                gpui::point(
                    bounds.origin.x + px(x * FILLED_GLYPH_SIZE),
                    bounds.origin.y + px(y * FILLED_GLYPH_SIZE),
                )
            };
            let mut builder = gpui::PathBuilder::fill();
            builder.move_to(at(0.25, 0.125));
            builder.line_to(at(0.8333, 0.5));
            builder.line_to(at(0.25, 0.875));
            builder.close();
            if let Ok(path) = builder.build() {
                window.paint_path(path, color);
            }
        },
    )
    .w(px(FILLED_GLYPH_SIZE))
    .h(px(FILLED_GLYPH_SIZE))
    .into_any_element()
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
