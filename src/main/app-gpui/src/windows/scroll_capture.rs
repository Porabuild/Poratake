//! macOS scroll-capture session UI — port of `scroll-capture-window.ts`

use std::sync::Arc;

use gpui::{
    div, img, prelude::*, px, size, AnyElement, App, Bounds, Context, Entity, Render, SharedString,
    Styled, Subscription, Window,
};
use herogpui::components::Button;
use herogpui::gpui;

use crate::capture::overlay::ScreenRect;
use crate::capture::scroll::ScrollSessionSignal;
use crate::daemon::DaemonHandle;
use crate::theme::vars::active_theme;
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::toolbar;

pub const PREVIEW_WIDTH: f32 = 240.0;
pub const PREVIEW_MAX_HEIGHT: f32 = 360.0;
pub const PREVIEW_GAP: f32 = 16.0;
pub const CONTROL_WIDTH: f32 = 168.0;
pub const CONTROL_HEIGHT: f32 = 52.0;
pub const CONTROL_GAP: f32 = 16.0;
const PREVIEW_SCALE: u32 = 2;
const PROMPT_TEXT: &str = "Move cursor here to continue";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FinishState {
    Capturing,
    Finishing,
    Retry,
    Cancelling,
}

pub struct ScrollCaptureUi {
    daemon: DaemonHandle,
    signal: smol::channel::Sender<ScrollSessionSignal>,
    preview: Option<PreviewImage>,
    preview_width: f32,
    preview_max_height: f32,
    cursor_outside: bool,
    auto_scrolling: bool,
    finish_state: FinishState,
    excluded_window_ids: Option<Vec<i64>>,
}

struct PreviewImage {
    image: Arc<gpui::RenderImage>,
    height: f32,
}

impl ScrollCaptureUi {
    fn cursor_prompt_visible(&self) -> bool {
        self.auto_scrolling && self.cursor_outside && self.finish_state == FinishState::Capturing
    }

    fn toggle_auto_scroll(&mut self, cx: &mut App) {
        if self.finish_state != FinishState::Capturing {
            return;
        }
        let client = self.daemon.scroll_capture();
        let result = if self.auto_scrolling {
            client.stop_auto_scroll()
        } else {
            let Some(ids) = self.excluded_window_ids.as_deref() else {
                crate::windows::toast::Toast::show(
                    cx,
                    "Auto-scroll failed",
                    "Could not identify the scroll capture controls",
                );
                return;
            };
            client.start_auto_scroll(ids)
        };
        if let Err(error) = result {
            crate::windows::toast::Toast::show(cx, "Auto-scroll failed", error.to_string());
        }
    }

    fn request_finish(&mut self) {
        if !matches!(
            self.finish_state,
            FinishState::Capturing | FinishState::Retry
        ) {
            return;
        }
        self.finish_state = FinishState::Finishing;
        let _ = self.signal.try_send(ScrollSessionSignal::Finish);
    }

    fn request_cancel(&mut self) {
        if self.finish_state == FinishState::Cancelling {
            return;
        }
        self.finish_state = FinishState::Cancelling;
        let _ = self.signal.try_send(ScrollSessionSignal::Cancel);
    }

    #[cfg(target_os = "macos")]
    fn mark_finishing(&mut self) {
        if self.finish_state == FinishState::Capturing {
            self.finish_state = FinishState::Finishing;
        }
    }

    #[cfg(target_os = "macos")]
    fn mark_cancelling(&mut self) {
        self.finish_state = FinishState::Cancelling;
    }

    #[cfg(any(target_os = "macos", test))]
    fn mark_retryable(&mut self) {
        if self.finish_state == FinishState::Finishing {
            self.finish_state = FinishState::Retry;
        }
    }

    fn apply_frame(&mut self, image: Option<PreviewImage>) {
        if image.is_some() {
            self.preview = image;
        }
    }
}

pub struct ScrollCaptureSession;

#[cfg(all(target_os = "macos", feature = "parity"))]
pub(crate) fn excluded_window_ids_for_parity(cx: &mut App) -> Option<Vec<i64>> {
    let control = cx
        .windows()
        .into_iter()
        .find_map(|handle| handle.downcast::<ScrollControlBar>())?;
    control
        .update(cx, |control, _, cx| {
            control.ui.read(cx).excluded_window_ids.clone()
        })
        .ok()
        .flatten()
}

impl ScrollCaptureSession {
    #[cfg(target_os = "macos")]
    pub fn include_selector_windows(
        ui: &Entity<ScrollCaptureUi>,
        selectors: Vec<i64>,
        cx: &mut App,
    ) {
        ui.update(cx, |ui, _| {
            let Some(ids) = ui.excluded_window_ids.as_mut() else {
                return;
            };
            let valid = !selectors.is_empty()
                && selectors.iter().all(|id| *id > 0 && !ids.contains(id))
                && selectors
                    .iter()
                    .enumerate()
                    .all(|(index, id)| !selectors[..index].contains(id));
            if valid {
                ids.extend(selectors);
            } else {
                ui.excluded_window_ids = None;
            }
        });
    }

    #[cfg(any(target_os = "macos", test))]
    pub fn open(
        cx: &mut App,
        daemon: DaemonHandle,
        area: ScreenRect,
        signal: smol::channel::Sender<ScrollSessionSignal>,
    ) -> Entity<ScrollCaptureUi> {
        let display = crate::windows::recording_control::display_for_rect(cx, area);
        let display_bounds = display
            .as_ref()
            .map(|display| crate::system::work_area::display_bounds(display.as_ref()))
            .unwrap_or(Bounds {
                origin: gpui::point(px(0.0), px(0.0)),
                size: size(px(1920.0), px(1080.0)),
            });
        let platform_display_id = display.as_ref().map(|display| display.id());
        Self::open_on_display(
            cx,
            daemon,
            area,
            signal,
            display_bounds,
            platform_display_id,
            true,
        )
    }

