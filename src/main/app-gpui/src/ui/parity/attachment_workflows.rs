use base64::Engine as _;
use gpui::AppContext;
use herogpui::gpui;

use crate::editor::layers::Edge;
use crate::editor::window::EditorWindow;
use crate::history_store::{HistoryItem, HistoryItemType};
use crate::system::clipboard::ClipboardService;

use super::daemon_fixture::calls;
use super::workflows::{area_picker, select_area, SELECTED_RECT};
use super::{
    capture, escape, event, hover, measured, px, settle, size, take_notifications,
    HeadlessAppContext, Modifiers, MouseButton, MouseDownEvent, Path, PlatformInput,
};

pub(super) fn check_edge_overlay(
    cx: &mut HeadlessAppContext,
    editor: gpui::WindowHandle<EditorWindow>,
    output: &Path,
    scale: f32,
) {
    let window = editor.into();
    editor
        .update(cx, |editor, _, cx| editor.toggle_capture_mode(cx))
        .unwrap();
    settle(cx, window);
    let top = measured("capture-edges", 0);
    let left = measured("capture-edges", 2);
    for index in 0..4 {
        let bounds = measured("capture-edges", index);
        let thickness = if index < 2 {
            bounds.size.height
        } else {
            bounds.size.width
        };
        assert_eq!(thickness, px(32.0), "edge {index} hit target thickness");
    }
    hover(
        cx,
        window,
        f32::from(top.center().x),
        f32::from(left.center().y),
    );
    capture(cx, window, output, "attachment-edges-default");
    let idle = cx.capture_screenshot(window).unwrap();
    for index in 0..4 {
        let bounds = measured("capture-edges", index);
        let center = bounds.center();
        hover(cx, window, f32::from(center.x), f32::from(center.y));
        let hovered = cx.capture_screenshot(window).unwrap();
        if index == 3 {
            capture(cx, window, output, "attachment-edges-hover");
        }
        let x = (f32::from(center.x) * scale) as u32;
        let y = (f32::from(center.y) * scale) as u32;
        let radius = (12.0 * scale) as u32;
        let changed = (y - radius..y + radius)
            .flat_map(|y| (x - radius..x + radius).map(move |x| (x, y)))
            .filter(|&(x, y)| idle.get_pixel(x, y) != hovered.get_pixel(x, y))
            .count();
        assert!(
            changed > 50,
            "edge {index} hover must brighten its camera icon"
        );
        let (line_x, line_y) = match index {
            0 => (f32::from(center.x) - 60.0, f32::from(bounds.top()) + 0.5),
            1 => (f32::from(center.x) - 60.0, f32::from(bounds.bottom()) - 0.5),
            2 => (f32::from(bounds.left()) + 0.5, f32::from(center.y) - 60.0),
            3 => (f32::from(bounds.right()) - 0.5, f32::from(center.y) - 60.0),
            _ => unreachable!(),
        };
        let (x, y) = ((line_x * scale) as u32, (line_y * scale) as u32);
        assert_ne!(
            idle.get_pixel(x, y),
            hovered.get_pixel(x, y),
            "edge {index} hover must paint its border"
        );
    }
    editor
        .update(cx, |editor, _, cx| editor.toggle_capture_mode(cx))
        .unwrap();
    settle(cx, window);
}

