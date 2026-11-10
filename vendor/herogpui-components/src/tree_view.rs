//! `TreeView` and `TreeItem` — a hierarchical list with expandable rows
//! (HeroGPUI extension; HeroUI v3 has no tree, only `Table`'s tree rows).
//!
//! The tree is one tab stop with a roving cursor, like `ListBox`, and uses
//! the same list navigation (`list_nav`) and selection rules (`selection`):
//!
//! - **Navigation.** Up and Down move over the visible rows, skipping
//!   disabled ones; Home and End jump to the first and last. Right on a
//!   collapsed parent expands it and on an expanded one moves to its first
//!   child; Left on an expanded parent collapses it and otherwise moves to the
//!   row's parent. Typing a row's first letters moves to it.
//! - **Expansion.** Pressing a row's disclosure chevron toggles it. With
//!   `selection_mode` `None`, pressing the row itself, Enter or Space toggles
//!   it too.
//! - **Selection.** `SelectionMode::Single` or `Multiple` (the `ListBox`
//!   model: a press, Enter or Space toggles the row; Single replaces).
//!   In `Multiple`, Shift+Up/Down and a Shift press extend the selection
//!   from the anchor (the last row a press or key selected) over the visible
//!   rows between, skipping disabled ones, as React Stately's
//!   `extendSelection` does; Shift+Home/End move the cursor only, as they do
//!   in `ListBox` without Control. Escape clears a non-empty selection unless
//!   empty selection is disallowed. Disabled rows cannot be focused, selected
//!   or expanded.
//! - **Virtualisation.** Rows render through a uniform
//!   [`VirtualList`](crate::VirtualList), so only the rows in view are built:
//!   with [`max_h`](TreeView::max_h), or in a parent that bounds its height,
//!   a tree of thousands of open rows scrolls and builds a screenful. The
//!   keyboard cursor scrolls into view.
//! - **Accessibility.** `Role::Tree` on the root and `Role::TreeItem` on each
//!   row, named by its label, with its level, its position among its
//!   siblings (`aria-posinset` / `aria-setsize`, as React Aria's
//!   `useGridListItem` counts them), `expanded` on parents and `selected`
//!   when the tree selects.
//!
//! Expanded and selected keys are each controlled (`expanded_keys`,
//! `selected_keys`) or uncontrolled (`default_expanded_keys`,
//! `default_selected_keys`), and changes are reported either way.
//!
//! ```
//! use herogpui_components::{SelectionMode, TreeItem, TreeView};
//!
//! let tree = TreeView::new(
//!     "files",
//!     vec![
//!         TreeItem::new("src", "src").children(vec![
//!             TreeItem::new("lib", "lib.rs"),
//!             TreeItem::new("main", "main.rs"),
//!         ]),
//!         TreeItem::new("readme", "README.md"),
//!     ],
//! )
//! .selection_mode(SelectionMode::Single)
//! .default_expanded_keys(["src".into()])
//! .on_selection_change(|keys, _window, _cx| println!("{keys:?}"));
//! ```

use std::collections::HashSet;
use std::sync::Arc;

use gpui::{
    div, prelude::FluentBuilder as _, px, App, ElementId, InteractiveElement, IntoElement,
    MouseButton, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window,
};
use herogpui_core::{element_id, SelectionMode};
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};
use crate::list_box::{extend_selection_range, ListBoxSelectionRange as SelectionRange};
use crate::{icons, util};

type KeysCallback = Arc<dyn Fn(&HashSet<SharedString>, &mut Window, &mut App) + 'static>;

/// The height of one tree row.
const ROW_HEIGHT: gpui::Pixels = px(32.);
/// The space between two rows. Each virtual row slot is a row plus this gap.
const ROW_GAP: gpui::Pixels = px(2.);
/// The tree's inner padding.
const PADDING: gpui::Pixels = px(4.);
/// The indent one level of depth adds.
const INDENT: f32 = 16.;

/// One node of a [`TreeView`]: a row, and the rows nested under it.
#[must_use = "builder methods return a new value; pass it on to its component"]
pub struct TreeItem {
    key: SharedString,
    label: SharedString,
    icon: Option<SharedString>,
    children: Vec<TreeItem>,
    is_disabled: bool,
}

