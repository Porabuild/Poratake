//! `ResizablePanelGroup` and `ResizablePanel` — split panels with a drag
//! handle between each pair (HeroGPUI extension; HeroUI v3 has no resizable
//! or split-view component).
//!
//! A group lays its panels out side by side ([`Orientation::Horizontal`], the
//! default) or stacked ([`Orientation::Vertical`]). Sizes are **percentages
//! of the space the panels share** (the group's length minus its handles),
//! so a layout survives a window resize unchanged; they always sum to 100.
//! Sizes that do not (defaults or controlled) are scaled to 100, or split
//! evenly when they sum to zero, and then moved inside the panels' limits.
//!
//! - **Pointer.** Pressing a handle focuses it and starts a drag. The drag
//!   captures the pointer (`Window::capture_pointer`, re-taken every frame
//!   because hitbox ids are per frame), so it keeps its resize cursor and
//!   keeps resizing after the pointer leaves the handle or the group; the
//!   release ends it.
//! - **Keyboard.** Every handle is a tab stop. The arrow keys along the
//!   group's axis (Left/Right for a horizontal group, Up/Down for a vertical
//!   one) move it by [`ResizablePanelGroup::keyboard_step`] percent, four
//!   times that with Shift; Home and End move it as far as the two panels'
//!   limits allow toward the start and the end.
//! - **Limits.** Moving a handle resizes only the two panels beside it, and
//!   both stay inside their [`ResizablePanel::min_size`] /
//!   [`ResizablePanel::max_size`] during a drag and on every key. Pixel
//!   limits ([`ResizablePanel::min_size_px`] / [`ResizablePanel::max_size_px`])
//!   are converted to percent of the group's measured length and combined
//!   with those (the stricter wins), for the initial layout, a drag, a key
//!   and a window resize alike. The group is measured when it is laid out,
//!   so a pixel limit applies from the frame after the first.
//! - **Collapsing.** A [`ResizablePanel::collapsible`] panel may also sit at
//!   its [`ResizablePanel::collapsed_size`] (default 0), below its minimum.
//!   Dragging it past halfway between the two collapses it, and dragging it
//!   back past halfway restores the minimum; an arrow key that would leave it
//!   between them jumps to the other side. Enter on a handle collapses the
//!   panel before it (or, when only that one is collapsible, the panel after
//!   it) and, when collapsed, restores the size it had before, as the
//!   WAI-ARIA window splitter pattern describes.
//! - **Callbacks.** [`ResizablePanelGroup::on_resize`] runs on every change,
//!   [`ResizablePanelGroup::on_resize_end`] once per gesture: at the release
//!   of a drag that changed the sizes, and after each key that did.
//! - **Accessibility.** Each handle reports `Role::Splitter` (the WAI-ARIA
//!   window splitter, a focusable `separator`) with the orientation of the
//!   line it draws and a value range: the size of the panel before it, within
//!   the limits the pair allows.
//!
//! ```
//! use herogpui_components::{ResizablePanel, ResizablePanelGroup};
//! use gpui::{div, prelude::*};
//!
//! let split = ResizablePanelGroup::new("editor-split")
//!     .panel(
//!         ResizablePanel::new()
//!             .default_size(25.)
//!             .min_size(15.)
//!             .max_size(40.)
//!             .child(div().child("Files")),
//!     )
//!     .panel(ResizablePanel::new().child(div().child("Editor")))
//!     .on_resize(|sizes, _window, _cx| println!("{sizes:?}"));
//! ```

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    div, prelude::FluentBuilder as _, px, AnyElement, App, Bounds, ElementId, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Point, RenderOnce, SharedString, Styled,
    Window,
};
use herogpui_core::{element_id, Orientation};
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};

type ResizeCallback = Arc<dyn Fn(&[f32], &mut Window, &mut App) + 'static>;

/// The thickness of a handle along the group's axis. The visible line is one
/// pixel in its centre; the rest is the press target.
pub const RESIZABLE_HANDLE_SIZE: Pixels = px(8.);

/// One panel of a [`ResizablePanelGroup`]. Sizes are percentages of the space
/// the group's panels share.
#[must_use = "builder methods return a new value; pass it on to its component"]
pub struct ResizablePanel {
    default_size: Option<f32>,
    min_size: f32,
    max_size: f32,
    min_size_px: Option<Pixels>,
    max_size_px: Option<Pixels>,
    collapsible: bool,
    collapsed_size: f32,
    children: Vec<AnyElement>,
}

impl Default for ResizablePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ResizablePanel {
    /// A panel with no size of its own: panels without a
    /// [`default_size`](Self::default_size) share what the others leave.
    pub fn new() -> Self {
        Self {
            default_size: None,
            min_size: 0.,
            max_size: 100.,
            min_size_px: None,
            max_size_px: None,
            collapsible: false,
            collapsed_size: 0.,
            children: Vec::new(),
        }
    }

    /// The initial size in percent, used while the group is uncontrolled.
    /// Defaults are scaled together when they do not add up to 100.
    pub fn default_size(mut self, percent: f32) -> Self {
        self.default_size = Some(percent.max(0.));
        self
    }

    /// The smallest size in percent a resize may leave this panel at
    /// (default 0).
    pub fn min_size(mut self, percent: f32) -> Self {
        self.min_size = percent.clamp(0., 100.);
        self
    }

