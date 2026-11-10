//! `Sidebar` — a collapsible application sidebar (HeroGPUI extension; HeroUI
//! v3 has no sidebar). The API follows gpui-kit's `Sidebar` and shadcn/ui's
//! sidebar: a header, groups of menu items under optional labels, and a
//! footer.
//!
//! - **Items.** Each [`SidebarItem`] has a label, an optional icon
//!   ([`crate::IconName`] or any served SVG) and an optional badge. One item is
//!   active (`active_key`, controlled, or `default_active_key`); pressing an
//!   item, or Enter / Space on it, makes it active and reports it through
//!   `on_select`.
//! - **Groups.** A [`SidebarGroup`] may carry a label, and a collapsible group
//!   turns its label into a disclosure trigger whose panel expands and
//!   collapses with the same measured-height motion and rotating chevron as
//!   `Disclosure`. Expanded groups are controlled (`expanded_keys`) or seeded
//!   by each group's `default_expanded`.
//! - **Collapse to icons.** `is_collapsed` (controlled) or `default_collapsed`
//!   narrows the sidebar to `collapsed_width`, hides the labels and group
//!   headings, lists every item as its icon and names it with a `Tooltip` to
//!   the right, shown on hover and on keyboard focus. The built-in toggle in
//!   the footer (`show_collapse_toggle`) flips it and reports through
//!   `on_collapsed_change`.
//! - **Keyboard.** The menu is one tab stop with a roving cursor: Up / Down /
//!   Home / End move over the visible items and collapsible group headers,
//!   skipping disabled items; Right expands and Left collapses a group from
//!   its header; Enter and Space activate; typing a label's first letters
//!   moves to it. The focus handle moves with the cursor row, which is what
//!   lets a collapsed item's tooltip open on keyboard focus.
//! - **Width.** `width` (256px) and `collapsed_width` (56px) are fixed. For a
//!   user-resizable sidebar, put it in a `ResizablePanelGroup` panel; the
//!   panel sizes are percentages, so there is no pixel-width drag built in.
//!
//! **Accessibility.** The root reports `Role::Navigation` named by `label`,
//! each group `Role::Group` named by its label, a collapsible group's header
//! `Role::Button` with `expanded`, and each item `Role::Link` named by its
//! label, the cursor row marked as the active descendant. The active item's
//! `aria-current="page"` is written through `A11y::a11y_current`, which the
//! pinned gpui cannot express yet (a no-op, see `a11y.rs`).
//!
//! ```
//! use herogpui_components::{IconName, Sidebar, SidebarGroup, SidebarItem};
//!
//! let sidebar = Sidebar::new(
//!     "app-sidebar",
//!     vec![
//!         SidebarGroup::new("main").items(vec![
//!             SidebarItem::new("inbox", "Inbox").icon(IconName::Inbox).badge("12"),
//!             SidebarItem::new("drafts", "Drafts").icon(IconName::File),
//!         ]),
//!         SidebarGroup::new("projects")
//!             .label("Projects")
//!             .is_collapsible(true)
//!             .items(vec![SidebarItem::new("web", "Website").icon(IconName::Globe)]),
//!     ],
//! )
//! .default_active_key("inbox")
//! .on_select(|key, _window, _cx| println!("go to {key}"));
//! ```

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    div, prelude::FluentBuilder as _, px, AnyElement, App, ElementId, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use herogpui_core::element_id;
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};
use crate::{anim, util, IconName, Tooltip, TooltipPlacement};

type KeyCallback = Arc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;
type BoolCallback = Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;
type KeysCallback = Arc<dyn Fn(&HashSet<SharedString>, &mut Window, &mut App) + 'static>;

/// The height of one menu item.
const ITEM_HEIGHT: Pixels = px(32.);

/// One menu item of a [`SidebarGroup`].
#[must_use]
pub struct SidebarItem {
    key: SharedString,
    label: SharedString,
    icon: Option<SharedString>,
    badge: Option<SharedString>,
    is_disabled: bool,
}

impl SidebarItem {
    /// An item `label` identified by `key`, unique within the sidebar.
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            icon: None,
            badge: None,
            is_disabled: false,
        }
    }

    /// The icon before the label, and the whole item when collapsed.
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// A short trailing text (a count, "New"), hidden when collapsed.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// A disabled item renders dimmed and cannot be focused or activated.
    pub fn is_disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }
}

