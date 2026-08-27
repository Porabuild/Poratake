//! Settings window — port of `renderer/windows/settings-window.tsx`: a
//! searchable sidebar over the shared registry, section-grouped category
//! pages, and the About page that carries the AGPL notices.

pub mod about;
pub mod item_row;
pub mod registry;
pub mod shortcut_items;

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, AnyElement, Context, Entity, FocusHandle, Render, SharedString, Styled,
    Window,
};

use crate::config::schema::SettingsConfig;
use crate::config::store::ConfigStore;
use crate::theme::presets::{resolve_theme_mode, AppThemePreset, ThemeMode, APP_THEME_PRESETS};
use crate::theme::vars::{active_theme, update_theme, ThemeVars};
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::menu::MenuHandle;
use crate::ui::text_field::TextField;
use crate::windows::settings::registry::{Category, Item, PathKind};

const SIDEBAR_WIDTH: f32 = chrome::SETTINGS_SIDEBAR_WIDTH;
const CONTENT_MAX_WIDTH: f32 = chrome::SETTINGS_CONTENT_MAX;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CloudTest {
    Idle,
    Running,
    Passed,
    Failed,
}

/// The rendered shape of the Test Connection row.
pub struct CloudTestStatus {
    pub icon: Option<&'static str>,
    pub spinning: bool,
    pub disabled: bool,
    pub unconfigured: bool,
    pub message: Option<(String, gpui::Hsla)>,
}

pub struct SettingsWindow {
    store: Arc<ConfigStore>,
    config: SettingsConfig,
    active: Category,
    search: Entity<TextField>,
    shortcut_search: Entity<TextField>,
    fields: HashMap<String, Entity<TextField>>,
    recording_shortcut: Option<&'static str>,
    cloud_test: CloudTest,
    cloud_test_error: Option<String>,
    devices: Option<crate::system::devices::MediaDeviceLists>,
    menu: MenuHandle,
    focus_handle: FocusHandle,
    /// The update check's state, shared with the thread that performs it.
    update: crate::update::Shared,
    /// Which "beyond Electron" disclosures the user has opened this session.
    /// Collapsed by default so each page reads exactly like its Electron
    /// counterpart until the extras are asked for.
    extras_open: std::collections::HashSet<&'static str>,
}

