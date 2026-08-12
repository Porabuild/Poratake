mod com;
mod desktop_frame;
mod display_color;
mod modules;
mod overlay;
mod panel;
mod protocol;
mod router;
mod tone_map;
mod ui;

use modules::all_in_one::AllInOneModule;
use modules::area_selector::AreaSelectorModule;
use modules::camera_preview::CameraPreviewModule;
use modules::desktop_helper::DesktopHelperModule;
use modules::desktop_wallpaper::DesktopWallpaperModule;
use modules::display_selector::DisplaySelectorModule;
use modules::freeze_screen::FreezeScreenModule;
use modules::media_devices::MediaDevicesModule;
use modules::ocr::OcrModule;
use modules::print::PrintModule;
use modules::qrcode::QrCodeModule;
use modules::recording_control::RecordingControlModule;
use modules::recording_overlay::RecordingOverlayModule;
use modules::screen_recorder::ScreenRecorderModule;
use modules::screenshot::ScreenshotModule;
use modules::scroll_capture::ScrollCaptureModule;
use modules::timer_control::TimerControlModule;
use modules::window_selector::WindowSelectorModule;
use protocol::{parse_request, send_event, send_response, Response};
use router::Router;
use serde_json::json;
use std::io::BufRead;

fn main() {
    ui::init();

    let mut router = Router::new();
    router.register(Box::new(AllInOneModule::new()));
    router.register(Box::new(AreaSelectorModule::new()));
    router.register(Box::new(OcrModule));
    router.register(Box::new(QrCodeModule));
    router.register(Box::new(DesktopHelperModule));
    router.register(Box::new(DesktopWallpaperModule));
    router.register(Box::new(FreezeScreenModule::new()));
    router.register(Box::new(TimerControlModule));
    router.register(Box::new(DisplaySelectorModule::new()));
    router.register(Box::new(WindowSelectorModule::new()));
    router.register(Box::new(PrintModule));
    router.register(Box::new(RecordingOverlayModule::new()));
    router.register(Box::new(RecordingControlModule::new()));
    router.register(Box::new(CameraPreviewModule::new()));
    router.register(Box::new(MediaDevicesModule::new()));
    router.register(Box::new(ScreenRecorderModule::new()));
    router.register(Box::new(ScrollCaptureModule::new()));
    router.register(Box::new(ScreenshotModule));

    send_event("system:ready", Some(json!({ "pid": std::process::id() })));

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

        router.route(request);
    }
}