    /// The largest size in percent a resize may give this panel
    /// (default 100).
    pub fn max_size(mut self, percent: f32) -> Self {
        self.max_size = percent.clamp(0., 100.);
        self
    }

    /// The smallest length in pixels a resize may leave this panel at,
    /// combined with [`min_size`](Self::min_size) (the larger wins).
    pub fn min_size_px(mut self, min: impl Into<Pixels>) -> Self {
        self.min_size_px = Some(min.into().max(px(0.)));
        self
    }

    /// The largest length in pixels a resize may give this panel, combined
    /// with [`max_size`](Self::max_size) (the smaller wins, but never below
    /// the minimum).
    pub fn max_size_px(mut self, max: impl Into<Pixels>) -> Self {
        self.max_size_px = Some(max.into().max(px(0.)));
        self
    }

    /// Lets the panel collapse to its [`collapsed_size`](Self::collapsed_size)
    /// below its minimum: by a drag past halfway, an arrow key past the
    /// minimum, Home/End, or Enter on the handle beside it.
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// The size in percent a [`collapsible`](Self::collapsible) panel
    /// collapses to (default 0). A collapsed size at or above the minimum
    /// leaves nothing to collapse.
    pub fn collapsed_size(mut self, percent: f32) -> Self {
        self.collapsed_size = percent.clamp(0., 100.);
        self
    }

    /// The panel's limits in percent of `available` pixels (the group's
    /// length minus its handles), or of the percentage limits alone while
    /// the group has not been measured.
    fn limits(&self, available: Option<f32>) -> Limit {
        let mut min = self.min_size;
        let mut max = self.max_size;
        if let Some(available) = available.filter(|a| *a > 0.) {
            if let Some(px_min) = self.min_size_px {
                min = min.max(f32::from(px_min) / available * 100.);
            }
            if let Some(px_max) = self.max_size_px {
                max = max.min(f32::from(px_max) / available * 100.);
            }
        }
        let min = min.clamp(0., 100.);
        let max = max.clamp(0., 100.).max(min);
        let collapsed = self
            .collapsible
            .then_some(self.collapsed_size.min(min))
            .filter(|collapsed| *collapsed < min - EPSILON);
        Limit {
            min,
            max,
            collapsed,
        }
    }

    fn has_pixel_limits(&self) -> bool {
        self.min_size_px.is_some() || self.max_size_px.is_some()
    }
}

/// Slack for comparing percentages.
const EPSILON: f32 = 0.0001;

/// One panel's allowed sizes in percent: `min..=max`, and the collapsed
/// size when the panel is collapsible (always below `min`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Limit {
    min: f32,
    max: f32,
    collapsed: Option<f32>,
}

impl Limit {
    #[cfg(test)]
    fn range(min: f32, max: f32) -> Self {
        Self {
            min,
            max,
            collapsed: None,
        }
    }

    /// The allowed sizes as closed ranges (the collapsed size is a point).
    fn pieces(self) -> impl Iterator<Item = (f32, f32)> {
        std::iter::once((self.min, self.max)).chain(self.collapsed.map(|c| (c, c)))
    }

    fn is_collapsed(self, size: f32) -> bool {
        self.collapsed
            .is_some_and(|collapsed| (size - collapsed).abs() <= 0.01)
    }
}

impl ParentElement for ResizablePanel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// A row or column of [`ResizablePanel`]s with a drag handle between each
/// pair. See the [module docs](self).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ResizablePanelGroup {
    id: ElementId,
    orientation: Orientation,
    panels: Vec<ResizablePanel>,
    /// The controlled sizes; `Some` only through [`Self::sizes`].
    sizes: Option<Vec<f32>>,
    keyboard_step: f32,
    is_disabled: bool,
    on_resize: Option<ResizeCallback>,
    on_resize_end: Option<ResizeCallback>,
}

impl ResizablePanelGroup {
    /// An empty horizontal group. `id` keys its sizes, drag and handle focus;
    /// give every instance its own.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            orientation: Orientation::Horizontal,
            panels: Vec::new(),
            sizes: None,
            keyboard_step: 5.,
            is_disabled: false,
            on_resize: None,
            on_resize_end: None,
        }
    }

    /// Side by side (`Horizontal`, the default) or stacked (`Vertical`).
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Appends a panel.
    pub fn panel(mut self, panel: ResizablePanel) -> Self {
        self.panels.push(panel);
        self
    }

    /// Appends several panels.
    pub fn panels(mut self, panels: impl IntoIterator<Item = ResizablePanel>) -> Self {
        self.panels.extend(panels);
        self
    }

    /// Controlled sizes in percent, one per panel. The group then reports
    /// every resize through [`Self::on_resize`] and renders only what it is
    /// given, normalized as the defaults are: scaled to sum to 100 (split
    /// evenly when they sum to zero; negative or non-finite entries count as
    /// zero) and moved inside the panels' limits. A list whose length does not
    /// match the panels is ignored in favour of the defaults.
    pub fn sizes(mut self, sizes: impl IntoIterator<Item = f32>) -> Self {
        self.sizes = Some(sizes.into_iter().collect());
        self
    }

    /// How far one arrow key moves a handle, in percent (default 5; Shift
    /// moves four times as far).
    pub fn keyboard_step(mut self, percent: f32) -> Self {
        self.keyboard_step = percent.max(0.);
        self
    }

    /// A disabled group draws its handles but does not resize, and its
    /// handles leave the tab order.
    pub fn is_disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }

    /// Runs with every panel's size, in percent, whenever a drag or a key
    /// changes them.
    pub fn on_resize(mut self, f: impl Fn(&[f32], &mut Window, &mut App) + 'static) -> Self {
        self.on_resize = Some(Arc::new(f));
        self
    }

    /// Runs with every panel's size, in percent, once a resize gesture ends:
    /// at the release of a drag that changed the sizes, and after each key
    /// (arrow, Home, End, Enter) that did. For persisting a layout without
    /// writing on every pointer move.
    pub fn on_resize_end(mut self, f: impl Fn(&[f32], &mut Window, &mut App) + 'static) -> Self {
        self.on_resize_end = Some(Arc::new(f));
        self
    }
}

