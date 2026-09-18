//! macOS scroll-capture session UI — port of `scroll-capture-window.ts`

use std::sync::Arc;

use gpui::{
    div, img, prelude::*, px, size, AnyElement, App, Bounds, Context, Entity, Render, SharedString,
    Styled, Subscription, Window,
};
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

pub struct ScrollCaptureUi {
    daemon: DaemonHandle,
    signal: smol::channel::Sender<ScrollSessionSignal>,
    preview: Option<PreviewImage>,
    cursor_outside: bool,
    auto_scrolling: bool,
}

struct PreviewImage {
    image: Arc<gpui::RenderImage>,
    height: f32,
}

impl ScrollCaptureUi {
    fn toggle_auto_scroll(&mut self, cx: &mut App) {
        let client = self.daemon.scroll_capture();
        let result = if self.auto_scrolling {
            client.stop_auto_scroll()
        } else {
            client.start_auto_scroll()
        };
        if let Err(error) = result {
            crate::windows::toast::Toast::show(cx, "Auto-scroll failed", error.to_string());
        }
    }

    fn request_finish(&self) {
        let _ = self.signal.try_send(ScrollSessionSignal::Finish);
    }

    fn request_cancel(&self) {
        let _ = self.signal.try_send(ScrollSessionSignal::Cancel);
    }

    fn apply_frame(&mut self, image: Option<PreviewImage>) {
        if image.is_some() {
            self.preview = image;
        }
    }
}

pub struct ScrollCaptureSession;

impl ScrollCaptureSession {
    pub fn open(
        cx: &mut App,
        daemon: DaemonHandle,
        area: ScreenRect,
        signal: smol::channel::Sender<ScrollSessionSignal>,
    ) -> Entity<ScrollCaptureUi> {
        let ui = cx.new(|_| ScrollCaptureUi {
            daemon,
            signal,
            preview: None,
            cursor_outside: false,
            auto_scrolling: false,
        });
        let display = crate::windows::recording_control::display_for_rect(cx, area);
        let scale = display
            .as_ref()
            .map(|display| crate::capture::overlay::display_scale_factor(display.as_ref(), cx))
            .unwrap_or(1.0)
            .max(1.0);
        let logical = logical_rect(area, scale);
        let display_bounds = display
            .as_ref()
            .map(|display| crate::system::work_area::display_bounds(display.as_ref()));
        let platform_display_id = display.as_ref().map(|display| display.id());
        let (origin_x, origin_y, bounds_width, bounds_height) = display_bounds
            .map(|bounds| {
                (
                    f32::from(bounds.origin.x),
                    f32::from(bounds.origin.y),
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height),
                )
            })
            .unwrap_or((logical.0, logical.1, 1920.0, 1080.0));

        let (control_x, control_y) = control_origin(logical, (bounds_width, bounds_height));
        let control_bounds = Bounds {
            origin: gpui::point(px(origin_x + control_x), px(origin_y + control_y)),
            size: size(px(CONTROL_WIDTH), px(CONTROL_HEIGHT)),
        };
        let preview_left = preview_left(logical, bounds_width);
        let preview_bounds = Bounds {
            origin: gpui::point(px(origin_x + preview_left), px(origin_y + logical.1)),
            size: size(px(PREVIEW_WIDTH), px(1.0)),
        };
        let prompt_bounds = Bounds {
            origin: gpui::point(px(origin_x + logical.0), px(origin_y + logical.1)),
            size: size(px(logical.2), px(logical.3)),
        };
        let to_local = |bounds: Bounds<gpui::Pixels>| {
            display
                .as_ref()
                .map(|display| {
                    crate::system::work_area::local_window_bounds(bounds, display.as_ref())
                })
                .unwrap_or(bounds)
        };

