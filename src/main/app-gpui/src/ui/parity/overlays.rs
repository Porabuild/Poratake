use gpui::{
    div, prelude::*, AppContext, Context, Entity, MouseButton, MouseDownEvent, MouseMoveEvent,
    PlatformInput, Render, Window,
};
use herogpui::gpui;

use crate::capture::all_in_one::{Choices, Mode, Target};
use crate::capture::intent::CaptureIntent;
use crate::capture::overlay::AreaOverlay;

use super::{
    capture, event, hover, point, px, settle, size, Bounds, HeadlessAppContext, Modifiers, Path,
};

struct Scene {
    overlay: Entity<AreaOverlay>,
    window_hovered: bool,
}

struct ColorScene {
    frame: image::RgbaImage,
    pointer: gpui::Point<gpui::Pixels>,
}

impl Render for ColorScene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().bg(gpui::rgb(0x4070c0)).child(
            crate::capture::color_picker::render(
                &self.frame,
                self.pointer,
                window,
                &crate::theme::vars::active_theme(cx),
            ),
        )
    }
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let overlay = self.overlay.update(cx, |overlay, cx| {
            overlay.surface(self.window_hovered, window, cx)
        });
        let overlay = crate::ui::window_root::focus_root(overlay, window, cx);
        div()
            .relative()
            .size_full()
            .bg(gpui::rgb(0xdce6f2))
            .child(
                div()
                    .absolute()
                    .left(px(40.))
                    .top(px(80.))
                    .w(px(400.))
                    .h(px(280.))
                    .bg(gpui::rgb(0x4070c0)),
            )
            .child(
                div()
                    .absolute()
                    .right(px(40.))
                    .bottom(px(60.))
                    .w(px(320.))
                    .h(px(240.))
                    .bg(gpui::rgb(0xf0a050)),
            )
            .child(div().absolute().inset_0().child(overlay))
    }
}

