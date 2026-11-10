//! Table — port of `@heroui/table` (v3).
//!
//! `variant` selects between the surface container (`primary`) and the flat,
//! transparent-row layout (`secondary`).
//!
//! Selection and sorting are controlled, as they are in v3: the table reports
//! what the user asked for and the caller owns the data. In particular the
//! table never reorders its own rows — it cannot, since a row is a list of
//! rendered cells, not a record.

use gpui::{
    prelude::*, px, AnyElement, App, ClickEvent, InteractiveElement, IntoElement, ParentElement,
    Pixels, RenderOnce, SharedString, Styled, Window,
};
use herogpui_core::{element_id, SelectionBehavior, SelectionMode};
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    checkbox::Checkbox,
    icons,
};

type OnRowClick = std::sync::Arc<dyn Fn(&usize, &ClickEvent, &mut Window, &mut App) + 'static>;
type OnSelectionChange = std::sync::Arc<dyn Fn(&[SharedString], &mut Window, &mut App) + 'static>;
type OnSortChange = std::sync::Arc<dyn Fn(&SortDescriptor, &mut Window, &mut App) + 'static>;
type OnLoadMore = std::sync::Arc<dyn Fn(&mut Window, &mut App) + 'static>;
type OnResize = std::sync::Arc<dyn Fn(&[(SharedString, Pixels)], &mut Window, &mut App) + 'static>;
type VirtualRowKey = std::sync::Arc<dyn Fn(usize) -> SharedString + 'static>;
type VirtualRow = std::sync::Arc<dyn Fn(usize) -> TableRow + 'static>;
type VirtualRowText = std::sync::Arc<dyn Fn(usize) -> SharedString + 'static>;
type VirtualTree = std::sync::Arc<dyn Fn(usize) -> VirtualTreeMetadata + 'static>;

const DEFAULT_COLUMN_MIN_WIDTH: f32 = 75.;

/// Visual variant of a table (`variant`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TableVariant {
    /// A surface container behind the rows.
    #[default]
    Primary,
    /// Flat, with transparent rows and no container.
    Secondary,
}

impl TableVariant {
    /// Every table variant, in display order.
    pub const ALL: [TableVariant; 2] = [TableVariant::Primary, TableVariant::Secondary];

    /// The variant's display name.
    pub fn label(self) -> &'static str {
        match self {
            TableVariant::Primary => "Primary",
            TableVariant::Secondary => "Secondary",
        }
    }
}

/// `sortDirection` on `Table.SortableColumnHeader`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    /// Sort from lowest to highest.
    Ascending,
    /// Sort from highest to lowest.
    Descending,
}

impl SortDirection {
    /// The direction a second click on the same column asks for.
    pub fn flipped(self) -> Self {
        match self {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        }
    }
}

type Indicator = std::sync::Arc<dyn Fn(SortDirection) -> AnyElement + 'static>;

/// `sortDescriptor` — which column is sorted, and which way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortDescriptor {
    /// Key of the sorted column.
    pub column: SharedString,
    /// Direction of the sort.
    pub direction: SortDirection,
}

impl SortDescriptor {
    /// Creates a descriptor for `column` sorted in `direction`.
    pub fn new(column: impl Into<SharedString>, direction: SortDirection) -> Self {
        Self {
            column: column.into(),
            direction,
        }
    }

    /// The descriptor a click on `column` should produce.
    ///
    /// Clicking the sorted column flips it; clicking any other column starts
    /// ascending, which is what React Aria does.
    pub fn next(current: Option<&SortDescriptor>, column: impl Into<SharedString>) -> Self {
        let column = column.into();
        match current {
            Some(d) if d.column == column => Self {
                column,
                direction: d.direction.flipped(),
            },
            _ => Self {
                column,
                direction: SortDirection::Ascending,
            },
        }
    }
}

/// One column (`Table.Column`).
#[must_use = "builder methods return a new value; pass the column to its table"]
#[derive(Clone, Debug)]
pub struct TableColumn {
    label: SharedString,
    allows_sorting: bool,
    is_row_header: bool,
    /// `allowsResizing` — whether a handle on this column's trailing edge
    /// resizes it.
    allows_resizing: bool,
    /// `width` — a caller-owned controlled column width.
    width: Option<Pixels>,
    /// `defaultWidth` — the uncontrolled starting width. Without one the
    /// column shares the row evenly, which is what `flex-1` does.
    default_width: Option<Pixels>,
    /// `minWidth` — the column's floor.
    min_width: Option<Pixels>,
    /// `maxWidth` — the column's ceiling.
    max_width: Option<Pixels>,
    /// Frozen at the leading edge (HeroGPUI extension).
    frozen: bool,
}

impl TableColumn {
    /// Creates a column with the given header label.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            allows_sorting: false,
            is_row_header: false,
            allows_resizing: false,
            width: None,
            default_width: None,
            min_width: None,
            max_width: None,
            frozen: false,
        }
    }

    /// `allowsResizing` — put a drag handle on this column's trailing edge.
    pub fn allows_resizing(mut self, v: bool) -> Self {
        self.allows_resizing = v;
        self
    }

    /// `defaultWidth` — the column's starting width, which a resize handle then
    /// moves.
    pub fn default_width(mut self, width: impl Into<Pixels>) -> Self {
        self.default_width = Some(width.into());
        self
    }

    /// `width` — the caller-owned controlled width of this column.
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Some(width.into());
        self
    }

    /// `minWidth` — the width this column will not go below.
    pub fn min_width(mut self, width: impl Into<Pixels>) -> Self {
        self.min_width = Some(width.into());
        self
    }

    /// `maxWidth` — the width this column will not exceed.
    pub fn max_width(mut self, width: impl Into<Pixels>) -> Self {
        self.max_width = Some(width.into());
        self
    }

    /// `allowsSorting` — makes the header a sort control.
    pub fn allows_sorting(mut self, v: bool) -> Self {
        self.allows_sorting = v;
        self
    }

    /// `isRowHeader` — this column names the row.
    ///
    /// With no accessibility layer to expose it to, the effect here is visual:
    /// the column's cells carry medium weight so the identifying value reads
    /// first.
    pub fn is_row_header(mut self, v: bool) -> Self {
        self.is_row_header = v;
        self
    }

    /// Freezes this column at the table's leading edge (HeroGPUI extension,
    /// after gpui-kit's `fixed_left` columns; not a HeroUI v3 prop): when the
    /// table is wider than its box, the frozen columns (and the selection
    /// column) stay put while the other columns scroll horizontally beneath
    /// the header, which scrolls with them.
    ///
    /// Frozen columns are always displayed first, in their relative order, so
    /// a `column_order` that interleaves them is shown with the frozen ones
    /// moved to the front, and a column move never crosses from the frozen
    /// region into the scrolling one or back: unfreeze the column to move it
    /// there. A frozen column is sized at its explicit, resized or measured
    /// width rather than sharing out the free space. When every column is
    /// frozen nothing scrolls and the table renders as an ordinary one.
    pub fn frozen(mut self, frozen: bool) -> Self {
        self.frozen = frozen;
        self
    }
}

impl From<SharedString> for TableColumn {
    fn from(label: SharedString) -> Self {
        TableColumn::new(label)
    }
}

impl From<&str> for TableColumn {
    fn from(label: &str) -> Self {
        TableColumn::new(label.to_owned())
    }
}

/// One row (`Table.Row`).
#[must_use = "builder methods return a new value; pass it on to its component"]
pub struct TableRow {
    key: Option<SharedString>,
    text_value: Option<SharedString>,
    cells: Vec<AnyElement>,
    /// The rows nested under this one. v3 calls them the row's `children`, and
    /// `expandedKeys` decides which parents show theirs.
    children: Vec<TableRow>,
}

/// Cheap tree structure for one preorder item in a virtual table.
///
/// The row factory still runs only for items the viewport builds. This metadata
/// is projected for the whole collection so expansion can derive the visible
/// indices without eagerly constructing any cells.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VirtualTreeMetadata {
    /// Nesting depth of the item; zero for a root.
    pub depth: usize,
    /// Key of the item's parent, or `None` for a root.
    pub parent_key: Option<SharedString>,
    /// Whether the item has children.
    pub has_children: bool,
}

impl TableRow {
    /// Creates a row from its cells, with no key, text value or children.
    pub fn new(cells: Vec<AnyElement>) -> Self {
        Self {
            key: None,
            text_value: None,
            cells,
            children: Vec::new(),
        }
    }

    /// The rows nested under this one.
    pub fn children(mut self, rows: Vec<TableRow>) -> Self {
        self.children = rows;
        self
    }

    /// The selection key. Defaults to the row's index.
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// The plain-text value used by the table's typeahead.
    ///
    /// Cells are opaque `AnyElement`s, so rows without this value do not
    /// participate in typeahead even when a cell visibly draws text.
    pub fn text_value(mut self, value: impl Into<SharedString>) -> Self {
        self.text_value = Some(value.into());
        self
    }
}

type OnExpandedChange = std::sync::Arc<dyn Fn(&[SharedString], &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowActivation {
    Pointer,
    Enter,
    Space,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowIntent {
    Action,
    Selection,
    None,
}

#[derive(Clone, Debug, Default)]
struct TableTypeahead {
    query: String,
    last: Option<web_time::Instant>,
}

#[derive(Clone, Debug, Default)]
struct TableSelectionRange {
    anchor: Option<SharedString>,
    current: Option<SharedString>,
    is_all: bool,
}

impl TableTypeahead {
    fn is_active(&self, now: web_time::Instant) -> bool {
        !self.query.is_empty()
            && self
                .last
                .is_some_and(|last| now.duration_since(last) <= crate::list_nav::TYPEAHEAD_TIMEOUT)
    }

    fn push(&mut self, key: &str, now: web_time::Instant) -> String {
        if self
            .last
            .is_none_or(|last| now.duration_since(last) > crate::list_nav::TYPEAHEAD_TIMEOUT)
        {
            self.query.clear();
        }
        self.last = Some(now);
        self.query.push_str(key);
        self.query.clone()
    }

    fn clear(&mut self) {
        self.query.clear();
        self.last = None;
    }
}

#[derive(Clone)]
struct TableTypeaheadNavigation {
    state: gpui::Entity<TableTypeahead>,
    labels: Vec<String>,
    virtual_indices: Option<Vec<usize>>,
    virtual_text: Option<VirtualRowText>,
    stops: Vec<usize>,
    keys: Vec<SharedString>,
    cursor: gpui::Entity<Option<SharedString>>,
    fixed_virtual: bool,
    fixed_scroll: crate::VirtualListHandle,
    variable_scroll: Option<gpui::ListState>,
}

impl TableTypeaheadNavigation {
    fn push(
        &self,
        character: &str,
        now: web_time::Instant,
        clear_on_failure: bool,
        cx: &mut App,
    ) -> bool {
        let from = self
            .cursor
            .read(cx)
            .as_ref()
            .and_then(|key| self.keys.iter().position(|row| row == key))
            .filter(|index| self.stops.contains(index));
        let mut query = String::new();
        self.state.update(cx, |state, _| {
            query = state.push(character, now);
        });
        let projected_labels = self
            .virtual_text
            .as_ref()
            .zip(self.virtual_indices.as_ref())
            .map(|(text_value, indices)| {
                indices
                    .iter()
                    .map(|index| text_value(*index).to_string())
                    .collect::<Vec<_>>()
            });
        let searchable_labels = projected_labels.as_ref().unwrap_or(&self.labels);
        let Some(next) =
            crate::list_nav::typeahead(searchable_labels, &self.stops, from, &query, false)
        else {
            if clear_on_failure {
                self.state.update(cx, |state, _| state.clear());
            }
            return false;
        };
        self.cursor.update(cx, |value, cx| {
            *value = self.keys.get(next).cloned();
            cx.notify();
        });
        if self.fixed_virtual {
            self.fixed_scroll
                .scroll_to_item(next, crate::VirtualListScroll::Center);
        } else if let Some(state) = &self.variable_scroll {
            state.scroll_to(gpui::ListOffset {
                item_ix: next,
                offset_in_item: px(0.),
            });
        }
        true
    }
}

#[derive(Clone, PartialEq, Eq)]
enum LoadMoreCollection {
    Rows(Vec<SharedString>),
    Virtual {
        count: usize,
        identity: SharedString,
    },
}

fn row_intent(
    activation: RowActivation,
    mode: SelectionMode,
    behavior: SelectionBehavior,
    selection_is_empty: bool,
    has_action: bool,
) -> RowIntent {
    // React Aria's replace behavior reserves a row's plain pointer activation
    // for selection whenever the row is selectable; its action becomes a
    // secondary (double-click/Enter) action. Toggle keeps the existing
    // single-click action while the collection is empty.
    let primary_action = has_action
        && (mode == SelectionMode::None
            || (behavior == SelectionBehavior::Toggle && selection_is_empty));
    match activation {
        RowActivation::Pointer if primary_action => RowIntent::Action,
        RowActivation::Pointer if mode != SelectionMode::None => RowIntent::Selection,
        RowActivation::Enter if primary_action => RowIntent::Action,
        RowActivation::Enter if !has_action && mode != SelectionMode::None => RowIntent::Selection,
        RowActivation::Space if mode != SelectionMode::None => RowIntent::Selection,
        _ => RowIntent::None,
    }
}

fn select_all_flags(selectable: &[SharedString], selected: &[SharedString]) -> (bool, bool) {
    let selected_count = selectable
        .iter()
        .filter(|key| selected.contains(key))
        .count();
    let all_selected = !selectable.is_empty() && selected_count == selectable.len();
    let indeterminate = !selected.is_empty() && !all_selected;
    (all_selected, indeterminate)
}

fn same_selection(left: &[SharedString], right: &[SharedString]) -> bool {
    left.len() == right.len()
        && left.iter().collect::<std::collections::HashSet<_>>()
            == right.iter().collect::<std::collections::HashSet<_>>()
}

fn extend_selection_range(
    current: &[SharedString],
    collection: &[SharedString],
    selectable: &[SharedString],
    range: &TableSelectionRange,
    target: &SharedString,
) -> Vec<SharedString> {
    if range.is_all {
        return vec![target.clone()];
    }
    let anchor = range.anchor.as_ref().unwrap_or(target);
    let previous = range.current.as_ref().unwrap_or(target);
    let mut anchor_at = None;
    let mut previous_at = None;
    let mut target_at = None;
    for (index, key) in collection.iter().enumerate() {
        anchor_at = anchor_at.or_else(|| (key == anchor).then_some(index));
        previous_at = previous_at.or_else(|| (key == previous).then_some(index));
        target_at = target_at.or_else(|| (key == target).then_some(index));
    }
    let between = |from: Option<usize>, to: Option<usize>| {
        from.zip(to)
            .map(|(from, to)| if from <= to { from..=to } else { to..=from })
    };
    let mut next = current.to_vec();
    if let Some(previous_range) = between(anchor_at, previous_at) {
        let removed: std::collections::HashSet<&SharedString> =
            previous_range.map(|index| &collection[index]).collect();
        next.retain(|key| !removed.contains(key));
    }
    if let Some(target_range) = between(anchor_at, target_at) {
        let selectable: std::collections::HashSet<&SharedString> = selectable.iter().collect();
        let mut selected: std::collections::HashSet<SharedString> = next.iter().cloned().collect();
        for index in target_range {
            let key = &collection[index];
            if selectable.contains(key) && selected.insert(key.clone()) {
                next.push(key.clone());
            }
        }
    }
    next
}

/// Pinned React Aria 3.51.0 `useSelectableCollection` registers Home and End
/// only for the chords each platform's handler admits: none, Shift, Alt, and
/// Alt+Shift on macOS -- no Meta or Control handler exists -- and none,
/// Shift, Control, and Control+Shift on Windows and Linux. The upstream
/// matcher reads exactly the browser's canonical modifier flags -- Alt,
/// Control, Meta, Shift -- so GPUI's `function` flag is ignored here: a
/// browser exposes no Fn state for it to read, so vetoing on the flag would
/// claim a pinned guard that does not exist, and the framework delivers an
/// Fn-bearing press with every matched modifier flag still false. A chord
/// outside the registration is entirely inert: no focus move, no selection,
/// no preventDefault. `macos` is simulated explicitly so every platform's
/// unit tests can prove both maps.
fn home_end_registered(modifiers: gpui::Modifiers, macos: bool) -> bool {
    if macos {
        !modifiers.control && !modifiers.platform
    } else {
        !modifiers.alt && !modifiers.platform
    }
}

/// Pinned `useSelectableCollection` (`isCtrlKeyPressed`): a Shift move
/// extends the range on the collection's navigation keys, while Home and
/// End extend only from Control+Shift on Windows and Linux. macOS registers
/// no Home/End extension at all -- its Shift and Alt+Shift chords move the
/// focus alone -- so the platform is an explicit bool rather than a `cfg!`.
fn shift_home_end_extends(key_name: &str, control: bool, macos: bool) -> bool {
    !matches!(key_name, "home" | "end") || (!macos && control)
}

/// A column with a `defaultWidth` takes it; the rest split what is left.
///
/// `flex_basis(0)` is the part that matters: a bare `flex_1` sizes a cell by its
/// content, so the tree column's indent shifted every column after it, and a
/// fraction of the *whole* row cannot account for the fixed columns.
fn flex_cell(el: gpui::Div) -> gpui::Div {
    el.flex_basis(px(0.)).flex_1()
}

/// Frozen leading columns (HeroGPUI extension, see [`TableColumn::frozen`]):
/// how many display positions are frozen, and the one horizontal scroll
/// state every row's scrolling part and the header's share.
///
/// GPUI has no sticky positioning, so each row (and the header) is split in
/// two: a frozen part that never moves and a clipped part that scrolls. The
/// row element stays the parent of both, so its hover group, press,
/// selection and keyboard cursor span the whole row, and a cell scrolled out
/// of view is clipped by its part's content mask rather than hidden under a
/// frozen cell, so a press on a frozen cell cannot reach it.
#[derive(Clone)]
struct FrozenColumns {
    /// The frozen display positions, `0..count`; always fewer than the
    /// columns, since a table with every column frozen does not split.
    count: usize,
    scroll: gpui::ScrollHandle,
    /// The view to notify when a wheel over a frozen part scrolls.
    view: gpui::EntityId,
}

impl FrozenColumns {
    /// The frozen part of a row or of the header. A horizontal wheel over it
    /// scrolls the rest of the table, the way it does over the scrolling
    /// part.
    fn frozen_part(&self) -> gpui::Div {
        let scroll = self.scroll.clone();
        let view = self.view;
        gpui::div()
            .flex()
            .flex_shrink_0()
            .on_scroll(move |event, window, cx| {
                let at = scroll.offset();
                crate::util::shift_wheel_scroll_x(&scroll, event, window);
                let delta = event.delta.pixel_delta(window.line_height());
                let x_delta = if delta.x != px(0.) {
                    delta.x
                } else {
                    scroll.offset().x - at.x
                };
                if x_delta == px(0.) {
                    return;
                }
                let x = (at.x + x_delta).clamp(-scroll.max_offset().x, px(0.));
                if x != at.x {
                    scroll.set_offset(gpui::point(x, at.y));
                    cx.notify(view);
                }
            })
    }

    /// The scrolling part: its cells are direct children, so the shared
    /// handle's child bounds are the cells' and `scroll_to_item` can reveal
    /// one. `restrict_scroll_to_axis` leaves a vertical wheel to the body.
    fn scrolling_part(&self, part: gpui::Div, id: gpui::ElementId) -> gpui::Stateful<gpui::Div> {
        part.flex()
            .flex_1()
            .min_w_0()
            .id(id)
            .overflow_x_scroll()
            .track_scroll(&self.scroll)
            .restrict_scroll_to_axis()
    }

    /// Reveals the cell at display position `column` when it scrolls.
    fn reveal(&self, column: usize) {
        if column >= self.count {
            self.scroll.scroll_to_item(column - self.count);
        }
    }
}

/// `to` held inside the region `from` is in, so a column move never crosses
/// between the frozen and the scrolling columns.
fn clamp_to_region(from: usize, to: usize, frozen: usize) -> usize {
    if frozen == 0 {
        to
    } else if from < frozen {
        to.min(frozen - 1)
    } else {
        to.max(frozen)
    }
}

/// `order` with the frozen columns first, each region keeping its order.
fn frozen_first(order: &[usize], columns: &[TableColumn]) -> Vec<usize> {
    let frozen = |ix: &usize| columns.get(*ix).is_some_and(|c| c.frozen);
    order
        .iter()
        .copied()
        .filter(frozen)
        .chain(order.iter().copied().filter(|ix| !frozen(ix)))
        .collect()
}

/// The pinned table stylesheet replaces the tree column's start padding with
/// one `1rem` step for every 1-based row level. `flatten_tree` stores the root
/// at depth zero, so a root keeps the regular 16px cell padding and a child at
/// depth `d` resolves to `16px * (d + 1)`. Keeping this in one helper avoids
/// the intrinsic-width probe and the painted cell drifting apart at deeper
/// levels.
fn tree_column_padding(depth: usize) -> Option<Pixels> {
    (depth > 0).then(|| px(16. * (depth as f32 + 1.)))
}

/// The horizontal extent of the direct children that a cell lays out. GPUI
/// reports absolute child bounds, so taking their union keeps a label and a
/// sort indicator together while ignoring an empty cell. One-pixel separator
/// overlays are deliberately excluded; they are decorative and must not turn
/// into a column's intrinsic width.
#[allow(clippy::float_cmp)] // f32::MAX is the unset sentinel, checked exactly
fn intrinsic_children_width(bounds: &[gpui::Bounds<Pixels>]) -> f32 {
    let mut left = f32::MAX;
    let mut right = f32::MIN;
    for bound in bounds.iter().filter(|bound| bound.size.width > px(2.)) {
        left = left.min(f32::from(bound.origin.x));
        right = right.max(f32::from(bound.origin.x + bound.size.width));
    }
    if left == f32::MAX {
        0.0
    } else {
        (right - left).max(0.0)
    }
}

/// Paints the split inset ring HeroUI uses for a focused row cell.
///
/// A single rounded overlay on the row makes the focus geometry look plausible
/// until a cell has its own clipping/background: the outer corners and the
/// shared vertical edges then bleed into neighboring cells. Keeping the four
/// strips on each cell reproduces the stylesheet's first/middle/last cases
/// without changing cell layout or intercepting the row's click handler.
fn table_cell_focus_ring(cx: &App, first: bool, last: bool) -> gpui::Div {
    let color = cx.colors().focus;
    let radius = crate::util::key_radius(cx);
    let mut top = gpui::div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(2.))
        .bg(color);
    let mut bottom = gpui::div()
        .absolute()
        .bottom_0()
        .left_0()
        .right_0()
        .h(px(2.))
        .bg(color);
    if first {
        top = top.rounded_tl(radius);
        bottom = bottom.rounded_bl(radius);
    }
    if last {
        top = top.rounded_tr(radius);
        bottom = bottom.rounded_br(radius);
    }
    let mut ring = gpui::div().absolute().inset_0().child(top).child(bottom);
    if first {
        ring = ring.child(
            gpui::div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(px(2.))
                .rounded_tl(radius)
                .rounded_bl(radius)
                .bg(color),
        );
    }
    if last {
        ring = ring.child(
            gpui::div()
                .absolute()
                .top_0()
                .bottom_0()
                .right_0()
                .w(px(2.))
                .rounded_tr(radius)
                .rounded_br(radius)
                .bg(color),
        );
    }
    ring
}

/// HeroUI Table.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Table {
    /// Distinguishes one table's keyed state from another's: the resized column
    /// widths, the drag in progress, the focus handle and the row cursor all
    /// hang off it. v3 needs no such prop -- React keys by position in the tree
    /// -- so the default is a name shared by every table, which only goes wrong
    /// when a page shows two of them.
    id: SharedString,
    /// `indicator` — v3's render prop for the sort chevron, handed the
    /// `sortDirection` the column is sorted in.
    indicator: Option<Indicator>,
    columns: Vec<TableColumn>,
    rows: Vec<TableRow>,
    /// `treeColumn` — which column carries the expand chevron.
    tree_column: usize,
    /// `expandedKeys` — the parent rows showing their children.
    expanded_keys: Vec<SharedString>,
    on_expanded_change: Option<OnExpandedChange>,
    /// `TableLayout`'s `rowHeight`. Together with [`Table::virtual_rows`] it
    /// virtualizes the body: a fixed row height is what lets the scroll geometry
    /// be computed rather than laid out.
    row_height: Option<Pixels>,
    /// `TableLayout`'s `estimatedRowHeight` — virtualizes rows whose heights
    /// differ, where `rowHeight` needs them all the same.
    estimated_row_height: Option<Pixels>,
    /// `TableLayout`'s `loaderHeight` — the fixed height of the load-more row.
    loader_height: Option<Pixels>,
    /// `scrollOffset` on `Table.LoadMore`, in viewport heights.
    load_more_offset: f32,
    /// How tall the virtual body is. v3 sets it with a `className`.
    max_h: Option<Pixels>,
    /// `TableLayout`'s `gap` and `padding`, both 0 in v3: rows meet, separated
    /// by their border rather than by space.
    gap: Option<Pixels>,
    padding: Option<Pixels>,
    /// The row factory a virtual table needs. Cells are `AnyElement`, which
    /// cannot be built up front and then handed out again on the next scroll,
    /// so a virtual table asks for its rows one at a time.
    virtual_rows: Option<(usize, SharedString, VirtualRowKey, VirtualRow)>,
    virtual_text_value: Option<VirtualRowText>,
    virtual_tree_metadata: Option<VirtualTree>,
    variant: TableVariant,
    selection_mode: SelectionMode,
    selection_behavior: SelectionBehavior,
    selected_keys: Vec<SharedString>,
    default_selected_keys: Vec<SharedString>,
    disabled_keys: Vec<SharedString>,
    disallow_empty_selection: bool,
    is_selection_controlled: bool,
    sort_descriptor: Option<SortDescriptor>,
    show_indicator: bool,
    is_pending: bool,
    empty_state: Option<AnyElement>,
    footer: Option<AnyElement>,
    on_row_click: Option<OnRowClick>,
    on_selection_change: Option<OnSelectionChange>,
    on_sort_change: Option<OnSortChange>,
    on_load_more: Option<OnLoadMore>,
    on_resize_start: Option<OnResize>,
    on_resize: Option<OnResize>,
    on_resize_end: Option<OnResize>,
    /// The fill a hovered *unselected* row takes, in place of the variant's
    /// hover wash. A selected row keeps its selection fill.
    row_hover_bg: Option<gpui::Hsla>,
    /// The wrapper shell's corner radius, in place of the owning
    /// `container_radius` helper. `Primary` paints its own tray radius over
    /// this one, exactly as it covers the helper's value.
    radius: Option<Pixels>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
    /// Column reordering (HeroGPUI extension; see [`Table::allows_column_reorder`]).
    column_reorder: ColumnReorder,
    /// Cell selection (HeroGPUI extension; see [`Table::cell_selectable`]).
    cell_selection: CellSelection,
}

