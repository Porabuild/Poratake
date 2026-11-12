use crate::capture::intent::CaptureIntent;
use crate::system::clipboard::ClipboardService;
use std::io::{Read, Write};

use super::daemon_fixture::calls;
use super::workflows::{area_picker, select_area, verify_saved_capture, SELECTED_RECT};
use super::{escape, take_notifications, HeadlessAppContext, Path};

struct ScrollThreadSpawnFailureGuard;

impl Drop for ScrollThreadSpawnFailureGuard {
    fn drop(&mut self) {
        crate::capture::coordinator::fail_next_scroll_cancel_thread_spawn(false);
        crate::capture::coordinator::fail_next_scroll_finish_thread_spawn(false);
    }
}

#[cfg(windows)]
pub(super) fn preview_corners(cx: &mut HeadlessAppContext, output: &Path, scale: f32) {
    use base64::Engine;

    let mut encoded = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        4,
        2,
        image::Rgba([255, 0, 0, 255]),
    ))
    .write_to(&mut encoded, image::ImageFormat::Png)
    .unwrap();
    let preview = cx.update(|cx| {
        crate::windows::scroll_capture::preview_for_parity(
            cx,
            base64::engine::general_purpose::STANDARD.encode(encoded.into_inner()),
        )
    });
    cx.update_window(preview, |_, window, _| window.set_scale_factor(scale))
        .unwrap();
    super::settle(cx, preview);
    let image = cx.capture_screenshot(preview).unwrap();
    image
        .save(output.join("scroll-preview-corners.png"))
        .unwrap();
    let center = image.get_pixel((100. * scale).round() as u32, (130. * scale).round() as u32);
    assert!(center[0] > 240 && center[1] < 10 && center[2] < 10);
    let right = (200. * scale).round() as u32 - 1;
    let top = (80. * scale).round() as u32;
    let bottom = (180. * scale).round() as u32 - 1;
    for (x, y) in [(0, top), (right, top), (0, bottom), (right, bottom)] {
        let corner = image.get_pixel(x, y);
        let difference: u16 = corner.0[..3]
            .iter()
            .zip(&center.0[..3])
            .map(|(actual, source)| u16::from(actual.abs_diff(*source)))
            .sum();
        assert!(
            difference >= 60,
            "square preview corner at {x}, {y}: {corner:?}"
        );
    }
    cx.update(crate::windows::scroll_capture::ScrollCaptureSession::close);
}

