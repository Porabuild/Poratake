pub mod item;
pub mod model;
pub mod toolbar;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, FocusHandle, KeyDownEvent, Pixels, Render,
    ScrollHandle, Styled, Window,
};
use herogpui::gpui;

use crate::config::store::ConfigStore;
use crate::history_store::{self, HistoryItem, HistoryItemType};
use crate::system::desktop;
use crate::system::native::TrayRect;
use crate::theme::vars::active_theme;
use crate::video::project::{recording_features, RecordingFeatures};
use crate::windows::history::model::{
    visible_items, HistoryFilter, HistoryLayout, HistorySortOrder,
};
use crate::windows::registry::{self, WindowKind as RegistryKind};

pub const POPOVER_WIDTH: f32 = crate::ui::chrome::HISTORY_POPOVER_WIDTH;
pub const POPOVER_HEIGHT: f32 = crate::ui::chrome::HISTORY_POPOVER_HEIGHT;

pub fn should_close_on_blur(window_active: bool, menu_open: bool) -> bool {
    !window_active && !menu_open
}

#[derive(Clone, Default)]
struct ItemMedia {
    thumbnail: Option<PathBuf>,
    features: RecordingFeatures,
}

pub struct HistoryWindow {
    items: Vec<HistoryItem>,
    media: HashMap<String, ItemMedia>,
    filter: HistoryFilter,
    sort_order: HistorySortOrder,
    layout: HistoryLayout,
    selected_index: usize,
    keyboard_navigation: bool,
    hovered_id: Option<String>,
    item_menu_open: bool,
    scroll: ScrollHandle,
    store: Arc<ConfigStore>,
    focus_handle: FocusHandle,
    now_ms: i64,
    activation: Option<gpui::Subscription>,
    revealing: bool,
    closing: bool,
}

impl HistoryWindow {
    pub fn new(store: Arc<ConfigStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let preferences = store.get().history;
        let mut view = Self {
            items: Vec::new(),
            media: HashMap::new(),
            filter: HistoryFilter::parse(&preferences.filter),
            sort_order: HistorySortOrder::parse(&preferences.sort_order),
            layout: HistoryLayout::parse(&preferences.layout),
            selected_index: 0,
            keyboard_navigation: false,
            hovered_id: None,
            item_menu_open: false,
            scroll: ScrollHandle::new(),
            store,
            focus_handle: cx.focus_handle(),
            now_ms: chrono::Local::now().timestamp_millis(),
            activation: None,
            revealing: cfg!(windows),
            closing: false,
        };
        view.activation = Some(cx.observe_window_activation(window, |this, window, cx| {
            if this.revealing
                || this.closing
                || !should_close_on_blur(window.is_window_active(), this.item_menu_open)
            {
                return;
            }
            this.close(window, cx);
        }));
        view.reload(cx);
        view
    }

    pub fn toggle(tray_rect: Option<TrayRect>, cx: &mut App) {
        registry::toggle(RegistryKind::History, cx, |cx| {
            let store = crate::state::state(cx).config;
            let (bounds, display_id) = popover_placement(tray_rect, cx);
            let window_bounds = display_id
                .and_then(|id| cx.find_display(id))
                .map(|display| {
                    crate::system::work_area::local_window_bounds(bounds, display.as_ref())
                })
                .unwrap_or(bounds);
            cx.open_window(
                super::popup_window_options(
                    window_bounds,
                    super::PopupWindowConfig {
                        show: !cfg!(windows),
                        display_id,
                        background: if cfg!(target_os = "macos") {
                            crate::system::reduced_transparency::window_background(
                                gpui::WindowBackgroundAppearance::Blurred,
                            )
                        } else {
                            gpui::WindowBackgroundAppearance::Opaque
                        },
                        ..Default::default()
                    },
                ),
                |window, cx| {
                    #[cfg(windows)]
                    if let Some(hwnd) = crate::windows::window_hwnd(window) {
                        crate::system::window_composition::disable_transitions(hwnd);
                        crate::system::window_composition::stage_window(
                            hwnd,
                            bounds,
                            window.scale_factor(),
                            true,
                        );
                    }
                    let view = cx.new(|cx| Self::new(store, window, cx));
                    let focus = view.read(cx).focus_handle.clone();
                    window.focus(&focus, cx);
                    #[cfg(windows)]
                    {
                        let settled = view.clone();
                        window.on_next_frame(move |window, _cx| {
                            window.on_next_frame(move |window, _| {
                                if let Some(hwnd) = crate::windows::window_hwnd(window) {
                                    crate::system::window_composition::reveal_window(hwnd, true, 0);
                                }
                                window.activate_window();
                                window.on_next_frame(move |window, cx| {
                                    settled.update(cx, |this, _cx| this.revealing = false);
                                    if !window.is_window_active() {
                                        settled.update(cx, |this, cx| this.close(window, cx));
                                    }
                                });
                            });
                        });
                        window.activate_window();
                    }
                    view
                },
            )
            .ok()
            .map(Into::into)
        });
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.items = history_store::load_history();
        self.now_ms = chrono::Local::now().timestamp_millis();
        self.selected_index = 0;
        self.load_media(cx);
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.closing = true;
        registry::close(RegistryKind::History, cx);
        window.remove_window();
    }

