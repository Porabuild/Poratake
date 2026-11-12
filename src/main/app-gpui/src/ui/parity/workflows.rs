use gpui::{AppContext, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput};
use herogpui::gpui;
use poratake_daemon_common::geometry::CaptureRect;

use crate::editor::window::EditorWindow;
use crate::history_store::{HistoryItem, HistoryItemType};

use super::{
    capture, click_measured, event, hover, point, px, settle, size, HeadlessAppContext, Modifiers,
    Path,
};

pub(super) const SELECTED_RECT: CaptureRect = CaptureRect {
    x: 80,
    y: 90,
    width: 640,
    height: 360,
};

pub(super) fn editor_sizes(cx: &mut HeadlessAppContext, scratch: &Path) {
    let mut expected = None;
    for (index, width, height) in [(0, 1800, 900), (1, 320, 240), (2, 2200, 1200)] {
        let path = scratch.join(format!("editor-size-{index}.png"));
        image::RgbaImage::from_pixel(width, height, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        assert!(cx.update(|cx| crate::open_editor_for(cx, path.to_str().unwrap())));
        let editor = cx.update(|cx| {
            cx.windows()
                .into_iter()
                .find(|handle| handle.downcast::<EditorWindow>().is_some())
                .unwrap()
        });
        settle(cx, editor);
        let viewport = cx
            .update_window(editor, |_, window, _| window.viewport_size())
            .unwrap();
        let actual = (f32::from(viewport.width), f32::from(viewport.height));
        assert_eq!(
            crate::config::window_state::get(crate::config::window_state::SCREENSHOT_EDITOR),
            Some(actual),
            "the first editor size must be remembered without requiring a resize"
        );
        if let Some(expected) = expected {
            assert_eq!(
                actual, expected,
                "opening another image must retain the editor size"
            );
        }
        expected = Some(actual);
        if index == 1 {
            cx.update_window(editor, |_, window, _| {
                window.resize(size(px(1200.), px(800.)))
            })
            .unwrap();
            settle(cx, editor);
            expected = Some((1200., 800.));
        }
        cx.update_window(editor, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
    println!("editor initial and resized dimensions survive reopening different image sizes");
}

pub(super) fn area_picker(cx: &mut HeadlessAppContext) -> gpui::AnyWindowHandle {
    let picker = cx.update(|cx| crate::capture::overlay::parity_active_overlay(cx).unwrap());
    settle(cx, picker);
    picker
}

pub(super) fn select_area(cx: &mut HeadlessAppContext, picker: gpui::AnyWindowHandle) {
    let scale = cx
        .update_window(picker, |_, window, _| window.scale_factor())
        .unwrap();
    assert_eq!(scale, 2.);
    for input in [
        PlatformInput::MouseDown(MouseDownEvent {
            position: point(px(80. / scale), px(90. / scale)),
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
        PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(700. / scale), px(430. / scale)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        }),
        PlatformInput::MouseUp(MouseUpEvent {
            position: point(px(720. / scale), px(450. / scale)),
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    ] {
        event(cx, picker, input);
    }
}

pub(super) fn recording_preference_handoffs(cx: &mut HeadlessAppContext, output: &Path) {
    use crate::windows::registry::{self, WindowKind};

    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({"freeze": "frozen"})).unwrap(),
    )
    .unwrap();
    for target in ["area", "window", "screen"] {
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.recording.system_audio = false;
                config.recording.mic_enabled = false;
                config.recording.camera.enabled = false;
                config.all_in_one.remember_choices = true;
                config.all_in_one.last_mode = "record".into();
                config.all_in_one.last_targets.record = target.into();
                config.all_in_one.recording.system_audio = true;
                config.all_in_one.recording.mic_enabled = false;
                config.all_in_one.recording.camera_enabled = false;
            });
            crate::capture::start_all_in_one(cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        if target == "area" {
            select_area(cx, picker);
        } else {
            event(
                cx,
                picker,
                PlatformInput::MouseDown(MouseDownEvent {
                    position: point(px(240.), px(180.)),
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers: Modifiers::none(),
                    first_mouse: false,
                }),
            );
        }
        cx.run_until_parked();
        let control = cx
            .update(|cx| registry::handle(WindowKind::RecordingControl, cx))
            .unwrap_or_else(|| panic!("{target} selection did not open recording controls"));
        settle(cx, control);
        let audio = if target == "window" { 6 } else { 4 };
        click_measured(cx, control, "recording-controls", audio);
        cx.update(|cx| {
            let config = crate::state::state(cx).config.get();
            assert!(
                !config.recording.system_audio,
                "{target} changed global preferences"
            );
            assert!(
                !config.all_in_one.recording.system_audio,
                "{target} did not update all-in-one preferences"
            );
        });
        if target == "area" {
            for input in [
                PlatformInput::MouseDown(MouseDownEvent {
                    position: point(px(200.), px(150.)),
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers: Modifiers::none(),
                    first_mouse: false,
                }),
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: point(px(210.), px(160.)),
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Modifiers::none(),
                }),
                PlatformInput::MouseUp(MouseUpEvent {
                    position: point(px(210.), px(160.)),
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers: Modifiers::none(),
                }),
            ] {
                event(cx, picker, input);
            }
            settle(cx, control);
            click_measured(cx, control, "recording-controls", audio);
            cx.update(|cx| {
                let config = crate::state::state(cx).config.get();
                assert!(!config.recording.system_audio);
                assert!(config.all_in_one.recording.system_audio);
            });
        }
        cx.update(|cx| {
            assert!(crate::windows::recording_control::RecordingControl::cancel_pre_recording(cx));
            crate::capture::overlay::close_all(cx);
        });
        cx.run_until_parked();
    }
}

pub(super) fn selection_captures(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path) {
    let display = CaptureRect {
        x: 0,
        y: 0,
        width: 3840,
        height: 2160,
    };
    for (kind, start, rect) in [
        (
            "window",
            crate::capture::start_window_capture as fn(&mut gpui::App),
            SELECTED_RECT,
        ),
        (
            "area",
            crate::capture::start_area_capture as fn(&mut gpui::App),
            SELECTED_RECT,
        ),
        (
            "display",
            crate::capture::start_all_in_one as fn(&mut gpui::App),
            display,
        ),
    ] {
        for (name, freeze, color) in [
            ("live", false, [240, 80, 64, 255]),
            ("frozen", true, [64, 112, 192, 255]),
            ("freeze-failed", true, [240, 80, 64, 255]),
            ("freeze-unavailable", true, [240, 80, 64, 255]),
        ] {
            let captures = scratch.join(format!("{kind}-{name}"));
            std::fs::create_dir_all(&captures).unwrap();
            let sentinel = format!("clipboard before {kind}-{name}");
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&serde_json::json!({
                    "freeze": name,
                    "rect": rect,
                    "windowId": (kind == "window").then_some(43),
                }))
                .unwrap(),
            )
            .unwrap();
            cx.update(|cx| {
                crate::state::state(cx).config.update(|config| {
                    config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                    config.screenshot.freeze_screen = freeze;
                    config.screenshot.show_preview = true;
                    config.preview.corner = "bottom-right".into();
                    config.screenshot.hide_desktop_icons = false;
                    config.screenshot.capture_to_clipboard = false;
                    config.screenshot.auto_copy_to_clipboard = kind == "area";
                    config.all_in_one.remember_choices = true;
                    config.all_in_one.last_mode = "screenshot".into();
                    config.all_in_one.last_targets.screenshot = "screen".into();
                });
                crate::system::clipboard::ClipboardService::write_text(cx, sentinel.clone());
                start(cx);
            });
            cx.run_until_parked();
            let picker = area_picker(cx);
            if kind == "area" {
                select_area(cx, picker);
            } else {
                event(
                    cx,
                    picker,
                    PlatformInput::MouseDown(MouseDownEvent {
                        position: point(px(240.), px(180.)),
                        button: MouseButton::Left,
                        click_count: 1,
                        modifiers: Modifiers::none(),
                        first_mouse: false,
                    }),
                );
            }
            cx.run_until_parked();
            verify_saved_capture(
                cx,
                output,
                &captures,
                (rect, color),
                &format!("workflow-{kind}-{name}"),
            );
            if kind == "area" {
                let path = std::fs::read_dir(&captures)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                let clipboard = cx.update(|cx| {
                    crate::system::clipboard::ClipboardService::read_image(cx).unwrap()
                });
                assert_eq!(
                    image::load_from_memory(&clipboard.bytes)
                        .unwrap()
                        .to_rgba8(),
                    image::open(path).unwrap().to_rgba8(),
                    "{kind}-{name}: the copied pixels must match the saved capture"
                );
            } else {
                assert_eq!(
                    cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
                    Some(sentinel),
                    "{kind}-{name}: disabled automatic copy must not change the clipboard"
                );
            }
        }
    }
    let calls: Vec<serde_json::Value> =
        std::fs::read_to_string(output.join("daemon-fixture.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .filter(|call: &serde_json::Value| call["module"] == "screenshot")
            .collect();
    assert_eq!(
        calls
            .iter()
            .map(|call| call["method"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "capture-window",
            "capture-area",
            "capture-window",
            "capture-window",
            "capture-area",
            "capture-area",
            "capture-area",
            "capture-area",
            "capture-area",
            "capture-area",
            "capture-area",
            "capture-area",
        ]
    );

    let captures = scratch.join("capture-collision");
    std::fs::create_dir_all(&captures).unwrap();
    let existing = captures.join("Collision.png");
    std::fs::write(&existing, b"previous capture").unwrap();
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "live", "rect": SELECTED_RECT, "windowId": null,
        }))
        .unwrap(),
    )
    .unwrap();
    let pattern = cx.update(|cx| {
        let state = crate::state::state(cx);
        let pattern = state.config.get().storage.naming_pattern;
        state.config.update(|config| {
            config.storage.screenshots_path = captures.to_string_lossy().into_owned();
            config.storage.naming_pattern = "Collision".into();
            config.screenshot.freeze_screen = false;
            config.screenshot.show_preview = true;
            config.screenshot.capture_to_clipboard = false;
            config.screenshot.auto_copy_to_clipboard = false;
        });
        crate::capture::start_area_capture(cx);
        pattern
    });
    cx.run_until_parked();
    let picker = area_picker(cx);
    select_area(cx, picker);
    cx.run_until_parked();
    assert_eq!(std::fs::read(&existing).unwrap(), b"previous capture");
    assert!(captures.join("Collision (1).png").is_file());
    std::fs::remove_file(&existing).unwrap();
    verify_saved_capture(
        cx,
        scratch,
        &captures,
        (SELECTED_RECT, [240, 80, 64, 255]),
        "workflow-capture-collision",
    );
    std::fs::write(&existing, b"previous capture").unwrap();
    let clipboard_pixels = [70, 120, 180, 255].repeat(20 * 10);
    cx.update(|cx| {
        crate::system::clipboard::ClipboardService::write_rgba(
            cx,
            20,
            10,
            clipboard_pixels.clone(),
        )
        .unwrap();
        crate::editor::open_clipboard(cx);
    });
    cx.run_until_parked();
    assert_eq!(std::fs::read(&existing).unwrap(), b"previous capture");
    let clipboard_path = captures.join("Collision (2).png");
    assert_eq!(
        image::open(&clipboard_path).unwrap().to_rgba8().into_raw(),
        clipboard_pixels
    );
    let editor = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .filter_map(|window| window.downcast::<EditorWindow>())
            .find(|editor| Path::new(&editor.read(cx).unwrap().file_path) == clipboard_path)
            .unwrap()
    });
    cx.update_window(editor.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        crate::state::state(cx)
            .config
            .update(|config| config.storage.naming_pattern = pattern);
    });

    let captures = scratch.join("clipboard-only");
    std::fs::create_dir_all(&captures).unwrap();
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "live", "rect": SELECTED_RECT, "windowId": null,
        }))
        .unwrap(),
    )
    .unwrap();
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.screenshots_path = captures.to_string_lossy().into_owned();
            config.screenshot.freeze_screen = false;
            config.screenshot.show_preview = false;
            config.screenshot.capture_to_clipboard = true;
            config.screenshot.auto_copy_to_clipboard = false;
        });
        crate::system::clipboard::ClipboardService::write_text(cx, "before capture".into());
        crate::capture::start_area_capture(cx);
    });
    cx.run_until_parked();
    let picker = area_picker(cx);
    select_area(cx, picker);
    cx.run_until_parked();
    let files: Vec<_> = std::fs::read_dir(&captures)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1, "clipboard-only capture must be saved");
    let path = &files[0];
    let copied = cx
        .update(|cx| crate::system::clipboard::ClipboardService::read_image(cx))
        .expect("clipboard-only capture must replace the clipboard with an image");
    let saved = image::open(path).unwrap().to_rgba8();
    assert_eq!(saved.dimensions(), (640, 360));
    assert!(saved.pixels().all(|pixel| pixel.0 == [240, 80, 64, 255]));
    assert_eq!(
        image::load_from_memory(&copied.bytes).unwrap().to_rgba8(),
        saved
    );
    assert!(crate::history_store::load_history()
        .iter()
        .any(|item| Path::new(&item.original_path) == path));
    assert!(cx.update(|cx| {
        crate::windows::registry::handle(crate::windows::registry::WindowKind::CapturePreview, cx)
            .is_none()
            && cx
                .windows()
                .into_iter()
                .filter_map(|window| window.downcast::<EditorWindow>())
                .all(|editor| Path::new(&editor.read(cx).unwrap().file_path) != path)
    }));
    let captures = scratch.join("screen-replaces-frozen-selection");
    std::fs::create_dir_all(&captures).unwrap();
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "frozen", "rect": display, "windowId": null,
        }))
        .unwrap(),
    )
    .unwrap();
    let before = super::daemon_fixture::calls(output).len();
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.screenshots_path = captures.to_string_lossy().into_owned();
            config.screenshot.freeze_screen = true;
        });
        crate::capture::start_area_capture(cx);
    });
    cx.run_until_parked();
    assert!(cx.update(crate::capture::overlay::parity_is_active));
    cx.update(crate::capture::start_screen_capture);
    cx.run_until_parked();
    assert!(!cx.update(crate::capture::overlay::parity_is_active));
    let files: Vec<_> = std::fs::read_dir(&captures)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    let saved = image::open(&files[0]).unwrap().to_rgba8();
    assert_eq!(
        saved.dimensions(),
        (display.width as u32, display.height as u32)
    );
    assert!(saved.pixels().all(|pixel| pixel.0 == [240, 80, 64, 255]));
    let copied = cx
        .update(|cx| crate::system::clipboard::ClipboardService::read_image(cx))
        .unwrap();
    assert_eq!(
        image::load_from_memory(&copied.bytes).unwrap().to_rgba8(),
        saved
    );
    let calls = super::daemon_fixture::calls(output);
    let calls = &calls[before..];
    let release = calls
        .iter()
        .position(|call| call["module"] == "freeze-screen" && call["method"] == "release")
        .unwrap();
    let capture = calls
        .iter()
        .position(|call| call["module"] == "screenshot" && call["method"] == "capture-area")
        .unwrap();
    assert!(release < capture);
    assert_eq!(calls[capture]["params"]["cached"], false);
    cx.update(|cx| {
        crate::state::state(cx)
            .config
            .update(|config| config.recording.camera.enabled = false);
        crate::capture::start_area_capture(cx);
    });
    cx.run_until_parked();
    assert!(cx.update(crate::capture::overlay::parity_is_active));
    let before = super::daemon_fixture::calls(output).len();
    cx.update(|cx| {
        use crate::windows::registry::{self, WindowKind};

        crate::capture::start_screen_recording(cx);
        assert!(!crate::capture::overlay::parity_is_active(cx));
        assert!(!registry::is_open(WindowKind::RecordingControl, cx));
    });
    cx.run_until_parked();
    cx.update(|cx| {
        use crate::windows::registry::{self, WindowKind};

        assert!(registry::is_open(WindowKind::RecordingControl, cx));
        registry::close(WindowKind::RecordingControl, cx);
    });
    assert!(super::daemon_fixture::calls(output)[before..]
        .iter()
        .any(|call| call["module"] == "freeze-screen" && call["method"] == "release"));
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.screenshot.show_preview = true;
            config.screenshot.capture_to_clipboard = false;
        });
    });
    println!("selection capture clipboard modes and saved pixels passed");
}