impl TreeItem {
    /// A row `label` identified by `key`, which must be unique in the tree.
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            icon: None,
            children: Vec::new(),
            is_disabled: false,
        }
    }

    /// Appends a child row.
    pub fn child(mut self, item: TreeItem) -> Self {
        self.children.push(item);
        self
    }

    /// Appends child rows. A row with children is a parent: it draws a
    /// disclosure chevron and can expand.
    pub fn children(mut self, items: impl IntoIterator<Item = TreeItem>) -> Self {
        self.children.extend(items);
        self
    }

    /// An SVG path drawn before the label.
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    /// A disabled row renders dimmed and cannot be focused, selected or
    /// expanded. Its children are not disabled by it.
    pub fn is_disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }
}

/// A tree of expandable rows. See the [module docs](self).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct TreeView {
    id: ElementId,
    items: Vec<TreeItem>,
    selection_mode: SelectionMode,
    selected_keys: HashSet<SharedString>,
    is_selection_controlled: bool,
    default_selected_keys: HashSet<SharedString>,
    disallow_empty_selection: bool,
    expanded_keys: HashSet<SharedString>,
    is_expansion_controlled: bool,
    default_expanded_keys: HashSet<SharedString>,
    disabled_keys: HashSet<SharedString>,
    is_disabled: bool,
    on_selection_change: Option<KeysCallback>,
    on_expanded_change: Option<KeysCallback>,
    max_h: Option<gpui::Pixels>,
}

impl TreeView {
    /// A tree over `items`. `id` keys its cursor, expansion and selection;
    /// give every instance its own.
    pub fn new(id: impl Into<ElementId>, items: Vec<TreeItem>) -> Self {
        Self {
            id: id.into(),
            items,
            // `ListBox`'s default, and React Stately's: a plain tree does not
            // select.
            selection_mode: SelectionMode::None,
            selected_keys: HashSet::new(),
            is_selection_controlled: false,
            default_selected_keys: HashSet::new(),
            disallow_empty_selection: false,
            expanded_keys: HashSet::new(),
            is_expansion_controlled: false,
            default_expanded_keys: HashSet::new(),
            disabled_keys: HashSet::new(),
            is_disabled: false,
            on_selection_change: None,
            on_expanded_change: None,
            max_h: None,
        }
    }

    /// Caps the tree's height. Taller content scrolls inside it, and only
    /// the rows in view are built. Without a cap the tree is as tall as its
    /// open rows unless its parent bounds it.
    pub fn max_h(mut self, height: impl Into<gpui::Pixels>) -> Self {
        self.max_h = Some(height.into());
        self
    }

    /// `None` (the default), `Single` or `Multiple`.
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// The controlled selection.
    pub fn selected_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.selected_keys = keys.into_iter().collect();
        self.is_selection_controlled = true;
        self
    }

    /// The initial selection of an uncontrolled tree.
    pub fn default_selected_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.default_selected_keys = keys.into_iter().collect();
        self
    }

    /// Keeps the last selected row from being deselected.
    pub fn disallow_empty_selection(mut self, v: bool) -> Self {
        self.disallow_empty_selection = v;
        self
    }

    /// The controlled set of expanded parents.
    pub fn expanded_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.expanded_keys = keys.into_iter().collect();
        self.is_expansion_controlled = true;
        self
    }

    /// The parents expanded at first in an uncontrolled tree.
    pub fn default_expanded_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.default_expanded_keys = keys.into_iter().collect();
        self
    }

    /// Rows that behave as [`TreeItem::is_disabled`].
    pub fn disabled_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.disabled_keys = keys.into_iter().collect();
        self
    }

    /// A disabled tree renders dimmed, leaves the tab order and ignores
    /// input.
    pub fn is_disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }

    /// Reports the selection after a press, Enter, Space or Escape changed it.
    pub fn on_selection_change(
        mut self,
        f: impl Fn(&HashSet<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Arc::new(f));
        self
    }

    /// Reports the expanded set after a chevron, a key or a press changed it.
    pub fn on_expanded_change(
        mut self,
        f: impl Fn(&HashSet<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_expanded_change = Some(Arc::new(f));
        self
    }
}