pub(super) fn run(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path) {
    #[cfg(windows)]
    icon_hides_wait_for_an_in_flight_show(cx, output);
    #[cfg(windows)]
    icon_visibility_recovers_from_a_lost_reply(cx, output);
    #[cfg(windows)]
    failed_icon_hides_stop_captures(cx, output);
    #[cfg(windows)]
    tray_icon_toggles_are_serialized(cx, output);
    #[cfg(windows)]
    replacement_waits_for_icons_before_freezing(cx, output);
    #[cfg(windows)]
    replacement_recovers_from_failed_icon_restore(cx, output);
    #[cfg(windows)]
    scroll_start_returns_before_daemon_reply(cx, output, scratch);
    for name in [
        "selection-cancelled",
        "early-finish",
        "early-cancel",
        "cancel-spawn-failed",
        "finish-spawn-failed",
        "start-failed",
        "finish-failed",
        "finish-partial-failed",
        "empty",
        "finish-held",
        "cancel-finishing-held",
        "retry",
        "retryable-success",
        "retryable-cancel",
        "finish-long-held",
        "cancel-held",
        "collision",
        "interrupted",
        "burst-finish",
        "burst-cancel",
    ] {
        if name == "finish-long-held"
            && std::env::var("PORATAKE_PARITY_THEME").as_deref() != Ok("dark")
        {
            continue;
        }
        let barrier = if name.ends_with("-held") {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap().to_string();
            let (tx, rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.read_exact(&mut [0]).unwrap();
                if name == "finish-long-held" {
                    std::thread::sleep(std::time::Duration::from_secs(31));
                }
                let active = crate::capture::scroll::is_active();
                let icons_hidden = crate::capture::desktop_icons::are_hidden();
                tx.send((active, icons_hidden)).unwrap();
                if name == "cancel-finishing-held" {
                    release_rx
                        .recv_timeout(std::time::Duration::from_secs(10))
                        .unwrap();
                }
                stream.write_all(&[1]).unwrap();
            });
            Some((address, rx, release_tx))
        } else {
            None
        };
        let cancel_barrier = if name == "cancel-finishing-held" {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap().to_string();
            let (received_tx, received_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.read_exact(&mut [0]).unwrap();
                received_tx.send(()).unwrap();
                release_rx
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
                stream.write_all(&[1]).unwrap();
            });
            Some((address, received_rx, release_tx))
        } else {
            None
        };
        let retry_barrier = if matches!(name, "retryable-success" | "retryable-cancel") {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap().to_string();
            let (ready_tx, ready_rx) = std::sync::mpsc::channel();
            let (action_tx, action_rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.read_exact(&mut [0]).unwrap();
                ready_tx.send(()).unwrap();
                let action = action_rx
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
                stream.write_all(&[action]).unwrap();
            });
            Some((address, ready_rx, action_tx))
        } else {
            None
        };
        let captures = scratch.join(format!("scroll-{name}"));
        std::fs::create_dir_all(&captures).unwrap();
        let existing = captures.join("Collision.png");
        if name == "collision" {
            std::fs::write(&existing, b"previous capture").unwrap();
        }
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen", "rect": SELECTED_RECT, "windowId": null, "scroll": name,
                "barrier": barrier.as_ref().map(|(address, _, _)| address),
                "cancelBarrier": cancel_barrier.as_ref().map(|(address, _, _)| address),
                "retryBarrier": retry_barrier.as_ref().map(|(address, _, _)| address),
            }))
            .unwrap(),
        )
        .unwrap();
        let before = calls(output).len();
        let retryable_count = crate::capture::coordinator::scroll_retryable_finish_count();
        let history: Vec<_> = crate::history_store::load_history()
            .into_iter()
            .map(|item| item.original_path)
            .collect();
        let sentinel = format!("clipboard before scroll {name}");
        take_notifications();
        let _thread_spawn_failure = match name {
            "cancel-spawn-failed" => {
                crate::capture::coordinator::fail_next_scroll_cancel_thread_spawn(true);
                Some(ScrollThreadSpawnFailureGuard)
            }
            "finish-spawn-failed" => {
                crate::capture::coordinator::fail_next_scroll_finish_thread_spawn(true);
                Some(ScrollThreadSpawnFailureGuard)
            }
            _ => None,
        };
        let pattern = cx.update(|cx| {
            let state = crate::state::state(cx);
            let pattern = state.config.get().storage.naming_pattern;
            state.config.update(|config| {
                config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                if name == "collision" {
                    config.storage.naming_pattern = "Collision".into();
                }
                config.screenshot.freeze_screen = true;
                config.scroll_capture.auto_scroll_speed = "fast".into();
                config.scroll_capture.max_height = 1800.;
                if cfg!(windows) && matches!(name, "finish-held" | "cancel-held") {
                    config.screenshot.hide_desktop_icons = true;
                }
            });
            ClipboardService::write_text(cx, sentinel.clone());
            crate::capture::start_area_selection(CaptureIntent::ScrollCapture, cx);
            pattern
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        if name == "selection-cancelled" {
            escape(cx, picker);
        } else {
            select_area(cx, picker);
        }
        cx.run_until_parked();
        if let Some((_, rx, release)) = barrier {
            let (active, icons_hidden) = rx
                .recv_timeout(std::time::Duration::from_secs(
                    if name == "finish-long-held" { 40 } else { 10 },
                ))
                .unwrap();
            assert!(
                active,
                "{name}: scroll capture must remain active until the daemon responds"
            );
            if cfg!(windows) && matches!(name, "finish-held" | "cancel-held") {
                assert!(
                    !icons_hidden,
                    "desktop icons must be restored before {name} completes"
                );
            }
            if name == "cancel-finishing-held" {
                let (_, received, release_cancel) = cancel_barrier.as_ref().unwrap();
                received
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
                release_cancel.send(()).unwrap();
                cx.run_until_parked();
                assert!(crate::capture::scroll::is_active());
                let methods: Vec<_> = calls(output)
                    .into_iter()
                    .skip(before)
                    .filter(|call| call["module"] == "scroll-capture")
                    .map(|call| call["method"].as_str().unwrap().to_owned())
                    .collect();
                assert_eq!(
                    methods,
                    vec!["start".to_owned(), "finish".to_owned(), "cancel".to_owned()]
                );
                assert!(
                    !crate::capture::scroll::begin(),
                    "a retry must remain gated while the finish writer is held"
                );
                release.send(()).unwrap();
            }
            cx.run_until_parked();
        }
        if let Some((_, ready, action)) = retry_barrier {
            ready
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            let started = std::time::Instant::now();
            while crate::capture::coordinator::scroll_retryable_finish_count() == retryable_count {
                cx.run_until_parked();
                assert!(
                    started.elapsed() < std::time::Duration::from_secs(10),
                    "{name}: coordinator did not observe the retryable Finish response"
                );
                std::thread::yield_now();
            }
            assert_eq!(
                crate::capture::coordinator::scroll_retryable_finish_count(),
                retryable_count + 1,
                "{name}: exactly one retryable Finish response must be observed"
            );
            assert!(
                crate::capture::scroll::is_active(),
                "{name}: retryable Finish must keep the capture session active"
            );
            action
                .send(if name == "retryable-success" { 1 } else { 2 })
                .unwrap();
            let started = std::time::Instant::now();
            loop {
                cx.run_until_parked();
                let methods = calls(output)
                    .into_iter()
                    .skip(before)
                    .filter(|call| call["module"] == "scroll-capture")
                    .map(|call| call["method"].as_str().unwrap().to_owned())
                    .collect::<Vec<_>>();
                let finish_paths = calls(output)
                    .into_iter()
                    .skip(before)
                    .filter(|call| call["module"] == "scroll-capture" && call["method"] == "finish")
                    .map(|call| {
                        Path::new(call["params"]["outputPath"].as_str().unwrap()).to_path_buf()
                    })
                    .collect::<Vec<_>>();
                let output_settled = if name == "retryable-success" {
                    methods.iter().filter(|method| *method == "finish").count() == 2
                        && std::fs::read_dir(&captures).unwrap().count() == 1
                } else {
                    methods.iter().any(|method| method == "cancel")
                        && std::fs::read_dir(&captures).unwrap().count() == 0
                };
                let temporary_settled = finish_paths.iter().all(|path| !path.exists());
                if !crate::capture::scroll::is_active() && output_settled && temporary_settled {
                    break;
                }
                assert!(
                    started.elapsed() < std::time::Duration::from_secs(10),
                    "{name}: retry session/output did not settle"
                );
                std::thread::yield_now();
            }
        }
        if matches!(name, "cancel-spawn-failed" | "finish-spawn-failed") {
            let started = std::time::Instant::now();
            loop {
                cx.run_until_parked();
                let cancel_sent = calls(output)
                    .into_iter()
                    .skip(before)
                    .any(|call| call["module"] == "scroll-capture" && call["method"] == "cancel");
                if cancel_sent && !crate::capture::scroll::is_active() {
                    break;
                }
                assert!(
                    started.elapsed() < std::time::Duration::from_secs(10),
                    "{name}: cancellation did not settle after thread creation failure"
                );
                std::thread::yield_now();
            }
        }
        assert!(
            !crate::capture::scroll::is_active(),
            "{name}: scroll session must settle"
        );
        assert!(
            !cx.update(crate::capture::overlay::parity_is_active),
            "{name}: selector must close"
        );
        assert_eq!(
            cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
            Some(sentinel)
        );
        let saved = matches!(
            name,
            "early-finish"
                | "finish-held"
                | "finish-long-held"
                | "retry"
                | "retryable-success"
                | "collision"
                | "interrupted"
                | "burst-finish"
        );
        if saved {
            if name == "collision" {
                assert_eq!(std::fs::read(&existing).unwrap(), b"previous capture");
                assert!(captures.join("Collision (1).png").is_file());
                std::fs::remove_file(existing).unwrap();
            }
            verify_saved_capture(
                cx,
                if matches!(
                    name,
                    "finish-held"
                        | "finish-long-held"
                        | "collision"
                        | "interrupted"
                        | "burst-finish"
                ) {
                    scratch
                } else {
                    output
                },
                &captures,
                (
                    poratake_daemon_common::geometry::CaptureRect {
                        height: 900,
                        ..SELECTED_RECT
                    },
                    [32, 168, 96, 255],
                ),
                &format!("workflow-scroll-{name}"),
            );
        } else {
            assert_eq!(
                std::fs::read_dir(&captures).unwrap().count(),
                usize::from(name == "finish-partial-failed")
            );
            assert_eq!(
                crate::history_store::load_history()
                    .into_iter()
                    .map(|item| item.original_path)
                    .collect::<Vec<_>>(),
                history
            );
        }
        let notifications = take_notifications();
        let body = match name {
            "start-failed" => Some("The scroll capture could not be started"),
            "finish-failed" | "empty" => Some("The scroll capture produced no image"),
            "finish-spawn-failed" => Some("parity-injected scroll finish thread creation failure"),
            _ => None,
        };
        let expected = if name == "finish-partial-failed" {
            let output_path = calls(output)
                .into_iter()
                .rev()
                .find(|call| call["module"] == "scroll-capture" && call["method"] == "finish")
                .unwrap()["params"]["outputPath"]
                .as_str()
                .unwrap()
                .to_owned();
            assert_eq!(
                std::fs::read(&output_path).unwrap(),
                b"partial scroll image"
            );
            vec![(
                "Scroll Capture Failed".to_owned(),
                format!(
                    "Fixture scroll finish failure. Capture data is available at {output_path}"
                ),
                false,
            )]
        } else if matches!(name, "burst-finish" | "burst-cancel") {
            vec![
                (
                    "Scroll Capture Stopped".to_string(),
                    "A frame could not be captured. You can save the captured portion.".to_string(),
                    false,
                );
                20
            ]
        } else if name == "interrupted" {
            [
                "The latest frame could not be aligned. You can save the captured portion.",
                "A frame could not be captured. You can save the captured portion.",
                "Scrolling could not be continued. You can save the captured portion.",
            ]
            .into_iter()
            .map(|body| {
                (
                    "Scroll Capture Stopped".to_string(),
                    body.to_string(),
                    false,
                )
            })
            .collect::<Vec<_>>()
        } else {
            body.map(|body| ("Scroll Capture Failed".to_string(), body.to_string(), false))
                .into_iter()
                .collect::<Vec<_>>()
        };
        assert_eq!(notifications, expected);
        if name == "finish-partial-failed" {
            let output_path = calls(output)
                .into_iter()
                .rev()
                .find(|call| call["module"] == "scroll-capture" && call["method"] == "finish")
                .unwrap()["params"]["outputPath"]
                .as_str()
                .unwrap()
                .to_owned();
            std::fs::remove_file(output_path).unwrap();
        }
        let calls: Vec<_> = calls(output).into_iter().skip(before).collect();
        if cfg!(windows) && matches!(name, "finish-held" | "cancel-held") {
            let position = |module: &str, method: &str| {
                calls
                    .iter()
                    .position(|call| call["module"] == module && call["method"] == method)
                    .unwrap()
            };
            assert!(position("desktop-helper", "hide") < position("scroll-capture", "start"));
            assert!(position("scroll-capture", "start") < position("desktop-helper", "show"));
            let terminal = if name == "finish-held" {
                "finish"
            } else {
                "cancel"
            };
            assert!(position("desktop-helper", "show") < position("scroll-capture", terminal));
            cx.update(|cx| {
                crate::state::state(cx)
                    .config
                    .update(|config| config.screenshot.hide_desktop_icons = false)
            });
        }
        assert!(!calls
            .iter()
            .any(|call| call["module"] == "freeze-screen" && call["method"] == "freeze"));
        assert!(!calls.iter().any(|call| call["module"] == "screenshot"));
        for (method, count) in [
            ("start", usize::from(name != "selection-cancelled")),
            (
                "finish",
                if name == "retryable-success" {
                    2
                } else {
                    usize::from(matches!(
                        name,
                        "early-finish"
                            | "finish-failed"
                            | "finish-partial-failed"
                            | "empty"
                            | "finish-held"
                            | "cancel-finishing-held"
                            | "finish-long-held"
                            | "retry"
                            | "retryable-cancel"
                            | "collision"
                            | "interrupted"
                            | "burst-finish"
                    ))
                },
            ),
            (
                "cancel",
                usize::from(matches!(
                    name,
                    "early-cancel"
                        | "cancel-spawn-failed"
                        | "finish-spawn-failed"
                        | "cancel-held"
                        | "cancel-finishing-held"
                        | "retryable-cancel"
                        | "burst-cancel"
                )),
            ),
        ] {
            assert_eq!(
                calls
                    .iter()
                    .filter(|call| call["module"] == "scroll-capture" && call["method"] == method)
                    .count(),
                count,
                "{name}: {method} requests"
            );
        }
        if matches!(name, "retryable-success" | "retryable-cancel") {
            for call in calls
                .iter()
                .filter(|call| call["module"] == "scroll-capture" && call["method"] == "finish")
            {
                let output_path = call["params"]["outputPath"].as_str().unwrap();
                assert!(
                    !Path::new(output_path).exists(),
                    "{name}: finish attempt temp should be removed or promoted"
                );
            }
        }
        if name == "cancel-finishing-held" {
            let finish = calls
                .iter()
                .position(|call| call["module"] == "scroll-capture" && call["method"] == "finish")
                .unwrap();
            let cancel = calls
                .iter()
                .position(|call| call["module"] == "scroll-capture" && call["method"] == "cancel")
                .unwrap();
            assert!(finish < cancel, "Finish must be enqueued before Cancel");
        }
        if name == "collision" {
            cx.update(|cx| {
                crate::state::state(cx)
                    .config
                    .update(|config| config.storage.naming_pattern = pattern);
            });
        }
    }
    println!("scroll capture selection, early events, save/history/editor output, failures and retry passed");
}

