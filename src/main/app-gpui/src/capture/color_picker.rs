use gpui::{div, prelude::*, px, AnyElement, Element, Pixels, Point, Styled};
use herogpui::gpui;

use crate::capture::selection;
use crate::theme::color::Srgba;
use crate::ui::chrome;

pub fn frame_point(
    frame: &image::RgbaImage,
    pointer: Point<Pixels>,
    viewport: selection::Size,
) -> (u32, u32) {
    let x = (f32::from(pointer.x) * frame.width() as f32 / viewport.width.max(1.0)).round();
    let y = (f32::from(pointer.y) * frame.height() as f32 / viewport.height.max(1.0)).round();
    (
        x.clamp(0.0, frame.width().saturating_sub(1) as f32) as u32,
        y.clamp(0.0, frame.height().saturating_sub(1) as f32) as u32,
    )
}

fn pixel_color(pixel: image::Rgba<u8>) -> gpui::Hsla {
    Srgba {
        r: pixel[0] as f32 / 255.0,
        g: pixel[1] as f32 / 255.0,
        b: pixel[2] as f32 / 255.0,
        a: pixel[3] as f32 / 255.0,
    }
    .to_hsla()
}

pub const HINT: &str = "Click to copy \u{b7} Esc to cancel";

struct PickerCard {
    pointer: Point<Pixels>,
    child: AnyElement,
}

impl gpui::IntoElement for PickerCard {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for PickerCard {
    type RequestLayoutState = gpui::LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let child = self.child.request_layout(window, cx);
        let style = gpui::Style {
            position: gpui::Position::Absolute,
            display: gpui::Display::Flex,
            ..Default::default()
        };
        (window.request_layout(style, [child], cx), child)
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        let size = window.layout_bounds(*child).size;
        let viewport = window.viewport_size();
        let offset = px(20.0);
        let place = |pointer: Pixels, length: Pixels, extent: Pixels| {
            let after = pointer + offset;
            let before = pointer - offset - length;
            let after_overflow = (after + length - extent).max(px(0.0));
            let before_overflow = (px(0.0) - before).max(px(0.0));
            let (preferred, overflow) = if after_overflow <= before_overflow {
                (after, after_overflow)
            } else {
                (before, before_overflow)
            };
            (
                preferred.max(px(0.0)).min((extent - length).max(px(0.0))),
                preferred,
                overflow,
            )
        };
        let (mut x, preferred_x, overflow_x) = place(self.pointer.x, size.width, viewport.width);
        let (mut y, preferred_y, overflow_y) = place(self.pointer.y, size.height, viewport.height);
        if self.pointer.x >= x
            && self.pointer.x <= x + size.width
            && self.pointer.y >= y
            && self.pointer.y <= y + size.height
        {
            if overflow_x <= overflow_y {
                x = preferred_x;
            } else {
                y = preferred_y;
            }
        }
        window.with_element_offset(gpui::point(x, y) - bounds.origin, |window| {
            self.child.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: gpui::Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        self.child.paint(window, cx);
    }
}

pub fn render(
    frame: &image::RgbaImage,
    pointer: Point<Pixels>,
    window: &gpui::Window,
    theme: &crate::theme::vars::ThemeVars,
) -> AnyElement {
    const GRID: i32 = 15;
    const HALF: i32 = GRID / 2;
    const CELL: f32 = 7.0;
    const SIZE: f32 = GRID as f32 * CELL;
    const PADDING: f32 = 6.0;
    const SWATCH_ROW: f32 = 14.0;
    const SWATCH_GAP: f32 = 6.0;
    const HINT_SIZE: f32 = 12.0;
    const HINT_ROW: f32 = 16.0;
    const HINT_GAP: f32 = 4.0;
    const CARD_WIDTH: f32 = 196.0;

    let viewport = window.viewport_size();
    let logical = selection::Size {
        width: f32::from(viewport.width),
        height: f32::from(viewport.height),
    };
    let (sample_x, sample_y) = frame_point(frame, pointer, logical);
    let pixel = frame.get_pixel(sample_x, sample_y);
    let hex = format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]);
    let mut loupe = div()
        .relative()
        .flex()
        .flex_col()
        .size(px(SIZE))
        .overflow_hidden()
        .rounded(px(chrome::RADIUS_LG));
    for row in -HALF..=HALF {
        let mut cells = div().flex().flex_row().h(px(CELL));
        for column in -HALF..=HALF {
            let x = (sample_x as i64 + column as i64)
                .clamp(0, frame.width().saturating_sub(1) as i64) as u32;
            let y = (sample_y as i64 + row as i64).clamp(0, frame.height().saturating_sub(1) as i64)
                as u32;
            cells = cells.child(
                div()
                    .flex_none()
                    .size(px(CELL))
                    .bg(pixel_color(*frame.get_pixel(x, y))),
            );
        }
        loupe = loupe.child(cells);
    }
    let marker_origin = window.pixel_snap(px(HALF as f32 * CELL));
    let marker_size = window.pixel_snap(px(CELL));
    let outline_width = window.pixel_snap(px(1.0));
    loupe = loupe.child(
        div()
            .absolute()
            .left(marker_origin - outline_width)
            .top(marker_origin - outline_width)
            .w(marker_size + outline_width * 2.0)
            .h(marker_size + outline_width * 2.0)
            .border_1()
            .border_color(crate::ui::colors::black(0.4)),
    );
    loupe = loupe.child(
        div()
            .absolute()
            .left(px(HALF as f32 * CELL))
            .top(px(HALF as f32 * CELL))
            .size(px(CELL))
            .border_1()
            .border_color(crate::ui::colors::white(0.9)),
    );

