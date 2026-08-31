//! The About page is the app's AGPL §5(d) "Appropriate Legal Notices"
//! surface: it must always show the copyright notices, the AGPL v3 license
//! with a working link, the no-warranty statement, a link to this exact
//! version's source, and the third-party notices link.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled, Window};

use crate::product;
use crate::system::desktop;
use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::windows::settings::SettingsWindow;

const NOTICES: [&str; 4] = [
    "Poratake is a modified version of Capty. Modifications made in 2026.",
    "Copyright \u{00a9} 2026 Capty.",
    "Copyright \u{00a9} 2026 Serhii Vecherenko for Poratake. Poratake is developed by Porabuild. Copyright in other contributions remains with their respective contributors.",
    "Licensed under GNU AGPL v3.0, without warranty. You may redistribute Poratake under the same license.",
];

fn link_row(
    id: &'static str,
    icon: &'static str,
    label: &'static str,
    url: String,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let _ = cx;
    let (hover, hovered) = crate::ui::primitives::hover_flag(id, window, cx);
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .py(px(4.0))
        .text_size(px(13.0))
        .text_color(if hovered {
            theme.foreground
        } else {
            theme.muted_foreground
        })
        .on_hover({
            let hover = hover.clone();
            move |over: &bool, _window, cx| {
                crate::ui::primitives::track_hover(&hover, *over, cx);
            }
        })
        .on_click(move |_event, _window, _cx| desktop::open_url(&url))
        .child(icon_element(icon, px(14.0)))
        .child(label)
        .into_any_element()
}

fn brand_logo(theme: &ThemeVars) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_end()
        .h(px(29.0))
        .text_size(px(24.0))
        .child(div().font_weight(gpui::FontWeight::BOLD).child("Pora"))
        .child(
            div()
                .relative()
                .h(px(24.0))
                .w(px(5.76))
                .ml(px(2.4))
                .mr(px(0.72))
                .child(
                    div()
                        .absolute()
                        .bottom(px(1.0))
                        .left(px(0.72))
                        .size(px(4.32))
                        .rounded_full()
                        .bg(theme.accent),
                ),
        )
        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("take"))
        .into_any_element()
}

/// `renderUpdateSection` in `about-tab.tsx`: a status line with its icon, a
/// `Check` button for the resting states, and a card naming the new version when
/// there is one. The reference's `Install Update` button has no counterpart --
/// see `crate::update` for why -- so an available update offers its release page.
fn update_section(
    theme: &ThemeVars,
    status: crate::update::Status,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    use crate::update::Status;

    let mut section = div().flex().flex_col().gap(px(12.0)).child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(if status.spins() {
                        crate::ui::icon::spinner_element("about-update-spinner", px(16.0))
                    } else {
                        div()
                            .text_color(match status {
                                Status::UpToDate => crate::ui::colors::green_500(1.0),
                                Status::Error { .. } => crate::ui::colors::red_500(1.0),
                                _ => theme.foreground,
                            })
                            .child(icon_element(status.icon(), px(16.0)))
                            .into_any_element()
                    })
                    .child(div().text_size(px(chrome::TEXT_SM)).child(status.text())),
            )
            .when(status.shows_check_button(), |el| {
                el.child(
                    Button::new("about-check-updates")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .icon("refresh-cw")
                        .label("Check")
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.check_for_updates(cx);
                        })),
                )
            }),
    );

    // `{status === 'downloading' && …}`: a 2px track with the primary fill and
    // the percentage right-aligned beneath it.
    if let Status::Downloading { progress, .. } = &status {
        let fraction = *progress;
        section = section.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .h(px(8.0))
                        .w_full()
                        .overflow_hidden()
                        .rounded_full()
                        .bg(theme.muted_background)
                        .child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(theme.primary)
                                .w(gpui::relative(fraction)),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .text_align(gpui::TextAlign::Right)
                        .text_size(px(chrome::TEXT_XS))
                        .text_color(theme.muted_foreground)
                        .child(format!("{}%", (fraction * 100.0).floor() as i32)),
                ),
        );
    }

    // `{(status === 'available' || status === 'ready') && latestVersion && …}`.
    if let Some(version) = status.version() {
        if !matches!(status, Status::Downloading { .. }) {
            section = section.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    // `rounded-lg border bg-green-500/5 p-3`.
                    .rounded(px(crate::ui::chrome::RADIUS_LG))
                    .border_1()
                    .border_color(theme.border)
                    .bg(crate::ui::colors::green_500(0.05))
                    .p(px(12.0))
                    .child(
                        div()
                            .text_size(px(chrome::TEXT_SM))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(format!("Version {version} is available")),
                    ),
            );
        }
    }

    // `Download update` is this shell's name for the step Electron takes when
    // `autoDownload` is off; the reference's own `Install Update` follows once
    // the artifact is verified.
    if matches!(status, Status::Available { .. }) {
        section = section.child(
            Button::new("about-download-update")
                .variant(ButtonVariant::Primary)
                .size(ButtonSize::Md)
                .icon("download")
                .label("Download Update")
                .full_width()
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.download_update(cx);
                })),
        );
    }

    if matches!(status, Status::Ready { .. }) {
        section = section.child(
            Button::new("about-install-update")
                .variant(ButtonVariant::Primary)
                .size(ButtonSize::Md)
                .icon("download")
                .label("Install Update")
                .full_width()
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.install_update(cx);
                })),
        );
    }

    if let Status::Error { message } = &status {
        section = section.child(
            div()
                .text_size(px(chrome::TEXT_XS))
                .text_color(crate::ui::colors::red_500(1.0))
                .child(message.clone()),
        );
    }

    section.into_any_element()
}