/// One visible row after flattening.
#[derive(Clone)]
struct Row {
    key: SharedString,
    label: SharedString,
    icon: Option<SharedString>,
    depth: usize,
    has_children: bool,
    expanded: bool,
    /// The parent's index in the flattened rows.
    parent: Option<usize>,
    disabled: bool,
    /// The zero-based position among the row's siblings.
    pos_in_set: usize,
    /// How many siblings the row has, itself included.
    set_size: usize,
}

/// Depth-first, through open parents only: a nested row does not exist
/// until its parent is expanded (the same walk as `Table`'s tree rows).
fn flatten(
    items: Vec<TreeItem>,
    depth: usize,
    parent: Option<usize>,
    expanded: &HashSet<SharedString>,
    disabled_keys: &HashSet<SharedString>,
    out: &mut Vec<Row>,
) {
    let set_size = items.len();
    for (pos_in_set, item) in items.into_iter().enumerate() {
        let has_children = !item.children.is_empty();
        let is_open = has_children && expanded.contains(&item.key);
        let index = out.len();
        out.push(Row {
            disabled: item.is_disabled || disabled_keys.contains(&item.key),
            key: item.key,
            label: item.label,
            icon: item.icon,
            depth,
            has_children,
            expanded: is_open,
            parent,
            pos_in_set,
            set_size,
        });
        if is_open {
            flatten(
                item.children,
                depth + 1,
                Some(index),
                expanded,
                disabled_keys,
                out,
            );
        }
    }
}

/// The keys of `key`'s ancestors, nearest first, or `None` when `key` is
/// not in the tree.
fn ancestors_of(items: &[TreeItem], key: &SharedString) -> Option<Vec<SharedString>> {
    for item in items {
        if &item.key == key {
            return Some(Vec::new());
        }
        if let Some(mut path) = ancestors_of(&item.children, key) {
            path.push(item.key.clone());
            return Some(path);
        }
    }
    None
}

/// The selection after activating `key`: `ListBox`'s rule.
fn toggled_selection(
    current: &HashSet<SharedString>,
    key: &SharedString,
    mode: SelectionMode,
    disallow_empty: bool,
) -> HashSet<SharedString> {
    let order: Vec<SharedString> = current.iter().cloned().collect();
    crate::selection::next_selection(&order, key, mode, disallow_empty)
        .into_iter()
        .collect()
}

/// What the event closures share, cloned into each.
#[derive(Clone)]
struct TreeState {
    cursor: gpui::Entity<Option<SharedString>>,
    selected: HashSet<SharedString>,
    selected_own: Option<gpui::Entity<HashSet<SharedString>>>,
    expanded: HashSet<SharedString>,
    expanded_own: Option<gpui::Entity<HashSet<SharedString>>>,
    mode: SelectionMode,
    disallow_empty: bool,
    on_selection_change: Option<KeysCallback>,
    on_expanded_change: Option<KeysCallback>,
    /// The Shift range's anchor and moving end.
    range: gpui::Entity<SelectionRange>,
    /// The visible rows' keys, in order: the collection a range spans.
    order: std::rc::Rc<Vec<SharedString>>,
    /// The keys a range may select: the visible rows that are not disabled.
    selectable: std::rc::Rc<HashSet<SharedString>>,
}