/// The sizes the panels start at: explicit defaults, the rest shared evenly,
/// then [normalized](normalize) to 100 and the panels' limits.
fn default_sizes(panels: &[ResizablePanel], limits: &[Limit]) -> Vec<f32> {
    let fixed: f32 = panels
        .iter()
        .filter_map(|p| p.default_size)
        .filter(|size| size.is_finite())
        .sum();
    let free = panels.iter().filter(|p| p.default_size.is_none()).count();
    let share = if free > 0 {
        (100. - fixed).max(0.) / free as f32
    } else {
        0.
    };
    let sizes: Vec<f32> = panels
        .iter()
        .map(|p| p.default_size.unwrap_or(share))
        .collect();
    normalize(&sizes, limits)
}

/// `sizes` scaled to sum to 100 (an even split when they sum to zero;
/// negative and non-finite entries count as zero), then moved inside
/// `limits` with the sum kept at 100: every panel is clamped to its own
/// range and the difference is shared among the panels with room left, in
/// proportion to that room. Limits no layout can satisfy (minimums over
/// 100 or maximums under it) leave the scaled sizes unclamped. A
/// collapsible panel under halfway between its collapsed size and its
/// minimum is collapsed, and then holds that size.
fn normalize(sizes: &[f32], limits: &[Limit]) -> Vec<f32> {
    let n = sizes.len();
    if n == 0 {
        return Vec::new();
    }
    let mut out: Vec<f32> = sizes
        .iter()
        .map(|&v| if v.is_finite() && v > 0. { v } else { 0. })
        .collect();
    let total: f32 = out.iter().sum();
    if !total.is_finite() || total <= 0. {
        out = vec![100. / n as f32; n];
    } else if (total - 100.).abs() > 0.001 {
        for size in &mut out {
            *size *= 100. / total;
        }
    }
    if limits.len() != n {
        return out;
    }
    let limits: Vec<(f32, f32)> = out
        .iter()
        .zip(limits)
        .map(|(&size, limit)| match limit.collapsed {
            Some(collapsed) if size < (collapsed + limit.min) / 2. => (collapsed, collapsed),
            _ => (limit.min, limit.max),
        })
        .collect();
    let limits = &limits[..];
    let min_total: f32 = limits.iter().map(|l| l.0).sum();
    let max_total: f32 = limits.iter().map(|l| l.1).sum();
    if min_total > 100. + 0.001 || max_total < 100. - 0.001 {
        return out;
    }
    for (size, &(min, max)) in out.iter_mut().zip(limits) {
        *size = size.clamp(min, max);
    }
    let diff = 100. - out.iter().sum::<f32>();
    if diff.abs() > 0.0001 {
        let room: Vec<f32> = out
            .iter()
            .zip(limits)
            .map(|(&size, &(min, max))| if diff > 0. { max - size } else { size - min })
            .collect();
        let total_room: f32 = room.iter().sum();
        if total_room > 0. {
            for (size, room) in out.iter_mut().zip(room) {
                *size += diff * room / total_room;
            }
        }
    }
    out
}

/// The sizes the panel before handle `handle` may take while the pair keeps
/// its combined size and both stay within their limits: at most four closed
/// ranges (each panel's `min..=max` or collapsed point, against the other's).
fn pair_pieces(sizes: &[f32], handle: usize, limits: &[Limit]) -> Vec<(f32, f32)> {
    let total = sizes[handle] + sizes[handle + 1];
    let mut pieces = Vec::new();
    for (min_a, max_a) in limits[handle].pieces() {
        for (min_b, max_b) in limits[handle + 1].pieces() {
            let lower = min_a.max(total - max_b);
            let upper = max_a.min(total - min_b);
            if lower <= upper + EPSILON {
                pieces.push((lower, upper.max(lower)));
            }
        }
    }
    pieces
}

/// The range the panel before handle `handle` may take: the span of
/// [`pair_pieces`]. `None` when the two limits cannot both hold.
fn pair_range(sizes: &[f32], handle: usize, limits: &[Limit]) -> Option<(f32, f32)> {
    let pieces = pair_pieces(sizes, handle, limits);
    let lower = pieces.iter().map(|p| p.0).reduce(f32::min)?;
    let upper = pieces.iter().map(|p| p.1).reduce(f32::max)?;
    Some((lower, upper))
}

