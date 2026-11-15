//! Pagination — port of `@heroui/pagination`.

use gpui::{
    prelude::*, px, App, InteractiveElement, IntoElement, RenderOnce, StatefulInteractiveElement,
    Styled, Window,
};
use herogpui_core::{element_id, Size};
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    icons,
};

type OnChange = std::sync::Arc<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

type Link = std::sync::Arc<dyn Fn(usize, bool) -> gpui::AnyElement + 'static>;

/// Tailwind's pinned `sm` breakpoint used by HeroUI's responsive pagination
/// root. Below it the summary and controls stack in one column.
const PAGINATION_SM_BREAKPOINT: f32 = 640.;

fn pagination_stacks_for_width(width: gpui::Pixels) -> bool {
    f32::from(width) < PAGINATION_SM_BREAKPOINT
}

/// HeroUI Pagination (controlled).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Pagination {
    /// `link` — v3's render prop for a page link, handed `isActive`.
    link: Option<Link>,
    previous_icon: Option<gpui::AnyElement>,
    next_icon: Option<gpui::AnyElement>,
    summary: Option<gpui::SharedString>,
    id: gpui::ElementId,
    page: usize,
    total: usize,
    is_disabled: bool,
    /// `Pagination.Link.isDisabled` / `Pagination.Previous.isDisabled` /
    /// `Pagination.Next.isDisabled` — the links and nav buttons disabled
    /// individually, keyed by the page each one navigates to: a page number
    /// in `1..=total` disables that link, `0` names Previous and
    /// `total + 1` names Next (the pages those two buttons point at).
    disabled_keys: std::collections::HashSet<usize>,
    size: Size,
    on_change: Option<OnChange>,
    /// The fill an enabled link or nav button takes while hovered or pressed,
    /// in place of `--default-hover`.
    hover_bg: Option<gpui::Hsla>,
    /// Expands the root to the available width.
    full_width: bool,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Pagination {
    /// Sets the size of the links and nav buttons.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The fill an enabled link or nav button takes while hovered or pressed,
    /// in place of `--default-hover`. Pressed stays coupled to hover, matching
    /// upstream's shared variable; the per-size press scale is unchanged.
    pub fn hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.hover_bg = Some(color.into());
        self
    }

    /// `fullWidth` — expands the root to the available width without
    /// redistributing the children.
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the bar's root element after every value the size and the
    /// active theme chose, so they win. The page cells and nav buttons keep
    /// their own ladder geometry.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// Creates a pagination for `page` of `total` pages; both are clamped to at least 1.
    pub fn new(id: impl Into<gpui::ElementId>, page: usize, total: usize) -> Self {
        Self {
            link: None,
            previous_icon: None,
            next_icon: None,
            summary: None,
            id: id.into(),
            page: page.max(1),
            total: total.max(1),
            is_disabled: false,
            disabled_keys: std::collections::HashSet::new(),
            size: Size::Md,
            on_change: None,
            hover_bg: None,
            full_width: false,
            sx: None,
        }
    }

    /// `Pagination.Summary` — the "Page 1 of 10" text v3 composes at the start
    /// of the row (`flex items-center gap-2 text-sm text-muted`).
    pub fn summary(mut self, text: impl Into<gpui::SharedString>) -> Self {
        self.summary = Some(text.into());
        self
    }

    /// `children` on `Pagination.Link` — replaces a page's label.
    ///
    /// The closure receives the page number and `isActive`, the values v3
    /// passes into the same render prop.
    pub fn link(mut self, render: impl Fn(usize, bool) -> gpui::AnyElement + 'static) -> Self {
        self.link = Some(std::sync::Arc::new(render));
        self
    }

    /// `Pagination.PreviousIcon.children` — replaces the default previous chevron.
    pub fn previous_icon(mut self, icon: impl IntoElement) -> Self {
        self.previous_icon = Some(icon.into_any_element());
        self
    }

    /// `Pagination.NextIcon.children` — replaces the default next chevron.
    pub fn next_icon(mut self, icon: impl IntoElement) -> Self {
        self.next_icon = Some(icon.into_any_element());
        self
    }

    /// Sets whether the whole pagination is disabled (`isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// `Pagination.Link.isDisabled`, `Pagination.Previous.isDisabled` and
    /// `Pagination.Next.isDisabled` — the links a caller wants disabled
    /// individually, keyed by the page each one navigates to. A page number
    /// in `1..=total` disables that link; `0` names Previous and
    /// `total + 1` names Next, the pages those buttons point at. A disabled
    /// link renders dimmed, leaves the tab order and answers no press,
    /// exactly like the whole-bar `is_disabled`.
    pub fn disabled_keys(mut self, keys: impl IntoIterator<Item = usize>) -> Self {
        self.disabled_keys = keys.into_iter().collect();
        self
    }

    /// Sets the handler called with the page number to navigate to (`onChange`).
    pub fn on_change(mut self, f: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(std::sync::Arc::new(f));
        self
    }
}

