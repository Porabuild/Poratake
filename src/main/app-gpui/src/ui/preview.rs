use std::sync::Arc;

use gpui::{div, prelude::*, px, AnyElement, App, SharedString, Window};
use herogpui::components::{Button, Tooltip, TooltipPlacement, Variant};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;
use crate::ui::icon::{icon_element, Icon};
use crate::ui::{chrome, tooltip::NativeTooltipPresenter};

#[cfg(target_os = "macos")]
pub trait PreviewFocusRevealHost: Sized + 'static {
    fn keyboard_reveal_target(
        &self,
        window: &Window,
        button: gpui::Bounds<gpui::Pixels>,
    ) -> Option<(gpui::ScrollHandle, gpui::Bounds<gpui::Pixels>)>;
}

#[cfg(target_os = "macos")]
pub fn focus_reveal<T: PreviewFocusRevealHost>(
    id: impl Into<gpui::ElementId>,
    owner: gpui::WeakEntity<T>,
    button: AnyElement,
) -> AnyElement {
    PreviewFocusReveal {
        id: id.into(),
        owner,
        button,
    }
    .into_any_element()
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
        .when(chrome::is_macos(), |el| {
            el.rounded(px(chrome::PREVIEW_RADIUS - 1.0))
        })
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
    presenter: &Arc<NativeTooltipPresenter>,
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
        presenter,
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
    presenter: &Arc<NativeTooltipPresenter>,
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
        presenter,
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
    tooltip: Option<SharedString>,
    placement: TooltipPlacement,
    presenter: &Arc<NativeTooltipPresenter>,
    content: impl FnOnce(Button) -> Button + 'static,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let id = id.into();
    #[cfg(test)]
    let debug_selector = format!("{id}-tooltip-trigger");
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
                .id((id.clone(), "preview-tooltip"))
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
            let trigger = div().id((id.clone(), "preview-native-tooltip"));
            #[cfg(test)]
            let trigger = trigger.debug_selector(move || debug_selector.clone());
            presenter.wrap(id, tooltip, tip, trigger.child(button))
        }
        None => button.into_any_element(),
    }
}

#[cfg(target_os = "macos")]
#[derive(gpui::IntoElement)]
struct PreviewFocusReveal<T: PreviewFocusRevealHost> {
    id: gpui::ElementId,
    owner: gpui::WeakEntity<T>,
    button: AnyElement,
}

#[cfg(target_os = "macos")]
impl<T: PreviewFocusRevealHost> RenderOnce for PreviewFocusReveal<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state((self.id, "preview-focus-reveal"), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        let scope = focus.clone();
        div()
            .flex()
            .track_focus(&focus)
            .on_children_prepainted(move |bounds, window, cx| {
                if !scope.contains_focused(window, cx) {
                    return;
                }
                let Some(button) = bounds.first() else {
                    return;
                };
                let Some((scroll, target_bounds)) = self
                    .owner
                    .upgrade()
                    .and_then(|owner| owner.read(cx).keyboard_reveal_target(window, *button))
                else {
                    return;
                };
                let viewport = scroll.bounds();
                if viewport.size.width <= px(0.) || viewport.size.height <= px(0.) {
                    return;
                }
                let reveal_delta = |start: gpui::Pixels,
                                    end: gpui::Pixels,
                                    visible_start: gpui::Pixels,
                                    visible_end: gpui::Pixels| {
                    let end = end.min(start + visible_end - visible_start);
                    if start < visible_start {
                        visible_start - start
                    } else if end > visible_end {
                        visible_end - end
                    } else {
                        px(0.)
                    }
                };
                let current = scroll.offset();
                let max = scroll.max_offset();
                let x = reveal_delta(
                    target_bounds.left(),
                    target_bounds.right(),
                    viewport.left(),
                    viewport.right(),
                );
                let y = reveal_delta(
                    target_bounds.top(),
                    target_bounds.bottom(),
                    viewport.top(),
                    viewport.bottom(),
                );
                let target = gpui::point(
                    (current.x + x).max(-max.x).min(px(0.)),
                    (current.y + y).max(-max.y).min(px(0.)),
                );
                if target != current {
                    let scope = scope.clone();
                    window.defer(cx, move |window, cx| {
                        if scope.contains_focused(window, cx) {
                            scroll.set_offset(target);
                            window.refresh();
                        }
                    });
                }
            })
            .child(self.button)
    }
}
