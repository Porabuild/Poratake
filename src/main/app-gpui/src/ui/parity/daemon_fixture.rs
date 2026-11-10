use std::fs::OpenOptions;
use std::io::{BufRead, Read, Write};
use std::path::Path;

use poratake_daemon_common::protocol::{self, Request};
use serde_json::json;

pub const PROCESS_ENV: &str = "PORATAKE_PARITY_DAEMON";

pub(super) fn calls(output: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(output.join("daemon-fixture.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn capture_fixture(output: &str) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(Path::new(output).join("capture-fixture.json")).unwrap())
        .unwrap()
}

fn wait_for_scroll_barrier(fixture: &serde_json::Value) {
    let Some(address) = fixture["barrier"].as_str() else {
        return;
    };
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    stream.write_all(&[1]).unwrap();
    stream.read_exact(&mut [0]).unwrap();
}

pub fn run() {
    let output = std::env::var("PORATAKE_PARITY_OUTPUT").unwrap();
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(Path::new(&output).join("daemon-fixture.jsonl"))
        .unwrap();
    protocol::send_event("system:ready", None);
    let mut frozen = false;
    let mut timer_visible = false;
    let mut scrolling = false;
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        let request: Request = serde_json::from_str(&line).unwrap();
        writeln!(log, "{line}").unwrap();
        let result = match (request.module.as_str(), request.method.as_str()) {
            ("system", "quit") => break,
            ("window-selector", "list") => json!({"windows": [{
                "windowId": 43,
                "title": "Fixture capture window",
                "ownerName": "Fixture",
                "ownerPid": 1,
                "bounds": {"x": 80., "y": 90., "width": 640., "height": 360.}
            }]}),
            ("freeze-screen", "freeze") => {
                match capture_fixture(&output)["freeze"].as_str().unwrap() {
                    "freeze-failed" => {
                        frozen = false;
                        protocol::respond_error(
                            &request.id,
                            "CAPTURE_FAILED",
                            "Fixture freeze failure",
                        );
                        continue;
                    }
                    "freeze-unavailable" => {
                        frozen = false;
                        json!({"frozen": false})
                    }
                    "frozen" => {
                        frozen = true;
                        json!({"frozen": true})
                    }
                    value => panic!("unexpected freeze case: {value}"),
                }
            }
            ("freeze-screen", "release") => {
                frozen = false;
                json!({"frozen": false})
            }
            ("timer-control", "show") => {
                assert!(!frozen, "timer capture must release the selection frame");
                assert!(
                    !timer_visible,
                    "a previous timer must be hidden before retry"
                );
                let params = request.params.as_ref().unwrap();
                assert_eq!(params["x"], 260);
                assert_eq!(params["y"], 40);
                assert_eq!(params["duration"], 5);
                let fixture = capture_fixture(&output);
                let result = fixture["timer"].as_str().unwrap();
                if result == "failed" {
                    protocol::respond_error(&request.id, "UI_ERROR", "Fixture timer start failure");
                    continue;
                }
                timer_visible = true;
                match result {
                    "completed" | "retry" => {
                        timer_visible = false;
                        protocol::send_event(
                            poratake_daemon_common::contract::TIMER_CONTROL_COMPLETED_EVENT,
                            None,
                        );
                    }
                    "cancelled" => {
                        timer_visible = false;
                        protocol::send_event(
                            poratake_daemon_common::contract::TIMER_CONTROL_CANCEL_EVENT,
                            None,
                        );
                    }
                    "timeout" => {}
                    value => panic!("unexpected timer case: {value}"),
                }
                protocol::respond_success(&request.id, json!({}));
                continue;
            }
            ("timer-control", "hide") => {
                timer_visible = false;
                json!({})
            }
            ("scroll-capture", "start") => {
                assert!(!frozen, "scroll capture must use a live screen");
                assert!(
                    !scrolling,
                    "a previous scroll session must be cleared before retry"
                );
                let fixture = capture_fixture(&output);
                let params = request.params.as_ref().unwrap();
                for key in ["x", "y", "width", "height"] {
                    assert_eq!(params[key], fixture["rect"][key]);
                }
                assert_eq!(params["scaleFactor"], 2.);
                assert_eq!(params["autoScrollSpeed"], "fast");
                assert_eq!(params["maxHeight"], 1800);
                assert_eq!(params["nativeControls"], !cfg!(target_os = "macos"));
                if fixture["scroll"] == "start-failed" {
                    protocol::respond_error(
                        &request.id,
                        "UI_ERROR",
                        "Fixture scroll start failure",
                    );
                    continue;
                }
                scrolling = true;
                if matches!(
                    fixture["scroll"].as_str(),
                    Some("early-cancel" | "cancel-held")
                ) {
                    scrolling = false;
                    protocol::send_event(
                        poratake_daemon_common::contract::SCROLL_CAPTURE_CANCELLED_EVENT,
                        None,
                    );
                } else {
                    protocol::send_event(
                        poratake_daemon_common::contract::SCROLL_CAPTURE_DONE_EVENT,
                        None,
                    );
                }
                protocol::respond_success(&request.id, json!({}));
                continue;
            }
            ("scroll-capture", "finish") => {
                assert!(scrolling);
                scrolling = false;
                let fixture = capture_fixture(&output);
                wait_for_scroll_barrier(&fixture);
                if fixture["scroll"] == "finish-failed" {
                    protocol::respond_error(
                        &request.id,
                        "CAPTURE_FAILED",
                        "Fixture scroll finish failure",
                    );
                    continue;
                }
                let params = request.params.as_ref().unwrap();
                let path = Path::new(params["outputPath"].as_str().unwrap());
                let success = fixture["scroll"] != "empty";
                if success {
                    image::RgbaImage::from_pixel(640, 900, image::Rgba([32, 168, 96, 255]))
                        .save(path)
                        .unwrap();
                }
                json!({"success": success, "outputPath": path, "width": 640, "height": 900, "frameCount": 3})
            }
            ("scroll-capture", "cancel") => {
                scrolling = false;
                wait_for_scroll_barrier(&capture_fixture(&output));
                json!({})
            }
            ("screenshot", method @ ("capture-area" | "capture-window")) => {
                let params = request.params.as_ref().unwrap();
                let fixture = capture_fixture(&output);
                assert_eq!(
                    params.get("windowId").and_then(serde_json::Value::as_i64),
                    fixture["windowId"].as_i64()
                );
                if method == "capture-area" {
                    for key in ["x", "y", "width", "height"] {
                        assert_eq!(params[key], fixture["rect"][key], "capture {key}");
                    }
                }
                let color =
                    if params.get("cached").and_then(serde_json::Value::as_bool) == Some(true) {
                        assert!(
                            frozen,
                            "the frozen frame must survive until capture completes"
                        );
                        image::Rgba([64, 112, 192, 255])
                    } else {
                        image::Rgba([240, 80, 64, 255])
                    };
                let path = Path::new(params["path"].as_str().unwrap());
                let width = fixture["rect"]["width"].as_u64().unwrap() as u32;
                let height = fixture["rect"]["height"].as_u64().unwrap() as u32;
                if fixture["outcome"] == "invalid-image" {
                    std::fs::write(path, b"invalid image").unwrap();
                } else {
                    let image = if fixture["pattern"] == "quadrants" {
                        image::RgbaImage::from_fn(width, height, |x, y| {
                            image::Rgba([
                                if x < width / 2 { 240 } else { 40 },
                                if y < height / 2 { 80 } else { 180 },
                                64,
                                255,
                            ])
                        })
                    } else {
                        image::RgbaImage::from_pixel(width, height, color)
                    };
                    image.save(path).unwrap();
                }
                if fixture["outcome"] == "capture-failed" {
                    protocol::respond_error(
                        &request.id,
                        "CAPTURE_FAILED",
                        "Fixture area capture failure",
                    );
                    continue;
                }
                json!({"path": path, "width": width, "height": height})
            }
            (module @ ("ocr" | "qrcode"), method @ ("recognize" | "detect")) => {
                let fixture = capture_fixture(&output);
                assert_eq!(fixture["analysis"], module);
                assert_eq!(
                    method,
                    if module == "ocr" {
                        "recognize"
                    } else {
                        "detect"
                    }
                );
                let params = request.params.as_ref().unwrap();
                let source = image::open(Path::new(params["imagePath"].as_str().unwrap())).unwrap();
                if module == "ocr" && !cfg!(target_os = "macos") {
                    assert_eq!((source.width(), source.height()), (1300, 731));
                    assert!(matches!(source, image::DynamicImage::ImageLuma8(_)));
                } else {
                    assert_eq!((source.width(), source.height()), (640, 360));
                }
                match fixture["outcome"].as_str().unwrap() {
                    "error" => {
                        protocol::respond_error(
                            &request.id,
                            "RECOGNITION_FAILED",
                            "Fixture recognition failure",
                        );
                        continue;
                    }
                    "empty" => json!({"text": " \n\t ", "payload": " \n\t "}),
                    "success" | "retry" => {
                        json!({"text": " \n Poratake fixture OCR \t ", "payload": " \n poratake://visual-fixture \t "})
                    }
                    value => panic!("unexpected analysis case: {value}"),
                }
            }
            ("desktop-wallpaper", "get") => json!({
                "type": "path",
                "value": Path::new(&std::env::var(crate::config::store::CONFIG_DIR_ENV).unwrap())
                    .join("capture.png")
            }),
            ("media-devices", "list") => {
                let kinds = &request.params.as_ref().unwrap()["kinds"];
                assert_eq!(kinds.as_array().unwrap().len(), 1);
                match kinds[0].as_str().unwrap() {
                    "microphone" => json!({
                        "microphones": [
                            {"id": "mic-internal", "label": "Built-in microphone"},
                            {"id": "mic-usb", "label": "USB microphone with a deliberately long device name"}
                        ],
                        "cameras": [],
                        "defaultMicrophoneId": "mic-internal",
                        "defaultCameraId": null
                    }),
                    "camera" => json!({
                        "microphones": [],
                        "cameras": [
                            {"id": "camera-internal", "label": "Built-in camera"},
                            {"id": "camera-usb", "label": "USB camera with a deliberately long device name"}
                        ],
                        "defaultMicrophoneId": null,
                        "defaultCameraId": "camera-internal"
                    }),
                    kind => panic!("unexpected device kind: {kind}"),
                }
            }
            ("recording-control", "listIOSDevices") => json!({"devices": []}),
            ("recording-overlay", "show" | "showWindow") => json!({"visible": true}),
            ("recording-overlay", "hide")
            | ("camera-preview", "show" | "hide" | "update" | "setContentProtection")
            | (
                "screen-recorder",
                "start" | "pause" | "resume" | "stop" | "setMicrophone" | "setSystemAudio"
                | "setCamera",
            ) => json!({}),
            (module, method) => {
                panic!("unexpected visual fixture call: {module} {method}");
            }
        };
        protocol::respond_success(&request.id, result);
    }
}