#[cfg(windows)]
pub(super) fn scaled_selection_capture(
    cx: &mut HeadlessAppContext,
    output: &Path,
    scratch: &Path,
    scale: f32,
) {
    let expected = CaptureRect {
        x: -517,
        y: -103,
        width: 517,
        height: 383,
    };
    let captures = scratch.join("scaled-selection");
    std::fs::create_dir_all(&captures).unwrap();
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "live", "rect": expected, "windowId": null, "pattern": "quadrants",
        }))
        .unwrap(),
    )
    .unwrap();
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.screenshots_path = captures.to_string_lossy().into_owned();
            config.screenshot.freeze_screen = false;
            config.screenshot.show_preview = true;
            config.screenshot.hide_desktop_icons = false;
            config.screenshot.capture_to_clipboard = false;
            config.screenshot.auto_copy_to_clipboard = false;
            config.preview.corner = "bottom-right".into();
            config.preview.auto_dismiss = false;
        });
    });
    let bounds = gpui::Bounds::new(
        point(px(-640. / scale), px(-200. / scale)),
        size(px(640. / scale), px(480. / scale)),
    );
    let picker = cx
        .open_window(bounds.size, |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| {
                crate::capture::overlay::AreaOverlay::with_focus(
                    bounds,
                    scale,
                    crate::state::state(cx),
                    crate::capture::intent::CaptureIntent::Screenshot,
                    cx.focus_handle(),
                )
            })
        })
        .unwrap()
        .into();
    settle(cx, picker);
    let start = point(px(123. / scale), px(97. / scale));
    let end = point(px(639. / scale), px(479. / scale));
    event(
        cx,
        picker,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    event(
        cx,
        picker,
        PlatformInput::MouseMove(MouseMoveEvent {
            position: end,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, picker);
    assert!(
        cx.update_window(picker, |_, window, _| {
            window.painted_quads().iter().any(|quad| {
                [
                    (quad.bounds.left().0, 123.),
                    (quad.bounds.top().0, 97.),
                    (quad.bounds.right().0, 640.),
                    (quad.bounds.bottom().0, 480.),
                ]
                .iter()
                .all(|(actual, expected)| (actual - expected).abs() < 0.01)
            })
        })
        .unwrap(),
        "selection frame must reach the display edge at {scale}x"
    );
    event(
        cx,
        picker,
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    cx.run_until_parked();
    let calls = super::daemon_fixture::calls(output);
    let call = calls
        .iter()
        .rev()
        .find(|call| call["module"] == "screenshot" && call["method"] == "capture-area")
        .expect("the selected area must reach the screenshot daemon");
    for (key, value) in [
        ("x", expected.x),
        ("y", expected.y),
        ("width", expected.width),
        ("height", expected.height),
    ] {
        assert_eq!(call["params"][key], value, "capture {key} at {scale}x");
    }
    let files: Vec<_> = std::fs::read_dir(&captures)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    let image = image::open(&files[0]).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (517, 383));
    assert!(cx.update(|cx| cx
        .windows()
        .into_iter()
        .filter_map(|window| window.downcast::<EditorWindow>())
        .all(|editor| Path::new(&editor.read(cx).unwrap().file_path) != files[0])));
    let preview = cx
        .update(|cx| {
            crate::windows::registry::handle(
                crate::windows::registry::WindowKind::CapturePreview,
                cx,
            )
        })
        .expect("capture must open a preview");
    cx.update_window(preview, |_, window, _| window.set_scale_factor(scale))
        .unwrap();
    settle(cx, preview);
    let bounds = super::measured("capture-previews", 0);
    let rendered = cx.capture_screenshot(preview).unwrap();
    for (x, y) in [(1, 1), (3, 1), (1, 3), (3, 3)] {
        let saved = image
            .get_pixel(image.width() * x / 4, image.height() * y / 4)
            .0;
        assert_eq!(
            saved,
            [
                if x == 1 { 240 } else { 40 },
                if y == 1 { 80 } else { 180 },
                64,
                255
            ]
        );
        let preview_x = f32::from(bounds.left()) + f32::from(bounds.size.width) * x as f32 / 4.;
        let preview_y = f32::from(bounds.top()) + f32::from(bounds.size.height) * y as f32 / 4.;
        assert_eq!(
            rendered
                .get_pixel(
                    (preview_x * scale).round() as u32,
                    (preview_y * scale).round() as u32,
                )
                .0,
            saved,
            "preview must match the saved capture at {scale}x ({x}, {y})"
        );
    }
    cx.update(|cx| {
        crate::windows::registry::close(crate::windows::registry::WindowKind::CapturePreview, cx)
    });
    cx.run_until_parked();
    println!("scaled selection, saved crop and preview passed at {scale}x");
}

pub(super) fn verify_saved_capture(
    cx: &mut HeadlessAppContext,
    output: &Path,
    captures: &Path,
    expected: (CaptureRect, [u8; 4]),
    name: &str,
) {
    let files: Vec<_> = std::fs::read_dir(captures)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1, "{name}: exactly one capture must be saved");
    let path = &files[0];
    assert_eq!(
        path.parent().unwrap().canonicalize().unwrap(),
        captures.canonicalize().unwrap()
    );
    assert!(
        cx.update(|cx| {
            cx.windows()
                .into_iter()
                .filter_map(|window| window.downcast::<EditorWindow>())
                .all(|editor| Path::new(&editor.read(cx).unwrap().file_path) != path)
        }),
        "{name}: capturing must show a preview without opening an editor"
    );
    let preview = cx
        .update(|cx| {
            crate::windows::registry::handle(
                crate::windows::registry::WindowKind::CapturePreview,
                cx,
            )
        })
        .expect("capturing with previews enabled must open its preview");
    cx.update_window(preview, |_, window, _| window.set_scale_factor(1.))
        .unwrap();
    settle(cx, preview);
    let position = super::measured("capture-previews", 0).center();
    event(
        cx,
        preview,
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 2,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    if cx.update(|cx| cx.windows().contains(&preview)) {
        event(
            cx,
            preview,
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                button: MouseButton::Left,
                click_count: 2,
                modifiers: Modifiers::none(),
            }),
        );
    }
    cx.advance_clock(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    let editor = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .filter_map(|window| window.downcast::<EditorWindow>())
            .find(|editor| Path::new(&editor.read(cx).unwrap().file_path) == path)
            .unwrap()
    });
    let saved = image::open(path).unwrap().to_rgba8();
    assert_eq!(
        saved.dimensions(),
        (expected.0.width as u32, expected.0.height as u32)
    );
    assert!(
        saved.pixels().all(|pixel| pixel.0 == expected.1),
        "{name}: incorrect live or frozen pixels"
    );
    assert!(crate::history_store::load_history()
        .iter()
        .any(|item| Path::new(&item.original_path) == path));
    let preview = cx.update(|cx| editor.read(cx).unwrap().export_png().unwrap());
    assert_eq!(image::load_from_memory(&preview).unwrap().to_rgba8(), saved);
    let window = editor.into();
    cx.update_window(window, |_, window, _| {
        window.resize(size(px(1280.), px(738.)));
        window.set_scale_factor(1.);
    })
    .unwrap();
    settle(cx, window);
    hover(cx, window, 20., 700.);
    capture(cx, window, output, name);
    cx.update_window(window, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

pub(super) fn timer_captures(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path) {
    let rect = SELECTED_RECT;
    for name in ["completed", "cancelled", "timeout", "failed", "retry"] {
        let captures = scratch.join(format!("timer-{name}"));
        std::fs::create_dir_all(&captures).unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen", "rect": rect, "windowId": null, "timer": name,
            }))
            .unwrap(),
        )
        .unwrap();
        let before = std::fs::read_to_string(output.join("daemon-fixture.jsonl"))
            .unwrap()
            .lines()
            .count();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                config.screenshot.freeze_screen = name != "retry";
            });
            crate::capture::start_area_selection(crate::capture::intent::CaptureIntent::Timer, cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        select_area(cx, picker);
        cx.run_until_parked();
        if name == "timeout" {
            assert!(crate::capture::timer::is_active());
            cx.advance_clock(std::time::Duration::from_secs(10));
            cx.run_until_parked();
        }
        assert!(
            !crate::capture::timer::is_active(),
            "{name}: timer session must settle"
        );
        assert!(
            !cx.update(crate::capture::overlay::parity_is_active),
            "{name}: timer selector must be cleared"
        );
        let saved = matches!(name, "completed" | "retry");
        if saved {
            verify_saved_capture(
                cx,
                output,
                &captures,
                (rect, [240, 80, 64, 255]),
                &format!("workflow-timer-{name}"),
            );
        } else {
            assert_eq!(std::fs::read_dir(&captures).unwrap().count(), 0);
            assert!(!crate::history_store::load_history()
                .iter()
                .any(|item| Path::new(&item.original_path).parent() == Some(captures.as_path())));
        }
        let calls: Vec<serde_json::Value> =
            std::fs::read_to_string(output.join("daemon-fixture.jsonl"))
                .unwrap()
                .lines()
                .skip(before)
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
        assert_eq!(
            calls
                .iter()
                .filter(|call| call["module"] == "screenshot")
                .count(),
            usize::from(saved)
        );
        assert_eq!(
            calls
                .iter()
                .filter(|call| call["module"] == "timer-control" && call["method"] == "show")
                .count(),
            1
        );
        assert_eq!(
            calls
                .iter()
                .filter(|call| call["module"] == "timer-control" && call["method"] == "hide")
                .count(),
            usize::from(name != "failed")
        );
    }
}

