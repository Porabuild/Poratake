use gpui::{hsla, px, ElementId, Hsla, ScrollHandle};
use herogpui::gpui;
use herogpui::{Orientation, Scrollbar};

const APP_THUMB_LIGHTNESS: f32 = 100.0 / 255.0;
const APP_THUMB_ALPHA: f32 = 0.5;
const APP_THUMB_HOVER_ALPHA: f32 = 0.7;
const OVERLAY_THUMB_ALPHA: f32 = 0.3;
const OVERLAY_THUMB_HOVER_ALPHA: f32 = 0.5;

fn with_alpha(color: Hsla, alpha: f32) -> Hsla {
    Hsla { a: alpha, ..color }
}

fn pinned(id: impl Into<ElementId>, handle: &ScrollHandle) -> Scrollbar {
    Scrollbar::new(id, handle.clone()).auto_hide(false)
}

pub fn app_vertical(id: impl Into<ElementId>, handle: &ScrollHandle) -> Scrollbar {
    pinned(id, handle)
        .track(px(8.0))
        .inset(px(2.0))
        .thumb_color(hsla(0.0, 0.0, APP_THUMB_LIGHTNESS, APP_THUMB_ALPHA))
        .thumb_hover_color(hsla(0.0, 0.0, APP_THUMB_LIGHTNESS, APP_THUMB_HOVER_ALPHA))
}

pub fn overlay_vertical(id: impl Into<ElementId>, handle: &ScrollHandle, muted: Hsla) -> Scrollbar {
    pinned(id, handle)
        .track(px(8.0))
        .inset(px(2.0))
        .thumb_color(with_alpha(muted, OVERLAY_THUMB_ALPHA))
        .thumb_hover_color(with_alpha(muted, OVERLAY_THUMB_HOVER_ALPHA))
}

pub fn overlay_horizontal(
    id: impl Into<ElementId>,
    handle: &ScrollHandle,
    muted: Hsla,
) -> Scrollbar {
    pinned(id, handle)
        .orientation(Orientation::Horizontal)
        .track(px(12.0))
        .inset(px(3.0))
        .thumb_color(with_alpha(muted, OVERLAY_THUMB_ALPHA))
        .thumb_hover_color(with_alpha(muted, OVERLAY_THUMB_HOVER_ALPHA))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overlay_thumb_keeps_the_muted_colour_and_only_moves_its_alpha() {
        let muted = hsla(0.25, 0.4, 0.6, 1.0);
        let resting = with_alpha(muted, OVERLAY_THUMB_ALPHA);
        let hovered = with_alpha(muted, OVERLAY_THUMB_HOVER_ALPHA);

        assert_eq!(resting.h, muted.h);
        assert_eq!(resting.s, muted.s);
        assert_eq!(resting.l, muted.l);
        assert_eq!(resting.a, 0.3);
        assert_eq!(hovered.a, 0.5);
    }

    #[test]
    fn the_app_thumb_is_the_stylesheets_grey() {
        let resting = hsla(0.0, 0.0, APP_THUMB_LIGHTNESS, APP_THUMB_ALPHA);
        assert_eq!(resting.s, 0.0);
        assert_eq!((resting.l * 255.0).round(), 100.0);
        assert_eq!(resting.a, 0.5);
        assert_eq!(APP_THUMB_HOVER_ALPHA, 0.7);
    }
}
