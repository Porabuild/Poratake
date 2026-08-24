use gpui::{prelude::*, px, size, App, Bounds};

use crate::capture::intent::CaptureIntent;
use crate::product;
use crate::system::capabilities::{is_supported, Feature};
use crate::system::desktop;
use crate::system::native::TrayRect;
use crate::system::tray::Intent;
use crate::windows::history::HistoryWindow;
use crate::windows::registry::{self, WindowKind};
use crate::windows::settings::registry::Category;
use crate::windows::settings::SettingsWindow;
use crate::windows::video_editor::VideoEditorWindow;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];
const VIDEO_EXTENSIONS: &[&str] = &["poratake", "mp4", "mov", "webm"];

pub fn dispatch(intent: Intent, tray_rect: Option<TrayRect>, cx: &mut App) {
    match intent {
        Intent::AllInOne => {
            if is_supported(Feature::AllInOne) {
                crate::capture::start_all_in_one(cx);
            }
        }
        Intent::CaptureArea => crate::capture::start_area_capture(cx),
        Intent::CaptureScreen => crate::capture::start_screen_capture(cx),
        Intent::CaptureWindow => {
            if is_supported(Feature::ScreenshotWindow) {
                crate::capture::start_window_capture(cx);
            }
        }
        Intent::CaptureText => start_selection(CaptureIntent::Ocr, Feature::Ocr, cx),
        Intent::ScanQrCode => start_selection(CaptureIntent::QrCode, Feature::QrCode, cx),
        Intent::TimerCapture => start_selection(CaptureIntent::Timer, Feature::TimerCapture, cx),
        Intent::ScrollCapture => {
            if crate::capture::scroll::is_active() {
                return;
            }
            start_selection(CaptureIntent::ScrollCapture, Feature::ScrollCapture, cx)
        }
        Intent::History => HistoryWindow::toggle(tray_rect, cx),
        Intent::OpenSettings => open_settings(Category::General, cx),
        Intent::OpenAbout => open_settings(Category::About, cx),
        Intent::OpenInEditor => open_picked_image(cx),
        Intent::OpenClipboardInEditor => crate::editor::open_clipboard(cx),
        Intent::Pin => pin_picked_image(cx),
        Intent::OpenInVideoEditor => open_picked_video(cx),
        Intent::OpenIssues => desktop::open_url(product::ISSUES_URL),
        Intent::HideTrayIcon => hide_tray_icon(cx),
        Intent::RecordScreen => start_recording(Recording::Screen, cx),
        Intent::RecordArea => start_recording(Recording::Area, cx),
        Intent::RecordWindow => start_recording(Recording::Window, cx),
        Intent::ToggleDesktopIcons => toggle_desktop_icons(cx),
        Intent::Quit => quit(cx),
    }
}

use crate::video::recorder::RecordingTarget as Recording;

fn start_recording(target: Recording, cx: &mut App) {
    if !is_supported(Feature::Recording) || crate::video::recorder::is_recording() {
        return;
    }
    match target {
        Recording::Screen => crate::capture::start_screen_recording(cx),
        Recording::Area => crate::capture::start_area_selection(CaptureIntent::Recording, cx),
        Recording::Window => crate::capture::start_window_recording(cx),
    }
}

fn start_selection(intent: CaptureIntent, feature: Feature, cx: &mut App) {
    if !is_supported(feature) {
        crate::windows::toast::Toast::show(
            cx,
            "Not available",
            "This capture mode is not supported on this platform",
        );
        return;
    }
    crate::capture::start_area_selection(intent, cx);
}

fn toggle_desktop_icons(cx: &mut App) {
    use crate::capture::desktop_icons::{self, HideSource};

    let daemon = crate::state::state(cx).daemon;
    if desktop_icons::are_hidden() {
        desktop_icons::show(&daemon, HideSource::Menu);
    } else {
        desktop_icons::hide(&daemon, HideSource::Menu);
    }
    refresh_shell(cx);
}

fn quit(cx: &mut App) {
    use crate::capture::desktop_icons::{self, HideSource};

    let service = crate::state::state(cx);
    desktop_icons::show(&service.daemon, HideSource::System);
    service.config.flush();
    service.daemon.stop();
    cx.quit();
}

/// Rebuilds the tray menu and re-registers the global shortcuts from the
/// current config, the way Electron's `rebuildTrayMenu` does after a change.
pub fn refresh_shell(cx: &mut App) {
    let config = crate::state::state(cx).config.get();
    let bridge = crate::state::native(cx);
    bridge.send(crate::system::native::NativeCommand::RebuildMenu(
        crate::system::tray::TrayMenuState::from_config(&config),
    ));
    bridge.send(crate::system::native::NativeCommand::SetHotkeys(
        crate::system::hotkeys::bindings(&config),
    ));
}

pub fn open_settings(category: Category, cx: &mut App) {
    registry::open_or_activate(WindowKind::Settings, cx, |cx| {
        let store = crate::state::state(cx).config;
        let bounds = Bounds::centered(
            None,
            size(
                px(crate::ui::chrome::SETTINGS_WINDOW_WIDTH),
                px(crate::ui::chrome::SETTINGS_WINDOW_HEIGHT),
            ),
            cx,
        );
        cx.open_window(
            crate::windows::app_window_options(
                bounds,
                Some(size(
                    px(crate::ui::chrome::SETTINGS_WINDOW_WIDTH),
                    px(crate::ui::chrome::SETTINGS_WINDOW_HEIGHT),
                )),
            ),
            |_, cx| cx.new(|cx| SettingsWindow::new(store, category, cx)),
        )
        .ok()
        .map(|handle| handle.into())
    });
}

fn pick_file(title: &str, extensions: &[&str]) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter("Supported files", extensions)
        .pick_file()
}

fn open_picked_image(cx: &mut App) {
    let Some(path) = pick_file("Open in Editor", IMAGE_EXTENSIONS) else {
        return;
    };
    crate::open_editor_for(cx, path.to_string_lossy().as_ref());
}

fn pin_picked_image(cx: &mut App) {
    let Some(path) = pick_file("Pin Image", IMAGE_EXTENSIONS) else {
        return;
    };
    match std::fs::read(&path) {
        Ok(bytes) => crate::windows::pin::PinWindow::open(cx, bytes),
        Err(error) => eprintln!("[intent] failed to read {}: {error}", path.display()),
    }
}

fn open_picked_video(cx: &mut App) {
    let Some(path) = pick_file("Open in Video Editor", VIDEO_EXTENSIONS) else {
        return;
    };
    VideoEditorWindow::open(cx, Some(path.to_string_lossy().to_string()));
}

fn hide_tray_icon(cx: &mut App) {
    let title = if cfg!(target_os = "macos") {
        "Hide Menu Bar Icon"
    } else {
        "Hide Tray Icon"
    };
    let detail = if cfg!(target_os = "macos") {
        "The app will continue running in the background. To restore the menu bar icon, launch the app again."
    } else {
        "The app will continue running in the background. To restore the tray icon, launch Poratake again from the Start menu."
    };
    let confirmed = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(title)
        .set_description(detail)
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            "Hide Icon".into(),
            "Cancel".into(),
        ))
        .show();
    if confirmed != rfd::MessageDialogResult::Custom("Hide Icon".into()) {
        return;
    }

    let config = crate::state::state(cx).config;
    config.update(|settings| settings.general.hide_menu_bar_icon = true);
    crate::state::native(cx).send(crate::system::native::NativeCommand::SetTrayVisible(false));
}
