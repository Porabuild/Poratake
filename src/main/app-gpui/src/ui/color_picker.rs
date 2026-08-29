use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, linear_color_stop, linear_gradient, prelude::*, px, App, Bounds, Context, ElementId,
    FocusHandle, Hsla, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, SharedString, Styled,
    Window,
};

use crate::theme::color::Srgba;
use crate::theme::vars::active_theme;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::ui::colors::transparent;
use crate::ui::menu::DismissHandler;

/// `HeroColorPicker.Popover className="w-64 rounded-2xl! ... p-3!"`.
const POPOVER_WIDTH: f32 = 256.0;
const POPOVER_PAD: f32 = 12.0;
/// `<ColorArea className="aspect-4/3 max-w-none">` inside the padded popover.
const AREA_HEIGHT: f32 = (POPOVER_WIDTH - POPOVER_PAD * 2.0) * 3.0 / 4.0;
/// `.color-area__thumb { size-4 rounded-xl border-3 border-white }`.
const AREA_THUMB: f32 = 16.0;
/// `.color-slider__track { h-5 }` with a `size-4` thumb.
const HUE_TRACK_HEIGHT: f32 = 20.0;
const HUE_THUMB: f32 = 16.0;
/// `<ColorSwatchPicker size="xs">`: `size-4 rounded-lg border`.
const SWATCH_SIZE: f32 = 16.0;
/// `.color-swatch-picker__item[data-selected] .swatch { scale(0.77) }`.
const SWATCH_SELECTED_INSET: f32 = SWATCH_SIZE * (1.0 - 0.77) / 2.0;
/// `.color-input-group { h-9 }` with `rounded-xl!` and an `ms-3` prefix.
const HEX_ROW_HEIGHT: f32 = 36.0;

pub type ColorHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
}

impl Hsv {
    pub fn from_rgb(r: f32, g: f32, b: f32) -> Self {
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let hue = if delta <= f32::EPSILON {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };

        Self {
            hue: if hue < 0.0 { hue + 360.0 } else { hue },
            saturation: if max <= f32::EPSILON {
                0.0
            } else {
                delta / max
            },
            value: max,
        }
    }

    pub fn to_rgb(self) -> (f32, f32, f32) {
        let chroma = self.value * self.saturation;
        let sector = (self.hue / 60.0).rem_euclid(6.0);
        let secondary = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
        let (r, g, b) = match sector as u32 {
            0 => (chroma, secondary, 0.0),
            1 => (secondary, chroma, 0.0),
            2 => (0.0, chroma, secondary),
            3 => (0.0, secondary, chroma),
            4 => (secondary, 0.0, chroma),
            _ => (chroma, 0.0, secondary),
        };
        let offset = self.value - chroma;
        (r + offset, g + offset, b + offset)
    }

    pub fn to_hex(self) -> String {
        let (r, g, b) = self.to_rgb();
        format!(
            "#{:02x}{:02x}{:02x}",
            (r * 255.0).round().clamp(0.0, 255.0) as u8,
            (g * 255.0).round().clamp(0.0, 255.0) as u8,
            (b * 255.0).round().clamp(0.0, 255.0) as u8
        )
    }

    pub fn to_hsla(self, alpha: f32) -> Hsla {
        let (r, g, b) = self.to_rgb();
        Srgba { r, g, b, a: alpha }.to_hsla()
    }
}

pub fn hsv_from_hex(hex: &str) -> Hsv {
    let parsed = Srgba::parse(hex);
    Hsv::from_rgb(parsed.r, parsed.g, parsed.b)
}

#[derive(Clone, Copy, PartialEq)]
enum Dragging {
    None,
    Area,
    Hue,
}

pub struct ColorPickerPopover {
    hsv: Hsv,
    palette: Vec<SharedString>,
    swatch_opacity: f32,
    on_change: ColorHandler,
    on_dismiss: DismissHandler,
    dragging: Dragging,
    area_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
    hue_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
    focus_handle: FocusHandle,
}

impl ColorPickerPopover {
    pub fn new(
        color: &str,
        palette: Vec<SharedString>,
        swatch_opacity: f32,
        on_change: ColorHandler,
        on_dismiss: DismissHandler,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            hsv: hsv_from_hex(color),
            palette,
            swatch_opacity,
            on_change,
            on_dismiss,
            dragging: Dragging::None,
            area_bounds: Rc::new(RefCell::new(None)),
            hue_bounds: Rc::new(RefCell::new(None)),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    fn emit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let hex = SharedString::from(self.hsv.to_hex());
        let handler = self.on_change.clone();
        handler(hex, window, cx);
        cx.notify();
    }