/// A group of [`SidebarItem`]s under an optional label.
#[must_use]
pub struct SidebarGroup {
    key: SharedString,
    label: Option<SharedString>,
    items: Vec<SidebarItem>,
    is_collapsible: bool,
    default_expanded: bool,
}

impl SidebarGroup {
    /// A group identified by `key`, unique within the sidebar.
    pub fn new(key: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: None,
            items: Vec::new(),
            is_collapsible: false,
            default_expanded: true,
        }
    }

    /// The heading above the items.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Appends one item.
    pub fn item(mut self, item: SidebarItem) -> Self {
        self.items.push(item);
        self
    }

    /// Appends items.
    pub fn items(mut self, items: impl IntoIterator<Item = SidebarItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Makes the heading a disclosure trigger that expands and collapses the
    /// items. A group without a label cannot collapse.
    pub fn is_collapsible(mut self, collapsible: bool) -> Self {
        self.is_collapsible = collapsible;
        self
    }

    /// Whether a collapsible group starts expanded (the default) in an
    /// uncontrolled sidebar.
    pub fn default_expanded(mut self, expanded: bool) -> Self {
        self.default_expanded = expanded;
        self
    }

    fn collapses(&self) -> bool {
        self.is_collapsible && self.label.is_some()
    }
}

/// A collapsible application sidebar. See the [module docs](self).
#[must_use]
#[derive(IntoElement)]
pub struct Sidebar {
    id: ElementId,
    groups: Vec<SidebarGroup>,
    header: Vec<AnyElement>,
    footer: Vec<AnyElement>,
    label: SharedString,
    active_key: Option<SharedString>,
    is_active_controlled: bool,
    default_active_key: Option<SharedString>,
    is_collapsed: Option<bool>,
    default_collapsed: bool,
    expanded_keys: HashSet<SharedString>,
    is_expansion_controlled: bool,
    show_collapse_toggle: bool,
    width: Pixels,
    collapsed_width: Pixels,
    is_disabled: bool,
    on_select: Option<KeyCallback>,
    on_collapsed_change: Option<BoolCallback>,
    on_expanded_change: Option<KeysCallback>,
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Sidebar {
    /// A sidebar over `groups`. `id` keys its state; give every instance its
    /// own.
    pub fn new(id: impl Into<ElementId>, groups: Vec<SidebarGroup>) -> Self {
        Self {
            id: id.into(),
            groups,
            header: Vec::new(),
            footer: Vec::new(),
            label: SharedString::new_static("Sidebar"),
            active_key: None,
            is_active_controlled: false,
            default_active_key: None,
            is_collapsed: None,
            default_collapsed: false,
            expanded_keys: HashSet::new(),
            is_expansion_controlled: false,
            show_collapse_toggle: true,
            width: px(256.),
            collapsed_width: px(56.),
            is_disabled: false,
            on_select: None,
            on_collapsed_change: None,
            on_expanded_change: None,
            sx: None,
        }
    }

    /// Content above the menu (a logo, a workspace switcher). It is clipped,
    /// not restyled, when the sidebar collapses; drive `is_collapsed` to adapt
    /// it.
    pub fn header(mut self, content: impl IntoElement) -> Self {
        self.header.push(content.into_any_element());
        self
    }

    /// Content below the menu, before the collapse toggle.
    pub fn footer(mut self, content: impl IntoElement) -> Self {
        self.footer.push(content.into_any_element());
        self
    }

    /// The navigation landmark's accessible name; "Sidebar" by default.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// The controlled active item (`None` for none).
    pub fn active_key(mut self, key: Option<SharedString>) -> Self {
        self.active_key = key;
        self.is_active_controlled = true;
        self
    }

    /// The initially active item of an uncontrolled sidebar.
    pub fn default_active_key(mut self, key: impl Into<SharedString>) -> Self {
        self.default_active_key = Some(key.into());
        self
    }

    /// The controlled collapsed (icons-only) state.
    pub fn is_collapsed(mut self, collapsed: bool) -> Self {
        self.is_collapsed = Some(collapsed);
        self
    }

    /// The initial collapsed state of an uncontrolled sidebar.
    pub fn default_collapsed(mut self, collapsed: bool) -> Self {
        self.default_collapsed = collapsed;
        self
    }

    /// The controlled set of expanded collapsible groups.
    pub fn expanded_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.expanded_keys = keys.into_iter().collect();
        self.is_expansion_controlled = true;
        self
    }

