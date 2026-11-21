use super::*;

#[test]
fn rounded_annotation_patches_match_the_flattened_export() {
    use crate::editor::wallpaper::{GradientOption, WallpaperSettings};

    let wallpaper = WallpaperSettings {
        gradient: Some(GradientOption {
            id: "rounded-test".into(),
            colors: vec!["#203060".into()],
            angle: 0.,
        }),
        padding: 10.,
        corners: 24.,
        ..Default::default()
    };
    for alpha in [128, 255] {
        let source = image::RgbaImage::from_fn(64, 64, |x, y| {
            image::Rgba([(x * 3) as u8, (y * 3) as u8, 110, alpha])
        });
        let base = image::DynamicImage::ImageRgba8(source.clone());
        let mut bgra = source.into_raw();
        for pixel in bgra.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let (matte, edges, composite) = crate::editor::export::render_backdrop(
            &wallpaper,
            84,
            84,
            64.,
            64.,
            None,
            &[],
            Some(&base),
            1.,
            [18, 18, 20],
        )
        .unwrap();
        assert!(!composite);
        let surface = Surface {
            image_id: None,
            width: 64,
            height: 64,
            offset_x: 0.,
            offset_y: 0.,
            source_is_composed: false,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: Some(RoundedClip {
                width: 64,
                height: 64,
                offset_x: 10,
                offset_y: 10,
                edges: &edges,
            }),
            pixels: Some(&bgra),
            matte: Some([18, 18, 20]),
            matte_image: Some((&matte, 10., 10.)),
            composite_backdrop: false,
        };
        for style in ["blackout", "blur", "pixelate", "blur-pixelate"] {
            let mut annotations = vec![
                Annotation::Rectangle {
                    id: "shape".into(),
                    x: 0.,
                    y: 0.,
                    width: 36.,
                    height: 36.,
                    stroke: "#00ff00".into(),
                    stroke_width: 2.,
                    fill: Some("#00ff00".into()),
                },
                Annotation::Redact {
                    id: "redact".into(),
                    x: 0.,
                    y: 0.,
                    width: 32.,
                    height: 32.,
                    style: if style == "blur-pixelate" {
                        "blur"
                    } else {
                        style
                    }
                    .into(),
                    intensity: 3.,
                },
            ];
            if style == "blur-pixelate" {
                annotations.push(Annotation::Redact {
                    id: "later-pixelate".into(),
                    x: 8.,
                    y: 4.,
                    width: 28.,
                    height: 28.,
                    style: "pixelate".into(),
                    intensity: 3.,
                });
            }
            let export =
                crate::editor::export::compose(Some(&base), 64, 64, &annotations, &wallpaper, 1.);
            let paints = renderer::screenshot_paints(annotations.iter());
            let last = paints.len() - 1;
            let patch = super::patch(paints[last], &paints[..last], &surface, 1.).unwrap();
            let pixels = crate::editor::export::to_rgba(&patch.pixmap);
            for (x, y, pixel) in pixels.enumerate_pixels() {
                let expected = export.get_pixel(
                    (10. + patch.x + f64::from(x)) as u32,
                    (10. + patch.y + f64::from(y)) as u32,
                );
                assert!(
                    pixel
                        .0
                        .iter()
                        .zip(expected.0)
                        .all(|(a, b)| a.abs_diff(b) <= 2),
                    "{style}, alpha {alpha}, at {x},{y}: {pixel:?} != {expected:?}"
                );
            }
        }
    }
}

