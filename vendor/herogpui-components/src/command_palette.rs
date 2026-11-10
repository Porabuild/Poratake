//! `CommandPalette` — a modal command search (HeroGPUI extension; HeroUI v3
//! has no command palette).
//!
//! The palette is a dialog over a scrim, on the same overlay stack as
//! `Modal`: it takes the focus as it opens (into its search field), keeps Tab
//! inside, closes on Escape or a press outside while it is the topmost
//! overlay, and hands the focus back to whatever held it when it closes.
//! Inside:
//!
//! - **Search.** Typing filters the commands. Every whitespace-separated word
//!   of the query must occur in a command's label, one of its `keywords` or
//!   its group name, compared by `useFilter`'s default `base` sensitivity
//!   (case and accents ignored). The filtered list is cached across frames
//!   (`matches.rs`) while the query and the commands stay unchanged.
//! - **Groups.** Commands keep their order and are grouped under their
//!   `group` heading, groups in the order they first appear.
//! - **Keyboard.** The search field keeps the focus; Up and Down move a
//!   highlighted command (wrapping, skipping disabled ones), Enter runs it,
//!   Escape closes. A new query highlights the first match. The pointer
//!   highlights what it hovers and a press runs it.
//! - **Shortcuts** are displayed as `Kbd` keys beside a command; the palette
//!   does not bind them.
//! - **Empty state.** With no match, `empty_text` (the `NoResults` UI string by
//!   default) takes the list's place.
//!
//! Open state is controlled (`is_open` + `on_open_change`) or uncontrolled
//! (`default_open`). To open it from anywhere with Cmd-K (Ctrl-K off macOS),
//! check [`is_command_palette_shortcut`] in a root `on_key_down`, or bind a
//! GPUI action to the `"secondary-k"` keystroke:
//!
//! ```
//! use gpui::{div, prelude::*, Context, Window};
//! use herogpui_components::{is_command_palette_shortcut, CommandItem, CommandPalette};
//!
//! struct App {
//!     palette_open: bool,
//! }
//!
//! impl gpui::Render for App {
//!     fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!         div()
//!             .size_full()
//!             .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
//!                 if is_command_palette_shortcut(&event.keystroke, cx) {
//!                     this.palette_open = !this.palette_open;
//!                     cx.notify();
//!                 }
//!             }))
//!             .child(
//!                 CommandPalette::new(
//!                     "palette",
//!                     vec![
//!                         CommandItem::new("new", "New file").group("File").shortcut(["⌘", "N"]),
//!                         CommandItem::new("theme", "Toggle theme").keywords(["dark", "light"]),
//!                     ],
//!                 )
//!                 .is_open(self.palette_open)
//!                 .on_open_change(cx.listener(|this, open: &bool, _, cx| {
//!                     this.palette_open = *open;
//!                     cx.notify();
//!                 }))
//!                 .on_select(|key, _, _| println!("run {key}")),
//!             )
//!     }
//! }
//! ```
//!
//! **Accessibility.** The panel is `Role::Dialog` named by `label`, the
//! results are `Role::ListBox` and each command `Role::ListBoxOption` named by
//! its label, with the highlighted one `selected` and marked as the active
//! descendant of the focused search field — the shape cmdk and React Aria's
//! Autocomplete give a search-driven list with virtual focus.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    div, prelude::FluentBuilder as _, px, App, ElementId, Entity, Focusable as _,
    InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use herogpui_core::element_id;
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};
use crate::input::{Input, InputState};
use crate::matches::MatchesCache;
use crate::picker_item::PickerItem;
use crate::{anim, util, Filter, Kbd, Sensitivity};

type SelectCallback = Arc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;
type OpenChange = Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;
type CommandFilter = Rc<dyn Fn(&CommandItem, &str) -> bool + 'static>;

/// The height of one command row.
const ROW_HEIGHT: Pixels = px(40.);
/// The height of a group heading.
const HEADING_HEIGHT: Pixels = px(28.);

/// Whether `keystroke` is the conventional command-palette shortcut: Cmd-K on
/// macOS, Ctrl-K elsewhere (GPUI's `secondary` modifier), with no other
/// modifier held.
pub fn is_command_palette_shortcut(keystroke: &gpui::Keystroke, cx: &App) -> bool {
    let m = &keystroke.modifiers;
    keystroke.key == "k" && m.secondary(cx) && !m.alt && !m.shift && !(m.control && m.platform)
}