impl Table {
    /// Creates a table with the given column labels.
    pub fn new(columns: Vec<SharedString>) -> Self {
        Self {
            id: SharedString::from("table"),
            indicator: None,
            columns: columns.into_iter().map(TableColumn::new).collect(),
            rows: Vec::new(),
            tree_column: 0,
            expanded_keys: Vec::new(),
            on_expanded_change: None,
            row_height: None,
            estimated_row_height: None,
            loader_height: None,
            load_more_offset: 1.0,
            max_h: None,
            gap: None,
            padding: None,
            virtual_rows: None,
            virtual_text_value: None,
            virtual_tree_metadata: None,
            variant: TableVariant::Primary,
            selection_mode: SelectionMode::None,
            selection_behavior: SelectionBehavior::Toggle,
            selected_keys: Vec::new(),
            default_selected_keys: Vec::new(),
            disabled_keys: Vec::new(),
            disallow_empty_selection: false,
            is_selection_controlled: false,
            sort_descriptor: None,
            show_indicator: true,
            is_pending: false,
            empty_state: None,
            footer: None,
            on_row_click: None,
            on_selection_change: None,
            on_sort_change: None,
            on_load_more: None,
            on_resize_start: None,
            on_resize: None,
            on_resize_end: None,
            row_hover_bg: None,
            radius: None,
            sx: None,
            column_reorder: ColumnReorder::default(),
            cell_selection: CellSelection::default(),
        }
    }