    fn open_on_display(
        cx: &mut App,
        daemon: DaemonHandle,
        area: ScreenRect,
        signal: smol::channel::Sender<ScrollSessionSignal>,
        display_bounds: Bounds<gpui::Pixels>,
        platform_display_id: Option<gpui::DisplayId>,
        show: bool,
    ) -> Entity<ScrollCaptureUi> {
        let work_area = platform_display_id
            .and_then(|id| cx.displays().into_iter().find(|display| display.id() == id))
            .map(|display| crate::system::work_area::work_area(display.as_ref()))
            .unwrap_or(display_bounds);
        let (control_bounds, preview_bounds, prompt_bounds) =
            session_bounds(area, display_bounds, work_area);
        let preview_width = f32::from(preview_bounds.size.width);
        let preview_max_height = f32::from(preview_bounds.size.height);
        let preview_offset = f32::from(prompt_bounds.origin.y - preview_bounds.origin.y);
        let ui = cx.new(|_| ScrollCaptureUi {
            daemon,
            signal,
            preview: None,
            preview_width,
            preview_max_height,
            cursor_outside: false,
            auto_scrolling: false,
            finish_state: FinishState::Capturing,
            excluded_window_ids: None,
        });

        #[cfg(all(target_os = "macos", not(test)))]
        let mut native_ids = Vec::with_capacity(3);
        let preview_entity = ui.clone();
        let _ = cx.open_window(
            super::popup_window_options(
                preview_bounds,
                super::PopupWindowConfig {
                    focus: false,
                    show,
                    display_id: platform_display_id,
                    ..Default::default()
                },
            ),
            |window, cx| {
                super::remove_popup_frame(window);
                crate::system::click_through::enable(window);
                #[cfg(all(target_os = "macos", not(test)))]
                native_ids.push(super::macos_window_id(window));
                let view_entity = preview_entity.clone();
                cx.new(|cx| {
                    let _subscription = observe_ui(cx, &view_entity);
                    ScrollPreviewPanel {
                        ui: view_entity,
                        offset: preview_offset,
                        max_height: preview_max_height,
                        _subscription,
                    }
                })
            },
        );
        let prompt_entity = ui.clone();
        let _ = cx.open_window(
            super::popup_window_options(
                prompt_bounds,
                super::PopupWindowConfig {
                    focus: false,
                    show,
                    display_id: platform_display_id,
                    ..Default::default()
                },
            ),
            |window, cx| {
                super::remove_popup_frame(window);
                crate::system::click_through::enable(window);
                #[cfg(all(target_os = "macos", not(test)))]
                native_ids.push(super::macos_window_id(window));
                let view_entity = prompt_entity.clone();
                cx.new(|cx| {
                    let _subscription = observe_ui(cx, &view_entity);
                    ScrollAreaPrompt {
                        ui: view_entity,
                        _subscription,
                    }
                })
            },
        );
        let control_entity = ui.clone();
        let _ = cx.open_window(
            super::popup_window_options(
                control_bounds,
                super::PopupWindowConfig {
                    focus: false,
                    show,
                    display_id: platform_display_id,
                    ..Default::default()
                },
            ),
            |window, cx| {
                super::remove_popup_frame(window);
                #[cfg(all(target_os = "macos", not(test)))]
                native_ids.push(super::macos_window_id(window));
                let view_entity = control_entity.clone();
                cx.new(|cx| {
                    let _subscription = observe_ui(cx, &view_entity);
                    ScrollControlBar {
                        ui: view_entity,
                        hovered_tooltip: None,
                        #[cfg(target_os = "macos")]
                        pointer_inside: false,
                        _subscription,
                    }
                })
            },
        );
        #[cfg(all(target_os = "macos", not(test)))]
        let excluded_window_ids = {
            #[cfg(feature = "parity")]
            if std::env::var_os(crate::ui::parity::daemon_fixture::PROCESS_ENV).is_some() {
                native_ids = vec![Some(7201), Some(7202), Some(7203)];
            }
            let ids: Option<Vec<i64>> = native_ids.into_iter().collect();
            ids.filter(|ids| ids.len() == 3 && ids.iter().all(|id| *id > 0))
                .filter(|ids| ids[0] != ids[1] && ids[0] != ids[2] && ids[1] != ids[2])
        };
        #[cfg(any(not(target_os = "macos"), test))]
        let excluded_window_ids = Some(Vec::new());
        ui.update(cx, |ui, _| ui.excluded_window_ids = excluded_window_ids);
        set_session_shortcuts(true, cx);
        ui
    }

    pub fn close(cx: &mut App) {
        set_session_shortcuts(false, cx);
        for window_handle in cx.windows() {
            if let Some(handle) = window_handle.downcast::<ScrollPreviewPanel>() {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            } else if let Some(handle) = window_handle.downcast::<ScrollAreaPrompt>() {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            } else if let Some(handle) = window_handle.downcast::<ScrollControlBar>() {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
        }
    }

    pub fn finish_requested(cx: &mut App) {
        Self::signal_from_bar(cx, ScrollCaptureUi::request_finish);
    }

    pub fn cancel_requested(cx: &mut App) {
        Self::signal_from_bar(cx, ScrollCaptureUi::request_cancel);
    }

    #[cfg(target_os = "macos")]
    pub fn mark_finishing(ui: &Entity<ScrollCaptureUi>, cx: &mut App) {
        ui.update(cx, |ui, cx| {
            ui.mark_finishing();
            cx.notify();
        });
    }

    #[cfg(target_os = "macos")]
    pub fn mark_cancelling(ui: &Entity<ScrollCaptureUi>, cx: &mut App) {
        ui.update(cx, |ui, cx| {
            ui.mark_cancelling();
            cx.notify();
        });
    }

    #[cfg(any(target_os = "macos", test))]
    pub fn mark_retryable(ui: &Entity<ScrollCaptureUi>, cx: &mut App) {
        ui.update(cx, |ui, cx| {
            ui.mark_retryable();
            cx.notify();
        });
    }

    fn signal_from_bar(cx: &mut App, send: fn(&mut ScrollCaptureUi)) {
        for window_handle in cx.windows() {
            let Some(handle) = window_handle.downcast::<ScrollControlBar>() else {
                continue;
            };
            let _ = handle.update(cx, |bar, _, cx| {
                bar.ui.update(cx, |ui, cx| {
                    send(ui);
                    cx.notify();
                });
            });
            return;
        }
    }
}

fn observe_ui<V: 'static>(cx: &mut Context<V>, ui: &Entity<ScrollCaptureUi>) -> Subscription {
    cx.observe(ui, |_, _, cx| cx.notify())
}

#[cfg(all(feature = "parity", target_os = "macos", not(test)))]
pub(crate) fn native_control_for_probe(cx: &mut App) -> Option<gpui::AnyWindowHandle> {
    cx.windows()
        .into_iter()
        .find(|handle| handle.downcast::<ScrollControlBar>().is_some())
}

#[cfg(all(feature = "parity", target_os = "macos", not(test)))]
pub(crate) fn open_native_control_probe(cx: &mut App) -> gpui::AnyWindowHandle {
    let display = cx.primary_display().expect("primary display");
    let (signal, _receiver) = smol::channel::unbounded();
    ScrollCaptureSession::open_on_display(
        cx,
        crate::state::state(cx).daemon,
        ScreenRect {
            x: 200,
            y: 200,
            width: 400,
            height: 300,
        },
        signal,
        crate::system::work_area::display_bounds(display.as_ref()),
        Some(display.id()),
        true,
    );
    native_control_for_probe(cx).expect("scroll control")
}

