//! Capture coordinator — a long-lived entity that owns the capture pipeline
//! so work started by short-lived windows (the area overlay) survives their
//! removal and can still open follow-up UI on the main thread.

use gpui::{AnyWindowHandle, Context, Entity, WeakEntity};
use herogpui::gpui;
use poratake_daemon_common::contract::{
    ScrollCaptureStartRequest, ScrollSpeed, SCROLL_CAPTURE_AUTO_SCROLL_EVENT,
    SCROLL_CAPTURE_CANCELLED_EVENT, SCROLL_CAPTURE_CURSOR_EVENT, SCROLL_CAPTURE_DONE_EVENT,
    SCROLL_CAPTURE_FRAME_PREVIEW_EVENT, SYSTEM_EXIT_EVENT,
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
    pending_attach: Option<PendingEditorAttach>,
}

struct PendingEditorAttach {
    window: AnyWindowHandle,
    editor: WeakEntity<crate::editor::window::EditorWindow>,
    edge: crate::editor::layers::Edge,
}

impl Coordinator {
    pub fn new(service: CaptureService) -> Self {
        Self {
            service,
            pending_attach: None,
        }
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
        if intent == CaptureIntent::EditorAttach {
            return self.attach_area(capture, reservation, cx);
        }
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
    let (history_enabled, max_items, play_sound, screenshot) = cx.update(|cx| {
        let service = &cx.global::<crate::state::AppState>().service;
        crate::capture::desktop_icons::restore_after_capture(&service.daemon);
        let config = service.config.get();
        (
            config.history.enabled,
            config.history.max_items as usize,
            config.general.play_sound_on_screenshot,
            config.screenshot.clone(),
        )
    });

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
        cx.update(|cx| {
            if let Ok(bytes) = std::fs::read(&clipboard_path) {
                crate::system::clipboard::ClipboardService::write_png(cx, bytes);
            }
        });
    }
    if screenshot.capture_to_clipboard {
        return;
    }

    let open_preview = screenshot.show_preview;
    cx.update(|cx| {
        if open_preview {
            crate::windows::capture_preview::CapturePreviewWindow::open(cx, path.clone());
        } else {
            crate::open_editor_for(cx, path.to_string_lossy().as_ref());
        }
    });
}

fn show_capture_error(cx: &mut gpui::AsyncApp, title: &'static str, body: &str) {
    let body = body.to_string();
    cx.update(|cx| {
        crate::capture::desktop_icons::restore_after_capture(
            &cx.global::<crate::state::AppState>().service.daemon,
        );
        crate::windows::toast::Toast::show(cx, title, body)
    });
}

fn restore_editor_window(window: AnyWindowHandle, cx: &mut gpui::App) {
    let _ = window.update(cx, |_, window, _| {
        crate::system::window_visibility::show(window);
        window.activate_window();
    });
}

impl Coordinator {
    /// Remembers which editor edge a pending attach capture belongs to. The
    /// editor hides itself before the overlay opens, so the frozen frame and
    /// the pixels both come out without it — the `screenshot:capture-for-editor`
    /// contract.
    pub fn begin_editor_attach(
        &mut self,
        window: AnyWindowHandle,
        editor: WeakEntity<crate::editor::window::EditorWindow>,
        edge: crate::editor::layers::Edge,
    ) {
        self.pending_attach = Some(PendingEditorAttach {
            window,
            editor,
            edge,
        });
    }

    /// Re-shows the editor after the attach overlay is dismissed without a
    /// selection. The `finally` branch of `screenshot:capture-for-editor`.
    pub fn cancel_editor_attach(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_attach.take() else {
            return;
        };
        crate::capture::desktop_icons::restore_after_capture(&self.service.daemon);
        restore_editor_window(pending.window, cx);
    }

