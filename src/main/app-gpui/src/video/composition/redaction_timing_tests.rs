#[test]
#[ignore]
fn video_redaction_quality_timing() {
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
    let mut redacted = Engine::new(config);
    let mut plain = Engine::new(Config::new(3840.0, 2160.0, state()));
    for (width, height) in [
        (384, 216),
        (432, 243),
        (480, 270),
        (640, 360),
        (854, 480),
        (960, 540),
        (1280, 720),
    ] {
        let mut plain_times = Vec::new();
        let mut redacted_times = Vec::new();
        for _ in 0..8 {
            let start = Instant::now();
            plain
                .render_frame_scaled(0.5, frames, width, height)
                .unwrap();
            plain_times.push(start.elapsed().as_secs_f64() * 1000.0);
            let start = Instant::now();
            redacted
                .render_frame_scaled(0.5, frames, width, height)
                .unwrap();
            redacted_times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        plain_times = plain_times[1..].to_vec();
        redacted_times = redacted_times[1..].to_vec();
        plain_times.sort_by(f64::total_cmp);
        redacted_times.sort_by(f64::total_cmp);
        println!(
            "{width}x{height}: plain {:.2} ms, redacted {:.2} ms",
            plain_times[3], redacted_times[3]
        );
    }
}

#[test]
#[ignore]
fn video_redaction_region_copy_timing() {
    use std::time::Instant;

    fn previous(canvas: &Canvas, x: i32, y: i32, width: u32, height: u32) -> Pixmap {
        let mut region = Pixmap::new(width, height).unwrap();
        let source_width = canvas.width() as i64;
        let source_height = canvas.height() as i64;
        let source = canvas.pixmap().data();
        let target = region.data_mut();
        for row in 0..height as i64 {
            let source_y = y as i64 + row;
            if source_y < 0 || source_y >= source_height {
                continue;
            }
            for column in 0..width as i64 {
                let source_x = x as i64 + column;
                if source_x < 0 || source_x >= source_width {
                    continue;
                }
                let from = ((source_y * source_width + source_x) * 4) as usize;
                let to = ((row * width as i64 + column) * 4) as usize;
                target[to..to + 4].copy_from_slice(&source[from..from + 4]);
            }
        }
        region
    }

    let mut canvas = Canvas::new(3840, 2160).unwrap();
    canvas.fill_all(Color::from_rgba8(32, 64, 128, 255));
    let expected = previous(&canvas, 1000, 500, 800, 600);
    let actual = canvas.read_region(1000, 500, 800, 600).unwrap();
    assert_eq!(actual.data(), expected.data());
    let mut previous_times = Vec::new();
    let mut current_times = Vec::new();
    for iteration in 0_u32..16 {
        let previous_run = || {
            let start = Instant::now();
            std::hint::black_box(previous(&canvas, 1000, 500, 800, 600));
            start.elapsed().as_secs_f64() * 1000.0
        };
        let current_run = || {
            let start = Instant::now();
            std::hint::black_box(canvas.read_region(1000, 500, 800, 600).unwrap());
            start.elapsed().as_secs_f64() * 1000.0
        };
        if iteration.is_multiple_of(2) {
            previous_times.push(previous_run());
            current_times.push(current_run());
        } else {
            current_times.push(current_run());
            previous_times.push(previous_run());
        }
    }
    previous_times = previous_times[2..].to_vec();
    current_times = current_times[2..].to_vec();
    previous_times.sort_by(f64::total_cmp);
    current_times.sort_by(f64::total_cmp);
    println!(
        "region copy: previous {:.3} ms, current {:.3} ms",
        previous_times[7], current_times[7]
    );
}
