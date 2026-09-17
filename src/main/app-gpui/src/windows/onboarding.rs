//! Onboarding window — port of `renderer/windows/onboarding-window.tsx`.

use std::time::Duration;

use gpui::{
    div, prelude::*, px, size, Animation, AnimationExt, App, Bounds, Context, ElementId,
    FocusHandle, KeyDownEvent, Render, ScrollHandle, SharedString, Styled, Window,
};
use herogpui::gpui;

use crate::config::store::ConfigStore;
use crate::theme::vars::{active_theme, ThemeVars};
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::shortcut_input::{self, ShortcutRecorder};
use crate::windows::registry::{self, WindowKind};
use herogpui::components::{Button, Variant};

/// `ONBOARDING_STEPS` for a non-macOS platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Welcome,
    DisableMacShortcuts,
    Shortcuts,
    Permissions,
}

const WINDOWS_STEPS: [Step; 2] = [Step::Welcome, Step::Shortcuts];
const MAC_STEPS: [Step; 4] = [
    Step::Welcome,
    Step::DisableMacShortcuts,
    Step::Shortcuts,
    Step::Permissions,
];

fn steps() -> &'static [Step] {
    if cfg!(target_os = "macos") {
        &MAC_STEPS
    } else {
        &WINDOWS_STEPS
    }
}

/// The shortcut fields the step offers, in the renderer's order.
const SHORTCUT_FIELDS: [(&str, &str); 3] = [
    ("onboarding-shortcut-area", "Capture Area"),
    ("onboarding-shortcut-window", "Capture Window"),
    ("onboarding-shortcut-screen", "Capture Full Screen"),
];

pub struct OnboardingWindow {
    store: std::sync::Arc<ConfigStore>,
    step: usize,
    recording_shortcut: Option<&'static str>,
    scroll: ScrollHandle,
    permission_polling: bool,
    focus_handle: FocusHandle,
}