#[cfg(all(feature = "parity", target_os = "macos", not(test)))]
pub(crate) fn native_scroll_exclusion_probe(cx: &mut App) {
    use base64::Engine;

    let display = cx.primary_display().expect("primary display");
    let work = crate::system::work_area::work_area(display.as_ref());
    let width = f32::from(work.size.width) as i32;
    let fixture_width = std::env::var("PORATAKE_NATIVE_SCROLL_FIXTURE_WIDTH")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|requested| *requested >= PREVIEW_WIDTH as i32 && *requested < width);
    let area = ScreenRect {
        x: f32::from(work.left()) as i32,
        y: f32::from(work.top()) as i32,
        width: fixture_width.unwrap_or(width),
        height: f32::from(work.size.height) as i32,
    };
    let (signal, _receiver) = smol::channel::bounded(1);
    let ui = ScrollCaptureSession::open(cx, crate::state::state(cx).daemon, area, signal);
    if fixture_width.is_some() {
        cx.windows()
            .into_iter()
            .find_map(|handle| handle.downcast::<ScrollPreviewPanel>())
            .expect("scroll preview window")
            .update(cx, |_, window, cx| {
                let mut bounds = super::desktop_window_bounds(window);
                bounds.origin.x = px(area.x as f32);
                assert!(super::set_macos_window_bounds(window, bounds));
                window.bounds_changed(cx);
            })
            .unwrap();
    }
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        240,
        180,
        image::Rgba([235, 14, 200, 255]),
    ))
    .write_to(&mut png, image::ImageFormat::Png)
    .unwrap();
    apply_progress(
        &ui,
        ScrollSessionSignal::Frame {
            preview: Some(base64::engine::general_purpose::STANDARD.encode(png.into_inner())),
        },
        cx,
    );
    apply_progress(&ui, ScrollSessionSignal::AutoScrolling(true), cx);
    apply_progress(&ui, ScrollSessionSignal::CursorOutside(true), cx);
    let ids = ui
        .read(cx)
        .excluded_window_ids
        .as_ref()
        .expect("native scroll controls");
    println!(
        "scroll exclusion fixture {} {} {} {} {} {} {}",
        area.x, area.y, area.width, area.height, ids[0], ids[1], ids[2]
    );
    let (done, finished) = smol::channel::bounded(1);
    std::thread::spawn(move || {
        let _ = std::io::stdin().read_line(&mut String::new());
        let _ = done.send_blocking(());
    });
    cx.spawn(async move |cx| {
        let _ = finished.recv().await;
        cx.update(|cx| {
            ScrollCaptureSession::close(cx);
            cx.quit();
        });
    })
    .detach();
}

#[cfg(all(feature = "parity", target_os = "macos", not(test)))]
pub(crate) fn native_window_probe(cx: &mut App, selector_id: Option<i64>) {
    use cocoa::appkit::NSWindowStyleMask;
    use cocoa::base::{id, nil, BOOL, YES};
    use cocoa::foundation::NSRect;
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let display = cx.primary_display().expect("primary display");
    let display_bounds = crate::system::work_area::display_bounds(display.as_ref());
    let work_area = crate::system::work_area::work_area(display.as_ref());
    let area = ScreenRect {
        x: f32::from(display_bounds.origin.x) as i32,
        y: f32::from(display_bounds.origin.y) as i32,
        width: f32::from(display_bounds.size.width) as i32,
        height: f32::from(display_bounds.size.height) as i32,
    };
    let (control, preview, prompt) = session_bounds(area, display_bounds, work_area);
    let (signal, _receiver) = smol::channel::bounded(1);
    let ui = ScrollCaptureSession::open_on_display(
        cx,
        crate::state::state(cx).daemon,
        area,
        signal,
        display_bounds,
        Some(display.id()),
        false,
    );
    if let Some(selector_id) = selector_id {
        ScrollCaptureSession::include_selector_windows(&ui, vec![selector_id], cx);
    }
    let excluded_window_ids = ui
        .read(cx)
        .excluded_window_ids
        .clone()
        .expect("scroll control IDs");
    assert_eq!(
        excluded_window_ids.len(),
        3 + usize::from(selector_id.is_some())
    );
    if let Some(selector_id) = selector_id {
        assert_eq!(excluded_window_ids.last(), Some(&selector_id));
        println!(
            "scroll exclusions: three native control IDs and retained native selector ID match"
        );
    }
    for (index, (name, expected, click_through)) in [
        ("preview", preview, true),
        ("prompt", prompt, true),
        ("control", control, false),
    ]
    .into_iter()
    .enumerate()
    {
        let handle = cx
            .windows()
            .into_iter()
            .find(|handle| match name {
                "preview" => handle.downcast::<ScrollPreviewPanel>().is_some(),
                "prompt" => handle.downcast::<ScrollAreaPrompt>().is_some(),
                "control" => handle.downcast::<ScrollControlBar>().is_some(),
                _ => false,
            })
            .expect("scroll window");
        cx.update_window(handle, |_, window, _| {
            assert_eq!(super::macos_window_id(window), Some(excluded_window_ids[index]));
            let handle = HasWindowHandle::window_handle(window).unwrap();
            let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
                panic!("expected AppKit window");
            };
            let native: id = unsafe { msg_send![appkit.ns_view.as_ptr() as id, window] };
            assert_ne!(native, nil);
            let style: NSWindowStyleMask = unsafe { msg_send![native, styleMask] };
            let shadow: BOOL = unsafe { msg_send![native, hasShadow] };
            let visible: BOOL = unsafe { msg_send![native, isVisible] };
            let ignores: BOOL = unsafe { msg_send![native, ignoresMouseEvents] };
            let frame: NSRect = unsafe { msg_send![native, frame] };
            assert!(!style.contains(NSWindowStyleMask::NSTitledWindowMask));
            assert_ne!(shadow, YES);
            assert_ne!(visible, YES);
            assert_eq!(ignores == YES, click_through);
            let expected_x = f32::from(display_bounds.origin.x + expected.origin.x) as f64;
            let expected_y = f32::from(display_bounds.size.height
                - display_bounds.origin.y
                - expected.origin.y
                - expected.size.height) as f64;
            assert_eq!(frame.origin.x, expected_x, "{name} x");
            assert_eq!(frame.origin.y, expected_y, "{name} y");
            assert_eq!(frame.size.width, f32::from(expected.size.width) as f64, "{name} width");
            assert_eq!(frame.size.height, f32::from(expected.size.height) as f64, "{name} height");
            if name != "prompt" {
                let bounds = super::desktop_window_bounds(window);
                assert!(
                    bounds.left() >= work_area.left()
                        && bounds.top() >= work_area.top()
                        && bounds.right() <= work_area.right()
                        && bounds.bottom() <= work_area.bottom(),
                    "scroll {name} bounds {bounds:?} must fit the work area {work_area:?}"
                );
            }
            println!("scroll {name}: borderless, hidden, click-through={click_through}, frame and exclusion ID match");
        })
        .unwrap();
    }
    ScrollCaptureSession::close(cx);
}

fn set_session_shortcuts(enabled: bool, cx: &App) {
    #[cfg(target_os = "macos")]
    {
        let Some(native) = crate::state::try_native(cx) else {
            return;
        };
        native.send(crate::system::native::NativeCommand::SetScrollCaptureShortcuts(enabled));
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (enabled, cx);
}

pub fn apply_progress(ui: &Entity<ScrollCaptureUi>, signal: ScrollSessionSignal, cx: &mut App) {
    match signal {
        ScrollSessionSignal::Frame { preview } => {
            let state = ui.read(cx);
            let image = preview.as_deref().and_then(|preview| {
                decode_preview(preview, state.preview_width, state.preview_max_height)
            });
            ui.update(cx, |ui, cx| {
                ui.apply_frame(image);
                cx.notify();
            });
        }
        ScrollSessionSignal::AutoScrolling(scrolling) => {
            ui.update(cx, |ui, cx| {
                ui.auto_scrolling = scrolling;
                if !scrolling {
                    ui.cursor_outside = false;
                }
                cx.notify();
            });
        }
        ScrollSessionSignal::CursorOutside(outside) => {
            ui.update(cx, |ui, cx| {
                ui.cursor_outside = outside;
                cx.notify();
            });
        }
        ScrollSessionSignal::Finish
        | ScrollSessionSignal::Cancel
        | ScrollSessionSignal::Stopped(_) => {}
    }
}

#[cfg(all(windows, feature = "parity"))]
pub(crate) fn preview_for_parity(cx: &mut App, encoded: String) -> gpui::AnyWindowHandle {
    let (signal, events) = smol::channel::bounded(2);
    let ui = ScrollCaptureSession::open_on_display(
        cx,
        crate::state::state(cx).daemon,
        ScreenRect {
            x: 100,
            y: 80,
            width: 80,
            height: 20,
        },
        signal,
        Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(200.0), px(180.0)),
        },
        None,
        true,
    );
    apply_progress(
        &ui,
        ScrollSessionSignal::Frame {
            preview: Some(encoded),
        },
        cx,
    );
    apply_progress(&ui, ScrollSessionSignal::AutoScrolling(true), cx);
    apply_progress(&ui, ScrollSessionSignal::CursorOutside(true), cx);
    assert!(ui.read(cx).auto_scrolling && ui.read(cx).cursor_outside);
    apply_progress(&ui, ScrollSessionSignal::CursorOutside(false), cx);
    ScrollCaptureSession::finish_requested(cx);
    ScrollCaptureSession::cancel_requested(cx);
    assert!(matches!(events.try_recv(), Ok(ScrollSessionSignal::Finish)));
    assert!(matches!(events.try_recv(), Ok(ScrollSessionSignal::Cancel)));
    cx.windows()
        .into_iter()
        .find(|handle| handle.downcast::<ScrollPreviewPanel>().is_some())
        .unwrap()
}