#[cfg(windows)]
fn scroll_start_returns_before_daemon_reply(
    cx: &mut HeadlessAppContext,
    output: &Path,
    scratch: &Path,
) {
    use poratake_daemon_common::geometry::DisplayOrigin;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let (returned_tx, returned_rx) = std::sync::mpsc::channel();
    let (observed_tx, observed_rx) = std::sync::mpsc::channel();
    let address = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.read_exact(&mut [0]).unwrap();
        let returned = returned_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .is_ok();
        stream.write_all(&[1]).unwrap();
        observed_tx.send(returned).unwrap();
    });
    let captures = scratch.join("scroll-start-held");
    std::fs::create_dir_all(&captures).unwrap();
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "live", "rect": SELECTED_RECT, "windowId": null,
            "scroll": "start-held", "barrier": address,
        }))
        .unwrap(),
    )
    .unwrap();
    let before = calls(output).len();
    take_notifications();
    assert!(crate::capture::desktop_icons::pending_transitions().is_empty());
    assert!(!crate::capture::desktop_icons::needs_restoration());
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.screenshots_path = captures.to_string_lossy().into_owned();
            config.screenshot.hide_desktop_icons = false;
            config.screenshot.show_preview = true;
            config.screenshot.capture_to_clipboard = false;
            config.screenshot.auto_copy_to_clipboard = false;
            config.scroll_capture.auto_scroll_speed = "fast".into();
            config.scroll_capture.max_height = 1800.;
        });
        let capture = crate::capture::DisplayCapture::new(
            SELECTED_RECT,
            2.,
            DisplayOrigin { x: 0, y: 0 },
            None,
        );
        crate::state::coordinator(cx).update(cx, |coordinator, cx| {
            coordinator.capture_area_reserved(
                capture,
                CaptureIntent::ScrollCapture,
                None,
                None,
                None,
                None,
                cx,
            );
        });
    });
    assert!(crate::capture::scroll::is_active());
    cx.update(|cx| {
        crate::intents::dispatch(crate::system::tray::Intent::ScrollCapture, None, cx);
        assert!(!crate::capture::overlay::parity_is_active(cx));
    });
    let _ = returned_tx.send(());
    cx.run_until_parked();
    assert!(
        observed_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap(),
        "scroll capture startup blocked the UI until the daemon replied"
    );
    assert!(!crate::capture::scroll::is_active());
    assert!(take_notifications().is_empty());
    let recorded = calls(output);
    let methods: Vec<_> = recorded[before..]
        .iter()
        .filter(|call| call["module"] == "scroll-capture")
        .map(|call| call["method"].as_str().unwrap())
        .collect();
    assert_eq!(methods, ["start", "finish"]);
    verify_saved_capture(
        cx,
        scratch,
        &captures,
        (
            poratake_daemon_common::geometry::CaptureRect {
                height: 900,
                ..SELECTED_RECT
            },
            [32, 168, 96, 255],
        ),
        "workflow-scroll-start-held",
    );
}

