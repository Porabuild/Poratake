#[cfg(any(windows, target_os = "macos"))]
use gpui::Styled;
use gpui::{div, prelude::*, px, AnyElement, App, Context, SharedString, Window};
use herogpui::components::{Button, Tooltip, TooltipPlacement, Variant};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::icon::{icon_element, Icon};

pub trait NativeTooltipHost: Sized + 'static {
    #[cfg(any(windows, target_os = "macos"))]
    fn preview_tooltip_hover(
        &mut self,
        key: &gpui::ElementId,
        tooltip: &str,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    );

    #[cfg(any(windows, target_os = "macos"))]
    fn preview_tooltip_cancel(&mut self, key: &gpui::ElementId, window: &mut Window);
}

pub const FINISHED_BADGE: f32 = 40.0;
pub const FINISHED_CHECK: f32 = 20.0;
pub const FINISHED_CHECK_STROKE: f32 = 3.0;
pub const FINISHED_ZOOM_FROM: f32 = 0.5;
pub const FINISHED_ENTER_MS: u64 = 300;
pub const PROGRESS_INSET: f32 = 8.0;
pub const PROGRESS_BOTTOM: f32 = 40.0;
pub const PROGRESS_HEIGHT: f32 = 6.0;

pub fn finished_zoom(progress: f32) -> f32 {
    FINISHED_ZOOM_FROM + (1.0 - FINISHED_ZOOM_FROM) * progress.clamp(0.0, 1.0)
}

pub fn finished_badge(progress: f32, theme: &ThemeVars) -> AnyElement {
    let scale = finished_zoom(progress);
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.5))
        .child(
            div()
                .size(px(FINISHED_BADGE * scale))
                .rounded_full()
                .bg(theme.foreground)
                .text_color(theme.background)
                .flex()
                .items_center()
                .justify_center()
                .children(
                    Icon::with_size("check", px(FINISHED_CHECK * scale))
                        .map(|icon| icon.stroke_width(FINISHED_CHECK_STROKE)),
                ),
        )
        .into_any_element()
}

pub fn progress_bar(fraction: f32, theme: &ThemeVars) -> AnyElement {
    div()
        .absolute()
        .left(px(PROGRESS_INSET))
        .right(px(PROGRESS_INSET))
        .bottom(px(PROGRESS_BOTTOM))
        .child(
            div()
                .h(px(PROGRESS_HEIGHT))
                .w_full()
                .overflow_hidden()
                .rounded_full()
                .bg(theme.background.opacity(0.3))
                .child(
                    div()
                        .h_full()
                        .w(gpui::relative(fraction.clamp(0.0, 1.0)))
                        .rounded_full()
                        .bg(theme.primary),
                ),
        )
        .into_any_element()
}

pub fn circle(
    cx: &mut Context<impl NativeTooltipHost>,
    id: impl Into<gpui::ElementId>,
    icon: &'static str,
    busy: bool,
    tooltip: impl Into<SharedString>,
    placement: TooltipPlacement,
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
        Some(tooltip.into()),
        placement,
        cx,
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
    cx: &mut Context<impl NativeTooltipHost>,
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    tooltip: Option<SharedString>,
    placement: TooltipPlacement,
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
        placement,
        cx,
        move |button| button.label(label),
        on_click,
    )
}

fn chip<T: NativeTooltipHost>(
    id: impl Into<gpui::ElementId>,
    recipe: &'static str,
    icon_only: bool,
    theme: &ThemeVars,
    hover_bg: gpui::Hsla,
    disabled: bool,
    tooltip: Option<SharedString>,
    placement: TooltipPlacement,
    #[cfg(any(windows, target_os = "macos"))] cx: &mut Context<T>,
    #[cfg(not(any(windows, target_os = "macos")))] _: &mut Context<T>,
    content: impl FnOnce(Button) -> Button + 'static,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let id = id.into();
    let surface = theme.background.opacity(0.8);
    let foreground = theme.foreground;
    let button = Button::new(id.clone())
        .variant(Variant::Ghost)
        .is_icon_only(icon_only)
        .when_some(tooltip.clone().filter(|_| icon_only), |button, tooltip| {
            button.aria_label(tooltip)
        })
        .is_disabled(disabled)
        .recipe(recipe)
        .height(px(chrome::PREVIEW_PILL_HEIGHT))
        .padding_x(px(chrome::BUTTON_SM_PAD_X))
        .text_size(px(chrome::BUTTON_XS_TEXT))
        .when(icon_only, |button| {
            button.width(px(chrome::PREVIEW_CONTROL))
        })
        .bg(surface)
        .text_color(foreground)
        .when(disabled, |el| {
            el.cursor(gpui::CursorStyle::OperationNotAllowed)
        })
        .hover_bg(hover_bg)
        .on_press(move |_event, window, cx| {
            on_click(window, cx);
            cx.stop_propagation();
        });
    let button = content(button);
    match tooltip {
        Some(tooltip) => {
            let mut tip = Tooltip::new(tooltip.clone())
                .placement(placement)
                .sx(|el| el.flex_shrink_0());
            if !icon_only {
                let body_tooltip = tooltip.clone();
                tip = tip.body(move |_, _| {
                    div()
                        .max_w(px(chrome::PREVIEW_WIDTH - 20.))
                        .line_clamp(3)
                        .text_ellipsis()
                        .child(body_tooltip.clone())
                        .into_any_element()
                });
            }
            #[cfg(any(windows, target_os = "macos"))]
            {
                let hover_tooltip = tooltip.to_string();
                let hover_key = id.clone();
                let press_key = hover_key.clone();
                let trigger = div()
                    .id((id.clone(), "preview-native-tooltip"))
                    .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                        this.preview_tooltip_hover(
                            &hover_key,
                            &hover_tooltip,
                            *hovered,
                            window,
                            cx,
                        );
                    }))
                    .capture_any_mouse_down(cx.listener(move |this, _, window, _| {
                        this.preview_tooltip_cancel(&press_key, window);
                    }))
                    .child(button);
                tip = tip
                    .trigger(herogpui::components::TooltipTrigger::Focus)
                    .child(trigger);
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            let tip = tip.child(button);
            tip.into_any_element()
        }
        None => button.into_any_element(),
    }
}
