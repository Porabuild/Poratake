use std::io::BufRead;
use std::path::PathBuf;

#[cfg(not(test))]
use herogpui::gpui::MouseExitEvent;
use herogpui::gpui::{
    point, px, App, AppContext, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PlatformInput,
};

use crate::capture::overlay::{AreaOverlay, ScreenRect};
use crate::video::recorder::{self, RecordingTarget};
use crate::windows::recording_control::{PreferenceScope, RecordingControl};
use crate::windows::registry::{self, WindowKind};
use crate::Intent;

#[cfg(not(test))]
fn probe_tooltip(window: &herogpui::gpui::Window) -> Option<String> {
    crate::system::macos_tooltip::status(window)
}

#[cfg(not(test))]
fn timer_edge_position(cx: &mut App) {
    use crate::capture::{overlay, timer};
    use crate::system::work_area;
    use poratake_daemon_common::contract::{
        TIMER_CONTROL_HEIGHT, TIMER_CONTROL_TOP_MARGIN, TIMER_CONTROL_WIDTH,
    };

    let display = cx.primary_display().expect("primary display");
    let bounds = work_area::display_bounds(display.as_ref());
    let scale = work_area::capture_scale_factor(display.as_ref(), cx);
    let full = overlay::display_capture(bounds, scale, Some(u64::from(display.id()) as u32));
    let work = timer::placement_bounds(full, cx).expect("timer placement bounds");
    let expected = overlay::physical_rect(work_area::work_area(display.as_ref()), scale);
    assert_eq!(work, expected);

    let mut left = full;
    left.rect.width = 10;
    left.rect.height = 100;
    let mut right = left;
    right.rect.x = full.rect.x + full.rect.width - right.rect.width;
    let left_position = timer::timer_position(left, Some(work));
    let right_position = timer::timer_position(right, Some(work));
    let top = work.y + TIMER_CONTROL_TOP_MARGIN;
    assert_eq!(left_position, (work.x, top));
    assert_eq!(
        right_position,
        (work.x + work.width - TIMER_CONTROL_WIDTH, top)
    );
    assert!(top + TIMER_CONTROL_HEIGHT <= work.y + work.height);
    println!(
        "timer edge placement: display={} {} {} {}, left={} {}, right={} {}",
        full.rect.x,
        full.rect.y,
        full.rect.width,
        full.rect.height,
        left_position.0,
        left_position.1,
        right_position.0,
        right_position.1
    );
}

