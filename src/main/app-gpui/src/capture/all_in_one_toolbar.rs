//! The all-in-one toolbar drawn on top of the area overlay — port of
//! `renderer/components/area-overlay/all-in-one-toolbar.tsx`.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled, Window};
use herogpui::gpui;

use crate::capture::all_in_one::{Choices, Mode, Target};
use crate::capture::overlay::AreaOverlay;
use crate::system::capabilities::{is_supported, Feature};
use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::menu::{MenuBuilder, MenuHandle, MenuItem, MenuPlacement};
use crate::ui::toolbar;

const TARGET_MENU_ID: &str = "all-in-one-target";
const TARGET_ICON_GAP: f32 = 4.0;
const TARGET_ICON_GROUP_WIDTH: f32 =
    chrome::TOOL_BUTTON_ICON + TARGET_ICON_GAP + chrome::OVERLAY_TARGET_CHEVRON;

pub fn render(
    choices: Choices,
    picking_color: bool,
    menu: &MenuHandle,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut Context<AreaOverlay>,
) -> AnyElement {
    let recording_enabled = is_supported(Feature::Recording);
    let ocr_enabled = is_supported(Feature::Ocr);
    let color_picker_enabled = is_supported(Feature::ColorPicker);

    let mut modes = div()
        .flex()
        .flex_row()
        .items_center()
        .rounded(px(chrome::OVERLAY_BUTTON_RADIUS))
        .bg(theme.muted_foreground.opacity(0.10));
    for mode in [Mode::Screenshot, Mode::Record] {
        if mode == Mode::Record && !recording_enabled {
            continue;
        }
        let active = mode_selected(choices, picking_color, mode);
        let id = SharedString::from(format!("all-in-one-mode-{}", mode.id()));
        modes = modes.child(
            toolbar::mode_tab(id, mode.icon(), active, theme, window, cx).on_click(cx.listener(
                move |this, _event, window, cx| {
                    this.close_all_in_one_menu(window);
                    this.set_all_in_one_mode(mode, cx);
                },
            )),
        );
    }

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
                .child(target_menu(choices, menu, theme, window, cx))
                .child(toolbar::hairline(theme))
                .when(ocr_enabled, |el| {
                    el.child(toolbar_button(
                        "all-in-one-ocr",
                        "scan-text",
                        "Capture text",
                        mode_selected(choices, picking_color, Mode::Ocr),
                        theme,
                        |this, window, cx| {
                            this.close_all_in_one_menu(window);
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
                        picking_color,
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

fn mode_selected(choices: Choices, picking_color: bool, mode: Mode) -> bool {
    !picking_color && choices.mode == mode
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
        .child(menu.render_dropdown(TARGET_MENU_ID))
        .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
            handle.toggle(
                MenuPlacement::below(TARGET_MENU_ID),
                entries.clone(),
                window,
                cx,
            );
            cx.stop_propagation();
        })
        .on_key_down(move |event, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                key_handle.toggle(
                    MenuPlacement::below(TARGET_MENU_ID),
                    key_entries.clone(),
                    window,
                    cx,
                );
                cx.stop_propagation();
            }
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn color_picker_replaces_the_selected_mode() {
        let choices = Choices::default();
        assert!(mode_selected(choices, false, Mode::Screenshot));
        assert!(!mode_selected(choices, true, Mode::Screenshot));
    }
}