/// One command of a [`CommandPalette`].
#[must_use]
#[derive(Clone)]
pub struct CommandItem {
    key: SharedString,
    label: SharedString,
    group: Option<SharedString>,
    description: Option<SharedString>,
    icon: Option<SharedString>,
    shortcut: Vec<SharedString>,
    keywords: Vec<SharedString>,
    is_disabled: bool,
}

impl CommandItem {
    /// A command `label` identified by `key`, which [`CommandPalette::on_select`]
    /// reports. Keys must be unique in the palette.
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            group: None,
            description: None,
            icon: None,
            shortcut: Vec::new(),
            keywords: Vec::new(),
            is_disabled: false,
        }
    }

    /// The heading this command is listed under.
    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// A secondary line drawn after the label, in the muted colour.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// An icon before the label: an [`crate::IconName`] or any served SVG path.
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// The keys of the command's shortcut, each drawn as one `Kbd`
    /// (`["⌘", "N"]`). Display only.
    pub fn shortcut(mut self, keys: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.shortcut = keys.into_iter().map(Into::into).collect();
        self
    }

    /// Extra words the search matches besides the label and the group.
    pub fn keywords(mut self, words: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.keywords = words.into_iter().map(Into::into).collect();
        self
    }

    /// A disabled command is listed dimmed and cannot be highlighted or run.
    pub fn is_disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }

    /// The command's key.
    pub fn key(&self) -> &SharedString {
        &self.key
    }

    /// The command's label.
    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// Everything the default search reads, joined by newlines so a query
    /// word cannot match across two fields.
    fn search_text(&self) -> String {
        let mut text = self.label.to_string();
        for part in self.keywords.iter().chain(self.group.iter()) {
            text.push('\n');
            text.push_str(part);
        }
        text
    }
}

/// Whether `item` matches `query` under the default search.
fn default_matches(filter: &Filter, text: &str, query: &str) -> bool {
    query
        .split_whitespace()
        .all(|word| text.split('\n').any(|field| filter.contains(field, word)))
}

/// A modal command search. See the [module docs](self).
#[must_use]
#[derive(IntoElement)]
pub struct CommandPalette {
    id: ElementId,
    items: Vec<CommandItem>,
    is_open: Option<bool>,
    default_open: bool,
    placeholder: Option<SharedString>,
    empty_text: Option<SharedString>,
    label: SharedString,
    width: Pixels,
    max_list_height: Pixels,
    close_on_select: bool,
    filter: Option<CommandFilter>,
    on_select: Option<SelectCallback>,
    on_open_change: Option<OpenChange>,
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl CommandPalette {
    /// A palette over `items`. `id` keys its state; give every instance its
    /// own.
    pub fn new(id: impl Into<ElementId>, items: Vec<CommandItem>) -> Self {
        Self {
            id: id.into(),
            items,
            is_open: None,
            default_open: false,
            placeholder: None,
            empty_text: None,
            label: SharedString::new_static("Command palette"),
            width: px(560.),
            max_list_height: px(320.),
            close_on_select: true,
            filter: None,
            on_select: None,
            on_open_change: None,
            sx: None,
        }
    }

    /// The controlled open state.
    pub fn is_open(mut self, open: bool) -> Self {
        self.is_open = Some(open);
        self
    }

    /// The initial open state of an uncontrolled palette.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// The search field's placeholder; the `Search` UI string by default.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// What the list shows when nothing matches; the `NoResults` UI string by
    /// default.
    pub fn empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.empty_text = Some(text.into());
        self
    }

    /// The dialog's accessible name; "Command palette" by default.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// The panel's width; 560px by default.
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = width.into();
        self
    }

    /// The results list's maximum height before it scrolls; 320px by default.
    pub fn max_list_height(mut self, height: impl Into<Pixels>) -> Self {
        self.max_list_height = height.into();
        self
    }

    /// Whether running a command closes the palette; `true` by default.
    pub fn close_on_select(mut self, close: bool) -> Self {
        self.close_on_select = close;
        self
    }

    /// Replaces the default search: `filter(item, query)` decides whether a
    /// command is listed. An empty query lists everything either way.
    pub fn filter(mut self, filter: impl Fn(&CommandItem, &str) -> bool + 'static) -> Self {
        self.filter = Some(Rc::new(filter));
        self
    }

    /// Runs with the chosen command's key.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Arc::new(f));
        self
    }

    /// Reports every open or close the palette asks for.
    pub fn on_open_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_change = Some(Arc::new(f));
        self
    }

    /// Caller-owned styling refined over the panel after the theme's values.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }
}