impl RenderOnce for Pagination {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Every interactive item is a tab stop with a ring. The handles come
        // first: `use_keyed_state` takes `cx` mutably and the theme is borrowed
        // for the rest of the render.
        let base_id = self.id.clone();
        let root_id = self.id.clone();
        let pages = visible_pages(self.page, self.total);
        // Only the page cells that are actually rendered need a tab stop.
        // Minting a keyed handle for every page in `0..=total` on every frame
        // grew with the collection even though `visible_pages` shows at most
        // a handful of them.
        let page_focus: Vec<(usize, gpui::FocusHandle)> = pages
            .iter()
            .filter_map(|page| match page {
                PageRef::Num(n) => Some((
                    *n,
                    crate::util::tab_stop_handle(
                        element_id::scoped(&element_id::indexed(&base_id, "page", *n), "focus"),
                        window,
                        cx,
                    ),
                )),
                PageRef::Ellipsis => None,
            })
            .collect();
        let prev_focus =
            crate::util::tab_stop_handle(element_id::scoped(&base_id, "prev-focus"), window, cx);
        let next_focus =
            crate::util::tab_stop_handle(element_id::scoped(&base_id, "next-focus"), window, cx);
        let ring_visible = crate::util::focus_visible(cx);

        let colors = cx.colors().clone();
        let control_hover_bg = self.hover_bg.unwrap_or(colors.default.hover());
        let layout = cx.layout().clone();
        let base = base_id;
        let stacked = pagination_stacks_for_width(window.viewport_size().width);

        let self_on_change: Option<OnChange> = self.on_change.clone();

        // v3 sizes the items; the nav buttons and page cells share it.
        let cell = match self.size {
            Size::Sm => px(28.),
            Size::Md => px(32.),
            Size::Lg => px(36.),
        };
        let press_scale = match self.size {
            Size::Sm => crate::anim::PRESSED_SCALE_SUBTLE,
            Size::Md => crate::anim::PRESSED_SCALE,
            Size::Lg => crate::anim::PRESSED_SCALE_FIRM,
        };
        let nav_padding = match self.size {
            Size::Sm => px(8.),
            Size::Md => px(10.),
            Size::Lg => px(12.),
        };
        let cell_text = self.size.text_size();
        let cell_leading = match self.size {
            Size::Sm => px(16.),
            Size::Md => px(20.),
            Size::Lg => px(24.),
        };
        let previous_icon = self.previous_icon.unwrap_or_else(|| {
            gpui::svg()
                .size(px(14.))
                .path(icons::CHEVRON_LEFT)
                .text_color(colors.foreground)
                .into_any_element()
        });
        let next_icon = self.next_icon.unwrap_or_else(|| {
            gpui::svg()
                .size(px(14.))
                .path(icons::CHEVRON_RIGHT)
                .text_color(colors.foreground)
                .into_any_element()
        });

        // `.pagination__content` is `gap-1`, not the 16px this used to leave.
        let mut row = gpui::div().flex().items_center().gap(px(4.));
        row = row.when(stacked, |row| row.self_start());

