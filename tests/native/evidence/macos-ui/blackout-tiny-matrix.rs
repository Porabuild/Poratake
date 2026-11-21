use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

fn main() {
    for offset_x in [0.0_f32, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875] {
        for offset_y in [0.0_f32, 0.25, 0.5, 0.75] {
            let x = 8.0 + offset_x;
            let y = 8.0 + offset_y;
            let radius = 2.0_f32;
            let control = radius * (1.0 - 0.552_284_8);
            let (right, bottom) = (x + 20.0, y + 20.0);
            let mut path = PathBuilder::new();
            path.move_to(x + radius, y);
            path.line_to(right - radius, y);
            path.cubic_to(right - control, y, right, y + control, right, y + radius);
            path.line_to(right, bottom - radius);
            path.cubic_to(right, bottom - control, right - control, bottom, right - radius, bottom);
            path.line_to(x + radius, bottom);
            path.cubic_to(x + control, bottom, x, bottom - control, x, bottom - radius);
            path.line_to(x, y + radius);
            path.cubic_to(x, y + control, x + control, y, x + radius, y);
            path.close();
            let mut pixmap = Pixmap::new(40, 40).unwrap();
            pixmap.fill(Color::from_rgba8(255, 0, 255, 255));
            let mut paint = Paint::default();
            paint.set_color(Color::BLACK);
            paint.anti_alias = true;
            pixmap.fill_path(&path.finish().unwrap(), &paint, FillRule::Winding, Transform::identity(), None);
            let red: Vec<Vec<u8>> = (0..12).map(|row| (0..12).map(|column| {
                pixmap.data()[(((7 + row) * 40 + 7 + column) * 4) as usize]
            }).collect()).collect();
            println!("{offset_x} {offset_y} {red:?}");
        }
    }
}