#[cfg(windows)]
fn icon_hides_wait_for_an_in_flight_show(cx: &mut HeadlessAppContext, output: &Path) {
    use crate::capture::desktop_icons::{self, HideSource};
    use std::sync::mpsc;
    use std::time::Duration;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "scroll": "icons-show-held",
            "barrier": listener.local_addr().unwrap().to_string(),
        }))
        .unwrap(),
    )
    .unwrap();
    let before = calls(output).len();
    let daemon = cx.update(|cx| crate::state::state(cx).daemon.clone());
    assert!(desktop_icons::hide(&daemon, HideSource::Capture));
    let showing_daemon = daemon.clone();
    let showing =
        std::thread::spawn(move || desktop_icons::show(&showing_daemon, HideSource::Capture));
    let (mut stream, _) = listener.accept().unwrap();
    stream.read_exact(&mut [0]).unwrap();

    let hiding_daemon = daemon.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let hiding = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result = desktop_icons::hide(&hiding_daemon, HideSource::Capture);
        finished_tx.send(result).unwrap();
    });
    started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let early = finished_rx.recv_timeout(Duration::from_millis(500)).ok();
    stream.write_all(&[1]).unwrap();
    let shown = showing.join().unwrap();
    let hidden =
        early.unwrap_or_else(|| finished_rx.recv_timeout(Duration::from_secs(10)).unwrap());
    hiding.join().unwrap();
    let still_hidden = desktop_icons::are_hidden();
    let restored = desktop_icons::show(&daemon, HideSource::Capture);
    let methods: Vec<_> = calls(output)
        .into_iter()
        .skip(before)
        .filter(|call| call["module"] == "desktop-helper")
        .map(|call| call["method"].as_str().unwrap().to_string())
        .collect();
    assert!(
        early.is_none(),
        "a new hide returned while a show was still in flight"
    );
    assert!(shown && hidden && still_hidden && restored);
    assert_eq!(methods, ["hide", "show", "hide", "show"]);
}

