//! The all-in-one toolbar drawn on top of the area overlay — port of
//! `renderer/components/area-overlay/all-in-one-toolbar.tsx`.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled, Window};
use herogpui::components::{TabItem, Tabs};
use herogpui::gpui;

use crate::capture::all_in_one::{Choices, Mode, Target};
use crate::capture::overlay::AreaOverlay;
use crate::system::capabilities::{is_supported, Feature};
use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::menu::{MenuBuilder, MenuHandle, MenuItem, MenuPlacement};
use crate::ui::toolbar;

const MODE_TABS_ID: &str = "all-in-one-mode";
const TARGET_MENU_ID: &str = "all-in-one-target";
const TARGET_MENU_MIN_WIDTH: f32 = 160.0;
const TARGET_ICON_GAP: f32 = 4.0;
const TARGET_ICON_GROUP_WIDTH: f32 =
    chrome::TOOL_BUTTON_ICON + TARGET_ICON_GAP + chrome::OVERLAY_TARGET_CHEVRON;

pub fn render(
    choices: Choices,
    menu: &MenuHandle,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut Context<AreaOverlay>,
) -> AnyElement {
    let recording_enabled = is_supported(Feature::Recording);
    let ocr_enabled = is_supported(Feature::Ocr);
    let color_picker_enabled = is_supported(Feature::ColorPicker);

    let mut items = Vec::new();
    for mode in [Mode::Screenshot, Mode::Record] {
        if mode == Mode::Record && !recording_enabled {
            continue;
        }
        items.push(
            TabItem::new(mode.id(), mode.label())
                .trigger(mode_trigger(mode, choices.mode == mode, theme))
                .width(px(chrome::OVERLAY_BUTTON_SIZE))
                .height(px(chrome::OVERLAY_BUTTON_SIZE))
                .padding_x(px(0.0)),
        );
    }
    let group_width = px(mode_group_width(items.len()));
    let selected = match choices.mode {
        Mode::Ocr => "",
        mode => mode.id(),
    };
    let modes = Tabs::new(MODE_TABS_ID, items, Mode::Screenshot.id())
        .selected_key(selected)
        .radius(px(chrome::OVERLAY_BUTTON_RADIUS))
        .list_bg(theme.muted_foreground.opacity(0.10))
        .indicator_bg(theme.muted_foreground.opacity(0.25))
        .indicator_shadow(false)
        .list_padding(px(0.0))
        .hover_fill(false)
        .sx(move |el| el.w(group_width).flex_none())
        .on_selection_change(cx.listener(move |this, key: &SharedString, window, cx| {
            this.close_all_in_one_menu(window, cx);
            this.set_all_in_one_mode(Mode::parse(key), cx);
        }));

    let mut bar = div()
        .absolute()
        .top(px(chrome::overlay_toolbar_top()))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            toolbar::surface(theme)
                .child(modes)
                .when(choices.mode != Mode::Ocr, |el| {
                    el.child(target_menu(choices, menu, theme, window, cx))
                })
                .child(toolbar::hairline(theme))
                .when(ocr_enabled, |el| {
                    el.child(toolbar_button(
                        "all-in-one-ocr",
                        Mode::Ocr.icon(),
                        Mode::Ocr.label(),
                        choices.mode == Mode::Ocr,
                        theme,
                        |this, window, cx| {
                            this.close_all_in_one_menu(window, cx);
                            this.set_all_in_one_mode(Mode::Ocr, cx);
                        },
                        cx,
                    ))
                })
                .when(color_picker_enabled, |el| {
                    el.child(toolbar_button(
                        "all-in-one-pick-color",
                        "pipette",
                        "Pick color",
                        false,
                        theme,
                        |this, window, cx| {
                            this.start_color_picker(window, cx);
                        },
                        cx,
                    ))
                })
                .child(toolbar::hairline(theme))
                .child(toolbar_button(
                    "all-in-one-close",
                    "x",
                    "Close",
                    false,
                    theme,
                    |_this, window, cx| {
                        crate::capture::overlay::dismiss(window, cx);
                    },
                    cx,
                )),
        );
    bar = bar.on_mouse_down(gpui::MouseButton::Left, |_event, _window, cx| {
        cx.stop_propagation();
    });
    bar.into_any_element()
}

fn mode_group_width(count: usize) -> f32 {
    count as f32 * chrome::OVERLAY_BUTTON_SIZE
}

