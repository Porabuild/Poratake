#![cfg(target_os = "linux")]

use std::io::Write as _;
use std::process::{Command, Stdio};

#[test]
fn headless_daemon_serves_the_shared_system_protocol() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_poratake-daemon-linux"))
        .args(["--session", "headless"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn Linux daemon");
    let mut stdin = child.stdin.take().expect("daemon stdin");
    writeln!(
        stdin,
        r#"{{"id":"ping","module":"system","method":"ping"}}"#
    )
    .expect("write ping");
    writeln!(
        stdin,
        r#"{{"id":"modules","module":"system","method":"list-modules"}}"#
    )
    .expect("write module request");
    writeln!(
        stdin,
        r#"{{"id":"freeze","module":"freeze-screen","method":"freeze"}}"#
    )
    .expect("write freeze request");
    writeln!(
        stdin,
        "{}",
        serde_json::json!({
            "id": "timer",
            "module": "timer-control",
            "method": "show",
            "params": {
                "x": 10,
                "y": 20,
                "duration": 1,
                "color": "#8892ef",
                "foregroundColor": "#ffffff"
            }
        })
    )
    .expect("write timer request");
    writeln!(
        stdin,
        "{}",
        serde_json::json!({
            "id": "print",
            "module": "print",
            "method": "image",
            "params": {
                "imageBase64": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
            }
        })
    )
    .expect("write print request");
    writeln!(
        stdin,
        r#"{{"id":"recorder-status","module":"screen-recorder","method":"status"}}"#
    )
    .expect("write recorder status request");
    writeln!(
        stdin,
        "{}",
        serde_json::json!({
            "id": "recorder-start",
            "module": "screen-recorder",
            "method": "start",
            "params": {
                "frameRate": 30,
                "outputPath": "/tmp/poratake-recorder-test/recording.mov"
            }
        })
    )
    .expect("write recorder start request");
    writeln!(
        stdin,
        "{}",
        serde_json::json!({
            "id": "recorder-window",
            "module": "screen-recorder",
            "method": "start",
            "params": {
                "windowId": 42,
                "frameRate": 30,
                "outputPath": "/tmp/poratake-recorder-test/recording.mov"
            }
        })
    )
    .expect("write recorder window request");
    writeln!(
        stdin,
        r#"{{"id":"quit","module":"system","method":"quit"}}"#
    )
    .expect("write quit");
    drop(stdin);

    let output = child.wait_with_output().expect("wait for Linux daemon");
    assert!(output.status.success(), "daemon failed: {output:?}");
    let messages: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .expect("utf8 output")
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSON response"))
        .collect();
    let response = |id: &str| {
        messages
            .iter()
            .find(|message| message["id"] == id)
            .expect("daemon response")
    };

    assert_eq!(messages[0]["data"]["backend"], "headless");
    assert_eq!(response("ping")["result"]["pong"], true);
    assert_eq!(
        response("modules")["result"]["modules"],
        serde_json::json!([
            "desktop-wallpaper",
            "freeze-screen",
            "print",
            "qrcode",
            "screen-recorder",
            "screenshot",
            "scroll-capture",
            "timer-control",
            "window-selector"
        ])
    );
    assert_eq!(response("freeze")["error"]["code"], "CAPTURE_FAILED");
    assert_eq!(response("timer")["error"]["code"], "UI_ERROR");
    assert_eq!(response("print")["error"]["code"], "UI_ERROR");
    assert_eq!(response("recorder-status")["result"]["state"], "idle");
    assert_eq!(
        response("recorder-start")["error"]["code"],
        "CONFIGURATION_ERROR"
    );
    assert_eq!(
        response("recorder-window")["error"]["code"],
        "CONFIGURATION_ERROR"
    );
    assert_eq!(response("quit")["result"]["status"], "exiting");
}