    fn load_media(&mut self, cx: &mut Context<Self>) {
        let pending: Vec<(String, PathBuf, HistoryItemType)> = self
            .items
            .iter()
            .filter(|item| !self.media.contains_key(&item.id))
            .map(|item| {
                (
                    item.id.clone(),
                    PathBuf::from(&item.original_path),
                    item.r#type,
                )
            })
            .collect();
        if pending.is_empty() {
            return;
        }

        let task = cx.background_executor().spawn(async move {
            pending
                .into_iter()
                .map(|(id, path, kind)| {
                    let media = ItemMedia {
                        thumbnail: crate::thumbnails::ensure(&path, kind),
                        features: if kind == HistoryItemType::Video {
                            recording_features(&path)
                        } else {
                            RecordingFeatures::default()
                        },
                    };
                    (id, media)
                })
                .collect::<Vec<_>>()
        });

        cx.spawn(async move |entity, cx| {
            let resolved = task.await;
            let _ = entity.update(cx, |this, cx| {
                this.media.extend(resolved);
                cx.notify();
            });
        })
        .detach();
    }

    fn visible(&self) -> Vec<&HistoryItem> {
        visible_items(&self.items, self.filter, self.sort_order)
    }

    fn persist_preferences(&self) {
        let filter = self.filter.as_str().to_string();
        let sort_order = self.sort_order.as_str().to_string();
        let layout = self.layout.as_str().to_string();
        self.store.update(move |config| {
            config.history.filter = filter;
            config.history.sort_order = sort_order;
            config.history.layout = layout;
        });
    }

    pub fn set_filter(&mut self, filter: HistoryFilter, cx: &mut Context<Self>) {
        self.filter = filter;
        self.selected_index = 0;
        self.scroll.scroll_to_item(0);
        self.persist_preferences();
        cx.notify();
    }

    pub fn toggle_sort_order(&mut self, cx: &mut Context<Self>) {
        self.sort_order = self.sort_order.toggled();
        self.selected_index = 0;
        self.scroll.scroll_to_item(0);
        self.persist_preferences();
        cx.notify();
    }

    /// Steps to the next filter, so the headless render test can cover each
    /// visible set (including the filtered empty state).
    #[cfg(test)]
    pub fn cycle_filter_for_test(&mut self, cx: &mut Context<Self>) {
        let next = match self.filter {
            HistoryFilter::All => HistoryFilter::Screenshot,
            HistoryFilter::Screenshot => HistoryFilter::Video,
            HistoryFilter::Video => HistoryFilter::All,
        };
        self.set_filter(next, cx);
    }

    pub fn toggle_layout(&mut self, cx: &mut Context<Self>) {
        self.layout = self.layout.toggled();
        self.persist_preferences();
        cx.notify();
    }

    pub fn set_hovered(&mut self, id: &str, hovered: bool, cx: &mut Context<Self>) {
        let next = if hovered {
            Some(id.to_string())
        } else if self.hovered_id.as_deref() == Some(id) {
            None
        } else {
            return;
        };
        if self.hovered_id == next {
            return;
        }
        self.hovered_id = next;
        cx.notify();
    }

    pub fn open_index(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.visible().get(index).map(|item| (*item).clone()) else {
            return;
        };
        let path = item.original_path.clone();
        let kind = item.r#type;
        if kind == HistoryItemType::Screenshot {
            if crate::open_editor_for(cx, &path) {
                self.close(window, cx);
            }
            return;
        }
        self.close(window, cx);
        cx.defer(move |cx| match kind {
            HistoryItemType::Video => {
                crate::windows::video_editor::VideoEditorWindow::open(cx, Some(path))
            }
            HistoryItemType::Screenshot => {}
        });
    }

    pub fn reveal_index(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(item) = self.visible().get(index).map(|item| (*item).clone()) else {
            return;
        };
        desktop::reveal_in_file_manager(std::path::Path::new(&item.original_path));
        cx.notify();
    }

