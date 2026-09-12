use std::rc::Rc;

use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Hsla, Render, SharedString, Styled, Window,
};
use herogpui::gpui;
use herogpui::{
    ColorArea, ColorChannel, ColorSlider, ColorSpace, ColorSwatchPicker, PickerColor, SizeXl,
    SwatchShape,
};

use crate::theme::color::Srgba;
use crate::theme::vars::active_theme;
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::menu::DismissHandler;
use herogpui::components::{Button, Size, Variant};

const POPOVER_WIDTH: f32 = 256.0;
const POPOVER_PAD: f32 = 12.0;
const AREA_HEIGHT: f32 = (POPOVER_WIDTH - POPOVER_PAD * 2.0) * 3.0 / 4.0;
const SHUFFLE_SIZE: f32 = 32.0;
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

    fn to_picker(self) -> PickerColor {
        PickerColor::hsb(self.hue, self.saturation, self.value)
    }

    fn from_picker(color: PickerColor) -> Self {
        Self {
            hue: color.hue,
            saturation: color.saturation,
            value: color.brightness,
        }
    }
}

pub fn hsv_from_hex(hex: &str) -> Hsv {
    let parsed = Srgba::parse(hex);
    Hsv::from_rgb(parsed.r, parsed.g, parsed.b)
}

pub struct ColorPickerPopover {
    hsv: Hsv,
    palette: Vec<SharedString>,
    swatch_opacity: f32,
    on_change: ColorHandler,
    on_dismiss: DismissHandler,
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

    fn apply(&mut self, color: PickerColor, window: &mut Window, cx: &mut Context<Self>) {
        self.hsv = Hsv::from_picker(color);
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let current = self.hsv.to_picker();
        let selected_hex = self.hsv.to_hex();
        let area_width = POPOVER_WIDTH - POPOVER_PAD * 2.0;
        let swatches = self
            .palette
            .iter()
            .filter_map(|entry| PickerColor::from_hex(entry))
            .collect::<Vec<_>>();
        let entity = cx.entity().downgrade();
        let area_entity = entity.clone();
        let hue_entity = entity.clone();
        let swatch_entity = entity;

        div()
            .id("color-picker-popover")
            .track_focus(&self.focus_handle)
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _event, window, cx| {
                let dismiss = this.on_dismiss.clone();
                dismiss(window, cx);
            }))
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
            .child(
                ColorSwatchPicker::new("color-swatches", swatches)
                    .size(SizeXl::Xs)
                    .shape(SwatchShape::Square)
                    .value(current)
                    .on_change(move |color, window, cx| {
                        if let Some(entity) = swatch_entity.upgrade() {
                            entity.update(cx, |this, cx| this.apply(*color, window, cx));
                        }
                    }),
            )
            .child(
                ColorArea::new("color-area", current)
                    .color_space(ColorSpace::Hsb)
                    .size(px(area_width), px(AREA_HEIGHT))
                    .sx(|el| el.rounded(px(chrome::RADIUS_2XL)))
                    .on_change(move |color, window, cx| {
                        if let Some(entity) = area_entity.upgrade() {
                            entity.update(cx, |this, cx| this.apply(*color, window, cx));
                        }
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        ColorSlider::new("color-hue", current, ColorChannel::Hue)
                            .show_label(false)
                            .length(px(area_width - SHUFFLE_SIZE - 8.0))
                            .on_change(move |color, window, cx| {
                                if let Some(entity) = hue_entity.upgrade() {
                                    entity.update(cx, |this, cx| this.apply(*color, window, cx));
                                }
                            }),
                    )
                    .child(
                        herogpui::components::Tooltip::new("Choose a random color").child(
                            Button::new("color-random")
                                .variant(Variant::Tertiary)
                                .size(Size::Sm)
                                .is_icon_only(true)
                                .sx(|el| el.rounded(px(9999.0)))
                                .child(icon_element("shuffle", px(chrome::TOOL_OPTION_CHEVRON)))
                                .on_press(cx.listener(|this, _event, window, cx| {
                                    this.randomize(window, cx)
                                })),
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
                            .bg(self.hsv.to_hsla(self.swatch_opacity)),
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

    #[test]
    fn picker_color_round_trips_hsv() {
        let hsv = hsv_from_hex("#3b82f6");
        let back = Hsv::from_picker(hsv.to_picker());
        assert!((back.hue - hsv.hue).abs() < 0.5);
        assert!((back.saturation - hsv.saturation).abs() < 0.01);
        assert!((back.value - hsv.value).abs() < 0.01);
    }
}
