use gpui::AppContext;
use herogpui::gpui;

use crate::editor::window::EditorWindow;
use crate::history_store::{HistoryItem, HistoryItemType};
use crate::windows::pin::PinWindow;

use super::{
    capture, click_measured, event, hover, measured, point, px, settle, size, HeadlessAppContext,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Path, PlatformInput,
    GEOMETRY,
};

pub(super) fn run(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path, scale: f32) {
    let source = scratch.join("capture.png");
    let decoded = image::open(&source).unwrap();
    let natural = size(
        px(decoded.width() as f32 / scale),
        px(decoded.height() as f32 / scale),
    );
    cx.update(|cx| PinWindow::open(cx, std::fs::read(&source).unwrap()));
    let pin = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .find(|window| window.downcast::<PinWindow>().is_some())
            .unwrap()
    });
    cx.update_window(pin, |_, window, _| window.set_scale_factor(scale))
        .unwrap();
    settle(cx, pin);
    hover(cx, pin, -5., -5.);
    GEOMETRY.with_borrow(|geometry| assert_eq!(geometry["pin-controls"].len(), 1));
    capture(cx, pin, output, "pin-default");
    hover(cx, pin, 40., 40.);
    assert_eq!(measured("pin-controls", 1).size, size(px(24.), px(24.)));
    capture(cx, pin, output, "pin-hover");
    for (name, viewport) in [
        ("enlarged", size(px(900.), px(700.))),
        ("narrow", size(px(240.), px(240.))),
    ] {
        cx.update_window(pin, |_, window, _| window.resize(viewport))
            .unwrap();
        settle(cx, pin);
        capture(cx, pin, output, &format!("pin-{name}"));
        let fit = (f32::from(viewport.width) / f32::from(natural.width))
            .min(f32::from(viewport.height) / f32::from(natural.height))
            .min(1.);
        let image_width = f32::from(natural.width) * fit;
        let image_height = f32::from(natural.height) * fit;
        let left = (f32::from(viewport.width) - image_width) * 0.5 * scale;
        let top = (f32::from(viewport.height) - image_height) * 0.5 * scale;
        let right = left + image_width * scale;
        let bottom = top + image_height * scale;
        let rendered = cx.capture_screenshot(pin).unwrap();
        let mut inside_blue = 0;
        let mut outside_blue = 0;
        for (x, y, pixel) in rendered.enumerate_pixels() {
            if x >= (f32::from(viewport.width) * scale).round() as u32
                || y >= (f32::from(viewport.height) * scale).round() as u32
            {
                continue;
            }
            if pixel[0].abs_diff(64) <= 2
                && pixel[1].abs_diff(112) <= 2
                && pixel[2].abs_diff(192) <= 2
            {
                if x as f32 + 1. < left
                    || x as f32 > right + 1.
                    || y as f32 + 1. < top
                    || y as f32 > bottom + 1.
                {
                    outside_blue += 1;
                } else {
                    inside_blue += 1;
                }
            }
        }
        assert!(inside_blue > 0, "pin-{name}: fixture image must be visible; outside={outside_blue}, natural={natural:?}, viewport={viewport:?}");
        assert_eq!(
            outside_blue, 0,
            "pin-{name}: image must stay centered and fit without upscaling"
        );
        let close = measured("pin-controls", 1);
        assert!(close.left() >= px(0.) && close.top() >= px(0.));
        assert!(close.right() <= viewport.width && close.bottom() <= viewport.height);
    }
    let close = measured("pin-controls", 1).center();
    event(
        cx,
        pin,
        PlatformInput::MouseDown(MouseDownEvent {
            position: point(close.x, close.y),
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    assert!(!cx.update(|cx| cx.windows().contains(&pin)));
    assert!(source.exists());
    println!("pin default, hover, enlarged, narrow, image bounds and standalone close passed");
}

pub(super) fn editor_roundtrip(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path) {
    let history_enabled = cx.update(|cx| crate::state::state(cx).config.get().history.enabled);
    for tracked in [true, false] {
        let name = if tracked { "tracked" } else { "untracked" };
        cx.update(|cx| {
            crate::state::state(cx)
                .config
                .update(|config| config.history.enabled = tracked)
        });
        let source = scratch.join(format!("pin-{name}-original.png"));
        std::fs::copy(scratch.join("capture.png"), &source).unwrap();
        if tracked {
            crate::history_store::add_item(
                HistoryItem {
                    id: "pin-roundtrip".into(),
                    timestamp: 0,
                    original_path: source.to_string_lossy().into_owned(),
                    r#type: HistoryItemType::Screenshot,
                    editor_state: None,
                    duration: None,
                },
                100,
                true,
            );
        }
        let history_paths: Vec<_> = crate::history_store::load_history()
            .into_iter()
            .map(|item| item.original_path)
            .collect();
        let editor = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                window.set_scale_factor(1.);
                cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
            })
            .unwrap();
        let window = editor.into();
        settle(cx, window);
        click_measured(cx, window, "editor-tools", 2);
        cx.update_window(window, |_, window, cx| {
            window.dispatch_action(Box::new(crate::editor::actions::ZoomReset), cx)
        })
        .unwrap();
        settle(cx, window);
        let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
        let start = bounds.origin + point(px(80.), px(80.));
        let end = bounds.origin + point(px(240.), px(180.));
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
        let (annotations, png) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            assert_eq!(editor.history.current().len(), 1);
            (
                editor.history.current().to_vec(),
                editor.export_png().unwrap(),
            )
        });
        click_measured(cx, window, "editor-tools", 20);
        cx.run_until_parked();
        assert!(!cx.update(|cx| cx.windows().contains(&window)));
        let pin = cx.update(|cx| {
            cx.windows()
                .into_iter()
                .find(|window| window.downcast::<PinWindow>().is_some())
                .unwrap()
        });
        cx.update_window(pin, |_, window, _| {
            window.set_scale_factor(1.);
            window.resize(size(px(640.), px(360.)));
        })
        .unwrap();
        settle(cx, pin);
        hover(cx, pin, -5., -5.);
        let expected = image::load_from_memory(&png).unwrap().to_rgba8();
        let rendered = cx.capture_screenshot(pin).unwrap();
        let actual = image::imageops::crop_imm(&rendered, 0, 0, 640, 360).to_image();
        std::fs::write(output.join("pin-export.png"), &png).unwrap();
        capture(cx, pin, output, &format!("workflow-pin-{name}-annotated"));
        assert!(
            actual
                .pixels()
                .zip(expected.pixels())
                .all(|(actual, expected)| actual
                    .0
                    .iter()
                    .zip(expected.0)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2)),
            "pinned pixels must equal the annotated export"
        );
        hover(cx, pin, 40., 40.);
        let close = measured("pin-controls", 1).center();
        event(
            cx,
            pin,
            PlatformInput::MouseDown(MouseDownEvent {
                position: close,
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Modifiers::none(),
                first_mouse: false,
            }),
        );
        cx.run_until_parked();
        assert!(!cx.update(|cx| cx.windows().contains(&pin)));
        let restored = cx.update(|cx| {
            cx.windows()
                .into_iter()
                .filter_map(|window| window.downcast::<EditorWindow>())
                .find(|editor| Path::new(&editor.read(cx).unwrap().file_path) == source)
                .unwrap()
        });
        cx.update(|cx| {
            let editor = restored.read(cx).unwrap();
            assert_eq!(editor.history.current(), annotations);
            assert_eq!(editor.export_png().unwrap(), png);
        });
        let window = restored.into();
        cx.update_window(window, |_, window, _| {
            window.resize(size(px(1280.), px(738.)));
            window.set_scale_factor(1.);
        })
        .unwrap();
        settle(cx, window);
        hover(cx, window, 20., 700.);
        capture(cx, window, output, &format!("workflow-pin-{name}-restored"));
        cx.update_window(window, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        assert_eq!(
            crate::history_store::load_history()
                .into_iter()
                .map(|item| item.original_path)
                .collect::<Vec<_>>(),
            history_paths
        );
    }
    cx.update(|cx| {
        crate::state::state(cx)
            .config
            .update(|config| config.history.enabled = history_enabled)
    });
    println!("annotated editor -> matching pinned pixels -> restored editor annotations and export passed");
}