#[cfg(windows)]
fn icon_visibility_recovers_from_a_lost_reply(cx: &mut HeadlessAppContext, output: &Path) {
    use crate::capture::desktop_icons::{self, HideSource};

    let daemon = cx.update(|cx| crate::state::state(cx).daemon.clone());
    for scenario in ["icons-hide-uncertain", "icons-show-uncertain"] {
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({"scroll": scenario})).unwrap(),
        )
        .unwrap();
        let before = calls(output).len();
        let hidden = desktop_icons::hide(&daemon, HideSource::Capture);
        assert_eq!(hidden, scenario != "icons-hide-uncertain");
        let shown = desktop_icons::show(&daemon, HideSource::Capture);
        assert_eq!(shown, scenario != "icons-show-uncertain");
        if scenario == "icons-show-uncertain" {
            assert!(desktop_icons::hide(&daemon, HideSource::Capture));
            assert!(desktop_icons::show(&daemon, HideSource::Capture));
        }
        let methods: Vec<_> = calls(output)
            .into_iter()
            .skip(before)
            .filter(|call| call["module"] == "desktop-helper")
            .map(|call| call["method"].as_str().unwrap().to_string())
            .collect();
        let expected = if scenario == "icons-hide-uncertain" {
            vec!["hide", "show"]
        } else {
            vec!["hide", "show", "hide", "show"]
        };
        assert_eq!(methods, expected, "{scenario}");
        assert!(!desktop_icons::are_hidden());
    }
}

