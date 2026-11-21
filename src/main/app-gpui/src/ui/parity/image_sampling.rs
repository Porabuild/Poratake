use std::path::Path;
use std::sync::Arc;

use gpui::{prelude::*, AppContext, Context, Render, Window};
use herogpui::gpui;

use super::{point, px, settle, size, HeadlessAppContext};

struct SmallImageProbe {
    image: Arc<gpui::RenderImage>,
    neighbor: Arc<gpui::RenderImage>,
    gradient: Arc<gpui::RenderImage>,
    wide_gradient: Arc<gpui::RenderImage>,
    tall_gradient: Arc<gpui::RenderImage>,
    transparent: Arc<gpui::RenderImage>,
    empty: Arc<gpui::RenderImage>,
}

impl Render for SmallImageProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        let neighbor = self.neighbor.clone();
        let image = self.image.clone();
        let gradient = self.gradient.clone();
        let wide_gradient = self.wide_gradient.clone();
        let tall_gradient = self.tall_gradient.clone();
        let transparent = self.transparent.clone();
        let empty = self.empty.clone();
        gpui::div().size_full().bg(gpui::black()).child(
            gpui::canvas(
                |_, _, _| (),
                move |_, (), window, _| {
                    let filler = gpui::Bounds::new(point(px(10.), px(70.)), size(px(16.), px(16.)));
                    window
                        .paint_image(filler, filler, gpui::Corners::default(), neighbor, 0, false)
                        .unwrap();
                    let bounds = gpui::Bounds::new(point(px(0.), px(0.)), size(px(200.), px(50.)));
                    let image_bounds =
                        gpui::Bounds::new(point(px(0.), px(-75.)), size(px(200.), px(200.)));
                    window
                        .paint_image(
                            bounds,
                            image_bounds,
                            gpui::Corners::default(),
                            image.clone(),
                            0,
                            false,
                        )
                        .unwrap();
                    let wide_bounds =
                        gpui::Bounds::new(point(px(300.), px(0.)), size(px(60.), px(50.)));
                    let wide_image =
                        gpui::Bounds::new(point(px(255.), px(0.)), size(px(200.), px(50.)));
                    window
                        .paint_image(
                            wide_bounds,
                            wide_image,
                            gpui::Corners::default(),
                            wide_gradient,
                            0,
                            false,
                        )
                        .unwrap();
                    let tall_bounds =
                        gpui::Bounds::new(point(px(150.), px(105.)), size(px(20.), px(60.)));
                    let tall_image =
                        gpui::Bounds::new(point(px(150.), px(60.)), size(px(20.), px(200.)));
                    window
                        .paint_image(
                            tall_bounds,
                            tall_image,
                            gpui::Corners::default(),
                            tall_gradient,
                            0,
                            false,
                        )
                        .unwrap();
                    let far_bounds =
                        gpui::Bounds::new(point(px(210.), px(0.)), size(px(10.), px(50.)));
                    let far_image =
                        gpui::Bounds::new(point(px(-785.), px(0.)), size(px(1000.), px(50.)));
                    window
                        .paint_image(
                            far_bounds,
                            far_image,
                            gpui::Corners::default(),
                            image,
                            0,
                            false,
                        )
                        .unwrap();
                    let gradient_bounds =
                        gpui::Bounds::new(point(px(230.), px(0.)), size(px(20.), px(50.)));
                    let gradient_image =
                        gpui::Bounds::new(point(px(140.), px(0.)), size(px(200.), px(50.)));
                    window
                        .paint_image(
                            gradient_bounds,
                            gradient_image,
                            gpui::Corners::default(),
                            gradient.clone(),
                            0,
                            false,
                        )
                        .unwrap();
                    let edge_bounds =
                        gpui::Bounds::new(point(px(270.), px(0.)), size(px(10.), px(50.)));
                    let edge_image =
                        gpui::Bounds::new(point(px(-1720.), px(0.)), size(px(2000.), px(50.)));
                    window
                        .paint_image(
                            edge_bounds,
                            edge_image,
                            gpui::Corners::default(),
                            gradient,
                            0,
                            false,
                        )
                        .unwrap();
                    let alpha_bounds =
                        gpui::Bounds::new(point(px(30.), px(70.)), size(px(20.), px(20.)));
                    let alpha_image =
                        gpui::Bounds::new(point(px(-60.), px(70.)), size(px(200.), px(20.)));
                    window
                        .paint_image(
                            alpha_bounds,
                            alpha_image,
                            gpui::Corners::default(),
                            transparent,
                            0,
                            false,
                        )
                        .unwrap();
                    let empty_bounds =
                        gpui::Bounds::new(point(px(180.), px(70.)), size(px(40.), px(20.)));
                    window
                        .paint_image(
                            empty_bounds,
                            empty_bounds,
                            gpui::Corners::default(),
                            empty,
                            0,
                            false,
                        )
                        .unwrap();
                },
            )
            .size_full(),
        )
    }
}

