//! The all-in-one toolbar drawn on top of the area overlay — port of
//! `renderer/components/area-overlay/all-in-one-toolbar.tsx`.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled, Window};

use crate::capture::all_in_one::{Choices, Mode, Target};
use crate::capture::overlay::AreaOverlay;
use crate::system::capabilities::{is_supported, Feature};
use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::menu::{MenuBuilder, MenuHandle, MenuItem, MenuPlacement};

const TARGET_MENU_ID: &str = "all-in-one-target";

pub fn render(
    choices: Choices,
    menu: &MenuHandle,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut Context<AreaOverlay>,
) -> AnyElement {
    let recording_enabled = is_supported(Feature::Recording);
    let ocr_enabled = is_supported(Feature::Ocr);

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
        let active = choices.mode == mode;
        let id = format!("all-in-one-mode-{}", mode.id());
        let focus = crate::ui::primitives::control_focus(&id, false, window, cx);
        modes = modes.child(
            div()
                .id(SharedString::from(id))
                .track_focus(&focus)
                .focus(|style| style.shadow(crate::ui::primitives::focus_ring(theme, 2.0)))
                .size(px(chrome::OVERLAY_BUTTON_SIZE))
                .rounded(px(chrome::OVERLAY_BUTTON_RADIUS))
                .flex()
                .items_center()
                .justify_center()
                .when(active, |el| el.bg(theme.muted_foreground.opacity(0.25)))
                .text_color(if active {
                    theme.foreground
                } else {
                    theme.muted_foreground.opacity(0.6)
                })
                .hover(|style: gpui::StyleRefinement| style.text_color(theme.muted_foreground))
                .on_click(cx.listener(move |this, _event, window, cx| {
                    this.close_all_in_one_menu(window);
                    this.set_all_in_one_mode(mode, cx);
                }))
                .child(icon_element(mode.icon(), px(chrome::TOOL_BUTTON_ICON))),
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
                .child(modes)
                .when(choices.mode != Mode::Ocr, |el| {
                    el.child(target_menu(choices, menu, theme, window, cx))
                })
                .child(hairline(theme))
                .when(ocr_enabled, |el| {
                    el.child(
                        toolbar_button("all-in-one-ocr", "scan-text", "Capture text")
                            .selected(choices.mode == Mode::Ocr)
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.close_all_in_one_menu(window);
                                this.set_all_in_one_mode(Mode::Ocr, cx);
                            })),
                    )
                })
                .child(
                    toolbar_button("all-in-one-pick-color", "pipette", "Pick color").on_click(
                        cx.listener(|this, _event, window, cx| {
                            this.start_color_picker(window, cx);
                        }),
                    ),
                )
                .child(hairline(theme))
                .child(
                    toolbar_button("all-in-one-close", "x", "Close").on_click(cx.listener(
                        |_this, _event, window, cx| {
                            crate::capture::overlay::close_all(cx);
                            window.remove_window();
                        },
                    )),
                ),
        );
    bar = bar.on_mouse_down(gpui::MouseButton::Left, |_event, _window, cx| {
        cx.stop_propagation();
    });
    bar.into_any_element()
}

/// `toolbar-button.tsx`: `size-8 rounded-3xl hover:bg-white/15` with
/// `--button-fg: rgb(255 255 255 / 0.85)`. The overlay floats over the frozen
/// desktop, so its chrome is white on every theme rather than themed.
fn toolbar_button(id: &'static str, icon: &'static str, tooltip: &'static str) -> Button {
    Button::new(id)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::IconSm)
        .radius(px(chrome::OVERLAY_BUTTON_RADIUS))
        .foreground(crate::ui::colors::white(0.85))
        .surface_hover(crate::ui::colors::white(0.15))
        .icon(icon)
        .tooltip(tooltip)
}

fn hairline(theme: &ThemeVars) -> AnyElement {
    div()
        .mx(px(chrome::OVERLAY_HAIRLINE_INSET))
        .h(px(chrome::OVERLAY_HAIRLINE_HEIGHT))
        .w(px(1.0))
        .flex_none()
        .bg(theme.border.opacity(0.7))
        .into_any_element()
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
        .gap(px(4.0))
        .h(px(chrome::OVERLAY_BUTTON_SIZE))
        .w(px(chrome::OVERLAY_TARGET_TRIGGER_WIDTH))
        .min_w(px(chrome::OVERLAY_TARGET_TRIGGER_WIDTH))
        .px(px(chrome::OVERLAY_TARGET_TRIGGER_PAD_X))
        .rounded(px(chrome::OVERLAY_BUTTON_RADIUS))
        .text_color(crate::ui::colors::white(0.85))
        .hover(|style: gpui::StyleRefinement| {
            style
                .bg(crate::ui::colors::white(0.15))
                .text_color(crate::ui::colors::white(1.0))
        })
        .child(icon_element(
            choices.target.icon(),
            px(chrome::TOOL_BUTTON_ICON),
        ))
        .child(icon_element(
            "chevron-down",
            px(chrome::OVERLAY_TARGET_CHEVRON),
        ))
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
