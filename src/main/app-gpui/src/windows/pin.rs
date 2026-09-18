//! Pin window — a borderless always-on-top image viewer, port of the Electron
//! `pin-window` (screenshot pinned to desktop).

use std::sync::Arc;

use gpui::{div, img, prelude::*, px, size, App, Bounds, Context, Render, Styled, Window};
use herogpui::gpui;

use crate::ui::chrome;

pub struct PinWindow {
    image: Arc<gpui::RenderImage>,
    restore_path: Option<String>,
    hovered: bool,
}

impl PinWindow {
    pub fn open(cx: &mut App, png_bytes: Vec<u8>) {
        Self::open_with_restore(cx, png_bytes, None);
    }

    /// Port of `screenshot:pin`: the editor closes into the pin and the pin
    /// reopens it, with its persisted state, when closed.
    pub fn open_for_editor(cx: &mut App, png_bytes: Vec<u8>, file_path: String) {
        Self::open_with_restore(cx, png_bytes, Some(file_path));
    }

    fn open_with_restore(cx: &mut App, png_bytes: Vec<u8>, restore_path: Option<String>) {
        let decoded = match image::load_from_memory(&png_bytes) {
            Ok(img) => img.to_rgba8(),
            Err(_) => return,
        };
        let displays = cx.displays();
        let display = displays.first();
        let scale = display
            .map(|display| crate::system::work_area::capture_scale_factor(display.as_ref(), cx))
            .unwrap_or(1.0)
            .max(1.0);
        let (width, height) = (
            decoded.width() as f32 / scale,
            decoded.height() as f32 / scale,
        );
        let mut buffer = decoded;
        for pixel in buffer.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let frame = image::Frame::new(buffer);
        let render_image = Arc::new(gpui::RenderImage::new(smallvec::smallvec![frame]));

        let (work_width, work_height) = display
            .map(|display| {
                (
                    f32::from(display.bounds().size.width),
                    f32::from(display.bounds().size.height),
                )
            })
            .unwrap_or((1920.0, 1080.0));
        let (window_width, window_height) =
            chrome::pin_window_size(width, height, work_width, work_height);
        let existing = cx
            .windows()
            .iter()
            .filter_map(|handle| handle.downcast::<PinWindow>())
            .count();
        let (origin_x, origin_y) = chrome::pin_window_origin(window_width, work_width, existing);
        let bounds = Bounds {
            origin: gpui::point(px(origin_x.max(0.0)), px(origin_y.max(0.0))),
            size: size(px(window_width), px(window_height)),
        };
        let mut options = super::popup_window_options(
            bounds,
            super::PopupWindowConfig {
                movable: true,
                resizable: true,
                background: gpui::WindowBackgroundAppearance::Opaque,
                ..Default::default()
            },
        );
        options.window_min_size = Some(size(px(chrome::PIN_MIN_SIZE), px(chrome::PIN_MIN_SIZE)));
        cx.open_window(options, |_, cx| {
            cx.new(|_| Self {
                image: render_image.clone(),
                restore_path,
                hovered: false,
            })
        })
        .ok();
    }

    fn close(&self, window: &mut Window, cx: &mut App) {
        if let Some(path) = &self.restore_path {
            crate::open_editor_for(cx, path);
        }
        window.remove_window();
    }
}

impl Render for PinWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::font::root()
            .id("pin-root")
            .size_full()
            .relative()
            .p(px(chrome::PIN_PAD))
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            .child(
                crate::ui::window_controls::drag_area("pin-drag")
                    .absolute()
                    .inset_0()
                    .items_center()
                    .justify_center()
                    .child(
                        img(self.image.clone())
                            .size_full()
                            .object_fit(gpui::ObjectFit::Contain),
                    ),
            )
            .when(self.hovered, |root| {
                root.child(
                    div()
                        .id("pin-close")
                        .absolute()
                        .top(px(6.0))
                        .right(px(6.0))
                        .w(px(24.0))
                        .h(px(24.0))
                        .rounded_full()
                        .bg(gpui::black().opacity(0.55))
                        .text_color(gpui::white())
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|this, _, window, cx| this.close(window, cx)),
                        )
                        .child(crate::ui::icon::icon_element("x", px(14.0))),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::chrome;
    use crate::windows::{popup_window_options, PopupWindowConfig};
    use gpui::{WindowBackgroundAppearance, WindowKind};

    #[test]
    fn pin_window_is_frameless_and_always_on_top() {
        let options = popup_window_options(
            Bounds {
                origin: gpui::point(px(0.0), px(0.0)),
                size: size(px(200.0), px(100.0)),
            },
            PopupWindowConfig {
                movable: true,
                resizable: true,
                background: gpui::WindowBackgroundAppearance::Opaque,
                ..Default::default()
            },
        );
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
        assert_eq!(chrome::PIN_MIN_SIZE, 100.0);
        assert_eq!(chrome::PIN_OFFSET, 30.0);
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
