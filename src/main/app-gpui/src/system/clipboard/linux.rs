use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::system::linux_session::{self, LinuxSession};

pub fn write_file(path: &Path) -> bool {
    let uri = format!("{}\r\n", super::file_uri_for_path(path));
    match linux_session::current() {
        LinuxSession::Wayland => {
            pipe_clipboard("wl-copy", &["--type", "text/uri-list"], uri.as_bytes())
        }
        LinuxSession::X11 => pipe_clipboard(
            "xclip",
            &["-selection", "clipboard", "-t", "text/uri-list", "-i"],
            uri.as_bytes(),
        ),
        LinuxSession::Headless => false,
    }
}

fn pipe_clipboard(program: &str, args: &[&str], payload: &[u8]) -> bool {
    let Ok(mut child) = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let Some(mut stdin) = child.stdin.take() else {
        return false;
    };
    if stdin.write_all(payload).is_err() {
        return false;
    }
    drop(stdin);
    child.wait().is_ok_and(|status| status.success())
}
