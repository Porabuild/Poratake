use gpui::{prelude::*, px, AnyElement, ElementId, SharedString};
use herogpui::components::{Button, Size, Tooltip, TooltipPlacement, Variant};
use herogpui::gpui;

use crate::ui::chrome;
use crate::ui::icon::icon_element;

pub fn ghost_icon(
    id: impl Into<ElementId>,
    icon: &'static str,
    icon_size: f32,
    label: impl Into<SharedString>,
) -> Button {
    Button::new(id)
        .variant(Variant::Ghost)
        .is_icon_only(true)
        .aria_label(label)
        .child(icon_element(icon, px(icon_size)))
}

pub fn compact(
    id: impl Into<ElementId>,
    icon: &'static str,
    label: impl Into<SharedString>,
) -> Button {
    compact_sized(id, icon, chrome::TOOL_BUTTON_ICON, label)
}

pub fn compact_sm(
    id: impl Into<ElementId>,
    icon: &'static str,
    label: impl Into<SharedString>,
) -> Button {
    compact_sized(id, icon, chrome::BUTTON_XS_ICON, label)
}

pub fn compact_muted(
    id: impl Into<ElementId>,
    icon: &'static str,
    label: impl Into<SharedString>,
) -> Button {
    compact(id, icon, label).recipe("muted")
}

pub fn compact_sm_muted(
    id: impl Into<ElementId>,
    icon: &'static str,
    label: impl Into<SharedString>,
) -> Button {
    compact_sm(id, icon, label).recipe("muted")
}

pub fn tertiary_md_icon(
    id: impl Into<ElementId>,
    icon: &'static str,
    label: impl Into<SharedString>,
) -> Button {
    Button::new(id)
        .variant(Variant::Tertiary)
        .size(Size::Md)
        .is_icon_only(true)
        .aria_label(label)
        .child(icon_element(icon, px(chrome::TOOL_BUTTON_ICON)))
}

fn compact_sized(
    id: impl Into<ElementId>,
    icon: &'static str,
    icon_size: f32,
    label: impl Into<SharedString>,
) -> Button {
    ghost_icon(id, icon, icon_size, label)
        .size(Size::Sm)
        .width(px(chrome::TOOL_OPTION_HEIGHT))
        .height(px(chrome::TOOL_OPTION_HEIGHT))
}

pub fn chip(id: impl Into<ElementId>, selected: bool) -> Button {
    Button::new(id)
        .size(Size::Sm)
        .height(px(chrome::HISTORY_CHIP_HEIGHT))
        .padding_x(px(chrome::HISTORY_CHIP_PAD_X))
        .text_size(px(chrome::BUTTON_XS_TEXT))
        .variant(if selected {
            Variant::Secondary
        } else {
            Variant::Ghost
        })
}

pub fn chip_icon(
    id: impl Into<ElementId>,
    icon: &'static str,
    label: impl Into<SharedString>,
) -> Button {
    ghost_icon(id, icon, chrome::HISTORY_TOOL_ICON, label)
        .size(Size::Sm)
        .width(px(chrome::HISTORY_CHIP_HEIGHT))
        .height(px(chrome::HISTORY_CHIP_HEIGHT))
        .recipe("muted")
}

pub fn with_tooltip(tooltip: impl Into<SharedString>, child: impl IntoElement) -> AnyElement {
    Tooltip::new(tooltip)
        .sx(|el| el.flex_shrink_0())
        .child(child)
        .into_any_element()
}

pub fn with_tooltip_below(tooltip: impl Into<SharedString>, child: impl IntoElement) -> AnyElement {
    Tooltip::new(tooltip)
        .placement(TooltipPlacement::Bottom)
        .sx(|el| el.flex_shrink_0())
        .child(child)
        .into_any_element()
}

pub fn hinted(label: &str, hint: &str) -> String {
    if hint.is_empty() {
        return label.to_string();
    }
    format!("{label} ({hint})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{div, point, Context, Modifiers, Render, Window};
    use herogpui::test::TestWindowExt;

    struct Header;

    impl Render for Header {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            crate::ui::window_root::focus_root(
                div().flex().pl(px(100.0)).child(with_tooltip_below(
                    "Header button",
                    compact("header-button", "mouse-pointer-2", "Header button"),
                )),
                window,
                cx,
            )
        }
    }

    #[herogpui::test]
    fn header_tooltips_paint_inside_the_window(cx: &mut gpui::TestAppContext) {
        let (_view, cx) = herogpui::test::open_window(cx, |_, _| Header);
        cx.update(|_, cx| {
            herogpui::theme::set_theme(
                crate::theme::bridge::to_herogpui(&crate::theme::vars::ThemeVars::default(), true),
                cx,
            );
        });
        cx.simulate_mouse_move(point(px(114.0), px(14.0)), None, Modifiers::none());
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(150));
        cx.settle();
        let tips = cx.update(|window, _| {
            let scale = window.scale_factor();
            window
                .painted_quads()
                .into_iter()
                .filter(|quad| quad.bounds.size.width.0 / scale > 50.0)
                .collect::<Vec<_>>()
        });
        assert!(!tips.is_empty(), "the tooltip must be visible");
        for tip in tips {
            assert!(
                tip.bounds.origin.y.0 >= 0.0,
                "the tooltip must fit below the header"
            );
        }
    }

    #[test]
    fn hinted_appends_the_hint_only_when_present() {
        assert_eq!(hinted("Undo", "Ctrl+Z"), "Undo (Ctrl+Z)");
        assert_eq!(hinted("Pin Screenshot", ""), "Pin Screenshot");
    }
}
