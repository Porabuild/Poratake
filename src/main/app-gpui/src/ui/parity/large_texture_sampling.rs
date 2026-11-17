use gpui::AppContext;
use herogpui::gpui;

use crate::editor::window::EditorWindow;

use super::{
    event, point, px, settle, size, HeadlessAppContext, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Path, PlatformInput,
};

pub(super) fn run(cx: &mut HeadlessAppContext, scratch: &Path) {
    let scale = std::env::var("PORATAKE_PARITY_LARGE_DPI")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(1.);
    let zoom = std::env::var("PORATAKE_PARITY_LARGE_ZOOM")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(0.25);
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
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, editor.into());
                windows.push(editor);
            }
            let large = windows[0];
            let small_editor = windows[1];
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
                    event(cx, large.into(), input);
                }
                settle(cx, large.into());
            }
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
            let mut changed = 0;
            let mut worst = 0;
            for row in 2..compare_height - 2 {
                for col in 2..compare_width - 2 {
                    let actual = large_image.get_pixel(large_left + col, large_top + row).0;
                    let expected = small_image.get_pixel(small_left + col, small_top + row).0;
                    let delta = actual
                        .iter()
                        .zip(expected)
                        .map(|(a, b)| a.abs_diff(b))
                        .max()
                        .unwrap();
                    changed += usize::from(delta > 2);
                    worst = worst.max(delta);
                }
            }
            println!("oversized sampling horizontal={horizontal} transparent={transparent} zoom={zoom} scale={scale} differences={changed} max={worst}");
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
            for editor in windows {
                cx.update_window(editor.into(), |_, window, _| window.remove_window())
                    .unwrap();
            }
        }
    }
}
