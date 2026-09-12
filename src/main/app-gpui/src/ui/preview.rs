use gpui::{prelude::*, px, AnyElement, App, SharedString, Window};
use herogpui::components::{Button, Variant};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::icon_button;

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
        .sx(move |el| el.bg(surface).text_color(foreground))
        .hover_bg(hover_bg)
        .on_press(move |_event, window, cx| {
            on_click(window, cx);
            cx.stop_propagation();
        });
    icon_button::with_tooltip(tooltip, content(button))
}
