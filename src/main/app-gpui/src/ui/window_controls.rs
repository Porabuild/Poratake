use gpui::{
    canvas, div, point, prelude::*, px, AnyElement, Hsla, PathBuilder, Pixels, Point, Styled,
    Window, WindowControlArea,
};

use crate::theme::color::Srgba;
use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::colors;

const CLOSE_HOVER: &str = "#c42b1c";
const GLYPH: f32 = 10.0;
const STROKE: f32 = 1.25;

#[derive(Clone, Copy)]
enum Glyph {
    Min,
    Max,
    Restore,
    Close,
}

pub fn render(window: &Window, theme: &ThemeVars) -> AnyElement {
    if chrome::is_macos() {
        return div().into_any_element();
    }
    windows_captions(window, theme)
}

pub fn leading_inset() -> AnyElement {
    if !chrome::is_macos() {
        return div().into_any_element();
    }
    div()
        .id("traffic-light-inset")
        .w(px(chrome::TRAFFIC_LIGHT_INSET))
        .h_full()
        .flex_none()
        .window_control_area(WindowControlArea::Drag)
        .into_any_element()
}

pub fn drag_strip(background: Hsla, window: &Window, theme: &ThemeVars) -> AnyElement {
    let mut strip = div()
        .flex()
        .flex_row()
        .h(px(chrome::TITLE_BAR_HEIGHT))
        .w_full()
        .flex_none()
        .bg(background);
    if chrome::is_macos() {
        strip = strip.child(leading_inset());
    }
    strip
        .child(
            div()
                .id("title-drag")
                .flex_1()
                .h_full()
                .window_control_area(WindowControlArea::Drag),
        )
        .child(render(window, theme))
        .into_any_element()
}

fn windows_captions(window: &Window, theme: &ThemeVars) -> AnyElement {
    let maximized = window.is_maximized();
    let hover = theme.row_hover;
    let ink = theme.foreground;
    div()
        .id("window-controls")
        .flex()
        .flex_row()
        .flex_none()
        .h_full()
        .child(caption(
            "win-min",
            WindowControlArea::Min,
            Glyph::Min,
            ink,
            hover,
            None,
        ))
        .child(caption(
            "win-max",
            WindowControlArea::Max,
            if maximized {
                Glyph::Restore
            } else {
                Glyph::Max
            },
            ink,
            hover,
            None,
        ))
        .child(caption(
            "win-close",
            WindowControlArea::Close,
            Glyph::Close,
            ink,
            Srgba::parse(CLOSE_HOVER).to_hsla(),
            Some(colors::white(1.0)),
        ))
        .into_any_element()
}

fn caption(
    id: &'static str,
    area: WindowControlArea,
    glyph: Glyph,
    ink: Hsla,
    hover_bg: Hsla,
    hover_fg: Option<Hsla>,
) -> AnyElement {
    div()
        .id(id)
        .w(px(chrome::WINDOW_CONTROL_WIDTH))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .window_control_area(area)
        .hover(move |style| {
            let style = style.bg(hover_bg);
            match hover_fg {
                Some(color) => style.text_color(color),
                None => style,
            }
        })
        .child(caption_glyph(glyph, ink))
        .into_any_element()
}

fn caption_glyph(glyph: Glyph, fallback: Hsla) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |bounds, _: (), window, _| {
            let color = window.text_style().color;
            let color = if color.a > 0.0 { color } else { fallback };
            let origin = bounds.center() - point(px(GLYPH / 2.0), px(GLYPH / 2.0));
            if let Some(path) = glyph_path(glyph, origin) {
                window.paint_path(path, color);
            }
        },
    )
    .w(px(GLYPH))
    .h(px(GLYPH))
    .into_any_element()
}

fn glyph_path(glyph: Glyph, origin: Point<Pixels>) -> Option<gpui::Path<Pixels>> {
    let mut builder = PathBuilder::stroke(px(STROKE));
    let p = |x: f32, y: f32| origin + point(px(x), px(y));
    match glyph {
        Glyph::Min => {
            builder.move_to(p(0.0, GLYPH / 2.0));
            builder.line_to(p(GLYPH, GLYPH / 2.0));
        }
        Glyph::Max => {
            builder.move_to(p(0.5, 0.5));
            builder.line_to(p(GLYPH - 0.5, 0.5));
            builder.line_to(p(GLYPH - 0.5, GLYPH - 0.5));
            builder.line_to(p(0.5, GLYPH - 0.5));
            builder.line_to(p(0.5, 0.5));
        }
        Glyph::Restore => {
            builder.move_to(p(2.5, 0.5));
            builder.line_to(p(GLYPH - 0.5, 0.5));
            builder.line_to(p(GLYPH - 0.5, GLYPH - 2.5));
            builder.move_to(p(0.5, 2.5));
            builder.line_to(p(GLYPH - 2.5, 2.5));
            builder.line_to(p(GLYPH - 2.5, GLYPH - 0.5));
            builder.line_to(p(0.5, GLYPH - 0.5));
            builder.line_to(p(0.5, 2.5));
        }
        Glyph::Close => {
            builder.move_to(p(0.5, 0.5));
            builder.line_to(p(GLYPH - 0.5, GLYPH - 0.5));
            builder.move_to(p(GLYPH - 0.5, 0.5));
            builder.line_to(p(0.5, GLYPH - 0.5));
        }
    }
    builder.build().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_cluster_matches_electron_overlay() {
        assert_eq!(chrome::WINDOW_CONTROL_WIDTH, 46.0);
        assert_eq!(
            chrome::WINDOW_CONTROLS_SPACER,
            chrome::WINDOW_CONTROL_WIDTH * 3.0
        );
        assert_eq!(chrome::TRAFFIC_LIGHT_INSET, 120.0);
        assert_eq!(chrome::is_macos(), cfg!(target_os = "macos"));
    }
}
