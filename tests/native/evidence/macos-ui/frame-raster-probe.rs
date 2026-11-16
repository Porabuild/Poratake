use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
fn main() {
    for scale in [1.0_f32, 1.25, 1.3333333, 2.] {
        for offset in [0.0_f32, 0.5] {
            for mode in ["stroke", "fill", "circle"] {
                let mut pixmap = Pixmap::new(240, 240).unwrap();
                pixmap.fill(Color::from_rgba8(232, 232, 232, 255));
                let mut builder = PathBuilder::new();
                let x = 20. + offset;
                let y = x;
                let r = 10. * scale;
                let right = x + 140.;
                let bottom = y + 148.;
                let k = r * (1. - 0.552_284_8);
                if mode == "circle" {
                    builder.push_circle(x + 13. * scale, y + 14. * scale, 6. * scale);
                } else {
                    builder.move_to(x + r, y);
                    builder.line_to(right - r, y);
                    builder.cubic_to(right - k, y, right, y + k, right, y + r);
                    builder.line_to(right, bottom - r);
                    builder.cubic_to(right, bottom - k, right - k, bottom, right - r, bottom);
                    builder.line_to(x + r, bottom);
                    builder.cubic_to(x + k, bottom, x, bottom - k, x, bottom - r);
                    builder.line_to(x, y + r);
                    builder.cubic_to(x, y + k, x + k, y, x + r, y);
                    builder.close();
                }
                let path = builder.finish().unwrap();
                let mut paint = Paint::default();
                paint.anti_alias = true;
                if mode == "stroke" {
                    paint.set_color(Color::from_rgba8(168, 168, 168, 255));
                    pixmap.stroke_path(
                        &path,
                        &paint,
                        &Stroke {
                            width: scale,
                            ..Stroke::default()
                        },
                        Transform::identity(),
                        None,
                    );
                } else {
                    paint.set_color(if mode == "circle" {
                        Color::from_rgba8(255, 95, 87, 255)
                    } else {
                        Color::from_rgba8(36, 104, 172, 255)
                    });
                    pixmap.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
                pixmap
                    .save_png(format!(
                        "/tmp/poratake-frame-isolate-{scale}-{offset}-{mode}.png"
                    ))
                    .unwrap();
            }
        }
    }
}