    /// Whether the footer carries the built-in collapse toggle; `true` by
    /// default.
    pub fn show_collapse_toggle(mut self, show: bool) -> Self {
        self.show_collapse_toggle = show;
        self
    }

    /// The expanded width; 256px by default.
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = width.into();
        self
    }

    /// The collapsed (icons-only) width; 56px by default.
    pub fn collapsed_width(mut self, width: impl Into<Pixels>) -> Self {
        self.collapsed_width = width.into();
        self
    }

    /// A disabled sidebar renders dimmed, leaves the tab order and ignores
    /// input.
    pub fn is_disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }

    /// Runs with the key of the item a press, Enter or Space activated.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Arc::new(f));
        self
    }

    /// Reports the collapsed state the toggle asks for.
    pub fn on_collapsed_change(
        mut self,
        f: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_collapsed_change = Some(Arc::new(f));
        self
    }

    /// Reports the expanded groups after a header press or key changed them.
    pub fn on_expanded_change(
        mut self,
        f: impl Fn(&HashSet<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_expanded_change = Some(Arc::new(f));
        self
    }

    /// Caller-owned styling refined over the root after the theme's values.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }
}

/// One keyboard stop: a collapsible group's header or an item.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Stop {
    Header { group: usize },
    Item { group: usize, item: usize },
}

/// The stops in visual order: a collapsible header (not in icon mode), then
/// its items when the group shows them. Disabled items are left out.
fn stops_of(groups: &[SidebarGroup], expanded: &HashSet<SharedString>, icons: bool) -> Vec<Stop> {
    let mut stops = Vec::new();
    for (g, group) in groups.iter().enumerate() {
        let collapses = group.collapses() && !icons;
        if collapses {
            stops.push(Stop::Header { group: g });
        }
        if collapses && !expanded.contains(&group.key) {
            continue;
        }
        for (i, item) in group.items.iter().enumerate() {
            if !item.is_disabled {
                stops.push(Stop::Item { group: g, item: i });
            }
        }
    }
    stops
}

/// The keyed cursor: which stop is highlighted, by key so it survives groups
/// opening and closing around it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum CursorKey {
    Header(SharedString),
    Item(SharedString),
}

fn cursor_key(groups: &[SidebarGroup], stop: &Stop) -> CursorKey {
    match *stop {
        Stop::Header { group } => CursorKey::Header(groups[group].key.clone()),
        Stop::Item { group, item } => CursorKey::Item(groups[group].items[item].key.clone()),
    }
}

/// What the event closures share, cloned into each.
#[derive(Clone)]
struct SidebarState {
    cursor: gpui::Entity<Option<CursorKey>>,
    active_own: Option<gpui::Entity<Option<SharedString>>>,
    expanded: HashSet<SharedString>,
    expanded_own: Option<gpui::Entity<HashSet<SharedString>>>,
    collapsed: bool,
    collapsed_own: Option<gpui::Entity<bool>>,
    on_select: Option<KeyCallback>,
    on_collapsed_change: Option<BoolCallback>,
    on_expanded_change: Option<KeysCallback>,
}

impl SidebarState {
    fn set_cursor(&self, key: CursorKey, cx: &mut App) {
        self.cursor.update(cx, |value, cx| {
            if value.as_ref() != Some(&key) {
                *value = Some(key);
                cx.notify();
            }
        });
    }

    fn select(&self, key: &SharedString, window: &mut Window, cx: &mut App) {
        if let Some(own) = &self.active_own {
            own.update(cx, |value, cx| {
                *value = Some(key.clone());
                cx.notify();
            });
        }
        if let Some(cb) = &self.on_select {
            cb(key, window, cx);
        }
    }

    fn set_expanded(&self, key: &SharedString, open: bool, window: &mut Window, cx: &mut App) {
        if self.expanded.contains(key) == open {
            return;
        }
        let mut next = self.expanded.clone();
        if open {
            next.insert(key.clone());
        } else {
            next.remove(key);
        }
        if let Some(own) = &self.expanded_own {
            own.update(cx, |value, cx| {
                value.clone_from(&next);
                cx.notify();
            });
        }
        if let Some(cb) = &self.on_expanded_change {
            cb(&next, window, cx);
        }
    }

