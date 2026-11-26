#[cfg(any(target_os = "macos", test, all(windows, feature = "parity")))]
use gpui::App;
use gpui::{
    div, prelude::*, px, AnyElement, Context, Div, ElementId, SharedString, Styled, Window,
};
use herogpui::components::{Button, Size, Tooltip, TooltipPlacement, Variant};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::icon::icon_element;

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

pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .variant(Variant::Ghost)
        .size(Size::Sm)
        .is_icon_only(true)
        .aria_label(label)
        .recipe("overlay")
}

pub fn desktop_selected(button: Button, selected: bool, theme: &ThemeVars) -> Button {
    let (surface, hover) = selected_surfaces(
        selected,
        theme.default,
        theme.default_hover,
        theme.foreground,
    );
    let resting = surface.unwrap_or_else(gpui::transparent_black);
    button.bg(resting).hover_bg(hover)
}

pub fn icon(
    id: impl Into<ElementId>,
    icon: &'static str,
    label: impl Into<SharedString>,
) -> Button {
    button(id, label).child(icon_element(icon, px(chrome::TOOL_BUTTON_ICON)))
}

pub fn selected_icon(
    id: impl Into<ElementId>,
    icon: &'static str,
    selected: bool,
    theme: &ThemeVars,
    label: impl Into<SharedString>,
) -> Button {
    desktop_selected(
        button(id, label).child(icon_element(icon, px(chrome::TOOL_BUTTON_ICON))),
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
    tooltip_button_at(button, TooltipPlacement::Top, tooltip, cx, on_click)
}

pub fn tooltip_button_at<V: 'static>(
    button: Button,
    placement: TooltipPlacement,
    tooltip: impl Into<SharedString>,
    cx: &mut Context<V>,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> AnyElement {
    button_with_tooltip(
        button,
        Tooltip::new(tooltip).placement(placement),
        cx,
        on_click,
    )
}

pub fn button_with_tooltip<V: 'static>(
    button: Button,
    tooltip: Tooltip,
    cx: &mut Context<V>,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> AnyElement {
    tooltip
        .sx(|el| el.flex_shrink_0())
        .child(
            button.on_press(cx.listener(move |this, _event, window, cx| {
                on_click(this, window, cx);
            })),
        )
        .into_any_element()
}

pub fn selected_surfaces(
    selected: bool,
    selected_surface: gpui::Hsla,
    selected_hover: gpui::Hsla,
    foreground: gpui::Hsla,
) -> (Option<gpui::Hsla>, gpui::Hsla) {
    if selected {
        return (Some(selected_surface), selected_hover);
    }
    (None, foreground.opacity(0.15))
}

pub const FILLED_GLYPH_SIZE: f32 = 14.0;

pub fn filled_glyph(color: gpui::Hsla, round: bool) -> AnyElement {
    let glyph = div().size(px(FILLED_GLYPH_SIZE)).bg(color);
    if round {
        return glyph.rounded_full().into_any_element();
    }
    glyph.rounded(px(chrome::RADIUS_SM)).into_any_element()
}

#[cfg(any(target_os = "macos", test, all(windows, feature = "parity")))]
pub fn filled_play(color: gpui::Hsla) -> AnyElement {
    const PLAY_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="white"><path d="M6 3 L20 12 L6 21 Z"/></svg>"#;

    gpui::canvas(
        |_, _, _| {},
        move |bounds: gpui::Bounds<gpui::Pixels>, _: (), window: &mut Window, cx: &mut App| {
            if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
                return;
            }
            let _ = window.paint_svg(
                bounds,
                "poratake-glyph:filled-play".into(),
                Some(PLAY_SVG),
                gpui::TransformationMatrix::unit(),
                color,
                cx,
            );
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
        let foreground = gpui::hsla(0.0, 0.0, 0.0, 1.0);
        assert_eq!(
            selected_surfaces(true, selected_surface, selected_hover, foreground),
            (Some(selected_surface), selected_hover)
        );
        assert_eq!(
            selected_surfaces(false, selected_surface, selected_hover, foreground),
            (None, foreground.opacity(0.15))
        );
    }
}