impl SettingsWindow {
    pub fn new(store: Arc<ConfigStore>, active: Category, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            TextField::new("", cx)
                .placeholder("Search settings")
                .leading_icon("search")
                .full_width(true)
                .bare()
        });
        let shortcut_search = cx.new(|cx| {
            TextField::new("", cx)
                .placeholder("Search shortcuts")
                .leading_icon("search")
                .full_width(true)
                .height(px(SHORTCUT_SEARCH_HEIGHT))
                .pad_x(px(SHORTCUT_SEARCH_PAD_X))
        });
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        cx.observe(&shortcut_search, |_, _, cx| cx.notify())
            .detach();

        Self {
            config: store.get(),
            store,
            active,
            search,
            shortcut_search,
            fields: HashMap::new(),
            recording_shortcut: None,
            cloud_test: CloudTest::Idle,
            cloud_test_error: None,
            devices: None,
            menu: MenuHandle::new(),
            update: std::sync::Arc::new(std::sync::Mutex::new(crate::update::Status::Idle)),
            extras_open: std::collections::HashSet::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn config(&self) -> &SettingsConfig {
        &self.config
    }

    /// The DOM keeps the label fixed at "Test Connection" and expresses the
    /// outcome through a leading icon plus a coloured message beside it, so the
    /// row needs the whole status rather than a label.
    pub fn cloud_test_status(&self) -> CloudTestStatus {
        let complete = crate::cloud::provider_fields_complete(&self.config.cloud);
        let running = matches!(self.cloud_test, CloudTest::Running);
        CloudTestStatus {
            icon: match self.cloud_test {
                CloudTest::Idle => None,
                CloudTest::Running => Some("loader-2"),
                CloudTest::Passed => Some("check-circle"),
                CloudTest::Failed => Some("x-circle"),
            },
            spinning: running,
            disabled: !complete || running,
            unconfigured: !complete,
            message: match self.cloud_test {
                CloudTest::Passed => Some((
                    "Connection successful!".to_string(),
                    crate::ui::colors::green_500(1.0),
                )),
                CloudTest::Failed => Some((
                    self.cloud_test_error
                        .clone()
                        .unwrap_or_else(|| "Connection failed".to_string()),
                    crate::ui::colors::red_500(1.0),
                )),
                _ => None,
            },
        }
    }

    pub fn mutate(&mut self, cx: &mut Context<Self>, apply: impl FnOnce(&mut SettingsConfig)) {
        let before_theme = (
            self.config.appearance.mode.clone(),
            self.config.appearance.theme.clone(),
        );
        let before_editor_shortcuts = self.config.shortcuts.editor.clone();
        apply(&mut self.config);
        let snapshot = self.config.clone();
        self.store.update(move |config| *config = snapshot);
        crate::intents::refresh_shell(cx);

        // The editor keymap is installed once; a rebound tool key has to
        // replace it or the old key would keep working.
        if before_editor_shortcuts != self.config.shortcuts.editor {
            cx.clear_key_bindings();
            crate::editor::actions::init_bindings(cx);
            crate::capture::overlay::init_bindings(cx);
        }

        let after_theme = (
            self.config.appearance.mode.clone(),
            self.config.appearance.theme.clone(),
        );
        if before_theme != after_theme {
            update_theme(
                cx,
                resolve_theme_mode(ThemeMode::parse(&after_theme.0)),
                &after_theme.1,
            );
        }
        cx.notify();
    }

    pub fn path_for(&self, kind: PathKind) -> String {
        match kind {
            PathKind::Screenshots => self.config.storage.screenshots_path.clone(),
            PathKind::Recordings => self.config.storage.recordings_path.clone(),
        }
    }

    pub fn pick_path(&mut self, kind: PathKind, cx: &mut Context<Self>) {
        let title = match kind {
            PathKind::Screenshots => "Choose screenshots folder",
            PathKind::Recordings => "Choose recordings folder",
        };
        let Some(folder) = rfd::FileDialog::new().set_title(title).pick_folder() else {
            return;
        };
        let path = folder.to_string_lossy().to_string();
        self.mutate(cx, move |config| match kind {
            PathKind::Screenshots => config.storage.screenshots_path = path,
            PathKind::Recordings => config.storage.recordings_path = path,
        });
    }

    /// `handleReset` writes an empty path, which is what makes the folder fall
    /// back to the default.
    pub fn reset_path(&mut self, kind: PathKind, cx: &mut Context<Self>) {
        self.mutate(cx, move |config| match kind {
            PathKind::Screenshots => config.storage.screenshots_path.clear(),
            PathKind::Recordings => config.storage.recordings_path.clear(),
        });
    }

    /// `handleReset` writes the default pattern back and clears the error.
    pub fn reset_naming_pattern(&mut self, cx: &mut Context<Self>) {
        let default = crate::config::schema::default_naming_pattern();
        let value = default.clone();
        self.mutate(cx, move |config| {
            config.storage.naming_pattern = default;
        });
        if let Some(field) = self.fields.get("storage.namingPattern").cloned() {
            field.update(cx, |field, cx| field.set_value(&value, cx));
        }
    }

    /// `handleToggleTest` in `microphone-device-setting.tsx`: start the test for
    /// the selected device, or stop the running one.
    pub fn toggle_mic_test(&mut self, cx: &mut Context<Self>) {
        let daemon = crate::state::state(cx).daemon.clone();
        if crate::system::device_test::mic_active() {
            crate::system::device_test::stop_mic_test(&daemon);
        } else {
            let id = self.config.recording.selected_mic_id.clone();
            let name = self.config.recording.selected_mic_name.clone();
            crate::system::device_test::start_mic_test(&daemon, id.as_deref(), name.as_deref());
        }
        cx.notify();
    }

    pub fn toggle_camera_test(&mut self, cx: &mut Context<Self>) {
        let daemon = crate::state::state(cx).daemon.clone();
        if crate::system::device_test::camera_active() {
            crate::system::device_test::stop_camera_test(&daemon);
        } else {
            let camera = &self.config.recording.camera;
            crate::system::device_test::start_camera_test(
                &daemon,
                camera.selected_device_id.as_deref(),
                camera.selected_device_name.as_deref(),
                camera.flipped,
            );
        }
        cx.notify();
    }

    pub fn update_status(&self) -> crate::update::Status {
        self.update
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default()
    }

    /// `handleCheckForUpdates`. The request happens off the UI thread; the
    /// result is published into the shared cell and the window redrawn.
    pub fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if let Ok(mut status) = self.update.lock() {
            if *status == crate::update::Status::Checking {
                return;
            }
            *status = crate::update::Status::Checking;
        }
        cx.notify();

        let shared = self.update.clone();
        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::update::check(crate::product::VERSION) })
                .await;
            if let Ok(mut status) = shared.lock() {
                *status = result;
            }
            let _ = entity.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    pub fn append_naming_token(&mut self, token: &'static str, cx: &mut Context<Self>) {
        self.mutate(cx, |config| {
            config.storage.naming_pattern.push_str(token);
        });
        let value = self.config.storage.naming_pattern.clone();
        if let Some(field) = self.fields.get("storage.namingPattern").cloned() {
            field.update(cx, |field, cx| field.set_value(&value, cx));
        }
    }

    pub fn test_cloud_connection(&mut self, cx: &mut Context<Self>) {
        let cloud = self.config.cloud.clone();
        self.cloud_test = CloudTest::Running;
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::cloud::test_connection(&cloud) })
                .await;
            let failure = match &result {
                Ok(()) => None,
                Err(error) => {
                    eprintln!("[cloud] test connection failed: {error}");
                    Some(error.to_string())
                }
            };
            let _ = entity.update(cx, |this, cx| {
                this.cloud_test = match &failure {
                    None => CloudTest::Passed,
                    Some(_) => CloudTest::Failed,
                };
                this.cloud_test_error = failure;
                cx.notify();
            });
            // The renderer clears the outcome after 3s.
            cx.background_executor()
                .timer(std::time::Duration::from_secs(3))
                .await;
            let _ = entity.update(cx, |this, cx| {
                this.cloud_test = CloudTest::Idle;
                this.cloud_test_error = None;
                cx.notify();
            });
        })
        .detach();
    }

    /// Whether `id` is the field currently capturing keys. Rows ask through
    /// this rather than reading the entity back out of the context, which
    /// panics mid-render.
    pub fn is_recording_shortcut(&self, id: &'static str) -> bool {
        self.recording_shortcut == Some(id)
    }

    pub fn stop_recording_shortcut(&mut self, cx: &mut Context<Self>) {
        self.recording_shortcut = None;
        cx.notify();
    }

    fn write_recorded_shortcut(&mut self, value: String, cx: &mut Context<Self>) {
        let Some(id) = self.recording_shortcut else {
            return;
        };
        let items = registry::items();
        let Some(item) = items.iter().find(|item| item.id == id) else {
            return;
        };
        let registry::Control::Shortcut { set, .. } = item.control else {
            return;
        };
        self.mutate(cx, move |config| set(config, &value));
        self.stop_recording_shortcut(cx);
    }

    fn on_key(&mut self, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let control = event.keystroke.modifiers.control || event.keystroke.modifiers.platform;

        if let Some(recording) = self.recording_shortcut {
            let single_key = registry::items()
                .iter()
                .find(|item| item.id == recording)
                .is_some_and(|item| {
                    matches!(
                        item.control,
                        registry::Control::Shortcut {
                            single_key: true,
                            ..
                        }
                    )
                });
            match key {
                "escape" => self.stop_recording_shortcut(cx),
                "backspace" => self.write_recorded_shortcut(String::new(), cx),
                _ => {
                    if let Some(combination) =
                        crate::ui::shortcut_input::combination_from(event, single_key)
                    {
                        self.write_recorded_shortcut(combination, cx);
                    }
                }
            }
            cx.stop_propagation();
            return;
        }

        if control && key == "f" {
            window.focus(&self.search.read(cx).focus_handle());
            cx.stop_propagation();
        }
    }

    /// The daemon enumerates devices lazily: the Devices page asks for them
    /// the first time it renders so opening Settings never triggers a
    /// permission prompt on its own.
    pub(crate) fn device_lists(
        &mut self,
        cx: &mut Context<Self>,
    ) -> crate::system::devices::MediaDeviceLists {
        if let Some(devices) = &self.devices {
            return devices.clone();
        }
        self.devices = Some(crate::system::devices::MediaDeviceLists::default());

        let daemon = crate::state::state(cx).daemon;
        cx.spawn(async move |entity, cx| {
            let listed = cx
                .background_executor()
                .spawn(async move {
                    crate::system::devices::list(
                        &daemon,
                        &[
                            crate::system::devices::DeviceKind::Microphone,
                            crate::system::devices::DeviceKind::Camera,
                        ],
                    )
                })
                .await;
            let _ = entity.update(cx, |this, cx| {
                this.devices = Some(listed);
                cx.notify();
            });
        })
        .detach();

        crate::system::devices::MediaDeviceLists::default()
    }

    pub fn select_category(&mut self, category: Category, cx: &mut Context<Self>) {
        self.active = category;
        self.recording_shortcut = None;
        self.search.update(cx, |field, cx| field.set_value("", cx));
        cx.notify();
    }

    /// Text fields keep their own caret and selection, so they are created
    /// once per key and reused across renders.
    fn text_field(
        &mut self,
        key: String,
        initial: String,
        placeholder: &'static str,
        secret: bool,
        cx: &mut Context<Self>,
        write: impl Fn(&mut SettingsConfig, &str) + 'static,
    ) -> Entity<TextField> {
        if let Some(existing) = self.fields.get(&key) {
            let existing = existing.clone();
            existing.update(cx, |field, cx| field.set_value(&initial, cx));
            return existing;
        }
        let owner = cx.entity().downgrade();
        let field = cx.new(|cx| {
            TextField::new(initial, cx)
                .placeholder(placeholder)
                .secret(secret)
                .full_width(true)
                .on_change(move |value, _window, app| {
                    let value = value.to_string();
                    if let Some(owner) = owner.upgrade() {
                        owner.update(app, |this, cx| {
                            this.mutate(cx, |config| write(config, &value));
                        });
                    }
                })
        });
        self.fields.insert(key, field.clone());
        field
    }

    pub(crate) fn text_field_for(
        &mut self,
        item: &Item,
        cx: &mut Context<Self>,
    ) -> Entity<TextField> {
        let registry::Control::Input {
            placeholder,
            secret,
            get,
            set,
            ..
        } = &item.control
        else {
            return self.text_field(item.id.to_string(), String::new(), "", false, cx, |_, _| {});
        };
        let (placeholder, secret, set) = (*placeholder, *secret, *set);
        let initial = get(&self.config);
        self.text_field(item.id.to_string(), initial, placeholder, secret, cx, set)
    }

    pub(crate) fn naming_pattern_field(&mut self, cx: &mut Context<Self>) -> Entity<TextField> {
        let initial = self.config.storage.naming_pattern.clone();
        self.text_field(
            "storage.namingPattern".to_string(),
            initial,
            "%type %Y-%m-%d at %H.%M.%S",
            false,
            cx,
            |config, value| config.storage.naming_pattern = value.to_string(),
        )
    }

    pub(crate) fn rest_header_fields(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> (Entity<TextField>, Entity<TextField>) {
        let header = self
            .config
            .cloud
            .rest
            .headers
            .get(index)
            .cloned()
            .unwrap_or_default();
        let key_field = self.text_field(
            format!("cloud.rest.headers.{index}.key"),
            header.key,
            "Header",
            false,
            cx,
            move |config, value| {
                if let Some(entry) = config.cloud.rest.headers.get_mut(index) {
                    entry.key = value.to_string();
                }
            },
        );
        let value_field = self.text_field(
            format!("cloud.rest.headers.{index}.value"),
            header.value,
            "Value",
            false,
            cx,
            move |config, value| {
                if let Some(entry) = config.cloud.rest.headers.get_mut(index) {
                    entry.value = value.to_string();
                }
            },
        );
        (key_field, value_field)
    }

    pub(crate) fn add_rest_header(&mut self, cx: &mut Context<Self>) {
        self.mutate(cx, |config| {
            config
                .cloud
                .rest
                .headers
                .push(crate::config::schema::RestHeader::default());
        });
    }

    pub(crate) fn remove_rest_header(&mut self, index: usize, cx: &mut Context<Self>) {
        self.fields
            .remove(&format!("cloud.rest.headers.{index}.key"));
        self.fields
            .remove(&format!("cloud.rest.headers.{index}.value"));
        self.mutate(cx, move |config| {
            if index < config.cloud.rest.headers.len() {
                config.cloud.rest.headers.remove(index);
            }
        });
    }

    fn search_query(&self, cx: &Context<Self>) -> String {
        self.search.read(cx).value().trim().to_string()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let query = self.search_query(cx);
        let searching = !query.is_empty();

        // The level meter reads a value the daemon's reader thread writes, so
        // while the mic test runs the window has to keep drawing -- nothing else
        // would notify it.
        if crate::system::device_test::mic_active() {
            window.request_animation_frame();
        }

        let content: AnyElement = if searching {
            search_results(&query, self, &theme, cx)
        } else if self.active == Category::About {
            about::render(&theme, self.update_status(), cx)
        } else {
            category_page(self.active, self, &theme, window, cx)
        };

        div()
            .id("settings-window")
            .key_context("Settings")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .flex()
            .flex_row()
            .size_full()
            .bg(theme.content_background)
            .text_color(theme.foreground)
            .child(sidebar(self, &theme, window, cx))
            .child(
                div()
                    .id("settings-content")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .bg(theme.content_background)
                    .child(crate::ui::window_controls::drag_strip(
                        theme.content_background,
                        window,
                        &theme,
                    ))
                    .child(
                        div()
                            .id("settings-content-body")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .px(px(chrome::SETTINGS_CONTENT_PAD_X))
                            .pt(px(chrome::SETTINGS_CONTENT_PAD_TOP))
                            .pb(px(chrome::SETTINGS_CONTENT_PAD_BOTTOM))
                            // `mx-auto max-w-[720px]` centres the column.
                            .child(
                                // `min_w_0` lets the column shrink to the slot.
                                // Without it a child that measures wider than
                                // the content area is centred *over* the
                                // sidebar and clipped on the left, which is what
                                // the About page did.
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_center()
                                    .w_full()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .w_full()
                                            .min_w_0()
                                            .max_w(px(CONTENT_MAX_WIDTH))
                                            .child(content),
                                    ),
                            ),
                    ),
            )
    }
}

