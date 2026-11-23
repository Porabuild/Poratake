//! Pin window — a borderless always-on-top image viewer, port of the Electron
//! `pin-window` (screenshot pinned to desktop).

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{div, img, prelude::*, px, size, App, Bounds, Context, Render, Styled, Window};
use herogpui::gpui;

use crate::ui::chrome;

pub struct PinWindow {
    image: Arc<gpui::RenderImage>,
    restore: Option<(String, crate::editor::window::PersistedEditorState)>,
    hovered: bool,
}

impl PinWindow {
    pub(crate) fn open_file(cx: &mut App, path: PathBuf) {
        cx.spawn(async move |cx| {
            let prepared = cx
                .background_executor()
                .spawn(async move {
                    let result = (|| {
                        let bytes = std::fs::read(&path)?;
                        let decoded = crate::render::images::decode(&bytes)?;
                        Ok::<_, anyhow::Error>(Self::prepare_image(decoded.into_rgba8()))
                    })();
                    result.map_err(|error| format!("{}: {error}", path.display()))
                })
                .await;
            cx.update(|cx| match prepared {
                Ok(image) => {
                    if !Self::open_with_restore(cx, image, None) {
                        crate::windows::toast::Toast::show(
                            cx,
                            "Pin failed",
                            "Could not open the pinned screenshot",
                        );
                    }
                }
                Err(error) => crate::windows::toast::Toast::show(cx, "Pin failed", error),
            });
        })
        .detach();
    }

    pub(crate) fn open_for_editor(
        cx: &mut App,
        image: Arc<gpui::RenderImage>,
        file_path: String,
        state: crate::editor::window::PersistedEditorState,
    ) -> bool {
        Self::open_with_restore(cx, image, Some((file_path, state)))
    }

    pub(crate) fn prepare_image(mut rgba: image::RgbaImage) -> Arc<gpui::RenderImage> {
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let frame = image::Frame::new(rgba);
        Arc::new(gpui::RenderImage::new(smallvec::smallvec![frame]))
    }

