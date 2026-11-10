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
                assert_picker_pixels(output, &name, width, scale, expected);
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
}

fn assert_picker_pixels(
    output: &Path,
    name: &str,
    width: f32,
    scale: f32,
    expected: Option<[f32; 4]>,
) {
    let image = image::open(output.join(format!("gpui-{name}.png")))
        .unwrap()
        .to_rgba8();
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
