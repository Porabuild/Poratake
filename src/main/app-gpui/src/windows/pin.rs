//! Pin window — a borderless always-on-top image viewer, port of the Electron
//! `pin-window` (screenshot pinned to desktop).

use std::sync::Arc;

use gpui::{div, img, prelude::*, px, size, App, Bounds, Context, Render, Styled, Window};
use herogpui::gpui;

use crate::ui::chrome;

pub struct PinWindow {
    image: Arc<gpui::RenderImage>,
    image_size: gpui::Size<gpui::Pixels>,
    restore: Option<(String, crate::editor::window::PersistedEditorState)>,
    hovered: bool,
}

impl PinWindow {
    pub fn open(cx: &mut App, png_bytes: Vec<u8>) {
        Self::open_with_restore(cx, png_bytes, None);
    }

    /// Port of `screenshot:pin`: the editor closes into the pin and the pin
    /// reopens it, with its persisted state, when closed.
    pub(crate) fn open_for_editor(
        cx: &mut App,
        png_bytes: Vec<u8>,
        file_path: String,
        state: crate::editor::window::PersistedEditorState,
    ) {
        Self::open_with_restore(cx, png_bytes, Some((file_path, state)));
    }

    fn open_with_restore(
        cx: &mut App,
        png_bytes: Vec<u8>,
        restore: Option<(String, crate::editor::window::PersistedEditorState)>,
    ) {
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
        #[cfg(feature = "parity")]
        let scale = if std::env::var_os(crate::ui::parity::daemon_fixture::PROCESS_ENV).is_some() {
            std::env::var("PORATAKE_PARITY_SCALE")
                .unwrap_or_else(|_| "2".into())
                .parse::<f32>()
                .unwrap()
        } else {
            scale
        };
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
        cx.open_window(options, |window, cx| {
            window.set_window_title("Poratake pinned screenshot");
            let view = cx.new(|_| Self {
                image: render_image.clone(),
                image_size: size(px(width), px(height)),
                restore,
                hovered: false,
            });
            let weak = view.downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                weak.update(cx, |view, cx| view.restore_editor(cx)).ok();
                true
            });
            view
        })
        .ok();
    }

    fn restore_editor(&mut self, cx: &mut App) {
        if let Some((path, state)) = self.restore.take() {
            crate::open_editor_with_state(cx, &path, Some(state));
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut App) {
        self.restore_editor(cx);
        window.remove_window();
    }
}

impl Render for PinWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let image = crate::ui::window_controls::drag_area("pin-drag")
            .absolute()
            .inset_0()
            .mx(px(0.))
            .flex()
            .items_center()
            .justify_center()
            .child(
                img(self.image.clone())
                    .w(self.image_size.width)
                    .h(self.image_size.height)
                    .max_w_full()
                    .max_h_full()
                    .object_fit(gpui::ObjectFit::Contain),
            );
        let content = crate::ui::font::root()
            .size_full()
            .relative()
            .p(px(chrome::PIN_PAD))
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center();
        #[cfg(feature = "parity")]
        let content = content.on_children_prepainted(|bounds, _, _| {
            crate::ui::parity::record("pin-controls", bounds);
        });
        let content = content
            .id("pin-root")
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            .child(image)
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
            });
        crate::ui::window_root::focus_root(content, window, cx)
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

    #[herogpui::test]
    fn native_close_restores_the_annotated_editor_without_history(cx: &mut gpui::TestAppContext) {
        use crate::config::store::ConfigStore;
        use crate::editor::annotations::Annotation;
        use crate::editor::window::{EditorWindow, PersistedEditorState};

        let directory = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(directory.path().join("config.json")).unwrap());
        config.update(|settings| settings.history.enabled = false);
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let path = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(320, 180, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        let path = path.to_string_lossy().into_owned();
        let annotations = vec![Annotation::Pen {
            id: "native-close".into(),
            points: vec![20., 30., 160., 90.],
            stroke: "#EF4D3B".into(),
            stroke_width: 8.,
        }];
        let state: PersistedEditorState =
            serde_json::from_value(serde_json::json!({"annotations": &annotations})).unwrap();
        let editor = cx.add_window(|window, cx| {
            EditorWindow::from_file_with_state(&path, state.clone(), window, cx)
        });
        let expected = editor
            .update(cx, |editor, _, _| editor.export_png().unwrap())
            .unwrap();
        cx.update(|cx| PinWindow::open_for_editor(cx, expected.clone(), path.clone(), state));
        editor
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        let pin = cx.read(|cx| {
            cx.windows()
                .into_iter()
                .find(|window| window.downcast::<PinWindow>().is_some())
                .unwrap()
        });
        let mut visual = gpui::VisualTestContext::from_window(pin, cx);
        let accepted = visual.simulate_close();
        cx.update_window(pin, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        let restored: Vec<_> = cx.read(|cx| {
            cx.windows()
                .into_iter()
                .filter_map(|window| window.downcast::<EditorWindow>())
                .collect()
        });
        assert_eq!(restored.len(), 1, "native close must restore the editor");
        assert!(accepted);
        restored[0]
            .update(cx, |editor, _, _| {
                assert_eq!(editor.file_path, path);
                assert_eq!(editor.history.current(), annotations);
                assert_eq!(editor.export_png().unwrap(), expected);
            })
            .unwrap();
    }

    #[herogpui::test]
    fn native_close_of_a_standalone_pin_does_not_open_an_editor(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(320, 180, image::Rgba([64, 112, 192, 255]))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        cx.update(|cx| PinWindow::open(cx, bytes.into_inner()));
        let pin = cx.read(|cx| cx.windows()[0]);
        let mut visual = gpui::VisualTestContext::from_window(pin, cx);
        assert!(visual.simulate_close());
        cx.update_window(pin, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        assert!(cx.read(|cx| cx.windows().is_empty()));
    }
}
