use std::sync::LazyLock;

use gpui::{
    canvas, div, point, prelude::*, px, size, AnyElement, App, Bounds, Div, ElementId, Hsla,
    Stateful, Styled, Window, WindowControlArea,
};
use herogpui::gpui;

use crate::theme::color::Srgba;
use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::colors;

static CLOSE_HOVER: LazyLock<Hsla> = LazyLock::new(|| Srgba::parse("#c42b1c").to_hsla());
const GLYPH: f32 = 10.0;

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

pub fn drag_area_leading_inset() -> f32 {
    if cfg!(windows) {
        1.0
    } else {
        0.0
    }
}

pub fn drag_area(id: impl Into<ElementId>) -> Stateful<Div> {
    let area = div()
        .id(id)
        .window_control_area(WindowControlArea::Drag)
        .mx(px(drag_area_leading_inset()));

    #[cfg(windows)]
    return area.on_mouse_down(gpui::MouseButton::Left, |event, window, cx| {
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
    paint_drag_strip(
        "title-drag",
        background,
        window,
        cx,
        theme,
        chrome::is_macos(),
    )
}

pub fn content_drag_strip(
    background: Hsla,
    window: &mut Window,
    cx: &mut App,
    theme: &ThemeVars,
) -> AnyElement {
    paint_drag_strip("content-title-drag", background, window, cx, theme, false)
}

fn paint_drag_strip(
    id: &'static str,
    background: Hsla,
    window: &mut Window,
    cx: &mut App,
    theme: &ThemeVars,
    reserve_leading: bool,
) -> AnyElement {
    let mut strip = div()
        .id(format!("{id}-strip"))
        .flex()
        .flex_row()
        .h(px(chrome::TITLE_BAR_HEIGHT))
        .w_full()
        .flex_none()
        .bg(background)
        .debug_selector(|| format!("{id}-strip"));
    if reserve_leading {
        strip = strip.child(leading_inset());
    }
    strip
        .child(drag_area(id).flex_1().h_full())
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
            *CLOSE_HOVER,
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
        move |bounds, _: (), window, cx| {
            let color = window.text_style().color;
            let color = if color.a > 0.0 { color } else { fallback };
            let origin = bounds.center() - point(px(GLYPH / 2.0), px(GLYPH / 2.0));
            let glyph_bounds = Bounds::new(origin, size(px(GLYPH), px(GLYPH)));
            let (key, svg) = glyph_svg(glyph);
            let _ = window.paint_svg(
                glyph_bounds,
                key.into(),
                Some(svg),
                gpui::TransformationMatrix::unit(),
                color,
                cx,
            );
        },
    )
    .w(px(GLYPH))
    .h(px(GLYPH))
    .into_any_element()
}

fn glyph_svg(glyph: Glyph) -> (&'static str, &'static [u8]) {
    const MIN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="white" stroke-width="1.25"><path d="M0 5 L10 5"/></svg>"#;
    const MAX: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="white" stroke-width="1.25"><path d="M0.5 0.5 L9.5 0.5 L9.5 9.5 L0.5 9.5 Z"/></svg>"#;
    const RESTORE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="white" stroke-width="1.25"><path d="M2.5 0.5 L9.5 0.5 L9.5 7.5 M0.5 2.5 L7.5 2.5 L7.5 9.5 L0.5 9.5 Z"/></svg>"#;
    const CLOSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="white" stroke-width="1.25"><path d="M0.5 0.5 L9.5 9.5 M9.5 0.5 L0.5 9.5"/></svg>"#;

    match glyph {
        Glyph::Min => ("poratake-caption:min", MIN),
        Glyph::Max => ("poratake-caption:max", MAX),
        Glyph::Restore => ("poratake-caption:restore", RESTORE),
        Glyph::Close => ("poratake-caption:close", CLOSE),
    }
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
