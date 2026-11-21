mod capture;
mod cloud;
mod config;
mod daemon;
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
mod util;
mod video;
mod windows;

use gpui::{prelude::*, px, size, App, Bounds};
use herogpui::application;
use herogpui::gpui;

use crate::system::native::{self, NativeCommand, NativeEvent};
use crate::system::tray::{Intent, TrayMenuState};
use crate::theme::presets::{resolve_theme_mode, ThemeMode};
#[cfg(any(windows, target_os = "linux"))]
use crate::theme::watcher;
use crate::ui::chrome;

fn dispatch_native_event(event: NativeEvent, cx: &mut App) {
    match event {
        NativeEvent::Intent { intent, tray_rect } => intents::dispatch(intent, tray_rect, cx),
        #[cfg(target_os = "macos")]
        NativeEvent::Hotkey { id } => {
            if let Some(event) = state::native(cx).resolve_hotkey(id) {
                dispatch_native_event(event, cx);
            }
        }
        NativeEvent::ToggleTrayMenu { tray_rect } => {
            windows::tray_menu::TrayMenuWindow::toggle(tray_rect, cx)
        }
        NativeEvent::CancelPreRecording => {
            #[cfg(target_os = "macos")]
            if intents::cancel_recording_capture_switch(cx) {
                return;
            }
            windows::recording_control::RecordingControl::cancel_pre_recording(cx);
        }
        #[cfg(target_os = "macos")]
        NativeEvent::FinishScrollCapture => {
            windows::scroll_capture::ScrollCaptureSession::finish_requested(cx);
        }
        #[cfg(target_os = "macos")]
        NativeEvent::CancelScrollCapture => {
            windows::scroll_capture::ScrollCaptureSession::cancel_requested(cx);
        }
        #[cfg(not(target_os = "macos"))]
        NativeEvent::FinishScrollCapture | NativeEvent::CancelScrollCapture => {}
    }
}

#[cfg(any(test, feature = "parity"))]
pub fn open_editor_for(cx: &mut App, path: &str) -> bool {
    open_editor_with_state(cx, path, None)
}

#[cfg(any(test, feature = "parity"))]
pub(crate) fn open_editor_with_state(
    cx: &mut App,
    path: &str,
    editor_state: Option<editor::window::PersistedEditorState>,
) -> bool {
    open_editor_with_image(
        cx,
        path,
        editor_state,
        editor::window::PreparedScreenshot::open(path),
    )
}

pub(crate) fn prepare_editor_open(
    cx: &App,
    path: String,
) -> gpui::Task<(
    Option<editor::window::PersistedEditorState>,
    image::ImageResult<editor::window::PreparedScreenshot>,
)> {
    cx.background_executor().spawn(async move {
        let image = editor::window::PreparedScreenshot::open(&path);
        let state = image
            .as_ref()
            .ok()
            .map(|_| editor::window::PersistedEditorState::load_for_screenshot(&path));
        (state, image)
    })
}

pub(crate) fn open_editor_async(cx: &mut App, path: String, activate: bool) {
    let prepared = prepare_editor_open(cx, path.clone());
    cx.spawn(async move |cx| {
        let (state, image) = prepared.await;
        cx.update(|cx| {
            open_editor_with_image(cx, &path, state, image);
            if activate {
                cx.activate(true);
            }
        });
    })
    .detach();
}

pub(crate) fn open_editor_with_image(
    cx: &mut App,
    path: &str,
    editor_state: Option<editor::window::PersistedEditorState>,
    image: image::ImageResult<editor::window::PreparedScreenshot>,
) -> bool {
    let image = match image {
        Ok(image) => image,
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
        .primary_display()
        .or_else(|| cx.displays().into_iter().next())
        .as_ref()
        .map(|display| {
            let bounds = crate::system::work_area::work_area(display.as_ref());
            (f32::from(bounds.size.width), f32::from(bounds.size.height))
        })
        .unwrap_or((1920.0, 1080.0));
    let stored = crate::config::window_state::get(crate::config::window_state::SCREENSHOT_EDITOR);
    let (image_width, image_height) = image.dimensions();
    let (width, height) =
        chrome::editor_window_size(image_width, image_height, work_width, work_height, stored);
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let opened = cx.open_window(
        windows::app_window_options(
            bounds,
            Some(size(
                px(chrome::EDITOR_MIN_WIDTH.min(width)),
                px(chrome::EDITOR_MIN_HEIGHT.min(height)),
            )),
        ),
        |window, cx| {
            native::register_dock_window(window.window_handle().window_id(), cx);
            cx.new(|cx| {
                editor::EditorWindow::from_prepared(path, editor_state, Some(image), window, cx)
            })
        },
    );
    if let Err(error) = opened {
        windows::toast::Toast::show(
            cx,
            "Unable to Open Screenshot",
            format!("The editor window could not be opened: {error}"),
        );
        return false;
    }
    crate::config::window_state::set(
        crate::config::window_state::SCREENSHOT_EDITOR,
        width,
        height,
    );
    true
}

fn is_video_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "poratake" | "mp4" | "mov" | "webm"
            )
        })
}