#[cfg(windows)]
fn failed_icon_hides_stop_captures(cx: &mut HeadlessAppContext, output: &Path) {
    use std::sync::atomic::Ordering;

    let mut previous_picker = None;
    for kind in ["area", "display", "screen"] {
        for error in ["not-applied", "lost-reply"] {
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&serde_json::json!({
                    "freeze": "frozen", "desktopHideError": error,
                }))
                .unwrap(),
            )
            .unwrap();
            let before = calls(output).len();
            take_notifications();
            crate::capture::DUPLICATE_SCREEN_TARGET_FOR_PARITY
                .store(kind == "display", Ordering::SeqCst);
            cx.update(|cx| {
                crate::state::state(cx).config.update(|config| {
                    config.screenshot.freeze_screen = true;
                    config.screenshot.hide_desktop_icons = true;
                });
                if kind == "area" {
                    crate::capture::start_area_capture(cx);
                } else {
                    crate::capture::start_screen_capture(cx);
                }
            });
            if kind == "screen" {
                cx.update_window(previous_picker.unwrap(), |_, _, _| ())
                    .unwrap();
            }
            cx.run_until_parked();
            crate::capture::DUPLICATE_SCREEN_TARGET_FOR_PARITY.store(false, Ordering::SeqCst);
            let recorded = calls(output);
            let recorded = &recorded[before..];
            assert!(
                !cx.update(crate::capture::overlay::parity_is_active),
                "{kind} {error}"
            );
            assert!(
                !recorded.iter().any(|call| call["module"] == "freeze-screen"
                    || call["module"] == "screenshot"),
                "{kind} {error}: {recorded:?}"
            );
            let methods: Vec<_> = recorded
                .iter()
                .filter(|call| call["module"] == "desktop-helper")
                .map(|call| call["method"].as_str().unwrap())
                .collect();
            assert_eq!(methods, ["hide", "show"], "{kind} {error}");
            assert_eq!(
                take_notifications(),
                [(
                    "Capture Failed".into(),
                    "Could not hide desktop icons".into(),
                    false
                )],
                "{kind} {error}"
            );
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&serde_json::json!({
                    "freeze": "frozen", "expectDesktopVisible": true,
                }))
                .unwrap(),
            )
            .unwrap();
            cx.update(|cx| crate::capture::start_area_selection(CaptureIntent::Recording, cx));
            cx.run_until_parked();
            assert_eq!(
                cx.update(crate::capture::overlay::active_intents_for_test),
                [CaptureIntent::Recording],
                "{kind} {error}"
            );
            let reservation = cx.update(|cx| crate::state::state(cx).reserve_cached_capture());
            assert!(reservation.wait_for_freeze(), "{kind} {error}");
            drop(reservation);
            previous_picker = Some(area_picker(cx));
            cx.update(crate::capture::overlay::close_all);
            cx.run_until_parked();
        }
    }
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "frozen", "desktopHideError": "lost-reply", "desktopShowError": true,
        }))
        .unwrap(),
    )
    .unwrap();
    let before = calls(output).len();
    take_notifications();
    cx.update(crate::capture::start_area_capture);
    cx.run_until_parked();
    assert!(!cx.update(crate::capture::overlay::parity_is_active));
    assert!(crate::capture::desktop_icons::needs_restoration());
    assert_eq!(
        take_notifications(),
        [(
            "Capture Failed".into(),
            "Could not restore desktop icons".into(),
            false
        )]
    );
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "frozen", "expectDesktopVisible": true,
        }))
        .unwrap(),
    )
    .unwrap();
    cx.update(|cx| crate::capture::start_area_selection(CaptureIntent::Recording, cx));
    cx.run_until_parked();
    let reservation = cx.update(|cx| crate::state::state(cx).reserve_cached_capture());
    assert!(reservation.wait_for_freeze());
    drop(reservation);
    let recorded = calls(output);
    let methods: Vec<_> = recorded[before..]
        .iter()
        .filter(|call| call["module"] == "desktop-helper")
        .map(|call| call["method"].as_str().unwrap())
        .collect();
    assert_eq!(methods, ["hide", "show", "show"]);
    assert!(!crate::capture::desktop_icons::needs_restoration());

    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "frozen", "desktopHideError": "not-applied",
        }))
        .unwrap(),
    )
    .unwrap();
    let before = calls(output).len();
    take_notifications();
    cx.update(crate::capture::start_area_capture);
    cx.run_until_parked();
    let recorded = calls(output);
    let recorded = &recorded[before..];
    assert!(!cx.update(crate::capture::overlay::parity_is_active));
    assert!(recorded
        .iter()
        .any(|call| call["module"] == "freeze-screen" && call["method"] == "release"));
    assert!(!recorded
        .iter()
        .any(|call| call["method"] == "freeze" || call["module"] == "screenshot"));
    assert_eq!(
        take_notifications(),
        [(
            "Capture Failed".into(),
            "Could not hide desktop icons".into(),
            false
        )]
    );
    for (intent, title) in [
        (CaptureIntent::Ocr, "OCR Failed"),
        (CaptureIntent::QrCode, "Scan Failed"),
    ] {
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen", "desktopHideError": "not-applied",
            }))
            .unwrap(),
        )
        .unwrap();
        take_notifications();
        cx.update(|cx| crate::capture::start_area_selection(intent, cx));
        cx.run_until_parked();
        let picker = area_picker(cx);
        let before = calls(output).len();
        select_area(cx, picker);
        cx.run_until_parked();
        let recorded = calls(output);
        let recorded = &recorded[before..];
        assert!(!recorded.iter().any(|call| call["module"] == "screenshot"));
        assert!(recorded
            .iter()
            .any(|call| call["module"] == "freeze-screen" && call["method"] == "release"));
        assert_eq!(
            take_notifications(),
            [(title.into(), "Could not hide desktop icons".into(), false)]
        );
    }
    cx.update(|cx| {
        crate::state::state(cx)
            .config
            .update(|config| config.screenshot.hide_desktop_icons = false);
    });
    println!("failed icon hides stop capture and restore desktop visibility");
}

