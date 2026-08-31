//! Capture coordinator — a long-lived entity that owns the capture pipeline
//! so work started by short-lived windows (the area overlay) survives their
//! removal and can still open follow-up UI on the main thread.

use gpui::{Context, Entity};
use poratake_daemon_common::contract::{
    ScrollCaptureStartRequest, ScrollSpeed, SCROLL_CAPTURE_CANCELLED_EVENT,
    SCROLL_CAPTURE_DONE_EVENT, SYSTEM_EXIT_EVENT,
};

use crate::capture::intent::CaptureIntent;
use crate::capture::{CachedCaptureReservation, CaptureService, DisplayCapture};

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

    /// Captures a display-bound region and routes the pixels according to `intent`: the
    /// screenshot flows go to the library, the analysis flows hand a temp file
    /// to the daemon and delete it again.
    pub fn capture_area_for(
        &mut self,
        capture: DisplayCapture,
        intent: CaptureIntent,
        cx: &mut Context<Self>,
    ) {
        let freeze = self.service.config.get().screenshot.freeze_screen;
        let reservation = freeze.then(|| self.service.reserve_cached_capture());
        self.capture_area_reserved(capture, intent, reservation, cx);
    }

    pub(crate) fn capture_area_reserved(
        &mut self,
        capture: DisplayCapture,
        intent: CaptureIntent,
        reservation: Option<CachedCaptureReservation>,
        cx: &mut Context<Self>,
    ) {
        if !intent.saves_to_library() {
            return self.analyze_area(capture, intent, reservation, cx);
        }
        if intent == CaptureIntent::Timer {
            return self.run_countdown(capture, cx);
        }
        if intent == CaptureIntent::ScrollCapture {
            return self.run_scroll_capture(capture, cx);
        }
        let service = self.service.clone();
        let path = service.generate_screenshot_path();

        let task = cx.background_executor().spawn(async move {
            service
                .capture_area_cached(capture, &path, reservation)
                .map(|_| path)
        });

        cx.spawn(async move |_entity, cx| match task.await {
            Ok(path) => finalize_capture(path, false, cx).await,
            Err(error) => show_capture_error(cx, "Capture Failed", &error.to_string()),
        })
        .detach();
    }
}

