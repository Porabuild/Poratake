use gpui::AppContext;
use herogpui::gpui;

use crate::editor::annotations::Annotation;
use crate::editor::window::EditorWindow;
use crate::history_store::{HistoryItem, HistoryItemType};

use super::{measured, px, settle, size, HeadlessAppContext, Path};

pub(super) fn run(cx: &mut HeadlessAppContext, scratch: &Path) {
    for scale in [1., 1.25, 2.] {
        for inset in [0., 8.] {
            for (corners, shadow) in [(20., 0.), (20., 50.), (0., 50.)] {
                for background in [false, true] {
                    for transparent in [false, true] {
                        run_case(
                            cx,
                            scratch,
                            scale,
                            inset,
                            (corners, shadow, background, transparent),
                        );
                    }
                }
            }
        }
    }
}

fn run_case(
    cx: &mut HeadlessAppContext,
    scratch: &Path,
    scale: f32,
    inset: f64,
    variant: (f64, f64, bool, bool),
) {
    let (corners, shadow, background, transparent) = variant;
    let padding = if background { 20. } else { 0. };
    let name =
        format!("balanced-effects-{scale}-{inset}-{corners}-{shadow}-{background}-{transparent}");
    let source = scratch.join(format!("{name}.png"));
    image::RgbaImage::from_fn(200, 160, |x, y| {
        if transparent && (110..120).contains(&x) && (70..90).contains(&y) {
            image::Rgba([64, 112, 192, 128])
        } else if ((55..75).contains(&x) || (100..120).contains(&x)) && (100..120).contains(&y) {
            if (x / 2 + y / 2) % 2 == 0 {
                image::Rgba([16, 32, 64, 255])
            } else {
                image::Rgba([224, 208, 176, 255])
            }
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
                    "id": "balanced-effects-background",
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
                            id: "pixelation".into(),
                            x: 55.,
                            y: 100.,
                            width: 20.,
                            height: 20.,
                            style: "pixelate".into(),
                            intensity: 5.,
                        },
                        Annotation::Redact {
                            id: "blur".into(),
                            x: 100.,
                            y: 100.,
                            width: 20.,
                            height: 20.,
                            style: "blur".into(),
                            intensity: 5.,
                        },
                    ]);
                }
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
        let native_inset = (inset * native_scale).round() as u32;
        let native_padding = (padding * native_scale).round() as u32;
        assert_eq!(
            saved.dimensions(),
            (
                140 + (native_inset + native_padding) * 2,
                120 + (native_inset + native_padding) * 2
            )
        );
        assert_eq!(
            saved
                .get_pixel(
                    50 + native_inset + native_padding,
                    60 + native_inset + native_padding
                )
                .0,
            [255, 0, 255, 255]
        );
        assert_eq!(
            saved
                .get_pixel(
                    95 + native_inset + native_padding,
                    75 + native_inset + native_padding
                )
                .0,
            [0, 0, 0, 255]
        );
        if transparent && !background && shadow == 0. && inset == 0. {
            assert_eq!(saved.get_pixel(85, 55).0[3], 128);
        }
        for x in [30, 75] {
            assert_ne!(
                saved
                    .get_pixel(
                        x + native_inset + native_padding,
                        85 + native_inset + native_padding
                    )
                    .0,
                [16, 32, 64, 255]
            );
        }
        let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
        image::imageops::overlay(&mut expected, &saved, 0, 0);
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(stage.left()) * scale).round() as u32;
        let top = (f32::from(stage.top()) * scale).round() as u32;
        let preview =
            image::imageops::crop_imm(&screenshot, left, top, saved.width(), saved.height())
                .to_image();
        let differences: Vec<_> = preview
            .pixels()
            .zip(expected.pixels())
            .map(|(actual, expected)| {
                actual
                    .0
                    .iter()
                    .zip(expected.0)
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
            || (scale == 1.
                && inset == 8.
                && corners == 20.
                && shadow == 0.
                && !background
                && !transparent
                && !reopened)
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
