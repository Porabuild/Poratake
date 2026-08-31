use gpui::{div, prelude::*, px, AnyElement, Pixels, Point, Styled};

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

pub fn render(
    frame: &image::RgbaImage,
    pointer: Point<Pixels>,
    viewport: gpui::Size<Pixels>,
    theme: &crate::theme::vars::ThemeVars,
) -> AnyElement {
    const GRID: i32 = 15;
    const HALF: i32 = GRID / 2;
    const CELL: f32 = 7.0;
    const SIZE: f32 = GRID as f32 * CELL;
    const CARD_WIDTH: f32 = 128.0;
    const CARD_HEIGHT: f32 = SIZE + 32.0;
    const OFFSET: f32 = 20.0;

    let logical = selection::Size {
        width: f32::from(viewport.width),
        height: f32::from(viewport.height),
    };
    let (sample_x, sample_y) = frame_point(frame, pointer, logical);
    let pixel = frame.get_pixel(sample_x, sample_y);
    let hex = format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]);
    let pointer_x = f32::from(pointer.x);
    let pointer_y = f32::from(pointer.y);
    let card_x = if pointer_x + OFFSET + CARD_WIDTH > logical.width {
        pointer_x - OFFSET - CARD_WIDTH
    } else {
        pointer_x + OFFSET
    };
    let card_y = if pointer_y + OFFSET + CARD_HEIGHT > logical.height {
        pointer_y - OFFSET - CARD_HEIGHT
    } else {
        pointer_y + OFFSET
    };

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
    loupe = loupe.child(
        div()
            .absolute()
            .left(px(HALF as f32 * CELL))
            .top(px(HALF as f32 * CELL))
            .size(px(CELL))
            .border_1()
            .border_color(crate::ui::colors::white(0.9))
            .shadow(vec![gpui::BoxShadow {
                color: crate::ui::colors::black(0.4),
                offset: gpui::point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(1.0),
            }]),
    );

    div()
        .absolute()
        .left(px(card_x.max(0.0)))
        .top(px(card_y.max(0.0)))
        .w(px(CARD_WIDTH))
        .rounded(px(chrome::RADIUS_2XL))
        .border_2()
        .border_color(theme.muted_foreground.opacity(0.35))
        .bg(theme.muted_background.opacity(0.95))
        .p(px(6.0))
        .shadow_2xl()
        .child(div().flex().justify_center().child(loupe))
        .child(
            div()
                .mt(px(6.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .px(px(2.0))
                .child(
                    div()
                        .size(px(14.0))
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
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