    fn open_with_restore(
        cx: &mut App,
        render_image: Arc<gpui::RenderImage>,
        restore: Option<(String, crate::editor::window::PersistedEditorState)>,
    ) -> bool {
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
        let image_size = render_image.size(0);
        let (width, height) = (
            image_size.width.0 as f32 / scale,
            image_size.height.0 as f32 / scale,
        );

        let (work_origin_x, work_origin_y, work_width, work_height) = display
            .map(|display| {
                let area = crate::system::work_area::work_area(display.as_ref());
                (
                    f32::from(area.origin.x),
                    f32::from(area.origin.y),
                    f32::from(area.size.width),
                    f32::from(area.size.height),
                )
            })
            .unwrap_or((0.0, 0.0, 1920.0, 1080.0));
        let (window_width, window_height) =
            chrome::pin_window_size(width, height, work_width, work_height);
        let aspect_ratio = chrome::pin_window_aspect_ratio(width, height, work_width, work_height);
        let existing = cx
            .windows()
            .iter()
            .filter_map(|handle| handle.downcast::<PinWindow>())
            .count();
        let (origin_x, origin_y) = chrome::pin_window_origin(window_width, work_width, existing);
        let bounds = Bounds {
            origin: gpui::point(
                px(work_origin_x + origin_x.max(0.0)),
                px(work_origin_y + origin_y.max(0.0)),
            ),
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
        options.window_min_size = Some(size(
            px(chrome::PIN_MIN_SIZE.min(work_width)),
            px(chrome::PIN_MIN_SIZE.min(work_height)),
        ));
        cx.open_window(options, |window, cx| {
            window.set_window_title("Poratake pinned screenshot");
            crate::system::native::register_dock_window(window.window_handle().window_id(), cx);
            #[cfg(all(target_os = "macos", not(test)))]
            if let Some((ratio_w, ratio_h)) = aspect_ratio {
                super::set_macos_window_aspect_ratio(window, ratio_w, ratio_h);
            }
            #[cfg(all(windows, not(test)))]
            if let Some((ratio_w, ratio_h)) = aspect_ratio {
                install_windows_aspect_ratio(window, f64::from(ratio_w) / f64::from(ratio_h));
            }
            #[cfg(not(any(all(target_os = "macos", not(test)), all(windows, not(test)))))]
            let _ = aspect_ratio;
            let view = cx.new(|_| Self {
                image: render_image.clone(),
                restore,
                hovered: false,
            });
            let weak = view.downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                let _ = weak.update(cx, |view, cx| {
                    let _ = window.drop_image(view.image.clone());
                    view.restore_editor(cx);
                });
                crate::system::memory::reclaim_idle_memory();
                true
            });
            view
        })
        .is_ok()
    }

    fn restore_editor(&mut self, cx: &mut App) {
        let Some((path, state)) = self.restore.take() else {
            return;
        };
        let source = path.clone();
        let image = cx
            .background_executor()
            .spawn(async move { crate::editor::window::PreparedScreenshot::open(&source) });
        cx.spawn(async move |cx| {
            let image = image.await;
            cx.update(|cx| crate::open_editor_with_image(cx, &path, Some(state), image));
        })
        .detach();
    }

    fn close(&mut self, window: &mut Window, cx: &mut App) {
        let _ = window.drop_image(self.image.clone());
        self.restore_editor(cx);
        crate::system::memory::reclaim_idle_memory();
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
                    .size_full()
                    .object_fit(gpui::ObjectFit::Contain),
            );
        #[cfg(not(windows))]
        let image = image.on_mouse_down(gpui::MouseButton::Left, |event, window, cx| {
            if event.click_count != 1 {
                return;
            }
            window.start_window_move();
            cx.stop_propagation();
        });
        #[cfg(target_os = "linux")]
        let image = image.children(linux_resize_handles());
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
                            cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.close(window, cx);
                            }),
                        )
                        .child(crate::ui::icon::icon_element("x", px(14.0))),
                )
            });
        crate::ui::window_root::focus_root(content, window, cx)
    }
}

#[cfg(target_os = "linux")]
fn linux_resize_handles() -> [gpui::AnyElement; 8] {
    use gpui::{CursorStyle, ResizeEdge};

    const EDGE: f32 = 6.0;
    const CORNER: f32 = 10.0;
    let handle = |id: &'static str, edge: ResizeEdge, cursor: CursorStyle| {
        div().id(id).absolute().cursor(cursor).on_mouse_down(
            gpui::MouseButton::Left,
            move |event, window, cx| {
                if event.click_count != 1 {
                    return;
                }
                window.start_window_resize(edge);
                cx.stop_propagation();
            },
        )
    };
    [
        handle("pin-resize-top", ResizeEdge::Top, CursorStyle::ResizeUpDown)
            .top_0()
            .left(px(CORNER))
            .right(px(CORNER))
            .h(px(EDGE))
            .into_any_element(),
        handle(
            "pin-resize-bottom",
            ResizeEdge::Bottom,
            CursorStyle::ResizeUpDown,
        )
        .bottom_0()
        .left(px(CORNER))
        .right(px(CORNER))
        .h(px(EDGE))
        .into_any_element(),
        handle(
            "pin-resize-left",
            ResizeEdge::Left,
            CursorStyle::ResizeLeftRight,
        )
        .left_0()
        .top(px(CORNER))
        .bottom(px(CORNER))
        .w(px(EDGE))
        .into_any_element(),
        handle(
            "pin-resize-right",
            ResizeEdge::Right,
            CursorStyle::ResizeLeftRight,
        )
        .right_0()
        .top(px(CORNER))
        .bottom(px(CORNER))
        .w(px(EDGE))
        .into_any_element(),
        handle(
            "pin-resize-top-left",
            ResizeEdge::TopLeft,
            CursorStyle::ResizeUpLeftDownRight,
        )
        .top_0()
        .left_0()
        .w(px(CORNER))
        .h(px(CORNER))
        .into_any_element(),
        handle(
            "pin-resize-top-right",
            ResizeEdge::TopRight,
            CursorStyle::ResizeUpRightDownLeft,
        )
        .top_0()
        .right_0()
        .w(px(CORNER))
        .h(px(CORNER))
        .into_any_element(),
        handle(
            "pin-resize-bottom-left",
            ResizeEdge::BottomLeft,
            CursorStyle::ResizeUpRightDownLeft,
        )
        .bottom_0()
        .left_0()
        .w(px(CORNER))
        .h(px(CORNER))
        .into_any_element(),
        handle(
            "pin-resize-bottom-right",
            ResizeEdge::BottomRight,
            CursorStyle::ResizeUpLeftDownRight,
        )
        .bottom_0()
        .right_0()
        .w(px(CORNER))
        .h(px(CORNER))
        .into_any_element(),
    ]
}

