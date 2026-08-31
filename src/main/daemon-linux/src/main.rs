#[cfg(target_os = "linux")]
mod capture;
#[cfg(target_os = "linux")]
mod desktop_wallpaper;
#[cfg(target_os = "linux")]
mod freeze_screen;
#[cfg(target_os = "linux")]
mod gtk_runtime;
#[cfg(target_os = "linux")]
mod hotkeys;
#[cfg(target_os = "linux")]
mod print;
#[cfg(target_os = "linux")]
mod qrcode;
#[cfg(target_os = "linux")]
mod recording_input;
#[cfg(target_os = "linux")]
mod recording_tracks;
#[cfg(target_os = "linux")]
mod screen_recorder;
#[cfg(target_os = "linux")]
mod screenshot;
#[cfg(target_os = "linux")]
mod scroll_capture;
#[cfg(target_os = "linux")]
mod timer_control;
#[cfg(target_os = "linux")]
mod window_selector;

#[cfg(target_os = "linux")]
use std::io::BufRead;

#[cfg(target_os = "linux")]
use poratake_daemon_common::contract::LINUX_MODULES;
#[cfg(target_os = "linux")]
use poratake_daemon_common::platform::LinuxBackend as Backend;
#[cfg(target_os = "linux")]
use poratake_daemon_common::protocol::{Response, parse_request, send_event, send_response};
#[cfg(target_os = "linux")]
use poratake_daemon_common::router::{RouteControl, Router};
#[cfg(target_os = "linux")]
use serde_json::json;

#[cfg(target_os = "linux")]
fn backend_from_args() -> Option<Backend> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--session" {
            return args.next()?.parse().ok();
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn main() {
    let Some(backend) = backend_from_args() else {
        eprintln!("poratake-daemon-linux requires --session wayland|x11|headless");
        std::process::exit(2);
    };

    let mut router = Router::new();
    let frozen = capture::FrozenFrames::default();
    let gtk = gtk_runtime::GtkRuntime::new(backend);
    router.register(Box::new(desktop_wallpaper::DesktopWallpaperModule::new(
        backend,
    )));
    router.register(Box::new(freeze_screen::FreezeScreenModule::new(
        backend,
        frozen.clone(),
    )));
    router.register(Box::new(qrcode::QrCodeModule::new()));
    router.register(Box::new(print::PrintModule::new(gtk.clone())));
    router.register(Box::new(screenshot::ScreenshotModule::new(backend, frozen)));
    router.register(Box::new(screen_recorder::ScreenRecorderModule::new(
        backend,
    )));
    router.register(Box::new(scroll_capture::ScrollCaptureModule::new(
        gtk.clone(),
    )));
    router.register(Box::new(timer_control::TimerControlModule::new(gtk)));
    router.register(Box::new(window_selector::WindowSelectorModule::new(
        backend,
    )));
    if let Err(message) = router.validate_modules(LINUX_MODULES) {
        eprintln!("{message}");
        std::process::exit(2);
    }

    send_event(
        "system:ready",
        Some(json!({ "pid": std::process::id(), "backend": backend.id() })),
    );

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(request) = parse_request(trimmed) else {
            send_response(Response::error(
                "unknown",
                "PARSE_ERROR",
                "Failed to parse request",
            ));
            continue;
        };
        if router.route(request) == RouteControl::Exit {
            break;
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("poratake-daemon-linux only runs on Linux");
}