fn open_path(cx: &mut App, path: String, activate: bool) {
    let path = system::single_instance::decode_file_url(&path).unwrap_or(path);
    let candidate = std::path::Path::new(&path);
    if is_video_path(candidate) {
        windows::video_editor::VideoEditorWindow::open(cx, Some(path));
        if activate {
            cx.activate(true);
        }
        return;
    }
    open_editor_async(cx, path, activate);
}

fn dispatch_cli_intent(id: Option<&str>, extra: Option<String>, cx: &mut App) {
    match id.and_then(Intent::from_id) {
        Some(Intent::OpenSettings) => {
            let category = extra
                .as_deref()
                .and_then(windows::settings::registry::Category::from_id)
                .unwrap_or(windows::settings::registry::Category::General);
            intents::open_settings(category, cx);
            cx.activate(true);
        }
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

fn dispatch_instance_command(command: system::single_instance::InstanceCommand, cx: &mut App) {
    use system::single_instance::InstanceCommand;

    match command {
        InstanceCommand::Activate => {
            let store = state::state(cx).config;
            if store.get().general.hide_menu_bar_icon {
                store.update(|settings| settings.general.hide_menu_bar_icon = false);
                intents::refresh_shell(cx);
            } else {
                windows::history::HistoryWindow::toggle(None, cx);
            }
            cx.activate(true);
        }
        InstanceCommand::OpenPaths(paths) => {
            for path in paths {
                open_path(cx, path, true);
            }
        }
        InstanceCommand::Intent { id, extra } => {
            dispatch_cli_intent(Some(id.as_str()), extra, cx);
        }
    }
}

struct SingleInstanceState {
    _guard: system::single_instance::SingleInstanceGuard,
    _tx: smol::channel::Sender<system::single_instance::InstanceCommand>,
}

impl gpui::Global for SingleInstanceState {}

fn main() {
    #[cfg(feature = "parity")]
    if std::env::var_os(ui::parity::daemon_fixture::PROCESS_ENV).is_some() {
        ui::parity::daemon_fixture::run();
        return;
    }
    #[cfg(feature = "parity")]
    if std::env::args().nth(1).as_deref() == Some("--parity") {
        ui::parity::capture_editor_interaction_states();
        return;
    }
    #[cfg(all(feature = "parity", target_os = "macos", not(test)))]
    if std::env::args().nth(1).as_deref() == Some("--native-popup-frame-probe") {
        ui::native_smoke::popup_frame_probe();
        return;
    }
    #[cfg(all(feature = "parity", target_os = "macos", not(test)))]
    if std::env::args().nth(1).as_deref() == Some("--native-ui-probe") {
        ui::native_smoke::native_ui_probe();
        return;
    }

    let argv: Vec<String> = std::env::args().collect();
    let Some(single_instance) = system::single_instance::acquire() else {
        let command = system::single_instance::InstanceCommand::from_args(&argv);
        let _ = system::single_instance::notify_running(&command);
        eprintln!("[single-instance] another Poratake GPUI instance is running — exiting");
        std::process::exit(0);
    };
    let (instance_tx, instance_rx) = single_instance.start_listener();
    let url_tx = instance_tx.clone();

    // Resolved after the single-instance gate so a CLI hand-off launch that is
    // about to exit never pays for display detection.
    #[cfg(target_os = "linux")]
    let display_environment = system::linux_session::configure();

    // `HeroGpuiAssets` carries the SVGs HeroGPUI's own chrome draws — the
    // select chevron, the menu check mark, the close glyphs. Without it those
    // resolve to nothing and the migrated components lose their indicators.
    // The app's own images are decoded to `RenderImage` rather than loaded
    // through an asset source, so there is nothing to fall back to.
    let app = application().with_assets(herogpui::HeroGpuiAssets);
    app.on_open_urls(move |urls| {
        let paths: Vec<String> = urls
            .into_iter()
            .filter_map(|url| system::single_instance::decode_file_url(&url))
            .collect();
        if !paths.is_empty() {
            let _ =
                url_tx.send_blocking(system::single_instance::InstanceCommand::OpenPaths(paths));
        }
    });
    #[cfg(target_os = "macos")]
    app.on_reopen(|cx| {
        if state::try_native(cx).is_none() {
            return;
        }
        let store = state::state(cx).config;
        if store.get().general.hide_menu_bar_icon {
            store.update(|settings| settings.general.hide_menu_bar_icon = false);
            intents::refresh_shell(cx);
        }
    });
    app.run(move |cx: &mut App| {
        cx.set_global(SingleInstanceState {
            _guard: single_instance,
            _tx: instance_tx,
        });
        cx.spawn(async move |cx| {
            while let Ok(command) = instance_rx.recv().await {
                cx.update(|cx| dispatch_instance_command(command, cx));
            }
        })
        .detach();
        #[cfg(target_os = "linux")]
        display_environment.restore();

        let mut args = std::env::args().skip(1);
        let argument = args.next();
        let defer_daemon = cfg!(target_os = "macos")
            && argument.as_deref().is_some_and(|argument| {
                !argument.starts_with("--") && std::path::Path::new(argument).is_file()
            });
        native::configure_app(cx);
        let config = state::init(cx, defer_daemon);
        let settings = config.get();
        let mode = resolve_theme_mode(ThemeMode::parse(&settings.appearance.mode));
        ui::font::register(cx);
        // `ThemeProvider` must exist before any HeroGPUI component renders;
        // the first `init_theme` below publishes the app's resolved tokens.
        herogpui::init(cx);
        theme::vars::init_theme(cx, mode, &settings.appearance.theme);
        #[cfg(any(windows, target_os = "linux"))]
        {
            let system_theme = watcher::spawn();
            cx.spawn(async move |cx| {
                while let Ok(mode) = system_theme.recv().await {
                    cx.update(|cx| watcher::apply_system_mode(mode, cx));
                }
            })
            .detach();
        }
        editor::actions::init_bindings(cx);
        capture::overlay::init_bindings(cx);
        // `capture/index.ts` prewarms the freeze pipeline at startup so the
        // first capture does not pay for initialising it.
        if !defer_daemon {
            capture::prewarm_freeze_screen(cx);
        }
        // `applyLoginItemSetting()` in the Electron shell reconciles the Run
        // entry with the stored preference on launch; this is the GPUI
        // shell's half of that, so both shells agree on one entry. Comparing
        // first keeps every launch from rewriting a registry value that is
        // already correct.
        system::startup::reconcile_open_at_login(settings.general.start_on_login);

        let bridge = native::spawn(
            TrayMenuState::from_config(&settings, &crate::update::current_status(cx)),
            system::hotkeys::bindings(&settings),
        );
        if settings.general.hide_menu_bar_icon {
            bridge.send(NativeCommand::SetTrayVisible(false));
        }
        let events = bridge.events();
        state::set_native(cx, bridge);
        crate::update::spawn_auto_check(cx);

        cx.spawn(async move |cx| {
            while let Ok(event) = events.recv().await {
                cx.update(|cx| dispatch_native_event(event, cx));
            }
        })
        .detach();

        #[cfg(target_os = "linux")]
        {
            // Probe verdicts arrive from background threads, but the native
            // tray menu above was built once up front; refresh it when a
            // pending verdict flips so late-starting backends are picked up.
            let session = system::linux_session::current();
            let snapshot = system::linux_session::capabilities();
            let pending = match session {
                system::linux_session::LinuxSession::Wayland if !snapshot.screen_cast => true,
                system::linux_session::LinuxSession::X11 if !snapshot.ffmpeg_encoder => true,
                _ => false,
            };
            if pending {
                cx.spawn(async move |cx| {
                    for _ in 0..120 {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(250))
                            .await;
                        if system::linux_session::capabilities() != snapshot {
                            cx.update(crate::intents::refresh_shell);
                            return;
                        }
                    }
                })
                .detach();
            }
        }

        windows::keepalive::KeepAlive::open(cx);
        #[cfg(windows)]
        if !settings.general.hide_menu_bar_icon {
            windows::tray_menu::TrayMenuWindow::prewarm(cx);
        }
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
        match argument.as_deref() {
            #[cfg(all(feature = "parity", target_os = "macos"))]
            Some("--native-smoke-recording") => ui::native_smoke::open(cx),
            Some("--intent") => {
                let id = args.next();
                let extra = args.next();
                dispatch_cli_intent(id.as_deref(), extra, cx);
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
                    windows::pin::PinWindow::open_file(cx, path.into());
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
                        None,
                        windows::recording_control::PreferenceScope::Global,
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
                let Some(display) = cx
                    .primary_display()
                    .or_else(|| cx.displays().into_iter().next())
                else {
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
            Some(path) => open_path(cx, path.to_owned(), true),
            None => {}
        }
    });
}

#[cfg(test)]
mod single_instance_url_tests {
    use crate::system::single_instance::decode_file_url;

    #[test]
    fn decodes_percent_encoded_macos_file_urls() {
        assert_eq!(
            decode_file_url("file:///Users/test/My%20Captures/shot%201.png").as_deref(),
            Some("/Users/test/My Captures/shot 1.png")
        );
        assert_eq!(
            decode_file_url("file://localhost/tmp/test.poratake").as_deref(),
            Some("/tmp/test.poratake")
        );
        assert!(decode_file_url("https://example.com/a.png").is_none());
    }
}