#[cfg(not(test))]
pub fn popup_frame_probe() {
    use cocoa::appkit::NSWindowStyleMask;
    use cocoa::base::{id, nil, BOOL, YES};
    use cocoa::foundation::NSRect;
    use herogpui::gpui::{div, App, Bounds, Context, Render, Window};
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    struct Probe;
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
            div()
        }
    }

    let directory =
        std::env::temp_dir().join(format!("poratake-scroll-probe-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let config_path = directory.join("config.json");
    herogpui::application()
        .with_assets(herogpui::HeroGpuiAssets)
        .run(move |cx: &mut App| {
        let primary_id = cx.primary_display().map(|display| display.id());
        for display in cx.displays() {
            let bounds = crate::system::work_area::display_bounds(display.as_ref());
            let work = crate::system::work_area::work_area(display.as_ref());
            println!(
                "GPUI display id={} preference={:?} primary={} bounds={:?} work={:?}",
                u64::from(display.id()),
                crate::system::work_area::display_preference_id(display.id()),
                Some(display.id()) == primary_id,
                bounds,
                work
            );
        }
        let options = crate::windows::popup_window_options(
            Bounds::new(point(px(100.), px(100.)), gpui::size(px(168.), px(52.))),
            crate::windows::PopupWindowConfig {
                show: false,
                focus: false,
                ..Default::default()
            },
        );
        let result = cx.open_window(options, |window, cx| {
            let handle = HasWindowHandle::window_handle(window).unwrap();
            let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
                panic!("expected an AppKit window");
            };
            let view = appkit.ns_view.as_ptr() as id;
            let native_window: id = unsafe { msg_send![view, window] };
            assert_ne!(native_window, nil);
            let visible: BOOL = unsafe { msg_send![native_window, isVisible] };
            assert_ne!(visible, YES);
            let before: NSWindowStyleMask = unsafe { msg_send![native_window, styleMask] };
            let before_shadow: BOOL = unsafe { msg_send![native_window, hasShadow] };
            let before_frame: NSRect = unsafe { msg_send![native_window, frame] };
            let before_view: NSRect = unsafe { msg_send![view, frame] };
            crate::windows::remove_popup_frame(window);
            let after: NSWindowStyleMask = unsafe { msg_send![native_window, styleMask] };
            let after_shadow: BOOL = unsafe { msg_send![native_window, hasShadow] };
            let after_frame: NSRect = unsafe { msg_send![native_window, frame] };
            let after_view: NSRect = unsafe { msg_send![view, frame] };
            assert!(before.contains(NSWindowStyleMask::NSTitledWindowMask));
            assert!(!after.contains(NSWindowStyleMask::NSTitledWindowMask));
            assert_ne!(after_shadow, YES);
            assert_eq!(before_frame.origin.x, after_frame.origin.x);
            assert_eq!(before_frame.origin.y, after_frame.origin.y);
            assert_eq!(before_frame.size.width, after_frame.size.width);
            assert_eq!(before_frame.size.height, after_frame.size.height);
            assert_eq!(before_view.origin.x, after_view.origin.x);
            assert_eq!(before_view.origin.y, after_view.origin.y);
            assert_eq!(before_view.size.width, after_view.size.width);
            assert_eq!(before_view.size.height, after_view.size.height);
            let accepts_motion: BOOL = unsafe { msg_send![native_window, acceptsMouseMovedEvents] };
            assert_eq!(
                accepts_motion, YES,
                "frameless popup must accept unpressed mouse movement"
            );
            println!("frameless popup accepts unpressed mouse movement: true");
            let ignores_before: BOOL = unsafe { msg_send![native_window, ignoresMouseEvents] };
            crate::system::click_through::enable(window);
            let ignores_after: BOOL = unsafe { msg_send![native_window, ignoresMouseEvents] };
            assert_ne!(ignores_before, YES);
            assert_eq!(ignores_after, YES);
            for origin in [point(px(100.), px(100.)), point(px(-384.), px(-144.))] {
                let desktop_bounds = Bounds::new(origin, gpui::size(px(168.), px(52.)));
                assert!(crate::windows::set_macos_window_bounds(window, desktop_bounds));
                assert_eq!(crate::windows::desktop_window_bounds(window), desktop_bounds);
            }
            let visible: BOOL = unsafe { msg_send![native_window, isVisible] };
            assert_ne!(visible, YES);
            println!("hidden popup round-trips positive and negative desktop coordinates");
            println!(
                "popup titled before={} after={}, native shadow before={} after={}, frame unchanged, click-through enabled",
                before.contains(NSWindowStyleMask::NSTitledWindowMask),
                after.contains(NSWindowStyleMask::NSTitledWindowMask),
                before_shadow == YES,
                after_shadow == YES
            );
            cx.new(|_| Probe)
        });
            assert!(result.is_ok());
            let config = std::sync::Arc::new(
                crate::config::store::ConfigStore::load_at(config_path).unwrap(),
            );
            crate::state::set_test_state(cx, config);
            crate::ui::font::register(cx);
            crate::windows::scroll_capture::native_window_probe(cx, None);
            crate::capture::overlay::native_scroll_selector_probe(cx);
            timer_edge_position(cx);
            cx.quit();
        });
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(not(test))]
fn attachment_cancel_probe(path: &std::path::Path, cx: &mut App) {
    use cocoa::base::{id, nil, BOOL, NO, YES};
    use cocoa::foundation::{NSPoint, NSString};
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let native_view = |window: &herogpui::gpui::Window| {
        let handle = HasWindowHandle::window_handle(window).unwrap();
        let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
            panic!("expected an AppKit view");
        };
        appkit.ns_view.as_ptr() as id
    };
    let visible = move |window: &herogpui::gpui::Window| {
        let native: id = unsafe { msg_send![native_view(window), window] };
        let visible: BOOL = unsafe { msg_send![native, isVisible] };
        visible == YES
    };
    let service = crate::state::state(cx);
    let freeze = std::env::var_os("PORATAKE_NATIVE_UI_ATTACHMENT_FREEZE").is_some();
    if freeze {
        assert!(std::env::var_os("PORATAKE_DAEMON_PATH").is_some());
    }
    service.config.update(|config| {
        config.screenshot.freeze_screen = freeze;
        config.screenshot.hide_desktop_icons = false;
        config.history.enabled = false;
    });
    crate::capture::overlay::init_bindings(cx);
    assert!(crate::open_editor_for(cx, path.to_str().unwrap()));
    let editor = cx
        .windows()
        .into_iter()
        .find_map(|handle| handle.downcast::<crate::editor::EditorWindow>())
        .expect("screenshot editor");
    let before = editor
        .update(cx, |editor, window, cx| {
            assert!(visible(window));
            let before = editor.export_png().expect("initial composition");
            editor.attach_layer(crate::editor::layers::Edge::Right, window, cx);
            assert!(!visible(window));
            before
        })
        .unwrap();
    let path = path.to_owned();
    cx.spawn(async move |cx| {
        let wait_step = std::time::Duration::from_millis(20);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
        let selector = loop {
            if let Some(selector) = cx.update(crate::capture::overlay::parity_active_overlay) {
                break selector;
            }
            assert!(std::time::Instant::now() < deadline, "attachment selector did not open");
            cx.background_executor().timer(wait_step).await;
        };
        let (view, number) = cx.update(|cx| {
            assert!(crate::state::coordinator(cx).read(cx).has_pending_editor_attach());
            selector.update(cx, |_, window, cx| {
                assert!(visible(window));
                window.draw(cx).clear(cx);
                let view = native_view(window);
                let native: id = unsafe { msg_send![view, window] };
                let number: isize = unsafe { msg_send![native, windowNumber] };
                (view, number)
            }).unwrap()
        });
        unsafe {
            let escape = NSString::alloc(nil).init_str("\u{1b}");
            let event: id = msg_send![class!(NSEvent), keyEventWithType: 10usize
                location: NSPoint::new(0., 0.) modifierFlags: 0usize timestamp: 0.0f64
                windowNumber: number context: nil characters: escape
                charactersIgnoringModifiers: escape isARepeat: NO keyCode: 53u16];
            assert_ne!(event, nil);
            let _: () = msg_send![view, keyDown: event];
            let _: () = msg_send![escape, release];
        }
        cx.update(|cx| {
            assert!(!crate::capture::overlay::parity_is_active(cx), "native Escape must close the attachment selector");
            assert!(!crate::state::coordinator(cx).read(cx).has_pending_editor_attach());
            assert!(selector.update(cx, |_, _, _| ()).is_err());
            editor.update(cx, |editor, window, _| {
                assert!(visible(window));
                assert_eq!(editor.export_png().expect("restored composition"), before);
            }).unwrap();
            assert!(path.is_file());
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
        while cx.update(crate::capture::overlay::parity_has_pending_freeze) {
            assert!(std::time::Instant::now() < deadline, "attachment freeze was not released");
            cx.background_executor().timer(wait_step).await;
        }
        if freeze {
            cx.background_executor().spawn(async move { service.daemon.stop() }).await;
        }
        cx.update(|cx| {
            println!("attachment cancel: AppKit Escape restored the editor and preserved the composition");
            if freeze {
                println!("attachment cancel: pending freeze release settled");
            }
            cx.quit();
        });
    }).detach();
}

#[cfg(not(test))]
fn open_recording_probe_toolbar(cx: &mut App, named_target: bool) {
    RecordingControl::open(
        cx,
        if named_target {
            RecordingTarget::Window
        } else {
            RecordingTarget::Area
        },
        ScreenRect {
            x: 200,
            y: 200,
            width: 640,
            height: 360,
        },
        None,
        named_target.then_some(1),
        named_target.then(|| "Generated window".into()),
        PreferenceScope::Global,
    );
}

#[cfg(not(test))]
pub fn native_ui_probe() {
    let directory =
        std::env::temp_dir().join(format!("poratake-native-ui-probe-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    unsafe {
        std::env::set_var(crate::config::store::CONFIG_DIR_ENV, &directory);
    }
    let image_path = directory.join("preview.png");
    image::RgbaImage::from_fn(640, 360, |x, y| {
        if (x / 40 + y / 40) % 2 == 0 {
            image::Rgba([64, 112, 192, 255])
        } else {
            image::Rgba([224, 232, 240, 255])
        }
    })
    .save(&image_path)
    .unwrap();
    let theme = std::env::var("PORATAKE_NATIVE_UI_THEME").unwrap_or_else(|_| "dark".into());
    assert!(matches!(theme.as_str(), "light" | "dark"));
    let normal_level = std::env::var_os("PORATAKE_NATIVE_UI_NORMAL_LEVEL").is_some();
    let native_shadow = std::env::var_os("PORATAKE_NATIVE_UI_NATIVE_SHADOW").is_some();
    let auto_dismiss = std::env::var_os("PORATAKE_NATIVE_UI_AUTO_DISMISS").is_some();
    let config_path = directory.join("config.json");
    let screenshots_path = directory.to_string_lossy().into_owned();
    let probe_directory = directory.clone();
    herogpui::application()
        .with_assets(herogpui::HeroGpuiAssets)
        .run(move |cx: &mut App| {
            let config = std::sync::Arc::new(
                crate::config::store::ConfigStore::load_at(config_path).unwrap(),
            );
            config.update(|config| {
                config.appearance.mode = theme;
                config.recording.system_audio = false;
                config.recording.mic_enabled = false;
                config.recording.camera.enabled = false;
                config.preview.corner = "bottom-right".into();
                config.preview.auto_dismiss = auto_dismiss;
                config.preview.auto_dismiss_seconds = 4.0;
                config.save_locations.screenshot = screenshots_path.clone();
                config.storage.recordings_path = screenshots_path.clone();
                config.storage.screenshots_path = screenshots_path;
                config.screenshot.format = "png".into();
                config.screenshot.close_on_save = false;
            });
            crate::state::set_test_state(cx, config);
            crate::ui::font::register(cx);
            crate::editor::actions::init_bindings(cx);
            if std::env::args().nth(2).as_deref() == Some("--attachment-cancel") {
                attachment_cancel_probe(&image_path, cx);
                return;
            }
            if std::env::args().nth(2).as_deref() == Some("--scroll-exclusion") {
                crate::windows::scroll_capture::native_scroll_exclusion_probe(cx);
                return;
            }
            open_recording_probe_toolbar(cx, false);
            crate::windows::capture_preview::CapturePreviewWindow::open(cx, image_path);
        for (kind, name) in [
            (WindowKind::RecordingControl, "recording control"),
            (WindowKind::CapturePreview, "capture preview"),
        ] {
            let handle = registry::handle(kind, cx).expect("native probe window");
            cx.update_window(handle, |_, window, _| {
                use cocoa::appkit::NSWindowStyleMask;
                use cocoa::base::{id, nil, BOOL, YES};
                use cocoa::foundation::NSRect;
                use objc::{msg_send, sel, sel_impl};
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                let RawWindowHandle::AppKit(appkit) = window.window_handle().unwrap().as_raw() else {
                    panic!("expected an AppKit window");
                };
                let view = appkit.ns_view.as_ptr() as id;
                let native: id = unsafe { msg_send![view, window] };
                assert_ne!(native, nil);
                if normal_level {
                    let _: () = unsafe { msg_send![native, setLevel: 0isize] };
                }
                if native_shadow && kind == WindowKind::RecordingControl {
                    let _: () = unsafe { msg_send![native, setHasShadow: YES] };
                }
                let visible: BOOL = unsafe { msg_send![native, isVisible] };
                let shadow: BOOL = unsafe { msg_send![native, hasShadow] };
                let level: isize = unsafe { msg_send![native, level] };
                let style: NSWindowStyleMask = unsafe { msg_send![native, styleMask] };
                let frame: NSRect = unsafe { msg_send![native, frame] };
                assert_eq!(visible, YES);
                assert_eq!(shadow == YES, native_shadow && kind == WindowKind::RecordingControl);
                assert!(if normal_level { level == 0 } else { level > 0 });
                assert!(!style.contains(NSWindowStyleMask::NSTitledWindowMask));
                    if kind == WindowKind::CapturePreview {
                        let screen: id = unsafe { msg_send![native, screen] };
                        assert_ne!(screen, nil);
                        let work_area: NSRect = unsafe { msg_send![screen, visibleFrame] };
                        let right = work_area.origin.x + work_area.size.width
                            - frame.origin.x - frame.size.width;
                        let bottom = frame.origin.y - work_area.origin.y;
                        let expected = f64::from(crate::ui::chrome::PREVIEW_MARGIN - crate::ui::chrome::PREVIEW_SHADOW_PADDING);
                        assert!((right - expected).abs() <= 0.5, "right margin {right}");
                        assert!((bottom - expected).abs() <= 0.5, "bottom margin {bottom}");
                        println!("capture preview: bottom and right AppKit work-area margins {bottom:.0},{right:.0} points");
                    }
                println!("{name}: visible, borderless, native shadow={}, level={level}; frame {:.0},{:.0} {:.0}x{:.0}", shadow == YES, frame.origin.x, frame.origin.y, frame.size.width, frame.size.height);
            }).unwrap();
        }
            let (tx, rx) = smol::channel::unbounded::<String>();
            std::thread::spawn(move || {
                for command in std::io::stdin().lock().lines().map_while(Result::ok) {
                    if tx.send_blocking(command).is_err() {
                        break;
                    }
                }
            });
            cx.spawn(async move |cx| {
                while let Ok(command) = rx.recv().await {
                    cx.update(|cx| {
                        if command.is_empty() || command == "quit" {
                            cx.quit();
                            return;
                        }
                        if command == "timer-edge-position" {
                            timer_edge_position(cx);
                            return;
                        }
                        if command == "rectangle" {
                            let Some(handle) = cx.windows().into_iter().find(|window| window.downcast::<crate::editor::EditorWindow>().is_some()) else {
                                println!("isolated editor is not open");
                                return;
                            };
                            let _ = cx.update_window(handle, |_, window, cx| window.dispatch_action(Box::new(crate::editor::actions::ToolRectangle), cx));
                            println!("isolated editor rectangle tool selected");
                            return;
                        }
                        if command == "annotation-status" {
                            let Some(handle) = cx.windows().into_iter().find_map(|window| window.downcast::<crate::editor::EditorWindow>()) else {
                                println!("isolated editor is not open");
                                return;
                            };
                            let _ = handle.update(cx, |editor, _, _| {
                                let exported = image::load_from_memory(&editor.export_png().unwrap()).unwrap().to_rgba8();
                                let original = image::open(probe_directory.join("preview.png")).unwrap().to_rgba8();
                                assert_eq!(exported.dimensions(), original.dimensions());
                                let changed = exported.pixels().zip(original.pixels()).filter(|(a, b)| a != b).count();
                                println!("isolated annotation changed pixels={changed}");
                            });
                            return;
                        }
                        if command == "reopen-saved" {
                            let images: Vec<_> = std::fs::read_dir(&probe_directory).unwrap().filter_map(Result::ok)
                                .map(|entry| entry.path())
                                .filter(|path| path.extension().is_some_and(|ext| ext == "png") && path.file_name().is_some_and(|name| name != "preview.png"))
                                .collect();
                            if images.len() != 1 {
                                println!("expected one isolated saved PNG, found {}", images.len());
                                return;
                            }
                            let saved = image::open(&images[0]).unwrap().to_rgba8();
                            let original = image::open(probe_directory.join("preview.png")).unwrap().to_rgba8();
                            assert_eq!(saved.dimensions(), original.dimensions());
                            let changed = saved.pixels().zip(original.pixels()).filter(|(a, b)| a != b).count();
                            println!("isolated saved PNG: {}x{}, changed pixels={changed}", saved.width(), saved.height());
                            if changed == 0 {
                                return;
                            }
                            let existing = cx.windows().into_iter().find_map(|window| window.downcast::<crate::editor::EditorWindow>()).unwrap();
                            let _ = existing.update(cx, |editor, _, _| {
                                let exported = image::load_from_memory(&editor.export_png().unwrap()).unwrap().to_rgba8();
                                assert_eq!(saved, exported, "saved PNG must match the editor output");
                            });
                            println!("isolated saved PNG matches the editor output");
                            let opened = crate::open_editor_for(cx, images[0].to_str().unwrap());
                            println!("isolated saved PNG reopened: {opened}");
                            if opened {
                                let mut matched = false;
                                for handle in cx.windows().into_iter().filter_map(|window| window.downcast::<crate::editor::EditorWindow>()) {
                                    let _ = handle.update(cx, |editor, _, _| {
                                        if std::path::Path::new(&editor.file_path) != images[0] {
                                            return;
                                        }
                                        let reopened = image::load_from_memory(&editor.export_png().unwrap()).unwrap().to_rgba8();
                                        assert_eq!(saved, reopened, "reopened image must match the saved PNG");
                                        matched = true;
                                    });
                                }
                                assert!(matched, "reopened editor must exist");
                                println!("isolated reopened editor matches the saved PNG");
                            }
                            return;
                        }
                        if command == "save-to-fixture" {
                            let Some(handle) = cx.windows().into_iter().find_map(|window| window.downcast::<crate::editor::EditorWindow>()) else {
                                println!("isolated editor is not open");
                                return;
                            };
                            let path = probe_directory.join("fixture-saved.png");
                            assert!(!path.exists());
                            let _ = handle.update(cx, |editor, _, _| {
                                let bytes = editor.export_png().unwrap();
                                crate::editor::screenshot_save_unix::write(&path, &bytes).unwrap();
                            });
                            println!("isolated fixture PNG saved");
                            return;
                        }
                        if command == "draw" {
                            let Some(handle) = cx.windows().into_iter().find(|window| window.downcast::<crate::editor::EditorWindow>().is_some()) else {
                                return;
                            };
                            for event in [
                                PlatformInput::MouseDown(MouseDownEvent {
                                    position: point(px(220.), px(200.)),
                                    button: MouseButton::Left,
                                    click_count: 1,
                                    modifiers: Modifiers::none(),
                                    first_mouse: false,
                                }),
                                PlatformInput::MouseMove(MouseMoveEvent {
                                    position: point(px(420.), px(330.)),
                                    pressed_button: Some(MouseButton::Left),
                                    modifiers: Modifiers::none(),
                                }),
                                PlatformInput::MouseUp(MouseUpEvent {
                                    position: point(px(420.), px(330.)),
                                    button: MouseButton::Left,
                                    click_count: 1,
                                    modifiers: Modifiers::none(),
                                }),
                            ] {
                                let _ = cx.update_window(handle, |_, window, cx| window.dispatch_event(event, cx));
                            }
                            println!("synthetic editor drag dispatched");
                            return;
                        }
                        if command == "save-open" {
                            let Some(handle) = cx.windows().into_iter().find_map(|window| window.downcast::<crate::editor::EditorWindow>()) else {
                                println!("isolated editor is not open");
                                return;
                            };
                            let _ = handle.update(cx, |editor, window, cx| editor.save_as(window, cx));
                            println!("isolated editor requested save dialog");
                            return;
                        }
                        if command == "save-status" {
                            let Some(handle) = cx.windows().into_iter().find_map(|window| window.downcast::<crate::editor::EditorWindow>()) else {
                                println!("isolated editor is not open");
                                return;
                            };
                            let _ = handle.update(cx, |editor, _, _| println!("isolated save pending={}", editor.parity_save_pending()));
                            return;
                        }
                        if matches!(command.as_str(), "save-panel-status" | "save-cancel") {
                            use cocoa::base::{id, nil, BOOL, YES};
                            use objc::{class, msg_send, sel, sel_impl};
                            let app: id = unsafe { msg_send![class!(NSApplication), sharedApplication] };
                            let windows: id = unsafe { msg_send![app, windows] };
                            let count: usize = unsafe { msg_send![windows, count] };
                            for index in 0..count {
                                let panel: id = unsafe { msg_send![windows, objectAtIndex: index] };
                                let is_save: BOOL = unsafe { msg_send![panel, isKindOfClass: class!(NSSavePanel)] };
                                let visible: BOOL = unsafe { msg_send![panel, isVisible] };
                                if is_save != YES || visible != YES {
                                    continue;
                                }
                                if command == "save-cancel" {
                                    let _: () = unsafe { msg_send![panel, cancel: nil] };
                                    println!("isolated save panel canceled");
                                    return;
                                }
                                let url: id = unsafe { msg_send![panel, directoryURL] };
                                let path: id = unsafe { msg_send![url, path] };
                                let name: id = unsafe { msg_send![panel, nameFieldStringValue] };
                                let path: *const std::ffi::c_char = unsafe { msg_send![path, UTF8String] };
                                let name: *const std::ffi::c_char = unsafe { msg_send![name, UTF8String] };
                                assert!(!path.is_null() && !name.is_null());
                                let path = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy();
                                let name = unsafe { std::ffi::CStr::from_ptr(name) }.to_string_lossy();
                                println!("save panel directory={path}, name={name}");
                                return;
                            }
                            println!("isolated save panel is not visible");
                            return;
                        }
                        if matches!(command.as_str(), "hover" | "hover-close" | "hover-delete") {
                            let Some(handle) = registry::handle(WindowKind::CapturePreview, cx) else {
                                return;
                            };
                            let position = match command.as_str() {
                                "hover-close" => point(px(25.), px(25.)),
                                "hover-delete" => point(px(183.), px(25.)),
                                _ => point(px(100.), px(60.)),
                            };
                            let _ = cx.update_window(handle, |_, window, cx| {
                                println!("preview active before synthetic hover: {}", window.is_window_active());
                                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent {
                                    position,
                                    pressed_button: None,
                                    modifiers: Modifiers::none(),
                                }), cx);
                            });
                            return;
                        }
                        if matches!(command.as_str(), "tooltip-status" | "record-tooltip-status") {
                            let kind = if command == "tooltip-status" { WindowKind::CapturePreview } else { WindowKind::RecordingControl };
                            let Some(handle) = registry::handle(kind, cx) else {
                                return;
                            };
                            let _ = cx.update_window(handle, |_, window, _| {
                                println!("{command}: {}", probe_tooltip(window).as_deref().unwrap_or("none"));
                            });
                            return;
                        }
                        if let Some((surface, key)) = command.split_once("-key ")
                            .filter(|(surface, _)| matches!(*surface, "record" | "scroll" | "preview"))
                        {
                            use gpui::{KeyDownEvent, KeyUpEvent, Keystroke};
                            let key = match key {
                                "tab" => "tab",
                                "shift-tab" => "shift-tab",
                                _ => panic!("unsupported probe keystroke"),
                            };
                            let keystroke = Keystroke::parse(key).expect("probe keystroke");
                            let handle = match surface {
                                "scroll" => crate::windows::scroll_capture::native_control_for_probe(cx),
                                "preview" => registry::handle(WindowKind::CapturePreview, cx),
                                _ => registry::handle(WindowKind::RecordingControl, cx),
                            };
                            if let Some(handle) = handle {
                                let _ = cx.update_window(handle, |_, window, cx| {
                                    window.dispatch_event(PlatformInput::KeyDown(KeyDownEvent {
                                        keystroke: keystroke.clone(),
                                        is_held: false,
                                        prefer_character_input: false,
                                    }), cx);
                                });
                                if cx.windows().contains(&handle) {
                                    let _ = cx.update_window(handle, |_, window, cx| {
                                        window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
                                    });
                                }
                            }
                            println!("{surface} key dispatched: {key}");
                            return;
                        }
                        if let Some(index) = command.strip_prefix("record-hover-control ") {
                            let index: usize = index.parse().expect("recording control index");
                            if let Some(handle) = registry::handle(WindowKind::RecordingControl, cx) {
                                let _ = cx.update_window(handle, |_, window, cx| {
                                    let position = crate::ui::parity::measured("recording-controls", index).center();
                                    window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent {
                                        position,
                                        pressed_button: None,
                                        modifiers: Modifiers::none(),
                                    }), cx);
                                });
                            }
                            println!("record hover dispatched: {index}");
                            return;
                        }
                        if command == "preview-activate" {
                            cx.activate(true);
                            if let Some(handle) = registry::handle(WindowKind::CapturePreview, cx) {
                                let _ = cx.update_window(handle, |_, window, _| window.activate_window());
                            }
                            println!("preview activation requested");
                            return;
                        }
                        if matches!(command.as_str(), "record-focus-status" | "preview-focus-status") {
                            let kind = if command == "record-focus-status" { WindowKind::RecordingControl } else { WindowKind::CapturePreview };
                            if let Some(handle) = registry::handle(kind, cx) {
                                let _ = cx.update_window(handle, |_, window, cx| {
                                    let background = gpui::Background::from(crate::theme::vars::active_theme(cx).overlay);
                                    let scale = window.scale_factor();
                                    let tips = window.painted_quads().into_iter().filter(|quad| {
                                        quad.background == background
                                            && (31. ..=35.).contains(&(quad.bounds.size.height.0 / scale))
                                            && quad.bounds.size.width.0 / scale > 35.
                                    }).count();
                                    if kind == WindowKind::CapturePreview {
                                        println!("preview focus status: active={} inline={tips} focus={:?}", window.is_window_active(), window.focused(cx));
                                    } else {
                                        println!("record focus status: visible={} inline={tips} native={}", herogpui::extend::focus_visible(cx), probe_tooltip(window).as_deref().unwrap_or("none"));
                                    }
                                });
                            }
                            return;
                        }
                        if matches!(command.as_str(), "record-hover" | "record-exit") {
                            let Some(handle) = registry::handle(WindowKind::RecordingControl, cx) else {
                                return;
                            };
                            let _ = cx.update_window(handle, |_, window, cx| {
                                let event = if command == "record-hover" {
                                    PlatformInput::MouseMove(MouseMoveEvent {
                                        position: point(px(22.), px(22.)),
                                        pressed_button: None,
                                        modifiers: Modifiers::none(),
                                    })
                                } else {
                                    PlatformInput::MouseExited(MouseExitEvent {
                                        position: point(window.bounds().size.width + px(3.), px(22.)),
                                        pressed_button: None,
                                        modifiers: Modifiers::none(),
                                    })
                                };
                                window.dispatch_event(event, cx);
                                println!("{command}: window active={}", window.is_window_active());
                            });
                            return;
                        }
                        if matches!(command.as_str(), "record-shadow-on" | "record-shadow-off") {
                            let Some(handle) = registry::handle(WindowKind::RecordingControl, cx) else {
                                return;
                            };
                            let _ = cx.update_window(handle, |_, window, _| {
                                use cocoa::base::{id, BOOL, YES};
                                use objc::{msg_send, sel, sel_impl};
                                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                                let RawWindowHandle::AppKit(appkit) = window.window_handle().unwrap().as_raw() else {
                                    panic!("expected an AppKit window");
                                };
                                let native_view = appkit.ns_view.as_ptr() as id;
                                let native: id = unsafe { msg_send![native_view, window] };
                                let _: () = unsafe { msg_send![native, setHasShadow: command == "record-shadow-on"] };
                                let _: () = unsafe { msg_send![native, invalidateShadow] };
                                let shadow: BOOL = unsafe { msg_send![native, hasShadow] };
                                println!("recording native shadow={}", shadow == YES);
                            });
                            return;
                        }
                        if command == "record-hit-test" {
                            let Some(handle) = registry::handle(WindowKind::RecordingControl, cx)
                                .and_then(|handle| handle.downcast::<RecordingControl>()) else {
                                return;
                            };
                            let _ = handle.update(cx, |view, window, _| {
                                use cocoa::base::id;
                                use cocoa::foundation::{NSPoint, NSRect};
                                use objc::{class, msg_send, sel, sel_impl};
                                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                                let RawWindowHandle::AppKit(appkit) = window.window_handle().unwrap().as_raw() else {
                                    panic!("expected an AppKit window");
                                };
                                let native_view = appkit.ns_view.as_ptr() as id;
                                let native: id = unsafe { msg_send![native_view, window] };
                                let frame: NSRect = unsafe { msg_send![native, frame] };
                                let screen: id = unsafe { msg_send![native, screen] };
                                let work_area: NSRect = unsafe { msg_send![screen, visibleFrame] };
                                let number: isize = unsafe { msg_send![native, windowNumber] };
                                let width = f64::from(view.parity_bar_width().expect("toolbar width"));
                                let margin = (frame.size.width - width) / 2.0;
                                let y = frame.origin.y + frame.size.height / 2.0;
                                let center = NSPoint::new(frame.origin.x + frame.size.width / 2.0, y);
                                let outside = NSPoint::new(if margin > 1.0 { frame.origin.x + margin / 2.0 } else { frame.origin.x - 4.0 }, y);
                                let center_hit: isize = unsafe { msg_send![class!(NSWindow), windowNumberAtPoint: center belowWindowWithWindowNumber: 0isize] };
                                let outside_hit: isize = unsafe { msg_send![class!(NSWindow), windowNumberAtPoint: outside belowWindowWithWindowNumber: 0isize] };
                                let center_error = (center.x - (work_area.origin.x + work_area.size.width / 2.0)).abs();
                                println!("recording hit test: window={number}, center={center_hit}, margin={outside_hit}, window_width={:.1}, bar_width={width:.1}, side_margin={margin:.1}, center_error={center_error:.1}", frame.size.width);
                                assert_eq!(center_hit, number);
                                assert!(margin.abs() <= 0.5);
                                assert_ne!(outside_hit, number);
                                assert!(center_error <= 1.0);
                            });
                            return;
                        }
                        if command == "record-menu-status" {
                            let Some(handle) = registry::handle(WindowKind::RecordingControl, cx)
                                .and_then(|handle| handle.downcast::<RecordingControl>()) else {
                                return;
                            };
                            let _ = handle.update(cx, |view, window, cx| {
                                use cocoa::base::id;
                                use cocoa::foundation::NSRect;
                                use objc::{msg_send, sel, sel_impl};
                                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                                let RawWindowHandle::AppKit(appkit) = window.window_handle().unwrap().as_raw() else {
                                    panic!("expected an AppKit window");
                                };
                                let native_view = appkit.ns_view.as_ptr() as id;
                                let native: id = unsafe { msg_send![native_view, window] };
                                let frame: NSRect = unsafe { msg_send![native, frame] };
                                let screen: id = unsafe { msg_send![native, screen] };
                                let work_area: NSRect = unsafe { msg_send![screen, visibleFrame] };
                                let center_error = (frame.origin.x + frame.size.width / 2.0 - work_area.origin.x - work_area.size.width / 2.0).abs();
                                println!("recording menu: open={}, frame={:.1}x{:.1}, center_error={center_error:.1}", view.parity_menu_open(cx), frame.size.width, frame.size.height);
                            });
                            return;
                        }
                        if command == "record-scene-snapshot" {
                            let Some(handle) = registry::handle(WindowKind::RecordingControl, cx) else {
                                return;
                            };
                            let path = probe_directory.join("recording-scene.png");
                            let _ = cx.update_window(handle, |_, window, _| {
                                window.render_to_image().expect("recording scene").save(&path).expect("save recording scene");
                                println!("recording scene saved: {}", path.display());
                            });
                            return;
                        }
                        if matches!(command.as_str(), "record-open-mic-menu" | "record-open-camera-menu" | "record-open-ios-menu") {
                            assert!(std::env::var_os("PORATAKE_NATIVE_UI_FAKE_DAEMON").is_some());
                            let Some(handle) = registry::handle(WindowKind::RecordingControl, cx) else {
                                return;
                            };
                            let (name, x) = match command.as_str() {
                                "record-open-camera-menu" => ("camera", 62.),
                                "record-open-ios-menu" => ("iOS device", 204.),
                                _ => ("microphone", 112.),
                            };
                            let position = point(px(x), px(22.));
                            for event in [
                                PlatformInput::MouseMove(MouseMoveEvent { position, pressed_button: None, modifiers: Modifiers::none() }),
                                PlatformInput::MouseDown(MouseDownEvent { position, button: MouseButton::Left, click_count: 1, modifiers: Modifiers::none(), first_mouse: false }),
                                PlatformInput::MouseUp(MouseUpEvent { position, button: MouseButton::Left, click_count: 1, modifiers: Modifiers::none() }),
                            ] {
                                let _ = cx.update_window(handle, |_, window, cx| window.dispatch_event(event, cx));
                            }
                            println!("synthetic {name} menu click dispatched using fixture daemon");
                            return;
                        }
                        if command == "record-dismiss-menu" {
                            let dismissed = RecordingControl::cancel_pre_recording(cx);
                            println!("recording menu dismissed; control closed={dismissed}");
                            return;
                        }
                        if matches!(command.as_str(), "record-countdown-stall" | "record-countdown-status") {
                            let Some(handle) = registry::handle(WindowKind::RecordingControl, cx)
                                .and_then(|handle| handle.downcast::<RecordingControl>()) else {
                                return;
                            };
                            if command == "record-countdown-stall" {
                                assert!(std::env::var_os("PORATAKE_NATIVE_UI_FAKE_DAEMON").is_some());
                                crate::state::state(cx).config.update(|config| config.recording.start_delay = 5.);
                                handle.update(cx, |control, window, cx| control.start(window, cx)).unwrap();
                                std::thread::sleep(std::time::Duration::from_millis(2250));
                                println!("recording countdown probe resumed");
                            } else {
                                handle.update(cx, |control, _, _| {
                                    println!("recording countdown {:?}", control.parity_countdown_remaining());
                                }).unwrap();
                            }
                            return;
                        }
                        if command == "record-reopen" {
                            open_recording_probe_toolbar(cx, false);
                            println!("recording control reopened");
                            return;
                        }
                        if matches!(command.as_str(), "record-target-window" | "record-target-area") {
                            open_recording_probe_toolbar(cx, command == "record-target-window");
                            println!("recording target updated: {command}");
                            return;
                        }
                        if command == "record-audio-status" {
                            let Some(handle) = registry::handle(WindowKind::RecordingControl, cx)
                                .and_then(|handle| handle.downcast::<RecordingControl>()) else {
                                return;
                            };
                            let _ = handle.update(cx, |view, _, _| {
                                println!("recording system-audio preference: {}", view.parity_system_audio());
                            });
                            return;
                        }
                        if command == "record-click-sound" {
                            let Some(handle) = registry::handle(WindowKind::RecordingControl, cx) else {
                                return;
                            };
                            let position = point(px(162.), px(22.));
                            for event in [
                                PlatformInput::MouseMove(MouseMoveEvent {
                                    position,
                                    pressed_button: None,
                                    modifiers: Modifiers::none(),
                                }),
                                PlatformInput::MouseDown(MouseDownEvent {
                                    position,
                                    button: MouseButton::Left,
                                    click_count: 1,
                                    modifiers: Modifiers::none(),
                                    first_mouse: false,
                                }),
                                PlatformInput::MouseUp(MouseUpEvent {
                                    position,
                                    button: MouseButton::Left,
                                    click_count: 1,
                                    modifiers: Modifiers::none(),
                                }),
                            ] {
                                let _ = cx.update_window(handle, |_, window, cx| window.dispatch_event(event, cx));
                            }
                            println!("synthetic recording system-sound click dispatched");
                            return;
                        }
                        if command == "display-open" {
                            let service = crate::state::state(cx);
                            let bounds = gpui::Bounds::new(point(px(100.), px(100.)), gpui::size(px(500.), px(300.)));
                            let options = crate::windows::popup_window_options(
                                bounds,
                                crate::windows::PopupWindowConfig { focus: false, ..Default::default() },
                            );
                            cx.open_window(options, |window, cx| {
                                window.set_window_title("Poratake display picker probe");
                                crate::windows::remove_popup_frame(window);
                                cx.new(|cx| AreaOverlay::with_focus(
                                    bounds,
                                    1.,
                                    service,
                                    crate::capture::intent::CaptureIntent::Screenshot,
                                    cx.focus_handle(),
                                ).with_screen_picker())
                            }).expect("open display picker probe");
                            println!("isolated display picker opened without capture");
                            return;
                        }
                        if matches!(command.as_str(), "display-hover" | "display-exit" | "display-status") {
                            let Some(handle) = cx.windows().into_iter().find_map(|window| window.downcast::<AreaOverlay>()) else {
                                return;
                            };
                            if command == "display-status" {
                                let _ = handle.update(cx, |view, window, _| {
                                    println!("display picker: pointer={}, window active={}", view.parity_pointer_inside(), window.is_window_active());
                                });
                                return;
                            }
                            let _ = cx.update_window(handle.into(), |_, window, cx| {
                                let event = if command == "display-hover" {
                                    PlatformInput::MouseMove(MouseMoveEvent {
                                        position: point(px(100.), px(100.)),
                                        pressed_button: None,
                                        modifiers: Modifiers::none(),
                                    })
                                } else {
                                    PlatformInput::MouseExited(MouseExitEvent {
                                        position: point(px(501.), px(100.)),
                                        pressed_button: None,
                                        modifiers: Modifiers::none(),
                                    })
                                };
                                window.dispatch_event(event, cx);
                                println!("{command}: window active={}", window.is_window_active());
                            });
                            return;
                        }
                        if command == "scroll-open" {
                            let handle = crate::windows::scroll_capture::open_native_control_probe(cx);
                            let _ = cx.update_window(handle, |_, window, _| {
                                use cocoa::base::id;
                                use objc::{msg_send, sel, sel_impl};
                                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                                window.set_window_title("Poratake scroll probe");
                                if normal_level {
                                    let RawWindowHandle::AppKit(appkit) = HasWindowHandle::window_handle(window).unwrap().as_raw() else {
                                        panic!("expected an AppKit window");
                                    };
                                    let view = appkit.ns_view.as_ptr() as id;
                                    let native: id = unsafe { msg_send![view, window] };
                                    let _: () = unsafe { msg_send![native, setLevel: 0isize] };
                                }
                            });
                            println!("isolated scroll controls opened without starting capture");
                            return;
                        }
                        if matches!(command.as_str(), "scroll-hover" | "scroll-exit" | "scroll-tooltip-status") {
                            let Some(handle) = crate::windows::scroll_capture::native_control_for_probe(cx) else {
                                return;
                            };
                            let _ = cx.update_window(handle, |_, window, cx| {
                                if command == "scroll-tooltip-status" {
                                    println!("scroll native tooltip: {}", probe_tooltip(window).as_deref().unwrap_or("none"));
                                    return;
                                }
                                let event = if command == "scroll-hover" {
                                    PlatformInput::MouseMove(MouseMoveEvent {
                                        position: point(px(50.), px(26.)),
                                        pressed_button: None,
                                        modifiers: Modifiers::none(),
                                    })
                                } else {
                                    PlatformInput::MouseExited(MouseExitEvent {
                                        position: point(px(169.), px(26.)),
                                        pressed_button: None,
                                        modifiers: Modifiers::none(),
                                    })
                                };
                                window.dispatch_event(event, cx);
                                println!("{command}: window active={}", window.is_window_active());
                            });
                            return;
                        }
                        if matches!(command.as_str(), "preview-under-pointer" | "preview-close-under-pointer" | "preview-away") {
                            let Some(handle) = registry::handle(WindowKind::CapturePreview, cx) else {
                                return;
                            };
                            let _ = cx.update_window(handle, |_, window, cx| {
                                use cocoa::base::id;
                                use cocoa::foundation::{NSPoint, NSRect};
                                use objc::{class, msg_send, sel, sel_impl};
                                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                                let RawWindowHandle::AppKit(appkit) = HasWindowHandle::window_handle(window).unwrap().as_raw() else {
                                    panic!("expected an AppKit window");
                                };
                                let view = appkit.ns_view.as_ptr() as id;
                                let native: id = unsafe { msg_send![view, window] };
                                let frame: NSRect = unsafe { msg_send![native, frame] };
                                let mouse: NSPoint = unsafe { msg_send![class!(NSEvent), mouseLocation] };
                                let origin = match command.as_str() {
                                    "preview-under-pointer" => NSPoint::new(mouse.x - 100., mouse.y - frame.size.height + 60.),
                                    "preview-close-under-pointer" => NSPoint::new(mouse.x - 25., mouse.y - frame.size.height + 25.),
                                    _ => NSPoint::new(mouse.x + 300., mouse.y - frame.size.height + 60.),
                                };
                                let _: () = unsafe { msg_send![native, setFrameOrigin: origin] };
                                let _: () = unsafe { msg_send![native, orderFrontRegardless] };
                                window.bounds_changed(cx);
                                println!("isolated preview repositioned: {command}");
                            });
                            return;
                        }
                        if command == "hover-status" {
                            let Some(handle) = registry::handle(WindowKind::CapturePreview, cx)
                                .and_then(|handle| handle.downcast::<crate::windows::capture_preview::CapturePreviewWindow>()) else {
                                println!("preview is closed");
                                return;
                            };
                            let _ = handle.update(cx, |view, window, _| {
                                use cocoa::base::id;
                                use cocoa::foundation::NSPoint;
                                use objc::{class, msg_send, sel, sel_impl};
                                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                                let RawWindowHandle::AppKit(appkit) = HasWindowHandle::window_handle(window).unwrap().as_raw() else {
                                    panic!("expected an AppKit window");
                                };
                                let native_view = appkit.ns_view.as_ptr() as id;
                                let native: id = unsafe { msg_send![native_view, window] };
                                let mouse: NSPoint = unsafe { msg_send![class!(NSEvent), mouseLocation] };
                                let window_number: isize = unsafe { msg_send![native, windowNumber] };
                                let top_number: isize = unsafe { msg_send![class!(NSWindow), windowNumberAtPoint: mouse belowWindowWithWindowNumber: 0isize] };
                                println!("preview at pointer: window={window_number}, top={top_number}");
                                println!("preview hover status: card={}, pointer={}, window={}", view.parity_hovered(), view.parity_pointer_inside(), window.is_window_hovered());
                            });
                            return;
                        }
                        if command == "hover-exit" {
                            let Some(handle) = registry::handle(WindowKind::CapturePreview, cx) else {
                                return;
                            };
                            let _ = cx.update_window(handle, |_, window, cx| {
                                window.dispatch_event(PlatformInput::MouseExited(MouseExitEvent {
                                    position: point(px(210.), px(60.)),
                                    pressed_button: None,
                                    modifiers: Modifiers::none(),
                                }), cx);
                            });
                            println!("synthetic preview exit dispatched");
                            return;
                        }
                        if command != "edit" {
                            return;
                        }
                        let Some(handle) = registry::handle(WindowKind::CapturePreview, cx) else {
                            return;
                        };
                        let event = PlatformInput::MouseDown(MouseDownEvent {
                            position: point(px(100.), px(60.)),
                            button: MouseButton::Left,
                            click_count: 2,
                            modifiers: Modifiers::none(),
                            first_mouse: false,
                        });
                        let _ = cx.update_window(handle, |_, window, cx| window.dispatch_event(event, cx));
                        println!("synthetic preview double-click dispatched");
                    });
                }
            })
            .detach();
        });
    std::fs::remove_dir_all(directory).unwrap();
}