/// Port of `capture/screenshot/finalize.ts`: record the capture in history,
/// honour the clipboard settings, then open the preview or the editor.
async fn finalize_capture(path: std::path::PathBuf, silent: bool, cx: &mut gpui::AsyncApp) {
    let (history_enabled, max_items, play_sound, screenshot) = cx
        .update(|cx| {
            let config = cx.global::<crate::state::AppState>().service.config.get();
            (
                config.history.enabled,
                config.history.max_items as usize,
                config.general.play_sound_on_screenshot,
                config.screenshot.clone(),
            )
        })
        .unwrap_or((
            true,
            50,
            false,
            crate::config::shortcuts::ScreenshotConfig::default(),
        ));

    #[cfg(target_os = "macos")]
    if play_sound && !silent {
        cx.background_executor()
            .spawn(async {
                let _ = std::process::Command::new("afplay")
                    .arg("/System/Library/Sounds/Glass.aiff")
                    .status();
            })
            .detach();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (play_sound, silent);

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
        history_enabled,
    );

    if screenshot.capture_to_clipboard || screenshot.auto_copy_to_clipboard {
        let clipboard_path = path.clone();
        let _ = cx.update(|cx| {
            if let Ok(bytes) = std::fs::read(&clipboard_path) {
                crate::system::clipboard::ClipboardService::write_png(cx, bytes);
            }
        });
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
            Ok(path) => finalize_capture(path, false, cx).await,
            Err(error) => show_capture_error(cx, "Window Capture Failed", &error.to_string()),
        })
        .detach();
    }

    /// Starts a daemon scroll-capture session over the selection. The daemon
    /// owns the on-screen control panel and reports when it is done.
    fn run_scroll_capture(&mut self, capture: DisplayCapture, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let config = service.config.get().scroll_capture;
        let request = ScrollCaptureStartRequest {
            capture,
            auto_scroll_speed: ScrollSpeed::parse(&config.auto_scroll_speed),
            max_height: config.max_height.round().clamp(1.0, i32::MAX as f64) as i32,
            native_controls: Some(true),
        };
        if !crate::capture::scroll::start(&service.daemon, &request) {
            crate::windows::toast::Toast::show(
                cx,
                "Scroll Capture Failed",
                "The scroll capture could not be started",
            );
            return;
        }

        let (tx, rx) = smol::channel::bounded::<bool>(1);
        let daemon = service.daemon.clone();
        let subscription =
            daemon.subscribe(std::sync::Arc::new(
                move |event: &str, _payload| match event {
                    SCROLL_CAPTURE_DONE_EVENT => {
                        let _ = tx.try_send(true);
                    }
                    SCROLL_CAPTURE_CANCELLED_EVENT => {
                        let _ = tx.try_send(false);
                    }
                    SYSTEM_EXIT_EVENT => {
                        let _ = tx.try_send(false);
                    }
                    _ => {}
                },
            ));

        let output = service.generate_screenshot_path();
        cx.spawn(async move |_entity, cx| {
            let finished = rx.recv().await.unwrap_or(false);
            drop(subscription);
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
                Some(path) => finalize_capture(path, true, cx).await,
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
    fn run_countdown(&mut self, capture: DisplayCapture, cx: &mut Context<Self>) {
        let Some(session) = crate::capture::timer::begin() else {
            return;
        };
        let daemon = self.service.daemon.clone();
        let (tx, rx) = smol::channel::bounded::<bool>(1);
        let subscription =
            daemon.subscribe(std::sync::Arc::new(move |event, _payload| match event {
                poratake_daemon_common::contract::TIMER_CONTROL_COMPLETED_EVENT => {
                    let _ = tx.try_send(true);
                }
                poratake_daemon_common::contract::TIMER_CONTROL_CANCEL_EVENT => {
                    let _ = tx.try_send(false);
                }
                poratake_daemon_common::contract::SYSTEM_EXIT_EVENT => {
                    let _ = tx.try_send(false);
                }
                _ => {}
            }));
        let theme = crate::theme::vars::active_theme(cx);
        if !crate::capture::timer::show(
            &daemon,
            capture,
            crate::capture::timer::TIMER_DURATION,
            theme.accent,
            theme.accent_foreground,
        ) {
            drop(subscription);
            drop(session);
            crate::windows::toast::Toast::show(
                cx,
                "Timer Capture Failed",
                "The countdown control could not be started",
            );
            return;
        }

        cx.spawn(async move |entity, cx| {
            let timeout = cx
                .background_executor()
                .timer(std::time::Duration::from_secs(
                    crate::capture::timer::TIMER_DURATION as u64 + 5,
                ));
            let completed = smol::future::or(async { rx.recv().await.unwrap_or(false) }, async {
                timeout.await;
                false
            })
            .await;
            drop(subscription);
            drop(session);
            cx.background_executor()
                .spawn(async move { crate::capture::timer::hide(&daemon) })
                .detach();
            if !completed {
                return;
            }
            let _ = entity.update(cx, |coordinator, cx| {
                coordinator.capture_area_for(capture, CaptureIntent::Screenshot, cx);
            });
        })
        .detach();
    }

    fn analyze_area(
        &mut self,
        capture: DisplayCapture,
        intent: CaptureIntent,
        reservation: Option<CachedCaptureReservation>,
        cx: &mut Context<Self>,
    ) {
        let service = self.service.clone();
        let path =
            std::env::temp_dir().join(format!("{}-{}.png", intent.temp_prefix(), uuid_simple()));

        let task = cx.background_executor().spawn(async move {
            service
                .capture_area_cached(capture, &path, reservation)
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
                if let Some(text) = outcome.clipboard {
                    crate::system::clipboard::ClipboardService::write_text(cx, text);
                }
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