    /// The id this table's own state is keyed by.
    ///
    /// Two tables on one page share their resized widths, their row cursor and
    /// their focus without it, because `use_keyed_state` keys by the id it is
    /// given and nothing else.
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
    }

    /// `indicator` — replaces the sort chevron.
    ///
    /// The closure receives `sortDirection`, the value v3 passes into the same
    /// render prop, so a caller can draw an arrow, a caret or a label without
    /// re-deriving which way the column is sorted.
    pub fn indicator(mut self, render: impl Fn(SortDirection) -> AnyElement + 'static) -> Self {
        self.indicator = Some(std::sync::Arc::new(render));
        self
    }

    /// Replaces the columns with fully configured ones.
    pub fn columns(mut self, columns: Vec<TableColumn>) -> Self {
        self.columns = columns;
        self
    }

    /// Adds one configured column.
    pub fn column(mut self, column: impl Into<TableColumn>) -> Self {
        self.columns.push(column.into());
        self
    }

    /// Sets the table's visual variant.
    pub fn variant(mut self, variant: TableVariant) -> Self {
        self.variant = variant;
        self
    }

    /// `TableLayout`'s `rowHeight` — and, with [`Table::virtual_rows`], what
    /// virtualizes the body.
    ///
    /// v3 wraps the table in `<Virtualizer layout={TableLayout}
    /// layoutOptions={{rowHeight: 40}}>`; the wrapper has no separate identity
    /// here, so the option that defines the layout carries it. a uniform
    /// [`VirtualList`](crate::VirtualList) builds only the rows the viewport shows, and it can do
    /// that because every row is this tall.
    pub fn row_height(mut self, height: impl Into<Pixels>) -> Self {
        self.row_height = Some(height.into());
        self
    }

    /// How tall the scrolling body is. v3's example sets it with `h-[400px]` on
    /// the table itself.
    pub fn max_h(mut self, height: impl Into<Pixels>) -> Self {
        self.max_h = Some(height.into());
        self
    }

    /// `TableLayout`'s `gap` -- space between rows, which v3 leaves at 0.
    pub fn gap(mut self, gap: impl Into<Pixels>) -> Self {
        self.gap = Some(gap.into());
        self
    }

    /// `TableLayout`'s `padding` -- space around the rows, 0 in v3.
    pub fn padding(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding = Some(padding.into());
        self
    }

    /// The rows of a virtualized table, built on demand.
    ///
    /// v3 writes `<Table items={users}>{(user) => <Table.Row>…}</Table>` and the
    /// Virtualizer calls that function for the rows in view. Cells here are
    /// `AnyElement`, which is built once and consumed once, so a virtual table
    /// cannot be handed its rows up front -- it has to be able to ask again
    /// every time the viewport moves.
    /// `key` projects the collection key without building the row. Keyboard
    /// navigation and select-all need the full collection, but calling `row`
    /// for every item would defeat virtualization.
    /// `identity` stays stable for one collection and changes when the caller
    /// replaces it, so a visible load-more sentinel can re-arm without building
    /// every row just to compare their keys.
    /// `TableLayout`'s `estimatedRowHeight` — virtualize rows that are not all
    /// one height.
    ///
    /// `rowHeight` maps to a uniform `VirtualList`, which measures one row and multiplies;
    /// this maps to gpui's `list`, which measures every row it builds. The
    /// estimate is what it renders beyond the viewport while it learns the real
    /// heights.
    pub fn estimated_row_height(mut self, height: impl Into<Pixels>) -> Self {
        self.estimated_row_height = Some(height.into());
        self
    }

    /// `TableLayout`'s `loaderHeight` — the height of the load-more row.
    pub fn loader_height(mut self, height: impl Into<Pixels>) -> Self {
        self.loader_height = Some(height.into());
        self
    }

    /// Virtual rows for a collection of `count` items.
    ///
    /// `identity` names the collection: it must stay stable for one collection
    /// and change when the caller replaces it. The key and row closures are
    /// treated as stable for that identity — their output is cached between
    /// frames and replayed whenever count, identity, tree mode and the
    /// expanded set are unchanged. If `key` (or [`Table::virtual_tree_metadata`]'s
    /// `metadata`) would return different output for the same indices without
    /// the collection being replaced, the caller must change `identity` so the
    /// cached projection is rebuilt.
    pub fn virtual_rows(
        mut self,
        count: usize,
        identity: impl Into<SharedString>,
        key: impl Fn(usize) -> SharedString + 'static,
        row: impl Fn(usize) -> TableRow + 'static,
    ) -> Self {
        self.virtual_rows = Some((
            count,
            identity.into(),
            std::sync::Arc::new(key),
            std::sync::Arc::new(row),
        ));
        self
    }

    /// Projects each virtual row's `textValue` without building its cells.
    /// Virtual rows do not participate in typeahead unless this is supplied.
    pub fn virtual_text_value(
        mut self,
        text_value: impl Fn(usize) -> SharedString + 'static,
    ) -> Self {
        self.virtual_text_value = Some(std::sync::Arc::new(text_value));
        self
    }

    /// Supplies preorder tree structure for [`Table::virtual_rows`].
    ///
    /// `count` remains the size of the underlying collection. The projection
    /// identifies each item's parent, depth and expandability; controlled
    /// `expanded_keys` then decides which source indices are visible. The
    /// metadata closure is treated as stable for the [`Table::virtual_rows`]
    /// collection identity: if its output changes for the same indices, the
    /// caller must change that identity so the cached projection is rebuilt.
    pub fn virtual_tree_metadata(
        mut self,
        metadata: impl Fn(usize) -> VirtualTreeMetadata + 'static,
    ) -> Self {
        self.virtual_tree_metadata = Some(std::sync::Arc::new(metadata));
        self
    }

    /// Adds one row of cells (`Table.Row` cells).
    /// `treeColumn` — the column whose cells carry the expand chevron.
    pub fn tree_column(mut self, index: usize) -> Self {
        self.tree_column = index;
        self
    }

    /// `expandedKeys` — the parent rows showing their children.
    pub fn expanded_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.expanded_keys = keys.into_iter().collect();
        self
    }

    /// Reports the expanded set after a chevron is pressed.
    pub fn on_expanded_change(
        mut self,
        f: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_expanded_change = Some(std::sync::Arc::new(f));
        self
    }

    /// Adds a row, with any nested rows it carries.
    pub fn tree_row(mut self, row: TableRow) -> Self {
        self.rows.push(row);
        self
    }

    /// Appends a row built from the given cells.
    pub fn row(mut self, cells: Vec<AnyElement>) -> Self {
        self.rows.push(TableRow::new(cells));
        self
    }

    /// Adds one row under an explicit selection key.
    pub fn keyed_row(mut self, key: impl Into<SharedString>, cells: Vec<AnyElement>) -> Self {
        self.rows.push(TableRow::new(cells).key(key));
        self
    }

    /// `selectionMode` — adds the selection column when not `None`.
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// `selectionBehavior` — how a multi-selection changes when a row is
    /// activated. `Toggle` preserves the current set; `Replace` makes a plain
    /// activation the sole selected row. Single selection keeps its own
    /// React Stately toggle/replace rule for both values.
    pub fn selection_behavior(mut self, behavior: SelectionBehavior) -> Self {
        self.selection_behavior = behavior;
        self
    }

    /// The fill a hovered *unselected* interactive row takes, in place of the
    /// variant's hover wash (`bg-surface/40` primary, `bg-default/50`
    /// secondary). A selected row keeps its `bg-surface/10` fill; a disabled
    /// or inert row never hovers.
    pub fn row_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.row_hover_bg = Some(color.into());
        self
    }

    /// The wrapper shell's corner radius, in place of the owning
    /// `container_radius` helper. `Primary` paints its own tray radius over
    /// this one, exactly as it covers the helper's value, so the override
    /// shows wherever the helper's value does. Not a v3 prop; the removed v2
    /// `radius` prop is prohibited and this is a per-component repository
    /// extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// `selectedKeys` — the controlled selection.
    pub fn selected_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.selected_keys = keys.into_iter().map(Into::into).collect();
        self.is_selection_controlled = true;
        self
    }

    /// `defaultSelectedKeys` — the initial uncontrolled selection. Later
    /// changes are owned by the table until `selected_keys` is supplied.
    pub fn default_selected_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.default_selected_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// `disallowEmptySelection` — keep the last selected row when a toggle or
    /// Escape would otherwise clear the collection.
    pub fn disallow_empty_selection(mut self, disallow: bool) -> Self {
        self.disallow_empty_selection = disallow;
        self
    }

    /// React Aria's inherited `disabledKeys` — rows excluded from selection,
    /// actions and the roving keyboard stops.
    pub fn disabled_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.disabled_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// `onSelectionChange` — the whole selection after a row is toggled.
    pub fn on_selection_change(
        mut self,
        f: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(std::sync::Arc::new(f));
        self
    }

    /// `sortDescriptor` — the current sort, shown in the header.
    pub fn sort_descriptor(mut self, descriptor: SortDescriptor) -> Self {
        self.sort_descriptor = Some(descriptor);
        self
    }

    /// `onSortChange` — a sortable header was clicked.
    ///
    /// The table does not reorder its rows; a row is already-rendered cells, so
    /// the caller sorts its data and rebuilds.
    pub fn on_sort_change(
        mut self,
        f: impl Fn(&SortDescriptor, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sort_change = Some(std::sync::Arc::new(f));
        self
    }

    /// `showIndicator` — whether a sorted column draws its direction chevron.
    pub fn show_indicator(mut self, v: bool) -> Self {
        self.show_indicator = v;
        self
    }

    /// `isLoading` on `Table.LoadMore` — shows the loading sentinel row.
    pub fn is_pending(mut self, v: bool) -> Self {
        self.is_pending = v;
        self
    }

    /// `scrollOffset` on `Table.LoadMore` — how many viewport heights before
    /// the sentinel enters view the load should start. React Aria defaults to
    /// one; v3's async example sets zero for an exact intersection.
    pub fn scroll_offset(mut self, offset: f32) -> Self {
        self.load_more_offset = offset;
        self
    }

    /// `onLoadMore` — the sentinel row entered the scroll viewport.
    pub fn on_load_more(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_load_more = Some(std::sync::Arc::new(f));
        self
    }

    /// `onResizeStart` on `Table.ResizableContainer` — reports the current
    /// pixel widths when pointer or keyboard resizing begins.
    pub fn on_resize_start(
        mut self,
        f: impl Fn(&[(SharedString, Pixels)], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize_start = Some(std::sync::Arc::new(f));
        self
    }

    /// `onResize` on `Table.ResizableContainer` — reports each proposed width
    /// map. Feed the values back through [`TableColumn::width`] for controlled
    /// resizing.
    pub fn on_resize(
        mut self,
        f: impl Fn(&[(SharedString, Pixels)], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize = Some(std::sync::Arc::new(f));
        self
    }

    /// `onResizeEnd` on `Table.ResizableContainer` — reports the final pixel
    /// widths after pointer or keyboard resizing ends.
    pub fn on_resize_end(
        mut self,
        f: impl Fn(&[(SharedString, Pixels)], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize_end = Some(std::sync::Arc::new(f));
        self
    }

    /// `Table.Footer` — a row under the body, which is where v3 puts a table's
    /// pagination.
    pub fn footer(mut self, content: impl IntoElement) -> Self {
        self.footer = Some(content.into_any_element());
        self
    }

    /// `renderEmptyState` — shown in place of rows when there are none.
    pub fn empty_state(mut self, content: impl IntoElement) -> Self {
        self.empty_state = Some(content.into_any_element());
        self
    }

    /// Called with the row index and click event when a row is clicked.
    pub fn on_row_click(
        mut self,
        f: impl Fn(&usize, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_row_click = Some(std::sync::Arc::new(f));
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the table's root element after every value the variant and
    /// the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// Every row's selection key, in order.
    fn row_keys(&self) -> Vec<SharedString> {
        fn collect(rows: &[TableRow], path: &str, out: &mut Vec<SharedString>) {
            for (index, row) in rows.iter().enumerate() {
                let key = match &row.key {
                    Some(key) => key.clone(),
                    None if path.is_empty() => SharedString::from(index.to_string()),
                    None => SharedString::from(format!("{path}-{index}")),
                };
                out.push(key.clone());
                collect(&row.children, key.as_ref(), out);
            }
        }

        let mut keys = Vec::new();
        collect(&self.rows, "", &mut keys);
        keys
    }
}

/// A virtual table's projected keys and visible rows, cached between frames.
///
/// The projection depends only on what [`Table::virtual_rows`] already names —
/// the collection's identity and count — plus the expanded set and whether
/// tree metadata is projected. Any frame where none of those changed reuses
/// the cached projection instead of calling the key and tree closures again;
/// the closures are free to return different values only when the caller
/// replaces the collection, which is what `identity` is for.
#[derive(Default)]
struct VirtualProjection {
    count: usize,
    identity: SharedString,
    expanded: std::sync::Arc<std::collections::HashSet<SharedString>>,
    tree: bool,
    full_keys: std::sync::Arc<Vec<SharedString>>,
    /// Behind an `Arc` because the list builders hold it across the frame.
    visible: std::sync::Arc<Vec<(usize, SharedString, VirtualTreeMetadata)>>,
    selectable_collection_keys: Option<std::sync::Arc<Vec<SharedString>>>,
    selectable_disabled_keys: Option<std::sync::Arc<Vec<SharedString>>>,
}

fn filtered_selectable_keys(
    full_keys: &[SharedString],
    disabled_keys: &[SharedString],
) -> std::sync::Arc<Vec<SharedString>> {
    std::sync::Arc::new(
        full_keys
            .iter()
            .filter(|key| !disabled_keys.contains(key))
            .cloned()
            .collect(),
    )
}

fn resolved_column_widths(
    columns: &[TableColumn],
    resized: &[Option<Pixels>],
    measured: &[Option<Pixels>],
    proposal: Option<(usize, Pixels)>,
) -> Vec<(SharedString, Pixels)> {
    columns
        .iter()
        .enumerate()
        .filter_map(|(index, column)| {
            let width = proposal
                .filter(|(proposed_index, _)| *proposed_index == index)
                .map(|(_, width)| width)
                .or(column.width)
                .or_else(|| resized.get(index).copied().flatten())
                .or(column.default_width)
                .or_else(|| measured.get(index).copied().flatten())?;
            let width = if column.allows_resizing {
                let min = column.min_width.map_or(DEFAULT_COLUMN_MIN_WIDTH, f32::from);
                let max = column.max_width.map_or(f32::MAX, f32::from);
                px(f32::from(width).floor().min(max).max(min))
            } else {
                width
            };
            Some((column.label.clone(), width))
        })
        .collect()
}

fn final_column_widths(
    columns: &[TableColumn],
    resized: &[Option<Pixels>],
    measured: &[Option<Pixels>],
    column: usize,
) -> Vec<(SharedString, Pixels)> {
    let proposal = resized
        .get(column)
        .copied()
        .flatten()
        .map(|width| (column, width));
    resolved_column_widths(columns, resized, measured, proposal)
}

fn clear_controlled_resize_proposal(
    columns: &[TableColumn],
    resized: &gpui::Entity<Vec<Option<Pixels>>>,
    column: usize,
    cx: &mut App,
) {
    if columns
        .get(column)
        .is_some_and(|column| column.width.is_some())
    {
        resized.update(cx, |values, cx| {
            if values
                .get_mut(column)
                .is_some_and(|width| width.take().is_some())
            {
                cx.notify();
            }
        });
    }
}

/// The keyed state one render reads: resize and measurement stores, the
/// load-more latch, the body's focus and cursor, the selection anchor and
/// the typeahead buffer. `use_keyed_state` takes `cx` mutably, so all of it
/// is created before the theme tokens are borrowed.
struct TableState {
    /// Column widths a resize handle has moved, and the drag in progress.
    resized: gpui::Entity<Vec<Option<Pixels>>>,
    resized_now: Vec<Option<Pixels>>,
    measured_widths: gpui::Entity<Vec<Option<Pixels>>>,
    measured_widths_now: Vec<Option<Pixels>>,
    dragging: gpui::Entity<Option<(usize, f32, f32)>>,
    drag_now: Option<(usize, f32, f32)>,
    keyboard_resizing: gpui::Entity<Option<usize>>,
    keyboard_resize_now: Option<usize>,
    load_more_state: gpui::Entity<(bool, Option<LoadMoreCollection>)>,
    /// The body is one tab stop with a cursor inside it, the way a list is.
    table_focus: gpui::FocusHandle,
    row_cursor: gpui::Entity<Option<SharedString>>,
    selection_range: gpui::Entity<TableSelectionRange>,
    typeahead: gpui::Entity<TableTypeahead>,
    selection_own: Option<gpui::Entity<Vec<SharedString>>>,
}

/// The virtual body for one render: the cached projection and the scroll
/// state of whichever virtual path is active.
struct VirtualBody {
    projection: Option<std::sync::Arc<VirtualProjection>>,
    visible_count: usize,
    visible_keys: Option<Vec<SharedString>>,
    row_heights: Option<gpui::Entity<(Vec<SharedString>, Vec<Option<Pixels>>)>>,
    scroll_now: crate::VirtualListHandle,
    list_state: Option<gpui::ListState>,
}

/// The header's focus handles and whether each one wears the ring.
struct HeaderFocus {
    header_focus: Vec<gpui::FocusHandle>,
    header_focused: Vec<bool>,
    resize_focus: Vec<Option<gpui::FocusHandle>>,
    resize_focused: Vec<bool>,
}

/// The one column-track computation the header and every row read.
struct ColumnTracks {
    resize_limits: Vec<(f32, f32)>,
    effective_widths: Vec<Option<Pixels>>,
    layout_width_bounds: Vec<(Option<Pixels>, Option<Pixels>)>,
    resize_columns: std::sync::Arc<Vec<TableColumn>>,
    resize_measurements: std::sync::Arc<Vec<Option<Pixels>>>,
}

/// The body rows after flattening, and what the keyboard reads off them.
struct RowCollection {
    flat: Vec<FlatRow>,
    tree_rows: Vec<(bool, Option<SharedString>)>,
    visible_collection_keys: Vec<SharedString>,
    typeahead_labels: Vec<String>,
    virtual_typeahead_indices: Option<Vec<usize>>,
    /// The row the keyboard cursor rings, when the body owns focus.
    cursor_at: Option<usize>,
    /// Whether any row in this table is expandable at all: a flat table
    /// keeps its cells flush, rather than reserving a chevron's width.
    tree_column_has_children: bool,
}

/// The per-render row facts [`Table::row_ctx`] folds into a [`RowCtx`].
struct RowCtxRows {
    row_keys: Vec<SharedString>,
    cursor: Option<usize>,
    tree_column_has_children: bool,
    expanded: std::rc::Rc<Vec<SharedString>>,
    is_tree: bool,
    virtualized: bool,
    round_top: Option<Pixels>,
    round_bottom: Option<Pixels>,
    cell: Option<CellCtx>,
    frozen: Option<FrozenColumns>,
}

impl RenderOnce for Table {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // The caller's id, as the root every part of this table hangs off.
        let base_id: gpui::ElementId = self.id.clone().into();
        // HeroGPUI extension: put the columns and cells in display order
        // first, so everything below works in display positions.
        let column_order = self.apply_column_order(&base_id, window, cx);
        let frozen = self.frozen_columns(&base_id, window, cx);
        let (selected_keys, selection_own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&base_id, "selected"),
            self.is_selection_controlled
                .then(|| self.selected_keys.clone()),
            self.default_selected_keys.clone(),
        );
        self.selected_keys = selected_keys;
        let state = self.table_state(&base_id, selection_own, window, cx);
        let mut virtual_body = self.virtual_body(&base_id, window, cx);
        let load_more_virtual_scroll = (self.row_height.is_some() && self.virtual_rows.is_some())
            .then(|| virtual_body.scroll_now.clone());
        let load_more_variable_scroll = virtual_body
            .list_state
            .clone()
            .zip(self.estimated_row_height)
            .map(|(list, estimate)| (list, virtual_body.visible_count, estimate));
        let focus = self.header_focus_state(&base_id, &state, window, cx);

        let resizable = self.columns.iter().any(|c| c.allows_resizing);
        let tracks = self.column_tracks(&state, frozen.as_ref());
        let colors = cx.colors().clone();
        // Copies of the tokens the tail needs: the row builder borrows `cx`
        // mutably, which ends the borrow `cx.colors()` holds.
        let muted = colors.muted;
        let secondary = self.variant == TableVariant::Secondary;
        let selectable = self.selection_mode != SelectionMode::None;
        let (selectable_collection_keys, load_more_collection) =
            self.selection_collections(&virtual_body);

        let mut wrapper = self.wrapper_tray(&state.table_focus, &colors, secondary, cx);

        // `useTable`'s `state.treeColumn != null` means the collection was
        // built as a tree, not merely that a column index was named:
        // `Table::tree_column` only picks which column carries the chevron,
        // and it defaults to `0` on every flat table. A tree here is a
        // collection with nested rows, virtual or not.
        let is_tree = self.virtual_tree_metadata.is_some()
            || self.rows.iter().any(|row| !row.children.is_empty());
        let mut table = self
            .grid_element(&base_id, selectable, is_tree, &virtual_body)
            .when(frozen.is_some(), |grid| grid.w_full());

        // ---- header ------------------------------------------------------
        let header_parts = HeaderParts {
            base_id: &base_id,
            colors: &colors,
            secondary,
            selectable,
            effective_widths: &tracks.effective_widths,
            layout_width_bounds: &tracks.layout_width_bounds,
            measured_widths: &state.measured_widths,
            measured_widths_now: &state.measured_widths_now,
            header_focus: &focus.header_focus,
            header_focused: &focus.header_focused,
            resize_focus: &focus.resize_focus,
            resize_focused: &focus.resize_focused,
            resized: &state.resized,
            dragging: &state.dragging,
            drag_now: state.drag_now,
            keyboard_resizing: &state.keyboard_resizing,
            keyboard_resize_now: state.keyboard_resize_now,
            resize_limits: &tracks.resize_limits,
            resize_columns: &tracks.resize_columns,
            resize_measurements: &tracks.resize_measurements,
            frozen: frozen.as_ref(),
        };
        let header = self.header_row(
            &header_parts,
            &column_order,
            &selectable_collection_keys,
            &state,
            window,
            cx,
        );
        table = table.child(header);

        // `.table__body` rounds to `min(32px, --radius-2xl)` and its cells are
        // `bg-surface`: the white block inside the tray.
        // `useTableRowGroup.mjs` (through `.../grid/useGridRowGroup.mjs`) is a
        // single `role: 'rowgroup'` — the `<tbody>` around the data rows.
        let mut body = gpui::div()
            .id(element_id::scoped(&base_id, "body"))
            .a11y(a11y::Role::RowGroup)
            .flex()
            .flex_col()
            .w_full();
        if !secondary {
            body = body
                .bg(colors.surface.background)
                .rounded(cx.layout().radius_2xl().min(px(32.)))
                .overflow_hidden();
        }

        if resizable {
            table = self.column_resize_drag(
                table,
                state.dragging.clone(),
                state.resized.clone(),
                tracks.resize_columns.clone(),
                tracks.resize_measurements.clone(),
            );
        }
        // ---- rows --------------------------------------------------------
        let rows = self.row_collection(&mut virtual_body, &state, window, cx);
        let expanded_keys = std::rc::Rc::new(std::mem::take(&mut self.expanded_keys));

        // Everything a row needs, held once. The virtual path builds its rows
        // inside a `'static` closure, so it cannot borrow `self` -- and having
        // one row builder for both paths is what keeps a virtual table drawing
        // the same row as a short one.
        let table_id = self.id.clone();
        let cell_selection = self.cell_selection_state(
            &base_id,
            &column_order,
            &state.row_cursor,
            &state.table_focus,
            window,
            cx,
        );
        wrapper = self.cell_selection_keys(
            wrapper,
            &cell_selection,
            &rows.visible_collection_keys,
            frozen.clone(),
        );
        let (round_top, round_bottom) = self.row_edge_rounding(
            secondary,
            &virtual_body.scroll_now,
            virtual_body.list_state.as_ref(),
            virtual_body.visible_count,
            cx,
        );
        let RowCollection {
            flat,
            tree_rows,
            visible_collection_keys,
            typeahead_labels,
            virtual_typeahead_indices,
            cursor_at,
            tree_column_has_children,
        } = rows;
        let ctx = std::rc::Rc::new(self.row_ctx(
            &base_id,
            &state,
            &tracks,
            RowCtxRows {
                row_keys: visible_collection_keys,
                cursor: cursor_at,
                tree_column_has_children,
                expanded: expanded_keys.clone(),
                is_tree,
                virtualized: virtual_body.projection.is_some(),
                round_top,
                round_bottom,
                cell: cell_selection,
                frozen,
            },
        ));

        wrapper = self.row_keyboard(
            wrapper,
            RowKeyboard {
                ctx: &ctx,
                expanded_keys,
                header_focus: focus.header_focus,
                row_cursor: state.row_cursor,
                selectable_collection_keys,
                selection_own: state.selection_own,
                selection_range: state.selection_range,
                table_focus: state.table_focus,
                tree_rows,
                typeahead_labels,
                typeahead: state.typeahead,
                virtual_list_state: &virtual_body.list_state,
                virtual_row_heights: &virtual_body.row_heights,
                virtual_scroll_now: &virtual_body.scroll_now,
                virtual_text_value: self.virtual_text_value.clone(),
                virtual_typeahead_indices,
            },
        );

        let row_count = if self.virtual_rows.is_some()
            && (self.row_height.is_some() || self.estimated_row_height.is_some())
        {
            virtual_body.visible_count
        } else {
            flat.len()
        };
        let virtual_projection = virtual_body
            .projection
            .map(|projection| std::sync::Arc::clone(&projection.visible))
            .unwrap_or_default();

        body = self.body_rows(
            body,
            BodyRows {
                base_id: &base_id,
                table_id: &table_id,
                ctx: &ctx,
                flat,
                row_count,
                virtual_projection,
                virtual_list_state: virtual_body.list_state,
                virtual_row_heights: virtual_body.row_heights,
                virtual_scroll_now: &virtual_body.scroll_now,
            },
            window,
            cx,
        );

        // ---- empty state -------------------------------------------------
        if row_count == 0 {
            if let Some(content) = self.empty_state.take() {
                body = body.child(Self::empty_state_row(content, muted));
            }
        }

        table = table.child(body);

        // ---- load-more sentinel ------------------------------------------
        if self.is_pending || self.on_load_more.is_some() {
            table = self.load_more(
                table,
                LoadMore {
                    base_id: &base_id,
                    state: state.load_more_state,
                    collection: load_more_collection,
                    virtual_scroll: load_more_virtual_scroll,
                    variable_scroll: load_more_variable_scroll,
                    muted,
                    loading_text: crate::i18n::ui_string(crate::i18n::UiString::LoadingMore, cx),
                },
            );
        }

        if let Some(footer) = self.footer.take() {
            table = table.child(Self::footer_row(footer));
        }

        let scroll_x = window
            .use_keyed_state(
                element_id::scoped(&base_id, "scroll-x-handle"),
                cx,
                |_, _| gpui::ScrollHandle::new(),
            )
            .read(cx)
            .clone();
        let el = wrapper.child(Self::scroll_container(&base_id, &table_id, table, scroll_x));
        crate::util::apply_sx(el, &self.sx)
    }
}

// ---- Table::render setup and chrome ----
impl Table {
    /// Creates (or reads back) the keyed state one render needs.
    fn table_state(
        &self,
        base_id: &gpui::ElementId,
        selection_own: Option<gpui::Entity<Vec<SharedString>>>,
        window: &mut Window,
        cx: &mut App,
    ) -> TableState {
        // Column widths a resize handle has moved, and the drag in progress.
        // `use_keyed_state` takes `cx` mutably, so both precede the tokens.
        let resized = window.use_keyed_state(element_id::scoped(base_id, "resized"), cx, |_, _| {
            Vec::<Option<Pixels>>::new()
        });
        let resized_now = resized.read(cx).clone();
        let measured_widths = window.use_keyed_state(
            element_id::scoped(base_id, "measured-widths"),
            cx,
            |_, _| Vec::<Option<Pixels>>::new(),
        );
        let measured_widths_now = measured_widths.read(cx).clone();
        let dragging =
            window.use_keyed_state(element_id::scoped(base_id, "resizing"), cx, |_, _| {
                None::<(usize, f32, f32)>
            });
        let drag_now = *dragging.read(cx);
        let keyboard_resizing = window.use_keyed_state(
            element_id::scoped(base_id, "keyboard-resizing"),
            cx,
            |_, _| None::<usize>,
        );
        let keyboard_resize_now = *keyboard_resizing.read(cx);
        let load_more_state = window.use_keyed_state(
            element_id::scoped(base_id, "load-more-state"),
            cx,
            |_, _| (false, None::<LoadMoreCollection>),
        );
        // The body is one tab stop with a cursor inside it, the way a list is.
        let table_focus =
            crate::util::tab_stop_handle(element_id::scoped(base_id, "focus"), window, cx);
        let row_cursor =
            window.use_keyed_state(element_id::scoped(base_id, "cursor"), cx, |_, _| {
                None::<SharedString>
            });
        let selection_range = window.use_keyed_state(
            element_id::scoped(base_id, "selection-range"),
            cx,
            |_, _| TableSelectionRange::default(),
        );
        let typeahead =
            window.use_keyed_state(element_id::scoped(base_id, "typeahead"), cx, |_, _| {
                TableTypeahead::default()
            });
        TableState {
            resized,
            resized_now,
            measured_widths,
            measured_widths_now,
            dragging,
            drag_now,
            keyboard_resizing,
            keyboard_resize_now,
            load_more_state,
            table_focus,
            row_cursor,
            selection_range,
            typeahead,
            selection_own,
        }
    }

    /// The frozen leading columns, once the columns are in display order:
    /// `None` when none (or every one) is frozen.
    fn frozen_columns(
        &self,
        base_id: &gpui::ElementId,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<FrozenColumns> {
        let count = self.columns.iter().take_while(|c| c.frozen).count();
        if count == 0 || count >= self.columns.len() {
            return None;
        }
        let scroll = window
            .use_keyed_state(element_id::scoped(base_id, "frozen-scroll"), cx, |_, _| {
                gpui::ScrollHandle::new()
            })
            .read(cx)
            .clone();
        Some(FrozenColumns {
            count,
            scroll,
            view: window.current_view(),
        })
    }

    /// The virtual projection and the scroll state of both virtual paths.
    fn virtual_body(
        &self,
        base_id: &gpui::ElementId,
        window: &mut Window,
        cx: &mut App,
    ) -> VirtualBody {
        let projection = self.virtual_projection(base_id, window, cx);
        let visible_count = projection
            .as_ref()
            .map_or(0, |projection| projection.visible.len());
        let visible_keys = projection.as_ref().map(|projection| {
            projection
                .visible
                .iter()
                .map(|(_, key, _)| key.clone())
                .collect::<Vec<_>>()
        });
        let row_heights = self.virtual_row_heights(base_id, visible_keys.as_ref(), window, cx);
        let virtual_scroll =
            window.use_keyed_state(element_id::scoped(base_id, "virtual-scroll"), cx, |_, _| {
                crate::VirtualListHandle::uniform(0)
            });
        let scroll_now = virtual_scroll.read(cx).clone();
        let list_state = self.virtual_list_state(base_id, visible_count, window, cx);
        VirtualBody {
            projection,
            visible_count,
            visible_keys,
            row_heights,
            scroll_now,
            list_state,
        }
    }

    /// The header's focus handles, and whether each wears the ring. Drops a
    /// keyboard resize whose column no longer has a handle.
    fn header_focus_state(
        &self,
        base_id: &gpui::ElementId,
        state: &TableState,
        window: &mut Window,
        cx: &mut App,
    ) -> HeaderFocus {
        // A sortable header had a click listener and no focus, so sorting was
        // mouse-only. v3's grid roves one tab stop across its cells; this port
        // gives each sortable header its own stop, which is the part that
        // matters. Created before the theme: `use_keyed_state` takes `cx`
        // mutably and `cx.colors()` holds a borrow.
        let sortable = self.on_sort_change.is_some();
        // Every column header is focusable, not just the sortable ones: v3's
        // PageUp leaves the body for the first column header whatever it is.
        // Only a sortable header is a *tab stop*, though -- it has to be, so
        // Enter and Space can sort it -- which keeps every table's Tab order
        // exactly as it was while giving the keyboard somewhere to land.
        let header_focus: Vec<gpui::FocusHandle> =
            self.header_focus_handles(base_id, sortable, window, cx);
        let ring_visible = crate::util::focus_visible(cx);
        let header_focused: Vec<bool> = header_focus
            .iter()
            .map(|h| h.is_focused(window) && ring_visible)
            .collect();
        let resize_focus: Vec<Option<gpui::FocusHandle>> =
            self.resize_focus_handles(base_id, window, cx);
        let resize_focused: Vec<bool> = resize_focus
            .iter()
            .map(|h| h.as_ref().is_some_and(|h| h.is_focused(window)) && ring_visible)
            .collect();
        if state.keyboard_resize_now.is_some_and(|column_index| {
            resize_focus
                .get(column_index)
                .and_then(Option::as_ref)
                .is_none()
        }) {
            state.keyboard_resizing.update(cx, |active, cx| {
                *active = None;
                cx.notify();
            });
        }
        HeaderFocus {
            header_focus,
            header_focused,
            resize_focus,
            resize_focused,
        }
    }

    /// The column tracks: resize bounds, the width each column renders at,
    /// and the shared `(min, max)` layout floor.
    fn column_tracks(&self, state: &TableState, frozen: Option<&FrozenColumns>) -> ColumnTracks {
        let resize_limits = column_resize_limits(&self.columns);
        let mut effective_widths =
            effective_column_widths(&self.columns, &state.resized_now, &resize_limits);
        // The one column-track computation. Upstream draws a `border-separate`
        // table, so the header's `<th>` cells resolve tracks every row shares;
        // this port stands in for that with one `(width, min, max)` triple per
        // column — the explicit width when one wins, otherwise `flex-basis:0;
        // flex-grow:1` floored by the widest cell measured across the header
        // and every body row. The header cells below and every `RowCtx` row
        // must read this same triple, and the sortable and resizable header
        // wrappers must carry it too: a wrapper that only grows equally would
        // resolve its own track and stagger the header off the body. A table
        // narrower than the tracks then overflows `scroll-x` as one grid
        // instead of squeezing each row separately.
        let layout_width_bounds = layout_width_bounds(
            &self.columns,
            &state.measured_widths_now,
            &effective_widths,
            &resize_limits,
        );
        // A frozen column does not share out the free space: its part is
        // sized to its cells, so the header's and every row's frozen parts
        // only agree when each frozen column has one width. Once the column
        // is measured that is its shared floor; a resizable column's probe
        // measures it first.
        if let Some(frozen) = frozen {
            for (c, column) in self.columns.iter().enumerate().take(frozen.count) {
                let measured = state.measured_widths_now.get(c).copied().flatten();
                if effective_widths[c].is_none() && (!column.allows_resizing || measured.is_some())
                {
                    effective_widths[c] = layout_width_bounds[c].0;
                }
            }
        }
        ColumnTracks {
            resize_limits,
            effective_widths,
            layout_width_bounds,
            resize_columns: std::sync::Arc::new(self.columns.clone()),
            resize_measurements: std::sync::Arc::new(state.measured_widths_now.clone()),
        }
    }

    /// The enabled-key set multiple selection consumes, and the identity the
    /// load-more latch compares.
    fn selection_collections(
        &self,
        virtual_body: &VirtualBody,
    ) -> (
        Option<std::sync::Arc<Vec<SharedString>>>,
        LoadMoreCollection,
    ) {
        let needs_selectable_keys = self.selection_mode == SelectionMode::Multiple;
        // Only multiple selection consumes the full enabled-key set. The
        // virtual projection shares it between frames; None and Single do not
        // materialize a collection that no consumer reads.
        let non_virtual_keys = virtual_body.projection.is_none().then(|| self.row_keys());
        let selectable_collection_keys: Option<std::sync::Arc<Vec<SharedString>>> =
            needs_selectable_keys.then(|| match virtual_body.projection.as_ref() {
                Some(projection) => projection
                    .selectable_collection_keys
                    .as_ref()
                    .expect("multiple virtual Tables cache selectable keys")
                    .clone(),
                None => filtered_selectable_keys(
                    non_virtual_keys
                        .as_ref()
                        .expect("multiple non-virtual Tables have row keys")
                        .as_slice(),
                    &self.disabled_keys,
                ),
            });
        let load_more_collection = match &self.virtual_rows {
            Some((count, identity, _, _)) => LoadMoreCollection::Virtual {
                count: *count,
                identity: identity.clone(),
            },
            None => LoadMoreCollection::Rows(non_virtual_keys.unwrap_or_default()),
        };
        (selectable_collection_keys, load_more_collection)
    }

    /// The table's root: the body's focus stop, and on the primary variant
    /// the `.table-root--primary` tray.
    fn wrapper_tray(
        &self,
        table_focus: &gpui::FocusHandle,
        colors: &herogpui_theme::ThemeColors,
        secondary: bool,
        cx: &App,
    ) -> gpui::Div {
        let mut wrapper = gpui::div()
            .w_full()
            .flex()
            .flex_col()
            .track_focus(table_focus)
            .overflow_hidden()
            .rounded(
                self.radius
                    .unwrap_or_else(|| crate::util::container_radius(cx)),
            )
            .text_color(colors.foreground);
        // A column, so the scroll container below is a vertical flex item: it
        // shrinks with a bounded parent (its `overflow_hidden` zeroes the
        // minimum), handing the virtual body its real viewport. A block would
        // size it to content and only clip the overflow.

        // `.table-root--primary` is a `bg-surface-secondary px-1 pb-1` tray with
        // `border-radius: min(32px, --radius * 2.5)`, and the rows sit in a
        // `bg-surface` block inside it; `secondary` is flat. This used to draw
        // one white card with a border, which is the block without its tray.
        if !secondary {
            let radius = cx.layout().radius_lg() * 2.5;
            wrapper = wrapper
                .bg(colors.surface_secondary)
                .rounded(radius.min(px(32.)))
                .px(px(4.))
                .pb(px(4.));
        }
        wrapper
    }

    /// `.table__content`: the grid itself, the column the header, body,
    /// load-more row and footer stack in.
    fn grid_element(
        &self,
        base_id: &gpui::ElementId,
        selectable: bool,
        is_tree: bool,
        virtual_body: &VirtualBody,
    ) -> gpui::Stateful<gpui::Div> {
        // The wrapper is a column, so this content column may shrink
        // vertically with a bounded parent -- that is what lets a virtual
        // body take the laid-out viewport rather than the `max_h` cap.
        // Horizontally it must never shrink: the scroll container at the
        // bottom of the render is a column with `items_start`, whose cross
        // axis leaves this child at its max-content width, so a table wider
        // than its box still slides on it. (A row-flex scroller would need
        // `flex_shrink_0` here for the same guarantee, which would also
        // forbid the vertical shrink.)
        //
        // The content column, whose width is what the scroller at the bottom
        // of the render measures against. A `w_full` child commits to the
        // scroller's width, which is exactly the scroller's own -- the scroll
        // maxima are then zero and a wide table clips at the tray edge instead
        // of sliding. `min_w_full` keeps the column at the viewport when no
        // column pins a width. `min_h_0` lets the column shrink vertically
        // with a bounded parent: without it the content-based minimum keeps
        // the max-content height and a virtual body never sees its real
        // viewport.
        // `aria-colcount` counts the collection's columns, and the selection
        // column is one of them upstream. `aria-rowcount` is only knowable
        // when the port virtualizes, which is exactly when upstream sets it.
        let grid_column_count = self.columns.len() + usize::from(selectable);
        let virtual_row_total = virtual_body
            .projection
            .as_ref()
            .map(|_| virtual_body.visible_count);
        let mut table = gpui::div()
            .flex()
            .flex_col()
            .min_w_full()
            .min_h_0()
            .text_size(px(14.))
            .line_height(px(20.))
            .when_some(self.gap, |el, g| el.gap(g))
            .when_some(self.padding, |el, p| el.p(p))
            // The accessibility contract goes on the *end* of this chain:
            // `.shots/design_audit.py` reads `.table__content`'s text size
            // through a 240-character window opening at `let mut table =
            // gpui::div(`, and it reports the rule unreadable when anything
            // is spliced in ahead of `.text_size(..)`.
            //
            // `react-aria/dist/private/table/useTable.mjs` starts from
            // `.../grid/useGrid.mjs`'s `role: 'grid'` and overrides it to
            // `'treegrid'` only `if (state.treeColumn != null)`. RAC's
            // `Table.mjs` adds no role of its own.
            //
            // `aria-rowcount` / `aria-colcount` are set only
            // `if (isVirtualized)` — the DOM then holds a window onto a
            // longer collection and the counts cannot be read off the tree —
            // and `useTable` then makes the row count
            // `state.collection.size + state.collection.headerRows.length`,
            // which is why the one header row is added to it. The
            // `aria-multiselectable` that `useGrid` sets beside them has no
            // gpui builder (see `crate::a11y`).
            .id(element_id::scoped(base_id, "grid"))
            .a11y(if is_tree {
                a11y::Role::TreeGrid
            } else {
                a11y::Role::Grid
            });
        if let Some(rows) = virtual_row_total {
            table = table.a11y_grid_size(rows + 1, grid_column_count);
        }
        table
    }

    /// `.table__header`, whose cells are `.table__column`s and whose
    /// sortable ones wrap in `.table__sortable-column-header`.
    fn header_row(
        &self,
        hx: &HeaderParts<'_>,
        column_order: &std::rc::Rc<Vec<usize>>,
        selectable_collection_keys: &Option<std::sync::Arc<Vec<SharedString>>>,
        state: &TableState,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let base_id = hx.base_id;
        let frozen = hx.frozen;
        let colors = hx.colors;
        let secondary = hx.secondary;
        // A frozen header measures its two parts instead (see
        // `frozen_header_parts`).
        let measured = if frozen.is_some() {
            gpui::div()
        } else {
            self.column_reorder_measure(gpui::div(), base_id, window, cx)
        };
        let mut header = measured
            .id(element_id::scoped(base_id, "header"))
            // `useTableHeaderRow.mjs` is a bare `role: 'row'`. Upstream wraps
            // it in a `<TableHeader>` whose `useTableRowGroup` gives it
            // `role: 'rowgroup'`; this port draws the single header row
            // directly into the table, so there is no second element for that
            // node to sit on. The body below *is* a container of rows, and it
            // carries the rowgroup.
            .a11y(a11y::Role::Row)
            .flex()
            .border_b_1()
            .border_color(colors.separator.alpha(0.5))
            .when(!secondary, |h| h.bg(colors.surface_secondary));

        let mut cells: Vec<AnyElement> = Vec::new();
        if hx.selectable {
            cells.push(
                self.select_all_header_cell(
                    base_id,
                    colors,
                    secondary,
                    selectable_collection_keys,
                    &state.selection_own,
                    &state.selection_range,
                    cx,
                )
                .into_any_element(),
            );
        }

        for (column_index, column) in self.columns.iter().enumerate() {
            cells.push(self.column_header_cell(hx, column_index, column, window, cx));
        }
        header = match frozen {
            Some(frozen) => {
                let (lead, rest) = self.frozen_header_parts(base_id, frozen, cells, window, cx);
                header.child(lead).child(rest)
            }
            None => header.children(cells),
        };
        self.column_reorder_header(
            header,
            base_id,
            column_order,
            hx.header_focus,
            hx.dragging,
            window,
            cx,
        )
    }

    /// Flattens the plain rows (or reads the virtual projection) into what
    /// the row builder and the keyboard read, and resolves the cursor.
    fn row_collection(
        &mut self,
        virtual_body: &mut VirtualBody,
        state: &TableState,
        window: &mut Window,
        cx: &mut App,
    ) -> RowCollection {
        let virtual_projection = &virtual_body.projection;
        let mut flat: Vec<FlatRow> = Vec::new();
        flatten_rows(
            std::mem::take(&mut self.rows),
            0,
            "",
            None,
            &self.expanded_keys,
            &mut flat,
        );
        let tree_rows: Vec<(bool, Option<SharedString>)> = virtual_projection.as_ref().map_or_else(
            || {
                flat.iter()
                    .map(|(_, _, has_children, _, parent)| (*has_children, parent.clone()))
                    .collect()
            },
            |projection| {
                projection
                    .visible
                    .iter()
                    .map(|(_, _, metadata)| (metadata.has_children, metadata.parent_key.clone()))
                    .collect()
            },
        );
        let visible_collection_keys: Vec<SharedString> = virtual_body
            .visible_keys
            .take()
            .unwrap_or_else(|| flat.iter().map(|(_, _, _, key, _)| key.clone()).collect());
        let typeahead_labels: Vec<String> = virtual_projection.as_ref().map_or_else(
            || {
                flat.iter()
                    .map(|(row, _, _, _, _)| {
                        row.text_value
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_default()
                    })
                    .collect()
            },
            |_| Vec::new(),
        );
        let virtual_typeahead_indices = virtual_projection.as_ref().map(|projection| {
            projection
                .visible
                .iter()
                .map(|(source_index, _, _)| *source_index)
                .collect::<Vec<_>>()
        });
        let row_cursor = &state.row_cursor;
        let table_focus = &state.table_focus;
        let cursor_key = row_cursor.read(cx).clone();
        let cursor_valid = cursor_key.as_ref().is_none_or(|key| {
            visible_collection_keys.contains(key) && !self.disabled_keys.contains(key)
        });
        if !cursor_valid {
            row_cursor.update(cx, |key, cx| {
                *key = None;
                cx.notify();
            });
        }
        let cursor_at = if window.is_window_active() && table_focus.is_focused(window) {
            cursor_key
                .as_ref()
                .and_then(|key| visible_collection_keys.iter().position(|row| row == key))
                .filter(|index| {
                    !self
                        .disabled_keys
                        .contains(&visible_collection_keys[*index])
                })
                .filter(|_| crate::util::focus_visible(cx))
        } else {
            None
        };
        // Whether any row in this table is expandable at all: a flat table
        // keeps its cells flush, rather than reserving a chevron's width.
        let tree_column_has_children = virtual_projection.as_ref().map_or_else(
            || flat.iter().any(|(_, _, has, _, _)| *has),
            |projection| {
                projection
                    .visible
                    .iter()
                    .any(|(_, _, metadata)| metadata.has_children)
            },
        );
        RowCollection {
            flat,
            tree_rows,
            visible_collection_keys,
            typeahead_labels,
            virtual_typeahead_indices,
            cursor_at,
            tree_column_has_children,
        }
    }

    /// Everything one body row reads, held once for both the plain and the
    /// virtual paths.
    fn row_ctx(
        &self,
        base_id: &gpui::ElementId,
        state: &TableState,
        tracks: &ColumnTracks,
        rows: RowCtxRows,
    ) -> RowCtx {
        RowCtx {
            id: base_id.clone(),
            measured_widths: state.measured_widths.clone(),
            measure_intrinsic: self
                .columns
                .iter()
                // `on_children_prepainted` reports the browser canvas child
                // bounds as the full flex track during a web rerender. If we
                // feed that transient width back as a minimum, the first
                // column consumes the whole table after a selection update.
                // Native GPUI reports intrinsic child bounds correctly, so
                // retain the measurement path there and let web flex tracks
                // remain fluid unless a caller pins a width.
                .map(|column| !column.allows_resizing && !cfg!(target_arch = "wasm32"))
                .collect(),
            widths: self
                .columns
                .iter()
                .enumerate()
                .map(|(c, _)| {
                    let (layout_min, layout_max) = tracks.layout_width_bounds[c];
                    (
                        tracks.effective_widths.get(c).copied().flatten(),
                        layout_min,
                        layout_max,
                    )
                })
                .collect(),
            row_header_columns: self.columns.iter().map(|c| c.is_row_header).collect(),
            tree_column: self.tree_column,
            tree_column_has_children: rows.tree_column_has_children,
            selectable: self.selection_mode != SelectionMode::None,
            selection_mode: self.selection_mode,
            selection_behavior: self.selection_behavior,
            selected_keys: self.selected_keys.clone(),
            row_keys: rows.row_keys,
            disabled_keys: self.disabled_keys.clone(),
            disallow_empty_selection: self.disallow_empty_selection,
            expanded: rows.expanded,
            on_expanded_change: self.on_expanded_change.clone(),
            on_selection_change: self.on_selection_change.clone(),
            selection_own: state.selection_own.clone(),
            selection_range: state.selection_range.clone(),
            on_row_click: self.on_row_click.clone(),
            focus: state.table_focus.clone(),
            cursor_own: state.row_cursor.clone(),
            cursor: rows.cursor,
            secondary: self.variant == TableVariant::Secondary,
            row_hover_bg: self.row_hover_bg,
            hover_group_prefix: self.id.clone(),
            virtualized: rows.virtualized,
            round_top: rows.round_top,
            round_bottom: rows.round_bottom,
            is_tree: rows.is_tree,
            cell: rows.cell,
            frozen: rows.frozen,
        }
    }

    /// The caller's empty state, centred in the body when there are no rows.
    fn empty_state_row(content: AnyElement, muted: gpui::Hsla) -> gpui::Div {
        gpui::div()
            .flex()
            .items_center()
            .justify_center()
            .w_full()
            .py(px(28.))
            .text_color(muted)
            .child(content)
    }

    /// `.table__footer` is `flex items-center px-4 py-2.5`.
    fn footer_row(footer: AnyElement) -> gpui::Div {
        gpui::div()
            .flex()
            .items_center()
            .w_full()
            .px(px(16.))
            .py(px(10.))
            .child(footer)
    }

    /// `.table__scroll-container` is `overflow-x-auto` around the content.
    fn scroll_container(
        base_id: &gpui::ElementId,
        table_id: &SharedString,
        table: gpui::Stateful<gpui::Div>,
        scroll: gpui::ScrollHandle,
    ) -> gpui::Stateful<gpui::Div> {
        // `.table__scroll-container` is `overflow-x-auto` around the content: a
        // column whose cross axis leaves the content column at its
        // max-content width, so a table wider than its box is free to exceed
        // the scroller and scroll *on* it, while `min_w_full` still fills a
        // narrow box. A row flex here would shrink that child to the viewport
        // unless it carried `flex_shrink_0`, which would also forbid the
        // column's vertical shrink inside a bounded parent; the column
        // direction shrinks the content vertically while never touching its
        // width. `min_h_0` is what permits that shrink: the scroller only
        // scrolls horizontally, so its visible y-overflow would otherwise keep
        // the content-based minimum and never yield.
        // The headless probe name for the scroll viewport's bounds, so a test
        // can read the grid's overflow against it.
        let scroll_selector = format!("{table_id}-scroll-x");
        gpui::div()
            .id(element_id::scoped(base_id, "scroll-x"))
            .debug_selector(move || scroll_selector)
            .flex()
            .flex_col()
            .items_start()
            .w_full()
            .min_h_0()
            .overflow_x_scroll()
            .track_scroll(&scroll)
            .restrict_scroll_to_axis()
            .on_scroll(move |event, window, _cx| {
                crate::util::shift_wheel_scroll_x(&scroll, event, window);
            })
            .child(table)
    }
}

/// Each column's `(min, max)` resize bounds, in pixels.
fn column_resize_limits(columns: &[TableColumn]) -> Vec<(f32, f32)> {
    columns
        .iter()
        .map(|column| {
            (
                column.min_width.map_or(DEFAULT_COLUMN_MIN_WIDTH, f32::from),
                column.max_width.map_or(f32::MAX, f32::from),
            )
        })
        .collect()
}

/// The width each column renders at: a controlled width, else the width a
/// resize left it at, else its default — clamped to its resize bounds when it
/// resizes. `None` is a flexible track.
fn effective_column_widths(
    columns: &[TableColumn],
    resized_now: &[Option<Pixels>],
    resize_limits: &[(f32, f32)],
) -> Vec<Option<Pixels>> {
    columns
        .iter()
        .enumerate()
        .map(|(column_index, column)| {
            let width = column.width.or_else(|| {
                resized_now
                    .get(column_index)
                    .copied()
                    .flatten()
                    .or(column.default_width)
            });
            if column.allows_resizing {
                let (min, max) = resize_limits[column_index];
                width.map(|width| px(f32::from(width).floor().min(max).max(min)))
            } else {
                width
            }
        })
        .collect()
}

/// The `(min, max)` track bounds each column's header and body cells share;
/// see the comment at its call site in `Table::render`.
fn layout_width_bounds(
    columns: &[TableColumn],
    measured_widths_now: &[Option<Pixels>],
    effective_widths: &[Option<Pixels>],
    resize_limits: &[(f32, f32)],
) -> Vec<(Option<Pixels>, Option<Pixels>)> {
    columns
        .iter()
        .enumerate()
        .map(|(column_index, column)| {
            let measured = measured_widths_now.get(column_index).copied().flatten();
            if column.allows_resizing {
                if effective_widths[column_index].is_some() {
                    (None, None)
                } else {
                    let min = measured.map_or(resize_limits[column_index].0, |width| {
                        resize_limits[column_index].0.max(f32::from(width))
                    });
                    (Some(px(min)), column.max_width)
                }
            } else {
                let min = match (column.min_width, measured) {
                    (Some(configured), Some(content)) => {
                        Some(px(f32::from(configured).max(f32::from(content))))
                    }
                    (Some(configured), None) => Some(configured),
                    (None, Some(content)) => Some(content),
                    (None, None) => None,
                };
                (min, column.max_width)
            }
        })
        .collect()
}

/// The per-render column state every header cell reads; see
/// [`Table::column_header_cell`].
#[derive(Clone, Copy)]
struct HeaderParts<'a> {
    base_id: &'a gpui::ElementId,
    colors: &'a herogpui_theme::ThemeColors,
    secondary: bool,
    selectable: bool,
    effective_widths: &'a [Option<Pixels>],
    layout_width_bounds: &'a [(Option<Pixels>, Option<Pixels>)],
    measured_widths: &'a gpui::Entity<Vec<Option<Pixels>>>,
    measured_widths_now: &'a [Option<Pixels>],
    header_focus: &'a [gpui::FocusHandle],
    header_focused: &'a [bool],
    resize_focus: &'a [Option<gpui::FocusHandle>],
    resize_focused: &'a [bool],
    resized: &'a gpui::Entity<Vec<Option<Pixels>>>,
    dragging: &'a gpui::Entity<Option<(usize, f32, f32)>>,
    drag_now: Option<(usize, f32, f32)>,
    keyboard_resizing: &'a gpui::Entity<Option<usize>>,
    keyboard_resize_now: Option<usize>,
    resize_limits: &'a [(f32, f32)],
    resize_columns: &'a std::sync::Arc<Vec<TableColumn>>,
    resize_measurements: &'a std::sync::Arc<Vec<Option<Pixels>>>,
    frozen: Option<&'a FrozenColumns>,
}

/// One plain body row after flattening: the row, its depth, whether it has
/// children, its key and its parent's key.
type FlatRow = (TableRow, usize, bool, SharedString, Option<SharedString>);

/// Depth-first, and only through the parents that are open: a nested row is
/// not rendered at all until its parent is expanded.
fn flatten_rows(
    rows: Vec<TableRow>,
    depth: usize,
    path: &str,
    parent: Option<&SharedString>,
    expanded: &[SharedString],
    out: &mut Vec<FlatRow>,
) {
    for (index, mut row) in rows.into_iter().enumerate() {
        let key = match &row.key {
            Some(k) => k.clone(),
            None if path.is_empty() => SharedString::from(index.to_string()),
            None => SharedString::from(format!("{path}-{index}")),
        };
        let children = std::mem::take(&mut row.children);
        let has_children = !children.is_empty();
        let is_open = expanded.contains(&key);
        out.push((row, depth, has_children, key.clone(), parent.cloned()));
        if has_children && is_open {
            flatten_rows(children, depth + 1, key.as_ref(), Some(&key), expanded, out);
        }
    }
}

/// What [`Table::row_keyboard`] reads from the rest of the render.
struct RowKeyboard<'a> {
    ctx: &'a RowCtx,
    expanded_keys: std::rc::Rc<Vec<SharedString>>,
    header_focus: Vec<gpui::FocusHandle>,
    row_cursor: gpui::Entity<Option<SharedString>>,
    selectable_collection_keys: Option<std::sync::Arc<Vec<SharedString>>>,
    selection_own: Option<gpui::Entity<Vec<SharedString>>>,
    selection_range: gpui::Entity<TableSelectionRange>,
    table_focus: gpui::FocusHandle,
    tree_rows: Vec<(bool, Option<SharedString>)>,
    typeahead_labels: Vec<String>,
    typeahead: gpui::Entity<TableTypeahead>,
    virtual_list_state: &'a Option<gpui::ListState>,
    virtual_row_heights: &'a Option<gpui::Entity<(Vec<SharedString>, Vec<Option<Pixels>>)>>,
    virtual_scroll_now: &'a crate::VirtualListHandle,
    virtual_text_value: Option<VirtualRowText>,
    virtual_typeahead_indices: Option<Vec<usize>>,
}

/// What [`Table::body_rows`] reads from the rest of the render.
struct BodyRows<'a> {
    base_id: &'a gpui::ElementId,
    table_id: &'a SharedString,
    ctx: &'a std::rc::Rc<RowCtx>,
    flat: Vec<FlatRow>,
    row_count: usize,
    virtual_projection: std::sync::Arc<Vec<(usize, SharedString, VirtualTreeMetadata)>>,
    virtual_list_state: Option<gpui::ListState>,
    virtual_row_heights: Option<gpui::Entity<(Vec<SharedString>, Vec<Option<Pixels>>)>>,
    virtual_scroll_now: &'a crate::VirtualListHandle,
}

/// What [`Table::load_more`] reads from the rest of the render.
struct LoadMore<'a> {
    base_id: &'a gpui::ElementId,
    state: gpui::Entity<(bool, Option<LoadMoreCollection>)>,
    collection: LoadMoreCollection,
    virtual_scroll: Option<crate::VirtualListHandle>,
    variable_scroll: Option<(gpui::ListState, usize, Pixels)>,
    muted: gpui::Hsla,
    /// The pending row's text, resolved through the i18n catalogue.
    loading_text: SharedString,
}

