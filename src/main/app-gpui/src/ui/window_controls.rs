use gpui::{
    canvas, div, point, prelude::*, px, AnyElement, App, Div, ElementId, Hsla, PathBuilder, Pixels,
    Point, Stateful, Styled, Window, WindowControlArea,
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

pub fn render(window: &mut Window, cx: &mut App, theme: &ThemeVars) -> AnyElement {
    if chrome::is_macos() {
        return div().into_any_element();
    }
    windows_captions(window, cx, theme)
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
        .into_any_element()
}

pub fn drag_area(id: impl Into<ElementId>) -> Stateful<Div> {
    let area = div().id(id).window_control_area(WindowControlArea::Drag);

    #[cfg(windows)]
    return area
        .mx(px(1.0))
        .on_mouse_down(gpui::MouseButton::Left, |event, window, cx| {
            if event.click_count != 1 {
                return;
            }
            start_window_drag(window);
            cx.stop_propagation();
        });

    #[cfg(not(windows))]
    area
}

#[cfg(windows)]
fn start_window_drag(window: &Window) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        PostMessageW, HTCAPTION, SC_MOVE, WM_SYSCOMMAND,
    };

    let Some(hwnd) = crate::windows::window_hwnd(window) else {
        return;
    };
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WM_SYSCOMMAND,
            WPARAM((SC_MOVE | HTCAPTION) as usize),
            LPARAM(0),
        );
    }
}

pub fn drag_strip(
    background: Hsla,
    window: &mut Window,
    cx: &mut App,
    theme: &ThemeVars,
) -> AnyElement {
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
        .child(drag_area("title-drag").flex_1().h_full())
        .child(render(window, cx, theme))
        .into_any_element()
}

fn windows_captions(window: &mut Window, cx: &mut App, theme: &ThemeVars) -> AnyElement {
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
            window,
            cx,
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
            window,
            cx,
        ))
        .child(caption(
            "win-close",
            WindowControlArea::Close,
            Glyph::Close,
            ink,
            Srgba::parse(CLOSE_HOVER).to_hsla(),
            Some(colors::white(1.0)),
            window,
            cx,
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
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    // A gated hover flag rather than a `.hover()` style: gpui paints that
    // against the window's last mouse position, which survives the pointer
    // leaving the window, so the caption would stay lit.
    let (hover, hovered) = crate::ui::primitives::hover_flag(id, window, cx);
    div()
        .id(id)
        .w(px(chrome::WINDOW_CONTROL_WIDTH))
        .h_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .flex_none()
        .occlude()
        .window_control_area(area)
        .when(hovered, |caption| {
            let caption = caption.bg(hover_bg);
            match hover_fg {
                Some(color) => caption.text_color(color),
                None => caption,
            }
        })
        .on_hover({
            let hover = hover.clone();
            move |over: &bool, _window, cx| {
                crate::ui::primitives::track_hover(&hover, *over, cx);
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