/// One row of the macOS shortcut-conflict `<ol>`: a primary number badge with
/// the step text beside it, `<strong>` segments in bold.
fn shortcut_conflict_step(
    number: &'static str,
    segments: &[(&'static str, bool)],
    theme: &ThemeVars,
) -> gpui::AnyElement {
    let mut text = div().flex().flex_row().flex_wrap().flex_1();
    for (segment, strong) in segments {
        text = text.child(
            div()
                .when(*strong, |el| el.font_weight(gpui::FontWeight::BOLD))
                .child(*segment),
        );
    }
    div()
        .flex()
        .flex_row()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .size(px(16.0))
                .rounded_full()
                .bg(theme.primary)
                .text_color(theme.primary_foreground)
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(number),
        )
        .child(text)
        .into_any_element()
}

impl OnboardingWindow {
    pub fn should_show(store: &ConfigStore) -> bool {
        let config = store.get();
        !config.onboarding.completed && !config.onboarding.skipped
    }

    pub fn open(cx: &mut App, store: std::sync::Arc<ConfigStore>) {
        registry::open_or_activate(WindowKind::Onboarding, cx, |cx| {
            let bounds = Bounds::centered(
                None,
                size(
                    px(crate::ui::chrome::ONBOARDING_WINDOW_WIDTH),
                    px(crate::ui::chrome::ONBOARDING_WINDOW_HEIGHT),
                ),
                cx,
            );
            cx.open_window(
                crate::windows::app_window_options_with_lights(
                    bounds,
                    Some(size(
                        px(crate::ui::chrome::ONBOARDING_WINDOW_WIDTH),
                        px(crate::ui::chrome::ONBOARDING_WINDOW_HEIGHT),
                    )),
                    gpui::point(px(16.0), px(18.0)),
                ),
                |window, cx| {
                    let view = cx.new(|cx| Self {
                        store: store.clone(),
                        step: 0,
                        recording_shortcut: None,
                        scroll: ScrollHandle::new(),
                        permission_polling: false,
                        focus_handle: cx.focus_handle(),
                    });
                    let focus = view.read(cx).focus_handle.clone();
                    window.focus(&focus, cx);
                    view
                },
            )
            .ok()
            .map(|handle| handle.into())
        });
    }

    fn current(&self) -> Step {
        let steps = steps();
        steps[self.step.min(steps.len() - 1)]
    }

    fn is_last_step(&self) -> bool {
        self.step + 1 >= steps().len()
    }

    fn next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_last_step() {
            self.step += 1;
            if self.current() == Step::Permissions {
                self.start_permission_polling(window, cx);
            }
            cx.notify();
        }
    }

    fn start_permission_polling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.permission_polling {
            return;
        }
        self.permission_polling = true;
        cx.spawn_in(window, async move |entity, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let Ok(keep_polling) = entity.update_in(cx, |this, _window, cx| {
                let keep_polling = this.current() == Step::Permissions;
                this.permission_polling = keep_polling;
                if keep_polling {
                    cx.notify();
                }
                keep_polling
            }) else {
                break;
            };
            if !keep_polling {
                break;
            }
        })
        .detach();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        if self.step > 0 {
            self.step -= 1;
            cx.notify();
        }
    }

    fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.store
            .update(|config| config.onboarding.completed = true);
        window.remove_window();
        registry::close(WindowKind::Onboarding, cx);
        cx.notify();
    }

    fn skip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.store.update(|config| config.onboarding.skipped = true);
        window.remove_window();
        registry::close(WindowKind::Onboarding, cx);
        cx.notify();
    }

    fn shortcut_value(&self, id: &str) -> String {
        let shortcuts = self.store.get().shortcuts.screenshot;
        match id {
            "onboarding-shortcut-window" => shortcuts.window,
            "onboarding-shortcut-screen" => shortcuts.screen,
            _ => shortcuts.area,
        }
    }

    fn set_shortcut(&mut self, id: &'static str, value: String, cx: &mut Context<Self>) {
        self.store.update(|config| match id {
            "onboarding-shortcut-window" => config.shortcuts.screenshot.window = value.clone(),
            "onboarding-shortcut-screen" => config.shortcuts.screenshot.screen = value.clone(),
            _ => config.shortcuts.screenshot.area = value.clone(),
        });
        self.recording_shortcut = None;
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.recording_shortcut else {
            return;
        };
        match event.keystroke.key.as_str() {
            "escape" => {
                self.recording_shortcut = None;
                cx.notify();
            }
            "backspace" => self.set_shortcut(id, String::new(), cx),
            _ => {
                // The onboarding fields are global accelerators, so a bare key is refused.
                if let Some(combination) = shortcut_input::combination_from(event, false) {
                    self.set_shortcut(id, combination, cx);
                }
            }
        }
    }

    fn welcome_step(&self, theme: &ThemeVars) -> gpui::AnyElement {
        let (status_title, status_description) = if cfg!(target_os = "macos") {
            (
                "Lives in Your Menu Bar",
                "Poratake runs quietly in your menu bar. Click the icon to access all features or use keyboard shortcuts for quick captures.",
            )
        } else {
            (
                "Lives in Your System Tray",
                "Poratake runs quietly in your system tray. Click the icon to access all features or use keyboard shortcuts for quick captures.",
            )
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            // `<img src={appIcon} className="mx-auto mb-4 h-16 w-16
            // rounded-2xl">`. The tile is only a fallback for an icon that
            // failed to decode.
            .child(
                div().mb(px(16.0)).child(
                    crate::ui::app_icon::element(
                        px(chrome::ONBOARDING_ICON),
                        px(chrome::ONBOARDING_ICON_RADIUS),
                    )
                    .unwrap_or_else(|| {
                        div()
                            .size(px(chrome::ONBOARDING_ICON))
                            .rounded(px(chrome::ONBOARDING_ICON_RADIUS))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(theme.accent)
                            .text_color(theme.accent_foreground)
                            .child(icon_element("aperture", px(32.0)))
                            .into_any_element()
                    }),
                ),
            )
            .child(
                div()
                    .text_size(px(chrome::ONBOARDING_TITLE_SIZE))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Welcome to Poratake"),
            )
            .child(
                div()
                    .mt(px(8.0))
                    .text_size(px(chrome::ONBOARDING_BODY_SIZE))
                    .text_color(theme.muted_foreground)
                    .child("Your new screenshot tool"),
            )
            .child(
                div()
                    .mt(px(24.0))
                    .flex()
                    .flex_col()
                    .gap(px(chrome::ONBOARDING_CARD_GAP))
                    .w_full()
                    .child(feature_card(
                        "monitor",
                        status_title,
                        status_description,
                        crate::ui::colors::blue_500(1.0),
                        theme,
                    ))
                    .child(feature_card(
                        "keyboard",
                        "Powerful Shortcuts",
                        "Capture screenshots instantly with customizable keyboard shortcuts for area, window, and full screen captures.",
                        crate::ui::colors::purple_500(1.0),
                        theme,
                    )),
            )
            .into_any_element()
    }

    fn shortcuts_step(&self, theme: &ThemeVars, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut fields = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .rounded(px(chrome::ONBOARDING_CARD_RADIUS))
            .bg(theme.muted_background)
            .p(px(chrome::ONBOARDING_CARD_PAD));

        for (index, (id, label)) in SHORTCUT_FIELDS.into_iter().enumerate() {
            if index > 0 {
                // A bare `border-t` divider inside the `space-y-1` flow, so the
                // surrounding gap already spaces it.
                fields = fields.child(div().h(px(1.0)).w_full().bg(theme.border));
            }
            let value = self.shortcut_value(id);
            fields = fields.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(chrome::ONBOARDING_BODY_SIZE))
                            .child(label),
                    )
                    .child(shortcut_input::render(
                        id,
                        &value,
                        false,
                        self.recording_shortcut == Some(id),
                        cx,
                        move |this, next, cx| this.set_shortcut(id, next, cx),
                    )),
            );
        }

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mb(px(16.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_center()
                    .child(
                        div()
                            .mb(px(16.0))
                            .size(px(chrome::ONBOARDING_ICON))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(crate::ui::colors::green_500(0.1))
                            .text_color(crate::ui::colors::green_500(1.0))
                            .child(icon_element("keyboard", px(32.0))),
                    )
                    .child(
                        div()
                            .text_size(px(chrome::ONBOARDING_TITLE_SIZE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Set Your Shortcuts"),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(px(chrome::ONBOARDING_BODY_SIZE))
                            .text_color(theme.muted_foreground)
                            .child(
                                "Customize keyboard shortcuts for quick captures. Click to record a new shortcut.",
                            ),
                    ),
            )
            .child(fields)
            .child(
                div()
                    .mt(px(12.0))
                    .text_center()
                    .text_size(px(chrome::ONBOARDING_HINT_SIZE))
                    .text_color(theme.muted_foreground)
                    .child("You can change these anytime in Settings \u{2192} Shortcuts"),
            )
            .into_any_element()
    }

    fn disable_mac_shortcuts_step(&self, theme: &ThemeVars) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mb(px(16.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_center()
                    .child(
                        div()
                            .mb(px(16.0))
                            .size(px(chrome::ONBOARDING_ICON))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(crate::ui::colors::orange_500(0.1))
                            .text_color(crate::ui::colors::orange_500(1.0))
                            .child(icon_element("keyboard", px(32.0))),
                    )
                    .child(
                        div()
                            .text_size(px(chrome::ONBOARDING_TITLE_SIZE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Disable macOS Screenshot Shortcuts"),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(px(chrome::ONBOARDING_BODY_SIZE))
                            .text_color(theme.muted_foreground)
                            .child("To use Poratake's shortcuts, disable the default macOS screenshot shortcuts first."),
                    ),
            )
            .child(
                div()
                    .rounded(px(chrome::ONBOARDING_CARD_RADIUS))
                    .bg(theme.muted_background)
                    .p(px(chrome::ONBOARDING_CARD_PAD))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .text_size(px(chrome::ONBOARDING_HINT_SIZE))
                    .text_color(theme.muted_foreground)
                    .child(shortcut_conflict_step(
                        "1",
                        &[("Click the button below to open Keyboard Settings", false)],
                        theme,
                    ))
                    .child(shortcut_conflict_step(
                        "2",
                        &[
                            ("Select ", false),
                            ("Screenshots", true),
                            (" in the left sidebar", false),
                        ],
                        theme,
                    ))
                    .child(shortcut_conflict_step(
                        "3",
                        &[
                            ("Uncheck ", false),
                            ("all screenshot shortcuts", true),
                            (" (⌘⇧3, ⌘⇧4, ⌘⇧5)", false),
                        ],
                        theme,
                    )),
            )
            .child(
                div().mt(px(12.0)).child(
                    crate::ui::rows::icon_text_button(
                        "onboarding-open-keyboard-settings",
                        "Open Keyboard Settings",
                        "external-link",
                        16.0,
                        8.0,
                    )
                    .variant(Variant::Secondary)
                    .full_width(true)
                        .on_press(|_event, _window, _cx| {
                            crate::system::permissions::open_keyboard_shortcut_preferences();
                        }),
                ),
            )
            .child(
                div()
                    .mt(px(12.0))
                    .text_center()
                    .text_size(px(chrome::ONBOARDING_HINT_SIZE))
                    .text_color(theme.muted_foreground)
                    .child("You can skip this step, but Poratake's shortcuts may conflict with macOS defaults."),
            )
            .into_any_element()
    }

    fn permissions_step(&self, theme: &ThemeVars) -> gpui::AnyElement {
        let screen = crate::system::permissions::screen_recording_granted();
        let accessibility = crate::system::permissions::accessibility_granted();
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mb(px(16.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_center()
                    .child(
                        div()
                            .mb(px(16.0))
                            .size(px(chrome::ONBOARDING_ICON))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(theme.accent.opacity(0.1))
                            .text_color(theme.accent)
                            .child(icon_element("shield", px(32.0))),
                    )
                    .child(
                        div()
                            .text_size(px(chrome::ONBOARDING_TITLE_SIZE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Setup Permissions"),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(px(chrome::ONBOARDING_BODY_SIZE))
                            .text_color(theme.muted_foreground)
                            .child("Poratake needs these permissions to work properly"),
                    ),
            )
            .child(permission_card(
                "monitor",
                "Screen Recording",
                "Required to capture screenshots of your screen",
                screen,
                "onboarding-screen-recording",
                crate::system::permissions::open_screen_recording_preferences,
                theme,
            ))
            .child(div().h(px(12.0)))
            .child(permission_card(
                "accessibility",
                "Accessibility",
                "Required to hide desktop icons when capturing",
                accessibility,
                "onboarding-accessibility",
                crate::system::permissions::open_accessibility_preferences,
                theme,
            ))
            .into_any_element()
    }
}

fn feature_card(
    icon: &'static str,
    title: &'static str,
    description: &'static str,
    tint: gpui::Hsla,
    theme: &ThemeVars,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(12.0))
        .rounded(px(chrome::ONBOARDING_CARD_RADIUS))
        .bg(theme.muted_background)
        .p(px(chrome::ONBOARDING_CARD_PAD))
        .text_left()
        .child(
            div()
                .size(px(32.0))
                .flex_shrink_0()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(tint.opacity(0.1))
                .text_color(tint)
                .child(icon_element(icon, px(16.0))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(chrome::ONBOARDING_BODY_SIZE))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(title),
                )
                .child(
                    div()
                        .mt(px(2.0))
                        .text_size(px(chrome::ONBOARDING_HINT_SIZE))
                        .text_color(theme.muted_foreground)
                        .child(description),
                ),
        )
        .into_any_element()
}

fn permission_card(
    icon: &'static str,
    title: &'static str,
    description: &'static str,
    granted: bool,
    id: &'static str,
    open: fn(),
    theme: &ThemeVars,
) -> gpui::AnyElement {
    let status = if granted { "check" } else { "x" };
    let status_color = if granted {
        crate::ui::colors::green_500(1.0)
    } else {
        theme.destructive
    };
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(12.0))
        .rounded(px(chrome::ONBOARDING_CARD_RADIUS))
        .bg(theme.muted_background)
        .p(px(chrome::ONBOARDING_CARD_PAD))
        .child(
            div()
                .size(px(32.0))
                .flex_shrink_0()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.accent.opacity(0.1))
                .text_color(theme.accent)
                .child(icon_element(icon, px(16.0))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(chrome::ONBOARDING_BODY_SIZE))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(title),
                        )
                        .child(
                            div()
                                .size(px(20.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(status_color.opacity(0.2))
                                .text_color(status_color)
                                .child(icon_element(status, px(12.0))),
                        ),
                )
                .child(
                    div()
                        .mt(px(2.0))
                        .text_size(px(chrome::ONBOARDING_HINT_SIZE))
                        .text_color(theme.muted_foreground)
                        .child(description),
                )
                .when(!granted, |el| {
                    el.child(
                        div().mt(px(12.0)).child(
                            Button::new(id)
                                .variant(Variant::Secondary)
                                .label("Open System Preferences")
                                .on_press(move |_event, _window, _cx| open()),
                        ),
                    )
                }),
        )
        .into_any_element()
}

/// `StepIndicator` — one dot per step, filled behind the current one.
fn step_indicator(current: usize, theme: &ThemeVars) -> gpui::AnyElement {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(8.0));
    for position in 0..steps().len() {
        let color = if position == current {
            theme.accent
        } else if position < current {
            theme.accent.opacity(0.5)
        } else {
            theme.default
        };
        let phase = if position == current {
            "current"
        } else if position < current {
            "past"
        } else {
            "upcoming"
        };
        row = row.child(
            div()
                .id(SharedString::from(format!(
                    "onboarding-dot-{position}-{phase}"
                )))
                .size(px(chrome::ONBOARDING_DOT))
                .rounded_full()
                .bg(color)
                .with_animation(
                    ElementId::Name(format!("onboarding-dot-fade-{position}-{phase}").into()),
                    Animation::new(std::time::Duration::from_millis(
                        crate::ui::primitives::OVERLAY_ENTER_MS,
                    ))
                    .with_easing(crate::ui::primitives::ease_out()),
                    |dot, delta| dot.opacity(delta.max(0.01)),
                ),
        );
    }
    row.into_any_element()
}

impl Render for OnboardingWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let step = self.current();
        let content = match step {
            Step::Welcome => self.welcome_step(&theme),
            Step::DisableMacShortcuts => self.disable_mac_shortcuts_step(&theme),
            Step::Shortcuts => self.shortcuts_step(&theme, cx),
            Step::Permissions => self.permissions_step(&theme),
        };
        let all_permissions_granted = crate::system::permissions::screen_recording_granted()
            && crate::system::permissions::accessibility_granted();

        let mut actions = div().flex().flex_row().w_full().gap(px(8.0));
        if self.step > 0 {
            actions = actions.child(
                crate::ui::rows::icon_text_button(
                    "onboarding-back",
                    "Back",
                    "chevron-left",
                    chrome::TOOL_BUTTON_ICON,
                    4.0,
                )
                .variant(Variant::Ghost)
                .sx(|el| el.flex_1().w_0())
                .on_press(cx.listener(|this, _event, _window, cx| this.back(cx))),
            );
        }
        actions = actions.child(if self.is_last_step() {
            Button::new("onboarding-continue")
                .variant(Variant::Primary)
                .label("Get Started")
                .is_disabled(step == Step::Permissions && !all_permissions_granted)
                .sx(|el| el.flex_1().w_0())
                .on_press(cx.listener(|this, _event, window, cx| this.finish(window, cx)))
        } else {
            Button::new("onboarding-next")
                .variant(Variant::Tertiary)
                .label("Next")
                .content(|_| {
                    crate::ui::primitives::icon_label(
                        "chevron-right",
                        "Next".into(),
                        px(chrome::TOOL_BUTTON_ICON),
                        px(4.0),
                        true,
                    )
                })
                .sx(|el| el.flex_1().w_0())
                .on_press(cx.listener(|this, _event, window, cx| this.next(window, cx)))
        });

        div()
            .id("onboarding-window")
            .key_context("Onboarding")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(crate::ui::window_controls::drag_strip(
                theme.background,
                window,
                cx,
                &theme,
            ))
            .child(
                div()
                    .id("onboarding-body")
                    .track_scroll(&self.scroll)
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(24.0))
                    .child(div().flex_1().child(content))
                    .child(
                        div()
                            .mt(px(24.0))
                            .flex()
                            .flex_col()
                            .gap(px(16.0))
                            .child(step_indicator(self.step, &theme))
                            .child(actions)
                            .child(
                                Button::new("onboarding-skip")
                                    .variant(Variant::Ghost)
                                    .label("Skip for now")
                                    .full_width(true)
                                    .recipe("muted")
                                    .on_press(cx.listener(|this, _event, window, cx| {
                                        this.skip(window, cx)
                                    })),
                            ),
                    ),
            )
    }
}

