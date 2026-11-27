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

fn load_media_batch(
    pending: impl IntoIterator<Item = (String, PathBuf, HistoryItemType)>,
    canceled: smol::channel::Receiver<()>,
) -> Vec<(String, ItemMedia)> {
    pending
        .into_iter()
        .take_while(|_| !canceled.is_closed())
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
        .collect()
}

pub struct HistoryWindow {
    items: Vec<HistoryItem>,
    media: HashMap<String, ItemMedia>,
    media_cancel: Option<smol::channel::Sender<()>>,
    filter: HistoryFilter,
    sort_order: HistorySortOrder,
    layout: HistoryLayout,
    selected_index: usize,
    keyboard_navigation: bool,
    hovered_id: Option<String>,
    item_menu_open: bool,
    scroll: ScrollHandle,
    viewport: Option<(gpui::Size<Pixels>, f32)>,
    store: Arc<ConfigStore>,
    focus_handle: FocusHandle,
    now_ms: i64,
    activation: Option<gpui::Subscription>,
    revealing: bool,
    closing: bool,
    opening_screenshot: bool,
    polishing: bool,
    mutation_status: Option<&'static str>,
    loading: bool,
}

impl HistoryWindow {
    pub fn new(store: Arc<ConfigStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let preferences = store.get().history;
        let mut view = Self {
            items: Vec::new(),
            media: HashMap::new(),
            media_cancel: None,
            filter: HistoryFilter::parse(&preferences.filter),
            sort_order: HistorySortOrder::parse(&preferences.sort_order),
            layout: HistoryLayout::parse(&preferences.layout),
            selected_index: 0,
            keyboard_navigation: false,
            hovered_id: None,
            item_menu_open: false,
            scroll: ScrollHandle::new(),
            viewport: None,
            store,
            focus_handle: cx.focus_handle(),
            now_ms: chrono::Local::now().timestamp_millis(),
            activation: None,
            revealing: cfg!(windows),
            closing: false,
            opening_screenshot: false,
            polishing: false,
            mutation_status: None,
            loading: true,
        };
        view.activation = Some(cx.observe_window_activation(window, |this, window, cx| {
            this.close_if_inactive(window, cx);
        }));
        Self::load_items(cx);
        view
    }