/// Everything the body's bubbling key handler reads, moved out of the render
/// because the handler outlives the frame. See [`Table::row_keyboard`].
struct RowKeyNav {
    /// The enabled rows' positions: the stops the cursor may rest on.
    stops: Vec<usize>,
    /// The row cursor.
    held: gpui::Entity<Option<SharedString>>,
    keys: Vec<SharedString>,
    table_focus: gpui::FocusHandle,
    typeahead: std::rc::Rc<TableTypeaheadNavigation>,
    on_row_click: Option<OnRowClick>,
    selection: Option<OnSelectionChange>,
    selection_own: Option<gpui::Entity<Vec<SharedString>>>,
    selection_range: gpui::Entity<TableSelectionRange>,
    selected_now: Vec<SharedString>,
    selectable_collection_keys: Option<std::sync::Arc<Vec<SharedString>>>,
    mode: SelectionMode,
    selection_behavior: SelectionBehavior,
    disallow_empty_selection: bool,
    select_on_focus: bool,
    plain_rows: bool,
    fixed_virtual: bool,
    fixed_row_height: Option<Pixels>,
    fixed_scroll: crate::VirtualListHandle,
    variable_scroll: Option<gpui::ListState>,
    variable_heights: Option<gpui::Entity<(Vec<SharedString>, Vec<Option<Pixels>>)>>,
    variable_estimate: Option<Pixels>,
    expanded: std::rc::Rc<Vec<SharedString>>,
    on_expanded: Option<OnExpandedChange>,
    tree_rows: Vec<(bool, Option<SharedString>)>,
    page_up_header: Option<gpui::FocusHandle>,
    headers: Vec<gpui::FocusHandle>,
}

impl RowKeyNav {
    /// The body's bubbling key handler.
    fn key_down(&self, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut App) {
        if !self.table_focus.contains_focused(window, cx) {
            return;
        }
        let stops = &self.stops;
        let keys = &self.keys;
        let from = self
            .held
            .read(cx)
            .as_ref()
            .and_then(|key| keys.iter().position(|row| row == key))
            .filter(|index| stops.contains(index));
        // Pinned React Aria `useTableRow`: horizontal keys belong
        // to the focused tree row before the list resolver sees
        // them. Right opens; Left closes or returns to the parent.
        let key_name = event.keystroke.key.as_str();
        let typed = event.keystroke.key_char.as_deref().unwrap_or(key_name);
        let modifiers = &event.keystroke.modifiers;
        let is_mac = cx.platform_info().is_mac();
        let now = web_time::Instant::now();
        let is_space = key_name == "space" || typed == " ";
        let is_character = {
            let mut chars = key_name.chars();
            chars.next().is_some() && chars.next().is_none() && !is_space
        };
        let is_typeahead =
            is_character && !modifiers.control && !modifiers.platform && !modifiers.alt;
        if is_typeahead {
            if self.typeahead.push(typed, now, true, cx) {
                cx.stop_propagation();
            }
            return;
        }
        // Pinned React Aria 3.51 `useSelectableCollection` binds
        // `Mod+A` -- the platform Mod, Control here -- to
        // `selectAll`, and only when the selection mode is
        // multiple. The shortcut matches its modifiers exactly,
        // so any extra modifier lets the event fall through.
        if key_name == "a"
            && event.keystroke.modifiers.secondary(cx)
            && !event.keystroke.modifiers.shift
            && !event.keystroke.modifiers.alt
            && !event.keystroke.modifiers.function
            && if is_mac {
                !event.keystroke.modifiers.control
            } else {
                !event.keystroke.modifiers.platform
            }
            && self.mode == SelectionMode::Multiple
        {
            self.select_all(window, cx);
            cx.stop_propagation();
            return;
        }
        // Other collection keys belong only to the body's roving
        // focus stop. A nested cell action must keep its own Enter
        // and Space handling even though Mod+A bubbles to the root.
        // Pinned TableKeyboardDelegate crosses header<->body in
        // both plain and virtual tables: Down/PageDown from a
        // focused header enters the body.
        if self.headers.iter().any(|header| header.is_focused(window)) {
            self.enter_from_header(key_name, window, cx);
            return;
        }
        if !self.table_focus.is_focused(window) {
            return;
        }
        if key_name == "escape"
            && self.mode != SelectionMode::None
            && !self.disallow_empty_selection
            && !self.selected_now.is_empty()
        {
            self.commit(&[], window, cx);
            self.selection_range.update(cx, |range, _| {
                *range = TableSelectionRange::default();
            });
            cx.stop_propagation();
            return;
        }
        if let Some(index) = from {
            if self.tree_key(index, key_name, window, cx) {
                return;
            }
        }
        let (fixed_page_move, variable_page_move) = self.page_moves(from, key_name, cx);
        let is_variable_page = fixed_page_move.is_none() && variable_page_move.is_some();
        if self.page_up_to_header(
            from,
            key_name,
            fixed_page_move,
            variable_page_move,
            window,
            cx,
        ) {
            return;
        }
        let plain_page_move = from
            .filter(|_| self.plain_rows)
            .and_then(|_| match key_name {
                "pagedown" => stops.last().copied(),
                _ => None,
            });
        let page_move = fixed_page_move
            .or(variable_page_move)
            .or(plain_page_move)
            .filter(|next| Some(*next) != from);
        // The pinned registrations install no Home/End handler
        // for an unregistered chord -- Cmd- or Ctrl-bearing on
        // macOS, Alt- or platform-bearing elsewhere -- so the
        // whole event stays inert: no focus move, no selection,
        // no preventDefault, and no first-press settle either.
        if matches!(key_name, "home" | "end") && !home_end_registered(*modifiers, is_mac) {
            return;
        }
        let initial_home_end_extends = shift_home_end_extends("home", modifiers.control, is_mac);
        let initial_shift_settle = from.is_none()
            && modifiers.shift
            && (key_name == "up"
                || (matches!(key_name, "home" | "end") && !initial_home_end_extends)
                || (key_name == "down" && stops.len() < 2));
        let navigation = if from.is_none() && modifiers.shift && key_name == "down" {
            stops
                .get(1)
                .or_else(|| stops.first())
                .copied()
                .map_or(crate::list_nav::Move::Ignore, crate::list_nav::Move::To)
        } else if initial_shift_settle {
            (if key_name == "end" {
                stops.last()
            } else {
                stops.first()
            })
            .copied()
            .map_or(crate::list_nav::Move::Ignore, crate::list_nav::Move::To)
        } else {
            page_move.map_or_else(
                || crate::list_nav::resolve(stops, from, key_name, false),
                crate::list_nav::Move::To,
            )
        };
        match navigation {
            crate::list_nav::Move::To(next) => {
                self.move_cursor(
                    next,
                    from,
                    key_name,
                    *modifiers,
                    initial_shift_settle,
                    is_variable_page,
                    window,
                    cx,
                );
            }
            crate::list_nav::Move::Activate => {
                let Some(index) = from else { return };
                self.activate(index, event, *modifiers, window, cx);
            }
            crate::list_nav::Move::Ignore => {}
        }
    }

    /// Writes a new selection to the uncontrolled store and reports it.
    fn commit(&self, next: &[SharedString], window: &mut Window, cx: &mut App) {
        if let Some(held) = &self.selection_own {
            held.update(cx, |value, cx| {
                *value = next.to_vec();
                cx.notify();
            });
        }
        if let Some(cb) = &self.selection {
            cb(next, window, cx);
        }
    }

    fn selectable_keys(&self) -> &[SharedString] {
        self.selectable_collection_keys
            .as_ref()
            .expect("multiple Tables have selectable keys")
            .as_slice()
    }

    /// `Mod+A` on a multiple selection.
    fn select_all(&self, window: &mut Window, cx: &mut App) {
        let selectable_collection_keys = self
            .selectable_collection_keys
            .as_ref()
            .expect("multiple Tables have selectable keys");
        let selectable_collection_keys = selectable_collection_keys.clone();
        let selectable_collection_keys = selectable_collection_keys.as_slice();
        let (all_selected, _) = select_all_flags(selectable_collection_keys, &self.selected_now);
        let materializes_all = same_selection(selectable_collection_keys, &self.selected_now);
        let already_all =
            all_selected || (self.selection_range.read(cx).is_all && materializes_all);
        // Pinned React Stately's `selectAll` is idempotent once
        // the whole selectable collection is already selected.
        if !already_all {
            let next = selectable_collection_keys.to_vec();
            self.commit(&next, window, cx);
            self.selection_range.update(cx, |range, _| {
                *range = TableSelectionRange {
                    is_all: true,
                    ..TableSelectionRange::default()
                };
            });
        }
    }

    /// Down/PageDown from a focused column header enters the body.
    fn enter_from_header(&self, key_name: &str, window: &mut Window, cx: &mut App) {
        let next = match key_name {
            "down" => self.stops.first(),
            "pagedown" => self.stops.last(),
            _ => None,
        };
        if let Some(next) = next {
            self.held.update(cx, |value, cx| {
                *value = Some(self.keys[*next].clone());
                cx.notify();
            });
            window.focus(&self.table_focus, cx);
            cx.stop_propagation();
        }
    }

    /// A tree row's Right and Left. Returns whether the key was consumed.
    fn tree_key(&self, index: usize, key_name: &str, window: &mut Window, cx: &mut App) -> bool {
        let keys = &self.keys;
        let expanded = &self.expanded;
        let focused_key = &keys[index];
        let Some((has_children, parent)) = self.tree_rows.get(index) else {
            return false;
        };
        if key_name == "right" && *has_children && !expanded.contains(focused_key) {
            if let Some(cb) = &self.on_expanded {
                let mut next = expanded.as_ref().clone();
                next.push(focused_key.clone());
                cb(&next, window, cx);
            }
            crate::util::set_focus_visible(true, cx);
            cx.stop_propagation();
            return true;
        } else if key_name == "left" {
            if *has_children && expanded.contains(focused_key) {
                if let Some(cb) = &self.on_expanded {
                    let mut next = expanded.as_ref().clone();
                    next.retain(|key| key != focused_key);
                    cb(&next, window, cx);
                }
                crate::util::set_focus_visible(true, cx);
                cx.stop_propagation();
                return true;
            } else if let Some(parent) = parent {
                if let Some(parent_index) = keys.iter().position(|key| key == parent) {
                    self.held.update(cx, |value, cx| {
                        *value = Some(parent.clone());
                        cx.notify();
                    });
                    if self.fixed_virtual {
                        self.fixed_scroll
                            .scroll_to_item(parent_index, crate::VirtualListScroll::Center);
                    } else if let Some(state) = &self.variable_scroll {
                        state.scroll_to(gpui::ListOffset {
                            item_ix: parent_index,
                            offset_in_item: px(0.),
                        });
                    }
                    crate::util::set_focus_visible(true, cx);
                    cx.stop_propagation();
                    return true;
                }
            }
        }
        false
    }

    /// The virtual bodies' PageUp/PageDown targets, by one viewport: the
    /// uniform body's `(fixed)` and the measured body's `(variable)`.
    fn page_moves(
        &self,
        from: Option<usize>,
        key_name: &str,
        cx: &App,
    ) -> (Option<usize>, Option<usize>) {
        let stops = &self.stops;
        let keys = &self.keys;
        let page_by_step = |from: usize, step: usize| match key_name {
            "pagedown" => {
                let boundary = from.saturating_add(step).min(keys.len() - 1);
                stops
                    .iter()
                    .copied()
                    .find(|stop| *stop >= boundary)
                    .or_else(|| stops.last().copied())
            }
            "pageup" => {
                let boundary = from.saturating_sub(step);
                stops
                    .iter()
                    .rev()
                    .copied()
                    .find(|stop| *stop <= boundary)
                    .or_else(|| stops.first().copied())
            }
            _ => None,
        };
        let fixed_page_move = from.and_then(|from| {
            let row_height = self.fixed_row_height?;
            let viewport_height = f32::from(self.fixed_scroll.viewport_bounds().size.height);
            if viewport_height <= 0. {
                return None;
            }
            let step =
                ((viewport_height / f32::from(row_height)).ceil() as usize).saturating_sub(1);
            page_by_step(from, step)
        });
        let variable_page_move = from.and_then(|from| {
            let viewport_height = self.variable_scroll.as_ref()?.viewport_bounds().size.height;
            let heights = self.variable_heights.as_ref()?.read(cx);
            let estimate = self.variable_estimate?;
            let height_at =
                |index: usize| heights.1.get(index).copied().flatten().unwrap_or(estimate);
            let mut distance = height_at(from);
            let mut target = from;
            match key_name {
                "pagedown" => {
                    if distance >= viewport_height {
                        return Some(target);
                    }
                    for next in stops.iter().copied().filter(|next| *next > from) {
                        for index in target + 1..=next {
                            distance += height_at(index);
                        }
                        target = next;
                        if distance >= viewport_height {
                            break;
                        }
                    }
                    Some(target)
                }
                "pageup" => {
                    if distance >= viewport_height {
                        return Some(target);
                    }
                    for previous in stops
                        .iter()
                        .rev()
                        .copied()
                        .filter(|previous| *previous < from)
                    {
                        for index in previous..target {
                            distance += height_at(index);
                        }
                        target = previous;
                        if distance >= viewport_height {
                            break;
                        }
                    }
                    Some(target)
                }
                _ => None,
            }
        });
        (fixed_page_move, variable_page_move)
    }

    /// PageUp out of the body into the first column header. Returns whether
    /// the focus left.
    fn page_up_to_header(
        &self,
        from: Option<usize>,
        key_name: &str,
        fixed_page_move: Option<usize>,
        variable_page_move: Option<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        // Pinned TableKeyboardDelegate sends PageDown to the last
        // enabled row, and PageUp out of the body entirely, into
        // the first column header. The header is focusable whether
        // or not it sorts, so this leaves the body rather than
        // stopping at its first row. Virtual tables page by
        // viewport first; only when the upward page move has
        // nowhere to go (the cursor is already the first enabled
        // stop, or the computed page target equals it) does
        // PageUp enter the header. The cursor stays seated so
        // Down from the header returns to the first enabled row.
        let plain_rows = self.plain_rows;
        if plain_rows && key_name == "pageup" {
            if let Some(header) = &self.page_up_header {
                window.focus(header, cx);
                cx.stop_propagation();
                return true;
            }
        } else if !plain_rows && key_name == "pageup" {
            let at_top = match from {
                Some(position) => {
                    self.stops.first().is_some_and(|first| *first == position)
                        || fixed_page_move.is_some_and(|target| Some(target) == from)
                        || variable_page_move.is_some_and(|target| Some(target) == from)
                }
                None => false,
            };
            if at_top {
                if let Some(header) = &self.page_up_header {
                    window.focus(header, cx);
                    cx.stop_propagation();
                    return true;
                }
            }
        }
        false
    }

    /// Moves the cursor to `next`, extending or replacing the selection the
    /// way the chord asks, and scrolls a virtual body to it.
    #[allow(clippy::too_many_arguments)]
    fn move_cursor(
        &self,
        next: usize,
        from: Option<usize>,
        key_name: &str,
        modifiers: gpui::Modifiers,
        initial_shift_settle: bool,
        is_variable_page: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let keys = &self.keys;
        let is_mac = cx.platform_info().is_mac();
        // Pinned `useSelectableCollection`: Shift extends a
        // multiple selection from the anchor with no other
        // chord, so plain Shift navigation is exact.
        let exact_shift_navigation = if is_mac {
            !modifiers.control && !modifiers.platform && !modifiers.function
        } else {
            !modifiers.alt && !modifiers.platform && !modifiers.function
        };
        let extends_selection = modifiers.shift
            && self.mode == SelectionMode::Multiple
            && exact_shift_navigation
            && !initial_shift_settle
            && Some(next) != from
            && shift_home_end_extends(key_name, modifiers.control, is_mac);
        if extends_selection {
            if let Some(target) = keys.get(next) {
                let range = self.selection_range.read(cx).clone();
                let next_selection = extend_selection_range(
                    &self.selected_now,
                    keys,
                    self.selectable_keys(),
                    &range,
                    target,
                );
                self.selection_range.update(cx, |range, _| {
                    if range.anchor.is_none() {
                        range.anchor = Some(target.clone());
                    }
                    range.current = Some(target.clone());
                    range.is_all = false;
                });
                if !same_selection(&next_selection, &self.selected_now) {
                    self.commit(&next_selection, window, cx);
                }
            }
        } else if self.select_on_focus
            && !modifiers.shift
            && !((is_mac && modifiers.alt) || (!is_mac && modifiers.control))
        {
            if let Some(target) = keys.get(next) {
                let next_selection = vec![target.clone()];
                if !same_selection(&next_selection, &self.selected_now) {
                    self.commit(&next_selection, window, cx);
                }
                self.selection_range.update(cx, |range, _| {
                    *range = TableSelectionRange {
                        anchor: Some(target.clone()),
                        current: Some(target.clone()),
                        ..TableSelectionRange::default()
                    };
                });
            }
        }
        self.held.update(cx, |v, cx| {
            *v = keys.get(next).cloned();
            cx.notify();
        });
        if self.fixed_virtual {
            self.fixed_scroll
                .scroll_to_item(next, crate::VirtualListScroll::Center);
        } else if let Some(state) = &self.variable_scroll {
            if is_variable_page {
                state.scroll_to_reveal_item(next);
            } else {
                // Unmeasured rows have no cached height, so
                // `scroll_to_reveal_item` cannot locate a far
                // target. Jump by logical index; the next
                // layout measures that row at the viewport.
                state.scroll_to(gpui::ListOffset {
                    item_ix: next,
                    offset_in_item: px(0.),
                });
            }
        }
    }

    /// Enter and Space on the cursor row: its action, or a selection change.
    fn activate(
        &self,
        index: usize,
        event: &gpui::KeyDownEvent,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut App,
    ) {
        let activation = if event.keystroke.key == "enter" {
            RowActivation::Enter
        } else {
            RowActivation::Space
        };
        match row_intent(
            activation,
            self.mode,
            self.selection_behavior,
            self.selected_now.is_empty(),
            self.on_row_click.is_some(),
        ) {
            RowIntent::Action => {
                if let Some(cb) = &self.on_row_click {
                    cb(&index, &ClickEvent::default(), window, cx);
                }
            }
            RowIntent::Selection => {
                if let Some(key) = self.keys.get(index) {
                    self.select_row(key, modifiers, window, cx);
                }
            }
            RowIntent::None => {}
        }
    }

    /// The keyboard's row-selection change, anchored the way a press is.
    fn select_row(
        &self,
        key: &SharedString,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mode = self.mode;
        let is_mac = cx.platform_info().is_mac();
        let selected_now = &self.selected_now;
        let was_selected = selected_now.contains(key);
        let extends_selection = modifiers.shift && mode == SelectionMode::Multiple;
        let next = if extends_selection {
            let range = self.selection_range.read(cx).clone();
            extend_selection_range(
                selected_now,
                &self.keys,
                self.selectable_keys(),
                &range,
                key,
            )
        } else {
            let non_contiguous = if is_mac {
                modifiers.alt
            } else {
                modifiers.control
            };
            crate::selection::next_selection_with_behavior(
                selected_now,
                key,
                mode,
                if non_contiguous {
                    SelectionBehavior::Toggle
                } else {
                    self.selection_behavior
                },
                self.disallow_empty_selection,
            )
        };
        let changed = !same_selection(&next, selected_now);
        if changed {
            self.commit(&next, window, cx);
        }
        if extends_selection && changed {
            self.selection_range.update(cx, |range, _| {
                if range.anchor.is_none() {
                    range.anchor = Some(key.clone());
                }
                range.current = Some(key.clone());
                range.is_all = false;
            });
        } else if !extends_selection && mode == SelectionMode::Multiple && !was_selected {
            self.selection_range.update(cx, |range, _| {
                range.anchor = Some(key.clone());
                range.current = Some(key.clone());
                range.is_all = false;
            });
        } else if !extends_selection && mode == SelectionMode::Multiple {
            self.selection_range.update(cx, |range, _| {
                if range.is_all {
                    *range = TableSelectionRange::default();
                }
            });
        }
    }
}