struct ScrollPreviewPanel {
    ui: Entity<ScrollCaptureUi>,
    offset: f32,
    max_height: f32,
    _subscription: Subscription,
}

impl Render for ScrollPreviewPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.ui.read(cx);
        let mut panel = crate::ui::font::root().size_full().relative();
        if let Some(image) = &state.preview {
            let top = self.offset.clamp(0.0, self.max_height - image.height);
            panel = panel.child(
                div()
                    .debug_selector(|| "scroll-preview-image".to_string())
                    .absolute()
                    .top(px(top))
                    .w(px(state.preview_width))
                    .h(px(image.height))
                    .overflow_hidden()
                    .rounded(px(chrome::RADIUS_LG))
                    .bg(crate::ui::colors::black(0.4))
                    .shadow_2xl()
                    .child(
                        img(image.image.clone())
                            .w(px(state.preview_width))
                            .h(px(image.height))
                            .rounded(px(chrome::RADIUS_LG)),
                    ),
            );
        }
        panel.into_any_element()
    }
}

struct ScrollAreaPrompt {
    ui: Entity<ScrollCaptureUi>,
    _subscription: Subscription,
}

impl Render for ScrollAreaPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let show_prompt = self.ui.read(cx).cursor_prompt_visible();
        crate::ui::font::root()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .children(show_prompt.then(|| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .rounded(px(chrome::RADIUS_LG))
                    .bg(crate::ui::colors::black(0.75))
                    .px(px(12.0))
                    .py(px(8.0))
                    .text_size(px(14.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(crate::ui::colors::white(1.0))
                    .child(icon_element("mouse-pointer-click", px(16.0)))
                    .child(PROMPT_TEXT)
            }))
    }
}

struct ScrollControlBar {
    ui: Entity<ScrollCaptureUi>,
    hovered_tooltip: Option<(&'static str, SharedString)>,
    #[cfg(target_os = "macos")]
    pointer_inside: bool,
    _subscription: Subscription,
}

impl ScrollControlBar {
    fn update_native_tooltip(
        &mut self,
        id: &'static str,
        tooltip: &SharedString,
        hovered: bool,
        window: &Window,
    ) {
        if !hovered {
            if self
                .hovered_tooltip
                .as_ref()
                .is_some_and(|(current, _)| *current == id)
            {
                set_native_tooltip(window, None);
                self.hovered_tooltip = None;
            }
            return;
        }
        #[cfg(target_os = "macos")]
        if !self.pointer_inside {
            return;
        }
        set_native_tooltip(window, Some(tooltip.as_ref()));
        self.hovered_tooltip = Some((id, tooltip.clone()));
    }

    fn native_tooltip_button(
        id: &'static str,
        button: Button,
        tooltip: SharedString,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hover_tooltip = tooltip.clone();
        let trigger = div()
            .id((gpui::ElementId::from("scroll-native-tooltip"), id))
            .on_hover(cx.listener(move |this, hovered: &bool, window, _| {
                this.update_native_tooltip(id, &hover_tooltip, *hovered, window);
            }))
            .child(button.on_press(cx.listener(move |this, _, window, cx| {
                on_click(this, window, cx);
            })));
        #[cfg(target_os = "macos")]
        let trigger = trigger.on_mouse_move(cx.listener(
            move |this, event: &gpui::MouseMoveEvent, window, _| {
                if event.pressed_button.is_none()
                    && this
                        .hovered_tooltip
                        .as_ref()
                        .is_none_or(|(current, _)| *current != id)
                {
                    this.update_native_tooltip(id, &tooltip, true, window);
                }
            },
        ));
        trigger.into_any_element()
    }

    fn button(
        id: &'static str,
        icon: &'static str,
        tooltip: impl Into<SharedString>,
        disabled: bool,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tooltip: SharedString = tooltip.into();
        Self::native_tooltip_button(
            id,
            toolbar::icon(id, icon, tooltip.clone()).is_disabled(disabled),
            tooltip,
            on_click,
            cx,
        )
    }

    fn filled_button(
        id: &'static str,
        glyph: AnyElement,
        tooltip: impl Into<SharedString>,
        disabled: bool,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tooltip: SharedString = tooltip.into();
        Self::native_tooltip_button(
            id,
            toolbar::button(id, tooltip.clone())
                .is_disabled(disabled)
                .child(glyph),
            tooltip,
            on_click,
            cx,
        )
    }
}

impl Render for ScrollControlBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if window.bounds().size != size(px(CONTROL_WIDTH), px(CONTROL_HEIGHT)) {
            window.resize(size(px(CONTROL_WIDTH), px(CONTROL_HEIGHT)));
        }
        let theme = active_theme(cx);
        let state = self.ui.read(cx);
        let auto_scrolling = state.auto_scrolling;
        let finish_state = state.finish_state;
        let finishing = finish_state == FinishState::Finishing;
        let retryable = finish_state == FinishState::Retry;
        let cancelling = finish_state == FinishState::Cancelling;
        let toggle_tooltip: SharedString = if auto_scrolling {
            "Stop auto-scroll".into()
        } else {
            "Start auto-scroll".into()
        };
        let refresh_tooltip = self.hovered_tooltip.as_ref().is_some_and(|(id, tooltip)| {
            *id == "scroll-toggle-auto-scroll" && *tooltip != toggle_tooltip
        });
        if refresh_tooltip {
            self.hovered_tooltip = Some(("scroll-toggle-auto-scroll", toggle_tooltip.clone()));
            set_native_tooltip(window, Some(toggle_tooltip.as_ref()));
        }
        let glyph_color = crate::ui::colors::white(0.85);
        let toggle_glyph = if auto_scrolling {
            toolbar::filled_glyph(glyph_color, false)
        } else {
            toolbar::filled_play(glyph_color)
        };
        let content = crate::ui::font::root()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                toolbar::surface(&theme)
                    .child(Self::filled_button(
                        "scroll-toggle-auto-scroll",
                        toggle_glyph,
                        toggle_tooltip,
                        finishing || retryable || cancelling,
                        |this, _, cx| {
                            this.ui.update(cx, |ui, cx| ui.toggle_auto_scroll(cx));
                        },
                        cx,
                    ))
                    .child(Self::button(
                        "scroll-done",
                        "check",
                        if finishing {
                            "Finishing…"
                        } else if retryable {
                            "Retry"
                        } else {
                            "Done (Enter)"
                        },
                        finishing || cancelling,
                        |this, _, cx| {
                            this.ui.update(cx, |ui, _| ui.request_finish());
                        },
                        cx,
                    ))
                    .child(Self::button(
                        "scroll-cancel",
                        "x",
                        if cancelling {
                            "Canceling…"
                        } else {
                            "Cancel (Esc)"
                        },
                        cancelling,
                        |this, _, cx| {
                            this.ui.update(cx, |ui, _| ui.request_cancel());
                        },
                        cx,
                    )),
            );
        #[cfg(target_os = "macos")]
        let content = {
            let control = cx.entity().downgrade();
            let pointer_listener = gpui::canvas(
                |_, _, _| (),
                move |_, (), window, _| {
                    let entering = control.clone();
                    window.on_mouse_event(move |_: &gpui::MouseMoveEvent, phase, _, cx| {
                        if phase == gpui::DispatchPhase::Capture {
                            let _ = entering.update(cx, |this, _| this.pointer_inside = true);
                        }
                    });
                    let exiting = control.clone();
                    window.on_mouse_event(move |_: &gpui::MouseExitEvent, phase, window, cx| {
                        if phase == gpui::DispatchPhase::Capture {
                            let _ = exiting.update(cx, |this, _| {
                                this.pointer_inside = false;
                                this.hovered_tooltip = None;
                                set_native_tooltip(window, None);
                            });
                        }
                    });
                },
            )
            .absolute()
            .inset_0();
            content.relative().child(pointer_listener)
        };
        crate::ui::window_root::focus_root(content, window, cx)
    }
}