        let preview_entity = ui.clone();
        let _ = cx.open_window(
            super::popup_window_options(
                to_local(preview_bounds),
                super::PopupWindowConfig {
                    focus: false,
                    display_id: platform_display_id,
                    ..Default::default()
                },
            ),
            |_, cx| {
                let view_entity = preview_entity.clone();
                cx.new(|cx| {
                    let _subscription = observe_ui(cx, &view_entity);
                    ScrollPreviewPanel {
                        ui: view_entity,
                        _subscription,
                    }
                })
            },
        );
        let prompt_entity = ui.clone();
        let _ = cx.open_window(
            super::popup_window_options(
                to_local(prompt_bounds),
                super::PopupWindowConfig {
                    focus: false,
                    display_id: platform_display_id,
                    ..Default::default()
                },
            ),
            |window, cx| {
                crate::system::click_through::enable(window);
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
                to_local(control_bounds),
                super::PopupWindowConfig {
                    focus: false,
                    display_id: platform_display_id,
                    ..Default::default()
                },
            ),
            |_, cx| {
                let view_entity = control_entity.clone();
                cx.new(|cx| {
                    let _subscription = observe_ui(cx, &view_entity);
                    ScrollControlBar {
                        ui: view_entity,
                        _subscription,
                    }
                })
            },
        );
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

    fn signal_from_bar(cx: &mut App, send: fn(&ScrollCaptureUi)) {
        for window_handle in cx.windows() {
            let Some(handle) = window_handle.downcast::<ScrollControlBar>() else {
                continue;
            };
            let _ = handle.update(cx, |bar, _, cx| {
                bar.ui.update(cx, |ui, _| send(ui));
            });
            return;
        }
    }
}

fn observe_ui<V: 'static>(cx: &mut Context<V>, ui: &Entity<ScrollCaptureUi>) -> Subscription {
    cx.observe(ui, |_, _, cx| cx.notify())
}

fn set_session_shortcuts(enabled: bool, cx: &App) {
    let Some(native) = crate::state::try_native(cx) else {
        return;
    };
    native.send(crate::system::native::NativeCommand::SetScrollCaptureShortcuts(enabled));
}

pub fn apply_progress(ui: &Entity<ScrollCaptureUi>, signal: ScrollSessionSignal, cx: &mut App) {
    match signal {
        ScrollSessionSignal::Frame { preview } => {
            let image = preview.as_deref().and_then(decode_preview);
            ui.update(cx, |ui, cx| {
                ui.apply_frame(image);
                cx.notify();
            });
        }
        ScrollSessionSignal::AutoScrolling(scrolling) => {
            ui.update(cx, |ui, cx| {
                ui.auto_scrolling = scrolling;
                cx.notify();
            });
        }
        ScrollSessionSignal::CursorOutside(outside) => {
            ui.update(cx, |ui, cx| {
                ui.cursor_outside = outside;
                cx.notify();
            });
        }
        ScrollSessionSignal::Finish | ScrollSessionSignal::Cancel => {}
    }
}

struct ScrollPreviewPanel {
    ui: Entity<ScrollCaptureUi>,
    _subscription: Subscription,
}