/// The allowed size nearest `target`. For a collapsible panel that is the
/// collapsed size below halfway to the minimum and the minimum above it.
fn nearest(pieces: &[(f32, f32)], target: f32) -> Option<f32> {
    pieces
        .iter()
        .map(|&(lower, upper)| target.clamp(lower, upper))
        .min_by(|a, b| (a - target).abs().total_cmp(&(b - target).abs()))
}

/// `sizes` with the panel before `handle` resized toward `target` and the
/// panel after it absorbing the difference, both inside their limits.
fn resize_pair(sizes: &[f32], handle: usize, target: f32, limits: &[Limit]) -> Vec<f32> {
    let mut next = sizes.to_vec();
    let Some(first) = nearest(&pair_pieces(sizes, handle, limits), target) else {
        return next;
    };
    let total = sizes[handle] + sizes[handle + 1];
    next[handle] = first;
    next[handle + 1] = total - first;
    next
}

/// [`resize_pair`] for a key: a step that the nearest allowed size would
/// swallow (a collapsed panel's arrow toward its minimum, or a panel at its
/// minimum stepping toward its collapsed size) moves on to the next allowed
/// size in the key's direction instead.
fn step_pair(sizes: &[f32], handle: usize, target: f32, limits: &[Limit]) -> Vec<f32> {
    let current = sizes[handle];
    let mut next = resize_pair(sizes, handle, target, limits);
    let pieces = pair_pieces(sizes, handle, limits);
    let jump = if target > current + EPSILON && next[handle] <= current + EPSILON {
        pieces
            .iter()
            .map(|p| p.0)
            .filter(|lower| *lower > current + EPSILON)
            .reduce(f32::min)
    } else if target < current - EPSILON && next[handle] >= current - EPSILON {
        pieces
            .iter()
            .map(|p| p.1)
            .filter(|upper| *upper < current - EPSILON)
            .reduce(f32::max)
    } else {
        None
    };
    if let Some(first) = jump {
        let total = sizes[handle] + sizes[handle + 1];
        next[handle] = first;
        next[handle + 1] = total - first;
    }
    next
}

/// Enter on handle `handle`: collapse the panel before it (or, when only
/// that one is collapsible, the panel after it), or restore it to its size
/// before it collapsed (its minimum when it started collapsed). `None` when
/// neither panel is collapsible.
fn toggle_collapse(
    sizes: &[f32],
    handle: usize,
    limits: &[Limit],
    restore: &[Option<f32>],
) -> Option<Vec<f32>> {
    let panel = if limits[handle].collapsed.is_some() {
        handle
    } else if limits[handle + 1].collapsed.is_some() {
        handle + 1
    } else {
        return None;
    };
    let limit = limits[panel];
    let collapsed = limit.collapsed?;
    let size = if limit.is_collapsed(sizes[panel]) {
        restore
            .get(panel)
            .copied()
            .flatten()
            .unwrap_or(limit.min)
            .max(limit.min)
    } else {
        collapsed
    };
    let total = sizes[handle] + sizes[handle + 1];
    let first = if panel == handle { size } else { total - size };
    Some(resize_pair(sizes, handle, first, limits))
}

/// A drag in progress: which handle, the pointer's axis coordinate at the
/// press, and the sizes then. Every move resizes from the press, so the
/// result does not drift with the number of move events. `last` is what the
/// drag last committed (the press-time sizes before the first move): several
/// moves can arrive between two frames, so a controlled group compares each
/// move with it rather than with the sizes the frame rendered.
#[derive(Clone)]
struct Drag {
    handle: usize,
    origin: f32,
    start: Vec<f32>,
    last: Vec<f32>,
}

/// What the event closures share.
#[derive(Clone)]
struct GroupState {
    store: gpui::Entity<Vec<f32>>,
    controlled: bool,
    limits: Rc<Vec<Limit>>,
    /// Each panel's size before it last collapsed, for Enter to restore.
    restore: gpui::Entity<Vec<Option<f32>>>,
    on_resize: Option<ResizeCallback>,
    on_resize_end: Option<ResizeCallback>,
}

impl GroupState {
    /// Commits `next` when it differs from `now`: the uncontrolled store
    /// takes it, and the caller hears about it either way. Returns whether
    /// it committed.
    fn apply(&self, now: &[f32], next: Vec<f32>, window: &mut Window, cx: &mut App) -> bool {
        if next.len() != now.len()
            || next
                .iter()
                .zip(now)
                .all(|(a, b)| (a - b).abs() <= f32::EPSILON)
        {
            return false;
        }
        // Remember what a panel measured before it collapsed.
        let collapsing: Vec<(usize, f32)> = now
            .iter()
            .zip(&next)
            .zip(self.limits.iter())
            .enumerate()
            .filter(|(_, ((was, is), limit))| {
                !limit.is_collapsed(**was) && limit.is_collapsed(**is)
            })
            .map(|(ix, ((was, _), _))| (ix, *was))
            .collect();
        if !collapsing.is_empty() {
            self.restore.update(cx, |restore, _| {
                for (ix, size) in collapsing {
                    if let Some(slot) = restore.get_mut(ix) {
                        *slot = Some(size);
                    }
                }
            });
        }
        if !self.controlled {
            self.store.update(cx, |sizes, cx| {
                sizes.clone_from(&next);
                cx.notify();
            });
        }
        if let Some(cb) = &self.on_resize {
            cb(&next, window, cx);
        }
        true
    }