    pub fn delete_index(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(item) = self.visible().get(index).map(|item| (*item).clone()) else {
            return;
        };
        if !history_store::delete_item(&item.id) {
            crate::windows::toast::Toast::show(
                cx,
                "Delete failed",
                "The capture could not be deleted",
            );
            return;
        }
        self.media.remove(&item.id);
        self.items.retain(|candidate| candidate.id != item.id);

        let remaining = self.visible().len();
        self.selected_index = match remaining {
            0 => 0,
            count if self.selected_index >= count => count - 1,
            _ => self.selected_index,
        };
        cx.notify();
    }

    pub fn clear_all(&mut self, cx: &mut Context<Self>) {
        if !history_store::clear_history() {
            self.reload(cx);
            crate::windows::toast::Toast::show(
                cx,
                "Clear history failed",
                "Some captures could not be deleted",
            );
            return;
        }
        self.items.clear();
        self.media.clear();
        self.selected_index = 0;
        cx.notify();
    }

    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close(window, cx);
        cx.defer(|cx| {
            crate::intents::dispatch(crate::system::tray::Intent::OpenSettings, None, cx)
        });
    }

    pub fn set_item_menu_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.item_menu_open == open {
            return;
        }
        self.item_menu_open = open;
        cx.notify();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self.visible().len();
        if count == 0 {
            return;
        }
        // The renderer flips `isKeyboardNavigationActive` on any navigation
        // key, so the ring appears even when the index is already clamped.
        let already_navigating = self.keyboard_navigation;
        self.keyboard_navigation = true;
        let next = (self.selected_index as isize + delta).clamp(0, count as isize - 1) as usize;
        if next == self.selected_index {
            if !already_navigating {
                cx.notify();
            }
            return;
        }
        self.selected_index = next;
        self.scroll.scroll_to_item(next);
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.item_menu_open || !self.focus_handle.is_focused(window) {
            return;
        }
        let columns = self.layout.columns() as isize;
        match event.keystroke.key.as_str() {
            "up" | "k" => self.move_selection(-columns, cx),
            "down" | "j" => self.move_selection(columns, cx),
            "left" | "h" => self.move_selection(-1, cx),
            "right" | "l" => self.move_selection(1, cx),
            "enter" => self.open_index(self.selected_index, window, cx),
            "backspace" | "d" => self.delete_index(self.selected_index, cx),
            "escape" | "q" => self.close(window, cx),
            _ => {}
        }
    }
}

fn popover_placement(
    tray_rect: Option<TrayRect>,
    cx: &mut App,
) -> (Bounds<Pixels>, Option<gpui::DisplayId>) {
    let popover = size(px(POPOVER_WIDTH), px(POPOVER_HEIGHT));
    let Some(display) = crate::windows::tray_menu::menu_display(tray_rect, cx) else {
        return (Bounds::centered(None, popover, cx), None);
    };
    (
        popover_bounds(display.tray_rect, display.work_area),
        Some(display.id),
    )
}

fn popover_bounds(tray_rect: Option<TrayRect>, work_area: Bounds<Pixels>) -> Bounds<Pixels> {
    let popover = size(
        px(POPOVER_WIDTH.min(f32::from(work_area.size.width))),
        px(POPOVER_HEIGHT.min(f32::from(work_area.size.height))),
    );
    let tray = tray_rect.map(|rect| {
        (
            rect.x - f32::from(work_area.origin.x),
            rect.y - f32::from(work_area.origin.y),
            rect.width,
            rect.height,
        )
    });
    let (x, y) = crate::ui::chrome::history_popover_origin(
        tray,
        f32::from(work_area.size.width),
        f32::from(work_area.size.height),
    );
    Bounds {
        origin: gpui::point(work_area.origin.x + px(x), work_area.origin.y + px(y)),
        size: popover,
    }
}

