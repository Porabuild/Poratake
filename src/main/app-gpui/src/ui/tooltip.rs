use gpui::{div, prelude::*, px, Context, Render, SharedString, Styled, Window};

use crate::theme::vars::active_theme;
use crate::ui::chrome;
use crate::ui::primitives;

pub struct Tooltip {
    text: SharedString,
    /// When the tooltip appeared, so `zoom-in-90` can be applied as a
    /// proportional re-layout over the entrance.
    opened_at: std::time::Instant,
}

impl Tooltip {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            opened_at: std::time::Instant::now(),
        }
    }
}

impl Render for Tooltip {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        // `zoom-in-90` cannot be a transform, so the box geometry scales
        // instead. The text size is held fixed: scaling it would re-shape the
        // glyphs every frame, a shimmer the real transform does not produce.
        let scale = primitives::enter_scale(self.opened_at, primitives::OVERLAY_ENTER_ZOOM_90);
        if primitives::entering(self.opened_at) {
            primitives::request_animation_frame(window);
        }

        // `.tooltip { max-w-xs bg-overlay p-2 text-xs }` with
        // `border-radius: min(32px, var(--radius-xl))` and no border — the dark
        // `--overlay-shadow` is an inset hairline, not an outline.
        primitives::overlay_enter(
            "tooltip-enter",
            primitives::EnterFrom::Bottom,
            div()
                .rounded(px(chrome::TOOLTIP_RADIUS * scale))
                .bg(theme.overlay)
                .text_color(theme.foreground)
                .shadow_md()
                .max_w(px(chrome::TOOLTIP_MAX_WIDTH))
                .p(px(chrome::TOOLTIP_PAD * scale))
                .text_size(px(chrome::TOOLTIP_TEXT))
                .child(self.text.clone()),
        )
    }
}
