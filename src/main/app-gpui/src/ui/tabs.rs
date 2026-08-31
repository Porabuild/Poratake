//! Tabs — HeroUI `variant=secondary` as used by `TabSelector`: p-0, h-8,
//! rounded-none, px-4, text-sm, underline indicator.

use gpui::{div, prelude::*, px, AnimationExt, App, ElementId, SharedString, Styled, Window};
use std::rc::Rc;

use crate::theme::vars::active_theme;
use crate::ui::chrome;

#[derive(Clone, PartialEq)]
pub struct TabItem {
    pub id: SharedString,
    pub label: SharedString,
}

impl TabItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    items: Vec<TabItem>,
    selected: Option<SharedString>,
    on_select: Option<Rc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>>,
    full_width: bool,
}

impl Tabs {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            selected: None,
            on_select: None,
            full_width: false,
        }
    }

    pub fn items(mut self, items: Vec<TabItem>) -> Self {
        self.items = items;
        self
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }
}

/// The tab index the indicator was last drawn under, so a selection change
/// animates and the first paint does not slide in from tab zero.
struct IndicatorState {
    index: Option<usize>,
}

impl RenderOnce for Tabs {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let theme = active_theme(cx);
        let base_id = self.id.clone();
        let count = self.items.len();
        let selected_index = self
            .items
            .iter()
            .position(|item| self.selected.as_ref() == Some(&item.id));

        // The secondary variant draws a hairline under the whole list container
        // and hangs the indicator off its bottom edge.
        let mut list = div()
            .id(self.id)
            .flex()
            .flex_row()
            .gap(px(chrome::TABS_GAP))
            .rounded(px(chrome::TABS_RADIUS))
            .p(px(chrome::TABS_PAD))
            .border_b_1()
            .border_color(theme.border)
            .when(self.full_width, |el| el.w_full());

        for item in self.items.iter() {
            let is_selected = self.selected.as_ref() == Some(&item.id);
            let mut tab = div()
                .id(ElementId::NamedInteger(
                    format!("{base_id}-{}", item.id).into(),
                    0,
                ))
                .flex()
                .items_center()
                .justify_center()
                .h(px(chrome::TAB_MIN_HEIGHT))
                .px(px(chrome::TAB_PAD_X))
                .rounded(px(chrome::TAB_RADIUS))
                .text_size(px(chrome::TAB_TEXT))
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_default()
                .when(self.full_width, |el| el.flex_1());

            if is_selected {
                tab = tab.relative().text_color(theme.foreground);
            } else {
                // `.tabs__tab { transition: opacity 150ms var(--ease-smooth) }`
                // fading to `opacity-70` while hovered.
                let key = format!("{base_id}-tab-{}", item.id);
                let (hover, hovered, (from, to)) = crate::ui::primitives::hover_fade(
                    &key,
                    gpui::hsla(0.0, 0.0, 0.0, 1.0),
                    gpui::hsla(0.0, 0.0, 0.0, chrome::TAB_HOVER_OPACITY),
                    window,
                    cx,
                );
                let (from, to) = (from.a, to.a);
                let travel = to - from;
                tab = tab.text_color(theme.muted_foreground).on_hover({
                    let hover = hover.clone();
                    move |over: &bool, _window, cx| {
                        crate::ui::primitives::track_hover(&hover, *over, cx);
                    }
                });
                list = list.child(
                    tab.child(item.label.clone()).with_animation(
                        gpui::ElementId::Name(format!("{key}-fade-{hovered}").into()),
                        gpui::Animation::new(std::time::Duration::from_millis(
                            chrome::TAB_HOVER_MS,
                        ))
                        .with_easing(crate::ui::primitives::ease_smooth()),
                        move |tab, delta| tab.opacity(from + travel * delta),
                    ),
                );
                continue;
            }

            if let Some(handler) = self.on_select.as_ref() {
                let handler = handler.clone();
                let id = item.id.clone();
                tab = tab.on_click(move |_event, window, cx| handler(&id, window, cx));
            }

            list = list.child(tab.child(item.label.clone()));
        }

        let Some(selected_index) = selected_index.filter(|_| count > 0) else {
            return list;
        };

        // `.tabs__indicator` is positioned over the list and animates
        // `translate, width` for 250ms on `--ease-out-fluid`. The tabs share the
        // row equally (`w-full` on each), so the indicator can be placed in
        // fractions of the container instead of measuring each tab.
        let indicator = window.use_keyed_state(
            gpui::ElementId::Name(format!("{base_id}-indicator").into()),
            cx,
            |_, _| IndicatorState { index: None },
        );
        let previous = indicator.read(cx).index;
        if previous != Some(selected_index) {
            indicator.update(cx, |state, _| state.index = Some(selected_index));
        }
        let span = 1.0 / count as f32;
        let to = selected_index as f32 * span;
        let from = previous.map_or(to, |index| index as f32 * span);
        let travel = to - from;

        list.relative().child(
            div()
                .absolute()
                .bottom_0()
                .h(px(chrome::TAB_INDICATOR))
                .w(gpui::relative(span))
                .bg(theme.accent)
                .with_animation(
                    gpui::ElementId::Name(format!("{base_id}-indicator-{selected_index}").into()),
                    gpui::Animation::new(std::time::Duration::from_millis(
                        chrome::TAB_INDICATOR_MS,
                    ))
                    .with_easing(crate::ui::primitives::ease_out_fluid()),
                    move |bar, delta| bar.left(gpui::relative(from + travel * delta)),
                ),
        )
    }
}