fn check_redaction_draft(
    cx: &mut HeadlessAppContext,
    editor: gpui::WindowHandle<EditorWindow>,
    draft_x: f64,
    draft_y: f64,
) {
    let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
    let left = f32::from(bounds.left()).round() as u32;
    let top = f32::from(bounds.top()).round() as u32;
    let x = left + ((draft_x as f32 + 8.5) * 1.5).floor() as u32;
    let y = top + ((draft_y as f32 + 8.5) * 1.5).floor() as u32;
    let baseline = cx
        .capture_screenshot(editor.into())
        .unwrap()
        .get_pixel(x, y)
        .0;
    assert!(baseline[..3].iter().any(|channel| *channel > 40));
    for style in ["blur", "pixelate", "blackout"] {
        editor
            .update(cx, |editor, _, cx| {
                editor.draft = Some(crate::editor::annotations::Annotation::Redact {
                    id: "draft-over-redaction".into(),
                    x: draft_x,
                    y: draft_y,
                    width: 24.,
                    height: 16.,
                    style: style.into(),
                    intensity: 5.,
                });
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        let draft = cx
            .capture_screenshot(editor.into())
            .unwrap()
            .get_pixel(x, y)
            .0;
        let opacity = if style == "blackout" { 0. } else { 0.15 };
        for channel in 0..3 {
            let expected = (f32::from(baseline[channel]) * opacity).round() as u8;
            assert!(
                draft[channel].abs_diff(expected) <= 3,
                "{style} draft at {draft_x},{draft_y}: {draft:?} vs {baseline:?}"
            );
        }
        editor
            .update(cx, |editor, _, cx| {
                editor.draft = None;
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
    }
}

pub(super) fn simple_inset_preview(
    cx: &mut HeadlessAppContext,
    scratch: &Path,
    scale: f32,
    padding_setting: f64,
    with_background: bool,
    inset_setting: f64,
    clipped_blur: bool,
) {
    let source = scratch.join(format!(
        "simple-inset-{scale}-{padding_setting}-{with_background}-{inset_setting}-{clipped_blur}.png"
    ));
    image::RgbaImage::from_fn(100, 80, |x, y| {
        if (30..70).contains(&x) && (20..60).contains(&y) {
            image::Rgba([220, 70, 90, 128])
        } else {
            image::Rgba([40, 120, 180, 255])
        }
    })
    .save(&source)
    .unwrap();
    crate::history_store::add_item(
        HistoryItem {
            id: format!("simple-inset-{scale}-{padding_setting}-{with_background}-{inset_setting}-{clipped_blur}"),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: Some(serde_json::json!({"annotations": [
                crate::editor::annotations::Annotation::Rectangle {
                    id: "before-inset-blur".into(),
                    x: 37.,
                    y: 26.,
                    width: 16.,
                    height: 16.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: Some("#ff00ff".into()),
                },
                crate::editor::annotations::Annotation::Redact {
                    id: "inset-blur".into(),
                    x: if clipped_blur { -5. } else { 32. },
                    y: 22.,
                    width: if clipped_blur { 40. } else { 30. },
                    height: 30.,
                    style: "blur".into(),
                    intensity: 2.,
                }
            ]})),
            duration: None,
        },
        100,
        true,
    );
    let editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, editor.into());
    editor
        .update(cx, |editor, _, cx| {
            editor.wallpaper.set_gradient(with_background.then(|| {
                crate::editor::wallpaper::GradientOption {
                    id: "inset-background".into(),
                    colors: if scale == 1. && padding_setting == 20. && inset_setting == 8. {
                        vec!["#2468ac".into(), "#de7840".into()]
                    } else {
                        vec!["#2468ac".into()]
                    },
                    angle: 45.,
                }
            }));
            editor.wallpaper.padding = padding_setting;
            editor.wallpaper.inset = inset_setting;
            let mut annotations = editor.history.current().to_vec();
            annotations.push(crate::editor::annotations::Annotation::Rectangle {
                id: "inset-annotation".into(),
                x: if clipped_blur { 18. } else { 42. },
                y: 32.,
                width: 16.,
                height: 16.,
                stroke: "#00ff00".into(),
                stroke_width: 1.,
                fill: Some("#00ff00".into()),
            });
            annotations.push(crate::editor::annotations::Annotation::Redact {
                id: "after-inset-blackout".into(),
                x: if clipped_blur { 22. } else { 46. },
                y: 36.,
                width: 6.,
                height: 6.,
                style: "blackout".into(),
                intensity: 2.,
            });
            annotations.push(crate::editor::annotations::Annotation::Redact {
                id: "after-inset-pixelation".into(),
                x: if clipped_blur { 5. } else { 32. },
                y: 24.,
                width: 8.,
                height: 8.,
                style: "pixelate".into(),
                intensity: 3.,
            });
            editor.history.push(annotations);
            cx.notify();
        })
        .unwrap();
    settle(cx, editor.into());
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 1. / scale;
            cx.notify();
        })
        .unwrap();
    settle(cx, editor.into());
    let (bounds, png, matte) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let theme = crate::theme::vars::active_theme(cx);
        let theme = gpui::Rgba::from(
            if inset_setting == 0. && padding_setting == 0. && !with_background {
                theme.surface
            } else {
                theme.background
            },
        );
        (
            editor.bounds.borrow().unwrap(),
            editor.export_png().unwrap(),
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ]),
        )
    });
    let saved = image::load_from_memory(&png).unwrap().to_rgba8();
    let native_inset = (inset_setting as f32 * scale).round() as u32;
    let margin = (padding_setting as f32 * scale).round() as u32 + native_inset;
    assert_eq!(saved.dimensions(), (100 + margin * 2, 80 + margin * 2));
    let original = image::open(&source).unwrap().to_rgba8();
    let mut unblurred = image::RgbaImage::from_pixel(
        100,
        80,
        if native_inset > 0 {
            image::Rgba([40, 120, 180, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        },
    );
    image::imageops::overlay(&mut unblurred, &original, 0, 0);
    assert_ne!(
        saved.get_pixel(margin + 33, margin + 23),
        unblurred.get_pixel(33, 23)
    );
    assert_eq!(
        saved
            .get_pixel(margin + if clipped_blur { 24 } else { 48 }, margin + 38)
            .0,
        [0, 0, 0, 255]
    );
    if native_inset > 0 {
        assert_eq!(
            saved.get_pixel(margin - 2, margin + 10).0,
            [40, 120, 180, 255]
        );
    }
    let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
    image::imageops::overlay(&mut expected, &saved, 0, 0);
    let screenshot = cx.capture_screenshot(editor.into()).unwrap();
    let left = (f32::from(bounds.left()) * scale).round() as u32 - margin;
    let top = (f32::from(bounds.top()) * scale).round() as u32 - margin;
    for y in 0..saved.height() {
        for x in 0..saved.width() {
            let actual = screenshot.get_pixel(left + x, top + y).0;
            let expected = expected.get_pixel(x, y).0;
            assert!(
                actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                "inset preview at {scale}x, padding {padding_setting}, inset {inset_setting}, background {with_background}, clipped {clipped_blur}, {x},{y}: {actual:?} vs {expected:?}"
            );
        }
    }
    let flattened_path = scratch.join("simple-inset-blur-flattened.png");
    std::fs::write(&flattened_path, &png).unwrap();
    let flattened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(flattened_path.to_str().unwrap(), window, cx))
        })
        .unwrap();
    flattened
        .update(cx, |editor, _, cx| {
            editor
                .wallpaper
                .set_gradient(Some(crate::editor::wallpaper::GradientOption {
                    id: "zoom-matte".into(),
                    colors: vec![format!(
                        "#{:02x}{:02x}{:02x}",
                        matte.0[0], matte.0[1], matte.0[2]
                    )],
                    angle: 0.,
                }));
            editor.wallpaper.padding = 0.;
            cx.notify();
        })
        .unwrap();
    settle(cx, flattened.into());
    for effective_zoom in [0.5, 0.73, 1.37, 1.5] {
        let zoom = effective_zoom / scale;
        for window in [editor, flattened] {
            window
                .update(cx, |editor, _, cx| {
                    editor.zoom = zoom;
                    cx.notify();
                })
                .unwrap();
            settle(cx, window.into());
        }
        let (preview_bounds, flattened_bounds) = cx.update(|cx| {
            (
                editor.read(cx).unwrap().bounds.borrow().unwrap(),
                flattened.read(cx).unwrap().bounds.borrow().unwrap(),
            )
        });
        let preview = cx.capture_screenshot(editor.into()).unwrap();
        let flattened_image = cx.capture_screenshot(flattened.into()).unwrap();
        let snap = |value: f32| (value.abs() - 0.5).ceil().copysign(value) as u32;
        let margin_offset = snap(margin as f32 * zoom * scale);
        let left = snap(f32::from(preview_bounds.left()) * scale) - margin_offset;
        let top = snap(f32::from(preview_bounds.top()) * scale) - margin_offset;
        let flat_left = snap(f32::from(flattened_bounds.left()) * scale);
        let flat_top = snap(f32::from(flattened_bounds.top()) * scale);
        if scale == 1. && padding_setting == 0. && !with_background && inset_setting == 8. {
            let width = (saved.width() as f32 * effective_zoom).round() as u32;
            let height = (saved.height() as f32 * effective_zoom).round() as u32;
            image::imageops::crop_imm(&preview, left, top, width, height)
                .to_image()
                .save(scratch.join(format!(
                    "blur-zoom-{effective_zoom}-{clipped_blur}-preview.png"
                )))
                .unwrap();
            image::imageops::crop_imm(&flattened_image, flat_left, flat_top, width, height)
                .to_image()
                .save(scratch.join(format!(
                    "blur-zoom-{effective_zoom}-{clipped_blur}-saved.png"
                )))
                .unwrap();
        }
        let mut worst = (0, 0, 0, [0; 4], [0; 4]);
        let mut mismatched = 0;
        let (start_x, end_x) = if clipped_blur { (0, 36) } else { (31, 63) };
        for y in (((margin + 21) as f32 * effective_zoom).ceil() as u32)
            ..(((margin + 53) as f32 * effective_zoom).floor() as u32)
        {
            for x in (((margin + start_x) as f32 * effective_zoom).ceil() as u32)
                ..(((margin + end_x) as f32 * effective_zoom).floor() as u32)
            {
                let actual = preview.get_pixel(left + x, top + y).0;
                let expected = flattened_image.get_pixel(flat_left + x, flat_top + y).0;
                let difference = actual
                    .iter()
                    .zip(expected)
                    .map(|(a, b)| a.abs_diff(b))
                    .max()
                    .unwrap();
                if difference > 2 {
                    mismatched += 1;
                }
                if difference > worst.0 {
                    worst = (difference, x, y, actual, expected);
                }
            }
        }
        assert_eq!(
                mismatched, 0,
                "blur at {effective_zoom}x, DPI {scale}, padding {padding_setting}, background {with_background}, inset {inset_setting}, clipped {clipped_blur}: worst {worst:?}"
            );
    }
    cx.update_window(flattened.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    if scale == 1. && padding_setting == 20. && with_background && inset_setting == 8. {
        let annotations = cx.update(|cx| editor.read(cx).unwrap().history.current().to_vec());
        let mut without_redactions = annotations
            .iter()
            .filter(|annotation| {
                !matches!(
                    annotation,
                    crate::editor::annotations::Annotation::Redact { .. }
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        without_redactions.push(crate::editor::annotations::Annotation::Rectangle {
            id: "outside-image".into(),
            x: -6.,
            y: 24.,
            width: 12.,
            height: 12.,
            stroke: "#ff00ff".into(),
            stroke_width: 1.,
            fill: Some("#ff00ff".into()),
        });
        for (restored, draft_check, current) in [
            (false, false, without_redactions),
            (false, true, Vec::new()),
            (
                false,
                false,
                vec![crate::editor::annotations::Annotation::Redact {
                    id: "outside-image-blackout".into(),
                    x: -6.,
                    y: 24.,
                    width: 12.,
                    height: 12.,
                    style: "blackout".into(),
                    intensity: 2.,
                }],
            ),
            (true, false, annotations),
        ] {
            editor
                .update(cx, |editor, _, cx| {
                    editor.history.push(current);
                    editor.zoom = 1. / scale;
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            let (bounds, current_png) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                assert!(!editor.snapshot.borrow().backdrop_composite);
                (
                    editor.bounds.borrow().unwrap(),
                    editor.export_png().unwrap(),
                )
            });
            if restored {
                assert_eq!(current_png, png);
            }
            let saved = image::load_from_memory(&current_png).unwrap().to_rgba8();
            let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
            image::imageops::overlay(&mut expected, &saved, 0, 0);
            let screenshot = cx.capture_screenshot(editor.into()).unwrap();
            let left = (f32::from(bounds.left()) * scale).round() as u32 - margin;
            let top = (f32::from(bounds.top()) * scale).round() as u32 - margin;
            for y in 0..saved.height() {
                for x in 0..saved.width() {
                    let actual = screenshot.get_pixel(left + x, top + y).0;
                    let expected = expected.get_pixel(x, y).0;
                    assert!(
                        actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                        "redaction transition {restored} at {x},{y}: {actual:?} vs {expected:?}"
                    );
                }
            }
            if draft_check {
                for zoom in [1_f32, 1.5] {
                    editor
                        .update(cx, |editor, _, cx| {
                            editor.zoom = zoom / scale;
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, editor.into());
                    let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
                    let left =
                        (f32::from(bounds.left()) * scale - margin as f32 * zoom).round() as u32;
                    let top =
                        (f32::from(bounds.top()) * scale - margin as f32 * zoom).round() as u32;
                    let before = cx.capture_screenshot(editor.into()).unwrap();
                    editor
                        .update(cx, |editor, _, cx| {
                            editor.draft =
                                Some(crate::editor::annotations::Annotation::Rectangle {
                                    id: "draft-outside-image".into(),
                                    x: -6.,
                                    y: 24.,
                                    width: 12.,
                                    height: 12.,
                                    stroke: "#ff00ff".into(),
                                    stroke_width: 1.,
                                    fill: Some("#ff00ff".into()),
                                });
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, editor.into());
                    cx.update(|cx| {
                        assert!(
                            !editor
                                .read(cx)
                                .unwrap()
                                .snapshot
                                .borrow()
                                .backdrop_composite
                        );
                    });
                    let screenshot = cx.capture_screenshot(editor.into()).unwrap();
                    assert_eq!(
                        screenshot
                            .get_pixel(
                                left + ((margin - 3) as f32 * zoom).round() as u32,
                                top + ((margin + 30) as f32 * zoom).round() as u32
                            )
                            .0,
                        [255, 0, 255, 255]
                    );
                    let mut changed = 0;
                    let mut first = None;
                    for y in 0..(saved.height() as f32 * zoom).round() as u32 {
                        for x in 0..(saved.width() as f32 * zoom).round() as u32 {
                            if ((margin - 8) as f32 * zoom).floor() as u32 <= x
                                && x < ((margin + 8) as f32 * zoom).ceil() as u32
                                && ((margin + 22) as f32 * zoom).floor() as u32 <= y
                                && y < ((margin + 38) as f32 * zoom).ceil() as u32
                            {
                                continue;
                            }
                            let actual = screenshot.get_pixel(left + x, top + y).0;
                            let previous = before.get_pixel(left + x, top + y).0;
                            if actual.iter().zip(previous).any(|(a, b)| a.abs_diff(b) > 2) {
                                changed += 1;
                                first.get_or_insert((x, y, actual, previous));
                            }
                        }
                    }
                    assert_eq!(
                        changed, 0,
                        "draft at {zoom}x changed pixels outside its bounds: {first:?}"
                    );
                    editor
                        .update(cx, |editor, _, cx| {
                            editor.draft = None;
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, editor.into());
                }
            }
        }
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.aspect_ratio = "4:3".into();
                editor
                    .history
                    .push(vec![crate::editor::annotations::Annotation::Redact {
                        id: "fractional-offset-blur".into(),
                        x: -5.,
                        y: 22.,
                        width: 40.,
                        height: 30.,
                        style: "blur".into(),
                        intensity: 2.,
                    }]);
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        let (bounds, current_png, offset) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            let (_, (x, y, _, _)) = crate::editor::wallpaper::layout(
                &editor.wallpaper.for_scale(f64::from(scale)),
                100.,
                80.,
                f64::from(scale),
                false,
            );
            assert_ne!(x, x.round());
            (
                editor.bounds.borrow().unwrap(),
                editor.export_png().unwrap(),
                (x.round() as u32, y.round() as u32),
            )
        });
        let saved = image::load_from_memory(&current_png).unwrap().to_rgba8();
        let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
        image::imageops::overlay(&mut expected, &saved, 0, 0);
        let screenshot = cx.capture_screenshot(editor.into()).unwrap();
        let left = f32::from(bounds.left()).round() as u32 - offset.0;
        let top = f32::from(bounds.top()).round() as u32 - offset.1;
        for y in 0..saved.height() {
            for x in 0..saved.width() {
                let actual = screenshot.get_pixel(left + x, top + y).0;
                let expected = expected.get_pixel(x, y).0;
                assert!(
                    actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                    "fractional wallpaper offset at {x},{y}: {actual:?} vs {expected:?}"
                );
            }
        }
    }
    if scale == 1.
        && padding_setting == 0.
        && !with_background
        && inset_setting == 8.
        && !clipped_blur
    {
        check_redaction_draft(cx, editor, 36., 26.);
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.inset = 0.;
                editor.push_image_layer(
                    source.to_string_lossy().into_owned(),
                    100.,
                    80.,
                    crate::editor::layers::Edge::Right,
                    cx,
                );
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 1.5;
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        cx.update(|cx| {
            assert!(
                editor
                    .read(cx)
                    .unwrap()
                    .snapshot
                    .borrow()
                    .backdrop_composite
            );
        });
        check_redaction_draft(cx, editor, 36., 26.);
    }
    cx.update_window(editor.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

pub(super) fn balance_restoration(
    cx: &mut HeadlessAppContext,
    scratch: &Path,
    scale: f32,
    padding_setting: f64,
    with_background: bool,
    inset: f64,
) {
    let source = scratch.join(format!(
        "balance-restoration-{scale}-{padding_setting}-{with_background}-{inset}.png"
    ));
    let image = image::RgbaImage::from_fn(200, 160, |x, y| {
        if (100..120).contains(&x) && (85..105).contains(&y) {
            image::Rgba([64, 112, 192, 128])
        } else if (40..160).contains(&x) && (30..130).contains(&y) {
            image::Rgba([64, 112, 192, 255])
        } else if (5..195).contains(&x) && (5..155).contains(&y) {
            image::Rgba([230, 230, 230, 255])
        } else {
            image::Rgba([255, 255, 255, 255])
        }
    });
    image.save(&source).unwrap();
    let expected = crate::render::color_detection::ContentBounds {
        left: 30,
        top: 20,
        right: 30,
        bottom: 20,
    };
    assert_eq!(
        crate::editor::export::balance_bounds(&image),
        Some(expected)
    );
    crate::history_store::add_item(
        HistoryItem {
            id: format!("balance-restoration-{scale}-{padding_setting}-{with_background}-{inset}"),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: Some(serde_json::json!({"wallpaper": {
                "padding": padding_setting,
                "balance": true,
                "inset": inset,
                "gradient": with_background.then(|| serde_json::json!({
                    "id": "balance-background",
                    "colors": ["#2468ac"],
                    "angle": 0
                }))
            }})),
            duration: None,
        },
        100,
        true,
    );
    let editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, editor.into());
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert_eq!(
            editor.snapshot.borrow().balance_crop,
            Some((
                expected.left as f32,
                expected.top as f32,
                expected.right as f32,
                expected.bottom as f32,
            )),
            "reopening a balanced screenshot must restore its crop"
        );
    });
    let window = editor.into();
    editor
        .update(cx, |editor, _, cx| {
            editor.history.push(vec![
                crate::editor::annotations::Annotation::Rectangle {
                    id: "balance-border-annotation".into(),
                    x: 0.,
                    y: 50.,
                    width: 12.,
                    height: 30.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: Some("#ff00ff".into()),
                },
                crate::editor::annotations::Annotation::Rectangle {
                    id: "balance-content-annotation".into(),
                    x: 60.,
                    y: 60.,
                    width: 20.,
                    height: 20.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: Some("#ff00ff".into()),
                },
            ]);
            editor.tool = crate::ui::colors::Tool::Select;
            editor.zoom = 1. / scale;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let (bounds, png, backdrop, matte) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
        (
            editor.bounds.borrow().unwrap(),
            editor.export_png().unwrap(),
            editor.snapshot.borrow().backdrop.clone().unwrap(),
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ]),
        )
    });
    let saved = image::load_from_memory(&png).unwrap().to_rgba8();
    let padding = (padding_setting as f32 * scale).round() as u32;
    let native_inset = (inset as f32 * scale).round() as u32;
    let dimensions = (
        200 - expected.left - expected.right + (padding + native_inset) * 2,
        160 - expected.top - expected.bottom + (padding + native_inset) * 2,
    );
    assert_eq!(
        saved.dimensions(),
        dimensions,
        "annotations must not alter the balance crop"
    );
    if inset > 0. {
        assert_eq!(
            saved
                .get_pixel(padding + native_inset / 2, saved.height() / 2)
                .0,
            [230, 230, 230, 255],
            "the inset must use the balanced source edge"
        );
    }
    assert_eq!(
        (
            u32::from(backdrop.size(0).width),
            u32::from(backdrop.size(0).height)
        ),
        dimensions,
        "balanced preview and export must have matching canvas dimensions"
    );
    let mut composited = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
    image::imageops::overlay(&mut composited, &saved, 0, 0);
    let screenshot = cx.capture_screenshot(window).unwrap();
    let left =
        (f32::from(bounds.left()) * scale).round() as u32 + expected.left - padding - native_inset;
    let top =
        (f32::from(bounds.top()) * scale).round() as u32 + expected.top - padding - native_inset;
    for y in 0..saved.height() {
        for x in 0..saved.width() {
            let actual = screenshot.get_pixel(left + x, top + y).0;
            let expected = composited.get_pixel(x, y).0;
            assert!(
                actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                "balanced preview differs at {x},{y}: {actual:?} vs {expected:?}"
            );
        }
    }
    if inset > 0. {
        editor
            .update(cx, |editor, _, cx| {
                editor.tool = crate::ui::colors::Tool::Crop;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
        for input in [
            PlatformInput::MouseDown(MouseDownEvent {
                position: bounds.origin + point(px(20. / scale), px(10. / scale)),
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Modifiers::none(),
                first_mouse: false,
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                position: bounds.origin + point(px(180. / scale), px(150. / scale)),
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Modifiers::none(),
            }),
        ] {
            event(cx, window, input);
        }
        editor
            .update(cx, |editor, _, cx| editor.apply_crop(cx))
            .unwrap();
        settle(cx, window);
        cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            let refreshed = editor.snapshot.borrow().backdrop.clone().unwrap();
            assert_eq!(
                (
                    u32::from(refreshed.size(0).width),
                    u32::from(refreshed.size(0).height)
                ),
                dimensions
            );
            assert_ne!(
                refreshed.id, backdrop.id,
                "a new image must refresh the inset backdrop"
            );
        });
        cx.update_window(window, |_, window, cx| {
            window.dispatch_action(Box::new(crate::editor::actions::Undo), cx);
        })
        .unwrap();
        settle(cx, window);
        cx.update(|cx| assert_eq!(editor.read(cx).unwrap().export_png().unwrap(), png));
    }
    editor
        .update(cx, |editor, _, cx| {
            editor.tool = crate::ui::colors::Tool::Crop;
            editor.zoom = 1. / scale;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
    let start = bounds.origin + point(px(40. / scale), px(30. / scale));
    let end = bounds.origin + point(px(160. / scale), px(130. / scale));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    event(
        cx,
        window,
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    editor
        .update(cx, |editor, _, cx| editor.apply_crop(cx))
        .unwrap();
    settle(cx, window);
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let snapshot = editor.snapshot.borrow();
        assert_eq!((snapshot.image_width, snapshot.image_height), (120., 100.));
        assert_eq!(
            snapshot.balance_crop, None,
            "cropping must discard the old balance margins"
        );
        assert_eq!(
            editor.base_image.as_ref().unwrap().to_rgba8(),
            image::imageops::crop_imm(&image, 40, 30, 120, 100).to_image(),
            "the crop must preserve the selected source pixels"
        );
    });
    cx.update_window(window, |_, window, cx| {
        window.dispatch_action(Box::new(crate::editor::actions::Undo), cx);
    })
    .unwrap();
    settle(cx, window);
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let snapshot = editor.snapshot.borrow();
        assert_eq!((snapshot.image_width, snapshot.image_height), (200., 160.));
        assert_eq!(
            snapshot.balance_crop,
            Some((
                expected.left as f32,
                expected.top as f32,
                expected.right as f32,
                expected.bottom as f32
            )),
            "undo must restore the balance margins"
        );
        assert_eq!(editor.export_png().unwrap(), png);
    });
    cx.update_window(editor.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    let reopened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, reopened.into());
    cx.update(|cx| {
        let editor = reopened.read(cx).unwrap();
        assert_eq!(
            editor.snapshot.borrow().balance_crop,
            Some((
                expected.left as f32,
                expected.top as f32,
                expected.right as f32,
                expected.bottom as f32
            ))
        );
        assert_eq!(editor.export_png().unwrap(), png);
    });
    if inset == 0. {
        let attached = scratch.join("balance-attachment.png");
        image::RgbaImage::from_pixel(100, 100, image::Rgba([240, 80, 64, 255]))
            .save(&attached)
            .unwrap();
        reopened
            .update(cx, |editor, _, cx| {
                editor.push_image_layer(
                    attached.to_string_lossy().into_owned(),
                    100.,
                    100.,
                    crate::editor::layers::Edge::Right,
                    cx,
                );
                let mut annotations = editor.history.current().to_vec();
                annotations.push(crate::editor::annotations::Annotation::Redact {
                    id: "balance-attachment-redaction".into(),
                    x: 155.,
                    y: 80.,
                    width: 30.,
                    height: 20.,
                    style: "pixelate".into(),
                    intensity: 5.,
                });
                editor.history.push(annotations);
                cx.notify();
            })
            .unwrap();
        settle(cx, reopened.into());
        reopened
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, reopened.into());
        let (bounds, png, matte) = cx.update(|cx| {
            let editor = reopened.read(cx).unwrap();
            let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
            (
                editor.bounds.borrow().unwrap(),
                editor.export_png().unwrap(),
                image::Rgba([
                    (theme.r * 255.).round() as u8,
                    (theme.g * 255.).round() as u8,
                    (theme.b * 255.).round() as u8,
                    255,
                ]),
            )
        });
        let saved = image::load_from_memory(&png).unwrap().to_rgba8();
        let cropped_width = 200 - expected.left - expected.right;
        let cropped_height = 160 - expected.top - expected.bottom;
        assert_eq!(
            saved.dimensions(),
            (
                cropped_width + cropped_height + padding * 2,
                cropped_height + padding * 2
            ),
            "attachments must be laid out against the balanced primary image"
        );
        let mut composited = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
        image::imageops::overlay(&mut composited, &saved, 0, 0);
        let screenshot = cx.capture_screenshot(reopened.into()).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32 + expected.left - padding;
        let top = (f32::from(bounds.top()) * scale).round() as u32 + expected.top - padding;
        assert_eq!(
            screenshot.get_pixel(left - 24, top + 40).0,
            matte.0,
            "annotations outside the balanced canvas must be clipped"
        );
        for y in 0..saved.height() {
            for x in 0..saved.width() {
                let actual = screenshot.get_pixel(left + x, top + y).0;
                let expected = composited.get_pixel(x, y).0;
                assert!(
                    actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                    "balanced attachment differs at {x},{y}: {actual:?} vs {expected:?}"
                );
            }
        }
        cx.update_window(reopened.into(), |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        let layered = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                window.set_scale_factor(scale);
                cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
            })
            .unwrap();
        settle(cx, layered.into());
        cx.update(|cx| {
            let editor = layered.read(cx).unwrap();
            assert_eq!(editor.export_png().unwrap(), png);
            assert_eq!(
                editor.snapshot.borrow().balance_crop,
                Some((
                    expected.left as f32,
                    expected.top as f32,
                    expected.right as f32,
                    expected.bottom as f32
                ))
            );
        });
        layered
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, layered.into());
        let bounds = cx.update(|cx| layered.read(cx).unwrap().bounds.borrow().unwrap());
        let screenshot = cx.capture_screenshot(layered.into()).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32 + expected.left - padding;
        let top = (f32::from(bounds.top()) * scale).round() as u32 + expected.top - padding;
        for y in 0..saved.height() {
            for x in 0..saved.width() {
                let actual = screenshot.get_pixel(left + x, top + y).0;
                let expected = composited.get_pixel(x, y).0;
                assert!(
                    actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                    "reopened balanced attachment differs at {x},{y}: {actual:?} vs {expected:?}"
                );
            }
        }
        cx.update_window(layered.into(), |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        return;
    }
    cx.update_window(reopened.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

pub(super) fn balanced_attachment_edges(
    cx: &mut HeadlessAppContext,
    scratch: &Path,
    scale: f32,
    with_background: bool,
) {
    use crate::editor::layers::Edge;

    for (edge, primary_x, primary_y, width, height, redact) in [
        (Edge::Left, 120, 0, 260, 120, (15., 80., 30., 20.)),
        (Edge::Top, 0, 140, 140, 260, (80., 5., 20., 30.)),
        (Edge::Bottom, 0, 0, 140, 260, (80., 125., 20., 30.)),
    ] {
        let name = format!("balance-edge-{scale}-{with_background}-{edge:?}");
        let source = scratch.join(format!("{name}.png"));
        image::RgbaImage::from_fn(200, 160, |x, y| {
            if (100..120).contains(&x) && (85..105).contains(&y) {
                image::Rgba([64, 112, 192, 128])
            } else if (40..160).contains(&x) && (30..130).contains(&y) {
                image::Rgba([64, 112, 192, 255])
            } else if (5..195).contains(&x) && (5..155).contains(&y) {
                image::Rgba([230, 230, 230, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        })
        .save(&source)
        .unwrap();
        let attached = scratch.join(format!("{name}-attachment.png"));
        image::RgbaImage::from_pixel(100, 100, image::Rgba([240, 80, 64, 255]))
            .save(&attached)
            .unwrap();
        let padding_setting = if with_background { 20. } else { 0. };
        crate::history_store::add_item(
            HistoryItem {
                id: name.clone(),
                timestamp: 0,
                original_path: source.to_string_lossy().into_owned(),
                r#type: HistoryItemType::Screenshot,
                editor_state: Some(serde_json::json!({"wallpaper": {
                    "padding": padding_setting,
                    "balance": true,
                    "gradient": with_background.then(|| serde_json::json!({
                        "id": "balance-background",
                        "colors": ["#2468ac"],
                        "angle": 0
                    }))
                }})),
                duration: None,
            },
            100,
            true,
        );
        let padding = (padding_setting as f32 * scale).round() as u32;
        let mut previous = None;
        for reopened in [false, true] {
            let editor = cx
                .open_window(size(px(1280.), px(738.)), |window, cx| {
                    window.set_scale_factor(scale);
                    cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
                })
                .unwrap();
            settle(cx, editor.into());
            if !reopened {
                editor
                    .update(cx, |editor, _, cx| {
                        editor.push_image_layer(
                            attached.to_string_lossy().into_owned(),
                            100.,
                            100.,
                            edge,
                            cx,
                        );
                        editor
                            .history
                            .push(vec![crate::editor::annotations::Annotation::Redact {
                                id: "balance-edge-redaction".into(),
                                x: redact.0,
                                y: redact.1,
                                width: redact.2,
                                height: redact.3,
                                style: "pixelate".into(),
                                intensity: 5.,
                            }]);
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, editor.into());
            }
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = 1. / scale;
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            let (bounds, png, matte) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                assert_eq!(
                    editor.snapshot.borrow().balance_crop,
                    Some((30., 20., 30., 20.))
                );
                let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
                (
                    editor.bounds.borrow().unwrap(),
                    editor.export_png().unwrap(),
                    image::Rgba([
                        (theme.r * 255.).round() as u8,
                        (theme.g * 255.).round() as u8,
                        (theme.b * 255.).round() as u8,
                        255,
                    ]),
                )
            });
            if let Some(previous) = &previous {
                assert_eq!(&png, previous, "{name} must reopen with the same output");
            } else {
                previous = Some(png.clone());
            }
            let saved = image::load_from_memory(&png).unwrap().to_rgba8();
            assert_eq!(
                saved.dimensions(),
                (width + padding * 2, height + padding * 2),
                "{name} must use the balanced primary dimensions"
            );
            let (sample_x, sample_y) = match edge {
                Edge::Left | Edge::Top => (padding + 30, padding + 30),
                Edge::Bottom => (padding + 30, padding + 150),
                _ => unreachable!(),
            };
            assert_eq!(saved.get_pixel(sample_x, sample_y).0, [240, 80, 64, 255]);
            let mut composited = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
            image::imageops::overlay(&mut composited, &saved, 0, 0);
            let screenshot = cx.capture_screenshot(editor.into()).unwrap();
            let left = ((f32::from(bounds.left()) * scale).round() as i32 + 30
                - padding as i32
                - primary_x) as u32;
            let top = ((f32::from(bounds.top()) * scale).round() as i32 + 20
                - padding as i32
                - primary_y) as u32;
            for y in 0..saved.height() {
                for x in 0..saved.width() {
                    let actual = screenshot.get_pixel(left + x, top + y).0;
                    let expected = composited.get_pixel(x, y).0;
                    assert!(
                        actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                        "{name}, reopened {reopened}, at {x},{y}: {actual:?} vs {expected:?}"
                    );
                }
            }
            cx.update_window(editor.into(), |_, window, _| window.remove_window())
                .unwrap();
            cx.run_until_parked();
        }
    }
}

pub(super) fn large_attachment_preview(cx: &mut HeadlessAppContext, scratch: &Path) {
    let blue = image::Rgba([64, 112, 192, 255]);
    let red = image::Rgba([240, 80, 64, 255]);
    let translucent = image::Rgba([40, 190, 80, 128]);
    for vertical in [false, true] {
        let source = scratch.join(format!("large-primary-{vertical}.png"));
        let attached = scratch.join(format!("large-attached-{vertical}.png"));
        let (width, height) = if vertical { (160, 9600) } else { (9600, 160) };
        image::RgbaImage::from_pixel(width, height, blue)
            .save(&source)
            .unwrap();
        image::RgbaImage::from_fn(width, height, |x, y| {
            let position = if vertical { y } else { x };
            if position < 100 {
                blue
            } else if (6700..6900).contains(&position) {
                translucent
            } else {
                red
            }
        })
        .save(&attached)
        .unwrap();
        let editor = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                window.set_scale_factor(1.);
                cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
            })
            .unwrap();
        let window = editor.into();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.push_image_layer(
                    attached.to_string_lossy().into_owned(),
                    f64::from(width),
                    f64::from(height),
                    if vertical {
                        crate::editor::layers::Edge::Bottom
                    } else {
                        crate::editor::layers::Edge::Right
                    },
                    cx,
                );
                let (x, y, width, height) = if vertical {
                    (80., 9200., 80., 800.)
                } else {
                    (9200., 80., 800., 80.)
                };
                editor
                    .history
                    .push(vec![crate::editor::annotations::Annotation::Rectangle {
                        id: "large-join".into(),
                        x,
                        y,
                        width,
                        height,
                        stroke: "#ff00ff".into(),
                        stroke_width: 1.,
                        fill: Some("#ff00ff".into()),
                    }]);
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 0.035;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        let (bounds, png) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                editor.export_png().unwrap(),
            )
        });
        let saved = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(
            saved.dimensions(),
            if vertical { (160, 19200) } else { (19200, 160) }
        );
        let coordinate = |along, across| {
            if vertical {
                (across, along)
            } else {
                (along, across)
            }
        };
        let matte = cx.update(|cx| {
            let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ])
        });
        let mut composited = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
        image::imageops::overlay(&mut composited, &saved, 0, 0);
        let screenshot = cx.capture_screenshot(window).unwrap();
        #[cfg(windows)]
        let tiles = cx.update(|cx| {
            editor
                .read(cx)
                .unwrap()
                .snapshot
                .borrow()
                .preview_edges
                .iter()
                .map(|tile| tile.image.clone())
                .collect::<Vec<_>>()
        });
        #[cfg(windows)]
        cx.update_window(window, |_, window, _| {
            assert_eq!(tiles.len(), 3);
            assert!(tiles
                .iter()
                .all(|image| window.has_image_atlas_entry(image)));
        })
        .unwrap();
        for (along, across, expected) in [
            (4100, 40, blue.0),
            (8190, 40, blue.0),
            (8210, 40, blue.0),
            (12300, 40, red.0),
            (16380, 40, translucent.0),
            (16400, 40, translucent.0),
            (17500, 40, red.0),
            (9400, 120, [255, 0, 255, 255]),
            (9800, 120, [255, 0, 255, 255]),
        ] {
            let (x, y) = coordinate(along, across);
            assert!(saved
                .get_pixel(x, y)
                .0
                .iter()
                .zip(expected)
                .all(|(a, e)| a.abs_diff(e) <= 2));
            let display = composited.get_pixel(x, y).0;
            let x = (f32::from(bounds.left()) + x as f32 * 0.035).round() as u32;
            let y = (f32::from(bounds.top()) + y as f32 * 0.035).round() as u32;
            let actual = screenshot.get_pixel(x, y).0;
            assert!(
                actual.iter().zip(display).all(|(a, e)| a.abs_diff(e) <= 2),
                "large attachment preview differs at {x},{y}: {actual:?} vs {display:?}"
            );
        }
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 0.25;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        for (scroll, join) in [(-1500., 8192.), (-2200., 16384.)] {
            event(
                cx,
                window,
                PlatformInput::Scroll(gpui::ScrollEvent {
                    position: point(px(600.), px(400.)),
                    delta: gpui::ScrollDelta::Pixels(if vertical {
                        point(px(0.), px(scroll))
                    } else {
                        point(px(scroll), px(0.))
                    }),
                    ..Default::default()
                }),
            );
            settle(cx, window);
            let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
            let seam = (f32::from(if vertical {
                bounds.top()
            } else {
                bounds.left()
            }) + join * 0.25)
                .round() as i32;
            let across = (f32::from(if vertical {
                bounds.left()
            } else {
                bounds.top()
            }) + 40. * 0.25)
                .round() as u32;
            assert!(
                (4..if vertical { 734 } else { 1276 }).contains(&seam),
                "the {join} tile join must be visible for vertical {vertical}: {seam}, {bounds:?}"
            );
            let screenshot = cx.capture_screenshot(window).unwrap();
            let (source_x, source_y) = coordinate(join as u32, 40);
            let color = composited.get_pixel(source_x, source_y).0;
            for delta in -3..=3 {
                let (x, y) = coordinate((seam + delta) as u32, across);
                let actual = screenshot.get_pixel(x, y).0;
                assert!(
                    actual.iter().zip(color).all(|(a, e)| a.abs_diff(e) <= 2),
                    "large tile seam differs at {delta}: {actual:?}"
                );
            }
        }
        #[cfg(windows)]
        {
            editor
                .update(cx, |editor, _, cx| {
                    editor.wallpaper.corners = 1.;
                    cx.notify();
                })
                .unwrap();
            settle(cx, window);
            cx.update_window(window, |_, window, _| {
                assert!(tiles
                    .iter()
                    .all(|image| !window.has_image_atlas_entry(image)));
            })
            .unwrap();
        }
        cx.update_window(window, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
}