fn sidebar(
    window: &mut SettingsWindow,
    theme: &ThemeVars,
    ui_window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let searching = !window.search_query(cx).is_empty();
    let active = window.active;
    let categories = Category::supported();

    let mut entry = |category: Category, cx: &mut Context<SettingsWindow>| {
        let selected = !searching && active == category;
        let focus = crate::ui::primitives::control_focus(
            &format!("settings-nav-{}", category.id()),
            false,
            ui_window,
            cx,
        );
        div()
            .id(SharedString::from(format!(
                "settings-nav-{}",
                category.id()
            )))
            .track_focus(&focus)
            .focus(|style| style.shadow(crate::ui::primitives::focus_ring(theme, 2.0)))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(chrome::SETTINGS_NAV_GAP))
            .w_full()
            .rounded(px(chrome::SETTINGS_NAV_RADIUS))
            .px(px(chrome::SETTINGS_NAV_PX))
            .py(px(chrome::SETTINGS_NAV_PY))
            .text_size(px(14.0))
            .text_color(if selected {
                theme.foreground
            } else {
                theme.muted_foreground
            })
            .when(selected, |el| el.bg(theme.row_active))
            .hover(move |style: gpui::StyleRefinement| style.bg(theme.row_hover))
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.select_category(category, cx);
            }))
            .child(icon_element(category.icon(), px(16.0)))
            .child(category.label())
    };

    let mut nav = div()
        .id("settings-nav")
        .flex()
        .flex_col()
        .gap(px(2.0))
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .px(px(8.0));
    for category in categories
        .iter()
        .copied()
        .filter(|category| category.searchable())
    {
        nav = nav.child(entry(category, cx));
    }

    // The footer group is `space-y-1`, unlike the `space-y-0.5` nav above it.
    let mut footer = div().flex().flex_col().gap(px(4.0)).px(px(8.0));
    for category in categories.iter().copied().filter(|c| !c.searchable()) {
        footer = footer.child(entry(category, cx));
    }

    div()
        .flex()
        .flex_col()
        .w(px(SIDEBAR_WIDTH))
        .flex_shrink_0()
        .h_full()
        // `.poratake-settings-sidebar` is a vertical wash from 88% to 72% of
        // `--sidebar-background` with a right hairline. Its `backdrop-filter`
        // has no gpui equivalent, so the wash sits on the content background
        // the blur would have sampled.
        .bg(gpui::linear_gradient(
            180.0,
            gpui::linear_color_stop(
                crate::theme::color::mix_hsla(
                    theme.sidebar_background,
                    88.0,
                    theme.content_background,
                ),
                0.0,
            ),
            gpui::linear_color_stop(
                crate::theme::color::mix_hsla(
                    theme.sidebar_background,
                    72.0,
                    theme.content_background,
                ),
                1.0,
            ),
        ))
        .border_r_1()
        .border_color(theme.hairline)
        .child(
            div()
                .flex()
                .items_center()
                .h(px(40.0))
                .flex_shrink_0()
                .px(px(16.0))
                .text_size(px(chrome::TEXT_XS))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.muted_foreground)
                .child(crate::ui::primitives::tracked_text(
                    "SETTINGS",
                    px(chrome::SETTINGS_TITLE_TRACKING),
                )),
        )
        // `<label className="flex cursor-text items-center gap-2 rounded-3xl
        // px-2 py-1.5 text-muted-foreground focus-within:bg-[var(--row-active)]
        // hover:bg-[var(--row-hover)]">` around a transparent input.
        .child(
            div().px(px(8.0)).pb(px(8.0)).child(
                div()
                    .id("settings-search-shell")
                    .flex()
                    .flex_row()
                    .items_center()
                    .rounded(px(chrome::RADIUS_3XL))
                    .px(px(8.0))
                    .py(px(6.0))
                    .text_color(theme.muted_foreground)
                    .when(searching, |el| el.bg(theme.row_active))
                    .hover(move |style: gpui::StyleRefinement| style.bg(theme.row_hover))
                    .child(window.search.clone()),
            ),
        )
        .child(nav)
        .child(
            div()
                .pt(px(8.0))
                .pb(px(8.0))
                // The `<Separator className="mb-2" />` spans the full sidebar
                // width; only the button groups are inset.
                .child(div().w_full().mb(px(8.0)).h(px(1.0)).bg(theme.separator))
                .child(footer),
        )
        .into_any_element()
}

