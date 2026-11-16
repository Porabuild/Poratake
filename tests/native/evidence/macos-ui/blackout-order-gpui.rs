#[test]
fn blackout_order_gpui_probe() {
    let mut cases = Vec::new();
    for scale in [1.0_f64, 1.25, 2.0] {
        for offset in [0.0_f64, 0.25, 0.5] {
            let width = (100.0 * scale).round() as u32;
            let height = (80.0 * scale).round() as u32;
            let x = (32.0 + offset) * scale;
            let y = (22.0 + offset) * scale;
            let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
                width,
                height,
                image::Rgba([64, 128, 192, 255]),
            ));
            let shape = Annotation::Rectangle {
                id: "shape".into(),
                x: (30.0 + offset) * scale,
                y: (20.0 + offset) * scale,
                width: 24.0 * scale,
                height: 24.0 * scale,
                stroke: "#ff00ff".into(),
                stroke_width: scale,
                fill: Some("#ff00ff".into()),
            };
            let blackout = Annotation::Redact {
                id: "blackout".into(),
                x,
                y,
                width: 20.0 * scale,
                height: 20.0 * scale,
                style: "blackout".into(),
                intensity: 2.0,
            };
            for (order, annotations) in [
                ("shape-blackout", vec![shape.clone(), blackout.clone()]),
                ("blackout-shape", vec![blackout, shape]),
            ] {
                let image = compose(
                    Some(&base),
                    width,
                    height,
                    &annotations,
                    &WallpaperSettings::default(),
                    1.0,
                );
                let rgba: Vec<Vec<[u8; 4]>> = (0..4)
                    .map(|row| {
                        (0..4)
                            .map(|col| {
                                image
                                    .get_pixel(x.floor() as u32 + col, y.floor() as u32 + row)
                                    .0
                            })
                            .collect()
                    })
                    .collect();
                cases.push(serde_json::json!({"scale": scale, "offset": offset, "order": order, "x": x, "y": y, "rgba": rgba}));
            }
        }
    }
    eprintln!(
        "BLACKOUT_ORDER_GPUI {}",
        serde_json::to_string(&cases).unwrap()
    );
}