fn mode_trigger(mode: Mode, active: bool, theme: &ThemeVars) -> AnyElement {
    let hovered = theme.muted_foreground;
    let resting = if active {
        theme.foreground
    } else {
        hovered.opacity(0.6)
    };
    div()
        .size(px(chrome::OVERLAY_BUTTON_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .text_color(resting)
        .when(!active, |el| {
            el.hover(move |style| style.text_color(hovered))
        })
        .child(icon_element(mode.icon(), px(chrome::TOOL_BUTTON_ICON)))
        .into_any_element()
}

fn toolbar_button(
    id: &'static str,
    icon: &'static str,
    tooltip: &'static str,
    selected: bool,
    theme: &ThemeVars,
    on_click: impl Fn(&mut AreaOverlay, &mut Window, &mut Context<AreaOverlay>) + 'static,
    cx: &mut Context<AreaOverlay>,
) -> AnyElement {
    toolbar::tooltip_button(
        toolbar::selected_icon(id, icon, selected, theme),
        tooltip,
        cx,
        on_click,
    )
}

fn target_menu_placement() -> MenuPlacement {
    MenuPlacement::below(TARGET_MENU_ID).min_width(px(TARGET_MENU_MIN_WIDTH))
}

fn target_menu(
    choices: Choices,
    menu: &MenuHandle,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut Context<AreaOverlay>,
) -> AnyElement {
    let handle = menu.clone();
    let entries = {
        let mut builder = MenuBuilder::new();
        for target in Target::ALL {
            if !target.is_supported() {
                continue;
            }
            builder = builder.item(
                MenuItem::new(target.label())
                    .icon(target.icon())
                    .trailing_check(choices.target == target)
                    .on_select({
                        let owner = cx.entity().downgrade();
                        move |_window, app| {
                            if let Some(owner) = owner.upgrade() {
                                owner.update(app, |this, cx| {
                                    this.set_all_in_one_target(target, cx);
                                });
                            }
                        }
                    }),
            );
        }
        builder.build()
    };

    let focus = crate::ui::primitives::control_focus(TARGET_MENU_ID, false, window, cx);
    let (trigger_hover, trigger_hovered) =
        crate::ui::primitives::hover_flag(TARGET_MENU_ID, window, cx);
    let key_handle = handle.clone();
    let key_entries = entries.clone();
    div()
        .id(TARGET_MENU_ID)
        .track_focus(&focus)
        .focus(|style| style.shadow(crate::ui::primitives::focus_ring(theme, 2.0)))
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .h(px(chrome::OVERLAY_BUTTON_SIZE))
        .w(px(chrome::OVERLAY_TARGET_TRIGGER_WIDTH))
        .min_w(px(chrome::OVERLAY_TARGET_TRIGGER_WIDTH))
        .px(px(chrome::OVERLAY_TARGET_TRIGGER_PAD_X))
        .rounded(px(chrome::OVERLAY_BUTTON_RADIUS))
        .text_color(if trigger_hovered {
            crate::ui::colors::white(1.0)
        } else {
            crate::ui::colors::white(0.85)
        })
        .when(trigger_hovered, |el| el.bg(crate::ui::colors::white(0.15)))
        .on_hover({
            let trigger_hover = trigger_hover.clone();
            move |over: &bool, _window, cx| {
                crate::ui::primitives::track_hover(&trigger_hover, *over, cx);
            }
        })
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .w(px(TARGET_ICON_GROUP_WIDTH))
                .gap(px(TARGET_ICON_GAP))
                .child(icon_element(
                    choices.target.icon(),
                    px(chrome::TOOL_BUTTON_ICON),
                ))
                .child(icon_element(
                    "chevron-down",
                    px(chrome::OVERLAY_TARGET_CHEVRON),
                )),
        )
        .child(menu.render_dropdown(TARGET_MENU_ID, cx))
        .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
            handle.toggle(target_menu_placement(), entries.clone(), window, cx);
            cx.stop_propagation();
        })
        .on_key_down(move |event, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                key_handle.toggle(target_menu_placement(), key_entries.clone(), window, cx);
                cx.stop_propagation();
            }
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_tabs_tile_the_tray_edge_to_edge() {
        assert_eq!(mode_group_width(1), chrome::OVERLAY_BUTTON_SIZE);
        assert_eq!(mode_group_width(2), 64.0);
        assert_eq!(
            chrome::overlay_bar_height(),
            chrome::OVERLAY_BUTTON_SIZE
                + chrome::OVERLAY_SURFACE_PADDING * 2.0
                + chrome::OVERLAY_BORDER_WIDTH * 2.0
        );
    }

    #[test]
    fn target_icon_group_has_a_fixed_centered_footprint() {
        assert_eq!(TARGET_ICON_GROUP_WIDTH, 32.0);
        assert_eq!(
            chrome::OVERLAY_TARGET_TRIGGER_WIDTH
                - chrome::OVERLAY_TARGET_TRIGGER_PAD_X * 2.0
                - TARGET_ICON_GROUP_WIDTH,
            4.0
        );
    }
}