#[cfg(windows)]
fn tray_icon_toggles_are_serialized(cx: &mut HeadlessAppContext, output: &Path) {
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({"scroll": "icons-tray"})).unwrap(),
    )
    .unwrap();
    let before = calls(output).len();
    cx.update(|cx| {
        for _ in 0..2 {
            crate::intents::dispatch(crate::system::tray::Intent::ToggleDesktopIcons, None, cx);
        }
    });
    cx.run_until_parked();
    let methods: Vec<_> = calls(output)
        .into_iter()
        .skip(before)
        .filter(|call| call["module"] == "desktop-helper")
        .map(|call| call["method"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(methods, ["hide", "show"]);
    assert!(!crate::capture::desktop_icons::are_hidden());
}

#[cfg(windows)]
fn replacement_waits_for_icons_before_freezing(cx: &mut HeadlessAppContext, output: &Path) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "frozen", "scroll": "icons-show-held-replacement",
            "barrier": listener.local_addr().unwrap().to_string(),
        }))
        .unwrap(),
    )
    .unwrap();
    let before = calls(output).len();
    let daemon_output = output.to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.read_exact(&mut [0]).unwrap();
        let frozen = calls(&daemon_output)[before..]
            .iter()
            .any(|call| call["module"] == "freeze-screen" && call["method"] == "freeze");
        stream.write_all(&[1]).unwrap();
        tx.send(frozen).unwrap();
    });
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.screenshot.freeze_screen = true;
            config.screenshot.hide_desktop_icons = true;
        });
        crate::capture::start_area_selection(CaptureIntent::ScrollCapture, cx);
    });
    cx.run_until_parked();
    assert!(cx.update(crate::capture::overlay::parity_is_active));
    cx.update(|cx| {
        crate::capture::start_area_selection(CaptureIntent::Recording, cx);
        assert!(!crate::capture::overlay::parity_is_active(cx));
    });
    cx.run_until_parked();
    assert!(
        !rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap(),
        "replacement froze the screen before desktop icons were restored"
    );
    assert_eq!(
        cx.update(crate::capture::overlay::active_intents_for_test),
        [CaptureIntent::Recording]
    );
    let calls = calls(output);
    let calls = &calls[before..];
    let position = |module: &str, method: &str| {
        calls
            .iter()
            .position(|call| call["module"] == module && call["method"] == method)
            .unwrap()
    };
    assert!(position("desktop-helper", "hide") < position("desktop-helper", "show"));
    assert!(position("desktop-helper", "show") < position("freeze-screen", "freeze"));
    cx.update(|cx| {
        crate::capture::overlay::close_all(cx);
        crate::state::state(cx)
            .config
            .update(|config| config.screenshot.hide_desktop_icons = false);
    });
    cx.run_until_parked();
}

