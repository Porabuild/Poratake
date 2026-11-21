use gpui::AppContext;
use herogpui::gpui;

use crate::editor::annotations::Annotation;
use crate::editor::window::EditorWindow;

use super::{
    event, point, px, settle, size, HeadlessAppContext, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Path, PlatformInput,
};

pub(super) fn ordered_annotations(style: &str, horizontal: bool, seam: f64) -> Vec<Annotation> {
    let rect = |long, short, long_size, short_size| {
        if horizontal {
            (long, short, long_size, short_size)
        } else {
            (short, long, short_size, long_size)
        }
    };
    let (x, y, width, height) = rect(seam - 40., 60., 80., 6.);
    let before = Annotation::Rectangle {
        id: "before".into(),
        x,
        y,
        width,
        height,
        stroke: "#ff0000".into(),
        stroke_width: 2.,
        fill: Some("#ff0000".into()),
    };
    let (x, y, width, height) = rect(seam - 30., 42., 60., 44.);
    let redact = Annotation::Redact {
        id: "redact".into(),
        x,
        y,
        width,
        height,
        style: style.into(),
        intensity: 5.,
    };
    let (x, y, width, height) = rect(seam - 10., 52., 20., 20.);
    let after = Annotation::Rectangle {
        id: "after".into(),
        x,
        y,
        width,
        height,
        stroke: "#00ff00".into(),
        stroke_width: 2.,
        fill: Some("#00ff00".into()),
    };
    vec![before, redact, after]
}