    fn close_if_inactive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.revealing || self.closing || self.polishing {
            return;
        }
        if !should_close_on_blur(window.is_window_active(), self.item_menu_open) {
            return;
        }
        self.close(window, cx);
    }

    #[cfg(feature = "parity")]
    pub(crate) fn focus_for_parity(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
    }

    #[cfg(feature = "parity")]
    pub(crate) fn parity_state(&self) -> (bool, bool, usize, HistoryLayout) {
        (
            !self.loading
                && self
                    .items
                    .iter()
                    .all(|item| self.media.contains_key(&item.id)),
            self.keyboard_navigation,
            self.selected_index,
            self.layout,
        )
    }

    #[cfg(feature = "parity")]
    pub(crate) fn has_items_for_parity(&self) -> bool {
        !self.loading && !self.items.is_empty()
    }

    #[cfg(feature = "parity")]
    pub(crate) fn parity_item_bounds(&self, index: usize) -> Option<Bounds<Pixels>> {
        self.scroll.bounds_for_item(index)
    }

    pub fn toggle(tray_rect: Option<TrayRect>, cx: &mut App) {
        if let Some(handle) =
            registry::handle(RegistryKind::History, cx).and_then(|handle| handle.downcast::<Self>())
        {
            let _ = handle.update(cx, |this, window, cx| this.close(window, cx));
            return;
        }
        registry::open_or_activate(RegistryKind::History, cx, |cx| {
            let store = crate::state::state(cx).config;
            let (bounds, display_id) = popover_placement(tray_rect, cx);
            let window_bounds = display_id
                .and_then(|id| cx.find_display(id))
                .map(|display| {
                    crate::system::work_area::local_window_bounds(bounds, display.as_ref())
                })
                .unwrap_or(bounds);
            let make_options = || {
                super::popup_window_options(
                    window_bounds,
                    super::PopupWindowConfig {
                        show: !cfg!(windows),
                        display_id,
                        background: if cfg!(target_os = "macos") {
                            crate::system::reduced_transparency::window_background(
                                gpui::WindowBackgroundAppearance::Blurred,
                            )
                        } else if cfg!(target_os = "linux") {
                            gpui::WindowBackgroundAppearance::Transparent
                        } else {
                            gpui::WindowBackgroundAppearance::Opaque
                        },
                        ..Default::default()
                    },
                )
            };
            let build_history = |window: &mut Window, cx: &mut App| {
                crate::system::window_surface::disable_transitions(window);
                crate::system::window_surface::stage(window, bounds, None, true);
                let view = cx.new(|cx| Self::new(store, window, cx));
                let focus = view.read(cx).focus_handle.clone();
                window.focus(&focus, cx);
                #[cfg(windows)]
                {
                    let settled = view.clone();
                    window.on_next_frame(move |window, _cx| {
                        crate::system::window_surface::set_rounded_clip(
                            window,
                            crate::ui::chrome::HISTORY_RADIUS,
                        );
                        window.on_next_frame(move |window, _| {
                            crate::system::window_surface::reveal(window, true);
                            window.activate_window();
                            window.on_next_frame(move |window, cx| {
                                settled.update(cx, |this, cx| {
                                    this.revealing = false;
                                    this.close_if_inactive(window, cx);
                                });
                            });
                        });
                    });
                    window.activate_window();
                }
                view
            };
            #[cfg(target_os = "linux")]
            let (anchor, margin) = {
                let work = display_id
                    .and_then(|id| cx.find_display(id))
                    .map(|display| crate::system::work_area::work_area(display.as_ref()))
                    .unwrap_or(bounds);
                let (corner, gaps) = super::window_corner_gaps(
                    f32::from(bounds.origin.x),
                    f32::from(bounds.origin.y),
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height),
                    f32::from(work.origin.x),
                    f32::from(work.origin.y),
                    f32::from(work.size.width),
                    f32::from(work.size.height),
                );
                (super::layer_anchor(corner), Some(super::layer_margin(gaps)))
            };
            #[cfg(target_os = "linux")]
            let opened = super::open_wayland_layer_or_fallback(
                cx,
                gpui::layer_shell::LayerShellOptions {
                    namespace: "poratake-history".to_string(),
                    layer: gpui::layer_shell::Layer::Overlay,
                    anchor,
                    margin,
                    keyboard_interactivity: gpui::layer_shell::KeyboardInteractivity::OnDemand,
                    ..Default::default()
                },
                make_options,
                build_history,
            );
            #[cfg(not(target_os = "linux"))]
            let opened = cx
                .open_window(make_options(), build_history)
                .map(|handle| (handle, false));
            opened.ok().map(|(handle, _)| handle).map(Into::into)
        });
    }

    fn load_items(cx: &mut Context<Self>) {
        cx.spawn(async move |entity, cx| {
            let items = cx
                .background_executor()
                .spawn(async { history_store::load_history() })
                .await;
            let _ = entity.update(cx, |this, cx| {
                if this.closing || !this.loading {
                    return;
                }
                this.loading = false;
                this.items = items;
                this.now_ms = chrono::Local::now().timestamp_millis();
                this.selected_index = 0;
                this.load_media(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.media_cancel.take();
        self.media.clear();
        registry::forget(RegistryKind::History, cx);
        #[cfg(all(target_os = "macos", not(test)))]
        crate::system::window_visibility::disable_macos_transitions(window);
        crate::system::memory::reclaim_idle_memory();
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

        let (cancel, canceled) = smol::channel::bounded(1);
        self.media_cancel = Some(cancel);
        let task = cx
            .background_executor()
            .spawn(async move { load_media_batch(pending, canceled) });

        cx.spawn(async move |entity, cx| {
            let resolved = task.await;
            let _ = entity.update(cx, |this, cx| {
                if this.closing {
                    return;
                }
                this.media_cancel.take();
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
        let keep_selection_visible = self.selected_is_visible();
        self.layout = self.layout.toggled();
        if keep_selection_visible {
            self.scroll.scroll_to_item(self.selected_index);
        }
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
        if self.closing
            || self.opening_screenshot
            || self.polishing
            || self.mutation_status.is_some()
        {
            return;
        }
        let Some(item) = self.visible().get(index).map(|item| (*item).clone()) else {
            return;
        };
        let path = item.original_path.clone();
        let kind = item.r#type;
        if kind == HistoryItemType::Screenshot {
            self.opening_screenshot = true;
            let prepared = crate::prepare_editor_open(cx, path.clone());
            cx.spawn_in(window, async move |entity, cx| {
                let (state, image) = prepared.await;
                let _ = entity.update_in(cx, |this, window, cx| {
                    this.opening_screenshot = false;
                    if !this.closing && crate::open_editor_with_image(cx, &path, state, image) {
                        this.close(window, cx);
                    }
                });
            })
            .detach();
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

    pub fn polish_index(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing
            || self.opening_screenshot
            || self.polishing
            || self.mutation_status.is_some()
        {
            return;
        }
        let Some(item) = self.visible().get(index).map(|item| (*item).clone()) else {
            return;
        };
        if item.r#type != HistoryItemType::Screenshot {
            return;
        }
        let Some(preset) = self.store.read(|config| {
            crate::windows::capture_preview::starred_wallpaper_preset(&config.wallpaper).cloned()
        }) else {
            return;
        };
        self.polishing = true;
        cx.notify();
        let path = PathBuf::from(item.original_path);
        let scale = window.scale_factor();
        cx.spawn_in(window, async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::windows::capture_preview::prepare_screenshot_copy(
                        &path,
                        Some(&preset),
                        scale,
                    )
                })
                .await;
            let _ = entity.update_in(cx, |this, window, cx| {
                this.polishing = false;
                if this.closing {
                    return;
                }
                match result {
                    Ok(bytes) => {
                        crate::system::clipboard::ClipboardService::write_png(cx, bytes);
                        this.close(window, cx);
                    }
                    Err(error) => {
                        crate::windows::toast::Toast::show(cx, "Copy failed", error);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub fn reveal_index(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(item) = self.visible().get(index).map(|item| (*item).clone()) else {
            return;
        };
        crate::system::desktop::show_capture_in_folder(
            cx,
            std::path::Path::new(&item.original_path),
        );
        cx.notify();
    }

    fn can_mutate(&self) -> bool {
        !self.closing
            && !self.loading
            && !self.opening_screenshot
            && !self.polishing
            && self.mutation_status.is_none()
    }

    pub fn delete_index(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.can_mutate() {
            return;
        }
        let Some(item) = self.visible().get(index).map(|item| (*item).clone()) else {
            return;
        };
        self.mutation_status = Some("Deleting capture…");
        cx.notify();
        let id = item.id;
        cx.spawn(async move |entity, cx| {
            let delete_id = id.clone();
            let outcome = cx.background_executor()
                .spawn(async move { history_store::delete_item(&delete_id) })
                .await;
            let deleted = cx.update(|cx| {
                match outcome {
                    history_store::DeleteOutcome::Deleted => {}
                    history_store::DeleteOutcome::DeletedWithBackupWarning(path) => {
                        crate::windows::toast::Toast::show(
                            cx,
                            "History recovery copy remains",
                            format!("Could not remove the saved recovery copy at {path}"),
                        );
                    }
                    history_store::DeleteOutcome::HistoryPersistenceFailed(error) => {
                        crate::windows::toast::Toast::show(
                            cx,
                            "History update failed",
                            format!(
                                "The capture was deleted. Its history entry may return after restart. {error}"
                            ),
                        );
                    }
                    history_store::DeleteOutcome::SourceDeletionFailed
                    | history_store::DeleteOutcome::HistoryUnavailable
                    | history_store::DeleteOutcome::NotFound => {
                        crate::windows::toast::Toast::show(
                            cx,
                            "Delete failed",
                            "The capture could not be deleted",
                        );
                        return false;
                    }
                }
                true
            });
            let _ = entity.update(cx, |this, cx| {
                this.mutation_status = None;
                if !deleted {
                    cx.notify();
                    return;
                }
                this.media.remove(&id);
                this.items.retain(|candidate| candidate.id != id);

                let remaining = this.visible().len();
                this.selected_index = match remaining {
                    0 => 0,
                    count if this.selected_index >= count => count - 1,
                    _ => this.selected_index,
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub fn clear_all(&mut self, cx: &mut Context<Self>) {
        if !self.can_mutate() {
            return;
        }
        self.mutation_status = Some("Clearing history…");
        cx.notify();
        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async { history_store::clear_history() })
                .await;
            let retained = cx.update(|cx| {
                let outcome = match result {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        crate::windows::toast::Toast::show(
                            cx,
                            "Clear history failed",
                            format!("History could not be read. No captures were deleted. {error}"),
                        );
                        return None;
                    }
                };
                if !outcome.history_updated || outcome.source_deletion_failures > 0 {
                    let source_failure = if outcome.source_deletion_failures == 0 {
                        String::new()
                    } else {
                        format!(
                            "{} capture(s) could not be deleted. ",
                            outcome.source_deletion_failures
                        )
                    };
                    let history_failure = if outcome.history_updated {
                        String::new()
                    } else {
                        format!(
                            "Deleted captures may return in history after restart: {}.",
                            outcome
                                .history_error
                                .as_deref()
                                .unwrap_or("history write failed")
                        )
                    };
                    crate::windows::toast::Toast::show(
                        cx,
                        if outcome.history_updated {
                            "Clear history incomplete"
                        } else {
                            "History update failed"
                        },
                        format!("{source_failure}{history_failure}"),
                    );
                }
                if let Some(path) = outcome.backup_warning.as_deref() {
                    crate::windows::toast::Toast::show(
                        cx,
                        "History recovery copy remains",
                        format!("Could not remove the saved recovery copy at {path}"),
                    );
                }
                Some(outcome.retained)
            });
            let _ = entity.update(cx, |this, cx| {
                this.mutation_status = None;
                let Some(retained) = retained else {
                    cx.notify();
                    return;
                };
                let retained_ids: std::collections::HashSet<_> =
                    retained.iter().map(|item| item.id.clone()).collect();
                this.items = retained;
                this.media.retain(|id, _| retained_ids.contains(id));
                this.selected_index = 0;
                cx.notify();
            });
        })
        .detach();
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

    fn selected_is_visible(&self) -> bool {
        if !self.keyboard_navigation {
            return false;
        }
        let Some(item) = self.scroll.bounds_for_item(self.selected_index) else {
            return false;
        };
        let viewport = self.scroll.bounds();
        let offset = self.scroll.offset().y;
        item.bottom() + offset > viewport.top() && item.top() + offset < viewport.bottom()
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.item_menu_open || !self.focus_handle.is_focused(window) {
            return;
        }
        let columns = if self.layout == HistoryLayout::Grid {
            grid_layout(window).0 as isize
        } else {
            1
        };
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

fn grid_layout(window: &Window) -> (usize, f32) {
    let scale = window.scale_factor();
    let width = window.pixel_snap(window.viewport_size().width)
        - window.pixel_snap(px(crate::ui::chrome::HISTORY_CONTENT_PAD)) * 2.0;
    let available = width - window.pixel_snap(px(crate::ui::chrome::HISTORY_GRID_GAP));
    let physical_card_width = ((f32::from(available) * scale).round() / 2.0).floor();
    let card_width = (physical_card_width / scale).max(0.0);
    let card_width = if window.pixel_snap(px(card_width)) * 2.0 > available {
        (physical_card_width - 1.0).max(0.0) / scale
    } else {
        card_width
    };
    let min_card_width = crate::ui::chrome::HISTORY_ITEM_ACTION_SIZE * 2.0 + 12.0;
    if card_width < min_card_width {
        (1, f32::from(width).max(0.0))
    } else {
        (model::GRID_COLUMNS, card_width)
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
        let viewport = (window.viewport_size(), window.scale_factor());
        if self.viewport.replace(viewport) != Some(viewport) && self.selected_is_visible() {
            self.scroll.scroll_to_item(self.selected_index);
        }
        let visible = self.visible();
        let polish_available = self.store.read(|config| {
            crate::windows::capture_preview::starred_wallpaper_preset(&config.wallpaper).is_some()
        });
        let (columns, card_width) = grid_layout(window);
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
                    .content_start()
                    .gap(px(crate::ui::chrome::HISTORY_GRID_GAP))
            })
            .when(self.layout == HistoryLayout::List, |el| {
                el.flex_col().gap(px(crate::ui::chrome::HISTORY_LIST_GAP))
            });

        if self.loading {
            scroller = scroller.flex_col().child(
                div()
                    .debug_selector(|| "history-loading".to_string())
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .text_color(theme.muted_foreground)
                    .text_center()
                    .text_size(px(crate::ui::chrome::TEXT_SM))
                    .child(crate::ui::icon::spinner_element("history-loading", px(16.)))
                    .child(div().w_full().child("Loading recent captures…")),
            );
        } else if !has_items {
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
                    polish_available: polish_available
                        && entry.r#type == HistoryItemType::Screenshot,
                    theme: &theme,
                };
                scroller = scroller.child(match self.layout {
                    HistoryLayout::Grid if columns == 1 => {
                        let card = item::grid_card(&view, (card_width - 8.0).max(0.0), columns, cx);
                        let padded = div().p(px(4.0)).child(card);
                        #[cfg(feature = "parity")]
                        let padded = if view.selected {
                            padded.on_children_prepainted(|bounds, _, _| {
                                crate::ui::parity::record("history-selected-grid-card", bounds);
                            })
                        } else {
                            padded
                        };
                        padded.into_any_element()
                    }
                    HistoryLayout::Grid => item::grid_card(&view, card_width, columns, cx),
                    HistoryLayout::List => {
                        item::list_row(&view, f32::from(window.viewport_size().width), cx)
                    }
                });
            }
        }

        let background =
            if cfg!(target_os = "macos") && !crate::system::reduced_transparency::enabled() {
                theme.background.opacity(0.9)
            } else {
                theme.background
            };
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
            .bg(background)
            .text_color(theme.foreground)
            .child(toolbar::header(has_items, columns == 1, &theme, cx))
            .children(
                if self.polishing {
                    Some("Polishing…")
                } else {
                    self.mutation_status
                }
                .map(|status| {
                    div()
                        .debug_selector(|| "history-status".to_string())
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(crate::ui::chrome::HISTORY_CONTENT_PAD))
                        .text_size(px(crate::ui::chrome::TEXT_SM))
                        .text_color(theme.muted_foreground)
                        .child(crate::ui::icon::spinner_element(
                            "history-mutation",
                            px(14.),
                        ))
                        .child(status)
                }),
            )
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
    fn thumbnail_batch_stops_after_cancellation_between_items() {
        let directory = tempfile::tempdir().unwrap();
        let (cancel, canceled) = smol::channel::bounded(1);
        let mut cancel = Some(cancel);
        let pending = (0..4).map(|index| {
            if index == 1 {
                cancel.take();
            }
            (
                index.to_string(),
                directory.path().join(format!("{index}.png")),
                HistoryItemType::Screenshot,
            )
        });
        let resolved = load_media_batch(pending, canceled);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, "0");
    }

    #[test]
    fn history_closes_when_the_window_blurs() {
        assert!(should_close_on_blur(false, false));
        assert!(!should_close_on_blur(true, false));
        assert!(!should_close_on_blur(false, true));
        assert!(!should_close_on_blur(true, true));
    }

    #[herogpui::test]
    fn history_reveal_uses_the_platform_file_manager(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(ConfigStore::load_at(directory.path().join("config.json")).unwrap());
        let path = directory.path().join("capture with spaces.png");
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let history = cx.add_window(|window, cx| {
            let mut history = HistoryWindow::new(store, window, cx);
            history.loading = false;
            history.items = vec![HistoryItem {
                id: "capture".into(),
                timestamp: 1,
                original_path: path.to_string_lossy().into_owned(),
                r#type: HistoryItemType::Screenshot,
                editor_state: None,
                duration: None,
            }];
            history.filter = HistoryFilter::All;
            history.sort_order = HistorySortOrder::Newest;
            history
        });

        history
            .update(cx, |history, _, cx| history.reveal_index(0, cx))
            .unwrap();
        cx.run_until_parked();

        assert_eq!(cx.revealed_paths(), vec![path]);
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
    fn history_retries_after_open_failure_and_prepares_only_one_editor(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::editor::EditorWindow;

        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(ConfigStore::load_at(directory.path().join("config.json")).unwrap());
        let path = directory.path().join("capture.png");
        let pixels = image::RgbaImage::from_pixel(20, 10, image::Rgba([64, 112, 192, 255]));
        pixels.save(&path).unwrap();
        let missing = directory.path().join("missing.png");
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let history = cx.add_window(|window, cx| {
            let mut history = HistoryWindow::new(store, window, cx);
            history.loading = false;
            history.items = [(&missing, 2), (&path, 1)]
                .into_iter()
                .map(|(path, timestamp)| HistoryItem {
                    id: timestamp.to_string(),
                    timestamp,
                    original_path: path.to_string_lossy().into_owned(),
                    r#type: HistoryItemType::Screenshot,
                    editor_state: None,
                    duration: None,
                })
                .collect();
            history.filter = HistoryFilter::All;
            history.sort_order = HistorySortOrder::Newest;
            history
        });
        let find_editors = |cx: &App| {
            cx.windows()
                .into_iter()
                .filter_map(|window| window.downcast::<EditorWindow>())
                .collect::<Vec<_>>()
        };

        history
            .update(cx, |history, window, cx| history.open_index(0, window, cx))
            .unwrap();
        cx.run_until_parked();
        assert!(cx.read(|cx| find_editors(cx).is_empty()));
        assert!(cx.read(|cx| cx.windows().contains(&history.into())));

        history
            .update(cx, |history, window, cx| {
                history.open_index(1, window, cx);
                history.open_index(1, window, cx);
                assert!(find_editors(cx).is_empty());
            })
            .unwrap();
        cx.run_until_parked();
        cx.read(|cx| {
            assert!(!cx.windows().contains(&history.into()));
            let editors = find_editors(cx);
            assert_eq!(editors.len(), 1);
            let editor = editors[0].read(cx).unwrap();
            assert_eq!(PathBuf::from(&editor.file_path), path);
            let png = editor.export_png().unwrap();
            assert_eq!(image::load_from_memory(&png).unwrap().to_rgba8(), pixels);
        });
    }

    fn set_polish_preset(store: &ConfigStore) {
        store.update(|settings| {
            settings.wallpaper.presets = vec![serde_json::from_value(serde_json::json!({
                "id": "starred", "name": "Starred", "padding": 20,
                "corners": 12, "shadow": 0,
            }))
            .unwrap()];
            settings.wallpaper.default_preset_id = Some("starred".into());
        });
    }

    #[herogpui::test]
    fn history_polish_copies_starred_screenshot_after_blur(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(ConfigStore::load_at(directory.path().join("config.json")).unwrap());
        set_polish_preset(&store);
        let path = directory.path().join("capture.png");
        let source = image::RgbaImage::from_pixel(160, 100, image::Rgba([240, 80, 64, 255]));
        source.save(&path).unwrap();
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let history = cx.add_window(|window, cx| {
            window.activate_window();
            window.set_scale_factor(2.0);
            let mut history = HistoryWindow::new(store.clone(), window, cx);
            history.loading = false;
            history.revealing = false;
            history.filter = HistoryFilter::All;
            history.items = vec![HistoryItem {
                id: "capture".into(),
                timestamp: 1,
                original_path: path.to_string_lossy().into_owned(),
                r#type: HistoryItemType::Screenshot,
                editor_state: None,
                duration: None,
            }];
            history
        });
        cx.run_until_parked();

        history
            .update(cx, |history, window, cx| {
                assert!(window.is_window_active());
                history.polish_index(0, window, cx);
                history.polish_index(0, window, cx);
                assert!(history.polishing);
            })
            .unwrap();
        let destination = cx.add_window(|window, _| {
            window.activate_window();
            gpui::Empty
        });
        cx.run_until_parked();

        cx.update(|cx| {
            assert_eq!(cx.windows(), vec![destination.into()]);
            let clipboard = crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
            let copied = image::load_from_memory(&clipboard.bytes)
                .unwrap()
                .to_rgba8();
            assert_eq!(copied.dimensions(), (240, 180));
            assert_eq!(copied.get_pixel(70, 70).0, [240, 80, 64, 255]);
        });
        assert_eq!(
            image::load_from_memory(&std::fs::read(&path).unwrap())
                .unwrap()
                .to_rgba8(),
            source
        );
    }

    #[herogpui::test]
    fn history_polish_failure_toasts_and_allows_retry(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(ConfigStore::load_at(directory.path().join("config.json")).unwrap());
        set_polish_preset(&store);
        let path = directory.path().join("capture.png");
        let notifications = crate::system::notification::capture_for_test();
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let history = cx.add_window(|window, cx| {
            window.set_scale_factor(2.0);
            let mut history = HistoryWindow::new(store.clone(), window, cx);
            history.loading = false;
            history.filter = HistoryFilter::All;
            history.sort_order = HistorySortOrder::Newest;
            history.items = vec![HistoryItem {
                id: "capture".into(),
                timestamp: 1,
                original_path: path.to_string_lossy().into_owned(),
                r#type: HistoryItemType::Screenshot,
                editor_state: None,
                duration: None,
            }];
            history
        });

        history
            .update(cx, |history, window, cx| {
                history.polish_index(0, window, cx)
            })
            .unwrap();
        cx.run_until_parked();
        cx.read(|cx| assert!(cx.windows().contains(&history.into())));
        assert_eq!(notifications.borrow().as_slice(), ["Copy failed"]);
        history
            .update(cx, |history, _, _| assert!(!history.polishing))
            .unwrap();

        image::RgbaImage::from_pixel(160, 100, image::Rgba([240, 80, 64, 255]))
            .save(&path)
            .unwrap();
        history
            .update(cx, |history, window, cx| {
                history.polish_index(0, window, cx)
            })
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(!cx.windows().contains(&history.into()));
            let clipboard = crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
            let copied = image::load_from_memory(&clipboard.bytes)
                .unwrap()
                .to_rgba8();
            assert_eq!(copied.dimensions(), (240, 180));
            assert_eq!(copied.get_pixel(70, 70).0, [240, 80, 64, 255]);
        });
        assert_eq!(
            notifications
                .borrow()
                .iter()
                .filter(|title| title.as_str() == "Copy failed")
                .count(),
            1
        );
    }

    #[herogpui::test]
    fn history_polish_rejects_videos_and_missing_starred_presets(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(ConfigStore::load_at(directory.path().join("config.json")).unwrap());
        let path = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 10, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let history = cx.add_window(|window, cx| {
            let mut history = HistoryWindow::new(store.clone(), window, cx);
            history.loading = false;
            history.filter = HistoryFilter::All;
            history.sort_order = HistorySortOrder::Newest;
            history.items = vec![HistoryItem {
                id: "capture".into(),
                timestamp: 1,
                original_path: path.to_string_lossy().into_owned(),
                r#type: HistoryItemType::Screenshot,
                editor_state: None,
                duration: None,
            }];
            history
        });

        history
            .update(cx, |history, window, cx| {
                history.polish_index(0, window, cx);
                assert!(!history.polishing);
            })
            .unwrap();
        cx.run_until_parked();
        cx.read(|cx| assert!(cx.windows().contains(&history.into())));

        set_polish_preset(&store);
        history
            .update(cx, |history, window, cx| {
                history.items[0].r#type = HistoryItemType::Video;
                history.polish_index(0, window, cx);
                assert!(!history.polishing);
            })
            .unwrap();
        cx.run_until_parked();
        cx.read(|cx| assert!(cx.windows().contains(&history.into())));
    }

    #[herogpui::test]
    fn history_polish_action_visibility_matches_starred_screenshots(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(ConfigStore::load_at(directory.path().join("config.json")).unwrap());
        set_polish_preset(&store);
        let source_path = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(160, 100, image::Rgba([240, 80, 64, 255]))
            .save(&source_path)
            .unwrap();
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let (history, visual) = herogpui::test::open_window(cx, |window, cx| {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            HistoryWindow {
                items: vec![
                    HistoryItem {
                        id: "capture".into(),
                        timestamp: 2,
                        original_path: source_path.to_string_lossy().into_owned(),
                        r#type: HistoryItemType::Screenshot,
                        editor_state: None,
                        duration: None,
                    },
                    HistoryItem {
                        id: "recording".into(),
                        timestamp: 1,
                        original_path: String::new(),
                        r#type: HistoryItemType::Video,
                        editor_state: None,
                        duration: None,
                    },
                ],
                media: HashMap::new(),
                media_cancel: None,
                filter: HistoryFilter::All,
                sort_order: HistorySortOrder::Newest,
                layout: HistoryLayout::Grid,
                selected_index: 0,
                keyboard_navigation: true,
                hovered_id: None,
                item_menu_open: false,
                scroll: ScrollHandle::new(),
                viewport: None,
                store: store.clone(),
                focus_handle,
                now_ms: 0,
                activation: None,
                revealing: false,
                closing: false,
                opening_screenshot: false,
                polishing: false,
                mutation_status: None,
                loading: false,
            }
        });
        visual.simulate_scale_factor_change(1.0);
        visual.simulate_resize(size(px(400.0), px(500.0)));
        visual.settle();
        assert!(visual
            .debug_bounds("history-polish-action-capture")
            .is_some());
        assert!(visual
            .debug_bounds("history-polish-action-recording")
            .is_none());

        visual.update(|_, cx| {
            history.update(cx, |history, cx| {
                history.selected_index = 1;
                cx.notify();
            });
        });
        visual.settle();
        assert!(visual
            .debug_bounds("history-polish-action-capture")
            .is_none());
        assert!(visual
            .debug_bounds("history-polish-action-recording")
            .is_none());

        visual.update(|_, cx| {
            history.update(cx, |history, cx| {
                history.selected_index = 0;
                cx.notify();
            });
        });
        visual.simulate_resize(size(px(160.0), px(500.0)));
        visual.settle();
        assert!(visual
            .debug_bounds("history-polish-action-capture")
            .is_none());
        assert!(visual
            .debug_bounds("history-grid-actions-capture")
            .is_some());

        visual.update(|_, cx| {
            history.update(cx, |history, cx| {
                history.selected_index = 0;
                history.layout = HistoryLayout::List;
                cx.notify();
            });
        });
        visual.simulate_resize(size(px(200.0), px(500.0)));
        visual.settle();
        assert!(visual
            .debug_bounds("history-polish-action-capture")
            .is_none());
        assert!(visual.debug_bounds("history-row-actions-capture").is_some());
        let baseline_label = visual.expect("history-row-label-capture");

        visual.simulate_resize(size(px(320.0), px(500.0)));
        visual.settle();
        assert!(visual
            .debug_bounds("history-polish-action-capture")
            .is_none());
        let narrow_label = visual.expect("history-row-label-capture");
        assert_eq!(
            narrow_label.size.width, baseline_label.size.width,
            "320: Screenshot label {narrow_label:?} must match its untruncated baseline {baseline_label:?}"
        );

        visual.simulate_resize(size(px(400.0), px(500.0)));
        visual.settle();
        assert!(visual
            .debug_bounds("history-polish-action-capture")
            .is_some());
        let full_label = visual.expect("history-row-label-capture");
        assert_eq!(
            full_label.size.width, baseline_label.size.width,
            "400: Screenshot label {full_label:?} must match its untruncated baseline {baseline_label:?}"
        );

        store.update(|settings| settings.wallpaper.default_preset_id = None);
        visual.update(|_, cx| {
            history.update(cx, |_, cx| cx.notify());
        });
        visual.settle();
        assert!(visual
            .debug_bounds("history-polish-action-capture")
            .is_none());

        set_polish_preset(&store);
        visual.update(|_, cx| history.update(cx, |_, cx| cx.notify()));
        let position = visual.expect("history-polish-action-capture").center();
        visual.simulate_mouse_move(position, None, gpui::Modifiers::none());
        visual.simulate_click(position, gpui::Modifiers::none());
        cx.run_until_parked();
        cx.read(|cx| {
            assert!(cx.windows().is_empty());
            let clipboard = crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
            let copied = image::load_from_memory(&clipboard.bytes).unwrap();
            assert_eq!((copied.width(), copied.height()), (200, 140));
        });
    }

    #[herogpui::test]
    fn history_polishing_uses_the_progress_row(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(ConfigStore::load_at(directory.path().join("config.json")).unwrap());
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let (history, visual) = herogpui::test::open_window(cx, |window, cx| {
            let mut history = HistoryWindow::new(store.clone(), window, cx);
            history.loading = false;
            history
        });
        visual.simulate_resize(size(px(400.0), px(500.0)));
        visual.settle();
        assert!(visual.debug_bounds("history-status").is_none());

        visual.update(|_, cx| {
            history.update(cx, |history, cx| {
                history.polishing = true;
                cx.notify();
            });
        });
        visual.settle();
        assert!(visual.debug_bounds("history-status").is_some());

        visual.update(|_, cx| {
            history.update(cx, |history, cx| {
                history.polishing = false;
                cx.notify();
            });
        });
        visual.settle();
        assert!(visual.debug_bounds("history-status").is_none());
    }

    #[herogpui::test]
    fn history_close_if_inactive_defers_blur_while_revealing_polishing_or_menu_open(
        cx: &mut gpui::TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(ConfigStore::load_at(directory.path().join("config.json")).unwrap());
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let (history, visual) = herogpui::test::open_window(cx, |window, cx| {
            window.activate_window();
            let mut history = HistoryWindow::new(store.clone(), window, cx);
            history.loading = false;
            history
        });
        visual.settle();
        assert!(visual.update(|window, _| window.is_window_active()));

        visual.update(|_, cx| {
            history.update(cx, |history, _| history.revealing = true);
        });
        visual.deactivate_window();
        visual.settle();
        assert!(!visual.update(|window, _| window.is_window_active()));
        assert_eq!(visual.cx.update(|cx| cx.windows().len()), 1);

        visual.update(|window, cx| {
            history.update(cx, |history, cx| {
                history.revealing = false;
                history.polishing = true;
                history.close_if_inactive(window, cx);
            });
        });
        assert_eq!(visual.cx.update(|cx| cx.windows().len()), 1);

        visual.update(|window, cx| {
            history.update(cx, |history, cx| {
                history.polishing = false;
                history.item_menu_open = true;
                history.close_if_inactive(window, cx);
            });
        });
        assert_eq!(visual.cx.update(|cx| cx.windows().len()), 1);

        visual.update(|window, cx| {
            history.update(cx, |history, cx| {
                history.item_menu_open = false;
                history.close_if_inactive(window, cx);
            });
        });
        assert_eq!(visual.cx.update(|cx| cx.windows().len()), 0);
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
                media_cancel: None,
                filter: HistoryFilter::All,
                sort_order: HistorySortOrder::Newest,
                layout: HistoryLayout::Grid,
                selected_index: 0,
                keyboard_navigation: false,
                hovered_id: None,
                item_menu_open: false,
                scroll: ScrollHandle::new(),
                viewport: None,
                store,
                focus_handle,
                now_ms: 0,
                activation: None,
                revealing: false,
                closing: false,
                opening_screenshot: false,
                polishing: false,
                mutation_status: None,
                loading: false,
            }
        });
        visual.update(|_, cx| {
            crate::theme::vars::update_theme(cx, crate::theme::presets::ThemeMode::Light, "default")
        });
        visual.simulate_scale_factor_change(2.0);
        visual.simulate_resize(size(px(400.0), px(500.0)));
        visual.update(|_, cx| {
            history.update(cx, |history, cx| {
                history.loading = true;
                cx.notify();
            });
        });
        visual.settle();
        assert!(visual.debug_bounds("history-loading").is_some());
        assert!(visual.debug_bounds("history-card-first").is_none());
        visual.update(|_, cx| {
            history.update(cx, |history, cx| {
                history.loading = false;
                cx.notify();
            });
        });
        visual.settle();
        let first = visual.expect("history-card-first");
        let second = visual.expect("history-card-second");
        let third = visual.expect("history-card-third");
        assert_eq!(first.top(), second.top());
        assert_eq!(
            third.top() - first.bottom(),
            px(crate::ui::chrome::HISTORY_GRID_GAP)
        );
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            visual.simulate_scale_factor_change(scale);
            for width in [140.0, 148.0] {
                visual.simulate_resize(size(px(width), px(364.0)));
                let first = visual.expect("history-card-first");
                let second = visual.expect("history-card-second");
                assert!(
                    second.top() >= first.bottom(),
                    "scale {scale}, width {width}: cards must use one column: {first:?}, {second:?}"
                );
                visual.update(|_, cx| {
                    history.update(cx, |history, cx| {
                        history.selected_index = 0;
                        history.keyboard_navigation = false;
                        history.scroll.scroll_to_item(0);
                        cx.notify();
                    });
                });
                visual.press("right");
                assert_eq!(
                    history.read_with(visual, |history, _| history.selected_index),
                    1
                );
                visual.press("left");
                assert_eq!(
                    history.read_with(visual, |history, _| history.selected_index),
                    0
                );
                visual.press("down");
                assert_eq!(
                    history.read_with(visual, |history, _| history.selected_index),
                    1,
                    "scale {scale}, width {width}: Down must select the next row"
                );
                visual.press("up");
                assert_eq!(
                    history.read_with(visual, |history, _| history.selected_index),
                    0,
                    "scale {scale}, width {width}: Up must select the previous row"
                );
                visual.press("down");
                let second = visual.expect("history-card-second");
                let actions = visual.expect("history-grid-actions-second");
                let scroller = visual.expect("history-scroll");
                assert!(second.top() >= scroller.top() && second.bottom() <= scroller.bottom());
                assert!(actions.left() >= second.left() && actions.right() <= second.right());
                assert!(actions.top() >= second.top() && actions.bottom() <= second.bottom());
                let filters = visual.expect("history-filters");
                let toolbar_actions = visual.expect("history-toolbar-actions");
                let header_actions = visual.expect("history-header-actions");
                assert!(filters.right() <= px(width));
                assert!(toolbar_actions.right() <= px(width));
                assert!(
                    header_actions.right() <= px(width),
                    "width {width}, scale {scale}: header actions {header_actions:?}"
                );
                visual.update(|window, cx| {
                    history.update(cx, |history, cx| {
                        history.layout = HistoryLayout::List;
                        window.focus(&history.focus_handle, cx);
                        cx.notify();
                    });
                });
                let thumb = visual.expect("history-row-thumb-second");
                let actions = visual.expect("history-row-actions-second");
                assert!(
                    actions.left() >= thumb.left()
                        && actions.right() <= thumb.right()
                        && actions.top() >= thumb.top()
                        && actions.bottom() <= thumb.bottom(),
                    "width {width}, scale {scale}: list actions {actions:?}, thumbnail {thumb:?}"
                );
                visual.update(|_, cx| {
                    history.update(cx, |history, cx| {
                        history.layout = HistoryLayout::Grid;
                        history.selected_index = 0;
                        history.keyboard_navigation = false;
                        history.scroll.scroll_to_item(0);
                        cx.notify();
                    });
                });
            }
        }
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            visual.simulate_scale_factor_change(scale);
            visual.simulate_resize(size(px(400.0), px(364.0)));
            visual.update(|_, cx| {
                history.update(cx, |history, cx| {
                    history.selected_index = 2;
                    history.keyboard_navigation = true;
                    history.scroll.scroll_to_item(2);
                    cx.notify();
                });
            });
            visual.simulate_resize(size(px(140.0), px(364.0)));
            let selected = visual.expect("history-card-third");
            let scroller = visual.expect("history-scroll");
            assert!(
                selected.top() >= scroller.top() && selected.bottom() <= scroller.bottom(),
                "scale {scale}: selected card {selected:?}, scroller {scroller:?}"
            );
            let actions = visual.expect("history-toolbar-actions");
            let toggle = gpui::point(actions.right() - px(12.0), actions.center().y);
            visual.click_at(toggle);
            assert_eq!(
                history.read_with(visual, |history, _| history.layout),
                HistoryLayout::List
            );
            let selected = visual.expect("history-row-third");
            let scroller = visual.expect("history-scroll");
            assert!(
                selected.top() >= scroller.top() && selected.bottom() <= scroller.bottom(),
                "scale {scale}: selected row {selected:?}, scroller {scroller:?}"
            );
            visual.click_at(toggle);
            assert_eq!(
                history.read_with(visual, |history, _| history.layout),
                HistoryLayout::Grid
            );
        }
        visual.simulate_scale_factor_change(1.0);
        visual.simulate_resize(size(px(155.6), px(364.0)));
        assert_eq!(visual.update(|window, _| grid_layout(window).0), 2);
        visual.update(|_, cx| {
            history.update(cx, |history, cx| {
                history.selected_index = 2;
                history.keyboard_navigation = true;
                history.scroll.scroll_to_item(2);
                cx.notify();
            });
        });
        visual.simulate_scale_factor_change(1.25);
        assert_eq!(
            visual.update(|window, _| window.viewport_size().width),
            px(155.6)
        );
        assert_eq!(visual.update(|window, _| grid_layout(window).0), 1);
        let selected = visual.expect("history-card-third");
        let scroller = visual.expect("history-scroll");
        assert!(
            selected.top() >= scroller.top() && selected.bottom() <= scroller.bottom(),
            "DPI change: selected card {selected:?}, scroller {scroller:?}"
        );
        visual.simulate_scale_factor_change(1.0);
        visual.simulate_resize(size(px(140.0), px(364.0)));
        let scroller = visual.expect("history-scroll");
        visual.simulate_event(gpui::ScrollEvent {
            position: scroller.center(),
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(40.0))),
            modifiers: gpui::Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        visual.settle();
        let selected = visual.expect("history-card-third");
        let scroller = visual.expect("history-scroll");
        assert!(selected.top() < scroller.bottom() && selected.bottom() > scroller.bottom());
        visual.simulate_resize(size(px(142.0), px(364.0)));
        let selected = visual.expect("history-card-third");
        let scroller = visual.expect("history-scroll");
        assert!(selected.top() >= scroller.top() && selected.bottom() <= scroller.bottom());
        visual.simulate_resize(size(px(140.0), px(364.0)));
        let scroller = visual.expect("history-scroll");
        visual.simulate_event(gpui::ScrollEvent {
            position: scroller.center(),
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(600.0))),
            modifiers: gpui::Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        visual.settle();
        let first = visual.expect("history-card-first");
        let selected = visual.expect("history-card-third");
        let scroller = visual.expect("history-scroll");
        let first_offset = first.top() - scroller.top();
        assert!(selected.top() >= scroller.bottom());
        visual.simulate_resize(size(px(142.0), px(364.0)));
        let first = visual.expect("history-card-first");
        let selected = visual.expect("history-card-third");
        let scroller = visual.expect("history-scroll");
        assert_eq!(first.top() - scroller.top(), first_offset);
        assert!(
            selected.top() >= scroller.bottom(),
            "resize must preserve the manually scrolled position"
        );
        let actions = visual.expect("history-toolbar-actions");
        visual.click_at(gpui::point(actions.right() - px(12.0), actions.center().y));
        let first = visual.expect("history-row-first");
        let selected = visual.expect("history-row-third");
        let scroller = visual.expect("history-scroll");
        assert_eq!(
            first.top() - scroller.top(),
            px(crate::ui::chrome::HISTORY_CONTENT_PAD)
        );
        assert!(
            selected.top() >= scroller.bottom(),
            "layout change must preserve the manually scrolled position"
        );
        visual.click_at(gpui::point(actions.right() - px(12.0), actions.center().y));
        visual.update(|window, cx| {
            history.update(cx, |history, cx| {
                history.selected_index = 0;
                history.keyboard_navigation = false;
                history.scroll.scroll_to_item(0);
                window.focus(&history.focus_handle, cx);
                cx.notify();
            });
        });
        for mode in [
            crate::theme::presets::ThemeMode::Light,
            crate::theme::presets::ThemeMode::Dark,
        ] {
            visual.update(|_, cx| crate::theme::vars::update_theme(cx, mode, "default"));
            for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
                visual.simulate_scale_factor_change(scale);
                for width in [200.0, 220.0, 240.0, 250.0, 300.0, 320.0, 400.0] {
                    visual.simulate_resize(size(px(width), px(364.0)));
                    let first = visual.expect("history-card-first");
                    assert_eq!(visual.update(|window, _| window.scale_factor()), scale);
                    let second = visual.expect("history-card-second");
                    let third = visual.expect("history-card-third");
                    let filters = visual.expect("history-filters");
                    let actions = visual.expect("history-toolbar-actions");
                    let scroller = visual.expect("history-scroll");
                    let title = visual.expect("history-title");
                    let header_actions = visual.expect("history-header-actions");
                    assert_eq!(
                        first.origin.y, second.origin.y,
                        "scale {scale}, width {width}, first {first:?}, second {second:?}, scroller {scroller:?}"
                    );
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
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            visual.simulate_scale_factor_change(scale);
            for physical_width in (140.0 * scale) as u32..=(400.0 * scale) as u32 {
                let width = physical_width as f32 / scale;
                visual.simulate_resize(size(px(width), px(364.0)));
                let first = visual.expect("history-card-first");
                let second = visual.expect("history-card-second");
                let scroller = visual.expect("history-scroll");
                let header_actions = visual.expect("history-header-actions");
                assert_eq!(visual.update(|window, _| window.scale_factor()), scale);
                assert!(
                    header_actions.right() <= px(width),
                    "scale {scale}, physical width {physical_width}: {header_actions:?}"
                );
                if first.top() == second.top() {
                    assert!(first.right() < second.left());
                    assert!(first.size.width >= px(60.0));
                } else {
                    assert!(second.top() >= first.bottom());
                }
                assert!(second.right() <= scroller.right());
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

        for mode in [
            crate::theme::presets::ThemeMode::Light,
            crate::theme::presets::ThemeMode::Dark,
        ] {
            visual.update(|_, cx| crate::theme::vars::update_theme(cx, mode, "default"));
            for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
                visual.simulate_scale_factor_change(scale);
                for width in [140.0, 148.0, 200.0, 240.0, 400.0] {
                    visual.simulate_resize(size(px(width), px(364.0)));
                    for (index, id) in [(0, "first"), (2, "third"), (1, "second")] {
                        visual.update(|_, cx| {
                            history.update(cx, |history, cx| {
                                history.selected_index = index;
                                history.scroll.scroll_to_item(index);
                                cx.notify();
                            });
                        });
                        let actions = visual.expect(&format!("history-grid-actions-{id}"));
                        let scroller_top = if id == "first" {
                            let first = visual.expect("history-card-first");
                            let scroller = visual.expect("history-scroll");
                            assert!(first.top() >= scroller.top());
                            f32::from(scroller.top())
                        } else {
                            0.0
                        };
                        for offset in [12.0, 40.0] {
                            let action =
                                gpui::point(actions.origin.x + px(offset), actions.center().y);
                            visual.simulate_mouse_move(action, None, gpui::Modifiers::none());
                            visual
                                .executor()
                                .advance_clock(std::time::Duration::from_millis(800));
                            visual.settle();
                            visual.update(|window, _| {
                                assert_eq!(window.scale_factor(), scale);
                                let tips: Vec<_> = window
                                    .painted_quads()
                                    .into_iter()
                                    .filter(|quad| {
                                        let bounds = quad.bounds;
                                        (31.0..=35.0).contains(&(bounds.size.height.0 / scale))
                                            && bounds.size.width.0 / scale > 30.0
                                            && bounds.bottom().0 <= f32::from(action.y) * scale
                                            && bounds.bottom().0
                                                >= (f32::from(action.y) - 30.0) * scale
                                    })
                                    .collect();
                                assert_eq!(tips.len(), 1, "width {width}, item {id}: {tips:?}");
                                let tip = tips[0];
                                let bounds = tip.bounds;
                                let mask = tip.content_mask.bounds;
                                assert!(
                                    bounds.left().0 >= 0.0
                                        && bounds.right().0 <= width * scale
                                        && bounds.top().0 >= 0.0
                                        && bounds.bottom().0 <= 364.0 * scale
                                        && mask.left().0 <= bounds.left().0
                                        && mask.right().0 >= bounds.right().0
                                        && mask.top().0 <= bounds.top().0
                                        && mask.bottom().0 >= bounds.bottom().0,
                                    "width {width}, item {id}: tooltip {bounds:?}, mask {mask:?}"
                                );
                                if id == "first" {
                                    assert!(
                                        bounds.top().0 < scroller_top * scale,
                                        "width {width}: first grid tooltip must extend above the scroll viewport: {bounds:?}, scroller top={scroller_top}"
                                    );
                                }
                            });
                        }
                    }
                }
            }
        }

        visual.simulate_scale_factor_change(1.0);
        visual.simulate_resize(size(px(140.0), px(364.0)));
        visual.update(|_, cx| {
            history.update(cx, |history, cx| {
                history.layout = HistoryLayout::List;
                history.selected_index = 0;
                history.keyboard_navigation = true;
                history.scroll.scroll_to_item(0);
                cx.notify();
            });
        });
        let scroller = visual.expect("history-scroll");
        let first = visual.expect("history-row-first");
        let thumb = visual.expect("history-row-thumb-first");
        let actions = visual.expect("history-row-actions-first");
        assert!(
            first.top() >= scroller.top()
                && first.top() <= scroller.top() + px(crate::ui::chrome::HISTORY_CONTENT_PAD)
        );
        assert!(actions.top() >= thumb.top() && actions.bottom() <= thumb.bottom());
        for (offset, label) in [(12.0, "reveal"), (40.0, "delete")] {
            let action = gpui::point(actions.left() + px(offset), actions.center().y);
            visual.simulate_mouse_move(action, None, gpui::Modifiers::none());
            visual
                .executor()
                .advance_clock(std::time::Duration::from_millis(800));
            visual.settle();
            visual.update(|window, _| {
                let tips: Vec<_> = window
                    .painted_quads()
                    .into_iter()
                    .filter(|quad| {
                        let bounds = quad.bounds;
                        (31.0..=35.0).contains(&bounds.size.height.0)
                            && bounds.size.width.0 > 30.0
                            && bounds.bottom().0 <= f32::from(action.y)
                            && bounds.bottom().0 >= f32::from(action.y) - 30.0
                    })
                    .collect();
                assert_eq!(tips.len(), 1, "first list {label} tooltip: {tips:?}");
                let tip = tips[0];
                let bounds = tip.bounds;
                let mask = tip.content_mask.bounds;
                assert!(
                    bounds.left().0 >= 0.0
                        && bounds.right().0 <= 140.0
                        && bounds.top().0 >= 0.0
                        && bounds.bottom().0 <= 364.0
                        && bounds.top().0 < f32::from(scroller.top())
                        && bounds.top().0 < f32::from(thumb.top())
                        && mask.left().0 <= bounds.left().0
                        && mask.right().0 >= bounds.right().0
                        && mask.top().0 <= bounds.top().0
                        && mask.bottom().0 >= bounds.bottom().0,
                    "first list {label} tooltip is clipped at its scroll/thumb edge: {bounds:?}, mask {mask:?}, scroller {scroller:?}, thumb {thumb:?}"
                );
            });
        }
        visual.update(|_, cx| {
            history.update(cx, |history, cx| {
                history.layout = HistoryLayout::Grid;
                history.selected_index = 0;
                history.scroll.scroll_to_item(0);
                cx.notify();
            });
        });

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
        for mode in [
            crate::theme::presets::ThemeMode::Light,
            crate::theme::presets::ThemeMode::Dark,
        ] {
            visual.update(|_, cx| crate::theme::vars::update_theme(cx, mode, "default"));
            for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
                visual.simulate_scale_factor_change(scale);
                visual.simulate_resize(size(px(400.0), px(364.0)));
                visual.update(|_, cx| {
                    history.update(cx, |history, cx| {
                        history.selected_index = 0;
                        cx.notify();
                    });
                });
                let baseline_label = visual.expect("history-row-label-second");
                visual.update(|_, cx| {
                    history.update(cx, |history, cx| {
                        history.selected_index = 1;
                        cx.notify();
                    });
                });
                let full_label = visual.expect("history-row-label-second");
                assert_eq!(visual.update(|window, _| window.scale_factor()), scale);
                assert_eq!(
                    full_label.size.width, baseline_label.size.width,
                    "width 400, scale {scale}: Screenshot label {full_label:?} must match its untruncated baseline {baseline_label:?}"
                );
                for width in [140.0, 148.0, 200.0, 240.0, 280.0, 300.0, 319.0, 320.0] {
                    visual.simulate_resize(size(px(width), px(364.0)));
                    let narrow_label = visual.expect("history-row-label-second");
                    assert_eq!(visual.update(|window, _| window.scale_factor()), scale);
                    assert!(
                        narrow_label.size.width >= baseline_label.size.width,
                        "width {width}, scale {scale}: narrow label {narrow_label:?} versus baseline {baseline_label:?}"
                    );
                    if width == 320.0 {
                        assert_eq!(
                            narrow_label.size.width, baseline_label.size.width,
                            "width 320, scale {scale}: Screenshot label {narrow_label:?} must match its untruncated baseline {baseline_label:?}"
                        );
                    }
                    if width < 320.0 {
                        let thumb = visual.expect("history-row-thumb-second");
                        let actions = visual.expect("history-row-actions-second");
                        assert!(actions.origin.x >= thumb.origin.x);
                        assert!(actions.origin.y >= thumb.origin.y);
                        assert!(
                            actions.origin.x + actions.size.width
                                <= thumb.origin.x + thumb.size.width
                        );
                        assert!(
                            actions.origin.y + actions.size.height
                                <= thumb.origin.y + thumb.size.height
                        );
                    }
                }
            }
        }
        visual.simulate_resize(size(px(200.0), px(364.0)));
        let selected = visual.expect("history-row-second");
        let scroller = visual.expect("history-scroll");
        assert!(selected.origin.x >= scroller.origin.x);
        assert!(selected.origin.x + selected.size.width <= scroller.origin.x + scroller.size.width);
        assert!(selected.origin.y >= scroller.origin.y);
        assert!(
            selected.origin.y + selected.size.height <= scroller.origin.y + scroller.size.height
        );
        visual.update(|_, cx| {
            history.update(cx, |history, cx| {
                history.items[1].r#type = HistoryItemType::Video;
                history.media.insert(
                    "second".into(),
                    ItemMedia {
                        thumbnail: None,
                        features: RecordingFeatures {
                            has_mic: true,
                            has_system_audio: true,
                            has_camera: true,
                            has_cursor: true,
                        },
                    },
                );
                cx.notify();
            });
        });
        let selected = visual.expect("history-row-second");
        let features = visual.expect("history-row-features-second");
        let scroller = visual.expect("history-scroll");
        assert!(
            features.origin.x + features.size.width
                <= selected.origin.x + selected.size.width - px(8.0)
        );
        assert!(
            features.origin.y + features.size.height
                <= selected.origin.y + selected.size.height - px(8.0)
        );
        assert!(selected.origin.y >= scroller.origin.y);
        assert!(
            selected.origin.y + selected.size.height <= scroller.origin.y + scroller.size.height
        );
    }
}
