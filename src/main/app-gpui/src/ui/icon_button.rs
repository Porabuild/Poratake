use gpui::{prelude::*, px, AnyElement, ElementId, SharedString};
use herogpui::components::{Button, Size, Variant};
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
    ghost_icon(id, icon, icon_size, label).recipe("compact-icon")
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

pub fn chip_icon(
    id: impl Into<ElementId>,
    icon: &'static str,
    label: impl Into<SharedString>,
) -> Button {
    ghost_icon(id, icon, chrome::HISTORY_TOOL_ICON, label)
        .recipe("chip-icon")
        .recipe("muted")
}

pub fn with_tooltip(tooltip: impl Into<SharedString>, child: impl IntoElement) -> AnyElement {
    herogpui::components::Tooltip::new(tooltip)
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

    #[test]
    fn hinted_appends_the_hint_only_when_present() {
        assert_eq!(hinted("Undo", "Ctrl+Z"), "Undo (Ctrl+Z)");
        assert_eq!(hinted("Pin Screenshot", ""), "Pin Screenshot");
    }
}