/// `<h2 className="text-xs font-medium text-muted-foreground">`, rendered only
/// in the shortcuts category.
fn section_heading(title: &str, theme: &ThemeVars) -> AnyElement {
    div()
        .text_size(px(chrome::TEXT_XS))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.muted_foreground)
        .child(title.to_string())
        .into_any_element()
}

/// `<label className="h-8 w-64 rounded-field border-0 bg-field px-2.5">`.
const SHORTCUT_SEARCH_HEIGHT: f32 = 32.0;
const SHORTCUT_SEARCH_PAD_X: f32 = 10.0;
const SHORTCUT_SEARCH_WIDTH: f32 = 256.0;

/// Gap between sections: `space-y-6`, or `space-y-4` for shortcuts.
const SECTION_GAP: f32 = 24.0;
const SECTION_GAP_COMPACT: f32 = 16.0;
/// `groupBySection` in `settings-registry.ts` collects items into a `Map` keyed
/// by section name, so an item belongs to its section's *first* appearance no
/// matter where it sits in the registry -- `general.rememberAllInOne` is
/// declared between two `Preview` rows and still renders after all of them.
/// Grouping only consecutive runs instead would reorder the page and put a
/// section gap in the middle of a section.
fn group_by_section(
    items: &[registry::Item],
    visible: Vec<usize>,
) -> Vec<(&'static str, Vec<usize>)> {
    let mut sections: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for index in visible {
        let section = items[index].section;
        match sections.iter_mut().find(|(name, _)| *name == section) {
            Some((_, group)) => group.push(index),
            None => sections.push((section, vec![index])),
        }
    }
    sections
}

