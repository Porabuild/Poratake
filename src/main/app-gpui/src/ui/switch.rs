//! Switch — 1:1 port of HeroUI `switch.css` with the app's pill override
//! (`base.css` rounds control + thumb to 9999px).

use gpui::{
    div, prelude::*, px, AnimationExt, Div, ElementId, InteractiveElement, Stateful,
    StatefulInteractiveElement, Styled,
};

use crate::theme::vars::{active_theme, ThemeVars};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[allow(dead_code)]
pub enum SwitchSize {
    #[default]
    Md,
    Sm,
    Lg,
}

impl SwitchSize {
    pub fn track(self) -> (gpui::Pixels, gpui::Pixels) {
        match self {
            Self::Sm => (
                px(crate::ui::chrome::SWITCH_SM_TRACK.0),
                px(crate::ui::chrome::SWITCH_SM_TRACK.1),
            ),
            Self::Md => (
                px(crate::ui::chrome::SWITCH_MD_TRACK.0),
                px(crate::ui::chrome::SWITCH_MD_TRACK.1),
            ),
            Self::Lg => (
                px(crate::ui::chrome::SWITCH_LG_TRACK.0),
                px(crate::ui::chrome::SWITCH_LG_TRACK.1),
            ),
        }
    }

    pub fn thumb(self) -> (gpui::Pixels, gpui::Pixels) {
        match self {
            Self::Sm => (
                px(crate::ui::chrome::SWITCH_SM_THUMB.0),
                px(crate::ui::chrome::SWITCH_SM_THUMB.1),
            ),
            Self::Md => (
                px(crate::ui::chrome::SWITCH_MD_THUMB.0),
                px(crate::ui::chrome::SWITCH_MD_THUMB.1),
            ),
            Self::Lg => (
                px(crate::ui::chrome::SWITCH_LG_THUMB.0),
                px(crate::ui::chrome::SWITCH_LG_THUMB.1),
            ),
        }
    }

    fn margin(self) -> gpui::Pixels {
        px(crate::ui::chrome::SWITCH_MARGIN)
    }

    /// `switch.css` moves the thumb to `ms-[calc(100% - 1.5rem)]` when checked,
    /// where `1.5rem` is the thumb width plus one margin — so the trailing gap
    /// equals the leading `ms-0.5`, not twice it.
    pub fn checked_offset(self) -> gpui::Pixels {
        let (track_w, _) = self.track();
        let (thumb_w, _) = self.thumb();
        track_w - thumb_w - self.margin()
    }
}

#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    disabled: bool,
    size: SwitchSize,
    on_change: Option<Box<dyn Fn(&bool, &mut gpui::Window, &mut gpui::App) + 'static>>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            disabled: false,
            size: SwitchSize::default(),
            on_change: None,
        }
    }

    pub fn size(mut self, size: SwitchSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&bool, &mut gpui::Window, &mut gpui::App) + 'static,
    ) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let theme = active_theme(cx);
        render_track(self, &theme, window, cx)
    }
}

/// Whether this switch has been rendered before. `switch.css` animates the
/// thumb only when it moves, so the first paint has to land on its resting
/// position rather than sliding in from the other side.
struct Mounted {
    seen: bool,
}

