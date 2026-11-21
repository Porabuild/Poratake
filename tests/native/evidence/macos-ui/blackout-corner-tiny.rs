use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};
fn main() {
    for scale in [1.0_f32, 1.25, 2.0] {
        for offset in [0.0_f32, 0.25, 0.5] {
            let dim = (40.0 * scale).round() as u32;
            let x = (8.0 + offset) * scale;
            let y = x;
            let r = 2.0_f32;
            let w = 20.0 * scale;
            let mut path = PathBuilder::new();
            let control = r * (1.0 - 0.552_284_8);
            let right=x+w; let bottom=y+w;
            path.move_to(x+r,y); path.line_to(right-r,y); path.cubic_to(right-control,y,right,y+control,right,y+r);
            path.line_to(right,bottom-r); path.cubic_to(right,bottom-control,right-control,bottom,right-r,bottom);
            path.line_to(x+r,bottom); path.cubic_to(x+control,bottom,x,bottom-control,x,bottom-r);
            path.line_to(x,y+r); path.cubic_to(x,y+control,x+control,y,x+r,y); path.close();
            let mut pixmap=Pixmap::new(dim,dim).unwrap(); pixmap.fill(Color::from_rgba8(255,0,255,255));
            let mut paint=Paint::default(); paint.set_color(Color::BLACK); paint.anti_alias=true;
            pixmap.fill_path(&path.finish().unwrap(), &paint, FillRule::Winding, Transform::identity(), None);
            let mut red=Vec::new();
            for row in 0..4 { let mut values=Vec::new(); for col in 0..4 {
                let i=(((y.floor() as u32+row)*dim+x.floor() as u32+col)*4) as usize;
                values.push(pixmap.data()[i]);
            } red.push(values); }
            println!("scale={scale} offset={offset} red={red:?}");
        }
    }
}
