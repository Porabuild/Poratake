mod capture;
mod cloud;
mod config;
mod daemon;
mod daemon_contract;
mod editor;
mod history_store;
mod intents;
mod product;
mod render;
mod state;
mod system;
mod theme;
mod thumbnails;
mod ui;
mod update;
mod video;
mod windows;

use gpui::{prelude::*, px, size, App, Application, Bounds};

use crate::system::native::{self, NativeCommand, NativeEvent};
use crate::system::tray::{Intent, TrayMenuState};
use crate::theme::presets::{resolve_theme_mode, ThemeMode};
use crate::theme::watcher;
use crate::ui::chrome;

pub fn open_editor_for(cx: &mut App, path: &str) -> bool {
    let (image_width, image_height) = match image::open(std::path::Path::new(path)) {
        Ok(image) => (image.width() as f32, image.height() as f32),
        Err(error) => {
            windows::toast::Toast::show(
                cx,
                "Unable to Open Screenshot",
                format!("The image could not be loaded: {error}"),
            );
            return false;
        }
    };
    let (work_width, work_height) = cx
        .displays()
        .first()
        .map(|display| {
            let bounds = display.bounds();
            (f32::from(bounds.size.width), f32::from(bounds.size.height))
        })
        .unwrap_or((1920.0, 1080.0));
    let stored = crate::config::window_state::get(crate::config::window_state::SCREENSHOT_EDITOR);
    let (width, height) =
        chrome::editor_window_size(image_width, image_height, work_width, work_height, stored);
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    let opened = cx.open_window(
        windows::app_window_options(
            bounds,
            Some(size(
                px(chrome::EDITOR_MIN_WIDTH),
                px(chrome::EDITOR_MIN_HEIGHT),
            )),
        ),
        |window, cx| cx.new(|cx| editor::EditorWindow::from_file(path, window, cx)),
    );
    if let Err(error) = opened {
        windows::toast::Toast::show(
            cx,
            "Unable to Open Screenshot",
            format!("The editor window could not be opened: {error}"),
        );
        return false;
    }
    true
}

