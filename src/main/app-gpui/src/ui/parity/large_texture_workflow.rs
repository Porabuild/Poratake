use gpui::AppContext;
use herogpui::gpui;

use crate::editor::window::EditorWindow;

use super::{
    event, point, px, settle, size, HeadlessAppContext, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Path, PlatformInput,
};

pub(super) fn run(cx: &mut HeadlessAppContext, scratch: &Path) {
    let zoom = std::env::var("PORATAKE_PARITY_LARGE_ZOOM")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(0.025);
    let scale = std::env::var("PORATAKE_PARITY_LARGE_DPI")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(1.);
    assert!(zoom == 0.025 || zoom == 0.25);
    for horizontal in [true, false] {
        let (width, height) = if horizontal {
            (17_000, 100)
        } else {
            (100, 17_000)
        };
        let source = scratch.join(format!("large-texture-{horizontal}.png"));
        image::RgbaImage::from_fn(width, height, |x, y| {
            let position = if horizontal { x } else { y };
            let rgb = match position {
                0..=5665 => [220, 30, 40],
                5666..=11332 => [30, 210, 50],
                _ => [40, 60, 220],
            };
            image::Rgba([rgb[0], rgb[1], rgb[2], 255])
        })
        .save(&source)
        .unwrap();
        let mut path = source.clone();
        for reopened in [false, true] {
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
            for panned in [false, true] {
                if panned {
                    if zoom != 0.25 {
                        continue;
                    }
                    let original =
                        cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
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
                    for _ in 0..if horizontal { 3 } else { 6 } {
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
                    let moved = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
                    assert_ne!(original.origin, moved.origin, "the image must pan");
                }
                let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
                let image = cx.capture_screenshot(editor.into()).unwrap();
                let (start, length, points) = if horizontal {
                    (
                        f32::from(bounds.left()),
                        f32::from(bounds.size.width),
                        [450., 640., 830.],
                    )
                } else {
                    (
                        f32::from(bounds.top()),
                        f32::from(bounds.size.height),
                        [240., 390., 540.],
                    )
                };
                let samples = if zoom == 0.025 {
                    vec![0.15, 0.475, 0.485, 0.5, 0.85, 0.96, 0.97]
                } else {
                    let mut points = points.to_vec();
                    if panned {
                        let seam = start + 8192. * zoom;
                        assert!(
                            (200. ..1000.).contains(&seam),
                            "tile seam is not visible: {seam}"
                        );
                        points.extend([seam - 2., seam, seam + 2.]);
                    }
                    points
                        .into_iter()
                        .map(|point| (point - start) / length)
                        .collect()
                };
                for fraction in samples {
                    assert!(
                        (0.01..0.99).contains(&fraction),
                        "sample outside image: {fraction}, {bounds:?}"
                    );
                    let expected = if fraction < 1. / 3. {
                        [220, 30, 40]
                    } else if fraction < 2. / 3. {
                        [30, 210, 50]
                    } else {
                        [40, 60, 220]
                    };
                    let x = f32::from(bounds.left())
                        + f32::from(bounds.size.width) * if horizontal { fraction } else { 0.5 };
                    let y = f32::from(bounds.top())
                        + f32::from(bounds.size.height) * if horizontal { 0.5 } else { fraction };
                    let pixel = image
                        .get_pixel((x * scale).round() as u32, (y * scale).round() as u32)
                        .0;
                    println!("large texture horizontal={horizontal} zoom={zoom} scale={scale} reopened={reopened} panned={panned} fraction={fraction} pixel={pixel:?}");
                    assert!(pixel[..3]
                        .iter()
                        .zip(expected)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 2));
                }
            }
            if !reopened {
                let saved = scratch.join(format!("large-texture-{horizontal}-saved.png"));
                let png = cx.update(|cx| editor.read(cx).unwrap().export_png().unwrap());
                std::fs::write(&saved, png).unwrap();
                let output = image::open(&saved).unwrap().to_rgba8();
                assert_eq!(
                    output
                        .get_pixel(
                            if horizontal { 8500 } else { 50 },
                            if horizontal { 50 } else { 8500 }
                        )
                        .0,
                    [30, 210, 50, 255]
                );
                path = saved;
            }
            cx.update_window(editor.into(), |_, window, _| window.remove_window())
                .unwrap();
        }
    }
}
