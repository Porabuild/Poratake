use std::path::PathBuf;

use gpui::{
    div, img, prelude::*, px, AnyElement, Context, ElementId, ObjectFit, SharedString, Styled,
};

use crate::history_store::{HistoryItem, HistoryItemType};
use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::ui::colors::{black, transparent, white};
use crate::ui::icon::icon_element;
use crate::video::project::RecordingFeatures;
use crate::windows::history::model::format_relative_time;
use crate::windows::history::HistoryWindow;

const POPOVER_WIDTH: f32 = 400.0;
const CONTENT_PADDING: f32 = 12.0;
const GRID_GAP: f32 = 12.0;
/// `grid grid-cols-2 gap-3` inside `p-3`. The keyboard-selection ring is
/// `ring-2 ring-offset-1`, i.e. a box shadow, so it takes no layout room and
/// the columns are exactly half the padded width.
pub const GRID_CARD_WIDTH: f32 = (POPOVER_WIDTH - CONTENT_PADDING * 2.0 - GRID_GAP) / 2.0;
const GRID_THUMB_HEIGHT: f32 = GRID_CARD_WIDTH * 9.0 / 16.0;
const LIST_THUMB_WIDTH: f32 = 80.0;
const LIST_THUMB_HEIGHT: f32 = 48.0;
/// `h-4 w-4` in the grid card, `h-3 w-3` in the list row.
const GRID_SPINNER: f32 = 16.0;
const LIST_SPINNER: f32 = 12.0;

pub struct ItemView<'a> {
    pub item: &'a HistoryItem,
    pub index: usize,
    pub selected: bool,
    pub hovered: bool,
    /// `None` while the thumbnail is still being generated in the background,
    /// which is when the renderer shows its spinner.
    pub loading: bool,
    pub thumbnail: Option<PathBuf>,
    pub features: RecordingFeatures,
    pub now_ms: i64,
    pub theme: &'a ThemeVars,
}

impl ItemView<'_> {
    fn is_video(&self) -> bool {
        self.item.r#type == HistoryItemType::Video
    }

    fn element_id(&self, prefix: &str) -> ElementId {
        ElementId::Name(SharedString::from(format!("{prefix}-{}", self.item.id)))
    }

    fn feature_icons(&self) -> Vec<&'static str> {
        let mut icons = Vec::new();
        if self.features.has_mic {
            icons.push("mic");
        }
        if self.features.has_system_audio {
            icons.push("volume-2");
        }
        if self.features.has_camera {
            icons.push("video");
        }
        if self.features.has_cursor {
            icons.push("mouse-pointer-2");
        }
        icons
    }

    fn thumbnail_element(&self, width: f32, height: f32) -> AnyElement {
        if self.loading {
            let spinner = if width > LIST_THUMB_WIDTH {
                GRID_SPINNER
            } else {
                LIST_SPINNER
            };
            return div()
                .w(px(width))
                .h(px(height))
                .flex()
                .items_center()
                .justify_center()
                .bg(self.theme.muted_background)
                .text_color(self.theme.muted_foreground)
                .child(crate::ui::icon::spinner_element(
                    ElementId::Name(SharedString::from(format!(
                        "history-spinner-{}",
                        self.item.id
                    ))),
                    px(spinner),
                ))
                .into_any_element();
        }
        match &self.thumbnail {
            Some(path) => img(path.clone())
                .w(px(width))
                .h(px(height))
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => div()
                .w(px(width))
                .h(px(height))
                .flex()
                .items_center()
                .justify_center()
                .bg(self.theme.muted_background)
                .text_size(px(chrome::HISTORY_ITEM_TEXT))
                .text_color(self.theme.muted_foreground)
                .child("No preview")
                .into_any_element(),
        }
    }
}

fn selection_shadow(color: gpui::Hsla) -> Vec<gpui::BoxShadow> {
    let ring = |color: gpui::Hsla, spread: f32| gpui::BoxShadow {
        color,
        offset: gpui::point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(spread),
    };
    vec![
        ring(transparent(), RING_OFFSET),
        ring(color, RING_OFFSET + RING_WIDTH),
    ]
}

const RING_OFFSET: f32 = 1.0;
const RING_WIDTH: f32 = 2.0;

fn overlay_action(
    id: ElementId,
    icon: &'static str,
    tooltip: &'static str,
    danger: bool,
) -> Button {
    Button::new(id)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::IconXs)
        .height(px(chrome::HISTORY_ITEM_ACTION_SIZE))
        .icon(icon)
        .icon_size(px(chrome::HISTORY_ITEM_ACTION_ICON))
        .tooltip(tooltip)
        .radius(px(chrome::RADIUS_3XL))
        .surface(black(0.6))
        .surface_hover(if danger {
            // `hover:bg-red-500/80`, a fixed Tailwind red rather than the
            // theme's danger token.
            crate::ui::colors::red_500(0.8)
        } else {
            black(0.8)
        })
        .foreground(white(1.0))
}