#[cfg(windows)]
fn replacement_recovers_from_failed_icon_restore(cx: &mut HeadlessAppContext, output: &Path) {
    use crate::capture::desktop_icons::{self, HideSource};

    let daemon = cx.update(|cx| crate::state::state(cx).daemon.clone());
    for scenario in ["icons-show-not-applied", "icons-show-never"] {
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen", "scroll": scenario, "expectDesktopVisible": true,
            }))
            .unwrap(),
        )
        .unwrap();
        let before = calls(output).len();
        take_notifications();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.screenshot.freeze_screen = true;
                config.screenshot.hide_desktop_icons = true;
            });
            crate::capture::start_area_selection(CaptureIntent::ScrollCapture, cx);
        });
        cx.run_until_parked();
        cx.update(|cx| crate::capture::start_area_selection(CaptureIntent::Recording, cx));
        cx.run_until_parked();
        let calls = calls(output);
        let calls = &calls[before..];
        assert_eq!(
            calls
                .iter()
                .filter(|call| call["module"] == "desktop-helper" && call["method"] == "show")
                .count(),
            2,
            "{scenario}: restore must be retried"
        );
        if scenario == "icons-show-not-applied" {
            assert_eq!(
                cx.update(crate::capture::overlay::active_intents_for_test),
                [CaptureIntent::Recording]
            );
            assert!(calls
                .iter()
                .any(|call| call["module"] == "freeze-screen" && call["method"] == "freeze"));
            assert!(take_notifications().is_empty());
            cx.update(crate::capture::overlay::close_all);
            cx.run_until_parked();
        } else {
            assert!(!cx.update(crate::capture::overlay::parity_is_active));
            assert!(!calls.iter().any(|call| call["module"] == "freeze-screen"));
            assert_eq!(
                take_notifications(),
                [(
                    "Recording failed".into(),
                    "Could not restore desktop icons".into(),
                    false
                )]
            );
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&serde_json::json!({"scroll": "icons-restored"})).unwrap(),
            )
            .unwrap();
            assert!(desktop_icons::show(&daemon, HideSource::Menu));
        }
    }
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "live", "scroll": "icons-show-never", "windowId": null,
            "rect": {"x": 0, "y": 0, "width": 3840, "height": 2160},
        }))
        .unwrap(),
    )
    .unwrap();
    let before = calls(output).len();
    take_notifications();
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.screenshot.freeze_screen = false;
            config.screenshot.hide_desktop_icons = true;
        });
        crate::capture::start_area_selection(CaptureIntent::ScrollCapture, cx);
    });
    cx.run_until_parked();
    let picker = area_picker(cx);
    cx.update(|cx| {
        crate::state::state(cx)
            .config
            .update(|config| config.screenshot.hide_desktop_icons = false);
        crate::capture::start_screen_capture(cx);
    });
    cx.update_window(picker, |_, _, _| ()).unwrap();
    cx.run_until_parked();
    let recorded = calls(output);
    let recorded = &recorded[before..];
    let notifications = take_notifications();
    assert_eq!(
        recorded
            .iter()
            .filter(|call| call["module"] == "desktop-helper" && call["method"] == "show")
            .count(),
        2,
        "direct screen restore: {recorded:?}; notifications: {notifications:?}"
    );
    assert!(!recorded.iter().any(|call| call["module"] == "screenshot"));
    assert_eq!(
        notifications,
        [(
            "Capture Failed".into(),
            "Could not restore desktop icons".into(),
            false
        )]
    );
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({"scroll": "icons-restored"})).unwrap(),
    )
    .unwrap();
    assert!(desktop_icons::show(&daemon, HideSource::Menu));
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({"scroll": "icons-show-never"})).unwrap(),
    )
    .unwrap();
    let before = calls(output).len();
    assert!(desktop_icons::hide(&daemon, HideSource::Menu));
    assert!(!desktop_icons::show(&daemon, HideSource::Menu));
    take_notifications();
    cx.update(crate::capture::start_screen_capture);
    cx.update_window(picker, |_, _, _| ()).unwrap();
    for _ in 0..1000 {
        if calls(output)[before..]
            .iter()
            .filter(|call| call["module"] == "desktop-helper" && call["method"] == "show")
            .count()
            == 2
        {
            break;
        }
        assert!(cx.background_executor().tick(), "restore task did not run");
    }
    assert_eq!(
        calls(output)[before..]
            .iter()
            .filter(|call| call["module"] == "desktop-helper" && call["method"] == "show")
            .count(),
        2
    );
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({"scroll": "icons-restored"})).unwrap(),
    )
    .unwrap();
    cx.update(crate::capture::start_area_capture);
    cx.run_until_parked();
    assert_eq!(
        cx.update(crate::capture::overlay::active_intents_for_test),
        [CaptureIntent::Screenshot]
    );
    assert!(
        take_notifications().is_empty(),
        "replaced capture reported an error"
    );
    assert!(!calls(output)[before..]
        .iter()
        .any(|call| call["module"] == "screenshot"));
    cx.update(crate::capture::overlay::close_all);
    cx.run_until_parked();
    cx.update(|cx| {
        crate::state::state(cx)
            .config
            .update(|config| config.screenshot.hide_desktop_icons = false);
    });
}