impl Render for HistoryWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let has_items = !self.items.is_empty();
        let visible = self.visible();
        let card_width = ((f32::from(window.viewport_size().width)
            - crate::ui::chrome::HISTORY_CONTENT_PAD * 2.0
            - crate::ui::chrome::HISTORY_GRID_GAP)
            / 2.0)
            .max(0.0);
        let hovered_id = if window.is_window_hovered() {
            self.hovered_id.as_deref()
        } else {
            None
        };

        let mut scroller = div()
            .id("history-scroll")
            .debug_selector(|| "history-scroll".to_string())
            .track_scroll(&self.scroll)
            .size_full()
            .overflow_y_scroll()
            .p(px(crate::ui::chrome::HISTORY_CONTENT_PAD))
            .flex()
            .when(self.layout == HistoryLayout::Grid, |el| {
                el.flex_row()
                    .flex_wrap()
                    .gap(px(crate::ui::chrome::HISTORY_GRID_GAP))
            })
            .when(self.layout == HistoryLayout::List, |el| {
                el.flex_col().gap(px(crate::ui::chrome::HISTORY_LIST_GAP))
            });

        if !has_items {
            scroller = scroller.flex_col().child(toolbar::empty_state(
                crate::ui::chrome::HISTORY_EMPTY_ICON,
                "No captures yet",
                Some("Take a screenshot or record a video"),
                &theme,
            ));
        } else if visible.is_empty() {
            scroller = scroller.flex_col().child(toolbar::empty_state(
                crate::ui::chrome::HISTORY_EMPTY_FILTER_ICON,
                self.filter.empty_label(),
                None,
                &theme,
            ));
        } else {
            for (index, entry) in visible.iter().enumerate() {
                let media = self.media.get(&entry.id).cloned().unwrap_or_default();
                let view = item::ItemView {
                    item: entry,
                    index,
                    selected: self.keyboard_navigation && index == self.selected_index,
                    hovered: hovered_id == Some(entry.id.as_str()),
                    loading: !self.media.contains_key(&entry.id),
                    thumbnail: media.thumbnail,
                    features: media.features,
                    now_ms: self.now_ms,
                    theme: &theme,
                };
                scroller = scroller.child(match self.layout {
                    HistoryLayout::Grid => item::grid_card(&view, card_width, cx),
                    HistoryLayout::List => item::list_row(&view, cx),
                });
            }
        }

        let content = crate::ui::font::root()
            .id("history-window")
            .key_context("HistoryWindow")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(crate::ui::chrome::HISTORY_RADIUS))
            .when(!cfg!(target_os = "macos"), |el| el.bg(theme.background))
            .text_color(theme.foreground)
            .child(toolbar::header(has_items, &theme, cx))
            .when(has_items, |el| {
                el.child(toolbar::toolbar(
                    self.filter,
                    self.sort_order,
                    self.layout,
                    &theme,
                    cx,
                ))
            })
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(scroller)
                    .child(crate::windows::scrollbars::app_vertical(
                        "history-scrollbar",
                        &self.scroll,
                    )),
            );
        crate::ui::window_root::focus_root(content, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use herogpui::test::TestWindowExt;

    #[test]
    fn history_closes_when_the_window_blurs() {
        assert!(should_close_on_blur(false, false));
        assert!(!should_close_on_blur(true, false));
        assert!(!should_close_on_blur(false, true));
        assert!(!should_close_on_blur(true, true));
    }

    #[test]
    fn history_bounds_fit_small_and_offset_work_areas() {
        for (width, height) in [(300.0, 400.0), (640.0, 384.0), (1920.0, 1080.0)] {
            let work_area = Bounds {
                origin: gpui::point(px(-1280.0), px(200.0)),
                size: size(px(width), px(height)),
            };
            for tray in [
                None,
                Some(TrayRect {
                    x: -1280.0,
                    y: 200.0,
                    width: 20.0,
                    height: 20.0,
                }),
                Some(TrayRect {
                    x: -1280.0 + width - 20.0,
                    y: 200.0 + height - 20.0,
                    width: 20.0,
                    height: 20.0,
                }),
            ] {
                let bounds = popover_bounds(tray, work_area);
                assert_eq!(
                    bounds.size,
                    size(px(width.min(400.0)), px(height.min(500.0)))
                );
                assert!(bounds.origin.x >= work_area.origin.x);
                assert!(bounds.origin.y >= work_area.origin.y);
                assert!(
                    bounds.origin.x + bounds.size.width
                        <= work_area.origin.x + work_area.size.width
                );
                assert!(
                    bounds.origin.y + bounds.size.height
                        <= work_area.origin.y + work_area.size.height
                );
            }
        }
    }

    #[herogpui::test]
    fn history_layout_fits_narrow_windows(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(ConfigStore::load_at(directory.path().join("config.json")).unwrap());
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let (history, visual) = herogpui::test::open_window(cx, |window, cx| {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            HistoryWindow {
                items: ["first", "second", "third"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, id)| HistoryItem {
                        id: id.into(),
                        timestamp: index as i64,
                        original_path: String::new(),
                        r#type: HistoryItemType::Screenshot,
                        editor_state: None,
                        duration: None,
                    })
                    .collect(),
                media: HashMap::new(),
                filter: HistoryFilter::All,
                sort_order: HistorySortOrder::Newest,
                layout: HistoryLayout::Grid,
                selected_index: 0,
                keyboard_navigation: false,
                hovered_id: None,
                item_menu_open: false,
                scroll: ScrollHandle::new(),
                store,
                focus_handle,
                now_ms: 0,
                activation: None,
                revealing: false,
                closing: false,
            }
        });
        for mode in [
            crate::theme::presets::ThemeMode::Light,
            crate::theme::presets::ThemeMode::Dark,
        ] {
            visual.update(|_, cx| crate::theme::vars::update_theme(cx, mode, "default"));
            for scale in [1.0, 1.25, 2.0] {
                visual.update(|window, _| window.set_scale_factor(scale));
                assert_eq!(visual.update(|window, _| window.scale_factor()), scale);
                for width in [200.0, 220.0, 240.0, 250.0, 300.0, 320.0, 400.0] {
                    visual.simulate_resize(size(px(width), px(364.0)));
                    let first = visual.expect("history-card-first");
                    let second = visual.expect("history-card-second");
                    let third = visual.expect("history-card-third");
                    let filters = visual.expect("history-filters");
                    let actions = visual.expect("history-toolbar-actions");
                    let scroller = visual.expect("history-scroll");
                    let title = visual.expect("history-title");
                    let header_actions = visual.expect("history-header-actions");
                    assert_eq!(first.origin.y, second.origin.y);
                    assert!(first.origin.x + first.size.width < second.origin.x);
                    assert!(second.origin.x + second.size.width <= px(width));
                    assert!(third.origin.y > first.origin.y);
                    assert!(filters.origin.x + filters.size.width <= px(width));
                    assert!(
                        filters.origin.x + filters.size.width <= actions.origin.x
                            || filters.origin.y + filters.size.height <= actions.origin.y
                    );
                    assert!(actions.origin.x + actions.size.width <= px(width));
                    assert!(actions.origin.y + actions.size.height <= scroller.origin.y);
                    assert!(
                        title.origin.x + title.size.width <= header_actions.origin.x
                            || title.origin.y + title.size.height <= header_actions.origin.y
                    );
                    assert!(header_actions.origin.x + header_actions.size.width <= px(width));
                    for filter in ["all", "screenshot", "video"] {
                        let control = visual.expect(&format!("history-filter-{filter}"));
                        assert!(control.origin.x >= filters.origin.x);
                        assert!(
                            control.origin.x + control.size.width
                                <= filters.origin.x + filters.size.width
                        );
                        assert!(control.origin.y >= filters.origin.y);
                        assert!(
                            control.origin.y + control.size.height
                                <= filters.origin.y + filters.size.height
                        );
                    }
                }
            }
        }
        visual.press("down");
        assert_eq!(
            history.read_with(visual, |history, _| history.selected_index),
            2
        );
        let third = visual.expect("history-card-third");
        let scroller = visual.expect("history-scroll");
        assert!(third.origin.y >= scroller.origin.y);
        assert!(third.origin.y + third.size.height <= scroller.origin.y + scroller.size.height);

        visual.simulate_resize(size(px(200.0), px(364.0)));
        visual.click("history-filter-video");
        assert_eq!(
            history.read_with(visual, |history, _| history.filter),
            HistoryFilter::Video
        );
        assert_eq!(
            history.read_with(visual, |history, _| history.visible().len()),
            0
        );
        visual.click("history-filter-all");
        assert_eq!(
            history.read_with(visual, |history, _| history.filter),
            HistoryFilter::All
        );
        let actions = visual.expect("history-toolbar-actions");
        visual.click_at(gpui::point(
            actions.origin.x + actions.size.width - px(12.0),
            actions.center().y,
        ));
        assert_eq!(
            history.read_with(visual, |history, _| history.layout),
            HistoryLayout::List
        );
        visual.update(|window, cx| {
            let focus = history.read(cx).focus_handle.clone();
            window.focus(&focus, cx);
        });
        visual.press("down");
        assert_eq!(
            history.read_with(visual, |history, _| history.selected_index),
            1
        );
        let selected = visual.expect("history-row-second");
        let scroller = visual.expect("history-scroll");
        assert!(selected.origin.x >= scroller.origin.x);
        assert!(selected.origin.x + selected.size.width <= scroller.origin.x + scroller.size.width);
        assert!(selected.origin.y >= scroller.origin.y);
        assert!(
            selected.origin.y + selected.size.height <= scroller.origin.y + scroller.size.height
        );
    }
}