#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResizeRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(any(windows, test))]
const RESIZE_EDGE_LEFT: u32 = 1;
#[cfg(any(windows, test))]
const RESIZE_EDGE_RIGHT: u32 = 2;
#[cfg(any(windows, test))]
const RESIZE_EDGE_TOP: u32 = 3;
#[cfg(any(windows, test))]
const RESIZE_EDGE_TOP_LEFT: u32 = 4;
#[cfg(any(windows, test))]
const RESIZE_EDGE_TOP_RIGHT: u32 = 5;
#[cfg(any(windows, test))]
const RESIZE_EDGE_BOTTOM: u32 = 6;
#[cfg(any(windows, test))]
const RESIZE_EDGE_BOTTOM_LEFT: u32 = 7;
#[cfg(any(windows, test))]
const RESIZE_EDGE_BOTTOM_RIGHT: u32 = 8;

#[cfg(any(windows, test))]
fn constrain_resize_rect(
    mut rect: ResizeRect,
    edge: u32,
    border_w: i32,
    border_h: i32,
    aspect: f64,
    min_client: i32,
) -> ResizeRect {
    if !aspect.is_finite() || aspect <= 0.0 {
        return rect;
    }
    let min_side = min_client.max(1);
    let min_w = min_side.max((f64::from(min_side) * aspect).ceil() as i32);
    let min_h = min_side.max((f64::from(min_side) / aspect).ceil() as i32);
    let cw = (rect.right - rect.left - border_w).max(min_w);
    let ch = (rect.bottom - rect.top - border_h).max(min_h);

    match edge {
        RESIZE_EDGE_LEFT => {
            let new_h = ((f64::from(cw) / aspect).round() as i32).max(min_h);
            rect.left = rect.right - (cw + border_w);
            rect.bottom = rect.top + (new_h + border_h);
        }
        RESIZE_EDGE_RIGHT => {
            let new_h = ((f64::from(cw) / aspect).round() as i32).max(min_h);
            rect.right = rect.left + (cw + border_w);
            rect.bottom = rect.top + (new_h + border_h);
        }
        RESIZE_EDGE_TOP => {
            let new_w = ((f64::from(ch) * aspect).round() as i32).max(min_w);
            rect.top = rect.bottom - (ch + border_h);
            rect.right = rect.left + (new_w + border_w);
        }
        RESIZE_EDGE_BOTTOM => {
            let new_w = ((f64::from(ch) * aspect).round() as i32).max(min_w);
            rect.bottom = rect.top + (ch + border_h);
            rect.right = rect.left + (new_w + border_w);
        }
        RESIZE_EDGE_TOP_LEFT
        | RESIZE_EDGE_TOP_RIGHT
        | RESIZE_EDGE_BOTTOM_LEFT
        | RESIZE_EDGE_BOTTOM_RIGHT => {
            let (new_w, new_h) = if f64::from(cw) / aspect >= f64::from(ch) {
                (cw, ((f64::from(cw) / aspect).round() as i32).max(min_h))
            } else {
                (((f64::from(ch) * aspect).round() as i32).max(min_w), ch)
            };
            match edge {
                RESIZE_EDGE_TOP_LEFT => {
                    rect.left = rect.right - (new_w + border_w);
                    rect.top = rect.bottom - (new_h + border_h);
                }
                RESIZE_EDGE_TOP_RIGHT => {
                    rect.right = rect.left + (new_w + border_w);
                    rect.top = rect.bottom - (new_h + border_h);
                }
                RESIZE_EDGE_BOTTOM_LEFT => {
                    rect.left = rect.right - (new_w + border_w);
                    rect.bottom = rect.top + (new_h + border_h);
                }
                _ => {
                    rect.right = rect.left + (new_w + border_w);
                    rect.bottom = rect.top + (new_h + border_h);
                }
            }
        }
        _ => {}
    }

    rect
}

