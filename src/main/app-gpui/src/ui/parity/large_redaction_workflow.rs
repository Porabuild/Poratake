use gpui::AppContext;
use herogpui::gpui;

use crate::editor::annotations::Annotation;
use crate::editor::window::EditorWindow;
use crate::history_store::{HistoryItem, HistoryItemType};

use super::{
    event, point, px, settle, size, HeadlessAppContext, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Path, PlatformInput,
};

pub(super) fn run(cx: &mut HeadlessAppContext, scratch: &Path) {
    for style in ["none", "pixelate", "blur"] {
        run_case(cx, scratch, 4096, style);
    }
}

pub(super) fn run_boundary(cx: &mut HeadlessAppContext, scratch: &Path) {
    run_case(cx, scratch, 3840, "pixelate");
}

fn run_case(cx: &mut HeadlessAppContext, scratch: &Path, source_width: u32, style: &str) {
    let zoom = std::env::var("PORATAKE_PARITY_LARGE_ZOOM")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(0.25);
    let dpi = std::env::var("PORATAKE_PARITY_LARGE_DPI")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(1.);
    let wide = std::env::var_os("PORATAKE_PARITY_LARGE_WIDE").is_some();
    let transparent = std::env::var_os("PORATAKE_PARITY_LARGE_TRANSPARENT").is_some();
    let window_size = if wide { (4600., 2600.) } else { (1280., 738.) };
    let source = scratch.join(format!("large-{style}.png"));
    image::RgbaImage::from_fn(source_width, 2160, |x, y| {
        let red = if (x / 3 + y / 5) % 2 == 0 { 24 } else { 224 };
        let alpha = if transparent {
            ((x + y * 3) % 256) as u8
        } else {
            255
        };
        image::Rgba([red, (y % 32 * 6) as u8, 80, alpha])
    })
    .save(&source)
    .unwrap();
    let annotation = Annotation::Redact {
        id: "large-redaction".into(),
        x: 0.,
        y: 0.,
        width: f64::from(source_width),
        height: 2160.,
        style: style.into(),
        intensity: 5.,
    };
    let annotations = if style == "none" {
        Vec::new()
    } else {
        vec![annotation]
    };
    crate::history_store::add_item(
        HistoryItem {
            id: format!("large-{style}"),
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
        .open_window(size(px(window_size.0), px(window_size.1)), |window, cx| {
            window.set_scale_factor(dpi);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, editor.into());
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = zoom;
            cx.notify();
        })
        .unwrap();
    settle(cx, editor.into());
    let png = cx.update(|cx| editor.read(cx).unwrap().export_png().unwrap());
    let original = image::open(&source).unwrap().to_rgba8();
    let exported = image::load_from_memory(&png).unwrap().to_rgba8();
    let modified = (0..32)
        .flat_map(|y| (0..32).map(move |x| (x * 31, y * 31)))
        .filter(|&(x, y)| original.get_pixel(x, y) != exported.get_pixel(x, y))
        .count();
    if style == "none" && !transparent {
        assert_eq!(modified, 0);
    } else if style != "none" {
        assert!(modified > 0, "the redaction must alter source pixels");
    }
    drop(original);
    drop(exported);
    let flattened_path = scratch.join(format!("large-{style}-flattened.png"));
    std::fs::write(&flattened_path, &png).unwrap();
    let flattened = cx
        .open_window(size(px(window_size.0), px(window_size.1)), |window, cx| {
            window.set_scale_factor(dpi);
            cx.new(|cx| EditorWindow::from_file(flattened_path.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, flattened.into());
    flattened
        .update(cx, |editor, _, cx| {
            editor.zoom = zoom;
            cx.notify();
        })
        .unwrap();
    settle(cx, flattened.into());
    if std::env::var_os("PORATAKE_PARITY_LARGE_PAN").is_some() {
        let original = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
        for window in [editor, flattened] {
            let modifiers = Modifiers {
                platform: true,
                control: true,
                ..Modifiers::none()
            };
            let start = point(px(600.), px(400.));
            let end = point(px(400.), px(260.));
            for input in [
                PlatformInput::MouseDown(MouseDownEvent {
                    position: start,
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers,
                    first_mouse: false,
                }),
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: end,
                    pressed_button: Some(MouseButton::Left),
                    modifiers,
                }),
                PlatformInput::MouseUp(MouseUpEvent {
                    position: end,
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers,
                }),
            ] {
                event(cx, window.into(), input);
            }
            settle(cx, window.into());
        }
        let moved = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
        assert_ne!(original.origin, moved.origin, "the drag must pan the image");
    }
    let (bounds, flat_bounds) = cx.update(|cx| {
        (
            editor.read(cx).unwrap().bounds.borrow().unwrap(),
            flattened.read(cx).unwrap().bounds.borrow().unwrap(),
        )
    });
    let preview = cx.capture_screenshot(editor.into()).unwrap();
    let expected = cx.capture_screenshot(flattened.into()).unwrap();
    let snap = |value: f32| {
        let value = value * dpi;
        (value.abs() - 0.5).ceil().copysign(value) as u32
    };
    let (left, top, flat_left, flat_top, width, height) = if zoom == 0.25 {
        let left = snap(f32::from(bounds.left()));
        let top = snap(f32::from(bounds.top()));
        let flat_left = snap(f32::from(flat_bounds.left()));
        let flat_top = snap(f32::from(flat_bounds.top()));
        let width = snap(f32::from(bounds.right())) - left;
        let height = snap(f32::from(bounds.bottom())) - top;
        assert_eq!(
            (width, height),
            (
                (source_width as f32 * zoom * dpi) as u32,
                (2160. * zoom * dpi) as u32
            )
        );
        assert_eq!(
            (width, height),
            (
                snap(f32::from(flat_bounds.right())) - flat_left,
                snap(f32::from(flat_bounds.bottom())) - flat_top
            )
        );
        (left, top, flat_left, flat_top, width, height)
    } else {
        assert_eq!(bounds, flat_bounds);
        let left = snap(f32::from(bounds.left()).max(300.));
        let top = snap(f32::from(bounds.top()).max(160.));
        let right = snap(f32::from(bounds.right()).min(window_size.0 - 330.));
        let bottom = snap(f32::from(bounds.bottom()).min(window_size.1 - 88.));
        assert!(right > left && bottom > top);
        if wide {
            assert!(u64::from(right - left) * u64::from(bottom - top) > 8_388_608);
        }
        (left, top, left, top, right - left, bottom - top)
    };
    let mut changed = 0u64;
    let mut total = 0u64;
    let mut maximum = 0u8;
    for y in 0..height {
        for x in 0..width {
            let actual = preview.get_pixel(left + x, top + y).0;
            let expected = expected.get_pixel(flat_left + x, flat_top + y).0;
            let difference = actual
                .iter()
                .zip(expected)
                .map(|(a, b)| a.abs_diff(b))
                .max()
                .unwrap();
            maximum = maximum.max(difference);
            total += u64::from(difference);
            changed += u64::from(difference > 2);
        }
    }
    image::imageops::crop_imm(&preview, left, top, width, height)
        .to_image()
        .save(scratch.join(format!("large-{style}-preview.png")))
        .unwrap();
    image::imageops::crop_imm(&expected, flat_left, flat_top, width, height)
        .to_image()
        .save(scratch.join(format!("large-{style}-expected.png")))
        .unwrap();
    println!(
        "large {source_width}x2160 {style} at {zoom}x, display {dpi}x: {modified} of 1024 source samples changed; {changed} of {} displayed pixels above two; max {maximum}; mean max-channel {:.4}",
        width * height,
        total as f64 / f64::from(width * height)
    );
    assert_eq!(
        changed, 0,
        "{style} preview differs from the reopened export"
    );
    for window in [editor, flattened] {
        cx.update_window(window.into(), |_, window, _| window.remove_window())
            .unwrap();
    }
    cx.run_until_parked();
}
