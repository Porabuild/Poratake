use gpui::{AppContext, Context, Entity, Render, Window};
use herogpui::gpui;

use crate::editor::window::EditorWindow;
use crate::windows::capture_preview::CapturePreviewWindow;
use crate::windows::registry::{self, WindowKind};

use super::{
    capture, event, hover, measured, point, px, settle, size, Duration, HeadlessAppContext,
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Path, PlatformInput, GEOMETRY,
};

struct HoveredPreview {
    preview: Entity<CapturePreviewWindow>,
}

impl Render for HoveredPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        self.preview
            .update(cx, |preview, cx| preview.surface(true, window, cx))
    }
}

pub(super) fn run(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path, scale: f32) {
    let config = cx.update(|cx| crate::state::state(cx).config.get().preview);
    let history: Vec<_> = crate::history_store::load_history()
        .into_iter()
        .map(|item| item.original_path)
        .collect();
    let colors = [
        [64, 112, 192, 255],
        [239, 77, 59, 255],
        [34, 160, 87, 255],
        [152, 79, 192, 255],
        [232, 169, 38, 255],
    ];
    let paths: Vec<_> = colors
        .iter()
        .enumerate()
        .map(|(index, color)| {
            let path = scratch.join(format!("preview-{index}.png"));
            image::RgbaImage::from_pixel(640, 360, image::Rgba(*color))
                .save(&path)
                .unwrap();
            path
        })
        .collect();
    for corner in ["bottom-right", "top-left", "top-right", "bottom-left"] {
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.preview.corner = corner.into();
                config.preview.auto_dismiss = false;
            });
            CapturePreviewWindow::open(cx, paths[0].clone());
        });
        let preview = cx
            .update(|cx| registry::handle(WindowKind::CapturePreview, cx))
            .unwrap();
        cx.update_window(preview, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, preview);
        GEOMETRY.with_borrow(|geometry| assert_eq!(geometry["capture-previews"].len(), 1));
        let single = measured("capture-previews", 0);
        assert_eq!(single.size, size(px(200.), px(140.)));
        assert_eq!(single.left(), px(4.));
        assert_eq!(
            single.top(),
            px(if corner.starts_with("bottom") {
                460.
            } else {
                4.
            })
        );
        if corner.ends_with("right") {
            capture(cx, preview, output, &format!("preview-{corner}-single"));
            let view = cx
                .update_window(preview, |view, _, _| {
                    view.downcast::<CapturePreviewWindow>().unwrap()
                })
                .unwrap();
            let hovered = cx
                .open_window(size(px(208.), px(604.)), |window, cx| {
                    window.set_scale_factor(scale);
                    cx.new(|_| HoveredPreview { preview: view })
                })
                .unwrap()
                .into();
            settle(cx, hovered);
            for (name, x, y) in [
                ("close", 24., 20.),
                ("delete", 184., 20.),
                ("copy", 24., 120.),
                ("upload", 184., 120.),
            ] {
                hover(cx, hovered, x, f32::from(single.top()) + y);
                capture(
                    cx,
                    hovered,
                    output,
                    &format!("preview-{corner}-tooltip-{name}"),
                );
                cx.update_window(hovered, |_, window, _| {
                    let tips: Vec<_> = window
                        .painted_quads()
                        .into_iter()
                        .filter(|quad| {
                            let height = quad.bounds.size.height.0 / scale;
                            (31. ..=35.).contains(&height) && quad.bounds.size.width.0 / scale > 30.
                        })
                        .collect();
                    assert!(!tips.is_empty(), "{corner}/{name}: tooltip must paint");
                    for tip in tips {
                        let bounds = tip.bounds;
                        assert!(
                            bounds.left().0 >= f32::from(single.left()) * scale - 0.5
                                && bounds.right().0 <= f32::from(single.right()) * scale + 0.5
                                && bounds.top().0 >= f32::from(single.top()) * scale - 0.5
                                && bounds.bottom().0 <= f32::from(single.bottom()) * scale + 0.5,
                            "{corner}/{name}: tooltip {bounds:?} must fit its preview {single:?}"
                        );
                    }
                })
                .unwrap();
            }
            hover(cx, hovered, -5., -5.);
            cx.advance_clock(Duration::from_millis(600));
            hover(
                cx,
                hovered,
                f32::from(single.center().x),
                f32::from(single.center().y),
            );
            cx.update_window(hovered, |_, window, _| {
                assert!(
                    window.painted_quads().into_iter().all(|quad| {
                        let height = quad.bounds.size.height.0 / scale;
                        !(31. ..=35.).contains(&height) || quad.bounds.size.width.0 / scale <= 30.
                    }),
                    "Edit is already labelled and must not show a tooltip"
                );
            })
            .unwrap();
            capture(cx, hovered, output, &format!("preview-{corner}-edit-hover"));
            cx.update_window(hovered, |_, window, _| window.remove_window())
                .unwrap();
            settle(cx, preview);
        }
        for path in &paths[1..] {
            cx.update(|cx| CapturePreviewWindow::open(cx, path.clone()));
            settle(cx, preview);
        }
        GEOMETRY.with_borrow(|geometry| assert_eq!(geometry["capture-previews"].len(), 4));
        let rendered = cx.capture_screenshot(preview).unwrap();
        for index in 0..4 {
            let bounds = measured("capture-previews", index);
            assert_eq!(bounds.size, size(px(200.), px(140.)));
            let top = if corner.starts_with("bottom") {
                3 - index
            } else {
                index
            };
            assert_eq!(bounds.origin, point(px(4.), px(4. + top as f32 * 152.)));
            let center = bounds.center();
            let pixel = rendered.get_pixel(
                (f32::from(center.x) * scale).round() as u32,
                (f32::from(center.y) * scale).round() as u32,
            );
            assert_eq!(
                pixel.0,
                colors[index + 1],
                "{corner}: newest four captures must remain in order"
            );
        }
        if corner.ends_with("right") {
            capture(cx, preview, output, &format!("preview-{corner}-stack"));
        }
        let before = cx.update(|cx| cx.windows());
        let position = measured("capture-previews", 3).center();
        event(
            cx,
            preview,
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                button: MouseButton::Left,
                click_count: 2,
                modifiers: Modifiers::none(),
                first_mouse: false,
            }),
        );
        event(
            cx,
            preview,
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                button: MouseButton::Left,
                click_count: 2,
                modifiers: Modifiers::none(),
            }),
        );
        settle(cx, preview);
        GEOMETRY.with_borrow(|geometry| assert_eq!(geometry["capture-previews"].len(), 3));
        let editor = cx.update(|cx| {
            cx.windows()
                .into_iter()
                .find(|handle| {
                    !before.contains(handle) && handle.downcast::<EditorWindow>().is_some()
                })
                .expect("double-click must open the selected capture in the editor")
        });
        let saved = cx
            .update_window(editor, |view, _, cx| {
                let editor = view.downcast::<EditorWindow>().unwrap();
                let editor = editor.read(cx);
                assert!(editor.history.current().is_empty());
                assert_eq!(editor.file_path, paths[4].to_string_lossy());
                editor.export_png().unwrap()
            })
            .unwrap();
        assert_eq!(
            image::load_from_memory(&saved).unwrap().to_rgba8(),
            image::open(&paths[4]).unwrap().to_rgba8()
        );
        cx.update_window(editor, |_, window, _| window.remove_window())
            .unwrap();
        cx.update(|cx| registry::close(WindowKind::CapturePreview, cx));
        assert!(paths.iter().all(|path| path.exists()));
    }
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.preview.auto_dismiss = true;
            config.preview.auto_dismiss_seconds = 1.;
        });
        CapturePreviewWindow::open(cx, paths[0].clone());
    });
    cx.run_until_parked();
    cx.advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    cx.advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    assert!(!cx.update(|cx| registry::is_open(WindowKind::CapturePreview, cx)));
    assert!(paths.iter().all(|path| path.exists()));
    assert_eq!(
        crate::history_store::load_history()
            .into_iter()
            .map(|item| item.original_path)
            .collect::<Vec<_>>(),
        history
    );
    cx.update(|cx| {
        crate::state::state(cx)
            .config
            .update(|settings| settings.preview = config)
    });
    println!("capture preview: four corners, newest-four stack, editor handoff, pixels and auto-dismiss passed");
}