#[test]
fn rounded_annotation_sampling_matches_the_flattened_export() {
    use crate::editor::wallpaper::{GradientOption, WallpaperSettings};

    let wallpaper = WallpaperSettings {
        gradient: Some(GradientOption {
            id: "rounded-sampling".into(),
            colors: vec!["#203060".into()],
            angle: 0.,
        }),
        padding: 450.,
        corners: 30.,
        ..Default::default()
    };
    let source = image::RgbaImage::from_fn(128, 128, |x, y| {
        image::Rgba([(x * 3) as u8, (y * 3) as u8, 110, 255])
    });
    let base = image::DynamicImage::ImageRgba8(source.clone());
    let mut bgra = source.into_raw();
    for pixel in bgra.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let (matte, edges, _) = crate::editor::export::render_backdrop(
        &wallpaper,
        1028,
        1028,
        128.,
        128.,
        None,
        &[],
        Some(&base),
        1.25,
        [18, 18, 20],
    )
    .unwrap();
    let surface = Surface {
        image_id: None,
        width: 128,
        height: 128,
        offset_x: 0.,
        offset_y: 0.,
        source_is_composed: false,
        align_to_composition_grid: false,
        native_scale: 1.25,
        rounded_clip: Some(RoundedClip {
            width: 128,
            height: 128,
            offset_x: 450,
            offset_y: 450,
            edges: &edges,
        }),
        pixels: Some(&bgra),
        matte: Some([18, 18, 20]),
        matte_image: Some((&matte, 450., 450.)),
        composite_backdrop: false,
    };
    let annotation = Annotation::Rectangle {
        id: "corner".into(),
        x: 0.,
        y: 0.,
        width: 48.,
        height: 48.,
        stroke: "#00ff00".into(),
        stroke_width: 2.,
        fill: Some("#00ff00".into()),
    };
    let export = crate::editor::export::compose(
        Some(&base),
        128,
        128,
        std::slice::from_ref(&annotation),
        &wallpaper,
        1.25,
    );
    let grid = SamplingGrid {
        origin: (224.8, 224.8),
        stage_bounds: (0., 0., 514., 514.),
        viewport: None,
        stage_size: (1028., 1028.),
        offset: (450., 450.),
        zoom: 0.5,
        dpi: 1.25,
    };
    let paint = ScreenshotPaint::Annotation(&annotation);
    let keys = super::begin_frame(&[paint]);
    let native = super::patch(paint, &[], &surface, 1.).unwrap();
    let native_pixels = crate::editor::export::to_rgba(&native.pixmap);
    for (x, y, pixel) in native_pixels.enumerate_pixels() {
        let expected = export.get_pixel(
            (450. + native.x + f64::from(x)) as u32,
            (450. + native.y + f64::from(y)) as u32,
        );
        assert!(
            pixel
                .0
                .iter()
                .zip(expected.0)
                .all(|(a, b)| a.abs_diff(b) <= 2),
            "native patch differs at {x},{y} (origin {},{}): {pixel:?} != {expected:?}",
            native.x,
            native.y
        );
    }
    let actual = super::aligned_image(&keys, 0, paint, &[], &surface, grid).unwrap();
    let size = actual.image.size(0);
    let (width, height) = (u32::from(size.width), u32::from(size.height));
    let origin = (224.8_f64 * 1.25 - 0.5).ceil();
    let sampling = AlignedSampling {
        left: 0.,
        top: 0.,
        x0: origin + actual.x * 0.625,
        y0: origin + actual.y * 0.625,
        scale_x: 642. / 1028.,
        scale_y: 642. / 1028.,
        offset: (450., 450.),
        matte: Some([18, 18, 20]),
    };
    let reference = Patch {
        pixmap: crate::editor::export::from_dynamic_image(&image::DynamicImage::ImageRgba8(export))
            .unwrap(),
        x: -450.,
        y: -450.,
        width: 1028.,
        height: 1028.,
    };
    let mut expected = image::RgbaImage::new(width, height);
    sampling.sample(&mut expected, &reference, (0, 0, width, height));
    let actual = actual.image.as_bytes(0).unwrap();
    let differences: Vec<_> = actual
        .as_chunks::<4>()
        .0
        .iter()
        .zip(expected.as_raw().as_chunks::<4>().0)
        .enumerate()
        .filter(|(_, (a, b))| a.iter().zip(*b).any(|(x, y)| x.abs_diff(*y) > 2))
        .take(10)
        .collect();
    assert!(
        differences.is_empty(),
        "sampled patch differs: {differences:?}"
    );
    let blur = Annotation::Redact {
        id: "blur".into(),
        x: 0.,
        y: 0.,
        width: 30.,
        height: 30.,
        style: "blur".into(),
        intensity: 5.,
    };
    let annotations = [annotation, blur];
    let export =
        crate::editor::export::compose(Some(&base), 128, 128, &annotations, &wallpaper, 1.25);
    let paints = renderer::screenshot_paints(annotations.iter());
    let patch = super::patch(paints[1], &paints[..1], &surface, 1.).unwrap();
    let pixels = crate::editor::export::to_rgba(&patch.pixmap);
    for (x, y, pixel) in pixels.enumerate_pixels() {
        let expected = export.get_pixel(
            (450. + patch.x + f64::from(x)) as u32,
            (450. + patch.y + f64::from(y)) as u32,
        );
        assert!(
            pixel
                .0
                .iter()
                .zip(expected.0)
                .all(|(a, b)| a.abs_diff(b) <= 2),
            "blur patch differs at {x},{y} (origin {},{}): {pixel:?} != {expected:?}",
            patch.x,
            patch.y
        );
    }
}