    let loupe_container = div().flex().justify_center().child(loupe);
    #[cfg(feature = "parity")]
    let loupe_container = loupe_container.on_children_prepainted(|bounds, _, _| {
        crate::ui::parity::record("color-picker-loupe", bounds);
    });

    let card = div()
        .debug_selector(|| "color-picker-card".to_string())
        .w(px(CARD_WIDTH))
        .rounded(px(chrome::RADIUS_2XL))
        .border_2()
        .border_color(theme.muted_foreground.opacity(0.35))
        .bg(theme.muted_background.opacity(0.95))
        .p(px(PADDING))
        .shadow_2xl()
        .child(loupe_container)
        .child(
            div()
                .mt(px(SWATCH_GAP))
                .flex()
                .items_center()
                .gap(px(6.0))
                .px(px(2.0))
                .child(
                    div()
                        .size(px(SWATCH_ROW))
                        .rounded_full()
                        .border_1()
                        .border_color(theme.border.opacity(0.6))
                        .bg(pixel_color(*pixel)),
                )
                .child(
                    div()
                        .font_family(crate::ui::colors::MONO_FONT)
                        .text_size(px(12.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(hex),
                ),
        )
        .child(
            div()
                .mt(px(HINT_GAP))
                .h(px(HINT_ROW))
                .px(px(2.0))
                .text_size(px(HINT_SIZE))
                .text_color(theme.muted_foreground)
                .child(HINT),
        );

    let wrapper = div().child(card);
    #[cfg(feature = "parity")]
    let wrapper = wrapper.on_children_prepainted(|bounds, _, _| {
        crate::ui::parity::record("color-picker-card", bounds);
    });

    PickerCard {
        pointer,
        child: wrapper.into_any_element(),
    }
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hint_is_the_line_the_reference_shows() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root")
            .to_path_buf();
        let reference = std::fs::read_to_string(
            root.join("src/renderer/components/area-overlay/color-picker.tsx"),
        )
        .expect("reference color picker");
        assert!(reference.contains(HINT));
    }

    #[test]
    fn frame_point_scales_and_clamps_logical_coordinates() {
        let frame = image::RgbaImage::new(1600, 1200);
        let viewport = selection::Size {
            width: 800.0,
            height: 600.0,
        };

        assert_eq!(
            frame_point(&frame, gpui::point(px(400.0), px(300.0)), viewport),
            (800, 600)
        );
        assert_eq!(
            frame_point(&frame, gpui::point(px(800.0), px(600.0)), viewport),
            (1599, 1199)
        );
    }

    #[herogpui::test]
    fn picker_card_stays_inside_display_edges(cx: &mut gpui::TestAppContext) {
        use herogpui::test::TestWindowExt;

        struct Scene {
            frame: image::RgbaImage,
            pointer: Point<Pixels>,
            clicks: std::rc::Rc<std::cell::Cell<usize>>,
        }

        impl gpui::Render for Scene {
            fn render(
                &mut self,
                window: &mut gpui::Window,
                cx: &mut gpui::Context<Self>,
            ) -> impl gpui::IntoElement {
                let clicks = self.clicks.clone();
                div()
                    .relative()
                    .size_full()
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                        clicks.set(clicks.get() + 1);
                    })
                    .child(super::render(
                        &self.frame,
                        self.pointer,
                        window,
                        &crate::theme::vars::active_theme(cx),
                    ))
            }
        }