pub(super) fn run(cx: &mut HeadlessAppContext, output: &Path) {
    let image = Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([64, 80, 240, 255])
        ))
    ]));
    let neighbor = Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(image::RgbaImage::from_pixel(
            1024,
            16,
            image::Rgba([255, 255, 255, 255])
        ))
    ]));
    let gradient = Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(image::RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                image::Rgba([0, 0, 255, 255])
            } else {
                image::Rgba([255, 0, 0, 255])
            }
        }))
    ]));
    let transparent = Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(image::RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                image::Rgba([0, 0, 255, 255])
            } else {
                image::Rgba([255, 0, 0, 0])
            }
        }))
    ]));
    let wide_gradient = Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(image::RgbaImage::from_fn(8, 1, |x, _| {
            let level = 20 + 30 * x as u8;
            image::Rgba([level, level, level, 255])
        }))
    ]));
    let tall_gradient = Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(image::RgbaImage::from_fn(1, 8, |_, y| {
            let level = 20 + 30 * y as u8;
            image::Rgba([level, level, level, 255])
        }))
    ]));
    let empty = Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(image::RgbaImage::new(0, 1))
    ]));
    let window = cx
        .open_window(size(px(360.), px(180.)), |window, cx| {
            window.set_scale_factor(2.);
            cx.new(|_| SmallImageProbe {
                image,
                neighbor,
                gradient,
                wide_gradient,
                tall_gradient,
                transparent,
                empty,
            })
        })
        .unwrap();
    settle(cx, window.into());
    let screenshot = cx.capture_screenshot(window.into()).unwrap();
    screenshot
        .save(output.join("small-cover-probe.png"))
        .unwrap();
    let pixel = screenshot.get_pixel(200, 50).0;
    assert!(
        pixel
            .iter()
            .zip([240_u8, 80, 64, 255])
            .all(|(a, b)| a.abs_diff(b) <= 2),
        "one-pixel cover image must retain its color: {pixel:?}"
    );
    let far_pixel = screenshot.get_pixel(424, 50).0;
    assert!(
        far_pixel
            .iter()
            .zip([240_u8, 80, 64, 255])
            .all(|(a, b)| a.abs_diff(b) <= 2),
        "a one-pixel image clipped at its far edge must retain its color: {far_pixel:?}"
    );
    assert_eq!(
        screenshot.get_pixel(400, 160).0,
        [0, 0, 0, 255],
        "an empty image must not paint pixels"
    );
    if cfg!(target_os = "macos") {
        for y in [2, 50, 97] {
            let blended = screenshot.get_pixel(480, y).0;
            assert!(blended.iter().zip([128_u8, 0, 128, 255]).all(|(a, b)| a.abs_diff(b) <= 2), "a narrow crop across two source pixels must preserve their blend at y={y}: {blended:?}");
        }
        let edge = screenshot.get_pixel(550, 50).0;
        assert!(
            edge.iter()
                .zip([0_u8, 0, 255, 255])
                .all(|(a, b)| a.abs_diff(b) <= 2),
            "a narrow crop at the source edge must not sample another atlas tile: {edge:?}"
        );
        for x in [602_u32, 620, 650, 680, 716] {
            let source_pixel =
                (((x as f32 + 0.5) / 2.0 - 255.0) / 200.0 * 8.0 - 0.5).clamp(0.0, 7.0);
            let expected = (20.0 + 30.0 * source_pixel).round() as u8;
            let pixel = screenshot.get_pixel(x, 50).0;
            assert!(
                pixel[..3].iter().all(|value| value.abs_diff(expected) <= 2),
                "fractional crop at device x={x} expected {expected}, got {pixel:?}"
            );
        }
        for y in [212_u32, 230, 260, 290, 326] {
            let source_pixel =
                (((y as f32 + 0.5) / 2.0 - 60.0) / 200.0 * 8.0 - 0.5).clamp(0.0, 7.0);
            let expected = (20.0 + 30.0 * source_pixel).round() as u8;
            let pixel = screenshot.get_pixel(310, y).0;
            assert!(
                pixel[..3].iter().all(|value| value.abs_diff(expected) <= 2),
                "fractional crop at device y={y} expected {expected}, got {pixel:?}"
            );
        }
        let alpha = screenshot.get_pixel(80, 160).0;
        assert!(
            alpha
                .iter()
                .zip([128_u8, 0, 0, 255])
                .all(|(a, b)| a.abs_diff(b) <= 2),
            "a narrow translucent crop must interpolate premultiplied pixels: {alpha:?}"
        );
    }
    cx.update_window(window.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}
