#![cfg(target_os = "linux")]

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Command, Stdio};

fn send(stdin: &mut std::process::ChildStdin, value: serde_json::Value) {
    writeln!(stdin, "{value}").expect("write daemon request");
    stdin.flush().expect("flush daemon request");
}

fn read(reader: &mut BufReader<std::process::ChildStdout>) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read daemon response");
    serde_json::from_str(&line).expect("parse daemon response")
}

#[test]
fn headless_daemon_detects_a_qr_code() {
    let pixels = include_bytes!("../../daemon-win/tests/fixtures/qr-hello.gray").to_vec();
    let image = image::GrayImage::from_raw(264, 264, pixels).expect("QR fixture dimensions");
    let image_path = std::env::temp_dir().join(format!(
        "poratake-linux-qrcode-test-{}.png",
        std::process::id()
    ));
    image.save(&image_path).expect("write QR fixture");

    let mut child = Command::new(env!("CARGO_BIN_EXE_poratake-daemon-linux"))
        .args(["--session", "headless"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn Linux daemon");
    let mut stdin = child.stdin.take().expect("daemon stdin");
    let mut reader = BufReader::new(child.stdout.take().expect("daemon stdout"));
    assert_eq!(read(&mut reader)["data"]["backend"], "headless");

    send(
        &mut stdin,
        serde_json::json!({
            "id": "detect",
            "module": "qrcode",
            "method": "detect",
            "params": { "imagePath": image_path }
        }),
    );
    let response = read(&mut reader);
    assert_eq!(response["success"], true);
    assert_eq!(response["result"]["payload"], "https://example.com/qr-test");

    send(
        &mut stdin,
        serde_json::json!({"id": "quit", "module": "system", "method": "quit"}),
    );
    assert_eq!(read(&mut reader)["success"], true);
    drop(stdin);
    assert!(child.wait().expect("wait for daemon").success());
    std::fs::remove_file(image_path).expect("remove QR fixture");
}
