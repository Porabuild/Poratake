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
                    config.screenshot.auto_copy_to_clipboard = false;
                    config.all_in_one.remember_choices = true;
                    config.all_in_one.last_mode = "screenshot".into();
                    config.all_in_one.last_targets.screenshot = "screen".into();
                });
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