fn separator(_theme: &ThemeVars) -> AnyElement {
    crate::ui::primitives::Separator::horizontal()
        .inset(px(12.0))
        .into_any_element()
}

pub fn render(
    theme: &ThemeVars,
    // Passed in rather than read back out of the context: `Entity::read` panics
    // while the entity is mid-render.
    update: crate::update::Status,
    window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let version = product::VERSION;
    let source_url = product::source_url_for_version();
    let license_url = format!("{}/blob/v{version}/LICENSE", product::SOURCE_URL);
    let notices_url = format!(
        "{}/blob/v{version}/THIRD_PARTY_NOTICES.md",
        product::SOURCE_URL
    );

    // Each notice is a `<p>`, so it wraps inside the content column rather than
    // stretching it -- the longest line is well over the 720px column.
    let mut notices = div().flex().flex_col().w_full().gap(px(4.0));
    for line in NOTICES {
        notices = notices.child(
            div()
                .w_full()
                .text_size(px(11.0))
                .text_color(theme.muted_foreground)
                .child(line),
        );
    }

    div()
        .flex()
        .flex_col()
        // Without this the column sizes to its widest child instead of the
        // content width, the long copyright line never wraps, and the whole page
        // is pushed out of the centred container and clipped by the sidebar.
        .w_full()
        .gap(px(4.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .gap(px(16.0))
                // `<img src={appIcon} className="h-16 w-16 rounded-xl">`. The
                // tile below is only a fallback for an icon that failed to
                // decode; the picture itself is `build/icon.png`.
                .child(
                    crate::ui::app_icon::element(
                        px(64.0),
                        px(crate::ui::chrome::RADIUS_XL),
                    )
                    .unwrap_or_else(|| {
                        div()
                            .size(px(64.0))
                            .rounded(px(crate::ui::chrome::RADIUS_XL))
                            .bg(theme.accent)
                            .text_color(theme.accent_foreground)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon_element("aperture", px(32.0)))
                            .into_any_element()
                    }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(brand_logo(theme))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(theme.muted_foreground)
                                .child(format!("Version {version}")),
                        ),
                ),
        )
        .child(
            div()
                .pt(px(12.0))
                .text_size(px(13.0))
                .text_color(theme.muted_foreground)
                .child(
                    "Capture, annotate, record, edit, and share from one focused workspace on macOS and Windows.",
                ),
        )
        .child(separator(theme))
        .child(update_section(theme, update, cx))
        .child(separator(theme))
        .child(
            div()
                .flex()
                .flex_col()
                .child(link_row(
                    "about-porabuild",
                    "globe",
                    "Porabuild website",
                    product::PORABUILD_URL.to_string(),
                    theme,
                    window,
                    cx,
                ))
                .child(link_row(
                    "about-poratake",
                    "globe",
                    "Poratake website",
                    product::PRODUCT_HOMEPAGE.to_string(),
                    theme,
                    window,
                    cx,
                ))
                .child(link_row(
                    "about-source",
                    "code-2",
                    "This version's source",
                    source_url.clone(),
                    theme,
                    window,
                    cx,
                ))
                .child(link_row(
                    "about-upstream",
                    "code-2",
                    "Original Capty project",
                    product::UPSTREAM_URL.to_string(),
                    theme,
                    window,
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(12.0))
                        .py(px(4.0))
                        .text_size(px(13.0))
                        .text_color(theme.muted_foreground)
                        .child(icon_element("heart", px(14.0)))
                        .child("Made for people who capture, explain, and share"),
                ),
        )
        .child(separator(theme))
        .child(notices)
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(8.0))
                .pt(px(12.0))
                .child(
                    Button::new("about-source-button")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .icon("code-2")
                        .label("This Version's Source")
                        .on_click(move |_event, _window, _cx| {
                            desktop::open_url(&source_url);
                        }),
                )
                .child(
                    Button::new("about-license-button")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .icon("scale")
                        .label("GNU AGPL v3.0")
                        .on_click(move |_event, _window, _cx| {
                            desktop::open_url(&license_url);
                        }),
                )
                .child(
                    Button::new("about-notices-button")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .label(SharedString::from("Third-party Notices"))
                        .on_click(move |_event, _window, _cx| {
                            desktop::open_url(&notices_url);
                        }),
                ),
        )
        .into_any_element()
}
