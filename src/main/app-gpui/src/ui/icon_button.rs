use gpui::{prelude::*, px, AnyElement, ElementId, SharedString};
use herogpui::components::{Button, Size, Variant};
use herogpui::gpui;

use crate::ui::chrome;
use crate::ui::icon::icon_element;

pub fn ghost_icon(id: impl Into<ElementId>, icon: &'static str, icon_size: f32) -> Button {
    Button::new(id)
        .variant(Variant::Ghost)
        .is_icon_only(true)
        .child(icon_element(icon, px(icon_size)))
}

pub fn compact(id: impl Into<ElementId>, icon: &'static str) -> Button {
    compact_sized(id, icon, chrome::TOOL_BUTTON_ICON)
}

pub fn compact_sm(id: impl Into<ElementId>, icon: &'static str) -> Button {
    compact_sized(id, icon, chrome::BUTTON_XS_ICON)
}

pub fn compact_muted(id: impl Into<ElementId>, icon: &'static str) -> Button {
    compact(id, icon).recipe("muted")
}

pub fn compact_sm_muted(id: impl Into<ElementId>, icon: &'static str) -> Button {
    compact_sm(id, icon).recipe("muted")
}

pub fn tertiary_md_icon(id: impl Into<ElementId>, icon: &'static str) -> Button {
    Button::new(id)
        .variant(Variant::Tertiary)
        .size(Size::Md)
        .is_icon_only(true)
        .child(icon_element(icon, px(chrome::TOOL_BUTTON_ICON)))
}

fn compact_sized(id: impl Into<ElementId>, icon: &'static str, icon_size: f32) -> Button {
    ghost_icon(id, icon, icon_size).recipe("compact-icon")
}

pub fn chip(id: impl Into<ElementId>, selected: bool) -> Button {
    Button::new(id)
        .variant(if selected {
            Variant::Secondary
        } else {
            Variant::Ghost
        })
        .recipe("chip")
}

pub fn chip_icon(id: impl Into<ElementId>, icon: &'static str) -> Button {
    ghost_icon(id, icon, chrome::HISTORY_TOOL_ICON)
        .recipe("chip-icon")
        .recipe("muted")
}

pub fn with_tooltip(tooltip: impl Into<SharedString>, child: impl IntoElement) -> AnyElement {
    herogpui::components::Tooltip::new(tooltip)
        .child(child)
        .into_any_element()
}