impl TreeState {
    fn set_cursor(&self, key: &SharedString, cx: &mut App) {
        self.cursor.update(cx, |value, cx| {
            if value.as_ref() != Some(key) {
                *value = Some(key.clone());
                cx.notify();
            }
        });
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

    fn set_selected(&self, next: HashSet<SharedString>, window: &mut Window, cx: &mut App) {
        if next == self.selected {
            return;
        }
        if let Some(own) = &self.selected_own {
            own.update(cx, |value, cx| {
                value.clone_from(&next);
                cx.notify();
            });
        }
        if let Some(cb) = &self.on_selection_change {
            cb(&next, window, cx);
        }
    }

    /// A Shift press or Shift+Up/Down onto `key` in multiple mode: select
    /// the selectable rows from the anchor to `key`, replacing the previous
    /// range, and move the range's end to `key`.
    fn extend_to(&self, key: &SharedString, window: &mut Window, cx: &mut App) {
        let range = self.range.read(cx).clone();
        let next =
            extend_selection_range(&self.selected, &self.order, &self.selectable, &range, key);
        self.range.update(cx, |range, _| {
            if range.anchor.is_none() {
                range.anchor = Some(key.clone());
            }
            range.current = Some(key.clone());
            range.is_all = false;
        });
        self.set_selected(next, window, cx);
    }

    /// Whether a Shift navigation or press extends the selection.
    fn extends(&self, shift: bool) -> bool {
        shift && self.mode == SelectionMode::Multiple
    }

    /// A press, Enter or Space on `row`: select it, or with no selection
    /// mode toggle a parent. Selecting a row in multiple mode seats the
    /// Shift range's anchor on it, as `ListBox` does.
    fn activate(&self, row: &Row, window: &mut Window, cx: &mut App) {
        if row.disabled {
            return;
        }
        if crate::selection::reports_changes(self.mode) {
            let was_selected = self.selected.contains(&row.key);
            let next = toggled_selection(&self.selected, &row.key, self.mode, self.disallow_empty);
            if self.mode == SelectionMode::Multiple && !was_selected {
                self.range.update(cx, |range, _| {
                    *range = SelectionRange {
                        anchor: Some(row.key.clone()),
                        current: Some(row.key.clone()),
                        is_all: false,
                    };
                });
            }
            self.set_selected(next, window, cx);
        } else if row.has_children {
            self.set_expanded(&row.key, !row.expanded, window, cx);
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

impl RenderOnce for TreeView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = self.id.clone();
        let selector = selector_base(&base);
        let focus_handle = window
            .use_keyed_state(element_id::scoped(&base, "focus"), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone()
            .tab_stop(!self.is_disabled);
        let cursor = window.use_keyed_state(element_id::scoped(&base, "cursor"), cx, |_, _| {
            None::<SharedString>
        });
        let typed = window.use_keyed_state(element_id::scoped(&base, "typed"), cx, |_, _| {
            crate::list_nav::Typeahead::default()
        });
        let range = window.use_keyed_state(element_id::scoped(&base, "range"), cx, |_, _| {
            SelectionRange::default()
        });
        let list = window
            .use_keyed_state(element_id::scoped(&base, "list"), cx, |_, _| {
                crate::VirtualListHandle::uniform(0)
            })
            .read(cx)
            .clone();
        let (selected, selected_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base, "selected"),
            self.is_selection_controlled
                .then(|| self.selected_keys.clone()),
            self.default_selected_keys.clone(),
        );
        let (expanded, expanded_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base, "expanded"),
            self.is_expansion_controlled
                .then(|| self.expanded_keys.clone()),
            self.default_expanded_keys.clone(),
        );

        // A cursor row hidden under a collapsed ancestor (a controlled
        // `expanded_keys` can collapse one without a key reaching the tree)
        // moves to its nearest visible ancestor, where a Left collapse would
        // have left it.
        let cursor_ancestors = cursor
            .read(cx)
            .as_ref()
            .and_then(|key| ancestors_of(&self.items, key))
            .unwrap_or_default();
        let mut rows = Vec::new();
        flatten(
            self.items,
            0,
            None,
            &expanded,
            &self.disabled_keys,
            &mut rows,
        );
        let disabled_tree = self.is_disabled;
        let stops: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| !row.disabled && !disabled_tree)
            .map(|(ix, _)| ix)
            .collect();

        // The cursor is a key, so it survives rows opening and closing above
        // it. One hidden by a collapsed ancestor moves to the nearest visible
        // ancestor; one that is otherwise no longer a visible stop falls
        // back, on focus, to the first selected row and then the first row,
        // as React Aria enters a collection.
        let has_focus = focus_handle.is_focused(window);
        let held = cursor
            .read(cx)
            .as_ref()
            .and_then(|key| rows.iter().position(|row| &row.key == key))
            .filter(|ix| stops.contains(ix))
            .or_else(|| {
                let ancestor = cursor_ancestors.iter().find_map(|key| {
                    rows.iter()
                        .position(|row| &row.key == key)
                        .filter(|ix| stops.contains(ix))
                })?;
                let key = rows[ancestor].key.clone();
                cursor.update(cx, |value, _| *value = Some(key));
                Some(ancestor)
            });
        let cursor_at = held.or_else(|| {
            has_focus
                .then(|| {
                    stops
                        .iter()
                        .copied()
                        .find(|ix| selected.contains(&rows[*ix].key))
                        .or_else(|| stops.first().copied())
                })
                .flatten()
        });
        let focused_at = (has_focus && window.is_window_active())
            .then_some(cursor_at)
            .flatten();

        let state = TreeState {
            cursor,
            selected: selected.clone(),
            selected_own,
            expanded,
            expanded_own,
            mode: self.selection_mode,
            disallow_empty: self.disallow_empty_selection,
            on_selection_change: self.on_selection_change.clone(),
            on_expanded_change: self.on_expanded_change.clone(),
            range,
            order: std::rc::Rc::new(rows.iter().map(|row| row.key.clone()).collect()),
            selectable: std::rc::Rc::new(stops.iter().map(|ix| rows[*ix].key.clone()).collect()),
        };
        let rows = std::rc::Rc::new(rows);
        if list.item_count() != rows.len() {
            list.splice(0..list.item_count(), rows.len());
        }

        let colors = cx.colors().clone();
        let radius = util::soft_radius(cx);
        let pointer = util::interactive_cursor(cx);
        let disabled_opacity = cx.layout().disabled_opacity;
        let selects = self.selection_mode != SelectionMode::None;

        let mut tree = div()
            .id(base.clone())
            .a11y(a11y::Role::Tree)
            .relative()
            .flex()
            .flex_col()
            .text_size(util::FIELD_TEXT)
            .text_color(colors.foreground)
            .debug_selector({
                let name = format!("{selector}-tree");
                move || name
            });

        if disabled_tree {
            tree = tree.opacity(disabled_opacity);
        } else {
            let keys_state = state.clone();
            let keys_rows = rows.clone();
            let keys_focus = focus_handle.clone();
            let keys_list = list.clone();
            let keys_stops = stops;
            tree = tree
                .track_focus(&focus_handle)
                .key_context("TreeView")
                .on_mouse_down(MouseButton::Left, {
                    let focus = focus_handle.clone();
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
                    let rows = &keys_rows;
                    let state = &keys_state;
                    let from = cursor_at;
                    let moved = |to: usize, cx: &mut App| {
                        state.set_cursor(&rows[to].key, cx);
                        keys_list.scroll_to_item(to, crate::VirtualListScroll::Reveal);
                        util::set_focus_visible(true, cx);
                    };
                    match key {
                        "right" => {
                            let Some(ix) = from else { return };
                            let row = &rows[ix];
                            if row.has_children && !row.expanded {
                                state.set_expanded(&row.key, true, window, cx);
                                util::set_focus_visible(true, cx);
                            } else if row.has_children {
                                let child = ix + 1;
                                if rows.get(child).is_some_and(|c| c.parent == Some(ix))
                                    && keys_stops.contains(&child)
                                {
                                    moved(child, cx);
                                }
                            }
                            cx.stop_propagation();
                        }
                        "left" => {
                            let Some(ix) = from else { return };
                            let row = &rows[ix];
                            if row.has_children && row.expanded {
                                state.set_expanded(&row.key, false, window, cx);
                                util::set_focus_visible(true, cx);
                            } else if let Some(parent) =
                                row.parent.filter(|p| keys_stops.contains(p))
                            {
                                moved(parent, cx);
                            }
                            cx.stop_propagation();
                        }
                        "escape"
                            if !m.shift
                                && crate::selection::reports_changes(state.mode)
                                && !state.disallow_empty
                                && !state.selected.is_empty() =>
                        {
                            state
                                .range
                                .update(cx, |range, _| *range = SelectionRange::default());
                            state.set_selected(HashSet::new(), window, cx);
                            cx.stop_propagation();
                        }
                        _ => match crate::list_nav::resolve(&keys_stops, from, key, false) {
                            crate::list_nav::Move::To(to) => {
                                moved(to, cx);
                                // Shift+Up/Down extend a multiple selection;
                                // Shift+Home/End only move, as in `ListBox`
                                // without Control.
                                if state.extends(m.shift)
                                    && matches!(key, "up" | "down")
                                    && Some(to) != from
                                {
                                    state.extend_to(&rows[to].key, window, cx);
                                }
                                cx.stop_propagation();
                            }
                            crate::list_nav::Move::Activate => {
                                if let Some(ix) = from {
                                    util::set_focus_visible(true, cx);
                                    state.activate(&rows[ix], window, cx);
                                }
                                cx.stop_propagation();
                            }
                            crate::list_nav::Move::Ignore => {
                                if m.shift || !crate::list_nav::is_typeahead_key(key) {
                                    return;
                                }
                                let labels: Vec<String> =
                                    rows.iter().map(|row| row.label.to_string()).collect();
                                let now = web_time::Instant::now();
                                let (query, repeat) = typed.update(cx, |t, _| {
                                    let query = t.push(key, now);
                                    (query, t.is_repeat())
                                });
                                if let Some(found) = crate::list_nav::typeahead(
                                    &labels,
                                    &keys_stops,
                                    from,
                                    &query,
                                    repeat,
                                ) {
                                    moved(found, cx);
                                }
                            }
                        },
                    }
                });
        }

        // Only the rows in view are built. Each slot is a row plus the gap
        // below it, so a slot's top is where a flex column with that gap
        // would put the row; the bottom padding is one gap short to match.
        let content_height = PADDING * 2. + (ROW_HEIGHT + ROW_GAP) * rows.len() as f32 - ROW_GAP;
        let rows_id = element_id::scoped(&base, "rows");
        let render_row = {
            move |ix: usize, _window: &mut Window, cx: &mut App| {
                let row = &rows[ix];
                let disabled = row.disabled || disabled_tree;
                let is_selected = selects && selected.contains(&row.key);
                let is_cursor = cursor_at == Some(ix);
                let fg = if is_selected {
                    colors.accent.soft_foreground(colors.foreground)
                } else {
                    colors.foreground
                };
                let hover_bg = colors.default.color;
                let mut el = div()
                    .id(element_id::scoped(
                        &element_id::scoped(&base, "item"),
                        row.key.clone(),
                    ))
                    .a11y_named(
                        a11y::Role::TreeItem,
                        &a11y::Name::labelled(row.label.clone()),
                    )
                    .a11y_level(row.depth)
                    .a11y_set_position(row.pos_in_set, row.set_size)
                    .when(row.has_children, |el| el.a11y_expanded(row.expanded))
                    .when(selects, |el| el.a11y_selected(is_selected))
                    // One focus handle on the tree and a cursor through its
                    // rows: gpui states that relation on the row, not the
                    // container.
                    .when(is_cursor, |el| el.a11y_active_descendant())
                    .relative()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .h(ROW_HEIGHT)
                    .flex_shrink_0()
                    .pl(px(4. + INDENT * row.depth as f32))
                    .pr(px(8.))
                    .rounded(radius)
                    .text_color(fg)
                    .when(is_selected, |el| el.bg(colors.accent.soft()))
                    .debug_selector({
                        let name = format!("{selector}-row-{}", row.key);
                        move || name
                    });
                if disabled {
                    el = el.opacity(disabled_opacity);
                } else {
                    el = el
                        .cursor(pointer)
                        .when(!is_selected, |el| el.hover(move |s| s.bg(hover_bg)));
                }
                el = util::with_focus_ring_overlay(
                    el,
                    util::shows_focus_ring(focused_at == Some(ix), cx),
                    true,
                    radius,
                    Vec::new(),
                    cx,
                );

                // The disclosure slot: a chevron on a parent, an empty slot on
                // a leaf so siblings line up.
                let mut chevron = div()
                    .id(element_id::scoped(
                        &element_id::scoped(&base, "toggle"),
                        row.key.clone(),
                    ))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(16.))
                    .flex_shrink_0();
                if row.has_children {
                    chevron = chevron
                        .debug_selector({
                            let name = format!("{selector}-toggle-{}", row.key);
                            move || name
                        })
                        .child(
                            gpui::svg()
                                .size(px(12.))
                                .path(if row.expanded {
                                    icons::CHEVRON_DOWN
                                } else {
                                    icons::CHEVRON_RIGHT
                                })
                                .text_color(colors.muted),
                        );
                    if !disabled {
                        let toggle = state.clone();
                        let focus = focus_handle.clone();
                        let key = row.key.clone();
                        let open = row.expanded;
                        chevron = chevron
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                util::set_focus_visible(false, cx);
                                window.focus(&focus, cx);
                                toggle.set_cursor(&key, cx);
                                toggle.set_expanded(&key, !open, window, cx);
                            });
                    }
                }
                el = el.child(chevron);
                if let Some(path) = &row.icon {
                    el = el.child(
                        gpui::svg()
                            .size(util::FIELD_ICON)
                            .path(path.clone())
                            .flex_shrink_0()
                            .text_color(fg),
                    );
                }
                el = el.child(div().flex_1().min_w_0().truncate().child(row.label.clone()));