pub(super) fn run(cx: &mut HeadlessAppContext, output: &Path, width: f32, scale: f32) {
    let mut cases = vec![
        (
            "screenshot".to_string(),
            CaptureIntent::Screenshot,
            None,
            Target::Area,
        ),
        ("ocr".to_string(), CaptureIntent::Ocr, None, Target::Area),
        ("qr".to_string(), CaptureIntent::QrCode, None, Target::Area),
        (
            "timer".to_string(),
            CaptureIntent::Timer,
            None,
            Target::Area,
        ),
        (
            "scroll".to_string(),
            CaptureIntent::ScrollCapture,
            None,
            Target::Area,
        ),
        (
            "record".to_string(),
            CaptureIntent::Recording,
            None,
            Target::Area,
        ),
        (
            "attach".to_string(),
            CaptureIntent::EditorAttach,
            None,
            Target::Area,
        ),
        (
            "window".to_string(),
            CaptureIntent::Screenshot,
            None,
            Target::Window,
        ),
        (
            "record-window".to_string(),
            CaptureIntent::Recording,
            None,
            Target::Window,
        ),
        (
            "display".to_string(),
            CaptureIntent::Screenshot,
            None,
            Target::Screen,
        ),
        (
            "record-display".to_string(),
            CaptureIntent::Recording,
            None,
            Target::Screen,
        ),
    ];
    for mode in [Mode::Screenshot, Mode::Record, Mode::Ocr] {
        for target in Target::ALL {
            if mode == Mode::Ocr && target != Target::Area {
                continue;
            }
            cases.push((
                format!("all-in-one-{}-{}", mode.id(), target.id()),
                CaptureIntent::Screenshot,
                Some(Choices { mode, target }),
                target,
            ));
        }
    }
    for (name, intent, choices, target) in cases {
        let scene = cx
            .open_window(size(px(width), px(738.)), |window, cx| {
                window.set_scale_factor(scale);
                let overlay = cx.new(|cx| {
                    let mut overlay = AreaOverlay::with_focus(
                        Bounds::new(point(px(-width), px(-100.)), size(px(width), px(738.))),
                        scale,
                        crate::state::state(cx),
                        intent,
                        cx.focus_handle(),
                    );
                    if let Some(choices) = choices {
                        overlay = overlay.with_all_in_one(choices);
                    }
                    match target {
                        Target::Window => {
                            let coordinate_scale =
                                crate::capture::overlay::capture_coordinate_scale(scale) as f64;
                            overlay.with_windows(
                                [
                                    (40., 80., 400., 280.),
                                    (-40., 40., 560., 400.),
                                    (width as f64 - 360., 438., 400., 340.),
                                ]
                                .into_iter()
                                .enumerate()
                                .map(|(index, (x, y, width_in_pixels, height))| {
                                    crate::capture::windows_list::WindowListItem {
                                        window_id: index as i64 + 1,
                                        title: format!("Fixture window {}", index + 1),
                                        owner_name: "Fixture".into(),
                                        owner_pid: 1,
                                        bounds: poratake_daemon_common::geometry::WindowBounds {
                                            x: (x - width as f64) * coordinate_scale,
                                            y: (y - 100.) * coordinate_scale,
                                            width: width_in_pixels * coordinate_scale,
                                            height: height * coordinate_scale,
                                        },
                                    }
                                })
                                .collect(),
                            )
                        }
                        Target::Screen => overlay.with_screen_picker(),
                        Target::Area => overlay,
                    }
                });
                cx.new(|_| Scene {
                    overlay,
                    window_hovered: false,
                })
            })
            .unwrap();
        let window = scene.into();
        settle(cx, window);
        hover(cx, window, 140., 180.);
        capture(cx, window, output, &format!("overlay-{name}-default"));
        if target == Target::Area {
            event(
                cx,
                window,
                PlatformInput::MouseDown(MouseDownEvent {
                    position: point(px(90.), px(120.)),
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
                    position: point(px(610.), px(420.)),
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Modifiers::none(),
                }),
            );
            settle(cx, window);
            let selection_painted = cx
                .update_window(window, |_, window, _| {
                    window.painted_quads().iter().any(|quad| {
                        [
                            (quad.bounds.origin.x.0 / scale, 90.),
                            (quad.bounds.origin.y.0 / scale, 120.),
                            (quad.bounds.size.width.0 / scale, 520.),
                            (quad.bounds.size.height.0 / scale, 300.),
                        ]
                        .iter()
                        .all(|(actual, expected)| (actual - expected).abs() <= 0.5 / scale + 0.001)
                    })
                })
                .unwrap();
            assert!(
                selection_painted,
                "{name}: the selection frame must match the drag bounds"
            );
            capture(cx, window, output, &format!("overlay-{name}-selected"));
        } else if target == Target::Window {
            for (state, x, y, expected) in [
                ("front", 140., 180., Some([40., 80., 400., 280.])),
                ("clipped-left", 500., 300., Some([-40., 40., 560., 400.])),
                (
                    "clipped-right",
                    width - 200.,
                    550.,
                    Some([width - 360., 438., 400., 340.]),
                ),
                ("leave", width - 200., 550., None),
            ] {
                cx.update(|cx| {
                    scene.update(cx, |scene, _, cx| {
                        scene.window_hovered = state != "leave";
                        cx.notify();
                    })
                })
                .unwrap();
                hover(cx, window, x, y);
                let name = format!("overlay-{name}-{state}");
                capture(cx, window, output, &name);
                assert_picker_pixels(cx, window, output, &name, width, scale, expected);
            }
        } else {
            for (state, hovered) in [("hover", true), ("leave", false)] {
                cx.update(|cx| {
                    scene.update(cx, |scene, _, cx| {
                        scene.window_hovered = hovered;
                        cx.notify();
                    })
                })
                .unwrap();
                settle(cx, window);
                let name = format!("overlay-{name}-{state}");
                capture(cx, window, output, &name);
                assert_picker_pixels(
                    cx,
                    window,
                    output,
                    &name,
                    width,
                    scale,
                    hovered.then_some([0., 0., width, 738.]),
                );
            }
        }
        cx.update_window(window, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }

    let color_scene = cx
        .open_window(size(px(width), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|_| ColorScene {
                frame: image::RgbaImage::from_fn(800, 600, |x, y| {
                    image::Rgba(match (x >= 400, y >= 300) {
                        (false, false) => [240, 80, 64, 255],
                        (true, false) => [64, 112, 192, 255],
                        (false, true) => [48, 160, 104, 255],
                        (true, true) => [184, 96, 192, 255],
                    })
                }),
                pointer: point(px(0.), px(0.)),
            })
        })
        .unwrap();
    let color_window = color_scene.into();
    for (name, x, y, expected) in [
        ("center", width / 2., 369., [184_u8, 96, 192]),
        ("top-left", 1., 1., [240, 80, 64]),
        ("top-right", width - 1., 1., [64, 112, 192]),
        ("bottom-left", 1., 737., [48, 160, 104]),
        ("bottom-right", width - 1., 737., [184, 96, 192]),
    ] {
        color_scene
            .update(cx, |scene, _, cx| {
                scene.pointer = point(px(x), px(y));
                cx.notify();
            })
            .unwrap();
        settle(cx, color_window);
        let card = super::measured("color-picker-card", 0);
        let loupe = super::measured("color-picker-loupe", 0);
        assert!(
            card.left() >= px(0.)
                && card.top() >= px(0.)
                && card.right() <= px(width)
                && card.bottom() <= px(738.),
            "color picker {name} is clipped: {card:?}"
        );
        let image = cx.capture_screenshot(color_window).unwrap();
        let x = (f32::from(loupe.left() + px(52.5)) * scale).round() as u32;
        let y = (f32::from(loupe.top() + px(52.5)) * scale).round() as u32;
        let pixel = image.get_pixel(x, y).0;
        assert!(
            pixel[..3]
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= 1),
            "color picker {name} darkened the sampled pixel: {pixel:?}"
        );
        capture(
            cx,
            color_window,
            output,
            &format!("overlay-color-picker-{name}"),
        );
    }
    cx.update_window(color_window, |_, window, _| {
        window.resize(size(px(400.), px(300.)))
    })
    .unwrap();
    color_scene
        .update(cx, |scene, _, cx| {
            scene.pointer = point(px(190.), px(120.));
            cx.notify();
        })
        .unwrap();
    settle(cx, color_window);
    let card = super::measured("color-picker-card", 0);
    let loupe = super::measured("color-picker-loupe", 0);
    assert!(
        card.left() >= px(0.)
            && card.top() >= px(0.)
            && card.right() <= px(400.)
            && card.bottom() <= px(300.)
            && (px(190.) < card.left()
                || px(190.) > card.right()
                || px(120.) < card.top()
                || px(120.) > card.bottom()),
        "narrow color picker clips or covers its sample: {card:?}"
    );
    let image = cx.capture_screenshot(color_window).unwrap();
    let x = (f32::from(loupe.left() + px(52.5)) * scale).round() as u32;
    let y = (f32::from(loupe.top() + px(52.5)) * scale).round() as u32;
    let pixel = image.get_pixel(x, y).0;
    assert!(
        pixel[..3]
            .iter()
            .zip([240_u8, 80, 64])
            .all(|(actual, expected)| actual.abs_diff(expected) <= 1),
        "narrow color picker darkened the sampled pixel: {pixel:?}"
    );
    capture(cx, color_window, output, "overlay-color-picker-narrow");
    cx.update_window(color_window, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();

    for (edge, start_x, end_x, start_y, end_y) in [
        ("top-left", 1., 21., 1., 21.),
        ("top-right", width - 21., width - 1., 1., 21.),
        ("bottom-left", 1., 21., 717., 737.),
        ("bottom-right", width - 21., width - 1., 717., 737.),
    ] {
        let scene = cx
            .open_window(size(px(width), px(738.)), |window, cx| {
                window.set_scale_factor(scale);
                let overlay = cx.new(|cx| {
                    AreaOverlay::with_focus(
                        Bounds::new(point(px(-width), px(-100.)), size(px(width), px(738.))),
                        scale,
                        crate::state::state(cx),
                        CaptureIntent::Screenshot,
                        cx.focus_handle(),
                    )
                });
                cx.new(|_| Scene {
                    overlay,
                    window_hovered: false,
                })
            })
            .unwrap();
        let window = scene.into();
        settle(cx, window);
        event(
            cx,
            window,
            PlatformInput::MouseDown(MouseDownEvent {
                position: point(px(start_x), px(start_y)),
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
                position: point(px(end_x), px(end_y)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
        );
        settle(cx, window);
        cx.update_window(window, |_, window, _| {
            let labels: Vec<_> = window
                .painted_quads()
                .into_iter()
                .filter(|quad| {
                    let bounds = quad.bounds;
                    (start_y - 40.).max(0.) <= bounds.top().0 / scale
                        && bounds.top().0 / scale <= (end_y + 36.).min(738.)
                        && (22. ..=34.).contains(&(bounds.size.height.0 / scale))
                        && (40. ..=130.).contains(&(bounds.size.width.0 / scale))
                })
                .collect();
            assert_eq!(labels.len(), 1, "{edge}: {labels:?}");
            let label = labels[0];
            let bounds = label.bounds;
            let mask = label.content_mask.bounds;
            assert!(
                bounds.left().0 >= 0.
                    && bounds.right().0 <= width * scale
                    && bounds.top().0 >= 0.
                    && bounds.bottom().0 <= 738. * scale
                    && mask.left().0 <= bounds.left().0
                    && mask.right().0 >= bounds.right().0
                    && mask.top().0 <= bounds.top().0
                    && mask.bottom().0 >= bounds.bottom().0,
                "{edge}: label {bounds:?}, mask {mask:?}"
            );
            if edge.starts_with("bottom") {
                assert!(
                    bounds.bottom().0 <= (start_y - 6.) * scale,
                    "{edge}: label overlaps selection {bounds:?}"
                );
            }
        })
        .unwrap();
        capture(
            cx,
            window,
            output,
            &format!("overlay-screenshot-label-{edge}"),
        );
        let screenshot =
            image::open(output.join(format!("gpui-overlay-screenshot-label-{edge}.png")))
                .unwrap()
                .to_rgba8();
        let span = (32. * scale).ceil() as u32;
        let (physical_width, physical_height) = cx
            .update_window(window, |_, window, _| {
                let viewport = window.viewport_size();
                (
                    (f32::from(window.pixel_snap(viewport.width)) * scale).round() as u32,
                    (f32::from(window.pixel_snap(viewport.height)) * scale).round() as u32,
                )
            })
            .unwrap();
        let x_start = if edge.ends_with("left") {
            0
        } else {
            physical_width - span
        };
        let y_start = if edge.starts_with("top") {
            0
        } else {
            physical_height - span
        };
        for x in x_start..x_start + span {
            assert_eq!(
                screenshot.get_pixel(x, (369. * scale).round() as u32).0,
                [110, 115, 121, 255],
                "{edge}: vertical scrim seam at {x}"
            );
        }
        for y in y_start..y_start + span {
            assert_eq!(
                screenshot
                    .get_pixel((width / 2. * scale).round() as u32, y)
                    .0,
                [110, 115, 121, 255],
                "{edge}: horizontal scrim seam at {y}"
            );
        }
        if edge == "top-left" {
            for (start, end) in [(100.25, 220.75), (300.5, 425.25), (600.75, 720.25)] {
                event(
                    cx,
                    window,
                    PlatformInput::MouseDown(MouseDownEvent {
                        position: point(px(start), px(200.)),
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
                        position: point(px(end), px(320.)),
                        pressed_button: Some(MouseButton::Left),
                        modifiers: Modifiers::none(),
                    }),
                );
                settle(cx, window);
                let selected = cx
                    .update_window(window, |_, window, _| {
                        window.painted_quads().iter().any(|quad| {
                            [
                                (quad.bounds.origin.x.0 / scale, start),
                                (quad.bounds.origin.y.0 / scale, 200.),
                                (quad.bounds.size.width.0 / scale, end - start),
                                (quad.bounds.size.height.0 / scale, 120.),
                            ]
                            .iter()
                            .all(|(actual, expected)| (actual - expected).abs() <= 1. / scale)
                        })
                    })
                    .unwrap();
                assert!(selected, "selection {start}..{end} was not painted");
                let screenshot = cx.capture_screenshot(window).unwrap();
                let center = (start + end) / 2.;
                let source = if center < 440. {
                    [64, 112, 192, 255]
                } else {
                    [220, 230, 242, 255]
                };
                let dimmed = [source[0] / 2, source[1] / 2, source[2] / 2, 255];
                let quarter = start + (end - start) / 4.;
                for (x, y, expected) in [
                    (center, 260., source),
                    (quarter, 204., source),
                    (quarter, 316., source),
                    (start + 4., 230., source),
                    (end - 4., 230., source),
                    (quarter, 196., dimmed),
                    (quarter, 324., dimmed),
                    (start - 4., 230., dimmed),
                    (end + 4., 230., dimmed),
                ] {
                    assert_eq!(
                        screenshot
                            .get_pixel((x * scale).round() as u32, (y * scale).round() as u32)
                            .0,
                        expected,
                        "selection {start}..{end}: wrong pixel at ({x}, {y})"
                    );
                }
                let [left, top, right, bottom] = cx
                    .update_window(window, |_, window, _| {
                        let left = window.pixel_snap(px(start));
                        let top = window.pixel_snap(px(200.));
                        let right = left + window.pixel_snap(px(end - start));
                        let bottom = top + window.pixel_snap(px(120.));
                        [left, top, right, bottom]
                            .map(|value| (f32::from(value) * scale).round() as u32)
                    })
                    .unwrap();
                let (quarter_x, side_y) = (
                    (quarter * scale).round() as u32,
                    (230. * scale).round() as u32,
                );
                for (x, y) in [
                    (left - 1, side_y),
                    (right, side_y),
                    (quarter_x, top - 1),
                    (quarter_x, bottom),
                ] {
                    let pixel = screenshot.get_pixel(x, y).0;
                    assert!(
                        pixel[..3]
                            .iter()
                            .zip(dimmed)
                            .all(|(actual, base)| *actual < base),
                        "selection {start}..{end}: missing outline at ({x}, {y}): {pixel:?}"
                    );
                }
                for x in 0..physical_width {
                    assert_eq!(
                        screenshot.get_pixel(x, (10. * scale).round() as u32).0,
                        [110, 115, 121, 255],
                        "selection {start}..{end}: scrim seam at {x}"
                    );
                }
            }
        }
        cx.update_window(window, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
}

fn assert_picker_pixels(
    cx: &mut HeadlessAppContext,
    window: gpui::AnyWindowHandle,
    output: &Path,
    name: &str,
    width: f32,
    scale: f32,
    expected: Option<[f32; 4]>,
) {
    let image = image::open(output.join(format!("gpui-{name}.png")))
        .unwrap()
        .to_rgba8();
    let (physical_width, scan_y, edges) = cx
        .update_window(window, |_, window, _| {
            let physical = |value| (f32::from(window.pixel_snap(px(value))) * scale).round() as i32;
            (
                physical(width) as u32,
                physical(700.) as u32,
                expected.map(|[left, top, width, height]| {
                    [
                        physical(left),
                        physical(top),
                        physical(left + width),
                        physical(top + height),
                    ]
                }),
            )
        })
        .unwrap();
    for x in 0..physical_width {
        let highlighted = edges.is_some_and(|[left, top, right, bottom]| {
            x as i32 >= left
                && (scan_y as i32) >= top
                && (x as i32) < right
                && (scan_y as i32) < bottom
        });
        let expected = if highlighted {
            [220, 230, 242, 255]
        } else {
            [110, 115, 121, 255]
        };
        let actual = image.get_pixel(x, scan_y).0;
        assert!(
            actual
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= 1),
            "{name}: scrim gap at physical ({x}, {scan_y}): {actual:?}, expected {expected:?}"
        );
    }
    for x in [
        20.,
        80.,
        300.,
        430.,
        450.,
        500.,
        540.,
        width - 300.,
        width - 20.,
    ] {
        for y in [10., 55., 155., 180., 300., 380., 450., 550., 720.] {
            if y < 180. && (x - width / 2.).abs() < 200. {
                continue;
            }
            let original: [u8; 3] = if (40. ..440.).contains(&x) && (80. ..360.).contains(&y) {
                [64, 112, 192]
            } else if (width - 360. ..width - 40.).contains(&x) && (438. ..678.).contains(&y) {
                [240, 160, 80]
            } else {
                [220, 230, 242]
            };
            let highlighted = expected.is_some_and(|[left, top, width, height]| {
                x >= left && x < left + width && y >= top && y < top + height
            });
            let actual = image.get_pixel((x * scale) as u32, (y * scale) as u32);
            for channel in 0..3 {
                let expected = if highlighted {
                    original[channel]
                } else {
                    original[channel] / 2
                };
                assert!(
                    actual[channel].abs_diff(expected) <= 1,
                    "{name}: incorrect scrim at ({x}, {y}): {actual:?}, expected {expected} in channel {channel}"
                );
            }
        }
    }
}