/// Gap between the items inside one section: `space-y-4`, or `space-y-1` for
/// shortcuts.
const ITEM_GAP: f32 = 16.0;
const ITEM_GAP_COMPACT: f32 = 4.0;

fn category_page(
    category: Category,
    window: &mut SettingsWindow,
    theme: &ThemeVars,
    ui_window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let is_shortcuts = category == Category::Shortcuts;
    let filter = if is_shortcuts {
        window.shortcut_search.read(cx).value().trim().to_string()
    } else {
        String::new()
    };

    let mut header = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(chrome::SETTINGS_HEADING_GAP))
        .pb(px(chrome::SETTINGS_HEADING_GAP))
        .child(
            div()
                .text_size(px(chrome::SETTINGS_HEADING_SIZE))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(category.label()),
        );
    if is_shortcuts {
        // `<label className="h-8 w-64 ...">`.
        header = header.child(
            div()
                .w(px(SHORTCUT_SEARCH_WIDTH))
                .child(window.shortcut_search.clone()),
        );
    }

    let mut page = div().flex().flex_col().child(header);

    let items = registry::items();
    let visible: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.category == category)
        .filter(|(_, item)| item.is_visible(window.config()))
        .filter(|(_, item)| filter.is_empty() || item.matches(&filter))
        // Rows this shell adds are held back for the disclosure below, so the
        // page above it is exactly the set Electron renders.
        .filter(|(_, item)| !EXTRA_ITEM_IDS.contains(&item.id))
        .map(|(index, _)| index)
        .collect();

    let rendered = visible.len();
    let sections = group_by_section(&items, visible);

    let mut stack = div().flex().flex_col().gap(px(if is_shortcuts {
        SECTION_GAP_COMPACT
    } else {
        SECTION_GAP
    }));
    for (section, group) in sections {
        let mut block = div().flex().flex_col().gap(px(if is_shortcuts {
            ITEM_GAP_COMPACT
        } else {
            ITEM_GAP
        }));
        if is_shortcuts {
            block = block.child(section_heading(section, theme));
        }
        for index in group {
            let item = &items[index];
            block = block.child(window.render_setting(item, theme, is_shortcuts, cx));
        }
        stack = stack.child(block);
    }
    page = page.child(stack);
    page = extras(page, window, category, &filter, theme, ui_window, cx);

    if rendered == 0 && is_shortcuts && !filter.is_empty() {
        page = page.child(
            div()
                .py(px(48.0))
                .w_full()
                .text_align(gpui::TextAlign::Center)
                .text_size(px(13.0))
                .text_color(theme.muted_foreground)
                .child(format!("No shortcuts found for \"{filter}\"")),
        );
    }

    page.into_any_element()
}