    fn resize_ended(&self, sizes: &[f32], window: &mut Window, cx: &mut App) {
        if let Some(cb) = &self.on_resize_end {
            cb(sizes, window, cx);
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

impl RenderOnce for ResizablePanelGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = self.id.clone();
        let selector = selector_base(&base);
        let count = self.panels.len();
        let horizontal = self.orientation == Orientation::Horizontal;
        let group_bounds = window
            .use_keyed_state(element_id::scoped(&base, "bounds"), cx, |_, _| {
                Rc::new(Cell::new(None::<Bounds<Pixels>>))
            })
            .read(cx)
            .clone();
        let handle_total = f32::from(RESIZABLE_HANDLE_SIZE) * count.saturating_sub(1) as f32;
        // Pixel limits need the space the panels share, which the last frame
        // measured (none before the first layout).
        let available = group_bounds.get().map(|bounds| {
            f32::from(if horizontal {
                bounds.size.width
            } else {
                bounds.size.height
            }) - handle_total
        });
        let has_pixel_limits = self.panels.iter().any(ResizablePanel::has_pixel_limits);
        let limits: Rc<Vec<Limit>> = Rc::new(
            self.panels
                .iter()
                .map(|panel| panel.limits(available))
                .collect(),
        );
        let defaults = default_sizes(&self.panels, &limits);

        let store = window.use_keyed_state(element_id::scoped(&base, "sizes"), cx, {
            let defaults = defaults.clone();
            move |_, _| defaults
        });
        if store.read(cx).len() != count {
            store.update(cx, |sizes, _| sizes.clone_from(&defaults));
        }
        let controlled = self.sizes.as_ref().is_some_and(|s| s.len() == count);
        let sizes: Vec<f32> = match &self.sizes {
            Some(sizes) if controlled => normalize(sizes, &limits),
            _ => {
                // Limits can change between renders; keep the store inside
                // them so the handles' ranges always contain their values.
                let stored = store.read(cx).clone();
                let normal = normalize(&stored, &limits);
                if normal != stored {
                    store.update(cx, |sizes, _| sizes.clone_from(&normal));
                }
                normal
            }
        };
        let drag =
            window.use_keyed_state(element_id::scoped(&base, "drag"), cx, |_, _| None::<Drag>);
        // Panels removed mid-drag can take the dragged handle with them.
        if drag
            .read(cx)
            .as_ref()
            .is_some_and(|d| d.handle + 1 >= count || d.start.len() != count)
        {
            drag.update(cx, |value, _| *value = None);
        }
        let restore = window.use_keyed_state(element_id::scoped(&base, "restore"), cx, |_, _| {
            Vec::<Option<f32>>::new()
        });
        if restore.read(cx).len() != count {
            restore.update(cx, |restore, _| *restore = vec![None; count]);
        }
        let handles: Vec<gpui::FocusHandle> = (0..count.saturating_sub(1))
            .map(|ix| {
                window
                    .use_keyed_state(
                        element_id::indexed(&base, "handle-focus", ix),
                        cx,
                        |_, cx| cx.focus_handle(),
                    )
                    .read(cx)
                    .clone()
                    .tab_stop(!self.is_disabled)
            })
            .collect();

        let state = GroupState {
            store,
            controlled,
            limits: limits.clone(),
            restore,
            on_resize: self.on_resize.clone(),
            on_resize_end: self.on_resize_end.clone(),
        };
        let dragging = drag.read(cx).as_ref().map(|d| d.handle);

        let colors = cx.colors().clone();
        let focus_visible = crate::util::focus_visible(cx);
        let cursor_style = if horizontal {
            gpui::CursorStyle::ResizeColumn
        } else {
            gpui::CursorStyle::ResizeRow
        };
        // The line a handle draws runs across the group's axis: a horizontal
        // group's handles are vertical lines.
        let separator_orientation = if horizontal {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        };
        let axis = move |point: Point<Pixels>| -> f32 {
            f32::from(if horizontal { point.x } else { point.y })
        };

        let mut root = div()
            .id(base.clone())
            .relative()
            .flex()
            .when(horizontal, |el| el.flex_row())
            .when(!horizontal, |el| el.flex_col())
            .size_full()
            .overflow_hidden()
            .debug_selector({
                let name = format!("{selector}-group");
                move || name
            });

        let panels = self.panels;
        for (ix, panel) in panels.into_iter().enumerate() {
            let size = sizes.get(ix).copied().unwrap_or(0.);
            root = root.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_basis(px(0.))
                    .flex_grow(size)
                    .flex_shrink(1.)
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .debug_selector({
                        let name = format!("{selector}-panel-{ix}");
                        move || name
                    })
                    .children(panel.children),
            );
            if ix + 1 >= count {
                continue;
            }

            let handle = handles[ix].clone();
            let focused = handle.is_focused(window);
            let active = dragging == Some(ix) || (focused && focus_visible);
            let (lower, upper) = pair_range(&sizes, ix, &limits).unwrap_or((size, size));
            let group_name = SharedString::from(format!("{selector}-handle-{ix}"));
            let mut handle_el = div()
                .id(element_id::indexed(&base, "handle", ix))
                // The WAI-ARIA window splitter: a focusable `separator` whose
                // value is the size of the pane before it. AccessKit spells
                // that role `Splitter` (its `Separator` would be the rule a
                // `Separator` component draws, which 0.24 does not define).
                .a11y(a11y::Role::Splitter)
                .a11y_orientation(separator_orientation)
                .a11y_range(
                    &a11y::Range::new(lower as f64, upper as f64, size as f64)
                        .step(self.keyboard_step as f64),
                )
                .group(group_name.clone())
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .when(horizontal, |el| el.w(RESIZABLE_HANDLE_SIZE).h_full())
                .when(!horizontal, |el| el.h(RESIZABLE_HANDLE_SIZE).w_full())
                .debug_selector({
                    let name = group_name.to_string();
                    move || name
                })
                .child(
                    div()
                        .when(horizontal, |el| el.w(px(1.)).h_full())
                        .when(!horizontal, |el| el.h(px(1.)).w_full())
                        .bg(if active {
                            colors.accent.color
                        } else {
                            colors.separator
                        })
                        .when(!self.is_disabled, |line| {
                            line.group_hover(group_name.clone(), |s| s.bg(colors.muted))
                        }),
                );
            if !self.is_disabled {
                handle_el = handle_el
                    .track_focus(&handle)
                    .cursor(cursor_style)
                    .when(focused && focus_visible, |el| {
                        el.child(crate::util::focus_ring_overlay(px(2.), false, cx))
                    });

                // Pointer capture: the handle's own hitbox, re-captured on
                // every frame of the drag because a hitbox id names one frame's
                // hitbox only. The captured hitbox counts as hovered wherever
                // the pointer is, so the resize cursor stays with the drag.
                let is_dragging = dragging == Some(ix);
                handle_el = handle_el.child(
                    gpui::canvas(
                        |bounds, window, _| {
                            window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal)
                        },
                        move |_, hitbox, window, _| {
                            if is_dragging {
                                window.capture_pointer(hitbox.id);
                                window.set_cursor_style(cursor_style, &hitbox);
                            }
                        },
                    )
                    .absolute()
                    .inset_0(),
                );

                let press_drag = drag.clone();
                let press_focus = handle.clone();
                let press_sizes = sizes.clone();
                handle_el = handle_el.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    crate::util::set_focus_visible(false, cx);
                    window.focus(&press_focus, cx);
                    press_drag.update(cx, |value, cx| {
                        *value = Some(Drag {
                            handle: ix,
                            origin: axis(event.position),
                            start: press_sizes.clone(),
                            last: press_sizes.clone(),
                        });
                        cx.notify();
                    });
                    cx.stop_propagation();
                });