/// One line of the rendered list: a heading or a command (by index into the
/// palette's items).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Line {
    Heading(usize),
    Command(usize),
}

/// The matched commands grouped under their headings, in first-appearance
/// order, as the flat line list the scroller renders.
fn group_lines(items: &[CommandItem], matched: &[usize]) -> (Vec<Line>, Vec<SharedString>) {
    let mut groups: Vec<Option<SharedString>> = Vec::new();
    let mut members: Vec<Vec<usize>> = Vec::new();
    for &ix in matched {
        let group = items[ix].group.clone();
        let at = match groups.iter().position(|g| *g == group) {
            Some(at) => at,
            None => {
                groups.push(group);
                members.push(Vec::new());
                groups.len() - 1
            }
        };
        members[at].push(ix);
    }
    let mut lines = Vec::new();
    let mut headings = Vec::new();
    for (group, rows) in groups.into_iter().zip(members) {
        if let Some(name) = group {
            lines.push(Line::Heading(headings.len()));
            headings.push(name);
        }
        lines.extend(rows.into_iter().map(Line::Command));
    }
    (lines, headings)
}

/// The open-state writer every close path shares.
#[derive(Clone)]
struct Opener {
    own: Option<Entity<bool>>,
    on_open_change: Option<OpenChange>,
}

impl Opener {
    fn set(&self, open: bool, window: &mut Window, cx: &mut App) {
        if let Some(own) = &self.own {
            own.update(cx, |value, cx| {
                *value = open;
                cx.notify();
            });
        }
        if let Some(cb) = &self.on_open_change {
            cb(&open, window, cx);
        }
    }
}