/// Port of `settings-search-results.tsx`: a title, a result count, then one
/// `space-y-4` section per category under an uppercase heading, with
/// `space-y-7` between the sections.
fn search_results(
    query: &str,
    window: &mut SettingsWindow,
    theme: &ThemeVars,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    const RESULT_SECTION_GAP: f32 = 28.0;

    let items = registry::items();
    let matches: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.is_visible(window.config()))
        .filter(|(_, item)| item.matches(query))
        .map(|(index, _)| index)
        .collect();

    let header = div()
        .flex()
        .flex_col()
        .child(
            div()
                .pb(px(8.0))
                .text_size(px(chrome::SETTINGS_HEADING_SIZE))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child("Search results"),
        )
        .child(
            div()
                .pb(px(24.0))
                .text_size(px(chrome::TEXT_XS))
                .text_color(theme.muted_foreground)
                .child(if matches.len() == 1 {
                    "1 result".to_string()
                } else {
                    format!("{} results", matches.len())
                }),
        );

    if matches.is_empty() {
        return div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .py(px(64.0))
            .w_full()
            .child(
                div()
                    .text_size(px(chrome::TEXT_SM))
                    .text_color(theme.muted_foreground)
                    .child(format!("No settings found for \"{query}\"")),
            )
            .into_any_element();
    }

    let mut grouped: Vec<(Category, Vec<usize>)> = Vec::new();
    for category in Category::supported() {
        let group: Vec<usize> = matches
            .iter()
            .copied()
            .filter(|index| items[*index].category == category)
            .collect();
        if !group.is_empty() {
            grouped.push((category, group));
        }
    }

    let mut stack = div().flex().flex_col().gap(px(RESULT_SECTION_GAP));
    for (category, group) in grouped {
        let mut block = div().flex().flex_col().gap(px(ITEM_GAP)).child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .text_size(px(chrome::TEXT_XS))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.muted_foreground)
                .child(icon_element(category.icon(), px(chrome::TOOL_BUTTON_ICON)))
                .child(category.label().to_uppercase()),
        );
        for index in group {
            let item = &items[index];
            block = block.child(window.render_setting(item, theme, false, cx));
        }
        stack = stack.child(block);
    }

    div()
        .flex()
        .flex_col()
        .child(header)
        .child(stack)
        .into_any_element()
}

/// Registry rows this shell adds that Electron has no equivalent for. They are
/// rendered under a collapsed disclosure rather than inline, so a page matches
/// Electron until the extras are expanded.
pub(crate) const EXTRA_ITEM_IDS: &[&str] = &["shortcuts.scrollCapture"];

/// Everything this shell offers beyond Electron on a given page, behind one
/// collapsed disclosure per page.
fn extras(
    page: gpui::Div,
    window: &mut SettingsWindow,
    category: Category,
    filter: &str,
    theme: &ThemeVars,
    ui_window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> gpui::Div {
    let items = registry::items();
    let extra: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.category == category)
        .filter(|(_, item)| EXTRA_ITEM_IDS.contains(&item.id))
        .filter(|(_, item)| item.is_visible(window.config()))
        .filter(|(_, item)| filter.is_empty() || item.matches(filter))
        .map(|(index, _)| index)
        .collect();
    let has_theme_cards = category == Category::Appearance;
    if extra.is_empty() && !has_theme_cards {
        return page;
    }

    let key = category.id();
    let open = window.extras_open.contains(key);
    let mut block = div()
        .flex()
        .flex_col()
        .gap(px(ITEM_GAP))
        .pt(px(SECTION_GAP))
        .child(disclosure_header(key, open, theme, ui_window, cx));
    if open {
        if has_theme_cards {
            block = block.child(theme_cards(window, theme, ui_window, cx));
        }
        for index in extra {
            let item = &items[index];
            block = block.child(window.render_setting(
                item,
                theme,
                category == Category::Shortcuts,
                cx,
            ));
        }
    }
    page.child(block)
}