pub fn open(cx: &mut App) {
    let rect: Vec<i32> = std::env::var("PORATAKE_NATIVE_SMOKE_RECT")
        .expect("PORATAKE_NATIVE_SMOKE_RECT")
        .split(',')
        .map(|value| value.parse().expect("invalid capture rectangle"))
        .collect();
    assert!(rect.len() == 4 && rect[2] > 0 && rect[3] > 0);
    let output = PathBuf::from(
        std::env::var_os("PORATAKE_NATIVE_SMOKE_OUTPUT").expect("PORATAKE_NATIVE_SMOKE_OUTPUT"),
    );
    assert!(output.is_absolute() && output.is_dir());
    let config_dir = PathBuf::from(
        std::env::var_os(crate::config::store::CONFIG_DIR_ENV).expect("config directory"),
    );
    assert!(config_dir.is_absolute() && config_dir.is_dir());
    assert!(std::env::var_os("PORATAKE_DAEMON_PATH").is_some());
    let target = match std::env::var("PORATAKE_NATIVE_SMOKE_TARGET").as_deref() {
        Ok("screen") => RecordingTarget::Screen,
        Ok("window") => RecordingTarget::Window,
        Ok("area") | Err(std::env::VarError::NotPresent) => RecordingTarget::Area,
        _ => panic!("invalid smoke target"),
    };
    let window_id = (target == RecordingTarget::Window).then(|| {
        std::env::var("PORATAKE_NATIVE_SMOKE_WINDOW")
            .expect("PORATAKE_NATIVE_SMOKE_WINDOW")
            .parse::<i64>()
            .expect("invalid target window")
    });
    let config = crate::state::state(cx).config;
    config.update(|config| {
        config.storage.recordings_path = output.to_string_lossy().into_owned();
        config.storage.screenshots_path = output.to_string_lossy().into_owned();
        config.screenshot.capture_to_clipboard = false;
        config.screenshot.auto_copy_to_clipboard = false;
        config.screenshot.hide_desktop_icons = false;
        config.screenshot.freeze_screen =
            std::env::var("PORATAKE_NATIVE_SMOKE_FREEZE").as_deref() == Ok("1");
        config.screenshot.show_preview = true;
        config.screenshot.format = "png".into();
        config.recording.start_delay = std::env::var("PORATAKE_NATIVE_SMOKE_START_DELAY")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0.);
        config.recording.frame_rate = 10;
        config.recording.system_audio =
            std::env::var("PORATAKE_NATIVE_SMOKE_SYSTEM_AUDIO").as_deref() == Ok("1");
        config.recording.mic_enabled = false;
        config.recording.camera.enabled = false;
        config.recording.auto_zoom = false;
        config.recording.show_preview = true;
        config.history.enabled = false;
        config.preview.corner = "bottom-right".into();
        config.preview.display_id = None;
        config.preview.auto_dismiss = false;
    });
    if let Some(window_id) = window_id {
        assert!(crate::capture::overlay::show_window_recording_outline(
            window_id, cx
        ));
    }
    RecordingControl::open(
        cx,
        target,
        ScreenRect {
            x: rect[0],
            y: rect[1],
            width: rect[2],
            height: rect[3],
        },
        None,
        window_id,
        window_id.map(|_| "Native smoke window".to_string()),
        PreferenceScope::Global,
    );
    let (tx, rx) = smol::channel::unbounded::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if tx.send_blocking(line).is_err() {
                break;
            }
        }
    });
    cx.spawn(async move |cx| {
        while let Ok(command) = rx.recv().await {
            let result = cx.update(|cx| {
                #[cfg(not(test))]
                if command == "snapshot" {
                    let Some(handle) = cx
                        .windows()
                        .into_iter()
                        .find(|window| window.downcast::<AreaOverlay>().is_some())
                    else {
                        return false;
                    };
                    let Some(id) = cx
                        .update_window(handle, |_, window, _| {
                            crate::windows::macos_window_id(window)
                        })
                        .ok()
                        .flatten()
                    else {
                        return false;
                    };
                    let capture = crate::capture::DisplayCapture::new(
                        ScreenRect {
                            x: rect[0],
                            y: rect[1],
                            width: rect[2],
                            height: rect[3],
                        },
                        2.0,
                        poratake_daemon_common::geometry::DisplayOrigin { x: 0, y: 0 },
                        None,
                    );
                    return crate::state::state(cx)
                        .capture_area_to_file_with_options(
                            capture,
                            &output.join("active-selector.png"),
                            false,
                            vec![id],
                        )
                        .is_ok();
                }
                if let Some(kind) = command.strip_prefix("capture ") {
                    let intent = match kind {
                        "area" => Intent::CaptureArea,
                        "window" => Intent::CaptureWindow,
                        "timer" => Intent::TimerCapture,
                        _ => return false,
                    };
                    RecordingControl::cancel_pre_recording(cx);
                    crate::intents::dispatch(intent, None, cx);
                    return true;
                }
                if command == "cancel overlay" {
                    let Some(handle) = crate::capture::overlay::parity_active_overlay(cx) else {
                        return false;
                    };
                    return handle
                        .update(cx, |_, window, cx| {
                            crate::capture::overlay::dismiss(window, cx)
                        })
                        .is_ok();
                }
                if command == "preview ready" {
                    let Some(handle) =
                        registry::handle(WindowKind::CapturePreview, cx).and_then(|handle| {
                            handle
                                .downcast::<crate::windows::capture_preview::CapturePreviewWindow>()
                        })
                    else {
                        return false;
                    };
                    return handle
                        .update(cx, |preview, _, _| preview.thumbnails_ready_for_parity())
                        .unwrap_or(false);
                }
                if command == "video preview ready" {
                    let Some(handle) =
                        registry::handle(WindowKind::CapturePreview, cx).and_then(|handle| {
                            handle
                                .downcast::<crate::windows::capture_preview::CapturePreviewWindow>()
                        })
                    else {
                        return false;
                    };
                    return handle
                        .update(cx, |preview, _, _| {
                            preview
                                .video_thumbnail_status_for_parity()
                                .is_some_and(|(_, image, _)| image)
                        })
                        .unwrap_or(false);
                }
                if command == "video editor ready" {
                    let Some(handle) =
                        registry::handle(WindowKind::VideoEditor, cx).and_then(|handle| {
                            handle.downcast::<crate::windows::video_editor::VideoEditorWindow>()
                        })
                    else {
                        return false;
                    };
                    return handle
                        .update(cx, |editor, _, _| editor.preview_image().is_some())
                        .unwrap_or(false);
                }
                if command == "preview editor" {
                    let Some(handle) = registry::handle(WindowKind::CapturePreview, cx) else {
                        return false;
                    };
                    let event = PlatformInput::MouseDown(MouseDownEvent {
                        position: point(px(100.), px(60.)),
                        button: MouseButton::Left,
                        click_count: 2,
                        modifiers: Modifiers::none(),
                        first_mouse: false,
                    });
                    return cx
                        .update_window(handle, |_, window, cx| window.dispatch_event(event, cx))
                        .is_ok();
                }
                if command == "close editor" {
                    let Some(handle) = cx
                        .windows()
                        .into_iter()
                        .find(|handle| handle.downcast::<crate::editor::EditorWindow>().is_some())
                    else {
                        return false;
                    };
                    return handle
                        .update(cx, |_, window, _| window.remove_window())
                        .is_ok();
                }
                if command == "quit" {
                    cx.quit();
                    return true;
                }
                if let Some(input) = command.strip_prefix("pointer ") {
                    let parts: Vec<_> = input.split_whitespace().collect();
                    if parts.len() != 3 {
                        return false;
                    }
                    let (Ok(x), Ok(y)) = (parts[1].parse::<f32>(), parts[2].parse::<f32>()) else {
                        return false;
                    };
                    let position = point(px(x), px(y));
                    let event = match parts[0] {
                        "down" => PlatformInput::MouseDown(MouseDownEvent {
                            position,
                            button: MouseButton::Left,
                            click_count: 1,
                            modifiers: Modifiers::none(),
                            first_mouse: false,
                        }),
                        "move" => PlatformInput::MouseMove(MouseMoveEvent {
                            position,
                            pressed_button: Some(MouseButton::Left),
                            modifiers: Modifiers::none(),
                        }),
                        "up" => PlatformInput::MouseUp(MouseUpEvent {
                            position,
                            button: MouseButton::Left,
                            click_count: 1,
                            modifiers: Modifiers::none(),
                        }),
                        _ => return false,
                    };
                    let Some(handle) = cx
                        .windows()
                        .into_iter()
                        .find(|window| window.downcast::<AreaOverlay>().is_some())
                    else {
                        return false;
                    };
                    return cx
                        .update_window(handle, |_, window, cx| window.dispatch_event(event, cx))
                        .is_ok();
                }
                if command == "status" {
                    return recorder::is_recording();
                }
                let Some(handle) = registry::handle(WindowKind::RecordingControl, cx)
                    .and_then(|handle| handle.downcast::<RecordingControl>())
                else {
                    return false;
                };
                match command.as_str() {
                    "start" | "start-stall" => {
                        if command == "start-stall" {
                            assert_eq!(
                                std::env::var("PORATAKE_NATIVE_SMOKE_START_DELAY").as_deref(),
                                Ok("5")
                            );
                        }
                        handle
                            .update(cx, |control, window, cx| {
                                control.start(window, cx);
                                if command == "start-stall" {
                                    std::thread::sleep(std::time::Duration::from_millis(2250));
                                }
                                recorder::is_recording()
                            })
                            .unwrap_or(false)
                    }
                    "stop" => handle
                        .update(cx, |control, window, cx| control.finish(false, window, cx))
                        .unwrap_or(false),
                    _ => false,
                }
            });
            println!("NATIVE_SMOKE {} {}", command, result);
        }
    })
    .detach();
    println!("NATIVE_SMOKE ready");
}