fn render_track(
    switch: Switch,
    theme: &ThemeVars,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> Stateful<Div> {
    let (track_w, track_h) = switch.size.track();
    let (thumb_w, thumb_h) = switch.size.thumb();
    let margin = switch.size.margin();

    let bg = if switch.checked {
        if switch.disabled {
            theme.accent.opacity(0.5)
        } else {
            theme.accent
        }
    } else if switch.disabled {
        theme.default.opacity(0.5)
    } else {
        theme.default
    };
    let hover_bg = if switch.checked {
        theme.accent_hover
    } else {
        // color-mix(in oklab, var(--switch-control-bg), transparent 20%)
        let mut mixed = theme.default;
        mixed.a *= 0.8;
        mixed
    };

    let thumb_bg = if switch.disabled {
        if switch.checked {
            theme.accent_foreground.opacity(0.4)
        } else {
            theme.default_foreground.opacity(0.2)
        }
    } else if switch.checked {
        theme.accent_foreground
    } else {
        crate::theme::color::Srgba::parse("#ffffff").to_hsla()
    };
    // `switch.id` is moved into the track below, so keep a copy for the
    // per-element animation keys.
    let element_key = format!("{}", switch.id);
    let focus = crate::ui::primitives::control_focus(&element_key, switch.disabled, window, cx);
    let resting = |checked: bool| {
        if checked {
            switch.size.checked_offset()
        } else {
            margin
        }
    };
    let thumb_x = resting(switch.checked);
    // `transition: margin 300ms var(--ease-out-fluid)`. The travel is driven
    // from per-element state so a toggle animates but the first paint does not.
    let mounted = window.use_keyed_state(
        gpui::ElementId::Name(format!("{element_key}-mounted").into()),
        cx,
        |_, _| Mounted { seen: false },
    );
    let first_paint = !mounted.read(cx).seen;
    if first_paint {
        mounted.update(cx, |state, _| state.seen = true);
    }
    let thumb_from = if first_paint {
        thumb_x
    } else {
        resting(!switch.checked)
    };

    // A gated hover flag rather than a `.hover()` style: gpui paints that
    // against the window's last mouse position, which survives the pointer
    // leaving the window, so the track would stay lit.
    let hover =
        (!switch.disabled).then(|| crate::ui::primitives::hover_flag(&element_key, window, cx));
    let hovering = hover.as_ref().is_some_and(|(_, over)| *over);

    let mut track: Stateful<Div> = div()
        .id(switch.id)
        .track_focus(&focus)
        .focus(|style| style.shadow(crate::ui::primitives::focus_ring(theme, 2.0)))
        .flex()
        .items_center()
        .w(track_w)
        .h(track_h)
        .rounded(px(crate::ui::chrome::SWITCH_RADIUS))
        .bg(if hovering { hover_bg } else { bg });

    if let Some((hover, _)) = hover {
        track = track.on_hover(move |over: &bool, _window, cx| {
            crate::ui::primitives::track_hover(&hover, *over, cx);
        });
        if let Some(handler) = switch.on_change {
            let next = !switch.checked;
            track = track.on_click(move |_event, window, cx| handler(&next, window, cx));
        }
    }

    let travel = f32::from(thumb_x) - f32::from(thumb_from);
    track.child(
        div()
            .rounded(px(crate::ui::chrome::SWITCH_RADIUS))
            .w(thumb_w)
            .h(thumb_h)
            .bg(thumb_bg)
            .with_animation(
                gpui::ElementId::Name(format!("{element_key}-thumb-{}", switch.checked).into()),
                gpui::Animation::new(std::time::Duration::from_millis(
                    crate::ui::chrome::SWITCH_TRAVEL_MS,
                ))
                .with_easing(crate::ui::primitives::ease_out_fluid()),
                move |thumb, delta| thumb.ml(thumb_from + px(travel * delta)),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::chrome;

    #[test]
    fn checked_offset_matches_electron_thumb_travel() {
        let md = SwitchSize::Md.checked_offset();
        assert_eq!(
            f32::from(md),
            chrome::SWITCH_MD_TRACK.0 - chrome::SWITCH_MD_THUMB.0 - chrome::SWITCH_MARGIN
        );
        // `ms-[calc(100% - 1.15625rem)]`, `calc(100% - 1.84375rem)` and
        // `calc(100% - 1.5rem)` respectively, at a 16px root font.
        assert_eq!(f32::from(SwitchSize::Sm.checked_offset()), 32.0 - 18.5);
        assert_eq!(f32::from(SwitchSize::Lg.checked_offset()), 48.0 - 29.5);
        assert_eq!(f32::from(SwitchSize::Md.checked_offset()), 40.0 - 24.0);
        // The trailing gap matches the leading `ms-0.5` at every size.
        for size in [SwitchSize::Sm, SwitchSize::Md, SwitchSize::Lg] {
            let (track_w, _) = size.track();
            let (thumb_w, _) = size.thumb();
            assert_eq!(
                f32::from(track_w - (size.checked_offset() + thumb_w)),
                chrome::SWITCH_MARGIN,
                "trailing gap for {size:?}"
            );
        }
        let unchecked = chrome::SWITCH_MARGIN;
        assert_ne!(f32::from(SwitchSize::Md.checked_offset()), unchecked);
        assert_eq!(chrome::SWITCH_TRAVEL_MS, 200);
    }
}
