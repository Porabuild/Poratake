//! Capture coordinator — a long-lived entity that owns the capture pipeline
//! so work started by short-lived windows (the area overlay) survives their
//! removal and can still open follow-up UI on the main thread.

use gpui::{Context, Entity};

use crate::capture::intent::CaptureIntent;
use crate::capture::overlay::ScreenRect;
use crate::capture::CaptureService;

fn uuid_simple() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}-{count:x}-{}", std::process::id())
}

pub struct Coordinator {
    pub service: CaptureService,
}

impl Coordinator {
    pub fn new(service: CaptureService) -> Self {
        Self { service }
    }

    /// Captures `rect` on the background executor and opens the editor with
    /// the result. Respects screenshot settings for clipboard/preview/editor
    /// routing, mirroring `capture/screenshot/finalize.ts`. Runs from this
    /// entity's context, which lives as long as the app.
    pub fn capture_area(&mut self, rect: ScreenRect, cx: &mut Context<Self>) {
        self.capture_area_for(rect, CaptureIntent::Screenshot, cx);
    }

    /// Captures `rect` and routes the pixels according to `intent`: the
    /// screenshot flows go to the library, the analysis flows hand a temp file
    /// to the daemon and delete it again.
    pub fn capture_area_for(
        &mut self,
        rect: ScreenRect,
        intent: CaptureIntent,
        cx: &mut Context<Self>,
    ) {
        if !intent.saves_to_library() {
            return self.analyze_area(rect, intent, cx);
        }
        if intent == CaptureIntent::Timer {
            return self.run_countdown(rect, cx);
        }
        if intent == CaptureIntent::ScrollCapture {
            return self.run_scroll_capture(rect, cx);
        }
        let service = self.service.clone();
        let path = service.generate_screenshot_path();
        let freeze = service.config.get().screenshot.freeze_screen;

        let task = cx.background_executor().spawn(async move {
            service
                .capture_area_cached(rect.x, rect.y, rect.width, rect.height, &path, freeze)
                .map(|_| path)
        });

        cx.spawn(async move |_entity, cx| match task.await {
            Ok(path) => finalize_capture(path, cx).await,
            Err(error) => show_capture_error(cx, "Capture Failed", &error.to_string()),
        })
        .detach();
    }
}

/// Port of `capture/screenshot/finalize.ts`: record the capture in history,
/// honour the clipboard settings, then open the preview or the editor.
async fn finalize_capture(path: std::path::PathBuf, cx: &mut gpui::AsyncApp) {
    let (max_items, screenshot) = cx
        .update(|cx| {
            let config = cx.global::<crate::state::AppState>().service.config.get();
            (config.history.max_items as usize, config.screenshot.clone())
        })
        .unwrap_or((50, crate::config::shortcuts::ScreenshotConfig::default()));

    crate::history_store::add_item(
        crate::history_store::HistoryItem {
            id: uuid_simple(),
            timestamp: chrono::Local::now().timestamp_millis(),
            original_path: path.to_string_lossy().to_string(),
            r#type: crate::history_store::HistoryItemType::Screenshot,
            editor_state: None,
            duration: None,
        },
        max_items,
    );

    if screenshot.capture_to_clipboard || screenshot.auto_copy_to_clipboard {
        copy_image_to_clipboard(&path);
    }
    if screenshot.capture_to_clipboard {
        return;
    }

    let open_preview = screenshot.show_preview;
    let result = cx.update(|cx| {
        if open_preview {
            crate::windows::capture_preview::CapturePreviewWindow::open(cx, path.clone());
        } else {
            crate::open_editor_for(cx, path.to_string_lossy().as_ref());
        }
    });
    if let Err(error) = result {
        show_capture_error(cx, "Capture Failed", &error.to_string());
    }
}

fn show_capture_error(cx: &mut gpui::AsyncApp, title: &'static str, body: &str) {
    let body = body.to_string();
    let _ = cx.update(|cx| crate::windows::toast::Toast::show(cx, title, body));
}

fn copy_image_to_clipboard(path: &std::path::Path) {
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let Ok(decoded) = image::load_from_memory(&bytes) else {
        return;
    };
    let rgba = decoded.to_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let copied = arboard::Clipboard::new().and_then(|mut clipboard| {
        clipboard.set_image(arboard::ImageData {
            width,
            height,
            bytes: std::borrow::Cow::Owned(rgba.into_raw()),
        })
    });
    if let Err(error) = copied {
        eprintln!("[clipboard] failed to copy capture: {error}");
    }
}

