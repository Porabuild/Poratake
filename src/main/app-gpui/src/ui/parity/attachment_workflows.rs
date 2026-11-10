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
    capture, escape, hover, px, settle, size, take_notifications, HeadlessAppContext, Path,
};

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
    ] {
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
            .update(cx, |editor, window, cx| {
                editor.attach_layer(edge, window, cx)
            })
            .unwrap();
        cx.run_until_parked();
        let picker = area_picker(cx);
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
            usize::from(name != "cancelled")
        );
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
        let succeeded = !matches!(name, "capture-failed" | "invalid-image" | "cancelled");
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
            usize::from(matches!(name, "capture-failed" | "invalid-image"))
        );
    }
    println!("editor attachment passed: four edges, live/frozen/fallback pixels, cancellation, failure, cleanup and reopen");
}