pub(super) fn restored_pixelation(
    cx: &mut HeadlessAppContext,
    scratch: &Path,
    scale: f32,
    transparent: bool,
    clipped: bool,
) {
    let source = scratch.join(format!(
        "restored-pixelation-{scale}-{transparent}-{clipped}.png"
    ));
    image::RgbaImage::from_fn(200, 120, |x, y| {
        let red = if x % 16 < 8 { 24 } else { 224 };
        image::Rgba([
            red,
            (y % 32 * 6) as u8,
            80,
            if transparent { 128 } else { 255 },
        ])
    })
    .save(&source)
    .unwrap();
    let annotation = crate::editor::annotations::Annotation::Redact {
        id: "restored-pixelation".into(),
        x: if clipped { -8. } else { 40. },
        y: 30.,
        width: 80.,
        height: 40.,
        style: "pixelate".into(),
        intensity: 5.,
    };
    let shape = crate::editor::annotations::Annotation::Rectangle {
        id: "before-pixelation".into(),
        x: 64.,
        y: 40.,
        width: 32.,
        height: 20.,
        stroke: "#00ff00".into(),
        stroke_width: 1.,
        fill: Some("#00ff00".into()),
    };
    let mut annotations = Vec::new();
    if scale == 1. && !clipped {
        annotations.push(shape);
    }
    annotations.push(annotation);
    annotations.push(crate::editor::annotations::Annotation::Rectangle {
        id: "after-pixelation".into(),
        x: if clipped { 50. } else { 100. },
        y: 48.,
        width: 12.,
        height: 12.,
        stroke: "#ff00ff".into(),
        stroke_width: 1.,
        fill: Some("#ff00ff".into()),
    });
    annotations.push(crate::editor::annotations::Annotation::Redact {
        id: "after-pixelation-blackout".into(),
        x: if clipped { 54. } else { 104. },
        y: 52.,
        width: 5.,
        height: 5.,
        style: "blackout".into(),
        intensity: 2.,
    });
    crate::history_store::add_item(
        HistoryItem {
            id: format!("restored-pixelation-{scale}-{transparent}-{clipped}"),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: Some(serde_json::json!({"annotations": annotations})),
            duration: None,
        },
        100,
        true,
    );
    let editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, editor.into());
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 1. / scale;
            cx.notify();
        })
        .unwrap();
    settle(cx, editor.into());
    let (bounds, png, matte) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).surface);
        (
            editor.bounds.borrow().unwrap(),
            editor.export_png().unwrap(),
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ]),
        )
    });
    let saved = image::load_from_memory(&png).unwrap().to_rgba8();
    let original = image::open(&source).unwrap().to_rgba8();
    let left_edge = if clipped { 0 } else { 40 };
    let right_edge = if clipped { 72 } else { 120 };
    assert_ne!(
        saved.get_pixel(left_edge + 3, 32),
        original.get_pixel(left_edge + 3, 32)
    );
    assert_eq!(
        saved.get_pixel(if clipped { 56 } else { 106 }, 54).0,
        [0, 0, 0, 255]
    );
    let mut composited = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
    image::imageops::overlay(&mut composited, &saved, 0, 0);
    let screenshot = cx.capture_screenshot(editor.into()).unwrap();
    let left = (f32::from(bounds.left()) * scale).round() as u32;
    let top = (f32::from(bounds.top()) * scale).round() as u32;
    for y in 30..70 {
        for x in left_edge..right_edge {
            let actual = screenshot.get_pixel(left + x, top + y).0;
            let expected = composited.get_pixel(x, y).0;
            assert!(
                actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                "restored pixelation {scale}x, transparent={transparent}, clipped={clipped} at {x},{y}: {actual:?} vs {expected:?}"
            );
        }
    }
    let flattened_path = scratch.join(format!("restored-pixelation-flat-{transparent}.png"));
    std::fs::write(&flattened_path, &png).unwrap();
    let flattened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(flattened_path.to_str().unwrap(), window, cx))
        })
        .unwrap();
    for effective_zoom in [0.5, 0.73, 1.37, 1.5] {
        let zoom = effective_zoom / scale;
        for window in [editor, flattened] {
            window
                .update(cx, |editor, _, cx| {
                    editor.zoom = zoom;
                    cx.notify();
                })
                .unwrap();
            settle(cx, window.into());
        }
        let (preview_bounds, flattened_bounds) = cx.update(|cx| {
            (
                editor.read(cx).unwrap().bounds.borrow().unwrap(),
                flattened.read(cx).unwrap().bounds.borrow().unwrap(),
            )
        });
        let preview = cx.capture_screenshot(editor.into()).unwrap();
        let flattened_image = cx.capture_screenshot(flattened.into()).unwrap();
        let snap = |value: f32| (value.abs() - 0.5).ceil().copysign(value) as u32;
        let left = snap(f32::from(preview_bounds.left()) * scale);
        let top = snap(f32::from(preview_bounds.top()) * scale);
        let flat_left = snap(f32::from(flattened_bounds.left()) * scale);
        let flat_top = snap(f32::from(flattened_bounds.top()) * scale);
        if scale == 1. {
            let width = (saved.width() as f32 * effective_zoom).round() as u32;
            let height = (saved.height() as f32 * effective_zoom).round() as u32;
            image::imageops::crop_imm(&preview, left, top, width, height)
                .to_image()
                .save(scratch.join(format!(
                    "pixelation-zoom-{effective_zoom}-{transparent}-{clipped}-preview.png"
                )))
                .unwrap();
            image::imageops::crop_imm(&flattened_image, flat_left, flat_top, width, height)
                .to_image()
                .save(scratch.join(format!(
                    "pixelation-zoom-{effective_zoom}-{transparent}-{clipped}-saved.png"
                )))
                .unwrap();
        }
        for y in ((29. * effective_zoom).ceil() as u32)..((71. * effective_zoom).floor() as u32) {
            for x in ((left_edge.saturating_sub(1) as f32 * effective_zoom).ceil() as u32)
                ..(((right_edge + 1).min(200) as f32 * effective_zoom).floor() as u32)
            {
                let actual = preview.get_pixel(left + x, top + y).0;
                let expected = flattened_image.get_pixel(flat_left + x, flat_top + y).0;
                assert!(
                        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                        "pixelation at zoom={effective_zoom}, DPI={scale}, transparent={transparent}, clipped={clipped}, {x},{y}: {actual:?} vs {expected:?}"
                    );
            }
        }
    }
    cx.update_window(flattened.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    if scale == 1. && !clipped {
        check_redaction_draft(cx, editor, f64::from(left_edge + 4), 32.);
        cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            assert!(!editor
                .snapshot
                .borrow()
                .redact_patches
                .contains_key("restored-pixelation"));
        });
        for gradient in [false, true] {
            editor
                .update(cx, |editor, _, cx| {
                    editor.wallpaper.padding = 20.;
                    editor.wallpaper.set_gradient(
                        gradient
                            .then(|| crate::editor::wallpaper::preset("slate"))
                            .flatten(),
                    );
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = 1.;
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            let (bounds, png, background) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                assert!(!editor
                    .snapshot
                    .borrow()
                    .redact_patches
                    .contains_key("restored-pixelation"));
                let background = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
                (
                    editor.bounds.borrow().unwrap(),
                    editor.export_png().unwrap(),
                    image::Rgba([
                        (background.r * 255.).round() as u8,
                        (background.g * 255.).round() as u8,
                        (background.b * 255.).round() as u8,
                        255,
                    ]),
                )
            });
            let wallpaper = image::load_from_memory(&png).unwrap().to_rgba8();
            let mut composited =
                image::RgbaImage::from_pixel(wallpaper.width(), wallpaper.height(), background);
            image::imageops::overlay(&mut composited, &wallpaper, 0, 0);
            let screenshot = cx.capture_screenshot(editor.into()).unwrap();
            let left = f32::from(bounds.left()).round() as u32;
            let top = f32::from(bounds.top()).round() as u32;
            for y in 30..70 {
                for x in 40..120 {
                    let actual = screenshot.get_pixel(left + x, top + y).0;
                    let expected = composited.get_pixel(x + 20, y + 20).0;
                    assert!(
                        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                        "pixelation with wallpaper gradient={gradient}, transparent={transparent} at {x},{y}: {actual:?} vs {expected:?}"
                    );
                }
            }
        }
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.set_gradient(None);
                editor.wallpaper.padding = 0.;
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            assert!(!editor
                .snapshot
                .borrow()
                .redact_patches
                .contains_key("restored-pixelation"));
        });
        cx.update_window(editor.into(), |_, window, _| window.set_scale_factor(2.))
            .unwrap();
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 0.5;
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        let (bounds, png) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                editor.export_png().unwrap(),
            )
        });
        let changed = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_ne!(saved, changed, "DPI changes must resize pixelation blocks");
        let mut composited = image::RgbaImage::from_pixel(changed.width(), changed.height(), matte);
        image::imageops::overlay(&mut composited, &changed, 0, 0);
        let screenshot = cx.capture_screenshot(editor.into()).unwrap();
        let left = (f32::from(bounds.left()) * 2.).round() as u32;
        let top = (f32::from(bounds.top()) * 2.).round() as u32;
        for y in 30..70 {
            for x in 40..120 {
                let actual = screenshot.get_pixel(left + x, top + y).0;
                let expected = composited.get_pixel(x, y).0;
                assert!(
                    actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                    "pixelation after DPI change at {x},{y}: {actual:?} vs {expected:?}"
                );
            }
        }
    }
    cx.update_window(editor.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

pub(super) fn screenshot(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path) {
    let source = scratch.join("workflow-capture.png");
    std::fs::copy(scratch.join("capture.png"), &source).unwrap();
    crate::history_store::add_item(
        HistoryItem {
            id: "visual-workflow".into(),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: None,
            duration: None,
        },
        10,
        true,
    );
    let editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    let window = editor.into();
    settle(cx, window);
    click_measured(cx, window, "editor-tools", 0);
    cx.update_window(window, |_, window, cx| {
        window.dispatch_action(Box::new(crate::editor::actions::ZoomReset), cx)
    })
    .unwrap();
    settle(cx, window);
    let bounds = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert_eq!(editor.zoom, 1.);
        editor.bounds.borrow().unwrap()
    });
    for (index, start, end) in [
        (0, (80., 80.), (200., 140.)),
        (1, (240., 100.), (380., 100.)),
        (2, (60., 200.), (220., 300.)),
        (8, (400., 200.), (560., 300.)),
    ] {
        click_measured(cx, window, "editor-tools", index);
        let start = bounds.origin + point(px(start.0), px(start.1));
        let end = bounds.origin + point(px(end.0), px(end.1));
        event(
            cx,
            window,
            PlatformInput::MouseDown(MouseDownEvent {
                position: start,
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Modifiers::none(),
                first_mouse: false,
            }),
        );
        event(
            cx,
            window,
            PlatformInput::MouseMove(MouseMoveEvent {
                position: end - point(px(20.), px(10.)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
        );
        event(
            cx,
            window,
            PlatformInput::MouseUp(MouseUpEvent {
                position: end,
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Modifiers::none(),
            }),
        );
        settle(cx, window);
    }
    hover(cx, window, 20., 700.);
    let (annotations, png) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert_eq!(editor.history.current().len(), 4);
        assert!(matches!(&editor.history.current()[2], crate::editor::annotations::Annotation::Rectangle { width, height, .. } if *width == 160. && *height == 100.), "the release position must finish the shape");
        assert!(matches!(&editor.history.current()[0], crate::editor::annotations::Annotation::Pen { points, .. } if points.ends_with(&[200., 140.])), "the release position must finish the stroke");
        (
            editor.history.current().to_vec(),
            editor.export_png().unwrap(),
        )
    });
    let saved = scratch.join("workflow-saved.png");
    std::fs::write(&saved, &png).unwrap();
    capture(cx, window, output, "workflow-annotated");
    let preview = cx.capture_screenshot(window).unwrap();
    let preview = image::imageops::crop_imm(
        &preview,
        f32::from(bounds.left()).round() as u32,
        f32::from(bounds.top()).round() as u32,
        640,
        360,
    )
    .to_image();
    preview.save(output.join("workflow-preview.png")).unwrap();
    let exported = image::load_from_memory(&png).unwrap().to_rgba8();
    exported.save(output.join("workflow-export.png")).unwrap();
    let significant = preview
        .pixels()
        .zip(exported.pixels())
        .filter(|(actual, expected)| {
            actual
                .0
                .iter()
                .zip(expected.0)
                .any(|(actual, expected)| actual.abs_diff(expected) > 2)
        })
        .count();
    assert_eq!(
        significant, 0,
        "preview/export differ beyond two channel levels"
    );
    cx.update_window(window, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    let reopened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, reopened.into());
    cx.update(|cx| {
        let reopened = reopened.read(cx).unwrap();
        assert_eq!(reopened.history.current(), annotations);
        assert_eq!(
            image::load_from_memory(&reopened.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
            exported
        );
    });
    capture(cx, reopened.into(), output, "workflow-history-reopened");
    let flattened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.);
            cx.new(|cx| EditorWindow::from_file(saved.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, flattened.into());
    cx.update(|cx| {
        let flattened = flattened.read(cx).unwrap();
        assert!(flattened.history.current().is_empty());
        assert_eq!(
            image::load_from_memory(&flattened.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
            exported
        );
    });
    capture(cx, flattened.into(), output, "workflow-saved-reopened");
    println!("annotation, PNG save, history reopen and flattened reopen passed; preview/export changed pixels above two channel levels: {significant}");
}

pub(super) fn scaled_wallpaper_preview(cx: &mut HeadlessAppContext, scratch: &Path) {
    let source = scratch.join("scaled-wallpaper.png");
    image::RgbaImage::from_fn(640, 360, |x, y| {
        if (240..320).contains(&x) && (120..180).contains(&y) {
            return image::Rgba([240, 80, 64, 128]);
        }
        if (x / 80 + y / 60) % 2 == 0 {
            image::Rgba([240, 80, 64, 255])
        } else {
            image::Rgba([64, 112, 192, 255])
        }
    })
    .save(&source)
    .unwrap();
    crate::history_store::add_item(
        HistoryItem {
            id: "layered-wallpaper-workflow".into(),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: None,
            duration: None,
        },
        100,
        true,
    );
    let editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(2.);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    let window = editor.into();
    editor
        .update(cx, |editor, _, cx| {
            editor
                .wallpaper
                .set_gradient(crate::editor::wallpaper::preset("slate"));
            editor.wallpaper.padding = 20.;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let previous = cx.update(|cx| {
        editor
            .read(cx)
            .unwrap()
            .snapshot
            .borrow()
            .backdrop
            .clone()
            .unwrap()
    });
    let original_mode = cx.update(|cx| crate::theme::vars::active_mode(cx));
    let other_mode = match original_mode {
        crate::theme::presets::ThemeMode::Light => crate::theme::presets::ThemeMode::Dark,
        _ => crate::theme::presets::ThemeMode::Light,
    };
    cx.update(|cx| crate::theme::vars::update_theme(cx, other_mode, "default"));
    super::frame(cx, window);
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert!(editor
            .snapshot
            .borrow()
            .backdrop
            .as_ref()
            .is_some_and(|backdrop| { std::sync::Arc::ptr_eq(backdrop, &previous) }));
    });
    cx.update(|cx| crate::theme::vars::update_theme(cx, original_mode, "default"));
    let stage_matte = cx.update(|cx| {
        let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
        image::Rgba([
            (theme.r * 255.).round() as u8,
            (theme.g * 255.).round() as u8,
            (theme.b * 255.).round() as u8,
            255,
        ])
    });
    super::frame(cx, window);
    editor
        .update(cx, |editor, _, cx| {
            editor.wallpaper.background_blur = 30.;
            cx.notify();
        })
        .unwrap();
    super::frame(cx, window);
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let snapshot = editor.snapshot.borrow();
        assert!(snapshot
            .backdrop
            .as_ref()
            .is_some_and(|backdrop| std::sync::Arc::ptr_eq(backdrop, &previous)));
    });
    cx.update_window(window, |_, window, _| window.set_scale_factor(1.25))
        .unwrap();
    super::frame(cx, window);
    cx.update(|cx| {
        assert!(editor
            .read(cx)
            .unwrap()
            .snapshot
            .borrow()
            .backdrop
            .is_none());
    });
    settle(cx, window);
    cx.update(|cx| {
        assert!(editor
            .read(cx)
            .unwrap()
            .snapshot
            .borrow()
            .backdrop
            .is_some());
    });
    editor
        .update(cx, |editor, _, cx| {
            editor.wallpaper.padding = 40.;
            cx.notify();
        })
        .unwrap();
    super::frame(cx, window);
    cx.update(|cx| {
        assert!(editor
            .read(cx)
            .unwrap()
            .snapshot
            .borrow()
            .backdrop
            .is_none());
    });
    for (scale, padding, transparent, blurred) in [
        (1.25_f32, 25_u32, false, false),
        (2., 40, false, false),
        (1.25, 25, true, false),
        (2., 40, true, false),
        (1.25, 25, false, true),
        (2., 40, false, true),
    ] {
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.set_gradient(if transparent {
                    None
                } else {
                    crate::editor::wallpaper::preset("slate")
                });
                editor.wallpaper.padding = 20.;
                editor.wallpaper.corners = 0.;
                editor.wallpaper.background_blur = if blurred { 30. } else { 0. };
                editor.wallpaper.shadow = 0.;
                cx.notify();
            })
            .unwrap();
        cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, window);
        cx.update(|cx| {
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = 1. / scale;
                    cx.notify();
                })
                .unwrap();
        });
        cx.advance_clock(std::time::Duration::from_millis(32));
        settle(cx, window);
        let (bounds, saved) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                image::load_from_memory(&editor.export_png().unwrap())
                    .unwrap()
                    .to_rgba8(),
            )
        });
        assert_eq!(saved.dimensions(), (640 + 2 * padding, 360 + 2 * padding));
        let mut expected = saved.clone();
        if transparent {
            assert_eq!(saved.get_pixel(0, 0).0[3], 0);
            expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), stage_matte);
            image::imageops::overlay(&mut expected, &saved, 0, 0);
        }
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32 - padding;
        let top = (f32::from(bounds.top()) * scale).round() as u32 - padding;
        let preview =
            image::imageops::crop_imm(&screenshot, left, top, saved.width(), saved.height())
                .to_image();
        let significant = preview
            .pixels()
            .zip(expected.pixels())
            .filter(|(actual, expected)| {
                actual
                    .0
                    .iter()
                    .zip(expected.0)
                    .any(|(actual, expected)| actual.abs_diff(expected) > 2)
            })
            .count();
        if significant > 0 {
            preview
                .save(scratch.join("scaled-wallpaper-preview.png"))
                .unwrap();
            expected
                .save(scratch.join("scaled-wallpaper-expected.png"))
                .unwrap();
        }
        assert_eq!(
            significant, 0,
            "wallpaper preview/export at {scale}x, transparent: {transparent}, blurred: {blurred}"
        );
    }
    for (scale, transparent, radius, shadow, padding_setting) in [
        (1.25_f32, true, 24., 0., 0.),
        (2., true, 24., 0., 0.),
        (1.25, true, 0., 50., 0.),
        (2., true, 24., 50., 0.),
        (1.25, false, 1., 0., 20.),
        (1.25, false, 24., 0., 20.),
        (1.25, true, 24., 0., 20.),
        (2., false, 200., 0., 20.),
        (2., false, 24., 0., 20.),
        (1.25, false, 24., 50., 20.),
        (1.25, true, 0., 50., 20.),
        (2., false, 0., 150., 20.),
        (2., true, 24., 50., 20.),
    ] {
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.set_gradient((!transparent).then(|| {
                    crate::editor::wallpaper::GradientOption {
                        id: "visual-solid".into(),
                        colors: vec!["#2468ac".into()],
                        angle: 0.,
                    }
                }));
                editor.wallpaper.padding = padding_setting;
                editor.wallpaper.corners = radius;
                editor.wallpaper.background_blur = 0.;
                editor.wallpaper.shadow = shadow;
                cx.notify();
            })
            .unwrap();
        cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        if padding_setting == 0. && radius == 24. && shadow == 0. && scale == 1.25 {
            let previous = cx.update(|cx| {
                editor
                    .read(cx)
                    .unwrap()
                    .snapshot
                    .borrow()
                    .backdrop
                    .clone()
                    .unwrap()
            });
            cx.update(|cx| crate::theme::vars::update_theme(cx, other_mode, "default"));
            settle(cx, window);
            let (bounds, new_backdrop, other_matte) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
                (
                    editor.bounds.borrow().unwrap(),
                    editor.snapshot.borrow().backdrop.clone().unwrap(),
                    [
                        (theme.r * 255.).round() as u8,
                        (theme.g * 255.).round() as u8,
                        (theme.b * 255.).round() as u8,
                    ],
                )
            });
            assert_ne!(
                previous.id, new_backdrop.id,
                "theme change must replace the transparent backdrop"
            );
            let screenshot = cx.capture_screenshot(window).unwrap();
            let x = (f32::from(bounds.left()) * scale).round() as u32;
            let y = (f32::from(bounds.top()) * scale).round() as u32;
            assert!(
                screenshot.get_pixel(x, y).0[..3]
                    .iter()
                    .zip(other_matte)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                "transparent corner must use the new theme background"
            );
            #[cfg(windows)]
            cx.update_window(window, |_, window, _| {
                assert!(
                    !window.has_image_atlas_entry(&previous),
                    "the old theme backdrop must leave the GPU atlas"
                );
            })
            .unwrap();
            cx.update(|cx| crate::theme::vars::update_theme(cx, original_mode, "default"));
            settle(cx, window);
        }
        let (bounds, saved) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                image::load_from_memory(&editor.export_png().unwrap())
                    .unwrap()
                    .to_rgba8(),
            )
        });
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32;
        let top = (f32::from(bounds.top()) * scale).round() as u32;
        let padding = (padding_setting as f32 * scale).round() as u32;
        let mut expected = saved.clone();
        if transparent {
            expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), stage_matte);
            image::imageops::overlay(&mut expected, &saved, 0, 0);
        }
        let mut worst = 0;
        let mut edge_outliers = 0;
        let mismatched = (0..expected.height())
            .flat_map(|y| (0..expected.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let difference = screenshot
                    .get_pixel(left - padding + x, top - padding + y)
                    .0
                    .iter()
                    .zip(expected.get_pixel(x, y).0)
                    .map(|(actual, expected)| actual.abs_diff(expected))
                    .max()
                    .unwrap_or(0);
                worst = worst.max(difference);
                if difference > 2 && transparent && radius > 0. && shadow > 0. {
                    let radius = (radius * f64::from(scale)).min(180.);
                    let x = f64::from(x) - f64::from(padding) + 0.5;
                    let y = f64::from(y) - f64::from(padding) + 0.5;
                    let dx = x - x.clamp(radius, 640. - radius);
                    let dy = y - y.clamp(radius, 360. - radius);
                    if (dx.hypot(dy) - radius).abs() <= 1.5 {
                        edge_outliers += 1;
                    }
                }
                difference > 2
            })
            .count();
        if mismatched > 0 {
            image::imageops::crop_imm(
                &screenshot,
                left - padding,
                top - padding,
                expected.width(),
                expected.height(),
            )
            .to_image()
            .save(scratch.join("rounded-preview.png"))
            .unwrap();
            expected.save(scratch.join("rounded-export.png")).unwrap();
        }
        assert!(
            mismatched == edge_outliers && edge_outliers <= 1 && worst <= 3,
            "rounded preview differs from saved pixels at {scale}x, transparent {transparent}, radius {radius}, shadow {shadow}, padding {padding_setting}: {mismatched} pixels, worst difference {worst}"
        );
    }
    editor
        .update(cx, |editor, _, cx| {
            editor
                .history
                .push(vec![crate::editor::annotations::Annotation::Rectangle {
                    id: "rounded-edge".into(),
                    x: 0.,
                    y: 330.,
                    width: 40.,
                    height: 30.,
                    stroke: "#00ff00".into(),
                    stroke_width: 1.,
                    fill: Some("#00ff00".into()),
                }]);
            editor.tool = crate::ui::colors::Tool::Select;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let (bounds, saved) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        (
            editor.bounds.borrow().unwrap(),
            image::load_from_memory(&editor.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
        )
    });
    let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), stage_matte);
    image::imageops::overlay(&mut expected, &saved, 0, 0);
    let screenshot = cx.capture_screenshot(window).unwrap();
    let left = (f32::from(bounds.left()) * 2.).round() as u32;
    let top = (f32::from(bounds.top()) * 2.).round() as u32;
    for y in 348..360 {
        for x in 0..12 {
            assert!(
                screenshot
                    .get_pixel(left + x, top + y)
                    .0
                    .iter()
                    .zip(expected.get_pixel(40 + x, 40 + y).0)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                "rounded annotation differs at {x},{y}"
            );
        }
    }
    super::click(
        cx,
        window,
        f32::from(bounds.left()) + 10.,
        f32::from(bounds.top()) + 172.5,
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().snapshot.borrow().selected,
            ["rounded-edge"]
        );
    });
    let selected = cx.capture_screenshot(window).unwrap();
    let foreground =
        cx.update(|cx| gpui::Rgba::from(crate::theme::vars::active_theme(cx).primary_foreground));
    for (x, y) in [(-2, -6), (2, -6), (-2, 2), (2, 2)] {
        assert!(
            selected
                .get_pixel((left as i32 + x) as u32, (top as i32 + 360 + y) as u32)
                .0[..3]
                .iter()
                .zip([foreground.r, foreground.g, foreground.b])
                .all(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs() <= 2.),
            "the rounded selection handle must remain visible at {x},{y}"
        );
    }
    editor
        .update(cx, |editor, _, cx| {
            editor.history.push(Vec::new());
            editor.tool = crate::ui::colors::Tool::Crop;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let start = bounds.origin + point(px(50.), px(50.));
    let end = bounds.origin + point(px(150.), px(110.));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    event(
        cx,
        window,
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
    let cropped = cx.capture_screenshot(window).unwrap();
    for (actual, original) in cropped.get_pixel(left, top).0[..3]
        .iter()
        .zip(screenshot.get_pixel(left, top).0[..3].iter())
    {
        assert!(
            actual.abs_diff(*original / 2) <= 1,
            "crop must dim the rounded cutout"
        );
    }
    editor
        .update(cx, |editor, _, cx| editor.cancel_crop(cx))
        .unwrap();
    let start = bounds.origin + point(px(0.5), px(0.5));
    let end = bounds.origin + point(px(35.), px(25.));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    event(
        cx,
        window,
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().snapshot.borrow().crop,
            Some((1., 1., 69., 49.))
        );
    });
    let cropped = cx.capture_screenshot(window).unwrap();
    for (x, y) in [(-2, -2), (2, -2), (-2, 2), (2, 2)] {
        assert!(
            cropped
                .get_pixel((left as i32 + x) as u32, (top as i32 + y) as u32)
                .0[..3]
                .iter()
                .zip([foreground.r, foreground.g, foreground.b])
                .all(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs() <= 2.),
            "the rounded crop handle must remain visible at {x},{y}"
        );
    }
    editor
        .update(cx, |editor, _, cx| editor.cancel_crop(cx))
        .unwrap();
    for (scale, style, content) in [
        (1.25_f32, "windows-dark", [18_u8, 18, 18]),
        (2., "windows-dark", [18, 18, 18]),
        (1.25, "windows-light", [255, 255, 255]),
        (2., "windows-light", [255, 255, 255]),
        (1.25, "macos-dark", [28, 28, 30]),
        (2., "macos-dark", [28, 28, 30]),
        (1.25, "macos-light", [255, 255, 255]),
        (2., "macos-light", [255, 255, 255]),
    ] {
        for shadow in [0., 50.] {
            #[cfg(windows)]
            let old_edges = cx.update(|cx| {
                editor
                    .read(cx)
                    .unwrap()
                    .snapshot
                    .borrow()
                    .preview_edges
                    .iter()
                    .map(|edge| edge.image.clone())
                    .collect::<Vec<_>>()
            });
            #[cfg(windows)]
            if !old_edges.is_empty() {
                cx.update_window(window, |_, window, _| {
                    assert!(
                        old_edges
                            .iter()
                            .any(|image| window.has_image_atlas_entry(image)),
                        "the previous frame edges must have been painted"
                    );
                })
                .unwrap();
            }
            editor
                .update(cx, |editor, _, cx| {
                    editor
                        .wallpaper
                        .set_gradient(Some(crate::editor::wallpaper::GradientOption {
                            id: "visual-solid".into(),
                            colors: vec!["#2468ac".into()],
                            angle: 0.,
                        }));
                    editor.wallpaper.padding = 20.;
                    editor.wallpaper.corners = 0.;
                    editor.wallpaper.background_blur = 0.;
                    editor.wallpaper.window_frame.style = style.into();
                    editor.wallpaper.shadow = shadow;
                    cx.notify();
                })
                .unwrap();
            cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
                .unwrap();
            settle(cx, window);
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = 1. / scale;
                    cx.notify();
                })
                .unwrap();
            cx.advance_clock(std::time::Duration::from_millis(32));
            settle(cx, window);
            #[cfg(windows)]
            cx.update_window(window, |_, window, _| {
                for image in &old_edges {
                    assert!(
                        !window.has_image_atlas_entry(image),
                        "replaced frame edges must leave the GPU atlas"
                    );
                }
            })
            .unwrap();
            let (bounds, saved) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                (
                    editor.bounds.borrow().unwrap(),
                    image::load_from_memory(&editor.export_png().unwrap())
                        .unwrap()
                        .to_rgba8(),
                )
            });
            let padding = (20. * scale).round() as u32;
            let title = (28. * scale).round() as u32;
            let screenshot = cx.capture_screenshot(window).unwrap();
            let left = (f32::from(bounds.left()) * scale).round() as u32;
            let top = (f32::from(bounds.top()) * scale).round() as u32;
            let expected = saved.get_pixel(padding + 250, padding + title + 130);
            let background = if shadow > 0. { content } else { [36, 104, 172] };
            for (actual, (foreground, background)) in expected.0[..3]
                .iter()
                .zip([240_u8, 80, 64].into_iter().zip(background))
            {
                let blended = (u16::from(foreground) * 128 + u16::from(background) * 127) / 255;
                assert!(actual.abs_diff(blended as u8) <= 2);
            }
            let significant = (120..180)
                .flat_map(|y| (240..320).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(saved.get_pixel(padding + x, padding + title + y).0)
                        .any(|(actual, expected)| actual.abs_diff(expected) > 2)
                })
                .count();
            assert_eq!(
                significant, 0,
                "transparent framed preview at {scale}x, {style}, shadow {shadow}: preview {:?}, saved {:?}",
                screenshot.get_pixel(left + 250, top + 130),
                saved.get_pixel(padding + 250, padding + title + 130)
            );
            let mismatched = (0..360)
                .flat_map(|y| (0..640).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(saved.get_pixel(padding + x, padding + title + y).0)
                        .any(|(actual, expected)| actual.abs_diff(expected) > 2)
                })
                .count();
            if mismatched > 0 {
                image::imageops::crop_imm(&screenshot, left, top, 640, 360)
                    .to_image()
                    .save(scratch.join("framed-preview.png"))
                    .unwrap();
                image::imageops::crop_imm(&saved, padding, padding + title, 640, 360)
                    .to_image()
                    .save(scratch.join("framed-export.png"))
                    .unwrap();
            }
            assert_eq!(
                mismatched, 0,
                "framed preview differs from saved pixels at {scale}x, {style}, shadow {shadow}"
            );
        }
    }
    editor
        .update(cx, |editor, _, cx| {
            editor.history.push(
                [("left", 0.), ("right", 600.)]
                    .into_iter()
                    .map(
                        |(id, x)| crate::editor::annotations::Annotation::Rectangle {
                            id: id.into(),
                            x,
                            y: 330.,
                            width: 40.,
                            height: 30.,
                            stroke: "#00ff00".into(),
                            stroke_width: 1.,
                            fill: Some("#00ff00".into()),
                        },
                    )
                    .collect(),
            );
            cx.notify();
        })
        .unwrap();
    for (scale, style) in [(1.25_f32, "macos-light"), (2., "windows-dark")] {
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.window_frame.style = style.into();
                editor.wallpaper.shadow = 0.;
                cx.notify();
            })
            .unwrap();
        cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        let (bounds, saved) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                image::load_from_memory(&editor.export_png().unwrap())
                    .unwrap()
                    .to_rgba8(),
            )
        });
        let padding = (20. * scale).round() as u32;
        let title = (28. * scale).round() as u32;
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32;
        let top = (f32::from(bounds.top()) * scale).round() as u32;
        for y in 348..360 {
            for x in (0..12).chain(628..640) {
                assert!(
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(saved.get_pixel(padding + x, padding + title + y).0)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                    "framed annotation differs at {x},{y}, {scale}x, {style}"
                );
            }
        }
    }
    editor
        .update(cx, |editor, _, cx| {
            editor.tool = crate::ui::colors::Tool::Select;
            cx.notify();
        })
        .unwrap();
    let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
    super::click(
        cx,
        window,
        f32::from(bounds.left()) + 20. * 0.5,
        f32::from(bounds.top()) + 345. * 0.5,
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().snapshot.borrow().selected,
            ["left"]
        );
    });
    let selected = cx.capture_screenshot(window).unwrap();
    let left = (f32::from(bounds.left()) * 2.).round() as u32;
    let top = (f32::from(bounds.top()) * 2.).round() as u32;
    let foreground =
        cx.update(|cx| gpui::Rgba::from(crate::theme::vars::active_theme(cx).primary_foreground));
    for (x, y) in [(-2, -6), (2, -6), (-2, 2), (2, 2)] {
        assert!(
            selected
                .get_pixel((left as i32 + x) as u32, (top as i32 + 360 + y) as u32)
                .0[..3]
                .iter()
                .zip([foreground.r, foreground.g, foreground.b])
                .all(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs() <= 2.),
            "the selected bottom-left handle must remain visible at {x},{y}"
        );
    }
    let start = bounds.origin + point(px(40. * 0.5), px(330. * 0.5));
    let end = bounds.origin + point(px(50. * 0.5), px(320. * 0.5));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    event(
        cx,
        window,
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert_eq!(
            editor.history.current()[0].bounds(),
            (0., 320., 50., 360.),
            "the framed annotation handle must resize the shape"
        );
    });
    let start = bounds.origin + point(px(-1.), px(180.5));
    let end = bounds.origin + point(px(5.), px(175.5));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    event(
        cx,
        window,
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().history.current()[0].bounds(),
            (10., 320., 50., 350.),
            "the part of the handle outside the image must resize the shape"
        );
    });
    editor
        .update(cx, |editor, _, cx| {
            editor.history.push(Vec::new());
            editor.tool = crate::ui::colors::Tool::Crop;
            cx.notify();
        })
        .unwrap();
    let start = bounds.origin + point(px(0.5), px(0.5));
    let end = bounds.origin + point(px(35.), px(25.));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    event(
        cx,
        window,
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().snapshot.borrow().crop,
            Some((1., 1., 69., 49.)),
            "the framed crop must track the pointer"
        );
    });
    let cropped = cx.capture_screenshot(window).unwrap();
    for (x, y) in [(-2, -2), (2, -2), (-2, 2), (2, 2)] {
        assert!(
            cropped
                .get_pixel((left as i32 + x) as u32, (top as i32 + y) as u32)
                .0[..3]
                .iter()
                .zip([foreground.r, foreground.g, foreground.b])
                .all(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs() <= 2.),
            "the crop handle must remain visible at {x},{y}"
        );
    }
    let transparent_wallpaper = scratch.join("transparent-wallpaper.png");
    image::RgbaImage::from_fn(32, 32, |x, y| {
        image::Rgba([100, 150, 200, [0, 128, 255][((x / 8 + y / 8) % 3) as usize]])
    })
    .save(&transparent_wallpaper)
    .unwrap();
    editor
        .update(cx, |editor, _, cx| {
            editor.cancel_crop(cx);
            editor
                .wallpaper
                .set_background_image(Some(transparent_wallpaper.to_string_lossy().into_owned()));
            editor.wallpaper.padding = 20.;
            cx.notify();
        })
        .unwrap();
    let original_mode = cx.update(|cx| crate::theme::vars::active_mode(cx));
    for mode in [
        crate::theme::presets::ThemeMode::Light,
        crate::theme::presets::ThemeMode::Dark,
    ] {
        cx.update(|cx| crate::theme::vars::update_theme(cx, mode, "default"));
        for (scale, style, shadow, blur) in [
            (1.25_f32, "windows-dark", 0., 0.),
            (2., "macos-light", 50., 0.),
            (1.25, "macos-light", 0., 30.),
            (2., "windows-dark", 50., 30.),
            (1.25, "none", 0., 0.),
            (1.25, "none", 50., 0.),
            (2., "none", 50., 30.),
            (2., "none", 0., 30.),
        ] {
            editor
                .update(cx, |editor, _, cx| {
                    editor.wallpaper.window_frame.style = style.into();
                    editor.wallpaper.corners = if style == "none" { 24. } else { 0. };
                    editor.wallpaper.shadow = shadow;
                    editor.wallpaper.background_blur = blur;
                    cx.notify();
                })
                .unwrap();
            cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
                .unwrap();
            settle(cx, window);
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = 1. / scale;
                    cx.notify();
                })
                .unwrap();
            settle(cx, window);
            let (bounds, saved, background) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                let background = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
                (
                    editor.bounds.borrow().unwrap(),
                    image::load_from_memory(&editor.export_png().unwrap())
                        .unwrap()
                        .to_rgba8(),
                    image::Rgba([
                        (background.r * 255.).round() as u8,
                        (background.g * 255.).round() as u8,
                        (background.b * 255.).round() as u8,
                        255,
                    ]),
                )
            });
            let mut expected =
                image::RgbaImage::from_pixel(saved.width(), saved.height(), background);
            image::imageops::overlay(&mut expected, &saved, 0, 0);
            let screenshot = cx.capture_screenshot(window).unwrap();
            let left = (f32::from(bounds.left()) * scale).round() as u32;
            let top = (f32::from(bounds.top()) * scale).round() as u32;
            let padding = (20. * scale).round() as u32;
            let title = if style == "none" {
                0
            } else {
                (28. * scale).round() as u32
            };
            let mismatched = (0..360)
                .flat_map(|y| (0..640).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(expected.get_pixel(padding + x, padding + title + y).0)
                        .any(|(actual, expected)| {
                            actual.abs_diff(expected)
                                > if style != "none" && (!(20..620).contains(&x) || y >= 340) {
                                    2
                                } else {
                                    3
                                }
                        })
                })
                .count();
            if mismatched > 0 {
                image::imageops::crop_imm(&screenshot, left, top, 640, 360)
                    .to_image()
                    .save(scratch.join("transparent-custom-preview.png"))
                    .unwrap();
                image::imageops::crop_imm(&expected, padding, padding + title, 640, 360)
                    .to_image()
                    .save(scratch.join("transparent-custom-expected.png"))
                    .unwrap();
            }
            assert_eq!(
                mismatched, 0,
                "transparent custom framed wallpaper differs at {scale}x, {style}, shadow {shadow}, blur {blur}, {mode:?}"
            );
            if style == "none" && shadow > 0. {
                let mismatched = (0..expected.height())
                    .flat_map(|y| (0..expected.width()).map(move |x| (x, y)))
                    .filter(|&(x, y)| {
                        screenshot
                            .get_pixel(left - padding + x, top - padding + y)
                            .0
                            .iter()
                            .zip(expected.get_pixel(x, y).0)
                            .any(|(actual, expected)| actual.abs_diff(expected) > 3)
                    })
                    .count();
                assert_eq!(
                    mismatched, 0,
                    "transparent custom shadow differs at {scale}x, blur {blur}, {mode:?}"
                );
            }
        }
    }
    cx.update(|cx| crate::theme::vars::update_theme(cx, original_mode, "default"));
    editor
        .update(cx, |editor, _, cx| {
            editor.wallpaper.set_gradient(None);
            editor.wallpaper.padding = 20.;
            editor.wallpaper.shadow = 0.;
            editor.wallpaper.corners = 0.;
            editor.wallpaper.window_frame.style = "macos-light".into();
            cx.notify();
        })
        .unwrap();
    cx.update_window(window, |_, window, _| window.set_scale_factor(1.25))
        .unwrap();
    settle(cx, window);
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 1. / 1.25;
            cx.notify();
        })
        .unwrap();
    cx.advance_clock(std::time::Duration::from_millis(32));
    settle(cx, window);
    let (bounds, saved) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        (
            editor.bounds.borrow().unwrap(),
            image::load_from_memory(&editor.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
        )
    });
    let screenshot = cx.capture_screenshot(window).unwrap();
    let left = (f32::from(bounds.left()) * 1.25).round() as u32;
    let top = (f32::from(bounds.top()) * 1.25).round() as u32;
    let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), stage_matte);
    image::imageops::overlay(&mut expected, &saved, 0, 0);
    for y in 0..360 {
        for x in 0..640 {
            assert!(
                screenshot
                    .get_pixel(left + x, top + y)
                    .0
                    .iter()
                    .zip(expected.get_pixel(25 + x, 25 + 35 + y).0)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                "transparent framed preview differs at {x},{y}"
            );
        }
    }
    for y in 120..180 {
        for x in 240..320 {
            let pixel = saved.get_pixel(25 + x, 25 + 35 + y).0;
            assert_eq!(pixel[3], 128);
            for (actual, (foreground, background)) in screenshot.get_pixel(left + x, top + y).0[..3]
                .iter()
                .zip(pixel[..3].iter().zip(stage_matte.0[..3].iter().copied()))
            {
                let blended = (u16::from(*foreground) * 128 + u16::from(background) * 127) / 255;
                assert!(actual.abs_diff(blended as u8) <= 2);
            }
        }
    }
    for scale in [1.25_f32, 2.] {
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.padding = 0.;
                editor.wallpaper.window_frame.style = "windows-dark".into();
                cx.notify();
            })
            .unwrap();
        cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        cx.advance_clock(std::time::Duration::from_millis(32));
        settle(cx, window);
        let (bounds, saved) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                image::load_from_memory(&editor.export_png().unwrap())
                    .unwrap()
                    .to_rgba8(),
            )
        });
        let title = (28. * scale).round() as u32;
        assert_eq!(saved.dimensions(), (640, 360 + title));
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32;
        let top = (f32::from(bounds.top()) * scale).round() as u32;
        assert_eq!(saved.get_pixel(320, 10).0, [32, 32, 32, 255]);
        assert_eq!(
            screenshot.get_pixel(left + 320, top - title + 10),
            saved.get_pixel(320, 10)
        );
        let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), stage_matte);
        image::imageops::overlay(&mut expected, &saved, 0, 0);
        for y in 0..360 {
            for x in 0..640 {
                assert!(
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(expected.get_pixel(x, title + y).0)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                    "frame-only preview differs at {x},{y}, {scale}x"
                );
            }
        }
        for y in 120..180 {
            for x in 240..320 {
                let pixel = saved.get_pixel(x, title + y).0;
                assert_eq!(pixel[3], 128);
                for (actual, (foreground, background)) in screenshot.get_pixel(left + x, top + y).0
                    [..3]
                    .iter()
                    .zip(pixel[..3].iter().zip(stage_matte.0[..3].iter().copied()))
                {
                    let blended =
                        (u16::from(*foreground) * 128 + u16::from(background) * 127) / 255;
                    assert!(actual.abs_diff(blended as u8) <= 2);
                }
            }
        }
    }
    let side = scratch.join("wallpaper-layer-side.png");
    let vertical = scratch.join("wallpaper-layer-vertical.png");
    for (path, width, height) in [(&side, 120, 360), (&vertical, 640, 90)] {
        image::RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([
                40,
                190,
                80,
                if (x / 30 + y / 30) % 3 == 0 { 128 } else { 255 },
            ])
        })
        .save(path)
        .unwrap();
    }
    for (name, scale, shadow, corners, spacing, background, aspect) in [
        ("plain125", 1.25_f32, 0., 0., 0., "none", "auto"),
        ("plain200", 2., 0., 0., 0., "none", "auto"),
        ("zero125", 1.25_f32, 50., 24., 0., "none", "auto"),
        ("zero200", 2., 50., 24., 0., "none", "auto"),
        ("right", 1.25_f32, 50., 24., 10., "solid", "auto"),
        ("left", 2., 50., 24., 0., "solid", "auto"),
        ("top", 1.25, 0., 24., 0., "none", "auto"),
        ("bottom", 2., 50., 0., 10., "solid", "auto"),
        ("wide", 1.25, 50., 24., 10., "solid", "21:9"),
        ("tall", 1.25, 50., 24., 10., "solid", "4:3"),
        ("stacked", 1.25, 50., 24., 0., "alpha", "auto"),
    ] {
        let edges = match name {
            "plain125" | "plain200" | "zero125" | "zero200" | "right" | "wide" | "tall" => {
                vec![crate::editor::layers::Edge::Right]
            }
            "left" => vec![crate::editor::layers::Edge::Left],
            "top" => vec![crate::editor::layers::Edge::Top],
            "bottom" => vec![crate::editor::layers::Edge::Bottom],
            _ => vec![
                crate::editor::layers::Edge::Left,
                crate::editor::layers::Edge::Right,
                crate::editor::layers::Edge::Top,
                crate::editor::layers::Edge::Bottom,
                crate::editor::layers::Edge::Right,
            ],
        };
        #[cfg(windows)]
        let old_images = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            let snapshot = editor.snapshot.borrow();
            snapshot
                .preview_edges
                .iter()
                .map(|edge| edge.image.clone())
                .chain(snapshot.backdrop.iter().cloned())
                .collect::<Vec<_>>()
        });
        #[cfg(windows)]
        if !old_images.is_empty() {
            cx.update_window(window, |_, window, _| {
                assert!(
                    old_images
                        .iter()
                        .any(|image| window.has_image_atlas_entry(image)),
                    "the previous layered images must have been painted"
                );
            })
            .unwrap();
        }
        editor
            .update(cx, |editor, _, cx| {
                editor.history.push(Vec::new());
                editor.clear_layers(cx);
                if background == "alpha" {
                    editor.wallpaper.set_background_image(Some(
                        transparent_wallpaper.to_string_lossy().into_owned(),
                    ));
                } else {
                    editor
                        .wallpaper
                        .set_gradient((background == "solid").then(|| {
                            crate::editor::wallpaper::GradientOption {
                                id: "visual-solid".into(),
                                colors: vec!["#2468ac".into()],
                                angle: 0.,
                            }
                        }));
                }
                editor.wallpaper.window_frame.style = "none".into();
                editor.wallpaper.padding = if name.starts_with("zero") || name.starts_with("plain")
                {
                    0.
                } else {
                    20.
                };
                editor.wallpaper.spacing = spacing;
                editor.wallpaper.corners = corners;
                editor.wallpaper.shadow = shadow;
                editor.wallpaper.aspect_ratio = aspect.into();
                editor.wallpaper.background_blur = if background == "alpha" { 30. } else { 0. };
                for edge in edges {
                    let (path, width, height) = match edge {
                        crate::editor::layers::Edge::Left | crate::editor::layers::Edge::Right => {
                            (&side, 120., 360.)
                        }
                        _ => (&vertical, 640., 90.),
                    };
                    editor.push_image_layer(
                        path.to_string_lossy().into_owned(),
                        width,
                        height,
                        edge,
                        cx,
                    );
                }
                cx.notify();
            })
            .unwrap();
        cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        #[cfg(windows)]
        cx.update_window(window, |_, window, _| {
            for image in &old_images {
                assert!(
                    !window.has_image_atlas_entry(image),
                    "replaced layered images must leave the GPU atlas"
                );
            }
        })
        .unwrap();
        let (bounds, saved, offset_x, offset_y, matte, rectangles) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            let wallpaper = editor.wallpaper.for_scale(f64::from(scale));
            let group = crate::editor::layers::compute(
                640.,
                360.,
                &editor.snapshot.borrow().layers,
                wallpaper.spacing,
            );
            let (_, (x, y, _, _)) = crate::editor::wallpaper::layout(
                &wallpaper,
                group.width,
                group.height,
                f64::from(scale),
                true,
            );
            let matte = stage_matte;
            let rectangles = std::iter::once(group.primary)
                .chain(group.layers.iter().map(|(_, rect)| *rect))
                .map(|rect| {
                    (
                        x.round() + rect.x,
                        y.round() + rect.y,
                        rect.width,
                        rect.height,
                    )
                })
                .collect::<Vec<_>>();
            (
                editor.bounds.borrow().unwrap(),
                image::load_from_memory(&editor.export_png().unwrap())
                    .unwrap()
                    .to_rgba8(),
                (x.round() + group.primary.x).round() as u32,
                (y.round() + group.primary.y).round() as u32,
                matte,
                rectangles,
            )
        });
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32 - offset_x;
        let top = (f32::from(bounds.top()) * scale).round() as u32 - offset_y;
        assert!(
            left + saved.width() <= screenshot.width()
                && top + saved.height() <= screenshot.height(),
            "{name} composition must fit in the viewport"
        );
        let preview =
            image::imageops::crop_imm(&screenshot, left, top, saved.width(), saved.height())
                .to_image();
        let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
        image::imageops::overlay(&mut expected, &saved, 0, 0);
        let mut mismatched = 0;
        let mut overlap_rounding = 0;
        let shadow_extent = crate::editor::export::image_shadow(shadow).map_or(0., |shadow| {
            (crate::render::blur::box_radius(crate::render::blur::sigma_for_shadow_blur(
                shadow.blur,
            )) * 3
                + 2) as f64
                + f64::from(shadow.offset_x.abs().max(shadow.offset_y.abs()).ceil())
                + 2.
        });
        for (index, (actual, expected)) in preview.pixels().zip(expected.pixels()).enumerate() {
            let difference = actual
                .0
                .iter()
                .zip(expected.0)
                .map(|(actual, expected)| actual.abs_diff(expected))
                .max()
                .unwrap();
            if difference <= if background == "alpha" { 3 } else { 2 } {
                continue;
            }
            let x = (index as u32 % saved.width()) as f64;
            let y = (index as u32 / saved.width()) as f64;
            let overlap = rectangles
                .iter()
                .filter(|&&(left, top, width, height)| {
                    x >= left - shadow_extent
                        && x < left + width + shadow_extent
                        && y >= top - shadow_extent
                        && y < top + height + shadow_extent
                })
                .count();
            if name == "stacked" && difference == 4 && overlap >= 2 {
                overlap_rounding += 1;
            } else {
                mismatched += 1;
            }
        }
        if mismatched > 0 {
            preview
                .save(scratch.join(format!("layered-wallpaper-{name}-preview.png")))
                .unwrap();
            expected
                .save(scratch.join(format!("layered-wallpaper-{name}-export.png")))
                .unwrap();
        }
        assert_eq!(
            mismatched, 0,
            "{name} attached wallpaper preview differs from export at {scale}x"
        );
        assert!(
            overlap_rounding <= 16,
            "{name} has {overlap_rounding} four-level overlap differences"
        );
        if name.starts_with("plain") {
            editor
                .update(cx, |editor, _, cx| {
                    editor.history.push(vec![
                        crate::editor::annotations::Annotation::Rectangle {
                            id: "plain-edge".into(),
                            x: 620.,
                            y: 140.,
                            width: 30.,
                            height: 30.,
                            stroke: "#ff00ff".into(),
                            stroke_width: 1.,
                            fill: Some("#ff00ff".into()),
                        },
                        crate::editor::annotations::Annotation::Redact {
                            id: "plain-black".into(),
                            x: 625.,
                            y: 200.,
                            width: 30.,
                            height: 30.,
                            style: "black".into(),
                            intensity: 5.,
                        },
                        crate::editor::annotations::Annotation::Redact {
                            id: "plain-pixelate".into(),
                            x: 623.,
                            y: 240.,
                            width: 30.,
                            height: 30.,
                            style: "pixelate".into(),
                            intensity: 10.,
                        },
                        crate::editor::annotations::Annotation::Redact {
                            id: "plain-blur".into(),
                            x: 625.,
                            y: 280.,
                            width: 30.,
                            height: 30.,
                            style: "blur".into(),
                            intensity: 2.,
                        },
                    ]);
                    editor.tool = crate::ui::colors::Tool::Select;
                    cx.notify();
                })
                .unwrap();
            settle(cx, window);
            let annotated = cx.update(|cx| {
                image::load_from_memory(&editor.read(cx).unwrap().export_png().unwrap())
                    .unwrap()
                    .to_rgba8()
            });
            assert_eq!(
                annotated.get_pixel(offset_x + 645, offset_y + 150).0,
                [255, 0, 255, 255]
            );
            assert_eq!(
                annotated.get_pixel(offset_x + 645, offset_y + 215).0,
                [0, 0, 0, 255]
            );
            let mut expected =
                image::RgbaImage::from_pixel(annotated.width(), annotated.height(), matte);
            image::imageops::overlay(&mut expected, &annotated, 0, 0);
            let screenshot = cx.capture_screenshot(window).unwrap();
            for (start_x, start_y, width, height) in [
                (620, 140, 30, 30),
                (625, 200, 30, 30),
                (623, 240, 30, 30),
                (625, 280, 30, 30),
            ] {
                for y in start_y..start_y + height {
                    for x in start_x..start_x + width {
                        let actual = screenshot
                            .get_pixel(left + offset_x + x, top + offset_y + y)
                            .0;
                        let expected_pixel = expected.get_pixel(offset_x + x, offset_y + y).0;
                        assert!(
                            actual.iter()
                                .zip(expected_pixel)
                                .all(|(actual, expected)| actual.abs_diff(expected) <= 4),
                            "{name} annotation differs at {x},{y}: {actual:?} vs {expected_pixel:?}"
                        );
                    }
                }
            }
            for start_y in [200, 240, 280] {
                assert!(
                    (start_y..start_y + 30).any(|y| (625..655).any(|x| {
                        annotated.get_pixel(offset_x + x, offset_y + y)
                            != saved.get_pixel(offset_x + x, offset_y + y)
                    })),
                    "{name} redaction at y={start_y} must change the saved image"
                );
            }
        }
    }
    let unannotated = cx.update(|cx| {
        image::load_from_memory(&editor.read(cx).unwrap().export_png().unwrap())
            .unwrap()
            .to_rgba8()
    });
    editor
        .update(cx, |editor, _, cx| {
            editor.history.push(vec![
                crate::editor::annotations::Annotation::Rectangle {
                    id: "layered-corner".into(),
                    x: 0.,
                    y: 0.,
                    width: 40.,
                    height: 30.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: Some("#ff00ff".into()),
                },
                crate::editor::annotations::Annotation::Rectangle {
                    id: "layered-edge".into(),
                    x: 620.,
                    y: 140.,
                    width: 30.,
                    height: 30.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: Some("#ff00ff".into()),
                },
                crate::editor::annotations::Annotation::Redact {
                    id: "layered-black".into(),
                    x: 625.,
                    y: 200.,
                    width: 30.,
                    height: 30.,
                    style: "black".into(),
                    intensity: 5.,
                },
                crate::editor::annotations::Annotation::Redact {
                    id: "layered-pixelate".into(),
                    x: 625.,
                    y: 240.,
                    width: 30.,
                    height: 30.,
                    style: "pixelate".into(),
                    intensity: 10.,
                },
                crate::editor::annotations::Annotation::Redact {
                    id: "layered-blur".into(),
                    x: 625.,
                    y: 280.,
                    width: 30.,
                    height: 30.,
                    style: "blur".into(),
                    intensity: 2.,
                },
            ]);
            editor.tool = crate::ui::colors::Tool::Select;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let (bounds, png, layers, annotations, offset_x, offset_y, matte) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let wallpaper = editor.wallpaper.for_scale(1.25);
        let layers = editor.snapshot.borrow().layers.clone();
        let group = crate::editor::layers::compute(640., 360., &layers, wallpaper.spacing);
        let (_, (x, y, _, _)) =
            crate::editor::wallpaper::layout(&wallpaper, group.width, group.height, 1.25, true);
        let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
        (
            editor.bounds.borrow().unwrap(),
            editor.export_png().unwrap(),
            layers,
            editor.history.current().to_vec(),
            (x.round() + group.primary.x).round() as u32,
            (y.round() + group.primary.y).round() as u32,
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ]),
        )
    });
    let saved = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(
        saved.get_pixel(offset_x + 630, offset_y + 150).0,
        [255, 0, 255, 255]
    );
    assert_eq!(
        saved.get_pixel(offset_x + 645, offset_y + 150).0,
        [255, 0, 255, 255]
    );
    assert_eq!(
        saved.get_pixel(offset_x + 645, offset_y + 215).0,
        [0, 0, 0, 255]
    );
    for start_y in [200, 240, 280] {
        assert!(
            (start_y..start_y + 30).any(|y| (625..655).any(|x| {
                saved.get_pixel(offset_x + x, offset_y + y)
                    != unannotated.get_pixel(offset_x + x, offset_y + y)
            })),
            "redaction at y={start_y} must change the saved image"
        );
    }
    let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
    image::imageops::overlay(&mut expected, &saved, 0, 0);
    let screenshot = cx.capture_screenshot(window).unwrap();
    let left = (f32::from(bounds.left()) * 1.25).round() as u32;
    let top = (f32::from(bounds.top()) * 1.25).round() as u32;
    for (start_x, start_y, width, height) in [
        (0, 0, 40, 30),
        (620, 140, 35, 30),
        (625, 200, 30, 30),
        (625, 240, 30, 30),
        (625, 280, 30, 30),
    ] {
        for y in start_y..start_y + height {
            for x in start_x..start_x + width {
                assert!(
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(expected.get_pixel(offset_x + x, offset_y + y).0)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 4),
                    "layered annotation differs at {x},{y}"
                );
            }
        }
    }
    cx.update_window(window, |_, window, _| window.set_scale_factor(2.))
        .unwrap();
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 0.5;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let (bounds_2x, saved_2x, offset_x_2x, offset_y_2x, matte_2x) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let wallpaper = editor.wallpaper.for_scale(2.);
        let group = crate::editor::layers::compute(
            640.,
            360.,
            &editor.snapshot.borrow().layers,
            wallpaper.spacing,
        );
        let (_, (x, y, _, _)) =
            crate::editor::wallpaper::layout(&wallpaper, group.width, group.height, 2., true);
        let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
        (
            editor.bounds.borrow().unwrap(),
            image::load_from_memory(&editor.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
            (x.round() + group.primary.x).round() as u32,
            (y.round() + group.primary.y).round() as u32,
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ]),
        )
    });
    assert_eq!(
        saved_2x.get_pixel(offset_x_2x + 645, offset_y_2x + 150).0,
        [255, 0, 255, 255]
    );
    assert_eq!(
        saved_2x.get_pixel(offset_x_2x + 645, offset_y_2x + 215).0,
        [0, 0, 0, 255]
    );
    let mut expected_2x =
        image::RgbaImage::from_pixel(saved_2x.width(), saved_2x.height(), matte_2x);
    image::imageops::overlay(&mut expected_2x, &saved_2x, 0, 0);
    let screenshot_2x = cx.capture_screenshot(window).unwrap();
    let left_2x = (f32::from(bounds_2x.left()) * 2.).round() as u32;
    let top_2x = (f32::from(bounds_2x.top()) * 2.).round() as u32;
    for (start_x, start_y, width, height) in [
        (0, 0, 40, 30),
        (620, 140, 35, 30),
        (625, 200, 30, 30),
        (625, 240, 30, 30),
        (625, 280, 30, 30),
    ] {
        for y in start_y..start_y + height {
            for x in start_x..start_x + width {
                assert!(
                    screenshot_2x
                        .get_pixel(left_2x + x, top_2x + y)
                        .0
                        .iter()
                        .zip(expected_2x.get_pixel(offset_x_2x + x, offset_y_2x + y).0)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 4),
                    "layered 2x annotation differs at {x},{y}"
                );
            }
        }
    }
    cx.update_window(window, |_, window, _| window.set_scale_factor(1.25))
        .unwrap();
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 0.8;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    super::click(
        cx,
        window,
        f32::from(bounds.left()) + 8.,
        f32::from(bounds.top()) + 12.,
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().snapshot.borrow().selected,
            ["layered-corner"]
        );
    });
    let selected = cx.capture_screenshot(window).unwrap();
    let foreground =
        cx.update(|cx| gpui::Rgba::from(crate::theme::vars::active_theme(cx).primary_foreground));
    for (dx, dy) in [(-2, -2), (2, -2), (-2, 2), (2, 2)] {
        assert!(
            selected
                .get_pixel((left as i32 + dx) as u32, (top as i32 + dy) as u32)
                .0[..3]
                .iter()
                .zip([foreground.r, foreground.g, foreground.b])
                .all(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs() <= 2.),
            "layered selection handle is clipped at {dx},{dy}"
        );
    }
    cx.update_window(window, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    let reopened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.25);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, reopened.into());
    cx.update(|cx| {
        let reopened = reopened.read(cx).unwrap();
        assert_eq!(reopened.snapshot.borrow().layers, layers);
        assert_eq!(reopened.history.current(), annotations);
        assert_eq!(reopened.export_png().unwrap(), png);
    });
    reopened
        .update(cx, |editor, _, cx| {
            editor.tool = crate::ui::colors::Tool::Crop;
            editor.zoom = 0.8;
            cx.notify();
        })
        .unwrap();
    settle(cx, reopened.into());
    let (crop_bounds, old_backdrop) = cx.update(|cx| {
        let editor = reopened.read(cx).unwrap();
        (
            editor.bounds.borrow().unwrap(),
            editor.snapshot.borrow().backdrop.clone().unwrap(),
        )
    });
    let start = crop_bounds.origin + point(px(8.), px(8.));
    let end = crop_bounds.origin + point(px(208.), px(128.));
    event(
        cx,
        reopened.into(),
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    event(
        cx,
        reopened.into(),
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, reopened.into());
    let (expected_width, expected_height) = cx.update(|cx| {
        let editor = reopened.read(cx).unwrap();
        let (_, _, width, height) = editor.snapshot.borrow().crop.unwrap();
        (width.abs() as u32, height.abs() as u32)
    });
    assert!((249..=250).contains(&expected_width) && (149..=150).contains(&expected_height));
    reopened
        .update(cx, |editor, _, cx| editor.apply_crop(cx))
        .unwrap();
    settle(cx, reopened.into());
    reopened
        .update(cx, |editor, _, cx| {
            editor.zoom = 0.8;
            cx.notify();
        })
        .unwrap();
    settle(cx, reopened.into());
    let (cropped_bounds, cropped_png, cropped_offset_x, cropped_offset_y) = cx.update(|cx| {
        let editor = reopened.read(cx).unwrap();
        assert_eq!(
            (editor.image_width as u32, editor.image_height as u32),
            (expected_width, expected_height)
        );
        assert!(editor.snapshot.borrow().backdrop_composite);
        assert_ne!(
            editor.snapshot.borrow().backdrop.as_ref().unwrap().id,
            old_backdrop.id
        );
        let snapshot = editor.snapshot.borrow();
        let wallpaper = &snapshot.wallpaper;
        let group = crate::editor::layers::compute(
            f64::from(expected_width),
            f64::from(expected_height),
            &snapshot.layers,
            wallpaper.spacing,
        );
        let (_, (x, y, _, _)) = crate::editor::wallpaper::layout(
            wallpaper,
            group.width,
            group.height,
            snapshot.frame_scale,
            true,
        );
        (
            editor.bounds.borrow().unwrap(),
            image::load_from_memory(&editor.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
            (x.round() + group.primary.x).round() as u32,
            (y.round() + group.primary.y).round() as u32,
        )
    });
    #[cfg(windows)]
    cx.update_window(reopened.into(), |_, window, _| {
        assert!(
            !window.has_image_atlas_entry(&old_backdrop),
            "the pre-crop composite must leave the GPU atlas"
        );
    })
    .unwrap();
    let mut cropped_expected =
        image::RgbaImage::from_pixel(cropped_png.width(), cropped_png.height(), matte);
    image::imageops::overlay(&mut cropped_expected, &cropped_png, 0, 0);
    let cropped_screenshot = cx.capture_screenshot(reopened.into()).unwrap();
    let cropped_left = (f32::from(cropped_bounds.left()) * 1.25).round() as u32;
    let cropped_top = (f32::from(cropped_bounds.top()) * 1.25).round() as u32;
    for y in 0..expected_height {
        for x in 0..expected_width {
            assert!(
                cropped_screenshot
                    .get_pixel(cropped_left + x, cropped_top + y)
                    .0
                    .iter()
                    .zip(
                        cropped_expected
                            .get_pixel(cropped_offset_x + x, cropped_offset_y + y)
                            .0
                    )
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 4),
                "cropped layered preview differs at {x},{y}"
            );
        }
    }
    cx.update_window(reopened.into(), |_, window, cx| {
        window.dispatch_action(Box::new(crate::editor::actions::Undo), cx);
    })
    .unwrap();
    settle(cx, reopened.into());
    cx.update(|cx| {
        let editor = reopened.read(cx).unwrap();
        assert_eq!((editor.image_width, editor.image_height), (640., 360.));
        assert_eq!(editor.export_png().unwrap(), png);
    });
    cx.update_window(reopened.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    println!(
        "wallpaper, rounded and framed previews match saved pixels at 1.25x and 2x, including transparent content and edge annotations"
    );
}

#[cfg(windows)]
pub(super) fn video(
    cx: &mut HeadlessAppContext,
    output: &Path,
    scratch: &Path,
    width: f32,
    scale: f32,
) {
    use crate::windows::registry::{self, WindowKind};
    use crate::windows::video_editor::{model, preview, sidebar::SidebarTab, VideoEditorWindow};

    let project = scratch.join("Visual regression.poratake");
    std::fs::create_dir_all(&project).unwrap();
    let recording = crate::video::project::recording_video_path(&project);
    std::fs::write(&recording, include_bytes!("fixtures/capture.mov")).unwrap();
    let mut state = model::VideoEditorState {
        source_duration: Some(2.),
        segments: vec![model::Segment::spanning(2.)],
        ..Default::default()
    };
    state.drawing_segments.push(model::DrawingSegment {
        id: "visual-drawing".into(),
        start_time: 0.,
        end_time: 2.,
        canvas_width: 320.,
        canvas_height: 180.,
        annotations: vec![crate::editor::annotations::Annotation::Rectangle {
            id: "visual-shape".into(),
            x: 24.,
            y: 24.,
            width: 72.,
            height: 48.,
            stroke: "#ffffff".into(),
            stroke_width: 3.,
            fill: Some("#e04040".into()),
        }],
    });
    model::save_state(&project, &state);
    let video = cx.update(|cx| {
        VideoEditorWindow::open(cx, Some(project.to_string_lossy().into_owned()));
        registry::handle(WindowKind::VideoEditor, cx)
            .unwrap()
            .downcast::<VideoEditorWindow>()
            .unwrap()
    });
    let window = video.into();
    cx.update_window(window, |_, window, _| {
        window.resize(size(px(width.max(1200.)), px(750.)));
        window.set_scale_factor(scale);
    })
    .unwrap();
    settle(cx, window);
    cx.update(|cx| {
        video
            .update(cx, |editor, _, cx| {
                editor.update_wallpaper(cx, |wallpaper| {
                    wallpaper.enabled = true;
                    wallpaper.padding = 12.;
                    wallpaper.corners = 8.;
                });
                editor.update_export(cx, |export| export.resolution = "original".into());
            })
            .unwrap();
    });
    cx.run_until_parked();
    cx.advance_clock(std::time::Duration::from_secs(1));
    settle(cx, window);
    let saved = model::load_state(&project);
    assert!(saved.wallpaper.enabled);
    assert_eq!(saved.wallpaper.padding, 12.);
    let mut expected = preview::Source::open(&project, saved).unwrap();
    let expected = preview::to_render_image(&expected.compose(0., None).unwrap()).unwrap();
    cx.update(|cx| {
        let actual = video.read(cx).unwrap().preview_image().unwrap();
        assert_eq!(actual.size(0), expected.size(0));
        assert_eq!(actual.as_bytes(0), expected.as_bytes(0));
    });
    hover(cx, window, 300., 300.);
    capture(cx, window, output, "video-loaded-default");
    for tab in SidebarTab::ALL {
        cx.update(|cx| {
            video
                .update(cx, |editor, _, cx| editor.activate_tab(tab, cx))
                .unwrap();
        });
        cx.run_until_parked();
        cx.advance_clock(std::time::Duration::from_secs(1));
        settle(cx, window);
        let saved = model::load_state(&project);
        assert!(saved.ui.sidebar_open);
        assert_eq!(saved.ui.sidebar_tab, tab.id());
        capture(cx, window, output, &format!("video-loaded-{}", tab.id()));
    }
    cx.update(|cx| {
        video
            .update(cx, |editor, _, cx| {
                editor.begin_export_for_parity(cx);
                editor.cancel_export(cx);
            })
            .unwrap();
    });
    settle(cx, window);
    capture(cx, window, output, "video-loaded-export-canceling");
    let footer = super::measured("video-export-panel", 1);
    let status = super::measured("video-export-canceling", 0);
    assert!(status.left() >= footer.left() && status.right() <= footer.right());
    assert!(status.top() >= footer.top() && status.bottom() <= footer.bottom());
    let viewport = cx
        .update_window(window, |_, window, _| window.viewport_size())
        .unwrap();
    assert!(footer.left() >= px(0.) && footer.right() <= viewport.width);
    assert!(footer.top() >= px(0.) && footer.bottom() <= viewport.height);
    cx.update(|cx| registry::close(WindowKind::VideoEditor, cx));
}