fn main() {
    let single_instance = system::single_instance::acquire();
    if single_instance.is_none() {
        eprintln!("[single-instance] another Poratake GPUI instance is running — exiting");
        std::process::exit(0);
    }

    Application::new().run(|cx: &mut App| {
        let config = state::init(cx);
        let settings = config.get();
        let mode = resolve_theme_mode(ThemeMode::parse(&settings.appearance.mode));
        theme::vars::init_theme(cx, mode, &settings.appearance.theme);
        // Electron's `nativeTheme.on('updated', ...)` follows the OS light/dark
        // switch live; the watcher reports the same switches so open windows
        // repaint when the user's appearance mode is `system`.
        let system_theme = watcher::spawn();
        cx.spawn(async move |cx| {
            while let Ok(mode) = system_theme.recv().await {
                let result = cx.update(|cx| watcher::apply_system_mode(mode, cx));
                if let Err(error) = result {
                    eprintln!("[theme] system-mode update failed: {error}");
                    break;
                }
            }
        })
        .detach();
        editor::actions::init_bindings(cx);
        capture::overlay::init_bindings(cx);
        // `capture/index.ts` prewarms the freeze pipeline at startup so the
        // first capture does not pay for initialising it.
        capture::prewarm_freeze_screen(cx);
        // `applyLoginItemSetting()` in the Electron shell reconciles the Run
        // entry with the stored preference on launch; this is the GPUI
        // shell's half of that, so both shells agree on one entry. Comparing
        // first keeps every launch from rewriting a registry value that is
        // already correct.
        if system::startup::is_open_at_login() != settings.general.start_on_login {
            system::startup::set_open_at_login(settings.general.start_on_login);
        }

        let bridge = native::spawn(
            TrayMenuState::from_config(&settings),
            system::hotkeys::bindings(&settings),
        );
        if settings.general.hide_menu_bar_icon {
            bridge.send(NativeCommand::SetTrayVisible(false));
        }
        let events = bridge.events();
        state::set_native(cx, bridge);

        cx.spawn(async move |cx| {
            while let Ok(event) = events.recv().await {
                let result = cx.update(|cx| match event {
                    NativeEvent::Intent { intent, tray_rect } => {
                        intents::dispatch(intent, tray_rect, cx)
                    }
                    NativeEvent::ToggleTrayMenu { tray_rect } => {
                        windows::tray_menu::TrayMenuWindow::toggle(tray_rect, cx)
                    }
                });
                if let Err(error) = result {
                    eprintln!("[intent] dispatch failed: {error}");
                    break;
                }
            }
        })
        .detach();

        windows::keepalive::KeepAlive::open(cx);
        #[cfg(windows)]
        capture::overlay::prewarm(cx);

        if windows::onboarding::OnboardingWindow::should_show(&config) {
            windows::onboarding::OnboardingWindow::open(cx, config.clone());
        }

        // A screenshot path passed on the CLI opens the editor directly,
        // mirroring Electron's open-file flow. A tray intent id instead --
        // `--intent open-settings` -- runs that menu item, which is the only
        // way to reach the settings and history windows without clicking the
        // tray, and so the only way to screenshot them for a parity check.
        let mut args = std::env::args().skip(1);
        match args.next().as_deref() {
            Some("--intent") => {
                let id = args.next();
                // An optional second argument: a settings category, or a project
                // path for the video editor.
                let extra = args.next();
                match id.as_deref().and_then(Intent::from_id) {
                    // `--intent open-settings shortcuts` is the CLI form of
                    // `settings-window.tsx` reading its tab from the URL hash.
                    Some(Intent::OpenSettings) => {
                        let category = extra
                            .as_deref()
                            .and_then(windows::settings::registry::Category::from_id)
                            .unwrap_or(windows::settings::registry::Category::General);
                        intents::open_settings(category, cx);
                        cx.activate(true);
                    }
                    // The tray item opens a file picker, which a script cannot
                    // answer, so the CLI takes the path directly -- or nothing,
                    // for the editor's own empty state.
                    Some(Intent::OpenInVideoEditor) => {
                        windows::video_editor::VideoEditorWindow::open(cx, extra);
                        cx.activate(true);
                    }
                    Some(intent) => {
                        intents::dispatch(intent, None, cx);
                        cx.activate(true);
                    }
                    None => eprintln!("[cli] --intent needs a tray intent id"),
                }
            }
            // Opens one of the transient windows for inspection. Several of
            // them only ever appear mid-capture or mid-recording, so there is
            // no other way to look at them without driving a real capture.
            Some("--preview-window") => match args.next().as_deref() {
                Some("capture-preview") => {
                    let Some(path) = args.next() else {
                        eprintln!("[cli] --preview-window capture-preview needs an image path");
                        return;
                    };
                    windows::capture_preview::CapturePreviewWindow::open(cx, path.into());
                }
                Some("pin") => {
                    let Some(path) = args.next() else {
                        eprintln!("[cli] --preview-window pin needs an image path");
                        return;
                    };
                    match std::fs::read(&path) {
                        Ok(bytes) => windows::pin::PinWindow::open(cx, bytes),
                        Err(error) => eprintln!("[cli] cannot read {path}: {error}"),
                    }
                }
                Some("toast") => {
                    windows::toast::Toast::show(cx, "Capture failed", "No display available")
                }
                Some("tray-menu") => windows::tray_menu::TrayMenuWindow::toggle(None, cx),
                Some("recording-control") => {
                    windows::recording_control::RecordingControl::open(
                        cx,
                        video::recorder::RecordingTarget::Screen,
                        capture::overlay::ScreenRect {
                            x: 0,
                            y: 0,
                            width: 1920,
                            height: 1080,
                        },
                        None,
                        None,
                    );
                }
                other => eprintln!("[cli] unknown --preview-window target: {other:?}"),
            },
            // Renders the area overlay in a small, unfocused window: no frozen
            // screen, no full-screen cover, no input grab. The overlay is
            // otherwise unreachable for inspection, because it exists only
            // during a live capture that takes over the display.
            Some("--preview-overlay") => {
                let service = state::state(cx);
                let Some(display) = cx.primary_display() else {
                    eprintln!("[cli] no display to preview the overlay on");
                    return;
                };
                let bounds = Bounds {
                    origin: gpui::point(px(120.0), px(120.0)),
                    size: size(px(900.0), px(600.0)),
                };
                if args.next().as_deref() == Some("all-in-one") {
                    capture::overlay::AreaOverlay::open_all_in_one(
                        service,
                        display.id(),
                        bounds,
                        capture::all_in_one::Choices::default(),
                        capture::overlay::OverlayLaunch {
                            focus: false,
                            deferred_show: false,
                            generation: 0,
                        },
                        cx,
                    );
                } else {
                    capture::overlay::AreaOverlay::open(
                        service,
                        display.id(),
                        bounds,
                        capture::intent::CaptureIntent::Screenshot,
                        capture::overlay::OverlayLaunch {
                            focus: false,
                            deferred_show: false,
                            generation: 0,
                        },
                        cx,
                    );
                }
            }
            Some(path) => {
                open_editor_for(cx, path);
                cx.activate(true);
            }
            None => {}
        }
    });
}
