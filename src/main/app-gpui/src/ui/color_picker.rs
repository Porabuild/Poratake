use std::rc::Rc;

use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, Render, SharedString, Window,
};
use herogpui::components::{Button, InputState, Size, Variant};
use herogpui::gpui;
use herogpui::{
    ColorArea, ColorChannel, ColorField, ColorSlider, ColorSpace, ColorSwatchPicker, FieldVariant,
    PickerColor, SizeXl, SwatchShape,
};

use crate::theme::color::Srgba;
use crate::theme::vars::active_theme;
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::menu::DismissHandler;

const POPOVER_WIDTH: f32 = 256.0;
const POPOVER_PAD: f32 = 12.0;
const AREA_HEIGHT: f32 = (POPOVER_WIDTH - POPOVER_PAD * 2.0) * 3.0 / 4.0;
const SHUFFLE_SIZE: f32 = 32.0;
const TRIGGER_WIDTH: f32 = chrome::TOOL_OPTION_PAD_X * 2.0
    + chrome::COLOR_SWATCH_XS
    + chrome::TOOL_OPTION_GAP
    + chrome::TOOL_OPTION_CHEVRON;
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

pub fn random_hex(seed: u64) -> String {
    let mut state = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    state = (state ^ (state >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    state ^= state >> 31;
    format!(
        "#{:02x}{:02x}{:02x}",
        (state >> 40) as u8,
        (state >> 24) as u8,
        (state >> 8) as u8
    )
}

pub fn hsv_from_hex(hex: &str) -> Hsv {
    let parsed = Srgba::parse(hex);
    Hsv::from_rgb(parsed.r, parsed.g, parsed.b)
}

pub struct ColorPickerPopover {
    hsv: Hsv,
    palette: Vec<SharedString>,
    on_change: ColorHandler,
    on_dismiss: DismissHandler,
    focus_handle: FocusHandle,
    hex_field: Entity<InputState>,
}

impl ColorPickerPopover {
    pub fn new(
        color: &str,
        palette: Vec<SharedString>,
        on_change: ColorHandler,
        on_dismiss: DismissHandler,
        cx: &mut Context<Self>,
    ) -> Self {
        let hsv = hsv_from_hex(color);
        let hex = hsv.to_hex().to_ascii_uppercase();
        Self {
            hsv,
            palette,
            on_change,
            on_dismiss,
            focus_handle: cx.focus_handle(),
            hex_field: cx.new(|cx| InputState::with_value(cx, hex)),
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    fn sync_hex_field(&self, cx: &mut App) {
        let hex = self.hsv.to_hex().to_ascii_uppercase();
        self.hex_field.update(cx, |state, cx| {
            if state.value() == hex {
                return;
            }
            state.set_value(hex);
            cx.notify();
        });
    }

    fn notify_change(&mut self, hex: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let handler = self.on_change.clone();
        handler(hex, window, cx);
        cx.notify();
    }

    fn emit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.hex_field.read(cx).focus_handle(cx).is_focused(window) {
            self.sync_hex_field(cx);
        }
        self.notify_change(SharedString::from(self.hsv.to_hex()), window, cx);
    }

    fn apply(&mut self, color: PickerColor, window: &mut Window, cx: &mut Context<Self>) {
        self.hsv = Hsv::from_picker(color);
        self.emit(window, cx);
    }

    fn randomize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos() as u64)
            .unwrap_or(0);
        self.hsv = hsv_from_hex(&random_hex(seed));
        self.emit(window, cx);
    }
}

impl Render for ColorPickerPopover {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let current = self.hsv.to_picker();
        let area_width = POPOVER_WIDTH - POPOVER_PAD * 2.0;
        let swatches = self
            .palette
            .iter()
            .filter_map(|entry| PickerColor::from_hex(entry))
            .collect::<Vec<_>>();
        let entity = cx.entity().downgrade();
        let area_entity = entity.clone();
        let hue_entity = entity.clone();
        let swatch_entity = entity.clone();
        let hex_entity = entity;

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
            .font_family(crate::ui::font::UI_FONT)
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
                ColorField::new("color-hex", current)
                    .variant(FieldVariant::Secondary)
                    .state(self.hex_field.clone())
                    .full_width(true)
                    .height(px(HEX_ROW_HEIGHT))
                    .padding_x(px(chrome::FIELD_PAD_X))
                    .radius(px(chrome::RADIUS_XL))
                    .placeholder("#000000")
                    .on_change(move |color, window, cx| {
                        let Some(color) = *color else {
                            return;
                        };
                        let Some(entity) = hex_entity.upgrade() else {
                            return;
                        };
                        entity.update(cx, |this, cx| this.apply(color, window, cx));
                    }),
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
        .w(px(TRIGGER_WIDTH))
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
    fn the_hex_field_parses_what_electron_parses() {
        assert_eq!(
            PickerColor::from_hex("aabbcc").map(|color| color.to_hex()),
            Some("#AABBCC".to_owned())
        );
        assert_eq!(
            PickerColor::from_hex("  #aAbBcC  ").map(|color| color.to_hex()),
            Some("#AABBCC".to_owned())
        );
        assert_eq!(
            PickerColor::from_hex("#abc").map(|color| color.to_hex()),
            Some("#AABBCC".to_owned())
        );
        assert!(PickerColor::from_hex("nothex").is_none());
        assert!(PickerColor::from_hex("").is_none());
    }

    #[test]
    fn a_typed_hex_reaches_the_annotation_unchanged() {
        for hex in [
            "#aabbcc", "#ff3b30", "#000000", "#ffffff", "#3b82f6", "#010203", "#7f8081", "#fe0154",
        ] {
            let color = PickerColor::from_hex(hex).expect("valid hex");
            assert_eq!(Hsv::from_picker(color).to_hex(), hex, "typed {hex}");
        }
        for seed in 0..4096u64 {
            let hex = random_hex(seed);
            let color = PickerColor::from_hex(&hex).expect("valid hex");
            assert_eq!(Hsv::from_picker(color).to_hex(), hex, "shuffled {hex}");
        }
    }

    #[test]
    fn the_shuffle_reaches_the_whole_rgb_range() {
        let samples: Vec<String> = (0..4096u64).map(random_hex).collect();
        assert!(samples
            .iter()
            .all(|hex| PickerColor::from_hex(hex).is_some()));
        let unique: std::collections::HashSet<&String> = samples.iter().collect();
        assert!(unique.len() > 4000, "{}", unique.len());

        let parsed: Vec<Hsv> = samples.iter().map(|hex| hsv_from_hex(hex)).collect();
        assert!(parsed.iter().any(|hsv| hsv.value < 0.25));
        assert!(parsed.iter().any(|hsv| hsv.saturation < 0.2));
        assert!(parsed.iter().any(|hsv| hsv.value > 0.9));
        assert_eq!(random_hex(7), random_hex(7));
        assert_ne!(random_hex(7), random_hex(8));
    }

    #[test]
    fn the_trigger_is_the_width_the_renderer_lays_out() {
        assert_eq!(TRIGGER_WIDTH, 54.0);
    }

    #[herogpui::test]
    fn a_rejected_hex_reverts_to_the_committed_colour(cx: &mut gpui::TestAppContext) {
        cx.update(herogpui::init);
        let window = cx.add_window(|_window, cx| {
            ColorPickerPopover::new(
                "#ff0000",
                Vec::new(),
                Rc::new(|_, _, _| {}),
                Rc::new(|_, _| {}),
                cx,
            )
        });
        cx.refresh().unwrap();
        cx.run_until_parked();

        window
            .update(cx, |this, window, cx| {
                let field = this.hex_field.read(cx).focus_handle(cx);
                window.focus(&field, cx);
            })
            .unwrap();
        cx.refresh().unwrap();
        cx.run_until_parked();

        window
            .update(cx, |this, window, cx| {
                let committed = PickerColor::from_hex("#3b82f6").expect("valid hex");
                this.apply(committed, window, cx);
                this.hex_field.update(cx, |state, cx| {
                    state.set_value("#ab");
                    cx.notify();
                });
            })
            .unwrap();
        cx.refresh().unwrap();
        cx.run_until_parked();

        let elsewhere = window
            .update(cx, |_, window, cx| {
                let handle = cx.focus_handle().tab_stop(true);
                window.focus(&handle, cx);
                handle
            })
            .unwrap();
        cx.refresh().unwrap();
        cx.run_until_parked();
        drop(elsewhere);

        let restored = window
            .update(cx, |this, _, cx| {
                this.hex_field.read(cx).value().to_string()
            })
            .unwrap();
        assert_eq!(restored, "#3B82F6");
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
