//! Capture coordinator — a long-lived entity that owns the capture pipeline
//! so work started by short-lived windows (the area overlay) survives their
//! removal and can still open follow-up UI on the main thread.

use anyhow::Context as _;
use gpui::{AnyWindowHandle, Context, Entity, WeakEntity};
use herogpui::gpui;
use poratake_daemon_common::contract::{ScrollCaptureStartRequest, ScrollSpeed};
use std::sync::Arc;

use crate::capture::desktop_icons::CaptureHide;
use crate::capture::intent::CaptureIntent;
use crate::capture::{
    prepare_screenshot, publish_screenshot, uuid_simple, CachedCaptureReservation, CaptureService,
    DisplayCapture,
};

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
    pub(crate) fn capture_area_reserved(
        &mut self,
        capture: DisplayCapture,
        intent: CaptureIntent,
        reservation: Option<CachedCaptureReservation>,
        release_after: Option<u64>,
        hide: Option<Arc<CaptureHide>>,
        owner: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        let should_hide = hide.is_none()
            && !matches!(intent, CaptureIntent::Timer | CaptureIntent::ScrollCapture)
            && crate::capture::desktop_icons::should_hide_for_capture(&self.service.config);
        let pending = crate::capture::desktop_icons::pending_transitions();
        let needs_restoration =
            hide.is_none() && crate::capture::desktop_icons::needs_restoration();
        if should_hide || !pending.is_empty() || needs_restoration {
            let transition = crate::capture::desktop_icons::track_transition();
            let service = self.service.clone();
            cx.spawn(async move |entity, cx| {
                for transition in pending {
                    let _ = transition.recv().await;
                }
                let stale = cx.update(|cx| {
                    crate::capture::desktop_icons::is_shutting_down()
                        || (intent == CaptureIntent::Screenshot
                            && owner.is_some_and(|owner| {
                                crate::capture::overlay::revision(cx) != owner
                            }))
                });
                if stale {
                    drop(transition);
                    let _ = entity.update(cx, |coordinator, cx| {
                        coordinator.capture_area_ready(
                            capture,
                            intent,
                            reservation,
                            release_after,
                            hide,
                            owner,
                            cx,
                        );
                    });
                    return;
                }
                let hide = if should_hide {
                    let capture_service = service.clone();
                    cx.background_executor()
                        .spawn(async move {
                            crate::capture::desktop_icons::hide_for_capture(
                                &capture_service.daemon,
                                &capture_service.config,
                            )
                        })
                        .await
                } else {
                    Ok(hide)
                };
                let (hide, mut failure) = match hide {
                    Ok(hide) => (hide, None),
                    Err(error) => (None, Some(error)),
                };
                if failure.is_none()
                    && hide.is_none()
                    && crate::capture::desktop_icons::needs_restoration()
                {
                    let daemon = service.daemon.clone();
                    let restored =
                        cx.background_executor()
                            .spawn(async move {
                                crate::capture::desktop_icons::restore_if_unowned(&daemon)
                            })
                            .await;
                    if !restored {
                        failure = Some("Could not restore desktop icons");
                    }
                }
                if let Some(error) = failure {
                    drop(transition);
                    let _ = entity.update(cx, |coordinator, cx| {
                        drop(reservation);
                        if let Some(generation) = release_after {
                            crate::capture::overlay::release_frozen_screen(generation, cx);
                        }
                        let stale = crate::capture::desktop_icons::is_shutting_down()
                            || (intent == CaptureIntent::Screenshot
                                && owner.is_some_and(|owner| {
                                    crate::capture::overlay::revision(cx) != owner
                                }));
                        if !stale {
                            if intent == CaptureIntent::EditorAttach {
                                coordinator.cancel_editor_attach(cx);
                            }
                            let title = match intent {
                                CaptureIntent::Ocr => "OCR Failed",
                                CaptureIntent::QrCode => "Scan Failed",
                                _ => "Capture Failed",
                            };
                            crate::windows::toast::Toast::show(cx, title, error);
                        }
                    });
                    return;
                }
                drop(transition);
                let _ = entity.update(cx, |coordinator, cx| {
                    coordinator.capture_area_ready(
                        capture,
                        intent,
                        reservation,
                        release_after,
                        hide,
                        owner,
                        cx,
                    );
                });
            })
            .detach();
            return;
        }
        self.capture_area_ready(capture, intent, reservation, release_after, hide, owner, cx);
    }

    fn capture_area_ready(
        &mut self,
        capture: DisplayCapture,
        intent: CaptureIntent,
        reservation: Option<CachedCaptureReservation>,
        release_after: Option<u64>,
        hide: Option<Arc<CaptureHide>>,
        owner: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        if crate::capture::desktop_icons::is_shutting_down()
            || (intent == CaptureIntent::Screenshot
                && owner.is_some_and(|owner| crate::capture::overlay::revision(cx) != owner))
        {
            drop(reservation);
            if let Some(generation) = release_after {
                crate::capture::overlay::release_frozen_screen(generation, cx);
            }
            crate::capture::desktop_icons::release_for_capture(hide, cx);
            return;
        }
        if intent == CaptureIntent::EditorAttach {
            return self.attach_area(capture, reservation, release_after, hide, cx);
        }
        if !intent.saves_to_library() {
            return self.analyze_area(capture, intent, reservation, release_after, hide, cx);
        }
        if intent == CaptureIntent::Timer {
            return self.run_countdown(capture, hide, owner.expect("timer selection owner"), cx);
        }
        if intent == CaptureIntent::ScrollCapture {
            return self.run_scroll_capture(capture, hide, cx);
        }
        let service = self.service.clone();
        let path = service.generate_screenshot_path();
        let transition = hide
            .as_ref()
            .map(|_| crate::capture::desktop_icons::track_transition());

        let task = cx.background_executor().spawn(async move {
            let result = (|| -> anyhow::Result<_> {
                let temporary = prepare_screenshot(&path)?;
                service
                    .capture_area_cached(capture, &temporary, reservation, None)
                    .with_context(|| {
                        format!("Capture data may be available at {}", temporary.display())
                    })?;
                publish_screenshot(&temporary, &path).with_context(|| {
                    format!("Capture data is available at {}", temporary.display())
                })
            })();
            if let Some(generation) = release_after {
                service.release_screen(generation);
            }
            drop(hide);
            drop(transition);
            result
        });

        cx.spawn(async move |_entity, cx| {
            let result = task.await;
            match result {
                Ok(path) => finalize_capture(path, false, cx).await,
                Err(error) => show_capture_error(cx, "Capture Failed", &error.to_string()),
            }
        })
        .detach();
    }
}

