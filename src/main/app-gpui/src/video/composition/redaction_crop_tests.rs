#[test]
fn source_resolution_crop_preserves_overlapping_video_redactions() {
    use crate::editor::annotations::Annotation;
    use crate::windows::video_editor::model::DrawingSegment;

    let mut config = Config::new(128.0, 96.0, state());
    config.state.wallpaper.enabled = true;
    config.state.wallpaper.padding = 12.0;
    config.state.wallpaper.corners = 8.0;
    config.state.wallpaper.gradient = Some(serde_json::json!({
        "id": "redaction-crop",
        "colors": ["#80402080", "#20408080"],
        "angle": 135.0,
    }));
    config.state.drawing_segments.push(DrawingSegment {
        id: "drawing".into(),
        start_time: 0.0,
        end_time: 3.0,
        canvas_width: 152.0,
        canvas_height: 120.0,
        annotations: vec![
            Annotation::Rectangle {
                id: "before".into(),
                x: 40.0,
                y: 30.0,
                width: 50.0,
                height: 40.0,
                stroke: "#ffffff".into(),
                stroke_width: 2.0,
                fill: Some("#e04040".into()),
            },
            Annotation::Redact {
                id: "pixelate".into(),
                x: 75.5,
                y: 35.5,
                width: 40.0,
                height: 30.0,
                style: "pixelate".into(),
                intensity: 5.0,
            },
            Annotation::Redact {
                id: "blur".into(),
                x: 60.5,
                y: 45.5,
                width: 35.0,
                height: 25.0,
                style: "blur".into(),
                intensity: 1.0,
            },
            Annotation::Rectangle {
                id: "after".into(),
                x: 80.0,
                y: 55.0,
                width: 20.0,
                height: 20.0,
                stroke: "#00ff00".into(),
                stroke_width: 2.0,
                fill: None,
            },
        ],
    });
    let mut video = source(128, 96, Color::from_rgba8(0, 0, 0, 255));
    for (index, pixel) in video.data_mut().chunks_exact_mut(4).enumerate() {
        let x = index % 128;
        let y = index / 128;
        pixel[0] = if x % 2 == 0 { 255 } else { 0 };
        pixel[1] = (y * 7 % 256) as u8;
        pixel[2] = (x * 11 % 256) as u8;
    }
    let frames = Frames {
        video: Some(video.as_ref()),
        camera: None,
    };
    let mut engine = Engine::new(config);
    let full = engine.render_frame(0.5, frames).unwrap();
    let mut crop = Canvas::new(110, 100).unwrap();
    crop.translate(-20.0, -10.0);
    engine.render_into(&mut crop, 0.5, frames);
    let crop = crop.into_pixmap();
    for y in 15..85 {
        for x in 25..115 {
            let full_index = (y * 152 + x) * 4;
            let crop_index = ((y - 10) * 110 + x - 20) * 4;
            assert_eq!(
                &full.data()[full_index..full_index + 4],
                &crop.data()[crop_index..crop_index + 4],
                "{x},{y}"
            );
        }
    }
    let mut reference = Canvas::new(76, 60).unwrap();
    reference.draw_pixmap(full.as_ref(), 0.0, 0.0, 76.0, 60.0);
    let preview = engine.render_frame_at(0.5, frames, 76, 60).unwrap();
    let mut corrected = Canvas::from_pixmap(preview.clone());
    corrected.draw_pixmap_blended(
        crop.as_ref(),
        10.0,
        5.0,
        55.0,
        50.0,
        tiny_skia::BlendMode::Source,
    );
    let mut before = 0_u64;
    let mut after = 0_u64;
    for y in 15..40 {
        for x in 20..55 {
            let index = (y * 76 + x) * 4;
            for channel in 0..4 {
                let expected = reference.pixmap().data()[index + channel];
                before += u64::from(preview.data()[index + channel].abs_diff(expected));
                after += u64::from(corrected.pixmap().data()[index + channel].abs_diff(expected));
            }
        }
    }
    assert!(before > 0);
    assert_eq!(after, 0);
}

#[test]
#[ignore]
fn source_resolution_redaction_crop_timing() {
    use crate::editor::annotations::Annotation;
    use crate::windows::video_editor::model::DrawingSegment;
    use std::time::Instant;

    let mut config = Config::new(3840.0, 2160.0, state());
    config.state.drawing_segments.push(DrawingSegment {
        id: "drawing".into(),
        start_time: 0.0,
        end_time: 3.0,
        canvas_width: 3840.0,
        canvas_height: 2160.0,
        annotations: vec![Annotation::Redact {
            id: "pixelate".into(),
            x: 1000.0,
            y: 500.0,
            width: 800.0,
            height: 600.0,
            style: "pixelate".into(),
            intensity: 5.0,
        }],
    });
    let video = source(3840, 2160, Color::from_rgba8(32, 64, 128, 255));
    let frames = Frames {
        video: Some(video.as_ref()),
        camera: None,
    };
    let mut engine = Engine::new(config);
    let mut unredacted = Engine::new(Config::new(3840.0, 2160.0, state()));
    let mut preview_times = Vec::new();
    let mut patch_times = Vec::new();
    let mut unredacted_patch_times = Vec::new();
    let mut full_times = Vec::new();
    for _ in 0..8 {
        let start = Instant::now();
        engine.render_frame_scaled(0.5, frames, 960, 540).unwrap();
        preview_times.push(start.elapsed().as_secs_f64() * 1000.0);
        let start = Instant::now();
        let mut crop = Canvas::new(832, 632).unwrap();
        crop.translate(-984.0, -484.0);
        engine.render_into(&mut crop, 0.5, frames);
        patch_times.push(start.elapsed().as_secs_f64() * 1000.0);
        let start = Instant::now();
        let mut crop = Canvas::new(832, 632).unwrap();
        crop.translate(-984.0, -484.0);
        unredacted.render_into(&mut crop, 0.5, frames);
        unredacted_patch_times.push(start.elapsed().as_secs_f64() * 1000.0);
        let start = Instant::now();
        engine.render_frame(0.5, frames).unwrap();
        full_times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    for (name, values) in [
        ("preview", preview_times),
        ("patch", patch_times),
        ("unredacted patch", unredacted_patch_times),
        ("full", full_times),
    ] {
        let mut values = values[1..].to_vec();
        values.sort_by(f64::total_cmp);
        println!("{name}: median {:.2}ms, samples {values:?}", values[3]);
    }
}