impl ShortcutRecorder for OnboardingWindow {
    fn start_recording_shortcut(
        &mut self,
        id: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.recording_shortcut = Some(id);
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_windows_flow_is_welcome_then_shortcuts() {
        assert_eq!(WINDOWS_STEPS, [Step::Welcome, Step::Shortcuts]);
    }

    #[test]
    fn the_macos_flow_includes_shortcut_and_permission_setup() {
        assert_eq!(
            MAC_STEPS,
            [
                Step::Welcome,
                Step::DisableMacShortcuts,
                Step::Shortcuts,
                Step::Permissions
            ]
        );
    }

    #[test]
    fn every_shortcut_field_has_a_distinct_id() {
        let ids: Vec<&str> = SHORTCUT_FIELDS.iter().map(|(id, _)| *id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
    }

    #[test]
    fn onboarding_is_hidden_once_it_is_completed_or_skipped() {
        let path = std::env::temp_dir().join("poratake-onboarding-test.json");
        let _ = std::fs::remove_file(&path);
        let store = ConfigStore::load_at(path.clone()).expect("store");
        assert!(OnboardingWindow::should_show(&store));

        store.update(|config| config.onboarding.skipped = true);
        assert!(!OnboardingWindow::should_show(&store));

        store.update(|config| {
            config.onboarding.skipped = false;
            config.onboarding.completed = true;
        });
        assert!(!OnboardingWindow::should_show(&store));
        let _ = std::fs::remove_file(&path);
    }
}