pub(super) fn run(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path) {
    for (name, edge, freeze, frozen) in [
        ("capture-failed", Edge::Right, "frozen", true),
        ("retry", Edge::Right, "frozen", true),
        ("invalid-image", Edge::Right, "frozen", true),
        ("cancelled", Edge::Top, "frozen", true),
        ("left", Edge::Left, "frozen", true),
        ("right", Edge::Right, "live", false),
        ("top", Edge::Top, "freeze-failed", true),
        ("bottom", Edge::Bottom, "freeze-unavailable", true),
        ("icons-restore-failed", Edge::Right, "live", false),
    ] {
        if name == "icons-restore-failed" && !cfg!(windows) {
            continue;
        }
        let directory = scratch.join(format!("attach-{name}"));
        std::fs::create_dir_all(&directory).unwrap();
        let source = directory.join("original.png");
        std::fs::copy(scratch.join("capture.png"), &source).unwrap();
        let original = image::open(&source).unwrap().to_rgba8();
        crate::history_store::add_item(
            HistoryItem {
                id: format!("attach-{name}"),
                timestamp: 0,
                original_path: source.to_string_lossy().into_owned(),
                r#type: HistoryItemType::Screenshot,
                editor_state: None,
                duration: None,
            },
            100,
            true,
        );
        let history: Vec<_> = crate::history_store::load_history()
            .into_iter()
            .map(|item| item.original_path)
            .collect();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": freeze, "rect": SELECTED_RECT, "windowId": null, "outcome": name,
                "scroll": (name == "icons-restore-failed").then_some("icons-show-never"),
            }))
            .unwrap(),
        )
        .unwrap();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.screenshot.freeze_screen = frozen;
                config.screenshot.show_preview = true;
                config.screenshot.capture_to_clipboard = true;
                config.screenshot.auto_copy_to_clipboard = true;
                if name == "icons-restore-failed" {
                    config.screenshot.hide_desktop_icons = false;
                }
                config.storage.screenshots_path = directory.to_string_lossy().into_owned();
            });
            ClipboardService::write_text(cx, "before editor attachment".into());
        });
        let editor = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                window.set_scale_factor(1.);
                cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
            })
            .unwrap();
        let window = editor.into();
        settle(cx, window);
        take_notifications();
        let before = calls(output).len();
        editor
            .update(cx, |editor, _, cx| editor.toggle_capture_mode(cx))
            .unwrap();
        settle(cx, window);
        let index = match edge {
            Edge::Top => 0,
            Edge::Bottom => 1,
            Edge::Left => 2,
            Edge::Right => 3,
            Edge::Primary => unreachable!(),
        };
        let at = measured("capture-edges", index).center();
        hover(cx, window, f32::from(at.x), f32::from(at.y));
        event(
            cx,
            window,
            PlatformInput::MouseDown(MouseDownEvent {
                position: at,
                button: if matches!(name, "retry" | "cancelled" | "left" | "bottom") {
                    MouseButton::Right
                } else {
                    MouseButton::Left
                },
                click_count: 1,
                modifiers: Modifiers::none(),
                first_mouse: false,
            }),
        );
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(
                editor.read(cx).unwrap().snapshot.borrow().draft.is_none(),
                "attach-{name}: choosing an edge must not start an annotation"
            );
        });
        let picker = area_picker(cx);
        if name == "left" {
            use crate::system::tray::Intent;

            for intent in [
                Intent::AllInOne,
                Intent::CaptureScreen,
                Intent::CaptureArea,
                Intent::CaptureWindow,
                Intent::ScrollCapture,
                Intent::CaptureText,
                Intent::ScanQrCode,
                Intent::TimerCapture,
                Intent::RecordScreen,
                Intent::RecordArea,
                Intent::RecordWindow,
            ] {
                cx.update(|cx| crate::intents::dispatch(intent, None, cx));
                cx.run_until_parked();
                assert_eq!(
                    area_picker(cx),
                    picker,
                    "{intent:?} replaced the attachment"
                );
            }
            let other = cx
                .open_window(size(px(1280.), px(738.)), |window, cx| {
                    cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
                })
                .unwrap();
            other
                .update(cx, |editor, window, cx| {
                    editor.attach_layer(Edge::Right, window, cx)
                })
                .unwrap();
            other
                .update(cx, |_, window, _| window.remove_window())
                .unwrap();
            cx.run_until_parked();
            assert_eq!(
                area_picker(cx),
                picker,
                "a second editor replaced the attachment"
            );
        }
        #[cfg(windows)]
        if name == "icons-restore-failed" {
            use crate::capture::desktop_icons::{self, HideSource};

            let daemon = cx.update(|cx| crate::state::state(cx).daemon.clone());
            assert!(desktop_icons::hide(&daemon, HideSource::Menu));
            assert!(!desktop_icons::show(&daemon, HideSource::Menu));
        }
        if name == "cancelled" {
            escape(cx, picker);
        } else {
            select_area(cx, picker);
        }
        cx.run_until_parked();
        assert!(!cx.update(crate::capture::overlay::parity_is_active));
        let calls: Vec<_> = calls(output).into_iter().skip(before).collect();
        assert_eq!(
            calls
                .iter()
                .filter(|call| call["module"] == "screenshot")
                .count(),
            usize::from(!matches!(name, "cancelled" | "icons-restore-failed"))
        );
        if name == "icons-restore-failed" {
            assert!(!cx.update(|cx| {
                crate::state::coordinator(cx)
                    .read(cx)
                    .has_pending_editor_attach()
            }));
            assert_eq!(cx.update(|cx| cx.active_window()), Some(window));
        }
        for call in &calls {
            if let Some(path) = call["params"]["path"].as_str() {
                assert!(
                    !Path::new(path).exists(),
                    "attach-{name}: temporary capture remains"
                );
            }
        }
        assert_eq!(
            cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
            Some("before editor attachment".into())
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        assert_eq!(
            crate::history_store::load_history()
                .into_iter()
                .map(|item| item.original_path)
                .collect::<Vec<_>>(),
            history
        );
        let succeeded = !matches!(
            name,
            "capture-failed" | "invalid-image" | "cancelled" | "icons-restore-failed"
        );
        let (layers, png) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.snapshot.borrow().layers.clone(),
                editor.export_png().unwrap(),
            )
        });
        assert_eq!(layers.len(), usize::from(succeeded));
        let actual = image::load_from_memory(&png).unwrap().to_rgba8();
        if succeeded {
            let layer = &layers[0];
            assert_eq!(layer.edge, edge);
            assert_eq!((layer.natural_width, layer.natural_height), (640., 360.));
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(
                    layer
                        .image_url
                        .strip_prefix("data:image/png;base64,")
                        .unwrap(),
                )
                .unwrap();
            let attached = image::load_from_memory(&bytes).unwrap().to_rgba8();
            let color = if freeze == "frozen" {
                [64, 112, 192, 255]
            } else {
                [240, 80, 64, 255]
            };
            assert!(attached.pixels().all(|pixel| pixel.0 == color));
            let (width, height, base_x, base_y, layer_x, layer_y) = match edge {
                Edge::Left => (1280, 360, 640, 0, 0, 0),
                Edge::Right => (1280, 360, 0, 0, 640, 0),
                Edge::Top => (640, 720, 0, 360, 0, 0),
                Edge::Bottom => (640, 720, 0, 0, 0, 360),
                Edge::Primary => unreachable!(),
            };
            let mut expected = image::RgbaImage::new(width, height);
            image::imageops::replace(&mut expected, &original, base_x, base_y);
            image::imageops::replace(&mut expected, &attached, layer_x, layer_y);
            assert_eq!(actual, expected);
            settle(cx, window);
            let (primary, zoom) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                (editor.bounds.borrow().unwrap(), editor.zoom)
            });
            let viewport = cx
                .update_window(window, |_, window, _| window.viewport_size())
                .unwrap();
            let left = primary.left() - px(base_x as f32 * zoom);
            let top = primary.top() - px(base_y as f32 * zoom);
            let right = left + px(width as f32 * zoom);
            let bottom = top + px(height as f32 * zoom);
            assert!(left >= px(0.) && right <= viewport.width && top >= px(crate::ui::chrome::TITLE_BAR_HEIGHT) && bottom <= viewport.height,
                "attach-{name}: the expanded composition must fit the editor viewport; ({left:?}, {top:?})..({right:?}, {bottom:?}), {viewport:?}");
            hover(cx, window, 20., 700.);
            if matches!(name, "left" | "retry") {
                let scale = cx
                    .update_window(window, |_, window, _| window.scale_factor())
                    .unwrap();
                let seam = if edge == Edge::Left {
                    primary.left()
                } else {
                    primary.right()
                };
                let y = if edge == Edge::Left { 20. } else { 60. };
                let x = (f32::from(seam) * scale).round() as i32;
                let y = ((f32::from(primary.top()) + y * zoom) * scale).round() as u32;
                let screenshot = cx.capture_screenshot(window).unwrap();
                for dx in -2..=2 {
                    assert!(
                        screenshot
                            .get_pixel((x + dx) as u32, y)
                            .0
                            .iter()
                            .zip([64_u8, 112, 192, 255])
                            .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                        "attach-{name}: a seam appeared at {dx}"
                    );
                }
            }
            capture(cx, window, output, &format!("workflow-attach-{name}"));
        } else {
            assert_eq!(actual, original);
        }
        cx.update_window(window, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        let reopened = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
            })
            .unwrap();
        cx.update(|cx| {
            let editor = reopened.read(cx).unwrap();
            assert_eq!(editor.snapshot.borrow().layers, layers);
            assert_eq!(editor.export_png().unwrap(), png);
        });
        reopened
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        let notifications = take_notifications();
        assert_eq!(
            notifications.len(),
            usize::from(matches!(
                name,
                "capture-failed" | "invalid-image" | "icons-restore-failed"
            ))
        );
        if name == "icons-restore-failed" {
            assert_eq!(
                notifications,
                [(
                    "Capture Failed".into(),
                    "Could not restore desktop icons".into(),
                    false
                )]
            );
        }
        #[cfg(windows)]
        if name == "icons-restore-failed" {
            use crate::capture::desktop_icons::{self, HideSource};

            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&serde_json::json!({"scroll": "icons-restored"})).unwrap(),
            )
            .unwrap();
            let daemon = cx.update(|cx| crate::state::state(cx).daemon.clone());
            assert!(desktop_icons::show(&daemon, HideSource::Menu));
        }
    }
    println!("editor attachment passed: four edges, live/frozen/fallback pixels, cancellation, failure, cleanup and reopen");
}