impl Coordinator {
    /// Captures a picked window through the daemon and routes the file the
    /// same way an area capture is routed.
    pub fn capture_window(&mut self, window_id: i64, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let path = service.generate_screenshot_path();
        let task = cx.background_executor().spawn(async move {
            service
                .capture_window_to_file(window_id, &path)
                .map(|_| path)
        });

        cx.spawn(async move |_entity, cx| match task.await {
            Ok(path) => finalize_capture(path, cx).await,
            Err(error) => show_capture_error(cx, "Window Capture Failed", &error.to_string()),
        })
        .detach();
    }

    /// Starts a daemon scroll-capture session over the selection. The daemon
    /// owns the on-screen control panel and reports when it is done.
    fn run_scroll_capture(&mut self, rect: ScreenRect, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let config = service.config.get().scroll_capture;
        let params = crate::capture::scroll::StartParams {
            rect,
            auto_scroll_speed: config.auto_scroll_speed,
            max_height: config.max_height,
            scale_factor: 1.0,
        };
        if !crate::capture::scroll::start(&service.daemon, &params) {
            crate::windows::toast::Toast::show(
                cx,
                "Scroll Capture Failed",
                "The scroll capture could not be started",
            );
            return;
        }

        let (tx, rx) = smol::channel::bounded::<bool>(1);
        let daemon = service.daemon.clone();
        daemon.on_event(std::sync::Arc::new(
            move |event: &str, _payload| match event {
                "scroll-capture:done" => {
                    let _ = tx.try_send(true);
                }
                "scroll-capture:cancelled" => {
                    let _ = tx.try_send(false);
                }
                _ => {}
            },
        ));

        let output = service.generate_screenshot_path();
        cx.spawn(async move |_entity, cx| {
            let finished = rx.recv().await.unwrap_or(false);
            if !finished {
                crate::capture::scroll::cancel(&service.daemon);
                return;
            }
            let daemon = service.daemon.clone();
            let stitched = cx
                .background_executor()
                .spawn(async move { crate::capture::scroll::finish(&daemon, &output) })
                .await;
            match stitched {
                Some(path) => finalize_capture(path, cx).await,
                None => show_capture_error(
                    cx,
                    "Scroll Capture Failed",
                    "The scroll capture produced no image",
                ),
            }
        })
        .detach();
    }

    /// Shows the daemon countdown above the selection, then captures it as a
    /// normal screenshot when the countdown finishes.
    fn run_countdown(&mut self, rect: ScreenRect, cx: &mut Context<Self>) {
        let daemon = self.service.daemon.clone();
        if !crate::capture::timer::show(&daemon, rect, crate::capture::timer::TIMER_DURATION) {
            self.capture_area_for(rect, CaptureIntent::Screenshot, cx);
            return;
        }

        cx.spawn(async move |entity, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(
                    crate::capture::timer::TIMER_DURATION as u64,
                ))
                .await;
            crate::capture::timer::hide(&daemon);
            let _ = entity.update(cx, |coordinator, cx| {
                coordinator.capture_area_for(rect, CaptureIntent::Screenshot, cx);
            });
        })
        .detach();
    }

    fn analyze_area(&mut self, rect: ScreenRect, intent: CaptureIntent, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let freeze = service.config.get().screenshot.freeze_screen;
        let path =
            std::env::temp_dir().join(format!("{}-{}.png", intent.temp_prefix(), uuid_simple()));

        let task = cx.background_executor().spawn(async move {
            service
                .capture_area_cached(rect.x, rect.y, rect.width, rect.height, &path, freeze)
                .map(|_| path)
        });

        cx.spawn(async move |entity, cx| {
            let captured = match task.await {
                Ok(path) => path,
                Err(error) => {
                    let title = match intent {
                        CaptureIntent::Ocr => "OCR Failed",
                        _ => "Scan Failed",
                    };
                    show_capture_error(cx, title, &error.to_string());
                    return;
                }
            };

            let daemon = entity
                .read_with(cx, |coordinator, _| coordinator.service.daemon.clone())
                .ok();
            let Some(daemon) = daemon else {
                return;
            };

            let analysis = cx.background_executor().spawn({
                let captured = captured.clone();
                async move {
                    let outcome = match intent {
                        CaptureIntent::Ocr => {
                            crate::capture::analysis::recognize_text(&daemon, &captured)
                        }
                        _ => crate::capture::analysis::scan_qr_code(&daemon, &captured),
                    };
                    let _ = std::fs::remove_file(&captured);
                    outcome
                }
            });
            let outcome = analysis.await;

            let _ = cx.update(|cx| {
                crate::windows::toast::Toast::show(cx, outcome.title, outcome.body);
            });
        })
        .detach();
    }
}

/// Global handle to the coordinator entity.
#[derive(Clone)]
pub struct CoordinatorHandle(pub Entity<Coordinator>);

impl gpui::Global for CoordinatorHandle {}