#[cfg(all(windows, not(test)))]
const PIN_ASPECT_SUBCLASS_ID: usize = 0x5049_4E41;

#[cfg(all(windows, not(test)))]
fn install_windows_aspect_ratio(window: &Window, aspect: f64) {
    use windows::Win32::UI::Shell::SetWindowSubclass;

    if !aspect.is_finite() || aspect <= 0.0 {
        return;
    }
    let Some(hwnd) = super::window_hwnd(window) else {
        return;
    };
    let boxed = Box::into_raw(Box::new(aspect));
    let installed = unsafe {
        SetWindowSubclass(
            hwnd,
            Some(pin_aspect_subclass),
            PIN_ASPECT_SUBCLASS_ID,
            boxed as usize,
        )
    };
    if !installed.as_bool() {
        drop(unsafe { Box::from_raw(boxed) });
    }
}

#[cfg(all(windows, not(test)))]
unsafe extern "system" fn pin_aspect_subclass(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    uidsubclass: usize,
    dwrefdata: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::{LRESULT, RECT};
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowRect, WM_NCDESTROY, WM_SIZING,
    };

    if msg == WM_NCDESTROY {
        let _ = unsafe { RemoveWindowSubclass(hwnd, Some(pin_aspect_subclass), uidsubclass) };
        if dwrefdata != 0 {
            drop(unsafe { Box::from_raw(dwrefdata as *mut f64) });
        }
        return unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
    }

    if msg == WM_SIZING && dwrefdata != 0 && lparam.0 != 0 {
        let aspect = unsafe { *(dwrefdata as *const f64) };
        let mut window_rect = RECT::default();
        let mut client_rect = RECT::default();
        let (border_w, border_h) = if unsafe {
            GetWindowRect(hwnd, &mut window_rect).is_ok()
                && GetClientRect(hwnd, &mut client_rect).is_ok()
        } {
            (
                (window_rect.right - window_rect.left) - (client_rect.right - client_rect.left),
                (window_rect.bottom - window_rect.top) - (client_rect.bottom - client_rect.top),
            )
        } else {
            (0, 0)
        };
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        let min_client = (chrome::PIN_MIN_SIZE * dpi as f32 / 96.0).round() as i32;
        let rect_ptr = lparam.0 as *mut RECT;
        let current = unsafe { *rect_ptr };
        let updated = constrain_resize_rect(
            ResizeRect {
                left: current.left,
                top: current.top,
                right: current.right,
                bottom: current.bottom,
            },
            wparam.0 as u32,
            border_w,
            border_h,
            aspect,
            min_client,
        );
        unsafe {
            (*rect_ptr).left = updated.left;
            (*rect_ptr).top = updated.top;
            (*rect_ptr).right = updated.right;
            (*rect_ptr).bottom = updated.bottom;
        }
        return LRESULT(1);
    }

    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
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
    fn pin_defaults_to_preview_size_at_top_right() {
        assert_eq!(chrome::PIN_PAD, 0.0);
        assert_eq!(chrome::PIN_MIN_SIZE, 48.0);
        assert_eq!(chrome::PIN_OFFSET, 30.0);
        assert_eq!(
            chrome::pin_window_size(800.0, 600.0, 1920.0, 1080.0),
            (186.0, 140.0)
        );
        assert_eq!(
            chrome::pin_window_size(3000.0, 2000.0, 1920.0, 1080.0),
            (200.0, 133.0)
        );
        assert_eq!(
            chrome::pin_window_size(1920.0, 1080.0, 1920.0, 1080.0),
            (200.0, 112.0)
        );
        assert_eq!(
            chrome::pin_window_size(120.0, 80.0, 1920.0, 1080.0),
            (120.0, 80.0)
        );
        assert_eq!(
            chrome::pin_window_origin(chrome::PREVIEW_WIDTH, 1920.0, 0),
            chrome::preview_origin(0.0, 0.0, 1920.0, 1080.0, 0, chrome::PreviewCorner::TopRight)
        );
        assert_eq!(
            chrome::pin_window_origin(200.0, 1920.0, 2),
            (
                1920.0 - 200.0 - chrome::PREVIEW_MARGIN - 60.0,
                chrome::PREVIEW_MARGIN + 60.0
            )
        );
        assert_eq!(
            chrome::pin_window_aspect_ratio(1920.0, 1080.0, 1920.0, 1080.0),
            Some((1920.0, 1080.0))
        );
        assert_eq!(
            chrome::pin_window_aspect_ratio(1000.0, 10.0, 1920.0, 1080.0),
            None
        );
    }

    #[test]
    fn constrain_resize_rect_preserves_aspect_ratio_and_anchors_opposite_edge() {
        let aspect = 16.0 / 9.0;
        let border_w = 16;
        let border_h = 8;
        let right = constrain_resize_rect(
            ResizeRect {
                left: 100,
                top: 100,
                right: 100 + 320 + border_w,
                bottom: 100 + 112 + border_h,
            },
            RESIZE_EDGE_RIGHT,
            border_w,
            border_h,
            aspect,
            48,
        );
        assert_eq!(right.left, 100);
        assert_eq!(right.top, 100);
        assert_eq!(right.right - right.left - border_w, 320);
        assert_eq!(right.bottom - right.top - border_h, 180);

        let top_left = constrain_resize_rect(
            ResizeRect {
                left: 100,
                top: 100,
                right: 500 + border_w,
                bottom: 300 + border_h,
            },
            RESIZE_EDGE_TOP_LEFT,
            border_w,
            border_h,
            aspect,
            48,
        );
        assert_eq!(top_left.right, 500 + border_w);
        assert_eq!(top_left.bottom, 300 + border_h);
        assert_eq!(top_left.right - top_left.left - border_w, 400);
        assert_eq!(top_left.bottom - top_left.top - border_h, 225);

        let min_clamped = constrain_resize_rect(
            ResizeRect {
                left: 100,
                top: 100,
                right: 120 + border_w,
                bottom: 120 + border_h,
            },
            RESIZE_EDGE_BOTTOM_RIGHT,
            border_w,
            border_h,
            aspect,
            48,
        );
        assert_eq!(min_clamped.left, 100);
        assert_eq!(min_clamped.top, 100);
        assert_eq!(min_clamped.right - min_clamped.left - border_w, 86);
        assert_eq!(min_clamped.bottom - min_clamped.top - border_h, 48);
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
        let image =
            PinWindow::prepare_image(image::load_from_memory(&expected).unwrap().into_rgba8());
        cx.update(|cx| assert!(PinWindow::open_for_editor(cx, image, path.clone(), state)));
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
        let path = directory.path().join("standalone.png");
        image::RgbaImage::from_pixel(320, 180, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        cx.update(|cx| PinWindow::open_file(cx, path));
        cx.run_until_parked();
        let pin = cx.read(|cx| cx.windows()[0]);
        let mut visual = gpui::VisualTestContext::from_window(pin, cx);
        assert!(visual.simulate_close());
        cx.update_window(pin, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        assert!(cx.read(|cx| cx.windows().is_empty()));
    }
}