pub fn grid_card(view: &ItemView, cx: &mut Context<HistoryWindow>) -> AnyElement {
    let theme = view.theme;
    let index = view.index;
    let id = view.item.id.clone();
    let show_actions = view.hovered || view.selected;

    let mut media = div()
        .relative()
        .w(px(GRID_CARD_WIDTH))
        .h(px(GRID_THUMB_HEIGHT))
        .overflow_hidden()
        .child(view.thumbnail_element(GRID_CARD_WIDTH, GRID_THUMB_HEIGHT));

    if view.is_video() {
        media = media.child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(40.0))
                        .rounded_full()
                        .bg(black(0.6))
                        .text_color(white(1.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon_element("play", px(20.0))),
                ),
        );

        let features = view.feature_icons();
        if !features.is_empty() {
            let mut badges = div()
                .absolute()
                .bottom(px(4.0))
                .left(px(4.0))
                .flex()
                .gap(px(2.0));
            for icon in features {
                badges = badges.child(
                    div()
                        .size(px(16.0))
                        // A bare `rounded`, which the radius scale leaves at 4px.
                        .rounded(px(4.0))
                        .bg(black(0.6))
                        .text_color(white(1.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon_element(icon, px(10.0))),
                );
            }
            media = media.child(badges);
        }
    }

    let card = div()
        .id(view.element_id("history-card"))
        .relative()
        .when(view.selected, |el| {
            el.shadow(selection_shadow(theme.primary))
        })
        .w(px(GRID_CARD_WIDTH))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(chrome::RADIUS_LG))
        .bg(if view.hovered {
            theme.muted_background
        } else {
            theme.secondary
        })
        .on_hover(cx.listener({
            let id = id.clone();
            move |this, hovered: &bool, _window, cx| this.set_hovered(&id, *hovered, cx)
        }))
        .on_click(cx.listener(move |this, _event, window, cx| this.open_index(index, window, cx)))
        .on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                this.open_item_menu(index, event.position, window, cx);
            }),
        )
        .child(media)
        .child(
            div()
                .px(px(8.0))
                .py(px(6.0))
                .text_size(px(chrome::HISTORY_ITEM_TEXT))
                .text_color(theme.muted_foreground)
                .child(format_relative_time(view.item.timestamp, view.now_ms)),
        )
        .when(show_actions, |el| {
            el.child(
                div()
                    .absolute()
                    .top(px(4.0))
                    .right(px(4.0))
                    .flex()
                    .gap(px(4.0))
                    .child(
                        overlay_action(
                            view.element_id("history-reveal"),
                            "folder-open",
                            "Show in folder",
                            false,
                        )
                        .on_click(cx.listener(
                            move |this, _event, _window, cx| {
                                this.reveal_index(index, cx);
                            },
                        )),
                    )
                    .child(
                        overlay_action(
                            view.element_id("history-delete"),
                            "trash-2",
                            "Delete",
                            true,
                        )
                        .on_click(cx.listener(
                            move |this, _event, _window, cx| {
                                this.delete_index(index, cx);
                            },
                        )),
                    ),
            )
        });

    card.into_any_element()
}

pub fn list_row(view: &ItemView, cx: &mut Context<HistoryWindow>) -> AnyElement {
    let theme = view.theme;
    let index = view.index;
    let id = view.item.id.clone();
    let show_actions = view.hovered || view.selected;
    let is_video = view.is_video();

    let mut thumb = div()
        .relative()
        .w(px(LIST_THUMB_WIDTH))
        .h(px(LIST_THUMB_HEIGHT))
        .flex_shrink_0()
        .overflow_hidden()
        .rounded(px(chrome::RADIUS_MD))
        .child(view.thumbnail_element(LIST_THUMB_WIDTH, LIST_THUMB_HEIGHT));

    if is_video {
        thumb = thumb.child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(24.0))
                        .rounded_full()
                        .bg(black(0.6))
                        .text_color(white(1.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon_element("play", px(12.0))),
                ),
        );
    }

    let mut meta = div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .text_size(px(chrome::HISTORY_ITEM_TEXT))
        .text_color(theme.muted_foreground)
        .child(format_relative_time(view.item.timestamp, view.now_ms));

    if is_video {
        let features = view.feature_icons();
        if !features.is_empty() {
            let mut icons = div().flex().gap(px(2.0));
            for icon in features {
                icons = icons.child(icon_element(icon, px(10.0)));
            }
            meta = meta.child(icons);
        }
    }

    let mut row = div()
        .id(view.element_id("history-row"))
        .when(view.selected, |el| {
            el.shadow(selection_shadow(theme.primary))
        })
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .p(px(8.0))
        .rounded(px(chrome::RADIUS_LG))
        .bg(if view.hovered {
            theme.muted_background
        } else {
            theme.secondary
        })
        .on_hover(cx.listener({
            let id = id.clone();
            move |this, hovered: &bool, _window, cx| this.set_hovered(&id, *hovered, cx)
        }))
        .on_click(cx.listener(move |this, _event, window, cx| this.open_index(index, window, cx)))
        .on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                this.open_item_menu(index, event.position, window, cx);
            }),
        )
        .child(thumb)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(2.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_color(theme.muted_foreground)
                                .child(icon_element(
                                    if is_video { "video" } else { "camera" },
                                    px(12.0),
                                )),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(chrome::HISTORY_ITEM_TEXT))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.foreground)
                                .child(if is_video { "Video" } else { "Screenshot" }),
                        ),
                )
                .child(meta),
        );

    if show_actions {
        row = row
            .child(
                Button::new(view.element_id("history-row-reveal"))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::IconXs)
                    .height(px(chrome::HISTORY_ITEM_ACTION_SIZE))
                    .icon("folder-open")
                    .icon_size(px(chrome::HISTORY_ITEM_ACTION_ICON))
                    .radius(px(chrome::RADIUS_3XL))
                    .foreground(theme.muted_foreground)
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.reveal_index(index, cx);
                    })),
            )
            .child(
                Button::new(view.element_id("history-row-delete"))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::IconXs)
                    .height(px(chrome::HISTORY_ITEM_ACTION_SIZE))
                    .icon("trash-2")
                    .icon_size(px(chrome::HISTORY_ITEM_ACTION_ICON))
                    .radius(px(chrome::RADIUS_3XL))
                    // `text-red-400 hover:bg-red-500/20`.
                    .foreground(crate::ui::colors::red_400(1.0))
                    .surface_hover(crate::ui::colors::red_500(0.2))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.delete_index(index, cx);
                    })),
            );
    }

    row.into_any_element()
}