    fn toggle_collapsed(&self, window: &mut Window, cx: &mut App) {
        let next = !self.collapsed;
        if let Some(own) = &self.collapsed_own {
            own.update(cx, |value, cx| {
                *value = next;
                cx.notify();
            });
        }
        if let Some(cb) = &self.on_collapsed_change {
            cb(&next, window, cx);
        }
    }
}

/// The debug-selector spelling of an id: its name when it has one.
fn selector_base(id: &ElementId) -> String {
    match id {
        ElementId::Name(name) => name.to_string(),
        other => format!("{other:?}"),
    }
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = self.id.clone();
        let selector = selector_base(&base);
        let disabled = self.is_disabled;
        let focus = window
            .use_keyed_state(element_id::scoped(&base, "focus"), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone()
            .tab_stop(!disabled);
        let cursor = window.use_keyed_state(element_id::scoped(&base, "cursor"), cx, |_, _| {
            None::<CursorKey>
        });
        let typed = window.use_keyed_state(element_id::scoped(&base, "typed"), cx, |_, _| {
            crate::list_nav::Typeahead::default()
        });
        let (active, active_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base, "active"),
            self.is_active_controlled.then(|| self.active_key.clone()),
            self.default_active_key.clone(),
        );
        let (collapsed, collapsed_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base, "collapsed"),
            self.is_collapsed,
            self.default_collapsed,
        );
        let default_expanded: HashSet<SharedString> = self
            .groups
            .iter()
            .filter(|g| g.collapses() && g.default_expanded)
            .map(|g| g.key.clone())
            .collect();
        let (expanded, expanded_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base, "expanded"),
            self.is_expansion_controlled
                .then(|| self.expanded_keys.clone()),
            default_expanded,
        );

        let groups = Rc::new(self.groups);
        let stops = if disabled {
            Vec::new()
        } else {
            stops_of(&groups, &expanded, collapsed)
        };
        let has_focus = focus.is_focused(window);
        // The held cursor, or on focus the active item, then the first stop:
        // how React Aria enters a collection.
        let held = cursor
            .read(cx)
            .as_ref()
            .and_then(|key| stops.iter().position(|s| &cursor_key(&groups, s) == key));
        let cursor_at = held.or_else(|| {
            has_focus
                .then(|| {
                    stops
                        .iter()
                        .position(|s| match *s {
                            Stop::Item { group, item } => {
                                Some(&groups[group].items[item].key) == active.as_ref()
                            }
                            Stop::Header { .. } => false,
                        })
                        .or((!stops.is_empty()).then_some(0))
                })
                .flatten()
        });
        let focused_at = (has_focus && window.is_window_active())
            .then_some(cursor_at)
            .flatten();
        let cursor_stop = cursor_at.map(|at| stops[at].clone());

        let state = SidebarState {
            cursor,
            active_own,
            expanded: expanded.clone(),
            expanded_own,
            collapsed,
            collapsed_own,
            on_select: self.on_select.clone(),
            on_collapsed_change: self.on_collapsed_change.clone(),
            on_expanded_change: self.on_expanded_change.clone(),
        };

        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        let radius = util::soft_radius(cx);
        let pointer = util::interactive_cursor(cx);
        let width = if collapsed {
            self.collapsed_width
        } else {
            self.width
        };

        // The menu: groups of items inside one scroller that owns the keys.
        let mut menu = div()
            .id(element_id::scoped(&base, "menu"))
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap(px(12.))
            .p(px(8.))
            .debug_selector({
                let name = format!("{selector}-menu");
                move || name
            });
        if cursor_stop.is_none() {
            menu = menu.track_focus(&focus);
        }
        if !disabled {
            let keys_state = state.clone();
            let keys_groups = groups.clone();
            let keys_focus = focus.clone();
            let keys_stops = stops;
            menu = menu
                .key_context("Sidebar")
                .on_mouse_down(MouseButton::Left, {
                    let focus = focus.clone();
                    move |_, window, cx| window.focus(&focus, cx)
                })
                .on_key_down(move |event, window, cx| {
                    if !keys_focus.is_focused(window) {
                        return;
                    }
                    let m = &event.keystroke.modifiers;
                    if m.control || m.alt || m.platform || m.function {
                        return;
                    }
                    let key = event.keystroke.key.as_str();
                    let state = &keys_state;
                    let groups = &keys_groups;
                    let stops = &keys_stops;
                    let moved = |to: usize, cx: &mut App| {
                        state.set_cursor(cursor_key(groups, &stops[to]), cx);
                        util::set_focus_visible(true, cx);
                    };
                    let here = cursor_at.map(|at| &stops[at]);
                    // Every stop is navigable; `list_nav` walks their indices.
                    let positions: Vec<usize> = (0..stops.len()).collect();
                    match (key, here) {
                        ("right" | "left", Some(Stop::Header { group })) => {
                            let group_key = groups[*group].key.clone();
                            state.set_expanded(&group_key, key == "right", window, cx);
                            util::set_focus_visible(true, cx);
                            cx.stop_propagation();
                        }
                        _ => match crate::list_nav::resolve(&positions, cursor_at, key, false) {
                            crate::list_nav::Move::To(to) => {
                                moved(to, cx);
                                cx.stop_propagation();
                            }
                            crate::list_nav::Move::Activate => {
                                // The cursor row owns the focus handle, and
                                // gpui activates a focused clickable element
                                // on Enter and Space itself: its `on_click`
                                // runs, so answering here too would act twice.
                                util::set_focus_visible(true, cx);
                            }
                            crate::list_nav::Move::Ignore => {
                                if m.shift || !crate::list_nav::is_typeahead_key(key) {
                                    return;
                                }
                                let labels: Vec<String> = stops
                                    .iter()
                                    .map(|s| match *s {
                                        Stop::Header { group } => groups[group]
                                            .label
                                            .as_ref()
                                            .map(|l| l.to_string())
                                            .unwrap_or_default(),
                                        Stop::Item { group, item } => {
                                            groups[group].items[item].label.to_string()
                                        }
                                    })
                                    .collect();
                                let now = web_time::Instant::now();
                                let (query, repeat) = typed.update(cx, |t, _| {
                                    let query = t.push(key, now);
                                    (query, t.is_repeat())
                                });
                                if let Some(found) = crate::list_nav::typeahead(
                                    &labels, &positions, cursor_at, &query, repeat,
                                ) {
                                    moved(found, cx);
                                }
                            }
                        },
                    }
                });
        }

        for (g, group) in groups.iter().enumerate() {
            let group_open = expanded.contains(&group.key);
            let collapses = group.collapses() && !collapsed;
            let mut block = div()
                .id(element_id::scoped(
                    &element_id::scoped(&base, "group"),
                    group.key.clone(),
                ))
                .a11y_named(a11y::Role::Group, &a11y::Name::maybe(group.label.clone()))
                .flex()
                .flex_col()
                .gap(px(2.));
            // The heading: text, or a disclosure trigger; a separator rule in
            // icon mode.
            if collapsed {
                if g > 0 {
                    block = block.child(
                        div()
                            .h(layout.border_width)
                            .mx(px(8.))
                            .mb(px(4.))
                            .bg(colors.separator),
                    );
                }
            } else if let Some(label) = &group.label {
                let mut heading = div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .h(px(28.))
                    .px(px(8.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.muted)
                    .child(div().flex_1().min_w_0().truncate().child(label.clone()));
                if collapses {
                    let is_cursor = cursor_stop == Some(Stop::Header { group: g });
                    let chevron_id = element_id::scoped(
                        &element_id::scoped(&base, "chevron"),
                        group.key.clone(),
                    );
                    let mut trigger = div()
                        .id(element_id::scoped(
                            &element_id::scoped(&base, "heading"),
                            group.key.clone(),
                        ))
                        .a11y_named(a11y::Role::Button, &a11y::Name::labelled(label.clone()))
                        .a11y_expanded(group_open)
                        .when(is_cursor, |el| el.a11y_active_descendant())
                        .relative()
                        .rounded(radius)
                        .debug_selector({
                            let name = format!("{selector}-heading-{}", group.key);
                            move || name
                        })
                        .child(
                            heading.child(anim::rotating_indicator(
                                &chevron_id,
                                group_open,
                                gpui::svg()
                                    .size(px(14.))
                                    .path(crate::icons::CHEVRON_DOWN)
                                    .flex_shrink_0()
                                    .text_color(colors.muted),
                                window,
                                cx,
                            )),
                        );
                    if is_cursor {
                        trigger = trigger.track_focus(&focus);
                    }
                    trigger = util::with_focus_ring_overlay(
                        trigger,
                        util::shows_focus_ring(is_cursor && focused_at.is_some(), cx),
                        true,
                        radius,
                        Vec::new(),
                        cx,
                    );
                    if !disabled {
                        let press = state.clone();
                        let focus = focus.clone();
                        let key = group.key.clone();
                        trigger = trigger
                            .cursor(pointer)
                            .hover(|s| s.bg(colors.default.color))
                            .on_click(move |event, window, cx| {
                                util::set_focus_visible(event.is_keyboard(), cx);
                                window.focus(&focus, cx);
                                press.set_cursor(CursorKey::Header(key.clone()), cx);
                                press.set_expanded(&key, !group_open, window, cx);
                            });
                    }
                    if is_cursor {
                        trigger = util::record_focus_bounds(trigger, &focus, window, cx);
                    }
                    block = block.child(trigger);
                } else {
                    heading = heading.debug_selector({
                        let name = format!("{selector}-heading-{}", group.key);
                        move || name
                    });
                    block = block.child(heading);
                }
            }

            let mut items = div().flex().flex_col().gap(px(2.));
            for (i, item) in group.items.iter().enumerate() {
                let is_active = active.as_ref() == Some(&item.key);
                let is_cursor = cursor_stop == Some(Stop::Item { group: g, item: i });
                let item_disabled = disabled || item.is_disabled;
                let fg = if is_active {
                    colors.accent.soft_foreground(colors.foreground)
                } else {
                    colors.foreground
                };
                let hover_bg = colors.default.color;
                let mut row = div()
                    .id(element_id::scoped(
                        &element_id::scoped(&base, "item"),
                        item.key.clone(),
                    ))
                    .a11y_named(a11y::Role::Link, &a11y::Name::labelled(item.label.clone()))
                    .when(is_active, |el| el.a11y_current(a11y::AriaCurrent::Page))
                    .when(is_cursor, |el| el.a11y_active_descendant())
                    .relative()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .h(ITEM_HEIGHT)
                    .flex_shrink_0()
                    .rounded(radius)
                    .text_size(px(14.))
                    .text_color(fg)
                    .when(collapsed, |el| el.w(px(40.)).justify_center())
                    .when(!collapsed, |el| el.w_full().px(px(8.)))
                    .when(is_active, |el| el.bg(colors.accent.soft()))
                    .debug_selector({
                        let name = format!("{selector}-item-{}", item.key);
                        move || name
                    });
                if is_cursor {
                    row = row.track_focus(&focus);
                }
                let icon = item.icon.clone().unwrap_or_else(|| IconName::Circle.into());
                if item.icon.is_some() || collapsed {
                    row = row.child(
                        gpui::svg()
                            .size(util::FIELD_ICON)
                            .flex_shrink_0()
                            .path(icon)
                            .text_color(fg),
                    );
                }
                if !collapsed {
                    row = row.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(item.label.clone()),
                    );
                    if let Some(badge) = &item.badge {
                        row = row.child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(12.))
                                .text_color(colors.muted)
                                .child(badge.clone()),
                        );
                    }
                }
                row = util::with_focus_ring_overlay(
                    row,
                    util::shows_focus_ring(is_cursor && focused_at.is_some(), cx),
                    true,
                    radius,
                    Vec::new(),
                    cx,
                );
                if item_disabled {
                    row = row.opacity(layout.disabled_opacity);
                } else {
                    let press = state.clone();
                    let focus = focus.clone();
                    let key = item.key.clone();
                    row = row
                        .cursor(pointer)
                        .when(!is_active, |el| el.hover(move |s| s.bg(hover_bg)))
                        .on_click(move |event, window, cx| {
                            util::set_focus_visible(event.is_keyboard(), cx);
                            window.focus(&focus, cx);
                            press.set_cursor(CursorKey::Item(key.clone()), cx);
                            press.select(&key, window, cx);
                        });
                }
                if is_cursor {
                    row = util::record_focus_bounds(row, &focus, window, cx);
                }
                let row = if collapsed {
                    Tooltip::new(item.label.clone())
                        .id(element_id::scoped(
                            &element_id::scoped(&base, "tip"),
                            item.key.clone(),
                        ))
                        .placement(TooltipPlacement::Right)
                        .delay(0)
                        .child(row)
                        .into_any_element()
                } else {
                    row.into_any_element()
                };
                items = items.child(row);
            }
            if collapses {
                let panel_id =
                    element_id::scoped(&element_id::scoped(&base, "panel"), group.key.clone());
                if let Some(panel) = anim::collapsible_panel(
                    &panel_id,
                    group_open,
                    div().id(panel_id.clone()),
                    items.into_any_element(),
                    window,
                    cx,
                ) {
                    block = block.child(panel);
                }
            } else {
                block = block.child(items);
            }
            menu = menu.child(block);
        }
        if cursor_stop.is_none() {
            menu = util::record_focus_bounds(menu, &focus, window, cx);
        }

        let mut footer = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .p(px(8.))
            .children(self.footer);
        if self.show_collapse_toggle {
            let toggle_state = state;
            let toggle_label: SharedString = if collapsed {
                "Expand sidebar".into()
            } else {
                "Collapse sidebar".into()
            };
            let mut toggle = crate::Button::new(element_id::scoped(&base, "toggle"))
                .variant(herogpui_core::Variant::Tertiary)
                .is_icon_only(true)
                .size(herogpui_core::Size::Sm)
                // The label is the accessible name; `content` draws the glyph
                // in its place.
                .label(toggle_label)
                .is_disabled(disabled)
                .content({
                    let muted = colors.muted;
                    move |_| {
                        gpui::svg()
                            .size(util::FIELD_ICON)
                            .path(IconName::PanelLeft)
                            .text_color(muted)
                            .into_any_element()
                    }
                });
            toggle =
                toggle.on_press(move |_, window, cx| toggle_state.toggle_collapsed(window, cx));
            footer = footer.child(
                div()
                    .flex()
                    .when(!collapsed, |el| el.justify_end())
                    .when(collapsed, |el| el.justify_center())
                    .child(
                        div()
                            .debug_selector({
                                let name = format!("{selector}-toggle");
                                move || name
                            })
                            .child(toggle),
                    ),
            );
        }

        let root = div()
            .id(base)
            .a11y_named(
                a11y::Role::Navigation,
                &a11y::Name::labelled(self.label.clone()),
            )
            .flex()
            .flex_col()
            .h_full()
            .w(width)
            .flex_shrink_0()
            .overflow_hidden()
            .bg(colors.surface.background)
            .text_color(colors.foreground)
            .border_r(layout.border_width)
            .border_color(colors.separator)
            .when(disabled, |el| el.opacity(layout.disabled_opacity))
            .debug_selector({
                let name = format!("{selector}-sidebar");
                move || name
            })
            .when(!self.header.is_empty(), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .p(px(8.))
                        .overflow_hidden()
                        .children(self.header),
                )
            })
            .child(menu)
            .child(footer);
        util::apply_sx(root, &self.sx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups() -> Vec<SidebarGroup> {
        vec![
            SidebarGroup::new("main").items(vec![
                SidebarItem::new("a", "A"),
                SidebarItem::new("b", "B").is_disabled(true),
            ]),
            SidebarGroup::new("more")
                .label("More")
                .is_collapsible(true)
                .items(vec![SidebarItem::new("c", "C")]),
            // Collapsible without a label cannot collapse.
            SidebarGroup::new("bare")
                .is_collapsible(true)
                .items(vec![SidebarItem::new("d", "D")]),
        ]
    }

    #[test]
    fn stops_skip_disabled_items_and_closed_groups() {
        let open: HashSet<SharedString> = ["more".into()].into_iter().collect();
        assert_eq!(
            stops_of(&groups(), &open, false),
            [
                Stop::Item { group: 0, item: 0 },
                Stop::Header { group: 1 },
                Stop::Item { group: 1, item: 0 },
                Stop::Item { group: 2, item: 0 },
            ]
        );
        assert_eq!(
            stops_of(&groups(), &HashSet::new(), false),
            [
                Stop::Item { group: 0, item: 0 },
                Stop::Header { group: 1 },
                Stop::Item { group: 2, item: 0 },
            ]
        );
    }

    #[test]
    fn icon_mode_lists_every_item_and_no_headers() {
        assert_eq!(
            stops_of(&groups(), &HashSet::new(), true),
            [
                Stop::Item { group: 0, item: 0 },
                Stop::Item { group: 1, item: 0 },
                Stop::Item { group: 2, item: 0 },
            ]
        );
    }
}

crate::util::impl_component_styled!(Sidebar);
