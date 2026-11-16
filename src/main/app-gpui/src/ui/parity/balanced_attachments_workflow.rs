use gpui::AppContext;
use herogpui::gpui;

use crate::editor::annotations::Annotation;
use crate::editor::layers::Edge;
use crate::editor::window::EditorWindow;
use crate::history_store::{HistoryItem, HistoryItemType};

use super::{measured, px, settle, size, HeadlessAppContext, Path};

pub(super) fn run(cx: &mut HeadlessAppContext, scratch: &Path) {
    for scale in [1., 1.25, 2.] {
        for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            for inset in [0., 8.] {
                for (corners, shadow) in [(20., 0.), (20., 50.), (0., 50.)] {
                    for background in [false, true] {
                        run_case(
                            cx,
                            scratch,
                            scale,
                            edge,
                            (inset, corners, shadow, background, "blackout"),
                        );
                    }
                }
            }
        }
    }
    for scale in [1., 1.25, 2.] {
        for inset in [0., 8.] {
            for background in [false, true] {
                for style in ["blur", "pixelate"] {
                    run_case(
                        cx,
                        scratch,
                        scale,
                        Edge::Right,
                        (inset, 20., 0., background, style),
                    );
                }
            }
        }
    }
}

fn run_case(
    cx: &mut HeadlessAppContext,
    scratch: &Path,
    scale: f32,
    edge: Edge,
    effects: (f64, f64, f64, bool, &str),
) {
    let (inset, corners, shadow, background, style) = effects;
    let padding = if background { 20. } else { 0. };
    let name = format!(
        "balanced-attachment-{scale}-{edge:?}-{inset}-{corners}-{shadow}-{background}-{style}"
    );
    let source = scratch.join(format!("{name}.png"));
    image::RgbaImage::from_fn(200, 160, |x, y| {
        if (110..120).contains(&x) && (70..90).contains(&y) {
            image::Rgba([64, 112, 192, 128])
        } else if (40..160).contains(&x) && (30..130).contains(&y) {
            image::Rgba([64, 112, 192, 255])
        } else if (5..195).contains(&x) && (5..155).contains(&y) {
            image::Rgba([230, 230, 230, 255])
        } else {
            image::Rgba([255, 255, 255, 255])
        }
    })
    .save(&source)
    .unwrap();
    let attachment = scratch.join(format!("{name}-attached.png"));
    image::RgbaImage::from_pixel(100, 100, image::Rgba([240, 80, 64, 255]))
        .save(&attachment)
        .unwrap();
    crate::history_store::add_item(
        HistoryItem {
            id: name.clone(),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: Some(serde_json::json!({"wallpaper": {
                "balance": true,
                "corners": corners,
                "inset": inset,
                "shadow": shadow,
                "padding": padding,
                "gradient": background.then(|| serde_json::json!({
                    "id": "attachment-background",
                    "colors": ["#2468ac"],
                    "angle": 0
                }))
            }})),
            duration: None,
        },
        100,
        true,
    );
    let mut previous = None;
    for reopened in [false, true] {
        let editor = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                window.set_scale_factor(scale);
                cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
            })
            .unwrap();
        let window = editor.into();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                if !reopened {
                    editor.push_image_layer(
                        attachment.to_string_lossy().into_owned(),
                        100.,
                        100.,
                        edge,
                        cx,
                    );
                    let native_scale =
                        crate::editor::wallpaper::native_scale(200., 160., f64::from(scale));
                    let native_inset = (inset * native_scale).round();
                    let seam = match edge {
                        Edge::Left => (30. - native_inset - 12., 70., 24., 20.),
                        Edge::Right => (170. + native_inset - 12., 70., 24., 20.),
                        Edge::Top => (70., 20. - native_inset - 12., 20., 24.),
                        Edge::Bottom => (70., 140. + native_inset - 12., 20., 24.),
                        Edge::Primary => unreachable!(),
                    };
                    editor.history.push(vec![
                        Annotation::Rectangle {
                            id: "shape".into(),
                            x: 70.,
                            y: 70.,
                            width: 20.,
                            height: 20.,
                            stroke: "#ff00ff".into(),
                            stroke_width: 1.,
                            fill: Some("#ff00ff".into()),
                        },
                        Annotation::Redact {
                            id: "blackout".into(),
                            x: 120.,
                            y: 90.,
                            width: 10.,
                            height: 10.,
                            style: "blackout".into(),
                            intensity: 5.,
                        },
                        Annotation::Redact {
                            id: "seam".into(),
                            x: seam.0,
                            y: seam.1,
                            width: seam.2,
                            height: seam.3,
                            style: style.into(),
                            intensity: 5.,
                        },
                    ]);
                }
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                let native_scale =
                    crate::editor::wallpaper::native_scale(200., 160., f64::from(scale));
                editor.zoom = (1. / native_scale) as f32;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        let (png, matte, stage) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            assert_eq!(
                editor.snapshot.borrow().balance_crop,
                Some((30., 20., 30., 20.))
            );
            let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
            (
                editor.export_png().unwrap(),
                image::Rgba([
                    (theme.r * 255.).round() as u8,
                    (theme.g * 255.).round() as u8,
                    (theme.b * 255.).round() as u8,
                    255,
                ]),
                measured("editor-stage", 0),
            )
        });
        if let Some(previous) = &previous {
            assert_eq!(&png, previous, "{name} must reopen unchanged");
        } else {
            previous = Some(png.clone());
        }
        let saved = image::load_from_memory(&png).unwrap().to_rgba8();
        let native_scale = crate::editor::wallpaper::native_scale(200., 160., f64::from(scale));
        let inset = (inset * native_scale).round() as u32;
        let padding = (padding * native_scale).round() as u32;
        let primary_width = 140 + inset * 2;
        let primary_height = 120 + inset * 2;
        let (primary_x, primary_y, attachment_x, attachment_y, width, height) = match edge {
            Edge::Left => (
                primary_height,
                0,
                0,
                0,
                primary_width + primary_height,
                primary_height,
            ),
            Edge::Right => (
                0,
                0,
                primary_width,
                0,
                primary_width + primary_height,
                primary_height,
            ),
            Edge::Top => (
                0,
                primary_width,
                0,
                0,
                primary_width,
                primary_height + primary_width,
            ),
            Edge::Bottom => (
                0,
                0,
                0,
                primary_height,
                primary_width,
                primary_height + primary_width,
            ),
            Edge::Primary => unreachable!(),
        };
        let (primary_x, primary_y, attachment_x, attachment_y, width, height) = (
            primary_x + padding,
            primary_y + padding,
            attachment_x + padding,
            attachment_y + padding,
            width + padding * 2,
            height + padding * 2,
        );
        assert_eq!(
            saved.dimensions(),
            (width, height),
            "{name} must balance the primary before layout"
        );
        assert_eq!(
            saved
                .get_pixel(primary_x + inset + 50, primary_y + inset + 60)
                .0,
            [255, 0, 255, 255]
        );
        assert_eq!(
            saved
                .get_pixel(primary_x + inset + 95, primary_y + inset + 75)
                .0,
            [0, 0, 0, 255]
        );
        let attachment_side = if matches!(edge, Edge::Left | Edge::Right) {
            primary_height
        } else {
            primary_width
        };
        assert_eq!(
            saved
                .get_pixel(
                    attachment_x + attachment_side / 2,
                    attachment_y + attachment_side / 2
                )
                .0,
            [240, 80, 64, 255]
        );
        let (seam_x, seam_y) = match edge {
            Edge::Left => (primary_x, padding + inset + 60),
            Edge::Right => (padding + primary_width, padding + inset + 60),
            Edge::Top => (padding + inset + 50, primary_y),
            Edge::Bottom => (padding + inset + 50, padding + primary_height),
            Edge::Primary => unreachable!(),
        };
        for (index, (x, y)) in if matches!(edge, Edge::Left | Edge::Right) {
            [(seam_x - 2, seam_y), (seam_x + 2, seam_y)]
        } else {
            [(seam_x, seam_y - 2), (seam_x, seam_y + 2)]
        }
        .into_iter()
        .enumerate()
        {
            if style == "blackout" {
                assert_eq!(saved.get_pixel(x, y).0, [0, 0, 0, 255], "{name} seam");
            } else {
                let original = if index == 0 {
                    [230, 230, 230, 255]
                } else {
                    [240, 80, 64, 255]
                };
                assert_ne!(saved.get_pixel(x, y).0, original, "{name} seam");
            }
        }
        let mut expected = image::RgbaImage::from_pixel(width, height, matte);
        image::imageops::overlay(&mut expected, &saved, 0, 0);
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(stage.left()) * scale).round() as u32;
        let top = (f32::from(stage.top()) * scale).round() as u32;
        let preview = image::imageops::crop_imm(&screenshot, left, top, width, height).to_image();
        let differences: Vec<_> = preview
            .pixels()
            .zip(expected.pixels())
            .map(|(a, e)| {
                a.0.iter()
                    .zip(e.0)
                    .map(|(a, e)| a.abs_diff(e))
                    .max()
                    .unwrap()
            })
            .collect();
        let mismatched = differences
            .iter()
            .filter(|difference| **difference > 2)
            .count();
        let worst = differences.into_iter().max().unwrap_or_default();
        if mismatched > 1
            || worst > 3
            || (scale == 1. && edge == Edge::Right && inset > 0 && shadow == 0. && !reopened)
        {
            preview
                .save(scratch.join(format!("{name}-preview.png")))
                .unwrap();
            expected
                .save(scratch.join(format!("{name}-expected.png")))
                .unwrap();
            saved
                .save(scratch.join(format!("{name}-saved.png")))
                .unwrap();
        }
        assert!(
            mismatched <= 1 && worst <= 3,
            "{name}, reopened {reopened}: {mismatched} pixels above two, maximum {worst}"
        );
        cx.update_window(window, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
}