        let clicks = std::rc::Rc::new(std::cell::Cell::new(0));
        let (scene, visual) = herogpui::test::open_window(cx, |_, _| Scene {
            frame: image::RgbaImage::from_pixel(800, 600, image::Rgba([64, 112, 192, 255])),
            pointer: gpui::point(px(0.), px(0.)),
            clicks: clicks.clone(),
        });
        for scale in [1., 1.25, 1.5, 1.75, 2.] {
            visual.simulate_scale_factor_change(scale);
            visual.simulate_resize(gpui::size(px(400.), px(300.)));
            for x in [
                0.,
                100.,
                190.,
                ((190. * scale).round() + 1.) / scale,
                200.,
                ((400. * scale).round() - 1.) / scale,
                399.,
            ] {
                for y in [
                    0.,
                    100.,
                    120.,
                    ((120. * scale).round() + 1.) / scale,
                    150.,
                    ((300. * scale).round() - 1.) / scale,
                    299.,
                ] {
                    visual.update(|_, cx| {
                        scene.update(cx, |scene, cx| {
                            scene.pointer = gpui::point(px(x), px(y));
                            cx.notify();
                        });
                    });
                    let card = visual.expect("color-picker-card");
                    assert_eq!(visual.update(|window, _| window.scale_factor()), scale);
                    assert!(
                        card.left() >= px(0.)
                            && card.top() >= px(0.)
                            && card.right() <= px(400.)
                            && card.bottom() <= px(300.),
                        "scale {scale}, pointer ({x}, {y}): card {card:?}"
                    );
                    assert!(
                        px(x) < card.left()
                            || px(x) > card.right()
                            || px(y) < card.top()
                            || px(y) > card.bottom(),
                        "scale {scale}, pointer ({x}, {y}) is covered by card {card:?}"
                    );
                    if x == 0. {
                        assert!(card.left() >= px(19.));
                    }
                    if x == 399. {
                        assert!(card.right() <= px(380.));
                    }
                    if y == 0. {
                        assert!(card.top() >= px(19.));
                    }
                    if y == 299. {
                        assert!(card.bottom() <= px(280.));
                    }
                }
            }
            visual.simulate_resize(gpui::size(px(200.), px(180.)));
            visual.update(|_, cx| {
                scene.update(cx, |scene, cx| {
                    scene.pointer = gpui::point(px(100.), px(90.));
                    cx.notify();
                });
            });
            let card = visual.expect("color-picker-card");
            assert!(
                card.right() <= px(81.)
                    || card.left() >= px(119.)
                    || card.bottom() <= px(71.)
                    || card.top() >= px(109.),
                "scale {scale}, pointer (100, 90) is covered by card {card:?}"
            );
            assert!(
                card.left() >= px(0.)
                    && card.right() <= px(200.)
                    && card.top() >= px(109.)
                    && card.top() <= px(111.),
                "scale {scale}, no-fit fallback clips more than necessary: {card:?}"
            );
        }
        visual.simulate_resize(gpui::size(px(400.), px(300.)));
        let card = visual.expect("color-picker-card");
        visual.simulate_mouse_down(
            card.center(),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        assert_eq!(clicks.get(), 1);
    }
}
