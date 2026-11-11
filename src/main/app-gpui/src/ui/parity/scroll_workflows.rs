use crate::capture::intent::CaptureIntent;
use crate::system::clipboard::ClipboardService;
use std::io::{Read, Write};

use super::daemon_fixture::calls;
use super::workflows::{area_picker, select_area, verify_saved_capture, SELECTED_RECT};
use super::{escape, take_notifications, HeadlessAppContext, Path};

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
    for name in [
        "selection-cancelled",
        "early-finish",
        "early-cancel",
        "start-failed",
        "finish-failed",
        "empty",
        "finish-held",
        "finish-long-held",
        "cancel-held",
        "retry",
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
            std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.read_exact(&mut [0]).unwrap();
                if name == "finish-long-held" {
                    std::thread::sleep(std::time::Duration::from_secs(31));
                }
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
                rx.recv_timeout(std::time::Duration::from_secs(
                    if name == "finish-long-held" { 40 } else { 10 }
                ))
                .unwrap(),
                "{name}: scroll capture must remain active until the daemon responds"
            );
            cx.run_until_parked();
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
            "early-finish" | "finish-held" | "finish-long-held" | "retry"
        );
        if saved {
            verify_saved_capture(
                cx,
                if matches!(name, "finish-held" | "finish-long-held") {
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
                    "early-finish"
                        | "finish-failed"
                        | "empty"
                        | "finish-held"
                        | "finish-long-held"
                        | "retry"
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