/// A zero-paint canvas over a flexible resizable column that records the
/// width the column laid out at, which is where a drag or a keyboard resize
/// starts from.
fn resize_width_probe(
    measured: gpui::Entity<Vec<Option<Pixels>>>,
    column_index: usize,
) -> impl IntoElement {
    gpui::canvas(
        move |bounds: gpui::Bounds<Pixels>, _, cx| {
            let width = px(f32::from(bounds.size.width).floor());
            measured.update(cx, |values, cx| {
                if values.len() <= column_index {
                    values.resize(column_index + 1, None);
                }
                if values[column_index] != Some(width) {
                    values[column_index] = Some(width);
                    cx.notify();
                }
            });
            bounds
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// What a press on a column's resize handle reads: the column, the width
/// the drag starts from, the width stores and the resize callbacks.
struct ResizePointerStart {
    column_index: usize,
    /// The width a resize starts from: the rendered width, else the
    /// measured one, else a 160px default.
    start_width: Pixels,
    /// The pointer drag in progress, as `(column, from_x, from_width)`.
    held: gpui::Entity<Option<(usize, f32, f32)>>,
    /// The column the keyboard is resizing.
    keyboard: gpui::Entity<Option<usize>>,
    widths: gpui::Entity<Vec<Option<Pixels>>>,
    columns: std::sync::Arc<Vec<TableColumn>>,
    measurements: std::sync::Arc<Vec<Option<Pixels>>>,
    focus: gpui::FocusHandle,
    on_resize_start: Option<OnResize>,
    on_resize_end: Option<OnResize>,
}

impl ResizePointerStart {
    /// A press on the handle: ends any keyboard resize, reports the start
    /// and seats the pointer drag.
    fn press(&self, ev: &gpui::MouseDownEvent, window: &mut Window, cx: &mut App) {
        let column_index = self.column_index;
        window.focus(&self.focus, cx);
        if let Some(active_column) = *self.keyboard.read(cx) {
            let current = final_column_widths(
                &self.columns,
                self.widths.read(cx),
                &self.measurements,
                active_column,
            );
            self.keyboard.update(cx, |active, _| *active = None);
            clear_controlled_resize_proposal(&self.columns, &self.widths, active_column, cx);
            if let Some(callback) = &self.on_resize_end {
                callback(&current, window, cx);
            }
        }
        clear_controlled_resize_proposal(&self.columns, &self.widths, column_index, cx);
        if let Some(callback) = &self.on_resize_start {
            let current = resolved_column_widths(
                &self.columns,
                self.widths.read(cx),
                &self.measurements,
                None,
            );
            callback(&current, window, cx);
        }
        let x = f32::from(ev.position.x);
        let start_width = self.start_width;
        self.held.update(cx, |v, _| {
            *v = Some((column_index, x, f32::from(start_width)));
        });
    }
}

// ---- Table::render part renderers ----
impl Table {
    /// The visible window onto `virtual_rows`: every key, the rows a tree's
    /// open parents expose, and (for multiple selection) the enabled keys.
    /// Cached in the window's keyed store until the collection, the expanded
    /// set or the disabled keys change.
    fn virtual_projection(
        &self,
        base_id: &gpui::ElementId,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<std::sync::Arc<VirtualProjection>> {
        let needs_selectable_keys = self.selection_mode == SelectionMode::Multiple;
        self.virtual_rows
            .as_ref()
            .map(|(count, identity, key_for_row, _)| {
                let cache = window.use_keyed_state(
                    element_id::scoped(base_id, "virtual-projection"),
                    cx,
                    |_, _| None::<std::sync::Arc<VirtualProjection>>,
                );
                let cached = cache.read(cx).clone();
                let tree = self.virtual_tree_metadata.is_some();
                match cached {
                    Some(cached)
                        if cached.count == *count
                            && cached.identity == *identity
                            && cached.tree == tree
                            && (!tree
                                || (cached.expanded.len() == self.expanded_keys.len()
                                    && self
                                        .expanded_keys
                                        .iter()
                                        .all(|key| cached.expanded.contains(key)))) =>
                    {
                        if !needs_selectable_keys
                            || cached
                                .selectable_disabled_keys
                                .as_ref()
                                .is_some_and(|disabled| {
                                    disabled.as_slice() == self.disabled_keys.as_slice()
                                })
                        {
                            cached
                        } else {
                            let projection = std::sync::Arc::new(VirtualProjection {
                                count: cached.count,
                                identity: cached.identity.clone(),
                                expanded: std::sync::Arc::clone(&cached.expanded),
                                tree: cached.tree,
                                full_keys: std::sync::Arc::clone(&cached.full_keys),
                                visible: std::sync::Arc::clone(&cached.visible),
                                selectable_collection_keys: Some(filtered_selectable_keys(
                                    cached.full_keys.as_slice(),
                                    &self.disabled_keys,
                                )),
                                selectable_disabled_keys: Some(std::sync::Arc::new(
                                    self.disabled_keys.clone(),
                                )),
                            });
                            cache.update(cx, |slot, _| {
                                *slot = Some(std::sync::Arc::clone(&projection));
                            });
                            projection
                        }
                    }
                    _ => {
                        let mut full_keys = Vec::with_capacity(*count);
                        let mut visible = Vec::with_capacity(*count);
                        if let Some(project) = &self.virtual_tree_metadata {
                            let mut visible_by_key =
                                std::collections::HashMap::with_capacity(*count);
                            for source_index in 0..*count {
                                let key = key_for_row(source_index);
                                let metadata = project(source_index);
                                let is_visible =
                                    metadata.parent_key.as_ref().is_none_or(|parent| {
                                        visible_by_key.get(parent).copied().unwrap_or(false)
                                            && self.expanded_keys.contains(parent)
                                    });
                                visible_by_key.insert(key.clone(), is_visible);
                                full_keys.push(key.clone());
                                if is_visible {
                                    visible.push((source_index, key, metadata));
                                }
                            }
                        } else {
                            for source_index in 0..*count {
                                let key = key_for_row(source_index);
                                full_keys.push(key.clone());
                                visible.push((source_index, key, VirtualTreeMetadata::default()));
                            }
                        }
                        let full_keys = std::sync::Arc::new(full_keys);
                        let selectable_collection_keys = needs_selectable_keys.then(|| {
                            filtered_selectable_keys(full_keys.as_slice(), &self.disabled_keys)
                        });
                        let selectable_disabled_keys = needs_selectable_keys
                            .then(|| std::sync::Arc::new(self.disabled_keys.clone()));
                        let projection = std::sync::Arc::new(VirtualProjection {
                            count: *count,
                            identity: identity.clone(),
                            expanded: if tree {
                                std::sync::Arc::new(self.expanded_keys.iter().cloned().collect())
                            } else {
                                std::sync::Arc::default()
                            },
                            tree,
                            full_keys,
                            visible: std::sync::Arc::new(visible),
                            selectable_collection_keys,
                            selectable_disabled_keys,
                        });
                        cache.update(cx, |slot, _| {
                            *slot = Some(std::sync::Arc::clone(&projection));
                        });
                        projection
                    }
                }
            })
    }

    /// The measured heights of an `estimatedRowHeight` virtual body, keyed by
    /// the visible keys so a changed window starts unmeasured.
    fn virtual_row_heights(
        &self,
        base_id: &gpui::ElementId,
        virtual_visible_keys: Option<&Vec<SharedString>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<gpui::Entity<(Vec<SharedString>, Vec<Option<Pixels>>)>> {
        match (
            &self.virtual_rows,
            self.estimated_row_height,
            virtual_visible_keys,
        ) {
            (Some((_, identity, _, _)), Some(_), Some(keys)) if self.row_height.is_none() => {
                let count = keys.len();
                let state = window.use_keyed_state(
                    element_id::scoped(
                        &element_id::scoped(base_id, "row-heights"),
                        identity.clone(),
                    ),
                    cx,
                    |_, _| (Vec::<SharedString>::new(), Vec::<Option<Pixels>>::new()),
                );
                if state.read(cx).0.as_slice() != keys.as_slice() {
                    let stored_keys = keys.clone();
                    state.update(cx, |stored, _| {
                        *stored = (stored_keys, vec![None; count]);
                    });
                }
                Some(state)
            }
            _ => None,
        }
    }

    /// The intrusive `gpui::list` state of an `estimatedRowHeight` virtual
    /// body, reset whenever the visible row count changes.
    fn virtual_list_state(
        &self,
        base_id: &gpui::ElementId,
        virtual_visible_count: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<gpui::ListState> {
        match (
            self.row_height,
            self.estimated_row_height,
            &self.virtual_rows,
        ) {
            (None, Some(estimate), Some((_, identity, _, _))) => {
                let count = virtual_visible_count;
                let overdraw = self.max_h.unwrap_or(px(400.)).max(estimate * 3.);
                let state = window
                    .use_keyed_state(
                        element_id::scoped(
                            &element_id::scoped(base_id, "list-state"),
                            identity.clone(),
                        ),
                        cx,
                        move |_, _| crate::VirtualListHandle::with_overdraw(count, overdraw),
                    )
                    .read(cx)
                    .list_state()
                    .clone();
                if state.item_count() != count {
                    state.reset(count);
                }
                Some(state)
            }
            _ => None,
        }
    }

    /// One focus handle per column header. Only a sortable header is a tab
    /// stop (Enter and Space sort it); the rest are focusable so PageUp has a
    /// header to land on.
    fn header_focus_handles(
        &self,
        base_id: &gpui::ElementId,
        sortable: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<gpui::FocusHandle> {
        self.columns
            .iter()
            .enumerate()
            .map(|(i, column)| {
                let id = element_id::scoped(&element_id::indexed(base_id, "sort", i), "focus");
                if column.allows_sorting && sortable {
                    crate::util::tab_stop_handle(id, window, cx)
                } else {
                    window
                        .use_keyed_state(id, cx, |_, cx| cx.focus_handle())
                        .read(cx)
                        .clone()
                }
            })
            .collect()
    }

    /// The tab stop on each resizable column's handle; `None` for a column
    /// that does not resize.
    fn resize_focus_handles(
        &self,
        base_id: &gpui::ElementId,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<Option<gpui::FocusHandle>> {
        self.columns
            .iter()
            .enumerate()
            .map(|(i, column)| {
                column.allows_resizing.then(|| {
                    crate::util::tab_stop_handle(
                        element_id::scoped(&element_id::indexed(base_id, "resize", i), "focus"),
                        window,
                        cx,
                    )
                })
            })
            .collect()
    }

    /// The selection column's header: v3's `columnheader` holding the
    /// select-all checkbox for a multiple selection, an empty aligned cell
    /// otherwise.
    #[allow(clippy::too_many_arguments)]
    fn select_all_header_cell(
        &self,
        base_id: &gpui::ElementId,
        colors: &herogpui_theme::ThemeColors,
        secondary: bool,
        selectable_collection_keys: &Option<std::sync::Arc<Vec<SharedString>>>,
        selection_own: &Option<gpui::Entity<Vec<SharedString>>>,
        selection_range: &gpui::Entity<TableSelectionRange>,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        // The select-all box only makes sense for a multiple selection; a
        // single-selection table keeps the column for alignment.
        let mut cell = gpui::div()
            .id(element_id::scoped(base_id, "select-all-cell"))
            // The selection column is a real column of the collection
            // upstream, so its header is a `role: 'columnheader'` like any
            // other (`useTableColumnHeader.mjs`), holding the select-all
            // checkbox `useTableSelectAllCheckbox` names.
            .a11y(a11y::Role::ColumnHeader)
            .a11y_column_index(0)
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .w(px(44.))
            .py(px(10.));
        if secondary {
            let radius = cx.layout().radius_2xl().min(px(32.));
            cell = cell
                .bg(colors.surface_secondary)
                .rounded_tl(radius)
                .rounded_bl(radius);
        }
        if self.selection_mode == SelectionMode::Multiple {
            // The `Mod+A` keydown handler reads the same set, so it gets a
            // clone rather than the variable itself.
            let all = selectable_collection_keys
                .as_ref()
                .expect("multiple Tables have selectable keys")
                .clone();
            let (all_selected, indeterminate) =
                select_all_flags(all.as_slice(), &self.selected_keys);
            let mut box_el = Checkbox::new(element_id::scoped(base_id, "select-all"))
                .is_selected(all_selected)
                .is_indeterminate(indeterminate)
                .aria_label(crate::i18n::ui_string(crate::i18n::UiString::SelectAll, cx));
            let cb = self.on_selection_change.clone();
            if cb.is_some() || selection_own.is_some() {
                let selection_own = selection_own.clone();
                let selection_range = selection_range.clone();
                let disallow_empty_selection = self.disallow_empty_selection;
                box_el = box_el.on_change(move |_next, window, cx| {
                    if all_selected && disallow_empty_selection {
                        return;
                    }
                    // Anything short of everything selects everything.
                    let next: Vec<SharedString> = if all_selected {
                        Vec::new()
                    } else {
                        all.as_slice().to_vec()
                    };
                    if let Some(held) = &selection_own {
                        held.update(cx, |value, cx| {
                            *value = next.clone();
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &cb {
                        cb(&next, window, cx);
                    }
                    selection_range.update(cx, |range, _| {
                        *range = if all_selected {
                            TableSelectionRange::default()
                        } else {
                            TableSelectionRange {
                                is_all: true,
                                ..TableSelectionRange::default()
                            }
                        };
                    });
                });
            }
            cell = cell.child(box_el);
        }
        cell
    }

    /// One column's header cell: the `.table__column` label with its sort
    /// indicator and separator, wrapped in a sort target when the column
    /// sorts, and in the resize container when it resizes.
    fn column_header_cell(
        &self,
        hx: &HeaderParts<'_>,
        column_index: usize,
        column: &TableColumn,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let HeaderParts {
            base_id,
            colors,
            secondary,
            selectable,
            effective_widths,
            layout_width_bounds,
            measured_widths,
            header_focus,
            header_focused,
            ..
        } = *hx;
        let sorted = self
            .sort_descriptor
            .as_ref()
            .filter(|d| d.column == column.label);
        // A resized column keeps the width the drag left it at.
        let effective = effective_widths[column_index];
        let (layout_min, layout_max) = layout_width_bounds[column_index];
        let first_header = column_index == 0 && !selectable;
        let last_header = column_index + 1 == self.columns.len();
        let mut cell = gpui::div()
            .when(effective.is_none(), flex_cell)
            .when_some(effective, |c, w| c.w(w))
            .when_some(layout_min, |c, w| c.min_w(w))
            .when_some(layout_max, |c, w| c.max_w(w))
            .flex()
            .items_center()
            .gap(px(4.))
            // `.table__column` is `px-4 py-2.5 text-xs`.
            .px(px(16.))
            .py(px(10.))
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(gpui::FontWeight::MEDIUM)
            .when(column.allows_sorting, |c| c.w_full().justify_between())
            .text_color(if sorted.is_some() {
                colors.foreground
            } else {
                colors.muted
            })
            .relative();
        if !column.allows_resizing && !cfg!(target_arch = "wasm32") {
            let measurements = measured_widths.clone();
            // The label and the sort indicator; a focused header's ring is an
            // overlay as wide as the cell (see `RowCtx::data_cell`).
            let content_children = 1 + usize::from(sorted.is_some() && self.show_indicator);
            cell = cell.on_children_prepainted(move |bounds, _, cx| {
                let content =
                    intrinsic_children_width(&bounds[..content_children.min(bounds.len())]);
                if content <= 0. {
                    return;
                }
                let next = px(content + 32.);
                measurements.update(cx, |values, cx| {
                    if values.len() <= column_index {
                        values.resize(column_index + 1, None);
                    }
                    if values[column_index].is_none_or(|current| next > current) {
                        values[column_index] = Some(next);
                        cx.notify();
                    }
                });
            });
        }
        cell = cell.child(column.label.clone());
        let header_selector = format!("table-header-track-{column_index}");
        cell = cell.debug_selector(move || header_selector);

        if secondary {
            let radius = cx.layout().radius_2xl().min(px(32.));
            cell = cell.bg(colors.surface_secondary);
            if first_header {
                cell = cell.rounded_tl(radius).rounded_bl(radius);
            }
            if last_header {
                cell = cell.rounded_tr(radius).rounded_br(radius);
            }
        }

        if let Some(descriptor) = sorted {
            if self.show_indicator {
                // `indicator` is v3's render prop on
                // `Table.SortableColumnHeader`: it receives the direction
                // the column is sorted in and replaces the chevron.
                cell = cell.child(match &self.indicator {
                    Some(render) => render(descriptor.direction),
                    None => {
                        let indicator_id = element_id::indexed(
                            &element_id::scoped(base_id, "sort-indicator"),
                            "column",
                            column_index,
                        );
                        crate::anim::rotating_indicator_with_duration(
                            &indicator_id,
                            descriptor.direction == SortDirection::Descending,
                            gpui::svg()
                                .size(px(12.))
                                .path(icons::CHEVRON_UP)
                                // svg() never inherits text colour.
                                .text_color(colors.foreground),
                            100,
                            window,
                            cx,
                        )
                    }
                });
            }
        }

        if !last_header && !column.allows_resizing {
            cell = cell.child(
                gpui::div()
                    .absolute()
                    .right_0()
                    .top(px(10.))
                    .h(px(16.))
                    .w(px(1.))
                    .rounded(crate::util::hairline_radius(cx))
                    .bg(colors.separator),
            );
        }

        // A sortable header wraps the cell in a click target, which is a
        // different element type, so both branches unify to AnyElement.
        let cell = match (column.allows_sorting, self.on_sort_change.clone()) {
            (true, Some(cb)) => {
                let next =
                    SortDescriptor::next(self.sort_descriptor.as_ref(), column.label.clone());
                // `.table__column[data-allows-sorting]:hover` recolours the
                // header text to `--foreground`; it paints no background.
                // A group name is a plain string, not an `ElementId`, so it
                // cannot carry structure -- but it still has to start from
                // this table's own id, or two tables share the group.
                let sort_group: SharedString =
                    format!("{}-sort-hover-{}", self.id, column.label).into();
                let header_cell = gpui::div()
                    .id(element_id::scoped(
                        &element_id::scoped(base_id, "sort"),
                        column.label.clone(),
                    ))
                    // `useTableColumnHeader.mjs` is `role: 'columnheader'`
                    // with `'aria-colindex'` from the grid cell props
                    // underneath it and `'aria-sort'` on a sortable
                    // column. The sort direction has no gpui builder —
                    // upstream itself drops it on Android Talkback and
                    // duplicates it into `aria-describedby` instead — so
                    // it is a recorded omission in `crate::a11y`.
                    .a11y_named(
                        a11y::Role::ColumnHeader,
                        &a11y::Name::labelled(column.label.clone()),
                    )
                    .a11y_column_index(column_index + usize::from(selectable))
                    .group(sort_group.clone())
                    // The wrapper owns the column's track, so it reads the
                    // same triple the body cell read: a fixed width when
                    // one wins, otherwise the shared flex floor. Leaving
                    // the wrapper an unconditional equal-growth track
                    // ignored the width entirely and resolved the header's
                    // tracks separately from the body's.
                    .when(effective.is_none(), |wrapper| {
                        wrapper.flex_basis(px(0.)).flex_grow(1.).flex_shrink(1.)
                    })
                    .when_some(effective, |wrapper, w| wrapper.w(w))
                    .when_some(layout_min, |wrapper, w| wrapper.min_w(w))
                    .when_some(layout_max, |wrapper, w| wrapper.max_w(w))
                    .flex()
                    .cursor(crate::util::interactive_cursor(cx))
                    // The focus is what makes Enter and Space sort: gpui
                    // fires a *focused* element's click listeners for them.
                    .track_focus(&header_focus[column_index])
                    .on_click(move |_, window, cx| cb(&next.clone(), window, cx))
                    .child(cell.group_hover(sort_group, |s| s.text_color(colors.foreground)));
                // `.table__column` rings *inside* itself: the next column
                // is flush against this one, and a ring drawn outside bled
                // through the transparent cell and filled it.
                let header_cell = header_cell
                    .relative()
                    .when(header_focused[column_index], |c| {
                        c.child(crate::util::inset_focus_ring(cx))
                    });
                crate::util::record_focus_bounds(
                    header_cell,
                    &header_focus[column_index],
                    window,
                    cx,
                )
                .into_any_element()
            }
            // Not sortable, so nothing to press -- but still focusable, so
            // PageUp has a header to land on, and it rings when it does.
            _ => {
                let header_cell = cell
                .id(element_id::indexed(base_id, "header", column_index))
                // The same `role: 'columnheader'`; a column that does not
                // sort simply has no `aria-sort` upstream either
                // (`ariaSort` stays `undefined` unless `allowsSorting`).
                .a11y_named(
                    a11y::Role::ColumnHeader,
                    &a11y::Name::labelled(column.label.clone()),
                )
                .a11y_column_index(column_index + usize::from(selectable))
                .track_focus(&header_focus[column_index])
                .relative()
                .when(header_focused[column_index], |c| {
                    c.child(crate::util::inset_focus_ring(cx))
                });
                crate::util::record_focus_bounds(
                    header_cell,
                    &header_focus[column_index],
                    window,
                    cx,
                )
                .into_any_element()
            }
        };

        // `allowsResizing` puts a handle on the column's trailing edge. The
        // wrapper is what keeps the handle inside the column's box.
        // `.table__resizable-container` is the box that keeps the handle
        // inside the column, which is what this wrapper is.
        if column.allows_resizing {
            self.resizable_column_header(hx, column_index, column, cell, cx)
        } else {
            cell
        }
    }

    /// `.table__resizable-container`: the box that keeps a resizable
    /// column's handle inside the column, with the handle's pointer drag
    /// start, outside-press exit and keyboard resizing.
    fn resizable_column_header(
        &self,
        hx: &HeaderParts<'_>,
        column_index: usize,
        column: &TableColumn,
        cell: AnyElement,
        cx: &mut App,
    ) -> AnyElement {
        let HeaderParts {
            base_id,
            colors,
            effective_widths,
            layout_width_bounds,
            measured_widths,
            measured_widths_now,
            resize_focus,
            resize_focused,
            resized,
            dragging,
            drag_now,
            keyboard_resizing,
            keyboard_resize_now,
            resize_limits,
            resize_columns,
            resize_measurements,
            ..
        } = *hx;
        let effective = effective_widths[column_index];
        let (layout_min, layout_max) = layout_width_bounds[column_index];
        let start_width = effective
            .or_else(|| measured_widths_now.get(column_index).copied().flatten())
            .unwrap_or(px(160.));
        let keyboard = keyboard_resizing.clone();
        let keyboard_out = keyboard.clone();
        let widths = resized.clone();
        let widths_for_outside = widths.clone();
        let (min_width, max_width) = resize_limits[column_index];
        let controlled = column.width.is_some();
        // As with the sort hover group: a string, but this table's own.
        let resizer_group: SharedString = format!("{}-resizer-{column_index}", self.id).into();
        let accent_color = colors.accent.color;
        let focus_color = colors.focus;
        let is_resizing = drag_now.is_some_and(|(index, _, _)| index == column_index)
            || keyboard_resize_now == Some(column_index);
        let measured = measured_widths.clone();
        let columns_for_outside = resize_columns.clone();
        let columns_for_keys = resize_columns.clone();
        let measurements_for_outside = resize_measurements.clone();
        let measurements_for_keys = resize_measurements.clone();
        let resize_start_for_keys = self.on_resize_start.clone();
        let resize_for_keys = self.on_resize.clone();
        let resize_end_for_outside = self.on_resize_end.clone();
        let resize_end_for_keys = self.on_resize_end.clone();
        let pointer_start = ResizePointerStart {
            column_index,
            start_width,
            held: dragging.clone(),
            keyboard: keyboard.clone(),
            widths: widths.clone(),
            columns: resize_columns.clone(),
            measurements: resize_measurements.clone(),
            focus: resize_focus[column_index]
                .as_ref()
                .expect("resizable columns have a focus handle")
                .clone(),
            on_resize_start: self.on_resize_start.clone(),
            on_resize_end: self.on_resize_end.clone(),
        };
        gpui::div()
            .relative()
            .when(effective.is_none(), flex_cell)
            .when_some(effective, |c, w| c.w(w))
            // The wrapper owns the column's track, so it carries the
            // shared bounds instead of leaning on the inner cell's
            // automatic minimum (see `layout_width_bounds`).
            .when_some(layout_min, |wrapper, w| wrapper.min_w(w))
            .when_some(layout_max, |wrapper, w| wrapper.max_w(w))
            .when(effective.is_none(), |wrapper| {
                wrapper.child(resize_width_probe(measured, column_index))
            })
            .child(cell)
            .child(
                gpui::div()
                    .id(element_id::indexed(base_id, "resize", column_index))
                    .track_focus(
                        resize_focus[column_index]
                            .as_ref()
                            .expect("resizable columns have a focus handle"),
                    )
                    .group(resizer_group.clone())
                    .absolute()
                    .top(px(0.))
                    // `px-2` around a `w-px` line, `box-content`: an
                    // 8px grab margin either side of the column edge.
                    .right(px(-8.))
                    .w(px(17.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        // `h-4 w-px rounded-sm bg-separator`, and
                        // `h-full w-0.5 bg-accent` while hovered.
                        gpui::div()
                            .w(px(1.))
                            .h(px(16.))
                            .rounded(crate::util::hairline_radius(cx))
                            .bg(colors.separator)
                            .group_hover(resizer_group, |s| {
                                s.w(px(2.)).h_full().bg(accent_color)
                            })
                            .when(is_resizing, |s| {
                                s.w(px(2.)).h_full().bg(accent_color)
                            })
                            .when(resize_focused[column_index], |s| {
                                s.w(px(2.)).h_full().bg(focus_color)
                            }),
                    )
                    .cursor(gpui::CursorStyle::ResizeLeftRight)
                    .on_mouse_down(gpui::MouseButton::Left, move |ev, window, cx| {
                        pointer_start.press(ev, window, cx);
                    })
                    .on_mouse_down_out(move |_, window, cx| {
                        if *keyboard_out.read(cx) == Some(column_index) {
                            let current = final_column_widths(
                                &columns_for_outside,
                                widths_for_outside.read(cx),
                                &measurements_for_outside,
                                column_index,
                            );
                            keyboard_out.update(cx, |active, cx| {
                                *active = None;
                                cx.notify();
                            });
                            clear_controlled_resize_proposal(
                                &columns_for_outside,
                                &widths_for_outside,
                                column_index,
                                cx,
                            );
                            if let Some(callback) = &resize_end_for_outside {
                                callback(&current, window, cx);
                            }
                        }
                    })
                    .on_key_down(move |event, window, cx| {
                        let key = event.keystroke.key.as_str();
                        let editing = *keyboard.read(cx) == Some(column_index);
                        match key {
                            "enter" => {
                                if editing {
                                    let current = final_column_widths(
                                        &columns_for_keys,
                                        widths.read(cx),
                                        &measurements_for_keys,
                                        column_index,
                                    );
                                    keyboard.update(cx, |active, cx| {
                                        *active = None;
                                        cx.notify();
                                    });
                                    clear_controlled_resize_proposal(
                                        &columns_for_keys,
                                        &widths,
                                        column_index,
                                        cx,
                                    );
                                    if let Some(callback) = &resize_end_for_keys {
                                        callback(&current, window, cx);
                                    }
                                } else {
                                    clear_controlled_resize_proposal(
                                        &columns_for_keys,
                                        &widths,
                                        column_index,
                                        cx,
                                    );
                                    let current = resolved_column_widths(
                                        &columns_for_keys,
                                        widths.read(cx),
                                        &measurements_for_keys,
                                        None,
                                    );
                                    keyboard.update(cx, |active, cx| {
                                        *active = Some(column_index);
                                        cx.notify();
                                    });
                                    if let Some(callback) = &resize_start_for_keys {
                                        callback(&current, window, cx);
                                    }
                                }
                                cx.stop_propagation();
                            }
                            "escape" | "space" | "tab" if editing => {
                                let current = final_column_widths(
                                    &columns_for_keys,
                                    widths.read(cx),
                                    &measurements_for_keys,
                                    column_index,
                                );
                                keyboard.update(cx, |active, cx| {
                                    *active = None;
                                    cx.notify();
                                });
                                clear_controlled_resize_proposal(
                                    &columns_for_keys,
                                    &widths,
                                    column_index,
                                    cx,
                                );
                                if let Some(callback) = &resize_end_for_keys {
                                    callback(&current, window, cx);
                                }
                                cx.stop_propagation();
                            }
                            "right" | "up" | "left" | "down" if editing => {
                                let delta = if matches!(key, "right" | "up") {
                                    10.
                                } else {
                                    -10.
                                };
                                let mut proposed = start_width;
                                widths.update(cx, |values, cx| {
                                    if values.len() <= column_index {
                                        values.resize(column_index + 1, None);
                                    }
                                    let current = if controlled {
                                        start_width
                                    } else {
                                        values[column_index].unwrap_or(start_width)
                                    };
                                    let next = (f32::from(current) + delta)
                                        .floor()
                                        .min(max_width)
                                        .max(min_width);
                                    proposed = px(next);
                                    values[column_index] = Some(proposed);
                                    cx.notify();
                                });
                                if let Some(callback) = &resize_for_keys {
                                    let current = resolved_column_widths(
                                        &columns_for_keys,
                                        widths.read(cx),
                                        &measurements_for_keys,
                                        Some((column_index, proposed)),
                                    );
                                    callback(&current, window, cx);
                                }
                                cx.stop_propagation();
                            }
                            _ => {}
                        }
                    }),
            )
            .into_any_element()
    }

    /// The drag itself: the pointer can leave the table, so paint-time
    /// window listeners own the move and release until the drag ends.
    fn column_resize_drag(
        &self,
        mut table: gpui::Stateful<gpui::Div>,
        dragging: gpui::Entity<Option<(usize, f32, f32)>>,
        resized: gpui::Entity<Vec<Option<Pixels>>>,
        resize_columns: std::sync::Arc<Vec<TableColumn>>,
        resize_measurements: std::sync::Arc<Vec<Option<Pixels>>>,
    ) -> gpui::Stateful<gpui::Div> {
        let held = dragging.clone();
        let held_up = dragging;
        let widths = resized;
        let widths_up = widths.clone();
        let columns_for_move = resize_columns.clone();
        let columns_for_up = resize_columns;
        let measurements_for_move = resize_measurements.clone();
        let measurements_for_up = resize_measurements;
        let resize_callback = self.on_resize.clone();
        let resize_end_callback = self.on_resize_end.clone();
        table = table.relative().child(
            gpui::canvas(
                |bounds, _, _| bounds,
                move |_, _, window, _| {
                    let held = held.clone();
                    let widths = widths.clone();
                    let columns_for_move = columns_for_move.clone();
                    let measurements_for_move = measurements_for_move.clone();
                    let resize_callback = resize_callback.clone();
                    window.on_mouse_event(
                        move |event: &gpui::MouseMoveEvent, phase, window, cx| {
                            if phase != gpui::DispatchPhase::Capture
                                || event.pressed_button != Some(gpui::MouseButton::Left)
                            {
                                return;
                            }
                            let Some((column, from_x, from_w)) = *held.read(cx) else {
                                return;
                            };
                            let raw = (from_w + f32::from(event.position.x) - from_x).floor();
                            let min = columns_for_move[column]
                                .min_width
                                .map_or(DEFAULT_COLUMN_MIN_WIDTH, f32::from);
                            let max = columns_for_move[column]
                                .max_width
                                .map_or(f32::MAX, f32::from);
                            let proposed = px(raw.min(max).max(min));
                            widths.update(cx, |values, cx| {
                                if values.len() <= column {
                                    values.resize(column + 1, None);
                                }
                                values[column] = Some(proposed);
                                cx.notify();
                            });
                            if let Some(callback) = &resize_callback {
                                let current = resolved_column_widths(
                                    &columns_for_move,
                                    widths.read(cx),
                                    &measurements_for_move,
                                    Some((column, proposed)),
                                );
                                callback(&current, window, cx);
                            }
                        },
                    );

                    let held_up = held_up.clone();
                    let widths_up = widths_up.clone();
                    let columns_for_up = columns_for_up.clone();
                    let measurements_for_up = measurements_for_up.clone();
                    let resize_end_callback = resize_end_callback.clone();
                    window.on_mouse_event(move |event: &gpui::MouseUpEvent, phase, window, cx| {
                        if phase != gpui::DispatchPhase::Capture
                            || event.button != gpui::MouseButton::Left
                        {
                            return;
                        }
                        let drag = *held_up.read(cx);
                        if let Some((column, _, _)) = drag {
                            let current = final_column_widths(
                                &columns_for_up,
                                widths_up.read(cx),
                                &measurements_for_up,
                                column,
                            );
                            held_up.update(cx, |value, cx| {
                                *value = None;
                                cx.notify();
                            });
                            clear_controlled_resize_proposal(
                                &columns_for_up,
                                &widths_up,
                                column,
                                cx,
                            );
                            if let Some(callback) = &resize_end_callback {
                                callback(&current, window, cx);
                            }
                        }
                    });
                },
            )
            .absolute()
            .inset_0(),
        );
        table
    }

    /// v3 gives a table a roving row focus: the arrows walk it, Home and End
    /// jump, Page Up/Down page (PageUp leaves for the header), Enter
    /// activates the row, Space and `Mod+A` select, a tree row opens and
    /// closes, and typing jumps by typeahead -- the same resolver every list
    /// here uses, over the rows that exist.
    fn row_keyboard(&self, mut wrapper: gpui::Div, keys: RowKeyboard<'_>) -> gpui::Div {
        let RowKeyboard {
            ctx,
            expanded_keys,
            header_focus,
            row_cursor,
            selectable_collection_keys,
            selection_own,
            selection_range,
            table_focus,
            tree_rows,
            typeahead_labels,
            typeahead,
            virtual_list_state,
            virtual_row_heights,
            virtual_scroll_now,
            virtual_text_value,
            virtual_typeahead_indices,
        } = keys;
        let stops: Vec<usize> = ctx
            .row_keys
            .iter()
            .enumerate()
            .filter_map(|(index, key)| (!self.disabled_keys.contains(key)).then_some(index))
            .collect();
        let keys = ctx.row_keys.clone();
        if keys.is_empty() {
            return wrapper;
        }
        let mode = self.selection_mode;
        let selection_behavior = self.selection_behavior;
        let fixed_virtual = self.row_height.is_some() && self.virtual_rows.is_some();
        let fixed_scroll = virtual_scroll_now.clone();
        let variable_scroll = virtual_list_state.clone();
        let typeahead_navigation = std::rc::Rc::new(TableTypeaheadNavigation {
            state: typeahead,
            labels: typeahead_labels,
            virtual_indices: virtual_typeahead_indices,
            virtual_text: virtual_text_value,
            stops: stops.clone(),
            keys: keys.clone(),
            cursor: row_cursor.clone(),
            fixed_virtual,
            fixed_scroll: fixed_scroll.clone(),
            variable_scroll: variable_scroll.clone(),
        });
        let capture_typeahead = typeahead_navigation.clone();
        let capture_typeahead_up = typeahead_navigation.clone();
        let capture_focus = table_focus.clone();
        let capture_focus_up = table_focus.clone();
        wrapper = wrapper.capture_key_down(move |event, window, cx| {
            if !capture_focus.contains_focused(window, cx) {
                return;
            }
            let key_name = event.keystroke.key.as_str();
            let typed = event.keystroke.key_char.as_deref().unwrap_or(key_name);
            let modifiers = &event.keystroke.modifiers;
            let now = web_time::Instant::now();
            let is_space = key_name == "space" || typed == " ";
            if is_space
                && capture_typeahead.state.read(cx).is_active(now)
                && !modifiers.control
                && !modifiers.platform
                && !modifiers.alt
            {
                cx.stop_propagation();
                capture_typeahead.push(" ", now, false, cx);
            }
        });
        wrapper = wrapper.capture_key_up(move |event, window, cx| {
            if !capture_focus_up.contains_focused(window, cx) {
                return;
            }
            let key_name = event.keystroke.key.as_str();
            let typed = event.keystroke.key_char.as_deref().unwrap_or(key_name);
            let modifiers = &event.keystroke.modifiers;
            let is_space = key_name == "space" || typed == " ";
            if is_space
                && capture_typeahead_up
                    .state
                    .read(cx)
                    .is_active(web_time::Instant::now())
                && !modifiers.control
                && !modifiers.platform
                && !modifiers.alt
            {
                cx.stop_propagation();
            }
        });
        let nav = RowKeyNav {
            stops,
            held: row_cursor,
            keys,
            table_focus,
            typeahead: typeahead_navigation,
            on_row_click: self.on_row_click.clone(),
            selection: self.on_selection_change.clone(),
            selection_own,
            selection_range,
            selected_now: self.selected_keys.clone(),
            selectable_collection_keys,
            mode,
            selection_behavior,
            disallow_empty_selection: self.disallow_empty_selection,
            // React Aria's collection hook defaults `selectOnFocus` to true
            // for `selectionBehavior="replace"`.
            select_on_focus: selection_behavior == SelectionBehavior::Replace
                && mode != SelectionMode::None,
            plain_rows: self.virtual_rows.is_none(),
            fixed_virtual,
            // Pinned `TableKeyboardDelegate` pages by one visible rectangle, so
            // the step reads the virtual body's own laid-out viewport -- the
            // uniform handle's `viewport_bounds()` -- and not the configured
            // `max_h` cap: a bounded parent (or a resized window) shows fewer
            // rows than the cap allows. A zero viewport answers nothing, which
            // the shared resolver turns into no movement.
            fixed_row_height: self.row_height.filter(|_| fixed_virtual),
            fixed_scroll,
            variable_scroll,
            variable_heights: virtual_row_heights.clone(),
            variable_estimate: self.estimated_row_height,
            expanded: expanded_keys,
            on_expanded: self.on_expanded_change.clone(),
            tree_rows,
            // The header PageUp hands the focus to. Cloned out because the
            // handler outlives this frame's `header_focus`.
            page_up_header: header_focus.first().cloned(),
            headers: header_focus,
        };
        wrapper.on_key_down(move |event, window, cx| nav.key_down(event, window, cx))
    }

    /// Fills `.table__body`: a uniform [`VirtualList`](crate::VirtualList) for fixed-height virtual rows, a
    /// measured [`VirtualList`](crate::VirtualList) for `estimatedRowHeight`, or every flattened
    /// plain row.
    fn body_rows(
        &self,
        mut body: gpui::Stateful<gpui::Div>,
        rows_in: BodyRows<'_>,
        window: &Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let BodyRows {
            base_id,
            table_id,
            ctx,
            flat,
            row_count,
            virtual_projection,
            virtual_list_state,
            virtual_row_heights,
            virtual_scroll_now,
        } = rows_in;
        // `estimatedRowHeight` takes the variable-height path: gpui's `list`
        // measures each row it builds, and its state is intrusive, so it lives in
        // the window's keyed store and resets when the count changes.
        if let (Some(row_height), Some((_, _, _, factory))) =
            (self.row_height, self.virtual_rows.clone())
        {
            // The body scrolls inside a uniform `VirtualList`, which asks for the rows the
            // viewport shows and no others. A fixed height caps the roomy-window
            // body at the configured value, so an unbounded (natural-height)
            // page parent sizes the whole table to header + cap + padding
            // instead of the rows' full natural height; `min_h_0` lets that
            // fixed height shrink as a flex item with a bounded parent, handing
            // the virtual body its real viewport. (`Infer` + `max_h` caps the
            // list's own bounds but reports the full natural height upward, so
            // the outer table stretches white past the shown rows.)
            let height = self.max_h.unwrap_or(px(400.));
            let rows = ctx.clone();
            let projection = virtual_projection;
            // The headless probe name for the virtual viewport's bounds.
            let rows_selector = format!("{table_id}-virtual-rows");
            let handle = virtual_scroll_now;
            if handle.item_count() != row_count {
                handle.splice(0..handle.item_count(), row_count);
            }
            body = body.child(
                crate::VirtualList::new(
                    element_id::scoped(base_id, "virtual-rows"),
                    handle,
                    move |i, _window, cx| {
                        let (source_index, key, metadata) = &projection[i];
                        let row_data = factory(*source_index);
                        rows.row(
                            i,
                            row_data,
                            metadata.depth,
                            metadata.has_children,
                            key,
                            Some(row_height),
                            _window,
                            cx,
                        )
                    },
                )
                .height(height)
                .debug_selector(rows_selector)
                .when(
                    ctx.frozen.is_some(),
                    crate::VirtualList::restrict_scroll_to_axis,
                ),
            );
        } else if let (Some(state), Some((_, _, _, factory))) =
            (virtual_list_state, self.virtual_rows.clone())
        {
            let height = self.max_h.unwrap_or(px(400.));
            let rows = ctx.clone();
            let projection = virtual_projection;
            let measured_heights =
                virtual_row_heights.expect("estimated row height creates a measurement store");
            body = body.child(
                crate::VirtualList::new(
                    element_id::scoped(base_id, "variable-rows"),
                    &crate::VirtualListHandle::from_list_state(state),
                    move |i, _window, cx| {
                        let (source_index, key, metadata) = &projection[i];
                        let row_data = factory(*source_index);
                        let row = rows.row(
                            i,
                            row_data,
                            metadata.depth,
                            metadata.has_children,
                            key,
                            None,
                            _window,
                            cx,
                        );
                        let measured = measured_heights.clone();
                        gpui::div()
                            .relative()
                            .w_full()
                            .child(row)
                            .child(
                                gpui::canvas(
                                    move |bounds: gpui::Bounds<Pixels>, _, cx| {
                                        measured.update(cx, |(_, heights), cx| {
                                            if heights.get(i).copied().flatten()
                                                != Some(bounds.size.height)
                                            {
                                                heights[i] = Some(bounds.size.height);
                                                cx.notify();
                                            }
                                        });
                                        bounds
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .inset_0(),
                            )
                            .into_any_element()
                    },
                )
                .height(height),
            );
        } else {
            for (i, (row_data, depth, has_children, tree_key, _)) in flat.into_iter().enumerate() {
                body = body.child(ctx.row(
                    i,
                    row_data,
                    depth,
                    has_children,
                    &tree_key,
                    None,
                    window,
                    cx,
                ));
            }
        }

        body
    }

    /// The load-more sentinel: a 1px canvas that reports `onLoadMore` when
    /// the end of the collection nears the viewport, and the pending row
    /// (`.table__load-more-content`) while `is_pending`.
    fn load_more(
        &self,
        mut table: gpui::Stateful<gpui::Div>,
        load_more: LoadMore<'_>,
    ) -> gpui::Stateful<gpui::Div> {
        let LoadMore {
            base_id,
            state,
            collection: load_more_collection,
            virtual_scroll,
            variable_scroll,
            muted,
            loading_text,
        } = load_more;
        if let Some(cb) = self.on_load_more.clone() {
            let scroll_offset = self.load_more_offset;
            table = table.child(
                gpui::canvas(
                    move |bounds, window, cx| {
                        // Plain rows place this canvas at the real content
                        // end, so ancestor masks provide its viewport
                        // intersection. Virtual rows keep that end inside
                        // their own scroll state instead.
                        let mask = window.content_mask().bounds;
                        let in_view = bounds.right() >= mask.left()
                            && bounds.left() <= mask.right()
                            && bounds.bottom() >= mask.top()
                            && bounds.top() <= mask.bottom() + mask.size.height * scroll_offset;
                        let virtual_end_is_near = if let Some(handle) = &virtual_scroll {
                            // The uniform body knows its tail exactly: the
                            // row count times the measured row, less the
                            // scroll offset and the viewport.
                            let viewport_height = handle.viewport_bounds().size.height;
                            handle.remaining_below().is_some_and(|remaining| {
                                remaining <= viewport_height * scroll_offset
                            })
                        } else if let Some((state, count, estimate)) = &variable_scroll {
                            let viewport = state.viewport_bounds();
                            let viewport_height = viewport.size.height;
                            let top = state.logical_scroll_top();
                            // Unmeasured ListState rows have zero height,
                            // so project the unseen tail from the caller's
                            // layout estimate instead.
                            let remaining = *estimate * count.saturating_sub(top.item_ix)
                                - top.offset_in_item
                                - viewport_height;
                            let margin = viewport_height * scroll_offset;
                            let measured_end_is_near = *count == 0
                                || state.bounds_for_item(count.saturating_sub(1)).is_some_and(
                                    |bounds| bounds.bottom() <= viewport.bottom() + margin,
                                );
                            measured_end_is_near || (scroll_offset > 0. && remaining <= margin)
                        } else {
                            true
                        };
                        let visible = if virtual_scroll.is_some() || variable_scroll.is_some() {
                            virtual_end_is_near
                        } else {
                            in_view
                        };
                        let previous = state.read(cx).clone();
                        let collection_changed = previous
                            .1
                            .as_ref()
                            .is_none_or(|identity| identity != &load_more_collection);
                        let should_load = visible && (!previous.0 || collection_changed);
                        if previous.0 != visible || should_load {
                            state.update(cx, |value, cx| {
                                value.0 = visible;
                                if should_load {
                                    value.1 = Some(load_more_collection.clone());
                                }
                                cx.notify();
                            });
                        }
                        if should_load {
                            cb(window, cx);
                        }
                    },
                    |_, _, _, _| {},
                )
                .size(px(1.)),
            );
        }

        if self.is_pending {
            let sentinel = gpui::div()
            .id(element_id::scoped(base_id, "load-more"))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .w_full()
            // `.table__load-more-content` is `gap-2 py-2`; `loaderHeight`
            // fixes the row's height instead when the caller sets it.
            .py(px(8.))
            .when_some(self.loader_height, |el, h| el.h(h))
            .text_size(px(13.))
            .text_color(muted)
                .child(
                    crate::spinner::Spinner::new(element_id::scoped(base_id, "load-spinner"))
                        .size(herogpui_core::Size::Sm),
                )
                .child(loading_text);
            table = table.child(sentinel);
        }
        table
    }

    /// Which of the row run's edges coincide with a curved edge of the box
    /// that clips it. A virtual body only meets its box at an edge it is
    /// actually scrolled to; an unresolved scroll state answers "no", which
    /// keeps a square fill rather than a wrongly rounded one.
    fn row_edge_rounding(
        &self,
        secondary: bool,
        virtual_scroll_now: &crate::VirtualListHandle,
        virtual_list_state: Option<&gpui::ListState>,
        virtual_visible_count: usize,
        cx: &App,
    ) -> (Option<Pixels>, Option<Pixels>) {
        let (touches_top, touches_bottom) =
            if self.row_height.is_some() && self.virtual_rows.is_some() {
                // A list that does not scroll at all answers "at the end":
                // every row is on screen.
                (
                    virtual_scroll_now.is_scrolled_to_top(),
                    virtual_scroll_now.is_scrolled_to_end(),
                )
            } else if let Some(state) = virtual_list_state {
                let top = state.logical_scroll_top();
                let viewport = state.viewport_bounds();
                (
                    top.item_ix == 0 && top.offset_in_item <= px(0.5),
                    virtual_visible_count == 0
                        || state
                            .bounds_for_item(virtual_visible_count - 1)
                            .is_some_and(|bounds| bounds.bottom() <= viewport.bottom() + px(0.5)),
                )
            } else {
                // Plain rows have no scroller of their own, so the run's
                // edges are the box's edges.
                (true, true)
            };
        edge_corner_rounding(
            secondary,
            self.footer.is_some(),
            touches_top,
            touches_bottom,
            cx.layout().radius_2xl().min(px(32.)),
            self.radius
                .unwrap_or_else(|| crate::util::container_radius(cx)),
        )
    }
}

/// Everything one body row needs, so both the plain and the virtualized path can
/// draw it from the same code.
/// Which corners of a body row's full-bleed fill must be rounded so the fill
/// does not square the curved edge it sits against.
///
/// Vanilla GPUI clips an `overflow_hidden()` child to its parent's rectangle
/// rather than to the parent's rounded corners, so the fill has to carry the
/// curve itself. Which box owns that curve differs by variant: the primary
/// variant's rows sit in the rounded `.table__body` block, while the
/// secondary variant is flat and the wrapper's own corners are the only
/// curve — and there the header sits above the rows, so the top corners are
/// never the rows', and a footer takes the bottom ones when one is present.
fn edge_corner_rounding(
    secondary: bool,
    has_footer: bool,
    touches_top: bool,
    touches_bottom: bool,
    body_radius: Pixels,
    wrapper_radius: Pixels,
) -> (Option<Pixels>, Option<Pixels>) {
    if secondary {
        (
            None,
            (touches_bottom && !has_footer).then_some(wrapper_radius),
        )
    } else {
        (
            touches_top.then_some(body_radius),
            touches_bottom.then_some(body_radius),
        )
    }
}

struct RowCtx {
    /// The table's id, so one table's row ids cannot collide with another's.
    /// Every row part is scoped off it.
    id: gpui::ElementId,
    /// Intrinsic content widths for non-resizable columns. The table uses
    /// these as shared minimum tracks so a long body cell cannot make its row
    /// disagree with the header or its siblings.
    measured_widths: gpui::Entity<Vec<Option<Pixels>>>,
    measure_intrinsic: Vec<bool>,
    /// `(defaultWidth or the resize, minWidth, maxWidth)` per column.
    widths: Vec<(Option<Pixels>, Option<Pixels>, Option<Pixels>)>,
    row_header_columns: Vec<bool>,
    tree_column: usize,
    tree_column_has_children: bool,
    selectable: bool,
    selection_mode: SelectionMode,
    selection_behavior: SelectionBehavior,
    selected_keys: Vec<SharedString>,
    row_keys: Vec<SharedString>,
    disabled_keys: Vec<SharedString>,
    disallow_empty_selection: bool,
    expanded: std::rc::Rc<Vec<SharedString>>,
    on_expanded_change: Option<OnExpandedChange>,
    on_selection_change: Option<OnSelectionChange>,
    selection_own: Option<gpui::Entity<Vec<SharedString>>>,
    selection_range: gpui::Entity<TableSelectionRange>,
    on_row_click: Option<OnRowClick>,
    focus: gpui::FocusHandle,
    cursor_own: gpui::Entity<Option<SharedString>>,
    /// The row the keyboard is on, which wears `status-focused`.
    cursor: Option<usize>,
    /// The `.table-root--secondary` flat layout, whose row hover is a
    /// different token from the primary's.
    secondary: bool,
    /// The caller's unselected-row hover fill, when one was named. Selected
    /// rows keep their selection fill regardless.
    row_hover_bg: Option<gpui::Hsla>,
    /// Prefix for per-row hover groups. HeroUI paints hover and selected
    /// surfaces on each cell, allowing the cell's radius/clip to own the
    /// resulting geometry instead of leaking a row-level fill through gaps.
    hover_group_prefix: SharedString,
    /// Whether the collection is windowed, which is the guard upstream puts
    /// `aria-rowindex` behind (`.../grid/useGridRow.mjs`).
    virtualized: bool,
    /// The radius the first row's outer cells must round their fill to, when
    /// the top of the row run really is the curved edge of its box. `None`
    /// leaves the fill square, which is what a scrolled virtual body needs.
    round_top: Option<Pixels>,
    /// The same for the last row's outer cells and the bottom edge.
    round_bottom: Option<Pixels>,
    /// Whether the collection is a tree, which is what makes
    /// `useTableRow.mjs` add `aria-level` and `aria-expanded` to a row.
    is_tree: bool,
    /// Cell selection (HeroGPUI extension), when enabled.
    cell: Option<CellCtx>,
    /// Frozen leading columns (HeroGPUI extension), when any scroll.
    frozen: Option<FrozenColumns>,
}

/// One body row's per-render facts, which every part of the row reads.
struct RowPaint {
    i: usize,
    key: SharedString,
    depth: usize,
    has_children: bool,
    is_selected: bool,
    is_disabled: bool,
    is_expanded: bool,
    selected_bg: Option<gpui::Hsla>,
    hover_bg: Option<gpui::Hsla>,
    hover_group: SharedString,
    /// The row the keyboard is on.
    row_focused: bool,
    data_count: usize,
    round_top: Option<Pixels>,
    round_bottom: Option<Pixels>,
}

impl RowCtx {
    /// One body row.
    ///
    /// `fixed_h` is `Some` only on the virtual path, where every row is one
    /// `rowHeight` tall because that is the number the scroll geometry comes
    /// from.
    #[allow(clippy::too_many_arguments)]
    /// One `.table__row`: `relative h-full` with a `border-separator/50`
    /// bottom edge except on the final visible row, and `.table__cell`s inside
    /// it.
    fn row(
        &self,
        i: usize,
        row_data: TableRow,
        depth: usize,
        has_children: bool,
        tree_key: &SharedString,
        fixed_h: Option<Pixels>,
        window: &Window,
        cx: &mut App,
    ) -> AnyElement {
        let colors = cx.colors();
        let key = tree_key.clone();
        let is_selected = self.selected_keys.contains(&key);
        let is_disabled = self.disabled_keys.contains(&key);
        let selected_bg = is_selected.then(|| colors.surface.background.alpha(0.1));
        let hover_bg = (!is_selected && !is_disabled).then(|| {
            self.row_hover_bg.unwrap_or(if self.secondary {
                colors.default.color.alpha(0.5)
            } else {
                colors.surface.background.alpha(0.4)
            })
        });
        let hover_group: SharedString = format!("{}-row-hover-{i}", self.hover_group_prefix).into();
        let row_focused = self.cursor == Some(i);
        let row_anchor = self.cursor_own.read(cx).as_ref() == Some(tree_key);

        let mut row = gpui::div()
            .id(element_id::indexed(&self.id, "row", i))
            // `useTableRow.mjs` delegates to `.../grid/useGridRow.mjs`:
            // `role: 'row'` with `'aria-selected': selectionMode !== 'none' ?
            // isSelected : undefined`, and `aria-rowindex` only
            // `if (isVirtualized)`. On a tree table it then adds
            // `'aria-expanded': hasChildRows ? ... : undefined` and
            // `'aria-level': treeNode.level + 1`.
            //
            // The `aria-posinset`/`aria-setsize` half of that tree block is
            // *not* stated, and not because gpui lacks the builders: upstream
            // counts a row's siblings out of the tree collection, and what
            // reaches this function is the flattened list of *visible* rows
            // plus this row's own depth. The sibling count at that depth is
            // not derivable from it, and a guess is worse than silence.
            .a11y(a11y::Role::Row)
            .when(self.selection_mode != SelectionMode::None, |row| {
                row.a11y_selected(is_selected)
            })
            .when(self.virtualized, |row| row.a11y_row_index(i))
            .when(self.is_tree, |row| row.a11y_level(depth))
            .when(self.is_tree && has_children, |row| {
                row.a11y_expanded(self.expanded.iter().any(|k| k == tree_key))
            })
            .flex()
            .group(hover_group.clone())
            // A virtual row is laid out on its own, so it takes the width it is
            // *given*: without `w_full` the columns bunch at the left edge. That
            // is true of both virtual paths, so it is not conditional on the
            // fixed height any more -- `gpui::list`'s rows have none.
            .w_full()
            .when_some(fixed_h, |e, h| e.h(h));
        if !self.secondary && i + 1 < self.row_keys.len() {
            row = row.border_b_1().border_color(colors.separator.alpha(0.5));
        }

        let data_count = row_data.cells.len();
        // The row's fill is painted per cell, so the outer cells of the first
        // and last rows carry the box's curve on the edge they touch.
        let round_top = (i == 0).then_some(self.round_top).flatten();
        let round_bottom = (i + 1 == self.row_keys.len())
            .then_some(self.round_bottom)
            .flatten();
        let paint = RowPaint {
            i,
            key,
            depth,
            has_children,
            is_selected,
            is_disabled,
            is_expanded: self.expanded.iter().any(|k| k == tree_key),
            selected_bg,
            hover_bg,
            hover_group,
            row_focused,
            data_count,
            round_top,
            round_bottom,
        };

        if let Some(frozen) = &self.frozen {
            row = self.frozen_row_parts(row, frozen, &paint, row_data, cx);
        } else {
            if self.selectable {
                row = row.child(self.select_cell(&paint, cx));
            }

            // Cells are flex rows so inline children (chips, buttons) size to
            // their content instead of stretching to the column width.
            let cx: &App = cx;
            row = row.children(
                row_data
                    .cells
                    .into_iter()
                    .enumerate()
                    .map(|(c, cell)| self.data_cell(&paint, c, cell, cx)),
            );
        }
        row = self.row_press(row, &paint, cx);

        // v3 rings the focused row *inside* itself: each cell carries its own
        // inset strips so the outline stays clipped to the cell geometry.
        row = row.when(is_disabled, |row| row.opacity(cx.layout().disabled_opacity));
        if row_anchor && self.focus.is_focused(window) {
            row = crate::util::record_focus_bounds(row, &self.focus, window, cx);
        }
        row.into_any_element()
    }

    /// A frozen table's row: the selection cell and the frozen cells in a
    /// part that stays put, the rest in a part that scrolls with the header.
    /// Both stay children of the row, which keeps the row's hover, press and
    /// cursor spanning the two.
    fn frozen_row_parts(
        &self,
        row: gpui::Stateful<gpui::Div>,
        frozen: &FrozenColumns,
        paint: &RowPaint,
        row_data: TableRow,
        cx: &App,
    ) -> gpui::Stateful<gpui::Div> {
        let data_count = row_data.cells.len();
        let mut lead = frozen.frozen_part();
        if self.selectable {
            lead = lead.child(self.select_cell(paint, cx));
        }
        let mut cells = row_data.cells.into_iter().enumerate();
        let lead = lead.children(
            cells
                .by_ref()
                .take(frozen.count)
                .map(|(c, cell)| self.data_cell(paint, c, cell, cx)),
        );
        let lead =
            lead.children((data_count..frozen.count).map(|c| self.empty_frozen_track(paint.i, c)));
        let rest = frozen
            .scrolling_part(
                gpui::div(),
                element_id::indexed(&self.id, "row-scroll", paint.i),
            )
            .children(cells.map(|(c, cell)| self.data_cell(paint, c, cell, cx)))
            .children(
                (data_count.max(frozen.count)..self.widths.len())
                    .map(|c| self.empty_frozen_track(paint.i, c)),
            );
        row.child(lead).child(rest)
    }

    /// Keep every row's shared horizontal scroll extent aligned with the
    /// header, even when a caller supplies fewer cells than columns. The
    /// empty track has no accessibility node or interaction target.
    fn empty_frozen_track(&self, row: usize, column: usize) -> gpui::Stateful<gpui::Div> {
        let (width, min_width, max_width) = self.widths[column];
        gpui::div()
            .when(width.is_none(), flex_cell)
            .when_some(width, |cell, width| cell.w(width))
            .when_some(min_width, |cell, width| cell.min_w(width))
            .when_some(max_width, |cell, width| cell.max_w(width))
            .id(element_id::indexed(
                &element_id::indexed(&self.id, "row", row),
                "empty-cell",
                column,
            ))
    }

    /// The selection column's body cell and its checkbox.
    fn select_cell(&self, paint: &RowPaint, cx: &App) -> gpui::Stateful<gpui::Div> {
        let RowPaint {
            i,
            is_selected,
            is_disabled,
            selected_bg,
            hover_bg,
            row_focused,
            data_count,
            round_top,
            round_bottom,
            ..
        } = *paint;
        let hover_group = &paint.hover_group;
        let mut cell = gpui::div()
            .id(element_id::indexed(&self.id, "select-cell", i))
            // The selection column's body cell. `useTableCell.mjs` gives
            // every data cell `.../grid/useGridCell.mjs`'s
            // `role: 'gridcell'` and `'aria-colindex': colIndex + 1`.
            .a11y(a11y::Role::GridCell)
            .a11y_column_index(0)
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .w(px(44.))
            .py(px(10.))
            .when_some(selected_bg, |cell, bg| cell.bg(bg))
            .when_some(hover_bg, |cell, bg| {
                cell.group_hover(hover_group.clone(), move |s| s.bg(bg))
            })
            .when_some(round_top, |cell, r| cell.rounded_tl(r))
            .when_some(round_bottom, |cell, r| cell.rounded_bl(r))
            .when(data_count == 0, |cell| {
                cell.when_some(round_top, |cell, r| cell.rounded_tr(r))
                    .when_some(round_bottom, |cell, r| cell.rounded_br(r))
            })
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            });
        let mut box_el = Checkbox::new(element_id::indexed(&self.id, "select", i))
            .is_selected(is_selected)
            .is_disabled(is_disabled)
            .aria_label(crate::i18n::ui_string(crate::i18n::UiString::Select, cx));
        let cb = self.on_selection_change.clone();
        if !is_disabled && (cb.is_some() || self.selection_own.is_some()) {
            let current = self.selected_keys.clone();
            let key2 = paint.key.clone();
            let mode = self.selection_mode;
            let disallow_empty_selection = self.disallow_empty_selection;
            let selection_own = self.selection_own.clone();
            let selection_range = self.selection_range.clone();
            box_el = box_el.on_change(move |_next, window, cx| {
                cx.stop_propagation();
                // A checkbox is the explicit selection affordance. React
                // Aria toggles it even when the surrounding row uses
                // `selectionBehavior="replace"`.
                let next = crate::selection::next_selection(
                    &current,
                    &key2,
                    mode,
                    disallow_empty_selection,
                );
                if let Some(held) = &selection_own {
                    held.update(cx, |value, cx| {
                        *value = next.clone();
                        cx.notify();
                    });
                }
                if let Some(cb) = &cb {
                    cb(&next, window, cx);
                }
                if mode == SelectionMode::Multiple && !is_selected {
                    selection_range.update(cx, |range, _| {
                        range.anchor = Some(key2.clone());
                        range.current = Some(key2.clone());
                        range.is_all = false;
                    });
                } else if mode == SelectionMode::Multiple {
                    selection_range.update(cx, |range, _| {
                        if range.is_all {
                            *range = TableSelectionRange::default();
                        }
                    });
                }
            });
        }
        cell = cell.child(box_el);
        if row_focused {
            cell = cell.child(table_cell_focus_ring(cx, true, data_count == 0));
        }
        cell
    }

    /// One `.table__cell`: the column's shared track, the row's fills, the
    /// tree column's indent and chevron, and the cell-selection decoration.
    fn data_cell(
        &self,
        paint: &RowPaint,
        c: usize,
        cell: AnyElement,
        cx: &App,
    ) -> gpui::Stateful<gpui::Div> {
        let colors = cx.colors();
        let RowPaint {
            i,
            depth,
            has_children,
            is_disabled,
            selected_bg,
            hover_bg,
            row_focused,
            data_count,
            round_top,
            round_bottom,
            ..
        } = *paint;
        let hover_group = &paint.hover_group;
        let key = &paint.key;
        let tree_column = self.tree_column;
        let row_header_columns = &self.row_header_columns;
        let (width, min_width, max_width) =
            self.widths.get(c).copied().unwrap_or((None, None, None));
        let mut cell_el = gpui::div()
            .when(width.is_none(), flex_cell)
            .when_some(width, |e, w| e.w(w))
            .when_some(min_width, |e, w| e.min_w(w))
            .when_some(max_width, |e, w| e.max_w(w))
            .flex()
            .items_center()
            .gap(px(6.))
            .relative()
            // `.table__cell` is `px-4 py-3`.
            .px(px(16.))
            .py(px(12.))
            .when(self.secondary, |e| {
                e.border_b_1()
                    .border_color(colors.separator_tertiary().alpha(0.5))
            })
            .when(row_header_columns.get(c).copied().unwrap_or(false), |e| {
                e.font_weight(gpui::FontWeight::MEDIUM)
            })
            .when_some(selected_bg, |cell, bg| cell.bg(bg))
            .when_some(hover_bg, |cell, bg| {
                cell.group_hover(hover_group.clone(), move |s| s.bg(bg))
            })
            .when(!self.selectable && c == 0, |cell| {
                cell.when_some(round_top, |cell, r| cell.rounded_tl(r))
                    .when_some(round_bottom, |cell, r| cell.rounded_bl(r))
            })
            .when(c + 1 == data_count, |cell| {
                cell.when_some(round_top, |cell, r| cell.rounded_tr(r))
                    .when_some(round_bottom, |cell, r| cell.rounded_br(r))
            });
        if self.measure_intrinsic.get(c).copied().unwrap_or(false) {
            let measurements = self.measured_widths.clone();
            let tree_indent = if c == tree_column {
                tree_column_padding(depth).map_or(0., f32::from)
            } else {
                0.
            };
            // Only the cell's own content: the tree column's chevron or
            // spacer, then the caller's element. The rings a focused row or a
            // selected cell add after them are `inset_0` overlays as wide as
            // the cell, and measuring them grew the floor every frame.
            let content_children = 1 + usize::from(
                c == tree_column && (has_children || depth > 0 || self.tree_column_has_children),
            );
            cell_el = cell_el.on_children_prepainted(move |bounds, _, cx| {
                let content =
                    intrinsic_children_width(&bounds[..content_children.min(bounds.len())]);
                if content <= 0. {
                    return;
                }
                let next = px(content + 32. + tree_indent);
                measurements.update(cx, |values, cx| {
                    if values.len() <= c {
                        values.resize(c + 1, None);
                    }
                    if values[c].is_none_or(|current| next > current) {
                        values[c] = Some(next);
                        cx.notify();
                    }
                });
            });
        }
        // At the end of the chain, and after `flex_cell` — that helper
        // takes a `gpui::Div`, which an `.id(..)` would have turned into a
        // `Stateful<Div>` — and clear of the `.table__cell` padding window
        // `.shots/design_audit.py` opens on the comment above.
        //
        // `useTableCell.mjs` is `.../grid/useGridCell.mjs`'s
        // `role: 'gridcell'`, swapped for `'rowheader'` when the cell's
        // column key is in `rowHeaderColumnKeys` — the port's
        // `TableColumn::is_row_header`. The cell's contents are an
        // arbitrary `AnyElement` here, so unlike a row (which upstream
        // names from `node.textValue`) it carries no name of its own;
        // whatever the caller composed inside reports itself.
        let mut cell_el = cell_el
            .debug_selector(move || format!("table-row-track-{i}-{c}"))
            .id(element_id::indexed(
                &element_id::indexed(&self.id, "row", i),
                "cell",
                c,
            ))
            .a11y(if row_header_columns.get(c).copied().unwrap_or(false) {
                a11y::Role::RowHeader
            } else {
                a11y::Role::GridCell
            })
            .a11y_column_index(c + usize::from(self.selectable));
        // The tree column carries the indent and the chevron; a row with
        // no children still indents, so siblings line up.
        if c == tree_column {
            if let Some(padding) = tree_column_padding(depth) {
                cell_el = cell_el.pl(padding);
            }
            if has_children {
                cell_el = cell_el.child(self.tree_chevron(paint, cx));
            } else if depth > 0 || self.tree_column_has_children {
                // A leaf keeps the chevron's width so its text lines up
                // with its expandable siblings'.
                cell_el = cell_el.child(gpui::div().size(px(18.)).flex_shrink_0());
            }
        }
        cell_el = cell_el.child(cell);
        if let Some(cell_ctx) = &self.cell {
            cell_el = cell_ctx.decorate(cell_el, key, c, is_disabled, cx);
        }
        if row_focused && self.cell.is_none() {
            let cell_index = c + usize::from(self.selectable);
            let total_cells = data_count + usize::from(self.selectable);
            cell_el = cell_el.child(table_cell_focus_ring(
                cx,
                cell_index == 0,
                cell_index + 1 == total_cells,
            ));
        }
        cell_el
    }

    /// The tree column's expand/collapse chevron on a row with children.
    fn tree_chevron(&self, paint: &RowPaint, cx: &App) -> gpui::Stateful<gpui::Div> {
        let colors = cx.colors();
        let is_disabled = paint.is_disabled;
        let is_expanded = paint.is_expanded;
        let toggle_key = paint.key.clone();
        let mut chevron = gpui::div()
            .id(element_id::scoped(
                &element_id::scoped(&self.id, "expand"),
                toggle_key.clone(),
            ))
            .flex()
            .items_center()
            .justify_center()
            .size(px(18.))
            .flex_shrink_0()
            .when(!is_disabled, |chevron| {
                chevron
                    .cursor(crate::util::interactive_cursor(cx))
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
            })
            .child(
                gpui::svg()
                    .size(px(12.))
                    .path(if is_expanded {
                        icons::CHEVRON_DOWN
                    } else {
                        icons::CHEVRON_RIGHT
                    })
                    .text_color(colors.muted),
            );
        if !is_disabled {
            if let Some(cb) = self.on_expanded_change.clone() {
                let key = toggle_key;
                let before = self.expanded.clone();
                let focus = self.focus.clone();
                let cursor = self.cursor_own.clone();
                chevron = chevron.on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    let mut next = before.as_ref().clone();
                    if let Some(at) = next.iter().position(|k| *k == key) {
                        next.remove(at);
                    } else {
                        next.push(key.clone());
                    }
                    cb(&next, window, cx);
                    window.focus(&focus, cx);
                    cursor.update(cx, |value, cx| {
                        *value = Some(key.clone());
                        cx.notify();
                    });
                });
            }
        }
        chevron
    }

    /// The row's press: focus and cursor on mouse down, then the row's
    /// action or a selection change on click.
    fn row_press(
        &self,
        row: gpui::Stateful<gpui::Div>,
        paint: &RowPaint,
        cx: &App,
    ) -> gpui::Stateful<gpui::Div> {
        let i = paint.i;
        let key = paint.key.clone();
        let row_action = self.on_row_click.clone();
        let row_selection = self.on_selection_change.clone();
        let selection_own = self.selection_own.clone();
        let selection_behavior = self.selection_behavior;
        let disallow_empty_selection = self.disallow_empty_selection;
        if paint.is_disabled
            || !(row_action.is_some()
                || (self.selectable && (row_selection.is_some() || selection_own.is_some())))
        {
            return row;
        }
        let current = self.selected_keys.clone();
        let mode = self.selection_mode;
        let selection_range = self.selection_range.clone();
        let range_collection = self.row_keys.clone();
        let range_selectable: Vec<SharedString> = self
            .row_keys
            .iter()
            .filter(|key| !self.disabled_keys.contains(key))
            .cloned()
            .collect();
        let moved = self.cursor_own.clone();
        let focus = self.focus.clone();
        let focus_for_click = self.focus.clone();
        let key_for_cursor = key.clone();
        row.cursor(crate::util::interactive_cursor(cx))
            .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                if window.default_prevented() {
                    return;
                }
                window.focus(&focus, cx);
                moved.update(cx, |value, cx| {
                    *value = Some(key_for_cursor.clone());
                    cx.notify();
                });
            })
            .on_click(move |ev, w, cx| {
                if !focus_for_click.is_focused(w) {
                    return;
                }
                match row_intent(
                    RowActivation::Pointer,
                    mode,
                    selection_behavior,
                    current.is_empty(),
                    row_action.is_some(),
                ) {
                    RowIntent::Action => {
                        if let Some(cb) = &row_action {
                            cb(&i, ev, w, cx);
                        }
                    }
                    RowIntent::Selection => {
                        let was_selected = current.contains(&key);
                        let extends_selection =
                            ev.modifiers().shift && mode == SelectionMode::Multiple;
                        let next = if extends_selection {
                            let range = selection_range.read(cx).clone();
                            extend_selection_range(
                                &current,
                                &range_collection,
                                &range_selectable,
                                &range,
                                &key,
                            )
                        } else {
                            let modifiers = ev.modifiers();
                            let non_contiguous = if cx.platform_info().is_mac() {
                                modifiers.alt
                            } else {
                                modifiers.control
                            };
                            crate::selection::next_selection_with_behavior(
                                &current,
                                &key,
                                mode,
                                if non_contiguous {
                                    SelectionBehavior::Toggle
                                } else {
                                    selection_behavior
                                },
                                disallow_empty_selection,
                            )
                        };
                        let changed = !same_selection(&next, &current);
                        if changed {
                            if let Some(held) = &selection_own {
                                held.update(cx, |value, cx| {
                                    *value = next.clone();
                                    cx.notify();
                                });
                            }
                            if let Some(cb) = &row_selection {
                                cb(&next, w, cx);
                            }
                        }
                        if extends_selection && changed {
                            selection_range.update(cx, |range, _| {
                                if range.anchor.is_none() {
                                    range.anchor = Some(key.clone());
                                }
                                range.current = Some(key.clone());
                                range.is_all = false;
                            });
                        } else if !extends_selection
                            && mode == SelectionMode::Multiple
                            && !was_selected
                        {
                            selection_range.update(cx, |range, _| {
                                range.anchor = Some(key.clone());
                                range.current = Some(key.clone());
                                range.is_all = false;
                            });
                        } else if !extends_selection && mode == SelectionMode::Multiple {
                            selection_range.update(cx, |range, _| {
                                if range.is_all {
                                    *range = TableSelectionRange::default();
                                }
                            });
                        }
                    }
                    RowIntent::None => {}
                }
            })
    }
}

// ---------------------------------------------------------------------------
// Column reordering and cell selection (HeroGPUI extension, after gpui-kit's
// data table: `movable` columns and `cell_selectable`). Neither is a HeroUI
// v3 prop; they are kept here, after the v3 surface, so the ported code above
// stays as it was.
// ---------------------------------------------------------------------------

type OnColumnMove = std::sync::Arc<dyn Fn(&ColumnMove, &mut Window, &mut App) + 'static>;
type OnCellSelect = std::sync::Arc<dyn Fn(&TableCell, &mut Window, &mut App) + 'static>;

/// A column move, as [`Table::on_column_move`] reports it (HeroGPUI
/// extension). Positions are display positions *before* the move; `order` is
/// the new display order as indices into the columns the table was given.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ColumnMove {
    /// The display position the column left.
    pub from: usize,
    /// The display position it moved to.
    pub to: usize,
    /// The whole new order, as indices into the given columns.
    pub order: Vec<usize>,
}

/// One table cell (HeroGPUI extension): the row's selection key and the
/// column's index in the columns the table was given, so it stays the same
/// cell when the columns are reordered.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TableCell {
    /// The row's selection key (see [`TableRow::key`]).
    pub row: SharedString,
    /// The column's index in the columns as given.
    pub column: usize,
}

impl TableCell {
    /// The cell of row `row` in column `column` (an index into the columns
    /// as given).
    pub fn new(row: impl Into<SharedString>, column: usize) -> Self {
        Self {
            row: row.into(),
            column,
        }
    }
}

/// The column-reordering configuration.
#[derive(Default)]
struct ColumnReorder {
    allowed: bool,
    order: Option<Vec<usize>>,
    default_order: Option<Vec<usize>>,
    on_move: Option<OnColumnMove>,
}

/// The cell-selection configuration.
#[derive(Default)]
struct CellSelection {
    enabled: bool,
    /// `Some` when controlled; the inner `None` is "no cell".
    selected: Option<Option<TableCell>>,
    default: Option<TableCell>,
    on_select: Option<OnCellSelect>,
}

/// `order` made a permutation of `0..n`: out-of-range and repeated indices
/// dropped, missing ones appended in order.
fn sanitize_order(order: &[usize], n: usize) -> Vec<usize> {
    let mut seen = vec![false; n];
    let mut out = Vec::with_capacity(n);
    for &ix in order {
        if ix < n && !seen[ix] {
            seen[ix] = true;
            out.push(ix);
        }
    }
    out.extend((0..n).filter(|ix| !seen[*ix]));
    out
}

/// The order after moving the column at display position `from` to `to`.
fn moved_order(order: &[usize], from: usize, to: usize) -> Vec<usize> {
    let mut next = order.to_vec();
    if from < next.len() && to < next.len() && from != to {
        let column = next.remove(from);
        next.insert(to, column);
    }
    next
}

/// Reorders a row's cells (and its children's) into display order. A row
/// with fewer cells than columns keeps the ones it has, in display order.
fn permute_row(row: &mut TableRow, order: &[usize]) {
    let mut cells: Vec<Option<AnyElement>> = std::mem::take(&mut row.cells)
        .into_iter()
        .map(Some)
        .collect();
    row.cells = order
        .iter()
        .filter_map(|&ix| cells.get_mut(ix).and_then(Option::take))
        .collect();
    for child in &mut row.children {
        permute_row(child, order);
    }
}

/// Moves per-position values (resized and measured widths) from the order
/// they were stored in to the new one.
fn permute_positions<T: Clone>(
    values: &[Option<T>],
    from: &[usize],
    to: &[usize],
) -> Vec<Option<T>> {
    to.iter()
        .map(|column| {
            from.iter()
                .position(|c| c == column)
                .and_then(|at| values.get(at).cloned().flatten())
        })
        .collect()
}

/// The header row's left edge and each column header's bounds.
type HeaderBounds = std::rc::Rc<std::cell::RefCell<(Pixels, Vec<gpui::Bounds<Pixels>>)>>;

/// The drag of a column header in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ColumnDrag {
    from: usize,
    start_x: f32,
    active: bool,
    target: usize,
}

/// The display position whose header contains `x`, or the nearest end.
fn header_at(bounds: &[gpui::Bounds<Pixels>], x: Pixels) -> Option<usize> {
    let first = bounds.first()?;
    if x < first.left() {
        return Some(0);
    }
    bounds
        .iter()
        .position(|b| x >= b.left() && x < b.right())
        .or(Some(bounds.len() - 1))
}

impl Table {
    /// Lets the user reorder the columns (HeroGPUI extension): drag a header
    /// to another column's place, or focus a header and press Alt+Left /
    /// Alt+Right. Every move is reported through [`Table::on_column_move`].
    pub fn allows_column_reorder(mut self, allowed: bool) -> Self {
        self.column_reorder.allowed = allowed;
        self
    }

    /// The controlled display order, as indices into the columns given
    /// (HeroGPUI extension). Out-of-range and repeated indices are dropped and
    /// missing columns appended.
    pub fn column_order(mut self, order: impl IntoIterator<Item = usize>) -> Self {
        self.column_reorder.order = Some(order.into_iter().collect());
        self
    }

    /// The initial display order of an uncontrolled table (HeroGPUI
    /// extension).
    pub fn default_column_order(mut self, order: impl IntoIterator<Item = usize>) -> Self {
        self.column_reorder.default_order = Some(order.into_iter().collect());
        self
    }

    /// Reports each column move (HeroGPUI extension).
    pub fn on_column_move(
        mut self,
        f: impl Fn(&ColumnMove, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.column_reorder.on_move = Some(std::sync::Arc::new(f));
        self
    }

    /// Makes single cells selectable (HeroGPUI extension): a press selects the
    /// cell under the pointer, Left / Right move between the cells of the
    /// row, Home / End to its first and last cell, and the row keys (Up, Down,
    /// PageUp, PageDown, Ctrl+Home, Ctrl+End) move to the same column of
    /// another row. Row selection keeps working alongside; in a tree table the
    /// chevron, not Left/Right, expands a row.
    pub fn cell_selectable(mut self, selectable: bool) -> Self {
        self.cell_selection.enabled = selectable;
        self
    }

    /// The controlled selected cell (HeroGPUI extension); `None` for none.
    pub fn selected_cell(mut self, cell: Option<TableCell>) -> Self {
        self.cell_selection.selected = Some(cell);
        self
    }

    /// The initially selected cell of an uncontrolled table (HeroGPUI
    /// extension).
    pub fn default_selected_cell(mut self, cell: TableCell) -> Self {
        self.cell_selection.default = Some(cell);
        self
    }

    /// Reports each cell a press or a key selected (HeroGPUI extension).
    pub fn on_cell_select(
        mut self,
        f: impl Fn(&TableCell, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.cell_selection.on_select = Some(std::sync::Arc::new(f));
        self
    }

    /// Resolves the display column order and puts the columns, every row's
    /// cells and the tree column into it, so the rest of the render works in
    /// display positions. Returns the order (indices into the given columns).
    fn apply_column_order(
        &mut self,
        base_id: &gpui::ElementId,
        window: &mut Window,
        cx: &mut App,
    ) -> std::rc::Rc<Vec<usize>> {
        let n = self.columns.len();
        let reorder = &self.column_reorder;
        let configured =
            reorder.allowed || reorder.order.is_some() || reorder.default_order.is_some();
        if !configured && !self.columns.iter().any(|c| c.frozen) {
            return std::rc::Rc::new((0..n).collect());
        }
        let order = if configured {
            let seed = sanitize_order(reorder.default_order.as_deref().unwrap_or(&[]), n);
            let (order, _) = crate::util::controlled(
                window,
                cx,
                element_id::scoped(base_id, "column-order"),
                reorder.order.as_ref().map(|order| sanitize_order(order, n)),
                seed,
            );
            sanitize_order(&order, n)
        } else {
            (0..n).collect()
        };
        // Frozen columns (HeroGPUI extension) are always displayed first.
        let order = frozen_first(&order, &self.columns);
        // The resized and measured widths are stored per display position;
        // when the order changes they move with their columns.
        let rendered = window.use_keyed_state(
            element_id::scoped(base_id, "rendered-column-order"),
            cx,
            |_, _| (0..n).collect::<Vec<usize>>(),
        );
        let previous = rendered.read(cx).clone();
        if previous != order {
            for part in ["resized", "measured-widths"] {
                let widths =
                    window.use_keyed_state(element_id::scoped(base_id, part), cx, |_, _| {
                        Vec::<Option<Pixels>>::new()
                    });
                widths.update(cx, |values, _| {
                    *values = permute_positions(values, &previous, &order);
                });
            }
            rendered.update(cx, |value, _| value.clone_from(&order));
        }
        if order.iter().enumerate().all(|(at, ix)| at == *ix) {
            return std::rc::Rc::new(order);
        }
        let mut given: Vec<Option<TableColumn>> = std::mem::take(&mut self.columns)
            .into_iter()
            .map(Some)
            .collect();
        self.columns = order
            .iter()
            .filter_map(|&ix| given.get_mut(ix).and_then(Option::take))
            .collect();
        for row in &mut self.rows {
            permute_row(row, &order);
        }
        if let Some((_, _, _, row)) = &mut self.virtual_rows {
            let inner = row.clone();
            let order = order.clone();
            *row = crate::util::shared(move |ix| {
                let mut built = inner(ix);
                permute_row(&mut built, &order);
                built
            });
        }
        if let Some(at) = order.iter().position(|ix| *ix == self.tree_column) {
            self.tree_column = at;
        }
        std::rc::Rc::new(order)
    }

    /// The header row's left edge and each column header's bounds, stored by
    /// the header row's prepaint for the column drag.
    fn header_bounds(base_id: &gpui::ElementId, window: &mut Window, cx: &mut App) -> HeaderBounds {
        window
            .use_keyed_state(element_id::scoped(base_id, "header-bounds"), cx, |_, _| {
                std::rc::Rc::new(std::cell::RefCell::new((
                    px(0.),
                    Vec::<gpui::Bounds<Pixels>>::new(),
                )))
            })
            .read(cx)
            .clone()
    }

    /// Records the header cells' bounds (before the header row takes its id,
    /// which `on_children_prepainted` needs) when reordering is allowed.
    fn column_reorder_measure(
        &self,
        header: gpui::Div,
        base_id: &gpui::ElementId,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Div {
        if !self.column_reorder.allowed || self.columns.len() < 2 {
            return header;
        }
        let bounds = Self::header_bounds(base_id, window, cx);
        let skip = usize::from(self.selection_mode != SelectionMode::None);
        let columns = self.columns.len();
        header.on_children_prepainted(move |children, _, _| {
            let origin = children.first().map_or(px(0.), |b| b.left());
            let headers = children.iter().skip(skip).take(columns).copied().collect();
            *bounds.borrow_mut() = (origin, headers);
        })
    }

    /// A frozen header's two parts: the selection cell and the frozen
    /// column headers, then the scrolling ones. When reordering is allowed
    /// each part records its headers' bounds into the one list the column
    /// drag reads, the frozen part first (it prepaints first).
    fn frozen_header_parts(
        &self,
        base_id: &gpui::ElementId,
        frozen: &FrozenColumns,
        mut cells: Vec<AnyElement>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::Div, gpui::Stateful<gpui::Div>) {
        let skip = usize::from(self.selection_mode != SelectionMode::None);
        let rest = cells.split_off((skip + frozen.count).min(cells.len()));
        let mut lead = frozen.frozen_part();
        let mut part = gpui::div();
        if self.column_reorder.allowed && self.columns.len() >= 2 {
            let bounds = Self::header_bounds(base_id, window, cx);
            let scrolled = bounds.clone();
            let count = frozen.count;
            lead = lead.on_children_prepainted(move |children, _, _| {
                let origin = children.first().map_or(px(0.), |b| b.left());
                let mut measured = bounds.borrow_mut();
                measured.0 = origin;
                measured.1.clear();
                measured.1.extend(children.iter().skip(skip).copied());
            });
            part = part.on_children_prepainted(move |children, _, _| {
                let mut measured = scrolled.borrow_mut();
                measured.1.truncate(count);
                measured.1.extend(children.iter().copied());
            });
        }
        (
            lead.children(cells),
            frozen
                .scrolling_part(part, element_id::scoped(base_id, "header-scroll"))
                .children(rest),
        )
    }

    /// The header row's column drag (pointer) and Alt+Left / Alt+Right
    /// (keyboard) moves, when reordering is allowed.
    #[allow(clippy::too_many_arguments)]
    fn column_reorder_header(
        &self,
        header: gpui::Stateful<gpui::Div>,
        base_id: &gpui::ElementId,
        order: &std::rc::Rc<Vec<usize>>,
        header_focus: &[gpui::FocusHandle],
        resizing: &gpui::Entity<Option<(usize, f32, f32)>>,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        if !self.column_reorder.allowed || self.columns.len() < 2 {
            return header;
        }
        let bounds = Self::header_bounds(base_id, window, cx);
        let drag =
            window.use_keyed_state(element_id::scoped(base_id, "column-drag"), cx, |_, _| {
                None::<ColumnDrag>
            });
        let own_order = if self.column_reorder.order.is_none() {
            Some(
                window.use_keyed_state(element_id::scoped(base_id, "column-order"), cx, |_, _| {
                    order.as_ref().clone()
                }),
            )
        } else {
            None
        };
        let on_move = self.column_reorder.on_move.clone();
        // A move stays in its region: frozen columns among the frozen ones,
        // the rest among the rest (see `TableColumn::frozen`).
        let frozen = self.columns.iter().take_while(|c| c.frozen).count();
        let frozen = if frozen < self.columns.len() {
            frozen
        } else {
            0
        };
        let commit = crate::util::shared({
            let order = order.clone();
            move |from: usize, to: usize, window: &mut Window, cx: &mut App| {
                let to = clamp_to_region(from, to, frozen);
                if from == to || to >= order.len() {
                    return;
                }
                let next = moved_order(&order, from, to);
                if let Some(own) = &own_order {
                    own.update(cx, |value, cx| {
                        value.clone_from(&next);
                        cx.notify();
                    });
                }
                if let Some(cb) = &on_move {
                    cb(
                        &ColumnMove {
                            from,
                            to,
                            order: next,
                        },
                        window,
                        cx,
                    );
                }
            }
        });

        let accent = cx.colors().accent.color;
        let drag_now = *drag.read(cx);
        let mut header = header.relative();
        if let Some(active) = drag_now.filter(|d| d.active) {
            let measured = bounds.borrow();
            if let Some(target) = measured.1.get(active.target) {
                // The insertion edge: after the target when moving right,
                // before it when moving left.
                let x = if active.target > active.from {
                    target.right()
                } else {
                    target.left()
                };
                header = header.cursor(gpui::CursorStyle::ClosedHand).child(
                    gpui::div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(x - measured.0 - px(1.))
                        .w(px(2.))
                        .bg(accent)
                        .debug_selector(|| "table-column-drop-indicator".into()),
                );
            }
        }
        let down_bounds = bounds.clone();
        let move_bounds = bounds;
        let down_drag = drag.clone();
        let move_drag = drag.clone();
        let up_drag = drag.clone();
        let out_drag = drag;
        let up_commit = commit.clone();
        let out_commit = commit.clone();
        let key_commit = commit;
        let resizing = resizing.clone();
        let focus = header_focus.to_vec();
        header
            .on_mouse_down(gpui::MouseButton::Left, move |event, _, cx| {
                // A resize handle's press started a resize, not a move.
                if resizing.read(cx).is_some() {
                    return;
                }
                let Some(from) = header_at(&down_bounds.borrow().1, event.position.x) else {
                    return;
                };
                down_drag.update(cx, |d, _| {
                    *d = Some(ColumnDrag {
                        from,
                        start_x: f32::from(event.position.x),
                        active: false,
                        target: from,
                    });
                });
            })
            .on_mouse_move(move |event, _, cx| {
                let Some(mut current) = *move_drag.read(cx) else {
                    return;
                };
                if event.pressed_button != Some(gpui::MouseButton::Left) {
                    move_drag.update(cx, |d, cx| {
                        *d = None;
                        cx.notify();
                    });
                    return;
                }
                let x = f32::from(event.position.x);
                if !current.active && (x - current.start_x).abs() < 6. {
                    return;
                }
                current.active = true;
                if let Some(target) = header_at(&move_bounds.borrow().1, event.position.x) {
                    current.target = clamp_to_region(current.from, target, frozen);
                }
                move_drag.update(cx, |d, cx| {
                    if *d != Some(current) {
                        *d = Some(current);
                        cx.notify();
                    }
                });
            })
            .on_mouse_up(gpui::MouseButton::Left, move |_, window, cx| {
                let done = up_drag.update(cx, |d, cx| {
                    cx.notify();
                    d.take()
                });
                if let Some(d) = done.filter(|d| d.active) {
                    up_commit(d.from, d.target, window, cx);
                }
            })
            .on_mouse_up_out(gpui::MouseButton::Left, move |_, window, cx| {
                let done = out_drag.update(cx, |d, cx| {
                    cx.notify();
                    d.take()
                });
                if let Some(d) = done.filter(|d| d.active) {
                    out_commit(d.from, d.target, window, cx);
                }
            })
            .on_key_down(move |event, window, cx| {
                let m = &event.keystroke.modifiers;
                if !m.alt || m.control || m.platform || m.shift || m.function {
                    return;
                }
                let forward = match event.keystroke.key.as_str() {
                    "right" => true,
                    "left" => false,
                    _ => return,
                };
                let Some(from) = focus.iter().position(|h| h.is_focused(window)) else {
                    return;
                };
                let to = if forward {
                    (from + 1).min(focus.len() - 1)
                } else {
                    from.saturating_sub(1)
                };
                let to = clamp_to_region(from, to, frozen);
                cx.stop_propagation();
                crate::util::set_focus_visible(true, cx);
                if to != from {
                    key_commit(from, to, window, cx);
                    // The focus follows the column it moved.
                    window.focus(&focus[to], cx);
                }
            })
    }
}

/// What the rows need to draw and press the selected cell.
#[derive(Clone)]
struct CellCtx {
    /// The selected cell, as its row key and display position.
    selected: Option<(SharedString, usize)>,
    /// The selected cell's display position.
    column: gpui::Entity<Option<usize>>,
    /// The table's row cursor, which is the selected cell's row.
    cursor: gpui::Entity<Option<SharedString>>,
    focus: gpui::FocusHandle,
}

impl CellCtx {
    /// Marks and rings the selected cell, and makes a press select a cell.
    fn decorate(
        &self,
        el: gpui::Stateful<gpui::Div>,
        row: &SharedString,
        column: usize,
        disabled: bool,
        cx: &App,
    ) -> gpui::Stateful<gpui::Div> {
        let is_selected = self
            .selected
            .as_ref()
            .is_some_and(|(r, c)| r == row && *c == column);
        let mut el = el.a11y_selected(is_selected);
        if is_selected {
            el = el.child(
                crate::util::inset_focus_ring(cx).debug_selector(|| "table-selected-cell".into()),
            );
        }
        if disabled {
            return el;
        }
        let (cursor, column_state, focus, row) = (
            self.cursor.clone(),
            self.column.clone(),
            self.focus.clone(),
            row.clone(),
        );
        el.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
            window.focus(&focus, cx);
            crate::util::set_focus_visible(false, cx);
            column_state.update(cx, |c, cx| {
                *c = Some(column);
                cx.notify();
            });
            cursor.update(cx, |c, cx| {
                *c = Some(row.clone());
                cx.notify();
            });
        })
    }
}

impl Table {
    /// Resolves the selected cell: its row is the row cursor, its column is
    /// kept beside it. Reports a change the user made, and snaps a controlled
    /// table back to its prop.
    fn cell_selection_state(
        &self,
        base_id: &gpui::ElementId,
        order: &[usize],
        row_cursor: &gpui::Entity<Option<SharedString>>,
        focus: &gpui::FocusHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<CellCtx> {
        if !self.cell_selection.enabled || order.is_empty() {
            return None;
        }
        let last = order.len() - 1;
        let position = |column: usize| order.iter().position(|c| *c == column);
        let column =
            window.use_keyed_state(element_id::scoped(base_id, "cell-column"), cx, |_, _| {
                None::<usize>
            });
        let reported =
            window.use_keyed_state(element_id::scoped(base_id, "cell-reported"), cx, |_, _| {
                None::<TableCell>
            });
        let seed = match &self.cell_selection.selected {
            Some(prop) => prop.clone(),
            None => self.cell_selection.default.clone(),
        };
        crate::util::seed_once(window, cx, element_id::scoped(base_id, "cell-seed"), |cx| {
            if let Some(seed) = seed {
                row_cursor.update(cx, |c, _| *c = Some(seed.row.clone()));
                column.update(cx, |c, _| *c = position(seed.column));
                reported.update(cx, |r, _| *r = Some(seed));
            }
        });
        let at = column.read(cx).unwrap_or(0).min(last);
        let requested = row_cursor.read(cx).clone().map(|row| TableCell {
            row,
            column: order[at],
        });
        if requested.is_some() && requested != *reported.read(cx) {
            reported.update(cx, |r, _| r.clone_from(&requested));
            if let (Some(cb), Some(cell)) =
                (self.cell_selection.on_select.clone(), requested.clone())
            {
                window.defer(cx, move |window, cx| cb(&cell, window, cx));
            }
        }
        let shown = match &self.cell_selection.selected {
            Some(prop) => {
                if *prop != requested {
                    let prop = prop.clone();
                    row_cursor.update(cx, |c, _| *c = prop.as_ref().map(|p| p.row.clone()));
                    column.update(cx, |c, _| {
                        *c = prop.as_ref().and_then(|p| position(p.column));
                    });
                    reported.update(cx, |r, _| r.clone_from(&prop));
                }
                prop.clone()
            }
            None => requested,
        };
        Some(CellCtx {
            selected: shown.and_then(|cell| Some((cell.row, position(cell.column)?))),
            column,
            cursor: row_cursor.clone(),
            focus: focus.clone(),
        })
    }

    /// Left / Right / Home / End between the cells of a row, ahead of the
    /// row keyboard, while the body holds the focus.
    fn cell_selection_keys(
        &self,
        wrapper: gpui::Div,
        cell: &Option<CellCtx>,
        rows: &[SharedString],
        frozen: Option<FrozenColumns>,
    ) -> gpui::Div {
        let Some(cell) = cell.clone() else {
            return wrapper;
        };
        let last = self.columns.len().saturating_sub(1);
        let first_row = rows
            .iter()
            .find(|key| !self.disabled_keys.contains(key))
            .cloned();
        wrapper.capture_key_down(move |event, window, cx| {
            if !cell.focus.is_focused(window) {
                return;
            }
            let m = &event.keystroke.modifiers;
            if m.control || m.alt || m.platform || m.shift || m.function {
                return;
            }
            let at = cell.column.read(cx).unwrap_or(0).min(last);
            let to = match event.keystroke.key.as_str() {
                "left" => at.saturating_sub(1),
                "right" => (at + 1).min(last),
                "home" => 0,
                "end" => last,
                _ => return,
            };
            cx.stop_propagation();
            crate::util::set_focus_visible(true, cx);
            if cell.cursor.read(cx).is_none() {
                let first = first_row.clone();
                cell.cursor.update(cx, |c, cx| {
                    *c = first;
                    cx.notify();
                });
            }
            cell.column.update(cx, |c, cx| {
                *c = Some(to);
                cx.notify();
            });
            if let Some(frozen) = &frozen {
                frozen.reveal(to);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_column_padding_uses_one_rem_per_one_based_level() {
        assert_eq!(tree_column_padding(0), None);
        assert_eq!(tree_column_padding(1), Some(px(32.)));
        assert_eq!(tree_column_padding(2), Some(px(48.)));
        assert_eq!(tree_column_padding(3), Some(px(64.)));
    }

    #[test]
    fn a_new_column_starts_ascending() {
        let next = SortDescriptor::next(None, "name");
        assert_eq!(next, SortDescriptor::new("name", SortDirection::Ascending));
    }

    #[test]
    fn clicking_the_sorted_column_flips_it() {
        let current = SortDescriptor::new("name", SortDirection::Ascending);
        let next = SortDescriptor::next(Some(&current), "name");
        assert_eq!(next.direction, SortDirection::Descending);
        let back = SortDescriptor::next(Some(&next), "name");
        assert_eq!(back.direction, SortDirection::Ascending);
    }

    #[test]
    fn disabled_only_controlled_selection_is_indeterminate() {
        let selectable = vec![SharedString::from("alpha")];
        let selected = vec![SharedString::from("beta")];
        assert_eq!(select_all_flags(&selectable, &selected), (false, true));
    }

    #[test]
    fn switching_column_resets_to_ascending() {
        let current = SortDescriptor::new("name", SortDirection::Descending);
        let next = SortDescriptor::next(Some(&current), "size");
        assert_eq!(next, SortDescriptor::new("size", SortDirection::Ascending));
    }

    #[test]
    fn row_keys_fall_back_to_the_index() {
        let table = Table::new(vec!["A".into()])
            .row(vec![])
            .keyed_row("second", vec![])
            .row(vec![]);
        assert_eq!(
            table.row_keys(),
            vec![
                SharedString::from("0"),
                SharedString::from("second"),
                SharedString::from("2"),
            ]
        );
    }

    /// The Home/End gate takes the platform as an explicit bool, so this
    /// truth table is free of `cfg!` and mechanically proves both maps from
    /// any host: no macOS chord ever extends -- Shift and Alt+Shift move the
    /// focus alone -- while Windows and Linux extend exactly from
    /// Control+Shift.
    #[test]
    fn shift_home_end_extends_only_from_control_outside_macos() {
        for key in ["home", "end"] {
            assert!(
                !shift_home_end_extends(key, true, true),
                "macOS registers no Home/End extension"
            );
            assert!(!shift_home_end_extends(key, false, true));
            assert!(
                shift_home_end_extends(key, true, false),
                "Control+Shift+{key} must extend on Windows and Linux"
            );
            assert!(
                !shift_home_end_extends(key, false, false),
                "plain Shift+{key} must only move the focus"
            );
        }
    }

    /// The grid delegate's arrows never consult the Home/End gate: their
    /// forbidden extra chords are rejected earlier, by
    /// `exact_shift_navigation`.
    #[test]
    fn shift_navigation_keys_do_not_consult_the_home_end_gate() {
        for key in ["left", "right", "up", "down"] {
            assert!(shift_home_end_extends(key, false, true));
            assert!(shift_home_end_extends(key, true, false));
        }
    }

    /// The registration gate takes `Modifiers`, so the pinned chord map can
    /// be spelled out: macOS registers none, Shift, Alt, and Alt+Shift and
    /// every Control- or Meta-bearing chord is entirely inert, while
    /// Windows and Linux register none, Shift, Control, and Control+Shift
    /// and reject every Alt- or Meta-bearing chord. The upstream matcher
    /// sees only the browser's Alt/Control/Meta/Shift flags, so GPUI's
    /// `function` flag is ignored: `fn` stays registered on both maps, and
    /// it never rescues a chord the platform itself rejects.
    #[test]
    fn home_end_registration_matches_the_pinned_chord_map() {
        let none = gpui::Modifiers::none();
        let shift = gpui::Modifiers {
            shift: true,
            ..none
        };
        let alt = gpui::Modifiers { alt: true, ..none };
        let alt_shift = gpui::Modifiers { shift: true, ..alt };
        let function = gpui::Modifiers {
            function: true,
            ..none
        };
        let function_alt = gpui::Modifiers {
            alt: true,
            ..function
        };
        for modifiers in [none, shift, alt, alt_shift, function, function_alt] {
            assert!(
                home_end_registered(modifiers, true),
                "macOS must register {modifiers:?}"
            );
        }
        let control = gpui::Modifiers {
            control: true,
            ..none
        };
        let control_shift = gpui::Modifiers {
            shift: true,
            ..control
        };
        let platform = gpui::Modifiers {
            platform: true,
            ..none
        };
        let platform_shift = gpui::Modifiers {
            shift: true,
            ..platform
        };
        for modifiers in [control, control_shift, platform, platform_shift] {
            assert!(
                !home_end_registered(modifiers, true),
                "macOS must not register {modifiers:?}"
            );
        }
        for modifiers in [none, shift, control, control_shift, function] {
            assert!(
                home_end_registered(modifiers, false),
                "Windows and Linux must register {modifiers:?}"
            );
        }
        for modifiers in [alt, alt_shift, function_alt, platform, platform_shift] {
            assert!(
                !home_end_registered(modifiers, false),
                "Windows and Linux must not register {modifiers:?}"
            );
        }
    }

    /// The keystroke spellings real events hand the gate: `ctrl` parses to
    /// the Control field the Windows/Linux registration admits and macOS
    /// vetoes, `cmd` to the platform field macOS vetoes, `alt-shift` to
    /// the chord that stays registered (focus-only) on macOS alone, and
    /// `fn` to the flag the browser matcher never sees, so it registers
    /// exactly like the bare key on both maps.
    #[test]
    fn keystroke_spellings_reach_the_registration_gate() {
        let ctrl_shift_home = gpui::Keystroke::parse("ctrl-shift-home").unwrap();
        assert!(home_end_registered(ctrl_shift_home.modifiers, false));
        assert!(!home_end_registered(ctrl_shift_home.modifiers, true));
        let cmd_shift_home = gpui::Keystroke::parse("cmd-shift-home").unwrap();
        assert!(!home_end_registered(cmd_shift_home.modifiers, true));
        let alt_shift_end = gpui::Keystroke::parse("alt-shift-end").unwrap();
        assert!(home_end_registered(alt_shift_end.modifiers, true));
        assert!(!home_end_registered(alt_shift_end.modifiers, false));
        let fn_home = gpui::Keystroke::parse("fn-home").unwrap();
        assert!(fn_home.modifiers.function);
        assert!(home_end_registered(fn_home.modifiers, true));
        assert!(home_end_registered(fn_home.modifiers, false));
    }

    // The pinned sortable column hover only recolours the header text to
    // `--foreground`; it paints no background. `soft()` is a lighter, wrong
    // wash, so the check is mechanical.
    #[test]
    fn the_sort_header_hover_paints_no_background() {
        // Scan the implementation only; this test's own text names the
        // forbidden accessor.
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains(".group_hover(sort_group, |s| s.text_color(colors.foreground))"),
            "the sortable header hover must recolour the text to `--foreground` \
             (pinned `.table__column[data-allows-sorting]:hover`)"
        );
        assert!(
            !source.contains("colors.default.soft()"),
            "the sortable header hover must not come back as a soft background"
        );
    }

    // The pinned row hover is variant-specific: `.table-root--primary` rows
    // fill `bg-surface/40`, `.table-root--secondary` rows fill
    // `bg-default/50`.
    #[test]
    fn the_row_hover_is_variant_specific() {
        // Scan the implementation only.
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("colors.default.color.alpha(0.5)")
                && source.contains("colors.surface.background.alpha(0.4)"),
            "the row hover must read `bg-default/50` on secondary and \
             `bg-surface/40` on primary"
        );
    }

    // The pinned selected row fills `bg-surface/10`, and the rule follows the
    // hover rule in the pinned stylesheet, so it wins the cascade: a hovered
    // selected row keeps the selected fill. `accent.soft()` is a role wash
    // the pinned table never paints, so the check is mechanical.
    #[test]
    fn the_selected_row_fills_surface_ten_percent_and_outranks_the_hover() {
        // Scan the implementation only; this test's own text names the
        // forbidden accessor.
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains(
                "let selected_bg = is_selected.then(|| colors.surface.background.alpha(0.1));"
            ),
            "the selected row must fill `bg-surface/10` \
             (pinned `.table__row[data-selected] .table__cell`)"
        );
        assert!(
            source.contains(".when_some(selected_bg, |cell, bg| cell.bg(bg))")
                && source.contains("cell.group_hover(hover_group.clone(), move |s| s.bg(bg))"),
            "selected and hover fills must be painted on each cell, with the \
             selected row omitting the hover group so HeroUI's selected rule wins"
        );
        assert!(
            !source.contains("accent.soft()"),
            "the selected row must not paint a role soft wash"
        );
    }

    #[test]
    fn table_focus_ring_is_split_per_cell() {
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("fn table_cell_focus_ring(cx: &App, first: bool, last: bool)")
                && source.contains(".top_0()")
                && source.contains(".bottom_0()")
                && source.contains("cell_el = cell_el.child(table_cell_focus_ring("),
            "focused rows must paint clipped edge strips on each cell"
        );
        assert!(
            !source.contains("r.child(crate::util::inset_focus_ring(cx))"),
            "the row must not use one rounded overlay that bleeds across cells"
        );
    }

    #[test]
    fn table_header_uses_medium_weight() {
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains(".text_size(px(12.))")
                && source.contains(".line_height(px(16.))")
                && source.contains(".font_weight(gpui::FontWeight::MEDIUM)"),
            "table column headers must use the pinned `font-medium` weight"
        );
    }

    #[test]
    fn table_header_preserves_caller_label_case() {
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains(".child(column.label.clone());"),
            "table column headers must render the caller's label without uppercasing it"
        );
    }

    #[test]
    fn table_header_and_rows_use_half_alpha_separator() {
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.match_indices("colors.separator.alpha(0.5)").count() >= 2,
            "the header and row separators must use the separator token at 50% alpha"
        );
    }

    #[test]
    fn table_last_row_omits_the_bottom_separator() {
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("if !self.secondary && i + 1 < self.row_keys.len() {")
                && source
                    .contains("row = row.border_b_1().border_color(colors.separator.alpha(0.5));"),
            "the final visible Table row must not receive a bottom separator"
        );
    }

    #[test]
    fn secondary_table_uses_surface_headers_and_cell_separators() {
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("cell = cell\n                .bg(colors.surface_secondary)")
                && source.contains("colors.separator_tertiary().alpha(0.5)"),
            "secondary tables must paint the header surface and per-cell tertiary separators"
        );
    }

    #[test]
    fn table_headers_keep_short_separators_and_rotate_builtin_sort_indicator() {
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains(".h(px(16.))")
                && source.contains(".bg(colors.separator)")
                && source.contains("rotating_indicator_with_duration")
                && source.contains(".path(icons::CHEVRON_UP)"),
            "table headers must draw the pinned separator and use the shared 100ms rotation"
        );
    }

    /// The primary variant's rows sit in the rounded `.table__body`, whose
    /// top and bottom curves both bound the row run.
    #[test]
    fn primary_row_fills_round_to_the_body_on_the_edges_they_touch() {
        let body = px(16.);
        let wrapper = px(24.);
        assert_eq!(
            edge_corner_rounding(false, false, true, true, body, wrapper),
            (Some(body), Some(body))
        );
        // A footer sits below the body, so it never takes the body's curve.
        assert_eq!(
            edge_corner_rounding(false, true, true, true, body, wrapper),
            (Some(body), Some(body))
        );
        // A virtual body scrolled off either edge is cut straight there.
        assert_eq!(
            edge_corner_rounding(false, false, false, true, body, wrapper),
            (None, Some(body))
        );
        assert_eq!(
            edge_corner_rounding(false, false, true, false, body, wrapper),
            (Some(body), None)
        );
        assert_eq!(
            edge_corner_rounding(false, false, false, false, body, wrapper),
            (None, None)
        );
    }

    /// The secondary variant is flat, so the wrapper's corners are the only
    /// curve -- and the header always stands between them and the first row.
    #[test]
    fn secondary_row_fills_round_only_to_the_wrapper_bottom() {
        let body = px(16.);
        let wrapper = px(24.);
        assert_eq!(
            edge_corner_rounding(true, false, true, true, body, wrapper),
            (None, Some(wrapper))
        );
        // A footer takes the wrapper's bottom edge instead.
        assert_eq!(
            edge_corner_rounding(true, true, true, true, body, wrapper),
            (None, None)
        );
        assert_eq!(
            edge_corner_rounding(true, false, true, false, body, wrapper),
            (None, None)
        );
    }

    /// The decision is taken once per render and threaded through `RowCtx`,
    /// and only the first and last rows' outer cells may consume it.
    #[test]
    fn only_the_outer_cells_of_the_edge_rows_round_their_fill() {
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("let round_top = (i == 0).then_some(self.round_top).flatten();")
                && source.contains("let round_bottom = (i + 1 == self.row_keys.len())"),
            "only the first and last body rows may round their cell fills"
        );
        assert!(
            source.contains(".when(!self.selectable && c == 0, |cell| {")
                && source.contains(".when(c + 1 == data_count, |cell| {")
                && source.contains(".when_some(round_top, |cell, r| cell.rounded_tl(r))")
                && source.contains(".when_some(round_bottom, |cell, r| cell.rounded_br(r))"),
            "the row's leading and trailing cells must carry the box's corners"
        );
        assert!(
            source.contains("virtual_scroll_now.is_scrolled_to_top()")
                && source.contains("virtual_scroll_now.is_scrolled_to_end()")
                && source.contains("top.item_ix == 0 && top.offset_in_item <= px(0.5)"),
            "a virtual body must only round the edges it is scrolled to"
        );
    }

    #[test]
    fn virtual_selectable_keys_are_not_materialized_on_every_render() {
        let source = include_str!("table.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            !source.contains("let selectable_collection_keys: Vec<SharedString> ="),
            "virtual Table must not allocate a selectable-key Vec for every render"
        );
    }
}

crate::util::impl_component_styled!(Table);