        // v3: "Disabled states properly communicated" — a disabled arrow is
        // `pointer-events-none` and React Aria's press never fires for it, so
        // the click handler is only attached when the arrow is enabled (the
        // tab stop is gated the same way inside `nav_button`).
        // `Pagination.Previous.isDisabled` force-disables the button beyond
        // the page bounds: `0` names it, the page the arrow navigates to.
        let prev_enabled = self.page > 1 && !self.is_disabled && !self.disabled_keys.contains(&0);
        row = row.child(
            nav_button(
                element_id::scoped(&base, "prev"),
                previous_icon,
                crate::i18n::ui_string(crate::i18n::UiString::Previous, cx),
                prev_enabled,
                NavStyle {
                    foreground: colors.foreground,
                    hover_bg: control_hover_bg,
                    disabled_opacity: layout.disabled_opacity,
                    cell,
                    text_size: cell_text,
                    line_height: cell_leading,
                    padding_x: nav_padding,
                    press_scale,
                    radius: crate::util::control_radius(cx),
                },
                &prev_focus,
                ring_visible && prev_focus.is_focused(window),
                window,
                cx,
            )
            .when(prev_enabled, |b| {
                b.on_click({
                    let cb: Option<OnChange> = self.on_change.clone();
                    move |_, w, cx| {
                        if let Some(cb) = &cb {
                            cb(&(self.page - 1), w, cx);
                        }
                    }
                })
            }),
        );

        for p in pages {
            match p {
                PageRef::Num(n) => {
                    let active = n == self.page;
                    // `Pagination.Link.isDisabled` — a link whose page number
                    // is in `disabled_keys` is disabled exactly like one under
                    // the whole-bar flag: it leaves the tab order and answers
                    // no press (v3's `status-disabled` is `pointer-events-none`
                    // and React Aria's press never fires for it).
                    let link_disabled = self.is_disabled || self.disabled_keys.contains(&n);
                    let mut btn = gpui::div()
                        .id(element_id::indexed(&base, "page", n))
                        // `@heroui/react/dist/components/pagination/
                        // pagination.js`'s `PaginationLink` is an RAC
                        // `Button` — a native `<button>` — carrying
                        // `"aria-current": isActive ? "page" : undefined`.
                        // Vanilla gpui publishes no setter for that state, so
                        // `a11y_current` is currently a documented no-op (see
                        // `a11y.rs`); the number is still restated as the name
                        // because a text child contributes none.
                        .a11y_named(a11y::Role::Button, &a11y::Name::labelled(n.to_string()))
                        .when(active, |b| b.a11y_current(a11y::AriaCurrent::Page))
                        .flex()
                        .items_center()
                        .justify_center()
                        // `.pagination__link` is `size-8` from `md` up: a square
                        // cell with no padding of its own.
                        .min_w(cell)
                        .h(cell)
                        .text_size(cell_text)
                        .line_height(cell_leading)
                        .rounded(crate::util::control_radius(cx))
                        .when(!link_disabled, |b| b.cursor(crate::util::interactive_cursor(cx)));

                    if active {
                        // `[data-active=true]` is v3's tertiary-button state:
                        // `--default` at rest and `--default-hover` while
                        // hovered or pressed, not the accent fill.
                        btn = btn.bg(colors.default.color).text_color(colors.foreground);
                    } else {
                        // The resting ghost link is transparent and borderless.
                        btn = btn.text_color(colors.foreground);
                    }
                    // The label joins the skin after the fade's fill — the
                    // fill is appended and would paint over a child already
                    // present — and before the press wrap, whose slot would
                    // inherit children added after it. `link` is v3's render
                    // prop on `Pagination.Link`: it receives `isActive`, so a
                    // caller can style the current page without re-deriving
                    // which one it is.
                    let paint_label = |btn: gpui::Stateful<gpui::Div>| match &self.link {
                        Some(render) => btn.child(render(n, active)),
                        None => btn.child(n.to_string()),
                    };
                    if !link_disabled {
                        let hover_bg = control_hover_bg;
                        // `.pagination__link[data-pressed]` applies to every
                        // enabled link, including the active page: the
                        // default-hover fill rides inside the press
                        // refinement, which owns the scale.
                        let pressed_bg = control_hover_bg;
                        let idle_bg = if active {
                            colors.default.color
                        } else {
                            herogpui_core::with_alpha(colors.foreground, 0.0)
                        };
                        let radius = crate::util::control_radius(cx);
                        btn = crate::anim::hover_fade(
                            btn,
                            element_id::scoped(&base, format!("page-hover-{n}")),
                            (idle_bg, hover_bg),
                            None,
                            None,
                            move |fill| fill.rounded(radius),
                            window,
                            cx,
                        );
                        btn = paint_label(btn);
                        btn = crate::anim::pressed_with_background(
                            btn,
                            crate::anim::PressBox {
                                height: cell,
                                padding_x: Some(px(6.)),
                                width: None,
                                min_width: Some(cell),
                                text_size: cell_text,
                                line_height: cell_leading,
                                gap: px(0.),
                                radius: crate::util::control_radius(cx),
                                shrink_x: true,
                                scale: press_scale,
                            },
                            pressed_bg,
                            cx,
                        );
                    } else {
                        btn = paint_label(btn);
                    }
                    // `aria-current` identifies the active page without
                    // disabling its React Aria Button. Every enabled numeric
                    // link forwards the same press callback, active or not.
                    if !link_disabled {
                        if let Some(cb) = self_on_change.clone() {
                            btn = btn.on_click(move |_, w, cx| cb(&n, w, cx));
                        }
                    }
                    // `.pagination__link:disabled` is `status-disabled` —
                    // dimmed, like the nav buttons at their own bounds.
                    if link_disabled {
                        btn = btn.opacity(layout.disabled_opacity);
                    }
                    // The focus tracking lands on the press slot (the element
                    // `pressed` returns) so keyboard activation and pointer
                    // activation answer on the same element.
                    if !link_disabled {
                        if let Some((_, handle)) = page_focus.iter().find(|(p, _)| *p == n) {
                            btn = btn.track_focus(handle);
                        }
                    }
                    // `.pagination__item:focus-visible` is `status-focused`.
                    let mut btn = crate::util::with_focus_ring_overlay(
                        btn,
                        ring_visible
                            && page_focus
                                .iter()
                                .any(|(p, handle)| *p == n && handle.is_focused(window)),
                        true,
                        crate::util::control_radius(cx),
                        Vec::new(),
                        cx,
                    );
                    if !link_disabled {
                        if let Some((_, handle)) = page_focus.iter().find(|(p, _)| *p == n) {
                            btn = crate::util::record_focus_bounds(btn, handle, window, cx);
                        }
                    }
                    row = row.child(btn);
                }
                PageRef::Ellipsis => {
                    row = row.child(
                        gpui::div()
                            .flex()
                            .items_center()
                            .justify_center()
                            // `.pagination__ellipsis` is the same `size-8
                            // text-sm` cell as a page link.
                            .size(cell)
                            .text_size(cell_text)
                            .line_height(cell_leading)
                            .text_color(colors.muted)
                            .child("…"),
                    );
                }
            }
        }

