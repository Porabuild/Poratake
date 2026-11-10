use crate::capture::intent::CaptureIntent;
use crate::system::clipboard::ClipboardService;

use super::daemon_fixture::calls;
use super::workflows::{area_picker, select_area, SELECTED_RECT};
use super::{escape, take_notifications, HeadlessAppContext, Path};

pub(super) fn run(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path) {
    for (intent, module, text, copied, empty, failed) in [
        (
            CaptureIntent::Ocr,
            "ocr",
            "Poratake fixture OCR",
            (
                "Text copied",
                "Recognized text has been copied to the clipboard",
            ),
            ("No Text Found", "No text was detected in the selected area"),
            ("OCR Failed", "Failed to extract text from the image"),
        ),
        (
            CaptureIntent::QrCode,
            "qrcode",
            "poratake://visual-fixture",
            (
                "QR Code Copied",
                "QR code value has been copied to clipboard",
            ),
            (
                "No QR Code Found",
                "No QR code was detected in the selected area",
            ),
            ("Scan Failed", "Failed to scan QR code from the image"),
        ),
    ] {
        for (name, freeze, freeze_case) in [
            ("success", true, "frozen"),
            ("empty", false, "live"),
            ("error", true, "freeze-failed"),
            ("capture-failed", true, "frozen"),
            ("cancelled", true, "frozen"),
            ("retry", true, "frozen"),
        ] {
            let captures = scratch.join(format!("{module}-{name}"));
            std::fs::create_dir_all(&captures).unwrap();
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&serde_json::json!({
                    "freeze": freeze_case,
                    "rect": SELECTED_RECT,
                    "windowId": null,
                    "analysis": module,
                    "outcome": name,
                }))
                .unwrap(),
            )
            .unwrap();
            let before = calls(output).len();
            let history: Vec<_> = crate::history_store::load_history()
                .into_iter()
                .map(|item| item.original_path)
                .collect();
            let sentinel = format!("clipboard before {module} {name}");
            take_notifications();
            cx.update(|cx| {
                crate::state::state(cx).config.update(|config| {
                    config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                    config.screenshot.freeze_screen = freeze;
                });
                ClipboardService::write_text(cx, sentinel.clone());
                crate::capture::start_area_selection(intent, cx);
            });
            cx.run_until_parked();
            let picker = area_picker(cx);
            if name == "cancelled" {
                escape(cx, picker);
            } else {
                select_area(cx, picker);
            }
            cx.run_until_parked();
            assert!(
                !cx.update(crate::capture::overlay::parity_is_active),
                "{module}-{name}: selector must close"
            );
            let succeeded = matches!(name, "success" | "retry");
            assert_eq!(
                cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
                Some(if succeeded {
                    text.to_string()
                } else {
                    sentinel
                }),
                "{module}-{name}: clipboard outcome"
            );
            let notification = match name {
                "success" | "retry" => Some((copied, true)),
                "empty" => Some((empty, false)),
                "error" => Some((failed, false)),
                "capture-failed" => {
                    Some(((failed.0, "Failed to capture the selected area"), false))
                }
                "cancelled" => None,
                _ => unreachable!(),
            };
            let expected: Vec<_> = notification
                .map(|((title, body), transient)| (title.to_string(), body.to_string(), transient))
                .into_iter()
                .collect();
            assert_eq!(
                take_notifications(),
                expected,
                "{module}-{name}: notification"
            );
            assert_eq!(std::fs::read_dir(&captures).unwrap().count(), 0);
            assert_eq!(
                crate::history_store::load_history()
                    .into_iter()
                    .map(|item| item.original_path)
                    .collect::<Vec<_>>(),
                history,
                "analysis must not enter screenshot history"
            );
            let calls: Vec<_> = calls(output).into_iter().skip(before).collect();
            assert_eq!(
                calls
                    .iter()
                    .filter(|call| call["module"] == "screenshot")
                    .count(),
                usize::from(name != "cancelled")
            );
            assert_eq!(
                calls.iter().filter(|call| call["module"] == module).count(),
                usize::from(!matches!(name, "cancelled" | "capture-failed"))
            );
            for call in &calls {
                let params = &call["params"];
                for key in ["path", "imagePath"] {
                    if let Some(path) = params[key].as_str() {
                        assert!(
                            !Path::new(path).exists(),
                            "{module}-{name}: temporary file remains: {path}"
                        );
                    }
                }
            }
        }
    }
    println!(
        "OCR/QR capture, clipboard, temporary-file cleanup, errors, cancellation and retry passed"
    );
}