impl Render for ScrollPreviewPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.ui.read(cx);
        let image_height = state
            .preview
            .as_ref()
            .map(|image| image.height)
            .unwrap_or(0.0);
        let height = image_height.max(1.0);
        if window.bounds().size.height != px(height) {
            window.resize(size(px(PREVIEW_WIDTH), px(height)));
        }
        let mut panel = crate::ui::font::root()
            .w(px(PREVIEW_WIDTH))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(chrome::RADIUS_LG))
            .bg(crate::ui::colors::black(0.4))
            .shadow_2xl();
        if let Some(image) = &state.preview {
            panel = panel.child(
                div()
                    .w(px(PREVIEW_WIDTH))
                    .h(px(image.height))
                    .overflow_hidden()
                    .child(
                        img(image.image.clone())
                            .w(px(PREVIEW_WIDTH))
                            .h(px(image.height)),
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
        let outside = self.ui.read(cx).cursor_outside;
        crate::ui::font::root()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .children(outside.then(|| {
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
    _subscription: Subscription,
}

impl ScrollControlBar {
    fn button(
        id: &'static str,
        icon: &'static str,
        tooltip: impl Into<SharedString>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        toolbar::tooltip_button(toolbar::icon(id, icon), tooltip, cx, on_click)
    }

    fn filled_button(
        id: &'static str,
        glyph: AnyElement,
        tooltip: impl Into<SharedString>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        toolbar::tooltip_button(
            toolbar::desktop(toolbar::button(id).child(glyph)),
            tooltip,
            cx,
            on_click,
        )
    }
}

impl Render for ScrollControlBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if window.bounds().size != size(px(CONTROL_WIDTH), px(CONTROL_HEIGHT)) {
            window.resize(size(px(CONTROL_WIDTH), px(CONTROL_HEIGHT)));
        }
        let theme = active_theme(cx);
        let auto_scrolling = self.ui.read(cx).auto_scrolling;
        let toggle_tooltip: SharedString = if auto_scrolling {
            "Stop auto-scroll".into()
        } else {
            "Start auto-scroll".into()
        };
        let glyph_color = crate::ui::colors::white(0.85);
        let toggle_glyph = if auto_scrolling {
            toolbar::filled_glyph(glyph_color, false)
        } else {
            toolbar::filled_play(glyph_color)
        };
        crate::ui::font::root()
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
                        |this, _, cx| {
                            this.ui.update(cx, |ui, cx| ui.toggle_auto_scroll(cx));
                        },
                        cx,
                    ))
                    .child(Self::button(
                        "scroll-done",
                        "check",
                        "Done (Enter)",
                        |this, _, cx| {
                            this.ui.update(cx, |ui, _| ui.request_finish());
                        },
                        cx,
                    ))
                    .child(Self::button(
                        "scroll-cancel",
                        "x",
                        "Cancel (Esc)",
                        |this, _, cx| {
                            this.ui.update(cx, |ui, _| ui.request_cancel());
                        },
                        cx,
                    )),
            )
    }
}

fn logical_rect(area: ScreenRect, scale: f32) -> (f32, f32, f32, f32) {
    (
        area.x as f32 / scale,
        area.y as f32 / scale,
        area.width as f32 / scale,
        area.height as f32 / scale,
    )
}

fn control_origin(area: (f32, f32, f32, f32), display: (f32, f32)) -> (f32, f32) {
    let center_x = area.0 + area.2 / 2.0;
    let preferred_top = area.1 + area.3 + CONTROL_GAP;
    let top = if preferred_top + CONTROL_HEIGHT <= display.1 {
        preferred_top
    } else {
        (area.1 - CONTROL_GAP - CONTROL_HEIGHT).max(0.0)
    };
    (center_x - CONTROL_WIDTH / 2.0, top)
}

fn preview_left(area: (f32, f32, f32, f32), display_width: f32) -> f32 {
    let space_right = display_width - (area.0 + area.2);
    if space_right >= PREVIEW_WIDTH + PREVIEW_GAP {
        area.0 + area.2 + PREVIEW_GAP
    } else {
        (area.0 - PREVIEW_WIDTH - PREVIEW_GAP).max(0.0)
    }
}

fn preview_image_height(decoded_width: u32, decoded_height: u32) -> f32 {
    if decoded_width == 0 || decoded_height == 0 {
        return 0.0;
    }
    (PREVIEW_WIDTH * decoded_height as f32 / decoded_width as f32).min(PREVIEW_MAX_HEIGHT)
}

fn decode_preview(encoded: &str) -> Option<PreviewImage> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    let decoded = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (width, height) = (decoded.width(), decoded.height());
    if width == 0 || height == 0 {
        return None;
    }
    let panel_height = preview_image_height(width, height);
    if panel_height <= 0.0 {
        return None;
    }
    let target_width = PREVIEW_WIDTH * PREVIEW_SCALE as f32;
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
    fn preview_panel_prefers_the_right_side() {
        assert_eq!(preview_left((100.0, 100.0, 400.0, 300.0), 1920.0), 516.0);
    }

    #[test]
    fn preview_panel_falls_back_to_the_left_side() {
        assert_eq!(preview_left((1500.0, 100.0, 400.0, 300.0), 1920.0), 1244.0);
    }

    #[test]
    fn preview_height_follows_the_stitch_aspect() {
        assert_eq!(preview_image_height(480, 360), 180.0);
        assert_eq!(preview_image_height(240, 720), 360.0);
        assert_eq!(preview_image_height(0, 100), 0.0);
    }
}