    fn attach_area(
        &mut self,
        capture: DisplayCapture,
        reservation: Option<CachedCaptureReservation>,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = self.pending_attach.take() else {
            crate::capture::desktop_icons::restore_after_capture(&self.service.daemon);
            return;
        };
        let service = self.service.clone();
        let path = std::env::temp_dir().join(format!(
            "{}-{}.png",
            CaptureIntent::EditorAttach.temp_prefix(),
            uuid_simple()
        ));
        let task = cx.background_executor().spawn(async move {
            service
                .capture_area_cached(capture, &path, reservation)
                .map(|_| path)
        });
        cx.spawn(async move |_entity, cx| {
            let captured = match task.await {
                Ok(path) => path,
                Err(error) => {
                    cx.update(|cx| {
                        crate::capture::desktop_icons::restore_after_capture(
                            &cx.global::<crate::state::AppState>().service.daemon,
                        );
                        restore_editor_window(pending.window, cx);
                        crate::windows::toast::Toast::show(cx, "Capture Failed", error.to_string());
                    });
                    return;
                }
            };
            let attached = cx.background_executor().spawn({
                let captured = captured.clone();
                async move {
                    let bytes = std::fs::read(&captured).ok();
                    let size = bytes
                        .as_ref()
                        .and_then(|bytes| image::load_from_memory(bytes).ok())
                        .map(|image| (image.width() as f64, image.height() as f64));
                    let _ = std::fs::remove_file(&captured);
                    bytes.zip(size)
                }
            });
            let Some((bytes, (width, height))) = attached.await else {
                cx.update(|cx| {
                    crate::capture::desktop_icons::restore_after_capture(
                        &cx.global::<crate::state::AppState>().service.daemon,
                    );
                    restore_editor_window(pending.window, cx);
                    crate::windows::toast::Toast::show(
                        cx,
                        "Capture Failed",
                        "The capture produced no image",
                    );
                });
                return;
            };
            use base64::Engine;
            let image_url = format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            );
            cx.update(|cx| {
                crate::capture::desktop_icons::restore_after_capture(
                    &cx.global::<crate::state::AppState>().service.daemon,
                );
                restore_editor_window(pending.window, cx);
                let _ = pending.editor.update(cx, |editor, cx| {
                    editor.push_image_layer(image_url, width, height, pending.edge, cx);
                });
            });
        })
        .detach();
    }

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

    /// Starts a daemon scroll-capture session over the selection. On macOS the
    /// daemon draws only the click-through area frame while the GPUI preview
    /// panel and control bar report progress; elsewhere the daemon owns the
    /// whole control panel and the app only finalizes the stitched image.
    fn run_scroll_capture(&mut self, capture: DisplayCapture, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let config = service.config.get().scroll_capture;
        #[cfg(target_os = "macos")]
        let area = capture.rect;
        let request = ScrollCaptureStartRequest {
            capture,
            auto_scroll_speed: ScrollSpeed::parse(&config.auto_scroll_speed),
            max_height: config.max_height.round().clamp(1.0, i32::MAX as f64) as i32,
            native_controls: Some(!cfg!(target_os = "macos")),
            boundary_only: cfg!(target_os = "macos").then_some(true),
        };
        if !crate::capture::scroll::start(&service.daemon, &request) {
            crate::capture::desktop_icons::restore_after_capture(&service.daemon);
            crate::windows::toast::Toast::show(
                cx,
                "Scroll Capture Failed",
                "The scroll capture could not be started",
            );
            return;
        }

        let (tx, rx) = smol::channel::bounded::<crate::capture::scroll::ScrollSessionSignal>(16);
        let daemon = service.daemon.clone();
        let event_tx = tx.clone();
        let subscription = daemon.subscribe(std::sync::Arc::new(move |event: &str, payload| {
            use crate::capture::scroll::ScrollSessionSignal as Signal;
            let signal = match event {
                SCROLL_CAPTURE_DONE_EVENT => Some(Signal::Finish),
                SCROLL_CAPTURE_CANCELLED_EVENT | SYSTEM_EXIT_EVENT => Some(Signal::Cancel),
                SCROLL_CAPTURE_FRAME_PREVIEW_EVENT => Some(Signal::Frame {
                    frame_count: payload
                        .get("frameCount")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as usize,
                    estimated_height: payload
                        .get("estimatedHeight")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(0),
                    preview: payload
                        .get("preview")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                }),
                SCROLL_CAPTURE_AUTO_SCROLL_EVENT => Some(Signal::AutoScrolling(
                    payload
                        .get("scrolling")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                )),
                SCROLL_CAPTURE_CURSOR_EVENT => Some(Signal::CursorOutside(
                    payload
                        .get("outside")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                )),
                _ => None,
            };
            if let Some(signal) = signal {
                let _ = event_tx.try_send(signal);
            }
        }));

        #[cfg(target_os = "macos")]
        let ui = crate::windows::scroll_capture::ScrollCaptureSession::open(
            cx,
            service.daemon.clone(),
            area,
            tx.clone(),
        );

        let output = service.generate_screenshot_path();
        cx.spawn(async move |_entity, cx| {
            use crate::capture::scroll::ScrollSessionSignal as Signal;
            loop {
                match rx.recv().await.unwrap_or(Signal::Cancel) {
                    Signal::Finish => {
                        drop(subscription);
                        #[cfg(target_os = "macos")]
                        cx.update(crate::windows::scroll_capture::ScrollCaptureSession::close);
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
                        return;
                    }
                    Signal::Cancel => {
                        drop(subscription);
                        crate::capture::scroll::cancel(&service.daemon);
                        crate::capture::desktop_icons::restore_after_capture(&service.daemon);
                        #[cfg(target_os = "macos")]
                        cx.update(crate::windows::scroll_capture::ScrollCaptureSession::close);
                        return;
                    }
                    #[cfg(target_os = "macos")]
                    progress => {
                        let ui = ui.clone();
                        cx.update(|cx| {
                            crate::windows::scroll_capture::apply_progress(&ui, progress, cx);
                        });
                    }
                    #[cfg(not(target_os = "macos"))]
                    _ => {}
                }
            }
        })
        .detach();
    }

    /// Shows the daemon countdown above the selection, then captures it from
    /// live pixels when the countdown finishes (`timer-capture.ts` captures
    /// uncached: the freeze was released before the countdown started).
    fn run_countdown(&mut self, capture: DisplayCapture, cx: &mut Context<Self>) {
        let Some(session) = crate::capture::timer::begin() else {
            crate::capture::desktop_icons::restore_after_capture(&self.service.daemon);
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
            crate::capture::desktop_icons::restore_after_capture(&self.service.daemon);
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
                cx.update(|cx| {
                    crate::capture::desktop_icons::restore_after_capture(
                        &cx.global::<crate::state::AppState>().service.daemon,
                    );
                });
                return;
            }
            let _ = entity.update(cx, |coordinator, cx| {
                coordinator.capture_area_reserved(capture, CaptureIntent::Screenshot, None, cx);
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
            crate::capture::desktop_icons::restore_after_capture(&daemon);

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

            cx.update(|cx| match outcome.clipboard {
                Some(text) => {
                    crate::system::clipboard::ClipboardService::write_text(cx, text);
                    crate::windows::toast::Toast::show_transient(cx, outcome.title, outcome.body);
                }
                None => {
                    crate::windows::toast::Toast::show(cx, outcome.title, outcome.body);
                }
            });
        })
        .detach();
    }
}

/// Global handle to the coordinator entity.
#[derive(Clone)]
pub struct CoordinatorHandle(pub Entity<Coordinator>);

impl gpui::Global for CoordinatorHandle {}
