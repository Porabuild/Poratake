fn prepare_gif_diagnostic(
    recording: &Path,
    state: &VideoEditorState,
    fps: usize,
) -> Option<(PathBuf, PathBuf, VideoEditorState)> {
    std::env::var_os("PORATAKE_GIF_DIAGNOSTIC_DIR")
        .filter(|_| fps == 50)
        .map(|directory| {
            let directory = PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            let mp4 = directory.join("intermediate.mp4");
            let mut mp4_state = state.clone();
            mp4_state.export_settings.format = "mp4".into();
            run(
                Request {
                    project: recording.to_path_buf(),
                    output: mp4.clone(),
                    state: mp4_state,
                },
                &mut |_| {},
                &|| false,
            )
            .unwrap();
            (directory, mp4, state.clone())
        })
}

fn capture_gif_diagnostic(
    diagnostic: (&PathBuf, &PathBuf, &VideoEditorState),
    recording: &Path,
    time: f64,
    gif: &image::RgbaImage,
    expected: &[u8],
    translucent_pixel: usize,
) {
    let (directory, mp4, state) = diagnostic;
    let decoder = VideoDecoder::open(recording).unwrap();
    let video_segments = segments::to_video_segments(&state.segments);
    let mut config = Config::new(160., 90., state.clone());
    config.fps = 50.;
    let mut engine = Engine::new(config);
    let (width, height) = engine.dimensions();
    let dimensions = export_dimensions(width, height, "480p");
    let first_frame_duration = engine.first_frame_duration();
    let mut session = Session {
        engine: &mut engine,
        decoder: &decoder,
        camera: None,
        video_segments: &video_segments,
        dimensions,
        fps: 50,
        total_frames: 100,
        total_duration: 2.,
        source_duration: 2.,
    };
    let composed = compose_pixmap(&mut session, first_frame_duration, time).unwrap();
    let composition = to_opaque_rgba_buffer(&composed);
    let mp4_frame = VideoDecoder::open(mp4).unwrap().frame_at(time).unwrap();
    let mut mp4_rgba = mp4_frame.bgra.to_vec();
    for pixel in mp4_rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let mp4_image =
        image::RgbaImage::from_raw(mp4_frame.width, mp4_frame.height, mp4_rgba).unwrap();
    let expected_image = image::RgbaImage::from_raw(184, 114, expected.to_vec()).unwrap();
    expected_image.save(directory.join("preview.png")).unwrap();
    composition.save(directory.join("composition.png")).unwrap();
    mp4_image.save(directory.join("mp4.png")).unwrap();
    gif.save(directory.join("gif.png")).unwrap();
    for (name, image) in [
        ("composition", &composition),
        ("mp4", &mp4_image),
        ("gif", gif),
    ] {
        let scaled =
            image::imageops::resize(image, 184, 114, image::imageops::FilterType::Lanczos3);
        let bytes = scaled.as_raw();
        let sum: u64 = bytes
            .iter()
            .zip(expected)
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        let large = bytes
            .iter()
            .zip(expected)
            .filter(|(a, b)| a.abs_diff(**b) > 32)
            .count();
        eprintln!(
            "diagnostic {name}: mean {:.3}, channels above 32 {large}, pixel {:?}",
            sum as f64 / bytes.len() as f64,
            &bytes[translucent_pixel * 4..translucent_pixel * 4 + 3]
        );
        scaled
            .save(directory.join(format!("{name}-resized.png")))
            .unwrap();
    }
}