#[cfg(all(target_os = "macos", not(test)))]
pub(crate) fn set_native_tooltip(window: &Window, tooltip: Option<&str>) {
    crate::system::macos_tooltip::set(window, tooltip);
}

#[cfg(test)]
thread_local! {
    static TEST_NATIVE_TOOLTIP: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_native_tooltip(_window: &Window, tooltip: Option<&str>) {
    TEST_NATIVE_TOOLTIP.with_borrow_mut(|value| *value = tooltip.map(str::to_owned));
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) fn test_native_tooltip() -> Option<String> {
    TEST_NATIVE_TOOLTIP.with_borrow(Clone::clone)
}

#[cfg(all(not(target_os = "macos"), not(test)))]
fn set_native_tooltip(_window: &Window, _tooltip: Option<&str>) {}

fn session_bounds(
    area: ScreenRect,
    display: Bounds<gpui::Pixels>,
    work_area: Bounds<gpui::Pixels>,
) -> (
    Bounds<gpui::Pixels>,
    Bounds<gpui::Pixels>,
    Bounds<gpui::Pixels>,
) {
    let work_origin = work_area.origin - display.origin;
    let selection = (
        area.x as f32 - f32::from(work_area.origin.x),
        area.y as f32 - f32::from(work_area.origin.y),
        area.width as f32,
        area.height as f32,
    );
    let work_size = (
        f32::from(work_area.size.width),
        f32::from(work_area.size.height),
    );
    let (control_x, control_y) = control_origin(selection, work_size);
    let preview_width = PREVIEW_WIDTH.min(work_size.0);
    let preview_height = PREVIEW_MAX_HEIGHT.min(work_size.1);
    (
        Bounds {
            origin: work_origin + gpui::point(px(control_x), px(control_y)),
            size: size(px(CONTROL_WIDTH), px(CONTROL_HEIGHT)),
        },
        Bounds {
            origin: work_origin
                + gpui::point(
                    px(preview_left(selection, work_size.0)),
                    px(selection.1.clamp(0.0, work_size.1 - preview_height)),
                ),
            size: size(px(preview_width), px(preview_height)),
        },
        Bounds {
            origin: work_origin + gpui::point(px(selection.0), px(selection.1)),
            size: size(px(selection.2), px(selection.3)),
        },
    )
}

fn control_origin(area: (f32, f32, f32, f32), display: (f32, f32)) -> (f32, f32) {
    let center_x = area.0 + area.2 / 2.0;
    let preferred_top = area.1 + area.3 + CONTROL_GAP;
    let top = if preferred_top + CONTROL_HEIGHT <= display.1 {
        preferred_top
    } else {
        area.1 - CONTROL_GAP - CONTROL_HEIGHT
    };
    (
        (center_x - CONTROL_WIDTH / 2.0).clamp(0.0, (display.0 - CONTROL_WIDTH).max(0.0)),
        top.clamp(0.0, (display.1 - CONTROL_HEIGHT).max(0.0)),
    )
}

fn preview_left(area: (f32, f32, f32, f32), display_width: f32) -> f32 {
    let preview_width = PREVIEW_WIDTH.min(display_width);
    let space_right = display_width - (area.0 + area.2);
    let left = if space_right >= preview_width + PREVIEW_GAP {
        area.0 + area.2 + PREVIEW_GAP
    } else {
        area.0 - preview_width - PREVIEW_GAP
    };
    left.clamp(0.0, display_width - preview_width)
}

fn preview_image_height(
    decoded_width: u32,
    decoded_height: u32,
    preview_width: f32,
    max_height: f32,
) -> f32 {
    if decoded_width == 0 || decoded_height == 0 {
        return 0.0;
    }
    (preview_width * decoded_height as f32 / decoded_width as f32).min(max_height)
}

fn decode_preview(encoded: &str, preview_width: f32, max_height: f32) -> Option<PreviewImage> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    let decoded = crate::render::images::decode(&bytes).ok()?.into_rgba8();
    let (width, height) = (decoded.width(), decoded.height());
    if width == 0 || height == 0 {
        return None;
    }
    let panel_height = preview_image_height(width, height, preview_width, max_height);
    if panel_height <= 0.0 || preview_width <= 0.0 {
        return None;
    }
    let target_width = (preview_width * PREVIEW_SCALE as f32).round().max(1.0);
    let target_height = (panel_height * PREVIEW_SCALE as f32).round().max(1.0);
    let target_aspect = target_width / target_height;
    let source_aspect = width as f32 / height as f32;
    let cropped = if source_aspect > target_aspect {
        let crop_width = (height as f32 * target_aspect)
            .round()
            .clamp(1.0, width as f32) as u32;
        let x = (width - crop_width) / 2;
        image::imageops::crop_imm(&decoded, x, 0, crop_width, height).to_image()
    } else {
        let crop_height = (width as f32 / target_aspect)
            .round()
            .clamp(1.0, height as f32) as u32;
        let y = height.saturating_sub(crop_height);
        image::imageops::crop_imm(&decoded, 0, y, width, crop_height).to_image()
    };
    let mut buffer = image::DynamicImage::ImageRgba8(cropped)
        .resize_exact(
            target_width as u32,
            target_height as u32,
            image::imageops::FilterType::Triangle,
        )
        .to_rgba8();
    for pixel in buffer.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let frame = image::Frame::new(buffer);
    Some(PreviewImage {
        image: Arc::new(gpui::RenderImage::new(smallvec::smallvec![frame])),
        height: panel_height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use herogpui::test::TestWindowExt;

    #[test]
    fn the_prompt_is_the_line_the_reference_shows() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root")
            .to_path_buf();
        let reference = std::fs::read_to_string(
            root.join("src/renderer/windows/scroll-capture-overlay-window.tsx"),
        )
        .expect("reference scroll overlay");
        assert!(reference.contains(PROMPT_TEXT));
    }

    #[test]
    fn control_bar_goes_below_the_area_when_it_fits() {
        assert_eq!(
            control_origin((100.0, 100.0, 400.0, 300.0), (1920.0, 1080.0)),
            (216.0, 416.0)
        );
    }

    #[test]
    fn control_bar_moves_above_the_area_at_the_bottom_edge() {
        assert_eq!(
            control_origin((100.0, 900.0, 400.0, 150.0), (1920.0, 1080.0)),
            (216.0, 832.0)
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn selection_coordinates_are_relative_to_the_display_without_backing_scale() {
        let display = Bounds {
            origin: gpui::point(px(-1600.0), px(120.0)),
            size: size(px(1600.0), px(900.0)),
        };
        let selection = Bounds {
            origin: gpui::point(px(-1390.0), px(260.0)),
            size: size(px(400.0), px(300.0)),
        };
        for scale in [1.0, 2.0] {
            let area = crate::capture::overlay::physical_rect(selection, scale);
            let (control, preview, prompt) = session_bounds(area, display, display);
            assert_eq!(control.origin, gpui::point(px(326.0), px(456.0)));
            assert_eq!(control.size, size(px(168.0), px(52.0)));
            assert_eq!(preview.origin, gpui::point(px(626.0), px(140.0)));
            assert_eq!(preview.size, size(px(240.0), px(360.0)));
            assert_eq!(prompt.origin, gpui::point(px(210.0), px(140.0)));
            assert_eq!(prompt.size, size(px(400.0), px(300.0)));
        }
    }

    #[test]
    fn control_bar_stays_within_the_display_at_horizontal_edges() {
        assert_eq!(
            control_origin((0.0, 100.0, 10.0, 300.0), (1920.0, 1080.0)),
            (0.0, 416.0)
        );
        assert_eq!(
            control_origin((1910.0, 100.0, 10.0, 300.0), (1920.0, 1080.0)),
            (1752.0, 416.0)
        );
    }

    #[test]
    fn scroll_controls_and_preview_stay_in_an_inset_work_area() {
        let display = Bounds::new(
            gpui::point(px(-1600.0), px(-200.0)),
            size(px(1600.0), px(900.0)),
        );
        let work_area = Bounds::new(
            gpui::point(px(-1540.0), px(-162.0)),
            size(px(1460.0), px(800.0)),
        );
        for (area, control_origin, preview_origin, prompt_origin) in [
            (
                ScreenRect {
                    x: -1600,
                    y: -200,
                    width: 1600,
                    height: 900,
                },
                (716.0, 38.0),
                (60.0, 38.0),
                (0.0, 0.0),
            ),
            (
                ScreenRect {
                    x: -12,
                    y: 560,
                    width: 12,
                    height: 46,
                },
                (1352.0, 692.0),
                (1280.0, 478.0),
                (1588.0, 760.0),
            ),
        ] {
            let (control, preview, prompt) = session_bounds(area, display, work_area);
            assert_eq!(
                control.origin,
                gpui::point(px(control_origin.0), px(control_origin.1))
            );
            assert_eq!(
                preview.origin,
                gpui::point(px(preview_origin.0), px(preview_origin.1))
            );
            assert_eq!(
                prompt.origin,
                gpui::point(px(prompt_origin.0), px(prompt_origin.1))
            );
            assert_eq!(
                prompt.size,
                size(px(area.width as f32), px(area.height as f32))
            );
        }
    }

    #[test]
    fn preview_window_fits_a_narrow_display() {
        let display = Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(200.0), px(180.0)),
        };
        let (_, preview, _) = session_bounds(
            ScreenRect {
                x: 100,
                y: 80,
                width: 80,
                height: 20,
            },
            display,
            display,
        );
        assert_eq!(preview.left(), px(0.0));
        assert_eq!(preview.size.width, px(200.0));
    }

    #[herogpui::test]
    fn narrow_preview_preserves_the_image_aspect(cx: &mut gpui::TestAppContext) {
        use base64::Engine;

        let directory = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        let (signal, _) = smol::channel::bounded(1);
        let ui = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            let daemon = crate::state::state(cx).daemon;
            ScrollCaptureSession::open_on_display(
                cx,
                daemon,
                ScreenRect {
                    x: 100,
                    y: 80,
                    width: 80,
                    height: 20,
                },
                signal,
                Bounds {
                    origin: gpui::point(px(0.0), px(0.0)),
                    size: size(px(200.0), px(180.0)),
                },
                None,
                true,
            )
        });
        let panel = cx.read(|cx| {
            cx.windows()
                .into_iter()
                .find(|handle| handle.downcast::<ScrollPreviewPanel>().is_some())
                .unwrap()
        });
        let mut visual = gpui::VisualTestContext::from_window(panel, cx);
        visual.update(|window, _| {
            assert_eq!(window.bounds().size, size(px(200.0), px(180.0)));
        });
        let mut encoded = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(4, 2))
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        visual.update(|_, cx| {
            apply_progress(
                &ui,
                ScrollSessionSignal::Frame {
                    preview: Some(
                        base64::engine::general_purpose::STANDARD.encode(encoded.into_inner()),
                    ),
                },
                cx,
            );
        });
        let bounds = visual.expect("scroll-preview-image");
        assert_eq!(bounds.top(), px(80.0));
        assert_eq!(bounds.size, size(px(200.0), px(100.0)));
        visual.read(|cx| {
            let decoded = ui.read(cx).preview.as_ref().unwrap();
            assert_eq!(decoded.image.size(0).width.0, 400);
            assert_eq!(decoded.image.size(0).height.0, 200);
        });
        let mut encoded = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(4, 8, |_, y| {
            if y < 4 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 255, 255])
            }
        }))
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
        visual.update(|_, cx| {
            apply_progress(
                &ui,
                ScrollSessionSignal::Frame {
                    preview: Some(
                        base64::engine::general_purpose::STANDARD.encode(encoded.into_inner()),
                    ),
                },
                cx,
            );
        });
        let bounds = visual.expect("scroll-preview-image");
        assert_eq!(bounds.top(), px(0.0));
        assert_eq!(bounds.size, size(px(200.0), px(180.0)));
        visual.read(|cx| {
            let decoded = ui.read(cx).preview.as_ref().unwrap();
            assert_eq!(decoded.image.size(0).height.0, 360);
            assert!(decoded
                .image
                .as_bytes(0)
                .unwrap()
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [255, 0, 0, 255]));
        });
    }

    #[herogpui::test]
    fn growing_preview_stays_visible_on_the_display(cx: &mut gpui::TestAppContext) {
        let (signal, _) = smol::channel::bounded(1);
        let daemon = DaemonHandle::with_binary(std::path::PathBuf::new());
        let (panel, visual) = herogpui::test::open_window(cx, move |_, cx| {
            let ui = cx.new(|_| ScrollCaptureUi {
                daemon,
                signal,
                preview: None,
                preview_width: 240.0,
                preview_max_height: 360.0,
                cursor_outside: false,
                auto_scrolling: false,
                finish_state: FinishState::Capturing,
                excluded_window_ids: Some(Vec::new()),
            });
            ScrollPreviewPanel {
                _subscription: observe_ui(cx, &ui),
                ui,
                offset: 230.0,
                max_height: 360.0,
            }
        });
        visual.simulate_resize(size(px(240.0), px(360.0)));
        let ui = panel.read_with(visual, |panel, _| panel.ui.clone());
        let image = Arc::new(gpui::RenderImage::new(smallvec::smallvec![
            image::Frame::new(image::RgbaImage::new(2, 2))
        ]));
        for (height, top) in [(100.0, 230.0), (180.0, 180.0), (360.0, 0.0)] {
            visual.update(|_, cx| {
                ui.update(cx, |ui, cx| {
                    ui.preview = Some(PreviewImage {
                        image: image.clone(),
                        height,
                    });
                    cx.notify();
                });
            });
            let bounds = visual.expect("scroll-preview-image");
            assert_eq!(bounds.top(), px(top));
            assert_eq!(bounds.size.height, px(height));
        }
    }

    #[herogpui::test]
    fn session_updates_preview_and_closes_all_windows(cx: &mut gpui::TestAppContext) {
        use base64::Engine;

        let directory = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        let displays = cx.read(|cx| {
            cx.displays()
                .iter()
                .map(|display| display.id())
                .collect::<Vec<_>>()
        });
        for display in displays {
            cx.simulate_display_removed(display);
        }
        let (signal, events) = smol::channel::bounded(4);
        let ui = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            let daemon = crate::state::state(cx).daemon;
            ScrollCaptureSession::open(
                cx,
                daemon,
                ScreenRect {
                    x: 100,
                    y: 950,
                    width: 200,
                    height: 20,
                },
                signal,
            )
        });
        let encode = |height| {
            let mut encoded = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image::RgbaImage::new(2, height))
                .write_to(&mut encoded, image::ImageFormat::Png)
                .unwrap();
            base64::engine::general_purpose::STANDARD.encode(encoded.into_inner())
        };
        cx.update(|cx| {
            apply_progress(
                &ui,
                ScrollSessionSignal::Frame {
                    preview: Some(encode(1)),
                },
                cx,
            );
        });
        let preview = cx.read(|cx| {
            cx.windows()
                .into_iter()
                .find(|handle| handle.downcast::<ScrollPreviewPanel>().is_some())
                .unwrap()
        });
        let mut visual = gpui::VisualTestContext::from_window(preview, cx);
        let bounds = visual.expect("scroll-preview-image");
        let window = visual.update(|window, _| window.bounds());
        assert_eq!(window.top() + bounds.top(), px(950.0));
        assert_eq!(bounds.size.height, px(120.0));
        cx.update(|cx| {
            apply_progress(
                &ui,
                ScrollSessionSignal::Frame {
                    preview: Some(encode(4)),
                },
                cx,
            );
            apply_progress(&ui, ScrollSessionSignal::AutoScrolling(true), cx);
            apply_progress(&ui, ScrollSessionSignal::CursorOutside(true), cx);
            let state = ui.read(cx);
            assert!(state.preview.is_some());
            assert!(state.cursor_prompt_visible());
            apply_progress(&ui, ScrollSessionSignal::AutoScrolling(false), cx);
            assert!(
                !ui.read(cx).cursor_outside,
                "stopping must clear the resume-scrolling prompt"
            );
            apply_progress(&ui, ScrollSessionSignal::CursorOutside(true), cx);
            assert!(
                !ui.read(cx).cursor_prompt_visible(),
                "a late cursor event must not restore the stopped prompt"
            );
            apply_progress(&ui, ScrollSessionSignal::AutoScrolling(true), cx);
            apply_progress(&ui, ScrollSessionSignal::CursorOutside(true), cx);
            ScrollCaptureSession::finish_requested(cx);
            assert_eq!(ui.read(cx).finish_state, FinishState::Finishing);
            assert!(!ui.read(cx).cursor_prompt_visible());
            ScrollCaptureSession::mark_retryable(&ui, cx);
            assert_eq!(ui.read(cx).finish_state, FinishState::Retry);
            ScrollCaptureSession::finish_requested(cx);
            assert_eq!(ui.read(cx).finish_state, FinishState::Finishing);
            ScrollCaptureSession::cancel_requested(cx);
            assert_eq!(ui.read(cx).finish_state, FinishState::Cancelling);
        });
        let bounds = visual.expect("scroll-preview-image");
        assert_eq!(window.top() + bounds.top(), px(720.0));
        assert_eq!(bounds.size.height, px(360.0));
        assert!(matches!(events.try_recv(), Ok(ScrollSessionSignal::Finish)));
        assert!(matches!(events.try_recv(), Ok(ScrollSessionSignal::Finish)));
        assert!(matches!(events.try_recv(), Ok(ScrollSessionSignal::Cancel)));
        cx.update(|cx| {
            assert_eq!(
                cx.windows()
                    .into_iter()
                    .filter(|handle| {
                        handle.downcast::<ScrollPreviewPanel>().is_some()
                            || handle.downcast::<ScrollAreaPrompt>().is_some()
                            || handle.downcast::<ScrollControlBar>().is_some()
                    })
                    .count(),
                3
            );
            ScrollCaptureSession::close(cx);
        });
        cx.update(|cx| {
            assert!(!cx.windows().into_iter().any(|handle| {
                handle.downcast::<ScrollPreviewPanel>().is_some()
                    || handle.downcast::<ScrollAreaPrompt>().is_some()
                    || handle.downcast::<ScrollControlBar>().is_some()
            }));
        });
    }

    #[herogpui::test]
    fn control_tooltips_follow_hover_and_auto_scroll_state(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        let (signal, events) = smol::channel::bounded(4);
        TEST_NATIVE_TOOLTIP.with_borrow_mut(|value| *value = None);
        let auto_scroll_calls = crate::daemon::capture_auto_scroll_for_test();
        let notifications = crate::system::notification::capture_for_test();
        let ui = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            let daemon = crate::state::state(cx).daemon;
            ScrollCaptureSession::open(
                cx,
                daemon,
                ScreenRect {
                    x: 100,
                    y: 100,
                    width: 200,
                    height: 200,
                },
                signal,
            )
        });
        let control = cx.read(|cx| {
            cx.windows()
                .into_iter()
                .find_map(|handle| handle.downcast::<ScrollControlBar>())
                .unwrap()
        });
        let mut visual = gpui::VisualTestContext::from_window(control.into(), cx);
        visual.simulate_mouse_move(
            gpui::point(px(50.0), px(26.0)),
            None,
            gpui::Modifiers::none(),
        );
        let tooltip = |cx: &mut gpui::TestAppContext| {
            control
                .update(cx, |control, _, _| control.hovered_tooltip.clone())
                .unwrap()
        };
        assert_eq!(
            tooltip(cx).map(|(_, text)| text),
            Some("Start auto-scroll".into())
        );
        assert_eq!(
            TEST_NATIVE_TOOLTIP.with_borrow(Clone::clone),
            Some("Start auto-scroll".into())
        );
        #[cfg(target_os = "macos")]
        {
            visual.simulate_event(gpui::MouseExitEvent {
                position: gpui::point(px(169.0), px(26.0)),
                pressed_button: None,
                modifiers: gpui::Modifiers::none(),
            });
            visual.settle();
            assert_eq!(TEST_NATIVE_TOOLTIP.with_borrow(Clone::clone), None);
            visual.simulate_mouse_move(
                gpui::point(px(50.0), px(26.0)),
                None,
                gpui::Modifiers::none(),
            );
            visual.settle();
            assert_eq!(
                TEST_NATIVE_TOOLTIP.with_borrow(Clone::clone),
                Some("Start auto-scroll".into())
            );
        }
        #[cfg(target_os = "macos")]
        cx.update(|cx| {
            ui.update(cx, |ui, _| ui.excluded_window_ids = Some(vec![11, 22, 33]));
            ScrollCaptureSession::include_selector_windows(&ui, vec![44, 55], cx);
            assert_eq!(
                ui.read(cx).excluded_window_ids,
                Some(vec![11, 22, 33, 44, 55])
            );
            ScrollCaptureSession::include_selector_windows(&ui, vec![55], cx);
            assert!(ui.read(cx).excluded_window_ids.is_none());
        });
        cx.update(|cx| ui.update(cx, |ui, _| ui.excluded_window_ids = None));
        visual.click_at(gpui::point(px(50.0), px(26.0)));
        assert!(auto_scroll_calls.borrow().is_empty());
        assert_eq!(*notifications.borrow(), ["Auto-scroll failed"]);
        cx.update(|cx| ui.update(cx, |ui, _| ui.excluded_window_ids = Some(vec![11, 22, 33])));
        #[cfg(target_os = "macos")]
        cx.update(|cx| ScrollCaptureSession::include_selector_windows(&ui, vec![44, 55], cx));
        visual.click_at(gpui::point(px(50.0), px(26.0)));
        cx.update(|cx| {
            apply_progress(&ui, ScrollSessionSignal::AutoScrolling(true), cx);
        });
        visual.settle();
        #[cfg(target_os = "macos")]
        let expected_ids = vec![11, 22, 33, 44, 55];
        #[cfg(not(target_os = "macos"))]
        let expected_ids = vec![11, 22, 33];
        assert_eq!(
            tooltip(cx).map(|(_, text)| text),
            Some("Stop auto-scroll".into())
        );
        assert_eq!(
            TEST_NATIVE_TOOLTIP.with_borrow(Clone::clone),
            Some("Stop auto-scroll".into())
        );
        visual.click_at(gpui::point(px(50.0), px(26.0)));
        assert_eq!(
            *auto_scroll_calls.borrow(),
            [
                (
                    poratake_daemon_common::contract::ScrollCaptureMethod::StartAutoScroll,
                    expected_ids.clone()
                ),
                (
                    poratake_daemon_common::contract::ScrollCaptureMethod::StopAutoScroll,
                    vec![]
                ),
            ]
        );
        let (control_tx, control_rx) = smol::channel::unbounded();
        let (progress_tx, progress_rx) = crate::capture::scroll::scroll_progress_channel();
        for _ in 0..64 {
            ScrollSessionSignal::Frame {
                preview: Some("invalid preview".into()),
            }
            .send(&control_tx, &progress_tx);
        }
        ScrollSessionSignal::AutoScrolling(false).send(&control_tx, &progress_tx);
        for _ in 0..2 {
            let signal =
                smol::block_on(ScrollSessionSignal::receive(&control_rx, &progress_rx)).unwrap();
            cx.update(|cx| apply_progress(&ui, signal, cx));
        }
        visual.settle();
        assert_eq!(
            tooltip(cx).map(|(_, text)| text),
            Some("Start auto-scroll".into())
        );
        visual.click_at(gpui::point(px(50.0), px(26.0)));
        assert_eq!(
            auto_scroll_calls.borrow().last(),
            Some(&(
                poratake_daemon_common::contract::ScrollCaptureMethod::StartAutoScroll,
                expected_ids,
            ))
        );
        for (x, expected) in [(84.0, "Done (Enter)"), (118.0, "Cancel (Esc)")] {
            visual.simulate_mouse_move(gpui::point(px(x), px(26.0)), None, gpui::Modifiers::none());
            assert_eq!(tooltip(cx).map(|(_, text)| text), Some(expected.into()));
            assert_eq!(
                TEST_NATIVE_TOOLTIP.with_borrow(Clone::clone),
                Some(expected.into())
            );
            #[cfg(target_os = "macos")]
            {
                visual.simulate_event(gpui::MouseExitEvent {
                    position: gpui::point(px(169.0), px(26.0)),
                    pressed_button: None,
                    modifiers: gpui::Modifiers::none(),
                });
                visual.settle();
                assert_eq!(TEST_NATIVE_TOOLTIP.with_borrow(Clone::clone), None);
                visual.simulate_mouse_move(
                    gpui::point(px(x), px(26.0)),
                    None,
                    gpui::Modifiers::none(),
                );
                visual.settle();
                assert_eq!(
                    TEST_NATIVE_TOOLTIP.with_borrow(Clone::clone),
                    Some(expected.into())
                );
            }
        }
        visual.click_at(gpui::point(px(84.0), px(26.0)));
        assert!(matches!(events.try_recv(), Ok(ScrollSessionSignal::Finish)));
        visual.click_at(gpui::point(px(118.0), px(26.0)));
        assert!(matches!(events.try_recv(), Ok(ScrollSessionSignal::Cancel)));
        visual.simulate_mouse_move(gpui::point(px(0.0), px(0.0)), None, gpui::Modifiers::none());
        assert!(tooltip(cx).is_none());
        assert!(TEST_NATIVE_TOOLTIP.with_borrow(|value| value.is_none()));
    }

    #[test]
    fn preview_panel_prefers_the_right_side() {
        assert_eq!(preview_left((100.0, 100.0, 400.0, 300.0), 1920.0), 516.0);
    }

    #[test]
    fn preview_panel_falls_back_to_the_left_side() {
        assert_eq!(preview_left((1500.0, 100.0, 400.0, 300.0), 1920.0), 1244.0);
    }

    #[test]
    fn preview_window_stays_within_the_display_when_the_image_grows() {
        for (display_y, display_height, area_y, top, height) in [
            (0.0, 1080.0, 100, 100.0, 360.0),
            (0.0, 1080.0, 950, 720.0, 360.0),
            (200.0, 900.0, 1010, 540.0, 360.0),
            (0.0, 200.0, 150, 0.0, 200.0),
        ] {
            let display = Bounds {
                origin: gpui::point(px(0.0), px(display_y)),
                size: size(px(1920.0), px(display_height)),
            };
            let (_, preview, _) = session_bounds(
                ScreenRect {
                    x: 100,
                    y: area_y,
                    width: 200,
                    height: 20,
                },
                display,
                display,
            );
            assert_eq!(preview.top(), px(top));
            assert_eq!(preview.size.height, px(height));
        }
    }

    #[test]
    fn preview_height_follows_the_stitch_aspect() {
        assert_eq!(preview_image_height(480, 360, 240.0, 360.0), 180.0);
        assert_eq!(preview_image_height(240, 720, 240.0, 360.0), 360.0);
        assert_eq!(preview_image_height(240, 720, 240.0, 200.0), 200.0);
        assert_eq!(preview_image_height(0, 100, 240.0, 360.0), 0.0);
    }
}