/// Port of `capture/screenshot/finalize.ts`: record the capture in history,
/// honour the clipboard settings, then open the preview or the editor.
async fn finalize_capture(path: std::path::PathBuf, silent: bool, cx: &mut gpui::AsyncApp) {
    let (history_enabled, max_items, play_sound, screenshot) = cx.update(|cx| {
        let service = &cx.global::<crate::state::AppState>().service;
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
    cx.update(|cx| crate::windows::toast::Toast::show(cx, title, body));
}

fn restore_editor_window(window: AnyWindowHandle, cx: &mut gpui::App) {
    let _ = window.update(cx, |_, window, _| {
        crate::system::window_visibility::show(window);
        window.activate_window();
    });
}

impl Coordinator {
    pub(crate) fn has_pending_editor_attach(&self) -> bool {
        self.pending_attach.is_some()
    }

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
        restore_editor_window(pending.window, cx);
    }

    fn attach_area(
        &mut self,
        capture: DisplayCapture,
        reservation: Option<CachedCaptureReservation>,
        release_after: Option<u64>,
        hide: Option<Arc<CaptureHide>>,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = self.pending_attach.take() else {
            drop(reservation);
            if let Some(generation) = release_after {
                crate::capture::overlay::release_frozen_screen(generation, cx);
            }
            crate::capture::desktop_icons::release_for_capture(hide, cx);
            return;
        };
        let service = self.service.clone();
        let path = std::env::temp_dir().join(format!(
            "{}-{}.png",
            CaptureIntent::EditorAttach.temp_prefix(),
            uuid_simple()
        ));
        let transition = hide
            .as_ref()
            .map(|_| crate::capture::desktop_icons::track_transition());
        let task = cx.background_executor().spawn(async move {
            let result = service.capture_area_cached(capture, &path, reservation, release_after);
            if result.is_err() {
                let _ = std::fs::remove_file(&path);
            }
            drop(hide);
            drop(transition);
            result.map(|_| path)
        });
        cx.spawn(async move |_entity, cx| {
            let captured = match task.await {
                Ok(path) => path,
                Err(error) => {
                    cx.update(|cx| {
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
    pub fn capture_window(
        &mut self,
        window_id: i64,
        capture: DisplayCapture,
        reservation: Option<CachedCaptureReservation>,
        release_after: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        let service = self.service.clone();
        let path = service.generate_screenshot_path();
        let task = cx.background_executor().spawn(async move {
            let result = (|| -> anyhow::Result<_> {
                let temporary = prepare_screenshot(&path)?;
                service
                    .capture_window_to_file(window_id, capture, &temporary, reservation)
                    .with_context(|| {
                        format!("Capture data may be available at {}", temporary.display())
                    })?;
                publish_screenshot(&temporary, &path).with_context(|| {
                    format!("Capture data is available at {}", temporary.display())
                })
            })();
            if let Some(generation) = release_after {
                service.release_screen(generation);
            }
            result
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
    fn run_scroll_capture(
        &mut self,
        capture: DisplayCapture,
        hide: Option<Arc<CaptureHide>>,
        cx: &mut Context<Self>,
    ) {
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
        let (tx, rx) = smol::channel::unbounded::<crate::capture::scroll::ScrollSessionSignal>();
        let (progress_tx, progress_rx) = smol::channel::bounded(16);
        let daemon = service.daemon.clone();
        let event_tx = tx.clone();
        let subscription = daemon.subscribe(std::sync::Arc::new(move |event: &str, payload| {
            use crate::capture::scroll::ScrollSessionSignal as Signal;
            if let Some(signal) = Signal::from_daemon_event(event, payload) {
                signal.send(&event_tx, &progress_tx);
            }
        }));
        if !crate::capture::scroll::begin() {
            crate::capture::desktop_icons::release_for_capture(hide, cx);
            crate::windows::toast::Toast::show(
                cx,
                "Scroll Capture Failed",
                "The scroll capture could not be started",
            );
            return;
        }

        let output = service.generate_screenshot_path();
        cx.spawn(async move |_entity, cx| {
            use crate::capture::scroll::ScrollSessionSignal as Signal;
            let daemon = service.daemon.clone();
            let started = cx
                .background_executor()
                .spawn(async move { crate::capture::scroll::start_reserved(&daemon, &request) })
                .await;
            if !started {
                cx.update(|cx| {
                    crate::capture::desktop_icons::release_for_capture(hide, cx);
                    if !crate::capture::desktop_icons::is_shutting_down() {
                        crate::windows::toast::Toast::show(
                            cx,
                            "Scroll Capture Failed",
                            "The scroll capture could not be started",
                        );
                    }
                });
                return;
            }
            if crate::capture::desktop_icons::is_shutting_down() {
                cx.background_executor()
                    .spawn(async move {
                        crate::capture::scroll::cancel(&service.daemon);
                        drop(hide);
                    })
                    .await;
                return;
            }
            #[cfg(target_os = "macos")]
            let ui = cx.update(|cx| {
                crate::windows::scroll_capture::ScrollCaptureSession::open(
                    cx,
                    service.daemon.clone(),
                    area,
                    tx.clone(),
                )
            });
            let finished = loop {
                match Signal::receive(&rx, &progress_rx)
                    .await
                    .unwrap_or(Signal::Cancel)
                {
                    Signal::Finish => break true,
                    Signal::Cancel => break false,
                    Signal::Stopped(message) => {
                        cx.update(|cx| crate::windows::toast::Toast::show(cx, "Scroll Capture Stopped", message));
                    }
                    #[cfg(target_os = "macos")]
                    progress => {
                        let ui = ui.clone();
                        cx.update(|cx| {
                            crate::windows::scroll_capture::apply_progress(&ui, progress, cx);
                        });
                    }
                    #[cfg(all(not(target_os = "macos"), any(test, all(windows, feature = "parity"))))]
                    _ => {}
                }
            };
            drop(subscription);
            let transition = hide
                .as_ref()
                .map(|_| crate::capture::desktop_icons::track_transition());
            if !finished {
                cx.background_executor()
                    .spawn(async move {
                        #[cfg(windows)]
                        drop(hide);
                        crate::capture::scroll::cancel(&service.daemon);
                        #[cfg(not(windows))]
                        drop(hide);
                        drop(transition);
                    })
                    .await;
                #[cfg(target_os = "macos")]
                cx.update(crate::windows::scroll_capture::ScrollCaptureSession::close);
                return;
            }
            #[cfg(target_os = "macos")]
            cx.update(crate::windows::scroll_capture::ScrollCaptureSession::close);
            let daemon = service.daemon.clone();
            let stitched = cx
                .background_executor()
                .spawn(async move {
                    #[cfg(not(windows))]
                    let _transition = transition;
                    #[cfg(not(windows))]
                    let _hide = hide;
                    #[cfg(windows)]
                    drop(hide);
                    #[cfg(windows)]
                    drop(transition);
                    let temporary = match prepare_screenshot(&output) {
                        Ok(temporary) => temporary,
                        Err(error) => {
                            crate::capture::scroll::cancel(&daemon);
                            return Err(anyhow::Error::from(error));
                        }
                    };
                    if crate::capture::scroll::finish(&daemon, &temporary).is_none() {
                        if std::fs::metadata(&temporary).is_ok_and(|metadata| metadata.len() == 0) {
                            let _ = std::fs::remove_file(&temporary);
                            return Err(anyhow::anyhow!("The scroll capture produced no image"));
                        }
                        return Err(anyhow::anyhow!(
                            "The scroll capture produced no image. Capture data may be available at {}",
                            temporary.display()
                        ));
                    }
                    publish_screenshot(&temporary, &output)
                        .with_context(|| format!("Capture data is available at {}", temporary.display()))
                })
                .await;
            match stitched {
                Ok(path) => finalize_capture(path, true, cx).await,
                Err(error) => show_capture_error(cx, "Scroll Capture Failed", &error.to_string()),
            }
        })
        .detach();
    }

    /// Shows the daemon countdown above the selection, then captures it from
    /// live pixels when the countdown finishes (`timer-capture.ts` captures
    fn run_countdown(
        &mut self,
        capture: DisplayCapture,
        hide: Option<Arc<CaptureHide>>,
        owner: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = crate::capture::timer::begin() else {
            if crate::capture::overlay::close_timer(owner, cx) {
                crate::windows::toast::Toast::show(
                    cx,
                    "Timer Capture Failed",
                    "Another timer capture is still finishing",
                );
            }
            crate::capture::desktop_icons::release_for_capture(hide, cx);
            return;
        };
        let daemon = self.service.daemon.clone();
        let (tx, rx) = smol::channel::bounded::<bool>(1);
        if !crate::capture::overlay::watch_timer(owner, tx.clone(), cx) {
            crate::capture::desktop_icons::release_for_capture(hide, cx);
            return;
        }
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
            crate::capture::overlay::close_timer(owner, cx);
            crate::capture::desktop_icons::release_for_capture(hide, cx);
            crate::windows::toast::Toast::show(
                cx,
                "Timer Capture Failed",
                "The countdown control could not be started",
            );
            return;
        }

        let transition = hide
            .as_ref()
            .map(|_| crate::capture::desktop_icons::track_transition());
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
            cx.background_executor()
                .spawn(async move { crate::capture::timer::hide(&daemon) })
                .await;
            drop(session);
            let owned = cx.update(|cx| crate::capture::overlay::close_timer(owner, cx));
            if !completed || !owned {
                cx.background_executor()
                    .spawn(async move {
                        drop(hide);
                        drop(transition);
                    })
                    .detach();
                return;
            }
            cx.update(|cx| {
                cx.defer(move |cx| {
                    drop(transition);
                    let _ = entity.update(cx, |coordinator, cx| {
                        coordinator.capture_area_reserved(
                            capture,
                            CaptureIntent::Screenshot,
                            None,
                            None,
                            hide,
                            None,
                            cx,
                        );
                    });
                });
            });
        })
        .detach();
    }

    fn analyze_area(
        &mut self,
        capture: DisplayCapture,
        intent: CaptureIntent,
        reservation: Option<CachedCaptureReservation>,
        release_after: Option<u64>,
        hide: Option<Arc<CaptureHide>>,
        cx: &mut Context<Self>,
    ) {
        let service = self.service.clone();
        let path =
            std::env::temp_dir().join(format!("{}-{}.png", intent.temp_prefix(), uuid_simple()));
        let transition = hide
            .as_ref()
            .map(|_| crate::capture::desktop_icons::track_transition());

        let task = cx.background_executor().spawn(async move {
            let result = service.capture_area_cached(capture, &path, reservation, release_after);
            drop(hide);
            drop(transition);
            if let Err(error) = result {
                let _ = std::fs::remove_file(&path);
                return Err(error);
            }
            Ok(path)
        });

        cx.spawn(async move |entity, cx| {
            let captured = match task.await {
                Ok(path) => path,
                Err(error) => {
                    eprintln!("[capture] analysis capture failed: {error}");
                    let title = match intent {
                        CaptureIntent::Ocr => "OCR Failed",
                        _ => "Scan Failed",
                    };
                    show_capture_error(cx, title, "Failed to capture the selected area");
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

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn scroll_start_queued_before_shutdown_does_not_start_daemon() {
        if std::env::var_os("PORATAKE_TEST_SCROLL_SHUTDOWN").is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "capture::coordinator::tests::scroll_start_queued_before_shutdown_does_not_start_daemon",
                    "--nocapture",
                ])
                .env("PORATAKE_TEST_SCROLL_SHUTDOWN", "1")
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                stderr
            );
            assert!(
                !stderr.contains("[scroll-capture] start failed:"),
                "{stderr}"
            );
            return;
        }

        use poratake_daemon_common::geometry::{CaptureRect, DisplayCaptureContext, DisplayOrigin};

        let cx = gpui::TestAppContext::single();
        let dir = tempfile::tempdir().unwrap();
        let config = std::sync::Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| settings.screenshot.hide_desktop_icons = false);
        let notifications = crate::system::notification::capture_for_test();
        cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            crate::state::coordinator(cx).update(cx, |coordinator, cx| {
                coordinator.capture_area_reserved(
                    DisplayCaptureContext::new(
                        CaptureRect {
                            x: 0,
                            y: 0,
                            width: 100,
                            height: 100,
                        },
                        1.0,
                        DisplayOrigin::default(),
                        None,
                    ),
                    crate::capture::intent::CaptureIntent::ScrollCapture,
                    None,
                    None,
                    None,
                    None,
                    cx,
                );
            });
            assert!(crate::capture::scroll::is_active());
            assert!(crate::capture::desktop_icons::begin_shutdown());
        });
        cx.run_until_parked();
        assert!(!crate::capture::scroll::is_active());
        assert!(notifications.borrow().is_empty());
    }

    fn start_timer(cx: &gpui::TestAppContext) {
        cx.update(|cx| {
            crate::capture::start_area_selection(crate::capture::intent::CaptureIntent::Timer, cx);
        });
        cx.run_until_parked();
        let window = cx
            .update(crate::capture::overlay::parity_active_overlay)
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(window, cx);
        visual.simulate_mouse_down(
            gpui::point(gpui::px(100.), gpui::px(100.)),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        visual.simulate_mouse_move(
            gpui::point(gpui::px(300.), gpui::px(250.)),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        visual.simulate_mouse_up(
            gpui::point(gpui::px(300.), gpui::px(250.)),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(crate::capture::timer::is_active());
    }

    #[test]
    fn an_old_timer_does_not_close_a_new_selection() {
        if std::env::var_os("PORATAKE_TEST_TIMER_CONTROL").is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "capture::coordinator::tests::an_old_timer_does_not_close_a_new_selection",
                    "--nocapture",
                ])
                .env("PORATAKE_TEST_TIMER_CONTROL", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let cx = gpui::TestAppContext::single();
        let dir = tempfile::tempdir().unwrap();
        let screenshots = dir.path().join("screenshots");
        std::fs::create_dir(&screenshots).unwrap();
        let config = std::sync::Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| {
            settings.screenshot.freeze_screen = false;
            settings.screenshot.hide_desktop_icons = false;
            settings.storage.screenshots_path = screenshots.to_string_lossy().into_owned();
        });
        let notifications = crate::system::notification::capture_for_test();
        cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            crate::capture::overlay::init_bindings(cx);
        });
        for event in [
            None,
            Some(poratake_daemon_common::contract::TIMER_CONTROL_COMPLETED_EVENT),
            Some(poratake_daemon_common::contract::TIMER_CONTROL_CANCEL_EVENT),
        ] {
            start_timer(&cx);
            let observer = event.is_none().then(|| {
                let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
                let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
                crate::capture::timer::gate_hide_for_test(entered_tx, release_rx);
                std::thread::spawn(move || {
                    entered_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    let active = crate::capture::timer::is_active();
                    let blocked = crate::capture::timer::begin().is_none();
                    release_tx.send(()).unwrap();
                    (active, blocked)
                })
            });
            cx.update(|cx| {
                if let Some(event) = event {
                    crate::state::state(cx).daemon.emit_for_test(event);
                }
                crate::capture::start_area_selection(
                    crate::capture::intent::CaptureIntent::Screenshot,
                    cx,
                );
            });
            cx.run_until_parked();
            if let Some(observer) = observer {
                assert_eq!(observer.join().unwrap(), (true, true));
                cx.run_until_parked();
            }
            assert!(!crate::capture::timer::is_active());
            assert!(cx.update(crate::capture::overlay::parity_is_active));
            cx.background_executor
                .advance_clock(std::time::Duration::from_secs(10));
            cx.run_until_parked();
            assert!(cx.update(crate::capture::overlay::parity_is_active));
            assert!(notifications.borrow().is_empty());
            assert_eq!(std::fs::read_dir(&screenshots).unwrap().count(), 0);
            cx.update(crate::capture::overlay::close_all);
            cx.run_until_parked();
        }
        start_timer(&cx);
        cx.update(|cx| {
            crate::state::state(cx)
                .daemon
                .emit_for_test(poratake_daemon_common::contract::TIMER_CONTROL_COMPLETED_EVENT);
        });
        cx.run_until_parked();
        assert!(!crate::capture::timer::is_active());
        assert!(!cx.update(crate::capture::overlay::parity_is_active));
        assert_eq!(*notifications.borrow(), ["Capture Failed"]);
        notifications.borrow_mut().clear();
        let previous = crate::capture::timer::begin().unwrap();
        start_timer(&cx);
        assert!(!cx.update(crate::capture::overlay::parity_is_active));
        assert_eq!(*notifications.borrow(), ["Timer Capture Failed"]);
        drop(previous);
    }
}