        // `Pagination.Next.isDisabled` force-disables the button beyond the
        // page bounds: `total + 1` names it, the page the arrow navigates to.
        let next_enabled = self.page < self.total
            && !self.is_disabled
            && !self.disabled_keys.contains(&(self.total + 1));
        row = row.child(
            nav_button(
                element_id::scoped(&base, "next"),
                next_icon,
                crate::i18n::ui_string(crate::i18n::UiString::Next, cx),
                next_enabled,
                NavStyle {
                    foreground: colors.foreground,
                    hover_bg: control_hover_bg,
                    disabled_opacity: layout.disabled_opacity,
                    cell,
                    text_size: cell_text,
                    line_height: cell_leading,
                    padding_x: nav_padding,
                    press_scale,
                    radius: crate::util::control_radius(cx),
                },
                &next_focus,
                ring_visible && next_focus.is_focused(window),
                window,
                cx,
            )
            .when(next_enabled, |b| {
                b.on_click({
                    let cb: Option<OnChange> = self.on_change.clone();
                    move |_, w, cx| {
                        if let Some(cb) = &cb {
                            cb(&(self.page + 1), w, cx);
                        }
                    }
                })
            }),
        );

        // `.pagination` is the root: `flex w-full items-center justify-between
        // gap-4` around the summary and the content.
        let el = gpui::div()
            .flex()
            .when(stacked, |root| root.flex_col())
            .items_center()
            .justify_between()
            .gap(px(16.))
            .children(self.summary.map(|text| {
                gpui::div()
                    .flex()
                    .items_center()
                    .when(stacked, |summary| summary.self_start())
                    // `.pagination__summary` is `gap-2 text-sm text-muted`.
                    .gap(px(8.))
                    .text_size(cell_text)
                    .line_height(cell_leading)
                    .text_color(colors.muted)
                    .child(text.to_string())
            }))
            .child(row);
        // Stated after the layout chain: `.shots/design_audit.py` reads this
        // component's metrics out of the builder chains with character
        // windows.
        //
        // `pagination.js`'s root is a plain `dom.nav` with `role:
        // "navigation"` and `"aria-label": "pagination"` hard-coded — not an
        // RAC component at all, so the label is a literal rather than a
        // localized string. Its `<ul>`/`<li>` layer has no counterpart
        // element here: this port draws the page cells straight into the
        // row, and a `list` node whose children are buttons rather than
        // list items would describe a structure that is not there.
        let mut el = el.id(root_id).a11y_named(
            a11y::Role::Navigation,
            &a11y::Name::labelled(crate::i18n::ui_string(
                crate::i18n::UiString::Pagination,
                cx,
            )),
        );
        if self.full_width {
            el = el.w_full();
        }
        crate::util::apply_sx(el, &self.sx)
    }
}