impl RenderOnce for CommandPalette {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = self.id.clone();
        let (open, own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base, "open"),
            self.is_open,
            self.default_open,
        );
        let (phase, token) =
            util::overlay_scope(window, cx, element_id::scoped(&base, "phase"), open, true);
        if phase == util::OverlayPhase::Closed {
            crate::modal::release_dialog_focus(&base, window, cx);
            return div().into_any_element();
        }
        let exiting = phase == util::OverlayPhase::Exiting;
        let opener = Opener {
            own,
            on_open_change: self.on_open_change.clone(),
        };

        // The query lives only while the palette is on screen, so every
        // opening starts from an empty search.
        let query_state =
            window.use_keyed_state(element_id::scoped(&base, "query"), cx, |_, cx| {
                InputState::new(cx)
            });
        let cursor = window.use_keyed_state(element_id::scoped(&base, "cursor"), cx, |_, _| {
            None::<SharedString>
        });
        let last_query =
            window.use_keyed_state(element_id::scoped(&base, "last-query"), cx, |_, _| {
                None::<String>
            });
        let scroll = window
            .use_keyed_state(element_id::scoped(&base, "scroll"), cx, |_, _| {
                ScrollHandle::new()
            })
            .read(cx)
            .clone();
        let cache = window.use_keyed_state(element_id::scoped(&base, "matches"), cx, |_, _| {
            MatchesCache::default()
        });
        let input_focus = query_state.read(cx).focus_handle(cx);
        // Claimed on every frame the palette is on screen, the exit included,
        // so the remembered return target survives until the close.
        crate::modal::claim_dialog_focus(&base, &input_focus, window, cx);

        // Match. The default search goes through the shared cache, keyed by
        // every command's key and search text; a custom filter cannot join a
        // cache key and runs every frame.
        let query = query_state.read(cx).value().to_owned();
        let items = Rc::new(self.items);
        let matched: Vec<usize> = if query.trim().is_empty() {
            (0..items.len()).collect()
        } else if let Some(filter) = &self.filter {
            (0..items.len())
                .filter(|&ix| filter(&items[ix], &query))
                .collect()
        } else {
            let index: HashMap<SharedString, usize> = items
                .iter()
                .enumerate()
                .map(|(ix, item)| (item.key.clone(), ix))
                .collect();
            let pickers: Rc<[PickerItem]> = items
                .iter()
                .map(|item| PickerItem::new(item.key.clone(), item.search_text()))
                .collect::<Vec<_>>()
                .into();
            let hits = cache.update(cx, |cache, _| {
                cache.get(pickers, &query, usize::MAX, |all| {
                    let filter = Filter::new(Sensitivity::Base);
                    all.iter()
                        .filter(|p| default_matches(&filter, p.label(), &query))
                        .cloned()
                        .collect()
                })
            });
            hits.iter()
                .filter_map(|p| index.get(p.key()).copied())
                .collect()
        };
        let (lines, headings) = group_lines(&items, &matched);
        let stops: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter_map(|(at, line)| match line {
                Line::Command(ix) if !items[*ix].is_disabled => Some(at),
                _ => None,
            })
            .collect();

        // A new query highlights the first match, as cmdk does; otherwise the
        // highlight is a key and follows its command through re-filtering.
        let query_changed = last_query.read(cx).as_deref() != Some(query.as_str());
        if query_changed {
            last_query.update(cx, |q, _| *q = Some(query.clone()));
        }
        let held_line = cursor.read(cx).as_ref().and_then(|key| {
            lines.iter().position(|line| match line {
                Line::Command(ix) => &items[*ix].key == key,
                Line::Heading(_) => false,
            })
        });
        let cursor_line = if query_changed {
            None
        } else {
            held_line.filter(|at| stops.contains(at))
        }
        .or_else(|| stops.first().copied());
        let cursor_key = cursor_line.and_then(|at| match lines[at] {
            Line::Command(ix) => Some(items[ix].key.clone()),
            Line::Heading(_) => None,
        });
        if *cursor.read(cx) != cursor_key {
            cursor.update(cx, |c, _| *c = cursor_key.clone());
        }
        if query_changed {
            if let Some(at) = cursor_line {
                scroll.scroll_to_item(at);
            }
        }

        let run = util::shared({
            let opener = opener.clone();
            let on_select = self.on_select.clone();
            let items = items.clone();
            let close_on_select = self.close_on_select;
            move |ix: usize, window: &mut Window, cx: &mut App| {
                let item = &items[ix];
                if item.is_disabled {
                    return;
                }
                if close_on_select {
                    opener.set(false, window, cx);
                }
                if let Some(cb) = &on_select {
                    cb(&item.key, window, cx);
                }
            }
        });
        let dismiss = util::shared({
            let opener = opener;
            move |window: &mut Window, cx: &mut App| {
                opener.set(false, window, cx);
                util::DismissResult::Handled
            }
        });

        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        let radius = util::container_radius(cx);
        let row_radius = util::soft_radius(cx);
        let pointer = util::interactive_cursor(cx);
        let placeholder = self
            .placeholder
            .clone()
            .unwrap_or_else(|| crate::i18n::ui_string(crate::i18n::UiString::Search, cx));
        let empty_text = self
            .empty_text
            .clone()
            .unwrap_or_else(|| crate::i18n::ui_string(crate::i18n::UiString::NoResults, cx));

        let search = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .h(px(48.))
            .border_b(layout.border_width)
            .border_color(colors.separator)
            .child(
                gpui::svg()
                    .size(util::FIELD_ICON)
                    .flex_shrink_0()
                    .path(crate::icons::SEARCH)
                    .text_color(colors.muted),
            )
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&query_state)
                        .placeholder(placeholder)
                        .is_bare(true)
                        .focus_ring(false)
                        .full_width(),
                ),
            );

        let mut list = div()
            .id(element_id::scoped(&base, "list"))
            .a11y_named(
                a11y::Role::ListBox,
                &a11y::Name::labelled(self.label.clone()),
            )
            .track_scroll(&scroll)
            .overflow_y_scroll()
            .max_h(self.max_list_height)
            .flex()
            .flex_col()
            .p(px(6.))
            .debug_selector({
                let name = format!("{}-list", selector_base(&base));
                move || name
            });
        if lines.is_empty() {
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(64.))
                    .text_color(colors.muted)
                    .debug_selector({
                        let name = format!("{}-empty", selector_base(&base));
                        move || name
                    })
                    .child(empty_text),
            );
        }
        for (at, line) in lines.iter().enumerate() {
            match *line {
                Line::Heading(h) => {
                    list = list.child(
                        div()
                            .flex()
                            .items_end()
                            .h(HEADING_HEIGHT)
                            .flex_shrink_0()
                            .px(px(8.))
                            .pb(px(4.))
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(colors.muted)
                            .child(headings[h].clone()),
                    );
                }
                Line::Command(ix) => {
                    let item = &items[ix];
                    let highlighted = cursor_line == Some(at);
                    let mut row = div()
                        .id(element_id::scoped(
                            &element_id::scoped(&base, "item"),
                            item.key.clone(),
                        ))
                        .a11y_named(
                            a11y::Role::ListBoxOption,
                            &a11y::Name::labelled(item.label.clone())
                                .described(item.description.clone()),
                        )
                        .a11y_selected(highlighted)
                        .when(highlighted, |el| el.a11y_active_descendant())
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .h(ROW_HEIGHT)
                        .flex_shrink_0()
                        .px(px(8.))
                        .rounded(row_radius)
                        .text_color(colors.foreground)
                        .when(highlighted, |el| el.bg(colors.default.color))
                        .debug_selector({
                            let name = format!("{}-item-{}", selector_base(&base), item.key);
                            move || name
                        });
                    if let Some(icon) = &item.icon {
                        row = row.child(
                            gpui::svg()
                                .size(util::FIELD_ICON)
                                .flex_shrink_0()
                                .path(icon.clone())
                                .text_color(colors.muted),
                        );
                    }
                    row = row.child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(8.))
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().child(item.label.clone()))
                            .when_some(item.description.clone(), |el, description| {
                                el.child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(px(12.))
                                        .text_color(colors.muted)
                                        .child(description),
                                )
                            }),
                    );
                    if !item.shortcut.is_empty() {
                        row = row.child(
                            div().flex().gap(px(4.)).flex_shrink_0().children(
                                item.shortcut
                                    .iter()
                                    .map(|key| Kbd::new().child(key.clone())),
                            ),
                        );
                    }
                    if item.is_disabled {
                        row = row.opacity(layout.disabled_opacity);
                    } else {
                        let hover_cursor = cursor.clone();
                        let key = item.key.clone();
                        let press = run.clone();
                        row = row
                            .cursor(pointer)
                            .on_hover(move |over, _, cx| {
                                if *over {
                                    hover_cursor.update(cx, |c, cx| {
                                        if c.as_ref() != Some(&key) {
                                            *c = Some(key.clone());
                                            cx.notify();
                                        }
                                    });
                                }
                            })
                            .on_click(move |_, window, cx| press(ix, window, cx));
                    }
                    list = list.child(row);
                }
            }
        }

        let keys_lines = Rc::new(lines);
        let keys_cursor = cursor.clone();
        let keys_scroll = scroll;
        let keys_items = items;
        let keys_run = run.clone();
        let panel = div()
            .id(element_id::scoped(&base, "dialog"))
            .a11y_named(
                a11y::Role::Dialog,
                &a11y::Name::labelled(self.label.clone()),
            )
            .relative()
            .flex()
            .flex_col()
            .w(self.width)
            .max_w_full()
            .bg(colors.overlay.background)
            .text_color(colors.foreground)
            .text_size(px(14.))
            .line_height(px(20.))
            .rounded(radius)
            .overflow_hidden()
            .when_some(layout.overlay_hairline, |el, hairline| {
                el.border(layout.border_width).border_color(hairline)
            })
            .shadow(layout.overlay_shadow)
            .debug_selector({
                let name = format!("{}-panel", selector_base(&base));
                move || name
            })
            .on_key_down(move |event, window, cx| {
                let m = &event.keystroke.modifiers;
                if m.control || m.alt || m.platform || m.shift || m.function {
                    return;
                }
                let key = event.keystroke.key.as_str();
                let from = keys_cursor.read(cx).as_ref().and_then(|held| {
                    keys_lines.iter().position(|line| match line {
                        Line::Command(ix) => &keys_items[*ix].key == held,
                        Line::Heading(_) => false,
                    })
                });
                match key {
                    "up" | "down" => {
                        let target = match crate::list_nav::resolve(&stops, from, key, true) {
                            crate::list_nav::Move::To(to) => Some(to),
                            _ => None,
                        };
                        if let Some(to) = target {
                            if let Line::Command(ix) = keys_lines[to] {
                                let key = keys_items[ix].key.clone();
                                keys_cursor.update(cx, |c, cx| {
                                    *c = Some(key);
                                    cx.notify();
                                });
                                keys_scroll.scroll_to_item(to);
                            }
                        }
                        cx.stop_propagation();
                    }
                    "enter" => {
                        if let Some(Line::Command(ix)) = from.map(|at| keys_lines[at]) {
                            keys_run(ix, window, cx);
                        }
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            })
            .child(search)
            .child(list);
        let panel = util::apply_sx(panel, &self.sx);
        let panel = if exiting {
            panel
        } else {
            util::dismiss_on_press_outside_with_token(panel, token.clone(), {
                let dismiss = dismiss.clone();
                move |window, cx| dismiss(window, cx)
            })
        };

        let focus_scope = window
            .use_keyed_state(element_id::scoped(&base, "scope"), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        let mut overlay = util::trap_tab(
            div()
                .id(element_id::scoped(&base, "overlay"))
                .track_focus(&focus_scope),
            &focus_scope,
        )
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .pt(window.viewport_size().height * 0.15)
        .px(px(16.));
        overlay = util::dismiss_on_escape_with_token(overlay, token, move |window, cx| {
            dismiss(window, cx)
        });
        let scrim = div()
            .id(element_id::scoped(&base, "backdrop"))
            .absolute()
            .inset_0()
            .bg(colors.backdrop);
        overlay = overlay.child(if exiting {
            anim::exiting(
                scrim,
                element_id::scoped(&base, "backdrop-out"),
                anim::ZoomBox::default(),
                anim::Motion::BACKDROP_OUT,
                cx,
            )
        } else {
            anim::entering(
                scrim,
                element_id::scoped(&base, "backdrop-in"),
                anim::Motion::BACKDROP_IN,
                cx,
            )
        });
        let zoom = anim::ZoomBox {
            width: Some(self.width),
            radius: Some(radius),
            ..Default::default()
        };
        overlay = overlay.child(if exiting {
            anim::exiting(
                panel,
                element_id::scoped(&base, "panel-out"),
                zoom,
                anim::Motion::PANEL_OUT,
                cx,
            )
        } else {
            anim::entering_zoom(
                panel,
                element_id::scoped(&base, "panel-in"),
                zoom,
                anim::Motion::PANEL_IN,
                cx,
            )
        });
        util::window_overlay(overlay, window).into_any_element()
    }
}

/// The debug-selector spelling of an id: its name when it has one.
fn selector_base(id: &ElementId) -> String {
    match id {
        ElementId::Name(name) => name.to_string(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<CommandItem> {
        vec![
            CommandItem::new("new", "New file").group("File"),
            CommandItem::new("theme", "Toggle theme").keywords(["dark", "light"]),
            CommandItem::new("open", "Open file").group("File"),
            CommandItem::new("close", "Close window").group("Window"),
        ]
    }

    #[test]
    fn groups_keep_first_appearance_order_and_ungrouped_rows_have_no_heading() {
        let items = items();
        let (lines, headings) = group_lines(&items, &[0, 1, 2, 3]);
        assert_eq!(headings, ["File", "Window"]);
        assert_eq!(
            lines,
            [
                Line::Heading(0),
                Line::Command(0),
                Line::Command(2),
                Line::Command(1),
                Line::Heading(1),
                Line::Command(3),
            ]
        );
    }

    #[test]
    fn every_query_word_must_match_a_field() {
        let filter = Filter::new(Sensitivity::Base);
        let theme = items()[1].search_text();
        assert!(default_matches(&filter, &theme, "toggle DARK"));
        assert!(!default_matches(&filter, &theme, "toggle window"));
        let new = items()[0].search_text();
        assert!(default_matches(&filter, &new, "file new"));
        // Words do not match across the label/keyword boundary.
        assert!(!default_matches(&filter, &theme, "themedark"));
    }

    #[test]
    fn the_shortcut_is_secondary_k_alone() {
        let mut cx = gpui::TestAppContext::single();
        for (os, target, platform_chord, other_chord) in [
            (
                gpui::OperatingSystem::Mac,
                gpui::PlatformTarget::Native,
                "cmd-k",
                "ctrl-k",
            ),
            (
                gpui::OperatingSystem::Windows,
                gpui::PlatformTarget::Web,
                "ctrl-k",
                "cmd-k",
            ),
        ] {
            cx.set_platform_info(gpui::PlatformInfo { os, target });
            cx.update(|cx| {
                let parse = |s: &'static str| gpui::Keystroke::parse_with_os(s, os).unwrap();
                assert!(
                    is_command_palette_shortcut(&parse("secondary-k"), cx),
                    "{os:?}/{target:?}"
                );
                assert!(
                    is_command_palette_shortcut(&parse(platform_chord), cx),
                    "{os:?}/{target:?}"
                );
                assert!(
                    !is_command_palette_shortcut(&parse(other_chord), cx),
                    "{os:?}/{target:?}"
                );
                assert!(!is_command_palette_shortcut(&parse("k"), cx));
                assert!(!is_command_palette_shortcut(
                    &parse("secondary-shift-k"),
                    cx
                ));
                assert!(!is_command_palette_shortcut(&parse("secondary-j"), cx));
            });
        }
    }
}

crate::util::impl_component_styled!(CommandPalette);
