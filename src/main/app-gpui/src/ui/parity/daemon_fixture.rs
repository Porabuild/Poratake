use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{BufRead, Read, Write};
use std::path::Path;

use poratake_daemon_common::contract::SCREEN_RECORDER_ERROR_EVENT;
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

fn optional_capture_fixture(output: &str) -> Option<serde_json::Value> {
    let path = Path::new(output).join("capture-fixture.json");
    match std::fs::read(path) {
        Ok(content) => Some(serde_json::from_slice(&content).unwrap()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("failed to read capture fixture: {error}"),
    }
}

fn wait_for_fixture_barrier(fixture: &serde_json::Value) {
    let Some(address) = fixture["barrier"].as_str() else {
        return;
    };
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(
            if fixture["scroll"] == "finish-long-held" {
                40
            } else {
                10
            },
        )))
        .unwrap();
    stream.write_all(&[1]).unwrap();
    stream.read_exact(&mut [0]).unwrap();
}

fn wait_for_scroll_cancel_barrier(fixture: &serde_json::Value) {
    let Some(address) = fixture["cancelBarrier"].as_str() else {
        return;
    };
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    stream.write_all(&[1]).unwrap();
    stream.read_exact(&mut [0]).unwrap();
}

fn wait_for_scroll_retry_barrier(fixture: &serde_json::Value) -> u8 {
    let Some(address) = fixture["retryBarrier"].as_str() else {
        return 0;
    };
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    stream.write_all(&[1]).unwrap();
    let mut action = [0];
    stream.read_exact(&mut action).unwrap();
    action[0]
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
    let mut frozen_desktop_icons_hidden = false;
    let mut freeze_release_failed = false;
    let mut timer_visible = false;
    let mut scrolling = false;
    let mut desktop_show_held = false;
    let mut desktop_show_hold_keys = HashSet::new();
    let mut desktop_hide_failed = false;
    let mut desktop_show_failed = false;
    let mut desktop_show_not_applied = false;
    let mut scroll_retryable_finish_attempts = 0;
    let mut recording_start_failed_after_error = false;
    let mut recording_area_start_failed = false;
    let mut recording_target_closed_during_start = false;
    let mut recording_stop_failed_after_error = false;
    let mut held_recording_start: Option<String> = None;
    let mut held_recording_stop: Option<String> = None;
    let mut recording_stop_was_held = false;
    let mut recording_outline_hide_barrier = None;
    let desktop_icons_hidden = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
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
                let fixture = capture_fixture(&output);
                if fixture["expectDesktopVisible"] == true
                    && desktop_icons_hidden.load(std::sync::atomic::Ordering::SeqCst)
                {
                    protocol::respond_error(
                        &request.id,
                        "FIXTURE_ERROR",
                        "Desktop icons are hidden",
                    );
                    continue;
                }
                match fixture["freeze"].as_str().unwrap() {
                    "freeze-failed" => {
                        frozen = true;
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
                        frozen_desktop_icons_hidden =
                            desktop_icons_hidden.load(std::sync::atomic::Ordering::SeqCst);
                        json!({"frozen": true})
                    }
                    value => panic!("unexpected freeze case: {value}"),
                }
            }
            ("freeze-screen", "release") => {
                let fixture = capture_fixture(&output);
                if fixture["releaseFailure"] == "once" && !freeze_release_failed {
                    freeze_release_failed = true;
                    std::fs::write(
                        Path::new(&output).join("freeze-release-failed.json"),
                        serde_json::to_vec(&json!({
                            "frozen": frozen,
                        }))
                        .unwrap(),
                    )
                    .unwrap();
                    protocol::respond_error(
                        &request.id,
                        "FIXTURE_ERROR",
                        "Fixture first release failure",
                    );
                    continue;
                }
                frozen = false;
                if freeze_release_failed && fixture["releaseFailure"] == "once" {
                    std::fs::write(
                        Path::new(&output).join("freeze-release-succeeded.json"),
                        serde_json::to_vec(&json!({
                            "frozen": frozen,
                        }))
                        .unwrap(),
                    )
                    .unwrap();
                }
                if fixture["holdFreezeRelease"] == true {
                    std::thread::spawn(move || {
                        wait_for_fixture_barrier(&fixture);
                        protocol::respond_success(&request.id, json!({"frozen": false}));
                    });
                    continue;
                }
                json!({"frozen": false})
            }
            ("desktop-helper", "hide") => {
                let fixture = capture_fixture(&output);
                if let Some(error) = fixture["desktopHideError"].as_str() {
                    if error == "lost-reply" {
                        desktop_icons_hidden.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    protocol::respond_error(&request.id, "FIXTURE_ERROR", "Fixture hide failed");
                    continue;
                }
                if fixture["scroll"] == "icons-hide-uncertain" && !desktop_hide_failed {
                    desktop_hide_failed = true;
                    desktop_icons_hidden.store(true, std::sync::atomic::Ordering::SeqCst);
                    protocol::respond_error(
                        &request.id,
                        "FIXTURE_ERROR",
                        "Fixture lost hide reply",
                    );
                    continue;
                }
                if fixture["desktopHideHeld"] == true {
                    let hidden = desktop_icons_hidden.clone();
                    let id = request.id.clone();
                    std::thread::spawn(move || {
                        wait_for_fixture_barrier(&fixture);
                        hidden.store(true, std::sync::atomic::Ordering::SeqCst);
                        protocol::respond_success(&id, json!({}));
                    });
                    continue;
                }
                desktop_icons_hidden.store(true, std::sync::atomic::Ordering::SeqCst);
                json!({})
            }
            ("desktop-helper", "show") => {
                let fixture = capture_fixture(&output);
                let capture_show_held = fixture["desktopShowHeld"] == true
                    && desktop_show_hold_keys.insert(
                        fixture["desktopShowHoldKey"]
                            .as_str()
                            .unwrap_or("default")
                            .to_owned(),
                    );
                if (fixture["scroll"] == "icons-show-held" && !desktop_show_held)
                    || fixture["scroll"] == "icons-show-held-replacement"
                    || capture_show_held
                {
                    if !capture_show_held {
                        desktop_show_held = true;
                    }
                    wait_for_fixture_barrier(&fixture);
                }
                if fixture["desktopShowError"] == true {
                    protocol::respond_error(&request.id, "FIXTURE_ERROR", "Fixture show failed");
                    continue;
                }
                if (fixture["scroll"] == "icons-show-not-applied" && !desktop_show_not_applied)
                    || fixture["scroll"] == "icons-show-never"
                {
                    desktop_show_not_applied = true;
                    protocol::respond_error(&request.id, "FIXTURE_ERROR", "Fixture show failed");
                    continue;
                }
                desktop_icons_hidden.store(false, std::sync::atomic::Ordering::SeqCst);
                if fixture["scroll"] == "icons-show-uncertain" && !desktop_show_failed {
                    desktop_show_failed = true;
                    protocol::respond_error(
                        &request.id,
                        "FIXTURE_ERROR",
                        "Fixture lost show reply",
                    );
                    continue;
                }
                json!({})
            }
            ("timer-control", "show") => {
                assert!(!frozen, "timer capture must release the selection frame");
                assert!(
                    !timer_visible,
                    "a previous timer must be hidden before retry"
                );
                let params = request.params.as_ref().unwrap();
                let fixture = capture_fixture(&output);
                assert_eq!(params["x"], fixture["timerPosition"][0]);
                assert_eq!(params["y"], fixture["timerPosition"][1]);
                assert_eq!(params["duration"], 5);
                let result = fixture["timer"].as_str().unwrap();
                if result == "failed" {
                    protocol::respond_error(&request.id, "UI_ERROR", "Fixture timer start failure");
                    continue;
                }
                timer_visible = true;
                match result {
                    "completed" | "retry" | "empty-screenshot" => {
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
                if matches!(
                    fixture["scroll"].as_str(),
                    Some("retryable-success" | "retryable-cancel")
                ) {
                    scroll_retryable_finish_attempts = 0;
                }
                let params = request.params.as_ref().unwrap();
                for key in ["x", "y", "width", "height"] {
                    assert_eq!(params[key], fixture["rect"][key]);
                }
                assert_eq!(params["scaleFactor"], 2.);
                assert_eq!(params["autoScrollSpeed"], "fast");
                assert_eq!(params["maxHeight"], 1800);
                assert_eq!(params["nativeControls"], !cfg!(target_os = "macos"));
                if fixture["scroll"] == "start-held" {
                    wait_for_fixture_barrier(&fixture);
                }
                if fixture["scroll"] == "start-failed" {
                    protocol::respond_error(
                        &request.id,
                        "UI_ERROR",
                        "Fixture scroll start failure",
                    );
                    continue;
                }
                scrolling = true;
                if fixture["scroll"] == "interrupted" {
                    for reason in [
                        "duplicate",
                        "max-height",
                        "no-overlap",
                        "capture-error",
                        "input-error",
                    ] {
                        protocol::send_event(
                            poratake_daemon_common::contract::SCROLL_CAPTURE_SCROLL_ENDED_EVENT,
                            Some(json!({ "reason": reason, "frameCount": 2 })),
                        );
                    }
                }
                if matches!(
                    fixture["scroll"].as_str(),
                    Some("burst-finish" | "burst-cancel")
                ) {
                    for _ in 0..20 {
                        protocol::send_event(
                            poratake_daemon_common::contract::SCROLL_CAPTURE_SCROLL_ENDED_EVENT,
                            Some(json!({ "reason": "capture-error", "frameCount": 2 })),
                        );
                    }
                }
                if matches!(
                    fixture["scroll"].as_str(),
                    Some("early-cancel" | "cancel-spawn-failed" | "cancel-held" | "burst-cancel")
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
                if matches!(
                    fixture["scroll"].as_str(),
                    Some("retryable-success" | "retryable-cancel")
                ) && scroll_retryable_finish_attempts == 0
                {
                    scroll_retryable_finish_attempts += 1;
                    scrolling = true;
                    protocol::respond_error(
                        &request.id,
                        "STITCH_RETRYABLE",
                        "could not start the scroll stitch worker",
                    );
                    let action = wait_for_scroll_retry_barrier(&fixture);
                    match action {
                        1 => protocol::send_event(
                            poratake_daemon_common::contract::SCROLL_CAPTURE_DONE_EVENT,
                            None,
                        ),
                        2 => {
                            scrolling = false;
                            protocol::send_event(
                                poratake_daemon_common::contract::SCROLL_CAPTURE_CANCELLED_EVENT,
                                None,
                            );
                        }
                        _ => panic!("unexpected scroll retry barrier action: {action}"),
                    }
                    continue;
                }
                if fixture["scroll"] == "retryable-success" {
                    scroll_retryable_finish_attempts += 1;
                }
                if fixture["scroll"] == "cancel-finishing-held" {
                    let params = request.params.as_ref().unwrap();
                    let response_path =
                        Path::new(params["outputPath"].as_str().unwrap()).to_path_buf();
                    let request_id = request.id.clone();
                    protocol::send_event(
                        poratake_daemon_common::contract::SCROLL_CAPTURE_CANCELLED_EVENT,
                        None,
                    );
                    std::thread::spawn(move || {
                        wait_for_fixture_barrier(&fixture);
                        protocol::respond_success(
                            &request_id,
                            json!({
                                "success": false,
                                "outputPath": response_path,
                                "width": 640,
                                "height": 900,
                                "frameCount": 3
                            }),
                        );
                    });
                    continue;
                }
                if fixture["scroll"] != "start-held" {
                    wait_for_fixture_barrier(&fixture);
                }
                if fixture["scroll"] == "finish-partial-failed" {
                    let path = request.params.as_ref().unwrap()["outputPath"]
                        .as_str()
                        .unwrap();
                    std::fs::write(path, b"partial scroll image").unwrap();
                    protocol::respond_error(
                        &request.id,
                        "STITCH_ERROR",
                        "Fixture scroll finish failure",
                    );
                    continue;
                }
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
                if success && fixture["scroll"] != "finish-empty-success" {
                    image::RgbaImage::from_pixel(640, 900, image::Rgba([32, 168, 96, 255]))
                        .save(path)
                        .unwrap();
                }
                json!({"success": success, "outputPath": path, "width": 640, "height": 900, "frameCount": 3})
            }
            ("scroll-capture", "cancel") => {
                scrolling = false;
                let fixture = capture_fixture(&output);
                if fixture["scroll"] == "cancel-finishing-held" {
                    wait_for_scroll_cancel_barrier(&fixture);
                } else {
                    wait_for_fixture_barrier(&fixture);
                }
                json!({})
            }
            ("screenshot", method @ ("capture-area" | "capture-window")) => {
                let params = request.params.as_ref().unwrap();
                let fixture = capture_fixture(&output);
                if fixture["expectDesktopHidden"] == true {
                    assert!(
                        desktop_icons_hidden.load(std::sync::atomic::Ordering::SeqCst),
                        "the desktop must be hidden before capture"
                    );
                }
                assert_eq!(
                    params.get("windowId").and_then(serde_json::Value::as_i64),
                    fixture["windowId"].as_i64()
                );
                if method == "capture-area" {
                    for key in ["x", "y", "width", "height"] {
                        assert_eq!(params[key], fixture["rect"][key], "capture {key}");
                    }
                    if let Some(expected) = fixture
                        .get("expectedExcludedWindowIds")
                        .filter(|value| !value.is_null())
                    {
                        assert_eq!(&params["excludedWindowIds"], expected, "capture exclusions");
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
                        if fixture["freeze"] == "freeze-failed" {
                            assert!(!frozen, "a failed freeze must be released before capture");
                        }
                        image::Rgba([240, 80, 64, 255])
                    };
                let path = Path::new(params["path"].as_str().unwrap());
                let width = fixture["rect"]["width"].as_u64().unwrap() as u32;
                let height = fixture["rect"]["height"].as_u64().unwrap() as u32;
                if fixture["timer"] == "empty-screenshot" {
                    protocol::respond_success(
                        &request.id,
                        json!({"path": path, "width": width, "height": height}),
                    );
                    continue;
                }
                if fixture["outcome"] == "invalid-image" {
                    std::fs::write(path, b"invalid image").unwrap();
                } else {
                    let mut image = if fixture["pattern"] == "quadrants" {
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
                    if fixture["frozenDesktopIcon"] == true
                        && params.get("cached").and_then(serde_json::Value::as_bool) == Some(true)
                        && !frozen_desktop_icons_hidden
                    {
                        for y in 0..16 {
                            for x in 0..16 {
                                image.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
                            }
                        }
                    }
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
            ("recording-control", "setTooltip") => json!({"visible": false}),
            ("recording-overlay", "show" | "showWindow") => {
                if let Some(fixture) = optional_capture_fixture(&output)
                    .filter(|fixture| fixture["recordingHoldOutlineShow"] == true)
                {
                    std::thread::spawn(move || {
                        wait_for_fixture_barrier(&fixture);
                        protocol::respond_success(&request.id, json!({"visible": true}));
                    });
                    continue;
                }
                json!({"visible": true})
            }
            ("screen-recorder", "start") => {
                recording_stop_was_held = false;
                if optional_capture_fixture(&output)
                    .is_some_and(|fixture| fixture["recordingAreaStartFails"] == true)
                    && !recording_area_start_failed
                    && request
                        .params
                        .as_ref()
                        .is_some_and(|params| !params.contains_key("windowId"))
                {
                    recording_area_start_failed = true;
                    if optional_capture_fixture(&output)
                        .is_some_and(|fixture| fixture["recordingAreaStartWritesPartial"] == true)
                    {
                        let output_path = request.params.as_ref().unwrap()["outputPath"]
                            .as_str()
                            .unwrap();
                        std::fs::write(output_path, include_bytes!("fixtures/capture.mov"))
                            .unwrap();
                    }
                    protocol::respond_error(
                        &request.id,
                        "START_FAILED",
                        "Fixture area start failure",
                    );
                    continue;
                }
                if optional_capture_fixture(&output)
                    .is_some_and(|fixture| fixture["recordingStartFailsAfterQueuedError"] == true)
                    && !recording_start_failed_after_error
                {
                    recording_start_failed_after_error = true;
                    protocol::send_event(
                        SCREEN_RECORDER_ERROR_EVENT,
                        Some(json!({
                            "code": "ENCODER_ERROR",
                            "message": "Fixture encoder failure",
                        })),
                    );
                    protocol::respond_error(
                        &request.id,
                        "START_FAILED",
                        "Fixture recording start failure",
                    );
                    continue;
                }
                if let Some(fixture) = optional_capture_fixture(&output) {
                    let params = request.params.as_ref().unwrap();
                    let source_key = match (
                        params.contains_key("windowId"),
                        fixture["recordingAreaFixtureVideo"] == true,
                        fixture["recordingWindowFixtureVideo"] == true,
                        fixture["recordingScreenFixtureVideo"] == true,
                    ) {
                        (false, true, _, _) => Some("recordingAreaSource"),
                        (true, _, true, _) => Some("recordingWindowSource"),
                        (false, _, _, true) => Some("recordingScreenSource"),
                        _ => None,
                    };
                    if let Some(source_key) = source_key {
                        let path = params["outputPath"].as_str().unwrap();
                        if let Some(source) = fixture[source_key].as_str() {
                            std::fs::copy(source, path).unwrap();
                        } else {
                            std::fs::write(path, include_bytes!("fixtures/capture.mov")).unwrap();
                        }
                    }
                    if fixture["recordingHoldStartUntilStop"] == true {
                        held_recording_start = Some(request.id.clone());
                        continue;
                    }
                }
                if optional_capture_fixture(&output)
                    .is_some_and(|fixture| fixture["recordingTargetClosedDuringStart"] == true)
                    && recording_start_failed_after_error
                    && !recording_target_closed_during_start
                {
                    recording_target_closed_during_start = true;
                    protocol::send_event(
                        SCREEN_RECORDER_ERROR_EVENT,
                        Some(json!({
                            "code": "TARGET_CLOSED",
                            "message": "The recorded window was closed",
                        })),
                    );
                }
                json!({})
            }
            ("screen-recorder", "stop") => {
                if let Some(start_id) = held_recording_start.take() {
                    if optional_capture_fixture(&output)
                        .is_some_and(|fixture| fixture["recordingHeldStartStopFirst"] == true)
                    {
                        held_recording_start = Some(start_id);
                        protocol::respond_error(
                            &request.id,
                            "INVALID_STATE",
                            "No recording is active",
                        );
                        continue;
                    }
                    if optional_capture_fixture(&output)
                        .is_some_and(|fixture| fixture["recordingHeldStartFails"] == true)
                    {
                        protocol::respond_error(
                            &start_id,
                            "START_FAILED",
                            "Fixture late start failure",
                        );
                        protocol::respond_error(
                            &request.id,
                            "INVALID_STATE",
                            "No recording is active",
                        );
                        continue;
                    }
                    protocol::respond_success(&start_id, json!({"success": true}));
                }
                if !recording_stop_was_held
                    && optional_capture_fixture(&output)
                        .is_some_and(|fixture| fixture["recordingHoldStop"] == true)
                {
                    recording_stop_was_held = true;
                    held_recording_stop = Some(request.id.clone());
                    if optional_capture_fixture(&output)
                        .is_some_and(|fixture| fixture["recordingHeldStopTerminalError"] == true)
                    {
                        protocol::send_event(
                            SCREEN_RECORDER_ERROR_EVENT,
                            Some(
                                json!({"code": "CAPTURE_ERROR", "message": "Fixture held stop terminal failure"}),
                            ),
                        );
                    }
                    continue;
                }
                if optional_capture_fixture(&output)
                    .is_some_and(|fixture| fixture["recordingErrorBeforeStopSuccess"] == true)
                {
                    protocol::send_event(
                        SCREEN_RECORDER_ERROR_EVENT,
                        Some(
                            json!({"code": "CAPTURE_ERROR", "message": "Fixture terminal failure"}),
                        ),
                    );
                }
                if optional_capture_fixture(&output)
                    .is_some_and(|fixture| fixture["recordingStopAudioFailure"] == true)
                {
                    protocol::respond_error(&request.id, "CAPTURE_ERROR", "Fixture audio failure");
                    continue;
                }
                if optional_capture_fixture(&output).is_some_and(|fixture| {
                    fixture["recordingStopFailsAfterQueuedTargetClosed"] == true
                }) && !recording_stop_failed_after_error
                {
                    recording_stop_failed_after_error = true;
                    protocol::send_event(
                        SCREEN_RECORDER_ERROR_EVENT,
                        Some(json!({
                            "code": "TARGET_CLOSED",
                            "message": "The recorded window was closed",
                        })),
                    );
                    protocol::respond_error(&request.id, "STOP_FAILED", "Fixture stop failure");
                    continue;
                }
                let fixture = optional_capture_fixture(&output);
                recording_outline_hide_barrier = fixture
                    .as_ref()
                    .filter(|fixture| fixture["recordingHoldOutlineHide"] == true)
                    .cloned();
                match fixture {
                    Some(fixture) if fixture["recordingAreaFixtureVideo"] == true => {
                        json!({"duration": fixture["recordingAreaDuration"]})
                    }
                    Some(fixture) if fixture["recordingWindowFixtureVideo"] == true => {
                        json!({"duration": fixture["recordingWindowDuration"]})
                    }
                    Some(fixture) if fixture["recordingScreenFixtureVideo"] == true => {
                        json!({"duration": fixture["recordingScreenDuration"]})
                    }
                    _ => json!({}),
                }
            }
            ("screen-recorder", "setCamera")
                if optional_capture_fixture(&output)
                    .is_some_and(|fixture| fixture["recordingCameraDisableFails"] == true)
                    && request.params.as_ref().is_some_and(|params| {
                        params.get("enabled").and_then(serde_json::Value::as_bool) == Some(false)
                    }) =>
            {
                protocol::respond_error(&request.id, "CAMERA_FAILED", "Fixture camera failure");
                continue;
            }
            ("camera-preview", "setContentProtection")
                if held_recording_stop.is_some()
                    && optional_capture_fixture(&output).is_some_and(|fixture| {
                        fixture["recordingReleaseHeldStopOnProtection"] == true
                    }) =>
            {
                let stop_id = held_recording_stop.take().unwrap();
                if optional_capture_fixture(&output)
                    .is_some_and(|fixture| fixture["recordingHeldStopFails"] == true)
                {
                    protocol::respond_error(&stop_id, "STOP_FAILED", "Fixture late stop failure");
                } else {
                    protocol::respond_success(&stop_id, json!({"duration": 2.0}));
                }
                json!({})
            }
            ("camera-preview", "setContentProtection")
                if held_recording_start.is_some()
                    && optional_capture_fixture(&output).is_some_and(|fixture| {
                        fixture["recordingReleaseHeldStartOnProtection"] == true
                    }) =>
            {
                let start_id = held_recording_start.take().unwrap();
                if optional_capture_fixture(&output)
                    .is_some_and(|fixture| fixture["recordingHeldStartFails"] == true)
                {
                    protocol::respond_error(
                        &start_id,
                        "START_FAILED",
                        "Fixture late start failure",
                    );
                } else {
                    protocol::respond_success(&start_id, json!({"success": true}));
                }
                json!({})
            }
            ("camera-preview", "setContentProtection")
                if optional_capture_fixture(&output)
                    .is_some_and(|fixture| fixture["recordingProtectionFails"] == true)
                    && request.params.as_ref().is_some_and(|params| {
                        params.get("enabled").and_then(serde_json::Value::as_bool) == Some(true)
                    }) =>
            {
                protocol::respond_error(
                    &request.id,
                    "CONTENT_PROTECTION_ERROR",
                    "Fixture protection failure",
                );
                continue;
            }
            ("screen-recorder", "setSystemAudio")
                if optional_capture_fixture(&output)
                    .is_some_and(|fixture| fixture["recordingAudioFailure"] == true) =>
            {
                protocol::send_event(
                    SCREEN_RECORDER_ERROR_EVENT,
                    Some(json!({"code": "CAPTURE_ERROR", "message": "Fixture audio failure"})),
                );
                json!({})
            }
            ("screen-recorder", method @ ("setSystemAudio" | "setMicrophone" | "setCamera")) => {
                if let Some(fixture) = optional_capture_fixture(&output)
                    .filter(|fixture| fixture["recordingSettingBarrier"] == method)
                {
                    std::thread::spawn(move || {
                        wait_for_fixture_barrier(&fixture);
                        protocol::respond_success(&request.id, json!({}));
                    });
                    continue;
                }
                json!({})
            }
            ("recording-overlay", "hide") => {
                if let Some(fixture) = recording_outline_hide_barrier.take() {
                    std::thread::spawn(move || {
                        wait_for_fixture_barrier(&fixture);
                        protocol::respond_success(&request.id, json!({}));
                    });
                    continue;
                }
                json!({})
            }
            ("camera-preview", "show") => json!({"visible": true}),
            ("camera-preview", "hide" | "update" | "setContentProtection")
            | (
                "screen-recorder",
                "pause" | "resume",
            ) => json!({}),
            (module, method) => {
                panic!("unexpected visual fixture call: {module} {method}");
            }
        };
        protocol::respond_success(&request.id, result);
    }
}
