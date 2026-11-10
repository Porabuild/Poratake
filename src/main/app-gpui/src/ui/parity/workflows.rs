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

pub(super) fn area_picker(cx: &mut HeadlessAppContext) -> gpui::AnyWindowHandle {
    let picker = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .find(|window| {
                window
                    .downcast::<crate::capture::overlay::AreaOverlay>()
                    .is_some()
            })
            .unwrap()
    });
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
                    config.screenshot.show_preview = false;
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
    use crate::video::encoder::{Encoder, Settings};
    use crate::windows::registry::{self, WindowKind};
    use crate::windows::video_editor::{model, preview, sidebar::SidebarTab, VideoEditorWindow};

    let project = scratch.join("Visual regression.poratake");
    std::fs::create_dir_all(&project).unwrap();
    let recording = crate::video::project::recording_video_path(&project);
    let mut encoder = Encoder::create(
        &recording,
        Settings {
            width: 320,
            height: 180,
            frame_rate: 12,
            bitrate: 4_000_000,
            has_audio: false,
        },
    )
    .unwrap();
    let mut pixels = image::open(scratch.join("capture.png"))
        .unwrap()
        .resize_exact(320, 180, image::imageops::FilterType::Nearest)
        .to_rgba8()
        .into_raw();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    for _ in 0..24 {
        encoder.write_frame(pixels.clone()).unwrap();
    }
    encoder.finish().unwrap();
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
    cx.update(|cx| registry::close(WindowKind::VideoEditor, cx));
}
