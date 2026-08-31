//! Pin window — a borderless always-on-top image viewer, port of the Electron
//! `pin-window` (screenshot pinned to desktop).

use std::sync::Arc;

use gpui::{
    div, img, prelude::*, px, size, App, Bounds, Context, Render, Styled, Window,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};

use crate::ui::chrome;

pub struct PinWindow {
    image: Arc<gpui::RenderImage>,
}

impl PinWindow {
    pub fn open(cx: &mut App, png_bytes: Vec<u8>) {
        let decoded = match image::load_from_memory(&png_bytes) {
            Ok(img) => img.to_rgba8(),
            Err(_) => return,
        };
        let (width, height) = (decoded.width() as f32, decoded.height() as f32);
        let mut buffer = decoded;
        for pixel in buffer.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let frame = image::Frame::new(buffer);
        let render_image = Arc::new(gpui::RenderImage::new(smallvec::smallvec![frame]));

        let (work_width, work_height) = cx
            .displays()
            .first()
            .map(|display| {
                (
                    f32::from(display.bounds().size.width),
                    f32::from(display.bounds().size.height),
                )
            })
            .unwrap_or((1920.0, 1080.0));
        let (window_width, window_height) =
            chrome::pin_window_size(width, height, work_width, work_height);
        let (origin_x, origin_y) = chrome::pin_window_origin(window_width, work_width, 0);
        let bounds = Bounds {
            origin: gpui::point(px(origin_x.max(0.0)), px(origin_y.max(0.0))),
            size: size(px(window_width), px(window_height)),
        };
        cx.open_window(pin_window_options(bounds), |_, cx| {
            cx.new(|_| Self {
                image: render_image.clone(),
            })
        })
        .ok();
    }
}

pub(crate) fn pin_window_options(bounds: Bounds<gpui::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        focus: true,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: true,
        is_resizable: true,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Opaque,
        ..Default::default()
    }
}

impl Render for PinWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p(px(chrome::PIN_PAD))
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .child(
                img(self.image.clone())
                    .size_full()
                    .object_fit(gpui::ObjectFit::Contain),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::chrome;

    #[test]
    fn pin_window_is_frameless_and_always_on_top() {
        let options = pin_window_options(Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(200.0), px(100.0)),
        });
        assert!(options.titlebar.is_none());
        assert_eq!(options.kind, WindowKind::PopUp);
        assert!(options.is_movable);
        assert!(options.is_resizable);
        assert!(!options.is_minimizable);
        assert_eq!(
            options.window_background,
            WindowBackgroundAppearance::Opaque
        );
        assert_eq!(chrome::PIN_PAD, 0.0);
    }

    #[test]
    fn pin_has_no_padded_chrome() {
        assert_eq!(chrome::PIN_PAD, 0.0);
        let electron_scale = |image_w: f32, image_h: f32, work_w: f32, work_h: f32| {
            1.0_f32
                .min((work_w * 0.5) / image_w)
                .min((work_h * 0.5) / image_h)
        };
        let scale = electron_scale(800.0, 600.0, 1920.0, 1080.0);
        assert_eq!(
            chrome::pin_window_size(800.0, 600.0, 1920.0, 1080.0),
            ((800.0 * scale).floor(), (600.0 * scale).floor())
        );
        let scale = electron_scale(3000.0, 2000.0, 1920.0, 1080.0);
        assert_eq!(
            chrome::pin_window_size(3000.0, 2000.0, 1920.0, 1080.0),
            ((3000.0 * scale).floor(), (2000.0 * scale).floor())
        );
        assert_eq!(
            chrome::pin_window_origin(400.0, 1920.0, 0),
            (1920.0 - 400.0 - 20.0, 20.0)
        );
        assert_eq!(
            chrome::pin_window_origin(400.0, 1920.0, 2),
            (1920.0 - 400.0 - 20.0 - 60.0, 80.0)
        );
    }
}