/// The disclosure's own row: a rotating chevron and a muted label, matching the
/// section headings on the shortcuts page rather than inventing a new style.
fn disclosure_header(
    key: &'static str,
    open: bool,
    theme: &ThemeVars,
    ui_window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let focus = crate::ui::primitives::control_focus(
        &format!("settings-extras-{key}"),
        false,
        ui_window,
        cx,
    );
    div()
        .id(SharedString::from(format!("settings-extras-{key}")))
        .track_focus(&focus)
        .focus(|style| style.shadow(crate::ui::primitives::focus_ring(theme, 2.0)))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .py(px(4.0))
        .text_size(px(chrome::TEXT_XS))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.muted_foreground)
        .hover(move |style: gpui::StyleRefinement| style.text_color(theme.foreground))
        .on_click(cx.listener(move |this, _event, _window, cx| {
            if !this.extras_open.remove(key) {
                this.extras_open.insert(key);
            }
            cx.notify();
        }))
        // Closed points right, open points down -- the usual disclosure.
        .children(
            crate::ui::icon::Icon::with_size("chevron-down", px(chrome::TEXT_XS))
                .map(|icon| icon.rotate_turns(if open { 0.0 } else { -0.25 })),
        )
        .child("More options")
        .into_any_element()
}

fn theme_cards(
    window: &SettingsWindow,
    theme: &ThemeVars,
    ui_window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let active_theme_id = window.config().appearance.theme.clone();
    let mut row = div().flex().flex_row().flex_wrap().gap(px(8.0)).pb(px(8.0));
    for preset in APP_THEME_PRESETS {
        row = row.child(theme_card(
            preset,
            active_theme_id == preset.id,
            theme,
            ui_window,
            cx,
        ));
    }
    row.into_any_element()
}

