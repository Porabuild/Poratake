use crate::capture::intent::CaptureIntent;
use crate::system::clipboard::ClipboardService;
use std::io::{Read, Write};

use super::daemon_fixture::calls;
use super::workflows::{area_picker, select_area, verify_saved_capture, SELECTED_RECT};
use super::{escape, take_notifications, HeadlessAppContext, Path};

pub(super) fn run(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path) {
    for name in [
        "selection-cancelled",
        "early-finish",
        "early-cancel",
        "start-failed",
        "finish-failed",
        "empty",
        "finish-held",
        "cancel-held",
        "retry",
    ] {
        let barrier = if name.ends_with("-held") {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap().to_string();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.read_exact(&mut [0]).unwrap();
                let active = crate::capture::scroll::is_active();
                stream.write_all(&[1]).unwrap();
                tx.send(active).unwrap();
            });
            Some((address, rx))
        } else {
            None
        };
        let captures = scratch.join(format!("scroll-{name}"));
        std::fs::create_dir_all(&captures).unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen", "rect": SELECTED_RECT, "windowId": null, "scroll": name,
                "barrier": barrier.as_ref().map(|(address, _)| address),
            }))
            .unwrap(),
        )
        .unwrap();
        let before = calls(output).len();
        let history: Vec<_> = crate::history_store::load_history()
            .into_iter()
            .map(|item| item.original_path)
            .collect();
        let sentinel = format!("clipboard before scroll {name}");
        take_notifications();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                config.screenshot.freeze_screen = true;
                config.scroll_capture.auto_scroll_speed = "fast".into();
                config.scroll_capture.max_height = 1800.;
            });
            ClipboardService::write_text(cx, sentinel.clone());
            crate::capture::start_area_selection(CaptureIntent::ScrollCapture, cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        if name == "selection-cancelled" {
            escape(cx, picker);
        } else {
            select_area(cx, picker);
        }
        cx.run_until_parked();
        if let Some((_, rx)) = barrier {
            assert!(
                rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap(),
                "{name}: scroll capture must remain active until the daemon responds"
            );
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
        let saved = matches!(name, "early-finish" | "finish-held" | "retry");
        if saved {
            verify_saved_capture(
                cx,
                if name == "finish-held" {
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
            assert_eq!(std::fs::read_dir(&captures).unwrap().count(), 0);
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
            _ => None,
        };
        assert_eq!(
            notifications,
            body.map(|body| ("Scroll Capture Failed".to_string(), body.to_string(), false))
                .into_iter()
                .collect::<Vec<_>>()
        );
        let calls: Vec<_> = calls(output).into_iter().skip(before).collect();
        assert!(!calls
            .iter()
            .any(|call| call["module"] == "freeze-screen" && call["method"] == "freeze"));
        assert!(!calls.iter().any(|call| call["module"] == "screenshot"));
        for (method, count) in [
            ("start", usize::from(name != "selection-cancelled")),
            (
                "finish",
                usize::from(matches!(
                    name,
                    "early-finish" | "finish-failed" | "empty" | "finish-held" | "retry"
                )),
            ),
            (
                "cancel",
                usize::from(matches!(name, "early-cancel" | "cancel-held")),
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
    }
    println!("scroll capture selection, early events, save/history/editor output, failures and retry passed");
}