/// Colours and metrics shared by the nav buttons.
struct NavStyle {
    foreground: gpui::Hsla,
    hover_bg: gpui::Hsla,
    disabled_opacity: f32,
    cell: gpui::Pixels,
    text_size: gpui::Pixels,
    line_height: gpui::Pixels,
    padding_x: gpui::Pixels,
    press_scale: f32,
    /// `.pagination__link` is `rounded-3xl`; `--nav` restates only the width.
    radius: gpui::Pixels,
}

#[allow(clippy::too_many_arguments)] // one parameter per nav-cell style channel
fn nav_button(
    id: gpui::ElementId,
    icon: gpui::AnyElement,
    name: gpui::SharedString,
    enabled: bool,
    style: NavStyle,
    focus: &gpui::FocusHandle,
    // Whether this button is the one holding a keyboard-visible focus, and so
    // the one that draws `status-focused`.
    ring: bool,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let NavStyle {
        foreground,
        hover_bg,
        disabled_opacity,
        cell,
        text_size,
        line_height,
        padding_x,
        press_scale,
        radius,
    } = style;
    let hover_id = element_id::scoped(&id, "hover-fade");
    let mut btn = gpui::div()
        .id(id)
        // `PaginationPrevious` / `PaginationNext` are RAC `Button`s whose only
        // child is an `"aria-hidden"` icon, so upstream leaves them unnamed.
        // HeroGPUI names them from the same i18n strings the Calendar arrows
        // use, since a control a screen reader cannot name is worse than a
        // name upstream does not ship.
        .a11y_named(a11y::Role::Button, &a11y::Name::labelled(name))
        .flex()
        .items_center()
        .justify_center()
        // `.pagination__link--nav` is `w-auto gap-1.5 px-2.5`: the height of a
        // page cell, but as wide as its content needs.
        .h(cell)
        .text_size(text_size)
        .line_height(line_height)
        .gap(px(6.))
        .px(padding_x)
        .rounded(radius)
        .text_color(foreground);
    if enabled {
        let radius_for_hover = radius;
        btn = btn.cursor(crate::util::interactive_cursor(cx));
        btn = crate::anim::hover_fade(
            btn,
            hover_id,
            (herogpui_core::with_alpha(foreground, 0.0), hover_bg),
            None,
            None,
            move |fill| fill.rounded(radius_for_hover),
            window,
            cx,
        );
        // The icon joins the skin after the fade's fill — the fill is
        // appended and would paint over a child already present — and
        // before the press wrap, whose slot would inherit children added
        // after it.
        btn = btn.child(icon);
        btn = crate::anim::pressed_with_background(
            btn,
            crate::anim::PressBox {
                height: cell,
                padding_x: Some(padding_x),
                width: None,
                min_width: None,
                text_size,
                line_height,
                gap: px(6.),
                radius,
                shrink_x: true,
                scale: press_scale,
            },
            hover_bg,
            cx,
        );
    } else {
        btn = btn.child(icon).opacity(disabled_opacity);
    }
    // A disabled control must leave the tab order — `track_focus` is what
    // puts it in, so gate it (v3 gives a disabled arrow `pointer-events-none`
    // and nothing for Tab to land on). The tracking lands on the press slot
    // (the element `pressed` returns) so keyboard activation and pointer
    // activation answer on the same element.
    if enabled {
        btn = btn.track_focus(focus);
    }
    // The ring goes on the element the press wrap returns -- the stable slot
    // that keeps the resting footprint, carries `radius` and owns the focus --
    // and not on the skin inside it, which shrinks under a press. A disabled
    // arrow is no tab stop, so `ring` is never set for one and the branch
    // resolves to the bare button.
    let mut btn = crate::util::with_focus_ring_overlay(btn, ring, true, radius, Vec::new(), cx);
    if enabled {
        btn = crate::util::record_focus_bounds(btn, focus, window, cx);
    }
    btn
}