fn theme_card(
    preset: &'static AppThemePreset,
    active: bool,
    theme: &ThemeVars,
    ui_window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let id = format!("appearance-theme-{}", preset.id);
    let focus = crate::ui::primitives::control_focus(&id, false, ui_window, cx);
    div()
        .id(SharedString::from(id))
        .track_focus(&focus)
        .focus(|style| style.shadow(crate::ui::primitives::focus_ring(theme, 2.0)))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .w(px(156.0))
        .p(px(8.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(if active { theme.accent } else { theme.border })
        .bg(theme.card)
        .hover(move |style: gpui::StyleRefinement| style.bg(theme.default_hover))
        .on_click(cx.listener(move |this, _event, _window, cx| {
            this.mutate(cx, |config| {
                config.appearance.theme = preset.id.to_string();
            });
        }))
        .child(
            div()
                .flex()
                .gap(px(8.0))
                .child(swatch("Dark", preset.dark, theme))
                .child(swatch("Light", preset.light, theme)),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(preset.label),
        )
        .into_any_element()
}

fn swatch(
    label: &'static str,
    variant: crate::theme::presets::ThemeVariant,
    theme: &ThemeVars,
) -> AnyElement {
    use crate::theme::color::Srgba;

    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .flex_1()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(
            div()
                .flex()
                .gap(px(4.0))
                .child(
                    div()
                        .h(px(24.0))
                        .flex_1()
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(theme.border)
                        .bg(Srgba::parse(variant.bg).to_hsla()),
                )
                .child(
                    div()
                        .h(px(24.0))
                        .flex_1()
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(theme.border)
                        .bg(Srgba::parse(variant.accent).to_hsla()),
                ),
        )
        .into_any_element()
}

impl crate::ui::shortcut_input::ShortcutRecorder for SettingsWindow {
    fn start_recording_shortcut(
        &mut self,
        id: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.recording_shortcut = Some(id);
        window.focus(&self.focus_handle);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `groupBySection` keys a `Map` by section name, so a row whose section
    /// already appeared joins that earlier group rather than starting a new one.
    /// The General page depends on it: `Remember All-in-One choices` is declared
    /// between two `Preview` rows and has to render after all of them.
    #[test]
    fn a_section_is_one_group_no_matter_where_its_rows_are_declared() {
        let items = registry::items();
        let visible: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.category == registry::Category::General)
            .map(|(index, _)| index)
            .collect();

        let sections = group_by_section(&items, visible);
        let names: Vec<&str> = sections.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            vec!["Application", "Preview", "All-in-One", "History"],
            "sections come out in order of first appearance, each exactly once"
        );

        let order: Vec<&str> = sections
            .iter()
            .flat_map(|(_, group)| group.iter())
            .map(|index| items[*index].label)
            .collect();
        let all_in_one = order
            .iter()
            .position(|label| *label == "Remember All-in-One choices")
            .expect("the All-in-One row");
        let dismiss_after = order
            .iter()
            .position(|label| *label == "Dismiss after")
            .expect("the last Preview row");
        assert!(
            dismiss_after < all_in_one,
            "the whole Preview section renders before All-in-One, got {order:?}"
        );
    }
}

#[cfg(test)]
mod extras_tests {
    use super::*;

    /// `EXTRA_ITEM_IDS` is only correct while those rows really are absent from
    /// Electron. If one is ever added there it has to come back inline, so this
    /// reads Electron's registry rather than trusting the list.
    #[test]
    fn every_extra_row_exists_here_and_nowhere_in_electron() {
        let items = registry::items();
        let renderer = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root")
            .join("src/renderer/components/settings/registry");

        let mut electron = String::new();
        for entry in std::fs::read_dir(&renderer).expect("read the registry directory") {
            let path = entry.expect("directory entry").path();
            if path.extension().is_some_and(|ext| ext == "ts") {
                electron.push_str(&std::fs::read_to_string(&path).expect("read a registry file"));
            }
        }
        assert!(
            electron.contains("shortcuts."),
            "the Electron registry was not read"
        );

        assert!(!EXTRA_ITEM_IDS.is_empty());
        for id in EXTRA_ITEM_IDS {
            assert!(
                items.iter().any(|item| item.id == *id),
                "`{id}` is listed as an extra but is not in the registry"
            );
            assert!(
                !electron.contains(&format!("id: '{id}'")),
                "`{id}` now exists in Electron too, so it should render inline"
            );
        }
    }

    /// The expanded body is a separate render path -- it is not built at all
    /// while the disclosure is closed -- so it needs its own draw.
    #[gpui::test]
    fn the_expanded_disclosure_renders(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = std::sync::Arc::new(
            ConfigStore::load_at(dir.path().join("config.json")).expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));

        // Appearance holds the theme grid, Shortcuts holds the extra rows.
        for category in [Category::Appearance, Category::Shortcuts] {
            let window =
                cx.add_window(|_window, cx| SettingsWindow::new(store.clone(), category, cx));
            cx.refresh().expect("schedule a redraw");
            cx.run_until_parked();
            window
                .update(cx, |view, _window, cx| {
                    view.extras_open.insert(category.id());
                    cx.notify();
                })
                .expect("expand the disclosure");
            cx.refresh().expect("schedule a redraw");
            cx.run_until_parked();
        }
    }

    /// The disclosure starts closed, which is what makes the page above it match
    /// Electron on first open -- and the row is still registered, so nothing was
    /// removed to get there.
    #[gpui::test]
    fn the_disclosure_starts_closed(cx: &mut gpui::TestAppContext) {
        let items = registry::items();
        assert!(
            items
                .iter()
                .any(|item| item.id == "shortcuts.scrollCapture"
                    && item.category == Category::Shortcuts),
            "the scroll capture row is still registered, just held back"
        );

        let dir = tempfile::tempdir().expect("temp dir");
        let store = std::sync::Arc::new(
            ConfigStore::load_at(dir.path().join("config.json")).expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let window =
            cx.add_window(|_window, cx| SettingsWindow::new(store, Category::Shortcuts, cx));
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();

        let open = window
            .update(cx, |view, _window, _cx| view.extras_open.len())
            .expect("read the disclosure state");
        assert_eq!(open, 0, "no disclosure is expanded on a fresh window");
    }
}

impl SettingsWindow {
    /// `handleDownloadUpdate`: fetch the verified installer, reporting progress.
    pub fn download_update(&mut self, cx: &mut Context<Self>) {
        let crate::update::Status::Available {
            version,
            artifact,
            sha512,
        } = self.update_status()
        else {
            return;
        };

        if let Ok(mut status) = self.update.lock() {
            *status = crate::update::Status::Downloading {
                version: version.clone(),
                progress: 0.0,
            };
        }
        cx.notify();

        let shared = self.update.clone();
        let progress_cell = self.update.clone();
        cx.spawn(async move |entity, cx| {
            let version_for_progress = version.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::update::download(&artifact, &sha512, move |fraction| {
                        if let Ok(mut status) = progress_cell.lock() {
                            *status = crate::update::Status::Downloading {
                                version: version_for_progress.clone(),
                                progress: fraction,
                            };
                        }
                    })
                })
                .await;

            if let Ok(mut status) = shared.lock() {
                *status = match result {
                    Ok(installer) => crate::update::Status::Ready { version, installer },
                    Err(message) => crate::update::Status::Error { message },
                };
            }
            let _ = entity.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    /// `quitAndInstall`: hand over to the installer, then leave -- NSIS cannot
    /// replace a binary that is still running.
    pub fn install_update(&mut self, cx: &mut Context<Self>) {
        let crate::update::Status::Ready { installer, .. } = self.update_status() else {
            return;
        };
        match crate::update::launch_installer(&installer) {
            Ok(()) => cx.quit(),
            Err(message) => {
                if let Ok(mut status) = self.update.lock() {
                    *status = crate::update::Status::Error { message };
                }
                cx.notify();
            }
        }
    }
}