                let keys = state.clone();
                let key_focus = handle.clone();
                let key_sizes = sizes.clone();
                let step = self.keyboard_step;
                handle_el = handle_el.on_key_down(move |event, window, cx| {
                    if !key_focus.is_focused(window) {
                        return;
                    }
                    let m = &event.keystroke.modifiers;
                    if m.control || m.alt || m.platform || m.function {
                        return;
                    }
                    let (decrease, increase) = if horizontal {
                        ("left", "right")
                    } else {
                        ("up", "down")
                    };
                    let delta = if m.shift { step * 4. } else { step };
                    let current = key_sizes[ix];
                    let next = match event.keystroke.key.as_str() {
                        k if k == decrease => {
                            step_pair(&key_sizes, ix, current - delta, &keys.limits)
                        }
                        k if k == increase => {
                            step_pair(&key_sizes, ix, current + delta, &keys.limits)
                        }
                        // The pair's extremes, a collapsed size included.
                        "home" if !m.shift => resize_pair(&key_sizes, ix, -1., &keys.limits),
                        "end" if !m.shift => resize_pair(&key_sizes, ix, 101., &keys.limits),
                        "enter" if !m.shift => {
                            let Some(next) = toggle_collapse(
                                &key_sizes,
                                ix,
                                &keys.limits,
                                keys.restore.read(cx),
                            ) else {
                                return;
                            };
                            next
                        }
                        _ => return,
                    };
                    crate::util::set_focus_visible(true, cx);
                    if keys.apply(&key_sizes, next.clone(), window, cx) {
                        keys.resize_ended(&next, window, cx);
                    }
                    cx.stop_propagation();
                });
            }
            if !self.is_disabled {
                handle_el = crate::util::record_focus_bounds(handle_el, &handle, window, cx);
            }
            root = root.child(handle_el);
        }

        // The group measures itself for the pixel limits and the drag. A
        // length change re-renders once, so pixel limits follow a resize.
        let probe = group_bounds.clone();
        let measure = move |bounds: Bounds<Pixels>, window: &mut Window, _: &mut App| {
            let changed = probe.get().map(|b| b.size) != Some(bounds.size);
            probe.set(Some(bounds));
            if changed && has_pixel_limits {
                window.request_animation_frame();
            }
        };
        // The drag outlives the handle's hitbox, so paint-time window
        // listeners own the move and the release until it ends.
        if self.is_disabled || count <= 1 {
            root = root.child(gpui::canvas(measure, |_, _, _, _| {}).absolute().inset_0());
        } else {
            let move_drag = drag.clone();
            let up_drag = drag;
            let up_state = state.clone();
            let move_state = state;
            root = root.child(
                gpui::canvas(measure, move |_, _, window, _| {
                    let bounds = group_bounds.clone();
                    let held = move_drag.clone();
                    let state = move_state.clone();
                    window.on_mouse_event(
                        move |event: &gpui::MouseMoveEvent, phase, window, cx| {
                            if phase != gpui::DispatchPhase::Capture {
                                return;
                            }
                            let Some(drag) = held.read(cx).clone() else {
                                return;
                            };
                            if event.pressed_button != Some(MouseButton::Left) {
                                // The release happened where no listener
                                // saw it (outside the window).
                                held.update(cx, |value, cx| {
                                    *value = None;
                                    cx.notify();
                                });
                                return;
                            }
                            // The panels changed under the drag (see
                            // render): a handle that no longer exists
                            // ends it.
                            if drag.handle + 1 >= state.limits.len()
                                || drag.start.len() != state.limits.len()
                            {
                                held.update(cx, |value, cx| {
                                    *value = None;
                                    cx.notify();
                                });
                                return;
                            }
                            let Some(bounds) = bounds.get() else {
                                return;
                            };
                            let length = f32::from(if horizontal {
                                bounds.size.width
                            } else {
                                bounds.size.height
                            });
                            let available = length - handle_total;
                            if available <= 0. {
                                return;
                            }
                            let delta = (axis(event.position) - drag.origin) / available * 100.;
                            let target = drag.start[drag.handle] + delta;
                            let next = resize_pair(&drag.start, drag.handle, target, &state.limits);
                            // Compare with what is current now, not with
                            // what the last frame rendered: several moves
                            // can land between two frames. The store is
                            // current for an uncontrolled group; a
                            // controlled one has only what it reported.
                            let now = if state.controlled {
                                drag.last
                            } else {
                                state.store.read(cx).clone()
                            };
                            if state.apply(&now, next.clone(), window, cx) {
                                held.update(cx, |value, _| {
                                    if let Some(value) = value {
                                        value.last = next;
                                    }
                                });
                            }
                        },
                    );
                    let held = up_drag.clone();
                    let state = up_state.clone();
                    window.on_mouse_event(move |event: &gpui::MouseUpEvent, phase, window, cx| {
                        if phase != gpui::DispatchPhase::Capture
                            || event.button != MouseButton::Left
                        {
                            return;
                        }
                        let Some(drag) = held.read(cx).clone() else {
                            return;
                        };
                        held.update(cx, |value, cx| {
                            *value = None;
                            cx.notify();
                        });
                        if drag.last != drag.start {
                            state.resize_ended(&drag.last, window, cx);
                        }
                    });
                })
                .absolute()
                .inset_0(),
            );
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(default: Option<f32>) -> ResizablePanel {
        let p = ResizablePanel::new();
        match default {
            Some(size) => p.default_size(size),
            None => p,
        }
    }

    #[test]
    fn defaults_share_the_remainder_and_scale_to_100() {
        let defaults = |panels: &[ResizablePanel]| {
            let limits: Vec<Limit> = panels.iter().map(|p| p.limits(None)).collect();
            default_sizes(panels, &limits)
        };
        assert_eq!(
            defaults(&[panel(Some(20.)), panel(None), panel(None)]),
            vec![20., 40., 40.]
        );
        assert_eq!(
            defaults(&[panel(Some(30.)), panel(Some(10.))]),
            vec![75., 25.]
        );
        assert_eq!(defaults(&[panel(None), panel(None)]), vec![50., 50.]);
    }

    #[test]
    fn a_pair_resize_respects_both_panels_limits() {
        let limits = [
            Limit::range(10., 60.),
            Limit::range(0., 100.),
            Limit::range(0., 100.),
        ];
        let sizes = [40., 40., 20.];
        // The first panel's own max.
        assert_eq!(resize_pair(&sizes, 0, 70., &limits), vec![60., 20., 20.]);
        // The second panel's min caps the first at 80 - 30 = 50, under its
        // own max.
        let tight = [
            Limit::range(10., 90.),
            Limit::range(30., 100.),
            Limit::range(0., 100.),
        ];
        assert_eq!(resize_pair(&sizes, 0, 70., &tight), vec![50., 30., 20.]);
        // The first panel's min.
        assert_eq!(resize_pair(&sizes, 0, -5., &limits), vec![10., 70., 20.]);
        // Only the pair moves.
        assert_eq!(resize_pair(&sizes, 1, 50., &limits), vec![40., 50., 10.]);
    }

    #[test]
    fn normalize_scales_splits_and_clamps() {
        let free = [Limit::range(0., 100.), Limit::range(0., 100.)];
        assert_eq!(normalize(&[1., 3.], &free), vec![25., 75.]);
        assert_eq!(normalize(&[0., 0.], &free), vec![50., 50.]);
        assert_eq!(normalize(&[f32::NAN, -5.], &free), vec![50., 50.]);
        assert_eq!(normalize(&[f32::INFINITY, 1.], &free), vec![0., 100.]);
        // Below a minimum: raised to it, the others give up the difference
        // in proportion to their room.
        let limits = [
            Limit::range(20., 100.),
            Limit::range(0., 100.),
            Limit::range(0., 100.),
        ];
        assert_eq!(normalize(&[10., 45., 45.], &limits), vec![20., 40., 40.]);
        // Over a maximum: the others take the excess.
        let capped = [Limit::range(0., 50.), Limit::range(0., 100.)];
        assert_eq!(normalize(&[80., 20.], &capped), vec![50., 50.]);
        // Unsatisfiable limits leave the scaled sizes alone.
        let impossible = [Limit::range(70., 100.), Limit::range(70., 100.)];
        assert_eq!(normalize(&[50., 50.], &impossible), vec![50., 50.]);
    }

    #[test]
    fn contradictory_limits_leave_the_sizes_alone() {
        let limits = [Limit::range(70., 100.), Limit::range(70., 100.)];
        assert_eq!(resize_pair(&[50., 50.], 0, 60., &limits), vec![50., 50.]);
        assert!(pair_range(&[50., 50.], 0, &limits).is_none());
    }

    fn collapsible(min: f32) -> Limit {
        Limit {
            min,
            max: 100.,
            collapsed: Some(0.),
        }
    }

    #[test]
    fn pixel_limits_convert_against_the_measured_length() {
        let panel = ResizablePanel::new()
            .min_size(10.)
            .min_size_px(px(100.))
            .max_size(90.)
            .max_size_px(px(300.));
        let percent = |limit: Limit| {
            assert_eq!(limit.collapsed, None);
            (
                (limit.min * 1000.).round() / 1000.,
                (limit.max * 1000.).round() / 1000.,
            )
        };
        // Unmeasured: the percentages alone.
        assert_eq!(percent(panel.limits(None)), (10., 90.));
        // 500px available: 100px is 20% (stricter than 10%), 300px is 60%.
        assert_eq!(percent(panel.limits(Some(500.))), (20., 60.));
        // 2000px available: the 10% minimum is stricter than 5%.
        assert_eq!(percent(panel.limits(Some(2000.))), (10., 15.));
        // A pixel maximum under the minimum yields to it.
        let squeezed = ResizablePanel::new().min_size(50.).max_size_px(px(10.));
        assert_eq!(percent(squeezed.limits(Some(100.))), (50., 50.));
    }

    #[test]
    fn a_collapsible_pair_snaps_at_halfway_to_the_minimum() {
        let limits = [collapsible(20.), Limit::range(0., 100.)];
        let sizes = [30., 70.];
        // Between the collapsed size and the minimum: the nearer end.
        assert_eq!(resize_pair(&sizes, 0, 11., &limits), vec![20., 80.]);
        assert_eq!(resize_pair(&sizes, 0, 9., &limits), vec![0., 100.]);
        assert_eq!(pair_range(&sizes, 0, &limits), Some((0., 100.)));
        // The panel after the handle collapses the same way.
        let after = [Limit::range(0., 100.), collapsible(20.)];
        assert_eq!(resize_pair(&sizes, 0, 95., &after), vec![100., 0.]);
        assert_eq!(resize_pair(&sizes, 0, 88., &after), vec![80., 20.]);
    }

    #[test]
    fn a_step_crosses_the_collapse_gap() {
        let limits = [collapsible(20.), Limit::range(0., 100.)];
        // From the minimum, a 5% step down would snap back: it collapses.
        assert_eq!(step_pair(&[20., 80.], 0, 15., &limits), vec![0., 100.]);
        // From collapsed, a 5% step up would snap back: it expands to the
        // minimum.
        assert_eq!(step_pair(&[0., 100.], 0, 5., &limits), vec![20., 80.]);
        // Ordinary steps are plain resizes.
        assert_eq!(step_pair(&[30., 70.], 0, 35., &limits), vec![35., 65.]);
    }

    #[test]
    fn enter_collapses_and_restores() {
        let limits = [collapsible(20.), Limit::range(0., 100.)];
        let restore = [Some(35.), None];
        assert_eq!(
            toggle_collapse(&[40., 60.], 0, &limits, &restore),
            Some(vec![0., 100.])
        );
        assert_eq!(
            toggle_collapse(&[0., 100.], 0, &limits, &restore),
            Some(vec![35., 65.])
        );
        // Started collapsed with nothing to restore: the minimum.
        assert_eq!(
            toggle_collapse(&[0., 100.], 0, &limits, &[None, None]),
            Some(vec![20., 80.])
        );
        // Only the panel after is collapsible: Enter collapses that one.
        let after = [Limit::range(0., 100.), collapsible(20.)];
        assert_eq!(
            toggle_collapse(&[40., 60.], 0, &after, &[None, None]),
            Some(vec![100., 0.])
        );
        let neither = [Limit::range(0., 100.), Limit::range(0., 100.)];
        assert_eq!(
            toggle_collapse(&[40., 60.], 0, &neither, &[None, None]),
            None
        );
    }

    #[test]
    fn normalize_keeps_a_collapsed_panel_collapsed() {
        let limits = [collapsible(20.), Limit::range(0., 100.)];
        assert_eq!(normalize(&[0., 100.], &limits), vec![0., 100.]);
        // Under halfway rounds down to collapsed, over it up to the minimum.
        assert_eq!(normalize(&[5., 95.], &limits), vec![0., 100.]);
        assert_eq!(normalize(&[15., 85.], &limits), vec![20., 80.]);
    }
}