    fn apply_area(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bounds) = *self.area_bounds.borrow() else {
            return;
        };
        let width = f32::from(bounds.size.width).max(1.0);
        let height = f32::from(bounds.size.height).max(1.0);
        self.hsv.saturation = (f32::from(position.x - bounds.left()) / width).clamp(0.0, 1.0);
        self.hsv.value = 1.0 - (f32::from(position.y - bounds.top()) / height).clamp(0.0, 1.0);
        self.emit(window, cx);
    }

    fn apply_hue(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bounds) = *self.hue_bounds.borrow() else {
            return;
        };
        let width = f32::from(bounds.size.width).max(1.0);
        self.hsv.hue = (f32::from(position.x - bounds.left()) / width).clamp(0.0, 1.0) * 360.0;
        self.emit(window, cx);
    }

    fn randomize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.subsec_nanos())
            .unwrap_or(0);
        self.hsv = Hsv {
            hue: (seed % 360) as f32,
            saturation: 0.55 + ((seed / 360) % 45) as f32 / 100.0,
            value: 0.6 + ((seed / 16_200) % 40) as f32 / 100.0,
        };
        self.emit(window, cx);
    }
}

impl Render for ColorPickerPopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let current = self.hsv;
        let hue_color = Hsv {
            hue: current.hue,
            saturation: 1.0,
            value: 1.0,
        }
        .to_hsla(1.0);
        let selected_hex = current.to_hex();

        let mut swatches = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(4.0));
        for (index, entry) in self.palette.iter().enumerate() {
            let color = Srgba::parse(entry).to_hsla();
            let value = entry.clone();
            let active = entry.eq_ignore_ascii_case(&selected_hex);
            // Gated hover flag instead of a `.hover()` style, which gpui
            // paints against the window's last mouse position and so survives
            // the pointer leaving the window.
            let swatch_key = format!("color-swatch-fill-{index}");
            let (swatch_hover, swatch_hovered) =
                crate::ui::primitives::hover_flag(&swatch_key, window, cx);
            swatches = swatches.child(
                // A selected item borders itself in its own colour and shrinks
                // the swatch inside it, which reads as a ring with a gap.
                div()
                    .id(ElementId::NamedInteger("color-swatch".into(), index as u64))
                    .size(px(SWATCH_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(chrome::RADIUS_LG))
                    .border_1()
                    .border_color(if active { color } else { transparent() })
                    .when(active, |el| el.p(px(SWATCH_SELECTED_INSET)))
                    .child(
                        div()
                            .id(SharedString::from(swatch_key))
                            .size_full()
                            .rounded(px(chrome::RADIUS_LG))
                            .bg(color)
                            .when(swatch_hovered, |el| el.opacity(0.85))
                            .on_hover({
                                let swatch_hover = swatch_hover.clone();
                                move |over: &bool, _window, cx| {
                                    crate::ui::primitives::track_hover(&swatch_hover, *over, cx);
                                }
                            }),
                    )
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.hsv = hsv_from_hex(&value);
                        this.emit(window, cx);
                    })),
            );
        }

        let area_bounds = self.area_bounds.clone();
        let hue_bounds = self.hue_bounds.clone();

        div()
            .id("color-picker-popover")
            .track_focus(&self.focus_handle)
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _event, window, cx| {
                let dismiss = this.on_dismiss.clone();
                dismiss(window, cx);
            }))
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|this, _event, _window, _cx| this.dragging = Dragging::None),
            )
            .on_mouse_move(
                cx.listener(
                    move |this, event: &MouseMoveEvent, window, cx| match this.dragging {
                        Dragging::Area => this.apply_area(event.position, window, cx),
                        Dragging::Hue => this.apply_hue(event.position, window, cx),
                        Dragging::None => {}
                    },
                ),
            )
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _event, _window, _cx| this.dragging = Dragging::None),
            )
            .flex()
            .flex_col()
            .gap(px(12.0))
            .w(px(POPOVER_WIDTH))
            .min_w(px(POPOVER_WIDTH))
            .flex_shrink_0()
            .rounded(px(chrome::RADIUS_2XL))
            .border_1()
            .border_color(theme.border)
            .bg(theme.overlay)
            .shadow_xl()
            .p(px(POPOVER_PAD))
            .child(swatches)
            .child(
                div()
                    .id("color-area")
                    .relative()
                    .w_full()
                    .h(px(AREA_HEIGHT))
                    .rounded(px(chrome::RADIUS_2XL))
                    .overflow_hidden()
                    .bg(hue_color)
                    .child(div().absolute().inset_0().bg(linear_gradient(
                        90.0,
                        linear_color_stop(crate::ui::colors::white(1.0), 0.0),
                        linear_color_stop(crate::ui::colors::white(0.0), 1.0),
                    )))
                    .child(div().absolute().inset_0().bg(linear_gradient(
                        180.0,
                        linear_color_stop(crate::ui::colors::black(0.0), 0.0),
                        linear_color_stop(crate::ui::colors::black(1.0), 1.0),
                    )))
                    .child(
                        div()
                            .absolute()
                            .left(px((POPOVER_WIDTH - POPOVER_PAD * 2.0) * current.saturation
                                - AREA_THUMB / 2.0))
                            .top(px(AREA_HEIGHT * (1.0 - current.value) - AREA_THUMB / 2.0))
                            .size(px(AREA_THUMB))
                            .rounded(px(chrome::RADIUS_XL))
                            .border_3()
                            .border_color(crate::ui::colors::white(1.0)),
                    )
                    .child(
                        gpui::canvas(
                            move |bounds, _window, _cx| {
                                *area_bounds.borrow_mut() = Some(bounds);
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.dragging = Dragging::Area;
                            this.apply_area(event.position, window, cx);
                        }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .id("color-hue")
                            .relative()
                            .flex_1()
                            .h(px(HUE_TRACK_HEIGHT))
                            .rounded(px(chrome::RADIUS_2XL))
                            .overflow_hidden()
                            .bg(hue_gradient())
                            .child(
                                div()
                                    .absolute()
                                    .top(px((HUE_TRACK_HEIGHT - HUE_THUMB) / 2.0))
                                    .left(gpui::relative((current.hue / 360.0).clamp(0.0, 1.0)))
                                    .size(px(HUE_THUMB))
                                    .ml(px(-HUE_THUMB / 2.0))
                                    .rounded(px(chrome::RADIUS_2XL))
                                    .border_3()
                                    .border_color(crate::ui::colors::white(1.0))
                                    .bg(hue_color),
                            )
                            .child(
                                gpui::canvas(
                                    move |bounds, _window, _cx| {
                                        *hue_bounds.borrow_mut() = Some(bounds);
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .inset_0(),
                            )
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                    this.dragging = Dragging::Hue;
                                    this.apply_hue(event.position, window, cx);
                                }),
                            ),
                    )
                    .child(
                        Button::new("color-random")
                            .variant(ButtonVariant::Tertiary)
                            .size(ButtonSize::IconSm)
                            .radius(px(9999.0))
                            .icon("shuffle")
                            .icon_size(px(chrome::TOOL_OPTION_CHEVRON))
                            .tooltip("Choose a random color")
                            .on_click(
                                cx.listener(|this, _event, window, cx| this.randomize(window, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .h(px(HEX_ROW_HEIGHT))
                    .rounded(px(chrome::RADIUS_XL))
                    .bg(theme.default)
                    .px(px(chrome::FIELD_PAD_X))
                    .child(
                        div()
                            .size(px(chrome::COLOR_SWATCH_XS))
                            .rounded(px(chrome::RADIUS_LG))
                            .bg(current.to_hsla(self.swatch_opacity)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(chrome::FIELD_TEXT))
                            .text_color(theme.field_foreground)
                            .child(selected_hex.to_uppercase()),
                    ),
            )
    }
}

fn hue_gradient() -> gpui::Background {
    linear_gradient(
        90.0,
        linear_color_stop(gpui::hsla(0.0, 1.0, 0.5, 1.0), 0.0),
        linear_color_stop(gpui::hsla(1.0, 1.0, 0.5, 1.0), 1.0),
    )
}

/// The trigger pill that opens the picker, matching the renderer's
/// `HeroColorPicker.Trigger`.
pub fn trigger(
    id: &'static str,
    color: &str,
    opacity: f32,
    open: bool,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let theme = active_theme(cx);
    let swatch = Srgba::parse(color).to_hsla().opacity(opacity);
    // Gated hover flag instead of a `.hover()` style, which gpui paints
    // against the window's last mouse position and so survives the pointer
    // leaving the window.
    let (hover, hovered) = crate::ui::primitives::hover_flag(id, window, cx);
    div()
        .id(id)
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(chrome::TOOL_OPTION_GAP))
        .h(px(chrome::TOOL_OPTION_HEIGHT))
        .rounded(px(chrome::TOOL_OPTION_RADIUS))
        .px(px(chrome::TOOL_OPTION_PAD_X))
        .flex_shrink_0()
        .bg(if open || hovered {
            theme.default_hover
        } else {
            theme.default
        })
        .on_hover({
            let hover = hover.clone();
            move |over: &bool, _window, cx| {
                crate::ui::primitives::track_hover(&hover, *over, cx);
            }
        })
        .child(
            div()
                .size(px(chrome::COLOR_SWATCH_XS))
                .rounded(px(4.0))
                .bg(swatch),
        )
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child(crate::ui::icon::chevron_element(
                    px(chrome::TOOL_OPTION_CHEVRON),
                    open,
                )),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(hex: &str) {
        let hsv = hsv_from_hex(hex);
        assert_eq!(hsv.to_hex(), hex.to_ascii_lowercase(), "round trip {hex}");
    }

    #[test]
    fn converts_between_hex_and_hsv() {
        round_trip("#ff3b30");
        round_trip("#000000");
        round_trip("#ffffff");
        round_trip("#3b82f6");
    }

    #[test]
    fn derives_hue_saturation_and_value() {
        let red = hsv_from_hex("#ff0000");
        assert!((red.hue - 0.0).abs() < 0.01);
        assert!((red.saturation - 1.0).abs() < 0.01);
        assert!((red.value - 1.0).abs() < 0.01);

        let grey = hsv_from_hex("#808080");
        assert!(grey.saturation < 0.01);
    }
}
