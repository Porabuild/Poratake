use crate::capture::intent::CaptureIntent;
use crate::system::clipboard::ClipboardService;

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
        "retry",
    ] {
        let captures = scratch.join(format!("scroll-{name}"));
        std::fs::create_dir_all(&captures).unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen", "rect": SELECTED_RECT, "windowId": null, "scroll": name,
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
        let saved = matches!(name, "early-finish" | "retry");
        if saved {
            verify_saved_capture(
                cx,
                output,
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
                    "early-finish" | "finish-failed" | "empty" | "retry"
                )),
            ),
            ("cancel", usize::from(name == "early-cancel")),
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