enum PageRef {
    Num(usize),
    Ellipsis,
}

fn visible_pages(page: usize, total: usize) -> Vec<PageRef> {
    // v3 removed the v2 `siblings`/`boundaries` props ("Removed (compose items
    // manually)"), so the window is fixed at one sibling either side.
    let siblings = 1;
    // total numbers to show = 2*siblings + 5 (first, last, current, 2 ellipsis slots)
    let total_slots = 2 * siblings + 5;
    if total <= total_slots {
        return (1..=total).map(PageRef::Num).collect();
    }

    let left = (page - siblings).max(2);
    let right = (page + siblings).min(total - 1);

    let mut out = vec![PageRef::Num(1)];
    if left > 2 {
        out.push(PageRef::Ellipsis);
    }
    for n in left..=right {
        out.push(PageRef::Num(n));
    }
    if right < total - 1 {
        out.push(PageRef::Ellipsis);
    }
    out.push(PageRef::Num(total));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_stacks_below_the_pinned_sm_breakpoint() {
        assert!(pagination_stacks_for_width(px(639.)));
        assert!(!pagination_stacks_for_width(px(640.)));
        assert!(!pagination_stacks_for_width(px(1024.)));
    }

    #[test]
    fn pagination_root_uses_the_responsive_stack_and_start_alignment() {
        let source = include_str!("pagination.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("pagination_stacks_for_width(window.viewport_size().width)")
                && source.contains(".when(stacked, |root| root.flex_col())")
                && source.contains(".when(stacked, |summary| summary.self_start())")
                && source.contains(".when(stacked, |row| row.self_start())"),
            "pagination must stack and align both children below HeroUI's sm breakpoint"
        );
    }

    /// `visible_pages` is the window `page_focus` is minted from. The bound is
    /// `2 * siblings + 5` with `siblings` fixed at 1, so a large `total` still
    /// produces a handful of slots — the allocation the render used to spend
    /// per page in `0..=total`.
    #[test]
    fn visible_page_window_is_bounded_for_a_large_total() {
        let cap = 7;
        for total in [8, 50, 2000, 10_000] {
            for page in [1, total / 2, total] {
                let pages = visible_pages(page.max(1), total);
                assert!(
                    pages.len() <= cap,
                    "page {page} of {total} showed {} slots, cap {cap}",
                    pages.len()
                );
            }
        }
        assert!(
            visible_pages(1, 5).len() <= cap,
            "a collection that fits in the window is still within the cap"
        );
    }

    #[test]
    fn page_focus_is_minted_from_the_visible_window() {
        let source = include_str!("pagination.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("let page_focus: Vec<(usize, gpui::FocusHandle)> = pages"),
            "page-cell focus handles must be minted from visible_pages"
        );
        assert!(
            !source.contains("(0..=self.total)"),
            "minting a handle per page in 0..=total grew with the collection"
        );
    }
}

crate::util::impl_component_styled!(Pagination);
