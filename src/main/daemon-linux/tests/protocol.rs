#![cfg(target_os = "linux")]

use std::io::{BufRead as _, BufReader, Write as _};
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
    let mut stdout = BufReader::new(child.stdout.take().expect("daemon stdout"));
    let mut read_message = || {
        let mut line = String::new();
        let bytes = stdout.read_line(&mut line).expect("read daemon output");
        assert!(bytes > 0, "daemon exited before responding");
        serde_json::from_str::<serde_json::Value>(&line).expect("JSON response")
    };

    let mut messages = vec![read_message()];
    let requests = [
        r#"{"id":"ping","module":"system","method":"ping"}"#.to_string(),
        r#"{"id":"modules","module":"system","method":"list-modules"}"#.to_string(),
        r#"{"id":"freeze","module":"freeze-screen","method":"freeze"}"#.to_string(),
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
        .to_string(),
        serde_json::json!({
            "id": "print",
            "module": "print",
            "method": "image",
            "params": {
                "imageBase64": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
            }
        })
        .to_string(),
        r#"{"id":"recorder-status","module":"screen-recorder","method":"status"}"#.to_string(),
        serde_json::json!({
            "id": "recorder-start",
            "module": "screen-recorder",
            "method": "start",
            "params": {
                "frameRate": 30,
                "outputPath": "/tmp/poratake-recorder-test/recording.mov"
            }
        })
        .to_string(),
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
        .to_string(),
    ];
    for request in &requests {
        writeln!(stdin, "{request}").expect("write request");
    }
    stdin.flush().expect("flush requests");
    for _ in 0..requests.len() {
        messages.push(read_message());
    }
    writeln!(
        stdin,
        r#"{{"id":"quit","module":"system","method":"quit"}}"#
    )
    .expect("write quit");
    stdin.flush().expect("flush quit");
    messages.push(read_message());
    drop(stdin);

    let status = child.wait().expect("wait for Linux daemon");
    assert!(status.success(), "daemon failed: {status:?}");
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
