use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 7);
    let image = Pixmap::load_png(&args[1]).unwrap();
    let width: f32 = args[4].parse().unwrap();
    let height: f32 = args[5].parse().unwrap();
    let expansion: f32 = args[6].parse().unwrap();
    let expanded_width = width + expansion * 2.;
    let expanded_height = height + expansion * 2.;
    let aspect = image.width() as f32 / image.height() as f32;
    let (draw_width, draw_height) = if aspect > expanded_width / expanded_height {
        (expanded_height * aspect, expanded_height)
    } else {
        (expanded_width, expanded_width / aspect)
    };
    let x = (expanded_width - draw_width) / 2. - expansion;
    let y = (expanded_height - draw_height) / 2. - expansion;
    let transform = Transform::from_translate(x, y).pre_scale(
        draw_width / image.width() as f32,
        draw_height / image.height() as f32,
    );
    let quality = match args[3].as_str() {
        "bilinear" => FilterQuality::Bilinear,
        "bicubic" => FilterQuality::Bicubic,
        _ => panic!("expected bilinear or bicubic"),
    };
    let paint = PixmapPaint {
        quality,
        ..Default::default()
    };
    let mut result = Pixmap::new(width as u32, height as u32).unwrap();
    result.draw_pixmap(0, 0, image.as_ref(), &paint, transform, None);
    result.save_png(&args[2]).unwrap();
}