pub(super) fn run(cx: &mut HeadlessAppContext, scratch: &Path) {
    let scale = std::env::var("PORATAKE_PARITY_LARGE_DPI")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(1.);
    let zoom = std::env::var("PORATAKE_PARITY_LARGE_ZOOM")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(0.25);
    let composed = std::env::var_os("PORATAKE_PARITY_LARGE_COMPOSED").is_some();
    let annotation = std::env::var("PORATAKE_PARITY_LARGE_ANNOTATION").ok();
    assert!(zoom == 0.25 || zoom == 0.5);
    for horizontal in [true, false] {
        for transparent in [false, true] {
            let (width, height) = if horizontal {
                (17_024, 128)
            } else {
                (128, 17_024)
            };
            let source = image::RgbaImage::from_fn(width, height, |x, y| {
                let (long, short) = if horizontal { (x, y) } else { (y, x) };
                let shade = if (long / 5 + short / 7) % 2 == 0 {
                    220
                } else {
                    30
                };
                let alpha = if transparent {
                    ((long * 11 + short * 7) % 256) as u8
                } else {
                    255
                };
                image::Rgba([shade, (long % 251) as u8, (short * 2) as u8, alpha])
            });
            let large_path = scratch.join(format!("sampling-large-{horizontal}-{transparent}.png"));
            source.save(&large_path).unwrap();
            let (x, y, w, h) = if horizontal {
                (7936, 0, 512, 128)
            } else {
                (0, 7936, 128, 512)
            };
            let small = image::imageops::crop_imm(&source, x, y, w, h).to_image();
            let small_path = scratch.join(format!("sampling-small-{horizontal}-{transparent}.png"));
            small.save(&small_path).unwrap();
            let mut windows = Vec::new();
            for path in [&large_path, &small_path] {
                let editor = cx
                    .open_window(size(px(1280.), px(738.)), |window, cx| {
                        window.set_scale_factor(scale);
                        cx.new(|cx| EditorWindow::from_file(path.to_str().unwrap(), window, cx))
                    })
                    .unwrap();
                editor
                    .update(cx, |editor, _, cx| {
                        editor.zoom = zoom;
                        if composed {
                            editor.wallpaper.gradient =
                                Some(crate::editor::wallpaper::GradientOption {
                                    id: "sampling".into(),
                                    colors: vec!["#203060".into()],
                                    angle: 0.,
                                });
                        }
                        if let Some(style) = &annotation {
                            let seam = if path == &large_path { 8192. } else { 256. };
                            editor
                                .history
                                .push(ordered_annotations(style, horizontal, seam));
                        }
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, editor.into());
                if composed {
                    cx.update(|cx| {
                        let editor = editor.read(cx).unwrap();
                        let snapshot = editor.snapshot.borrow();
                        assert!(
                            snapshot.backdrop_composite,
                            "composed backdrop was not ready"
                        );
                        if path == &large_path {
                            assert!(
                                !snapshot.preview_edges.is_empty(),
                                "large backdrop was not tiled"
                            );
                        }
                    });
                }
                windows.push(editor);
            }
            let large = windows[0];
            let small_editor = windows[1];
            pan(cx, large, horizontal, zoom);
            let (large_bounds, small_bounds) = cx.update(|cx| {
                (
                    large.read(cx).unwrap().bounds.borrow().unwrap(),
                    small_editor.read(cx).unwrap().bounds.borrow().unwrap(),
                )
            });
            let large_image = cx.capture_screenshot(large.into()).unwrap();
            let small_image = cx.capture_screenshot(small_editor.into()).unwrap();
            let (large_left, large_top) = (
                ((f32::from(large_bounds.left()) + x as f32 * zoom) * scale).round() as u32,
                ((f32::from(large_bounds.top()) + y as f32 * zoom) * scale).round() as u32,
            );
            let (small_left, small_top) = (
                (f32::from(small_bounds.left()) * scale).round() as u32,
                (f32::from(small_bounds.top()) * scale).round() as u32,
            );
            let (compare_width, compare_height) = (
                (w as f32 * zoom * scale).round() as u32,
                (h as f32 * zoom * scale).round() as u32,
            );
            let (changed, worst) = compare(
                &large_image,
                (large_left, large_top),
                &small_image,
                (small_left, small_top),
                (compare_width, compare_height),
            );
            println!("oversized sampling composed={composed} horizontal={horizontal} transparent={transparent} zoom={zoom} scale={scale} differences={changed} max={worst}");
            if changed > 0 {
                large_image
                    .save(scratch.join(format!(
                        "sampling-large-{horizontal}-{transparent}-preview.png"
                    )))
                    .unwrap();
                small_image
                    .save(scratch.join(format!(
                        "sampling-small-{horizontal}-{transparent}-preview.png"
                    )))
                    .unwrap();
            }
            assert_eq!(changed, 0, "oversized and cropped reference differ");
            if composed {
                let large_png = cx.update(|cx| large.read(cx).unwrap().export_png().unwrap());
                let small_png =
                    cx.update(|cx| small_editor.read(cx).unwrap().export_png().unwrap());
                let large_output = image::load_from_memory(&large_png).unwrap().to_rgba8();
                let small_output = image::load_from_memory(&small_png).unwrap().to_rgba8();
                if annotation.is_some() {
                    let unredacted_png = small_editor
                        .update(cx, |editor, _, _| {
                            let without = editor
                                .history
                                .current()
                                .iter()
                                .filter(|item| !matches!(item, Annotation::Redact { .. }))
                                .cloned()
                                .collect();
                            editor.history.push(without);
                            let png = editor.export_png().unwrap();
                            assert!(editor.history.undo());
                            png
                        })
                        .unwrap();
                    let unredacted = image::load_from_memory(&unredacted_png).unwrap().to_rgba8();
                    let modified = small_output
                        .pixels()
                        .zip(unredacted.pixels())
                        .filter(|(a, b)| a != b)
                        .count();
                    println!(
                        "oversized effect style={} modified_pixels={modified}",
                        annotation.as_deref().unwrap()
                    );
                    assert!(modified > 100, "the effect must change the exported pixels");
                }
                if let Some(style) = annotation.as_deref() {
                    if style == "blackout" {
                        let point = if horizontal { (8172, 62) } else { (62, 8172) };
                        assert_eq!(large_output.get_pixel(point.0, point.1).0, [0, 0, 0, 255]);
                    } else {
                        let point = if horizontal { (8172, 46) } else { (46, 8172) };
                        assert!(large_output.get_pixel(point.0, point.1).0[..3]
                            .iter()
                            .any(|channel| *channel > 30));
                    }
                    let point = if horizontal { (8192, 62) } else { (62, 8192) };
                    assert_eq!(large_output.get_pixel(point.0, point.1).0, [0, 255, 0, 255]);
                }
                assert_eq!(small_output.dimensions(), (w, h));
                for row in 0..h {
                    for col in 0..w {
                        assert_eq!(
                            large_output.get_pixel(x + col, y + row),
                            small_output.get_pixel(col, row)
                        );
                    }
                }
                let reopened_path =
                    scratch.join(format!("sampling-reopened-{horizontal}-{transparent}.png"));
                std::fs::write(&reopened_path, small_png).unwrap();
                let reopened = cx
                    .open_window(size(px(1280.), px(738.)), |window, cx| {
                        window.set_scale_factor(scale);
                        cx.new(|cx| {
                            EditorWindow::from_file(reopened_path.to_str().unwrap(), window, cx)
                        })
                    })
                    .unwrap();
                reopened
                    .update(cx, |editor, _, cx| {
                        editor.zoom = zoom;
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, reopened.into());
                let reopened_bounds =
                    cx.update(|cx| reopened.read(cx).unwrap().bounds.borrow().unwrap());
                assert_eq!(small_bounds, reopened_bounds);
                let reopened_image = cx.capture_screenshot(reopened.into()).unwrap();
                let (reopen_changed, reopen_worst) = compare(
                    &small_image,
                    (small_left, small_top),
                    &reopened_image,
                    (small_left, small_top),
                    (compare_width, compare_height),
                );
                println!("composed reopen horizontal={horizontal} transparent={transparent} zoom={zoom} scale={scale} differences={reopen_changed} max={reopen_worst}");
                assert_eq!(
                    reopen_changed, 0,
                    "composed reference differs from reopened export"
                );
                cx.update_window(reopened.into(), |_, window, _| window.remove_window())
                    .unwrap();
                let reopened_large_path = scratch.join(format!(
                    "sampling-reopened-large-{horizontal}-{transparent}.png"
                ));
                std::fs::write(&reopened_large_path, large_png).unwrap();
                let reopened_large = cx
                    .open_window(size(px(1280.), px(738.)), |window, cx| {
                        window.set_scale_factor(scale);
                        cx.new(|cx| {
                            EditorWindow::from_file(
                                reopened_large_path.to_str().unwrap(),
                                window,
                                cx,
                            )
                        })
                    })
                    .unwrap();
                reopened_large
                    .update(cx, |editor, _, cx| {
                        editor.zoom = zoom;
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, reopened_large.into());
                pan(cx, reopened_large, horizontal, zoom);
                let reopened_large_bounds =
                    cx.update(|cx| reopened_large.read(cx).unwrap().bounds.borrow().unwrap());
                assert_eq!(large_bounds, reopened_large_bounds);
                let reopened_large_image = cx.capture_screenshot(reopened_large.into()).unwrap();
                let (large_changed, large_worst) = compare(
                    &large_image,
                    (large_left, large_top),
                    &reopened_large_image,
                    (large_left, large_top),
                    (compare_width, compare_height),
                );
                println!("composed large reopen horizontal={horizontal} transparent={transparent} zoom={zoom} scale={scale} differences={large_changed} max={large_worst}");
                assert_eq!(
                    large_changed, 0,
                    "composed preview differs from reopened large export"
                );
                cx.update_window(reopened_large.into(), |_, window, _| window.remove_window())
                    .unwrap();
            }
            for editor in windows {
                cx.update_window(editor.into(), |_, window, _| window.remove_window())
                    .unwrap();
            }
        }
    }
}

pub(super) fn pan(
    cx: &mut HeadlessAppContext,
    editor: gpui::WindowHandle<EditorWindow>,
    horizontal: bool,
    zoom: f32,
) {
    let modifiers = Modifiers {
        platform: true,
        control: true,
        ..Modifiers::none()
    };
    let start = point(px(640.), px(390.));
    let end = if horizontal {
        point(px(100.), px(390.))
    } else {
        point(px(640.), px(90.))
    };
    let drags = match (horizontal, zoom == 0.25) {
        (true, true) => 2,
        (false, true) => 5,
        (true, false) => 7,
        (false, false) => 12,
    };
    for _ in 0..drags {
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
            event(cx, editor.into(), input);
        }
        settle(cx, editor.into());
    }
}

fn compare(
    actual: &image::RgbaImage,
    actual_origin: (u32, u32),
    expected: &image::RgbaImage,
    expected_origin: (u32, u32),
    size: (u32, u32),
) -> (usize, u8) {
    let (mut changed, mut worst) = (0, 0);
    for row in 2..size.1 - 2 {
        for col in 2..size.0 - 2 {
            let a = actual
                .get_pixel(actual_origin.0 + col, actual_origin.1 + row)
                .0;
            let b = expected
                .get_pixel(expected_origin.0 + col, expected_origin.1 + row)
                .0;
            let delta = a.iter().zip(b).map(|(a, b)| a.abs_diff(b)).max().unwrap();
            changed += usize::from(delta > 2);
            worst = worst.max(delta);
        }
    }
    (changed, worst)
}