                if !disabled {
                    let press = state.clone();
                    let press_rows = rows.clone();
                    el = el.on_click(move |event, window, cx| {
                        let row = &press_rows[ix];
                        util::set_focus_visible(false, cx);
                        press.set_cursor(&row.key, cx);
                        if press.extends(event.modifiers().shift) {
                            press.extend_to(&row.key, window, cx);
                        } else {
                            press.activate(row, window, cx);
                        }
                    });
                }
                if cursor_at == Some(ix) && focus_handle.is_focused(_window) {
                    el = util::record_focus_bounds(el, &focus_handle, _window, cx);
                }
                div()
                    .w_full()
                    .h(ROW_HEIGHT + ROW_GAP)
                    .child(el)
                    .into_any_element()
            }
        };
        let rows_list = crate::VirtualList::new(rows_id, &list, render_row).padding(
            PADDING,
            PADDING,
            PADDING - ROW_GAP,
        );
        let rows_list = match self.max_h {
            Some(max_h) => rows_list.height(max_h.min(content_height)),
            None => rows_list,
        };
        tree.child(rows_list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Vec<TreeItem> {
        vec![
            TreeItem::new("a", "A").children(vec![
                TreeItem::new("a1", "A1"),
                TreeItem::new("a2", "A2").child(TreeItem::new("a2x", "A2x")),
            ]),
            TreeItem::new("b", "B").is_disabled(true),
        ]
    }

    fn keys(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|row| row.key.as_ref()).collect()
    }

    #[test]
    fn flatten_walks_open_parents_only() {
        let mut rows = Vec::new();
        flatten(tree(), 0, None, &HashSet::new(), &HashSet::new(), &mut rows);
        assert_eq!(keys(&rows), ["a", "b"]);

        let open: HashSet<SharedString> = ["a".into(), "a2".into()].into_iter().collect();
        let mut rows = Vec::new();
        flatten(tree(), 0, None, &open, &HashSet::new(), &mut rows);
        assert_eq!(keys(&rows), ["a", "a1", "a2", "a2x", "b"]);
        assert_eq!(
            rows.iter().map(|r| r.depth).collect::<Vec<_>>(),
            [0, 1, 1, 2, 0]
        );
        assert_eq!(rows[3].parent, Some(2));
        assert!(rows[4].disabled);
        assert!(rows[0].expanded && !rows[1].has_children);
    }

    #[test]
    fn rows_count_their_position_among_siblings() {
        let open: HashSet<SharedString> = ["a".into(), "a2".into()].into_iter().collect();
        let mut rows = Vec::new();
        flatten(tree(), 0, None, &open, &HashSet::new(), &mut rows);
        // a (1 of 2), a1 (1 of 2), a2 (2 of 2), a2x (1 of 1), b (2 of 2).
        assert_eq!(
            rows.iter()
                .map(|r| (r.pos_in_set, r.set_size))
                .collect::<Vec<_>>(),
            [(0, 2), (0, 2), (1, 2), (0, 1), (1, 2)]
        );
    }

    #[test]
    fn an_open_key_on_a_leaf_is_not_expanded() {
        let open: HashSet<SharedString> = ["a1".into()].into_iter().collect();
        let mut rows = Vec::new();
        flatten(tree(), 0, None, &open, &HashSet::new(), &mut rows);
        assert!(rows.iter().all(|row| !row.expanded));
    }
}
