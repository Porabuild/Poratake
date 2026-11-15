//! `VirtualList` — a vertical list that renders only the rows in (and just
//! around) the viewport, with programmatic scrolling to a row (HeroGPUI
//! extension; HeroUI v3 has no such component). It has two modes, chosen by
//! the [`VirtualListHandle`] it is built over:
//!
//! - **Measured** ([`VirtualListHandle::new`]): **variable row heights**.
//!   It wraps GPUI's own `list` element, which measures each row the first
//!   time it is laid out and keeps a height summary, so rows need no declared
//!   height. `ListBox` and `Table` render their `estimated_row_height` bodies
//!   through it.
//! - **Uniform** ([`VirtualListHandle::uniform`]): every row is as tall as
//!   the first one, which is measured each frame and multiplied, so the
//!   geometry of a million rows costs one layout. It wraps GPUI's
//!   `uniform_list`, and it is what the fixed `row_height` paths of
//!   `ListBox`, `Table` and `ComboBox` render through: it centres a row
//!   ([`VirtualListScroll::Center`]), reports its laid-out viewport (which
//!   PageUp/PageDown step over by the declared row height) and how much
//!   content remains below it (which arms `Table`'s load-more).
//!
//! State lives in a caller-owned [`VirtualListHandle`], which is how a view
//! scrolls the list from outside (`scroll_to_item`) and tells it that the
//! collection changed (`set_item_count`, `splice`):
//!
//! ```
//! use herogpui_components::{VirtualListScroll, VirtualList, VirtualListHandle};
//! use gpui::{div, prelude::*, px};
//!
//! struct Log {
//!     rows: Vec<String>,
//!     list: VirtualListHandle,
//! }
//!
//! impl Log {
//!     fn new(rows: Vec<String>) -> Self {
//!         // `VirtualListHandle::uniform` when every row has one height.
//!         let list = VirtualListHandle::new(rows.len());
//!         Self { rows, list }
//!     }
//!
//!     fn jump_to_end(&self) {
//!         // Clamped to the last row; an empty list stays put.
//!         self.list.scroll_to_item(usize::MAX, VirtualListScroll::Reveal);
//!     }
//!
//!     fn view(&self) -> VirtualList {
//!         let rows = self.rows.clone();
//!         VirtualList::new("log", &self.list, move |ix, _window, _cx| {
//!             div().p(px(4.)).child(rows[ix].clone()).into_any_element()
//!         })
//!         .height(px(240.))
//!     }
//! }
//! ```

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    div, list, prelude::*, uniform_list, AnyElement, App, Bounds, ElementId, ListAlignment,
    ListOffset, ListSizingBehavior, ListState, Pixels, ScrollStrategy, UniformListScrollHandle,
    Window,
};

/// Where [`VirtualListHandle::scroll_to_item`] places the row.
///
/// Adding [`Center`](Self::Center) in 0.13 made this enum
/// `#[non_exhaustive]`; a `match` on it needs a wildcard arm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum VirtualListScroll {
    /// Scroll the least distance that makes the row fully visible; a row
    /// already fully visible does not move. In a measured list the distance
    /// needs the heights of the rows in between, so a row that was not laid
    /// out in the last frame (far outside the viewport, never measured) is
    /// brought to the top edge instead, as [`Top`](Self::Top) does.
    #[default]
    Reveal,
    /// Put the row's top edge at the top of the viewport (clamped, in a
    /// uniform list, so the last page stays full).
    Top,
    /// When the row is not fully visible, scroll so its centre sits at the
    /// viewport's centre, clamped at both ends of the content; a row already
    /// fully visible does not move. This is GPUI's
    /// `ScrollStrategy::Center`, which the keyboard cursor of the fixed-row
    /// collections follows. A measured list centres a row it laid out in the
    /// last frame and brings any other row to the top edge, as
    /// [`Top`](Self::Top) does.
    Center,
}

/// The scroll and measurement state of one [`VirtualList`]. Cheap to clone;
/// clones share the state. Keep one per list in the owning view.
#[derive(Clone)]
pub struct VirtualListHandle {
    engine: Engine,
}

#[derive(Clone)]
enum Engine {
    Measured(ListState),
    Uniform(Rc<UniformState>),
}

/// A uniform list's state: GPUI's scroll handle, the row count the next
/// frame renders, and the count the last frame rendered (the one its
/// measured content height is a multiple of).
struct UniformState {
    scroll: UniformListScrollHandle,
    count: Cell<usize>,
    rendered_count: Cell<usize>,
}

impl UniformState {
    /// The measured row height of the last frame.
    fn row_height(&self) -> Option<Pixels> {
        let size = self.scroll.0.borrow().last_item_size?;
        let rendered = self.rendered_count.get();
        (rendered > 0).then(|| size.contents.height / rendered as f32)
    }
}

impl std::fmt::Debug for VirtualListHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualListHandle")
            .field("uniform", &self.is_uniform())
            .field("item_count", &self.item_count())
            .finish()
    }
}

/// Rows rendered beyond each edge of the viewport, so a short scroll shows
/// already-laid-out rows instead of a blank band.
const OVERDRAW: f32 = 200.;

impl VirtualListHandle {
    /// A measured (variable-row-height) handle for a list of `item_count`
    /// rows, scrolled to the top.
    pub fn new(item_count: usize) -> Self {
        Self {
            engine: Engine::Measured(ListState::new(
                item_count,
                ListAlignment::Top,
                gpui::px(OVERDRAW),
            )),
        }
    }

    /// A uniform (fixed-row-height) handle for a list of `item_count` rows,
    /// scrolled to the top. Every row is laid out at the height of the
    /// first, so rows should be built at one explicit height.
    pub fn uniform(item_count: usize) -> Self {
        Self {
            engine: Engine::Uniform(Rc::new(UniformState {
                scroll: UniformListScrollHandle::new(),
                count: Cell::new(item_count),
                rendered_count: Cell::new(0),
            })),
        }
    }

    /// A handle with an explicit overdraw distance, for the components whose
    /// overdraw follows their row estimate.
    pub(crate) fn with_overdraw(item_count: usize, overdraw: Pixels) -> Self {
        Self {
            engine: Engine::Measured(ListState::new(item_count, ListAlignment::Top, overdraw)),
        }
    }

    /// Wraps a list state a component already holds.
    pub(crate) fn from_list_state(state: ListState) -> Self {
        Self {
            engine: Engine::Measured(state),
        }
    }

    /// The underlying GPUI list state of a measured handle, for the
    /// components that read its viewport and scroll top directly (paging,
    /// load-more, edge rounding).
    ///
    /// # Panics
    ///
    /// On a [uniform](Self::uniform) handle.
    pub(crate) fn list_state(&self) -> &ListState {
        match &self.engine {
            Engine::Measured(state) => state,
            Engine::Uniform(_) => panic!("a uniform VirtualListHandle has no ListState"),
        }
    }

    /// Whether this handle was made by [`uniform`](Self::uniform).
    pub fn is_uniform(&self) -> bool {
        matches!(self.engine, Engine::Uniform(_))
    }

    /// The number of rows the list renders.
    pub fn item_count(&self) -> usize {
        match &self.engine {
            Engine::Measured(state) => state.item_count(),
            Engine::Uniform(state) => state.count.get(),
        }
    }

    /// Replaces the collection: `item_count` rows, all remeasured, scrolled
    /// back to the top. For an append or a local edit, [`splice`](Self::splice)
    /// keeps the scroll position and the other rows' measurements.
    pub fn set_item_count(&self, item_count: usize) {
        match &self.engine {
            Engine::Measured(state) => state.reset(item_count),
            Engine::Uniform(state) => {
                state.count.set(item_count);
                let mut scroll = state.scroll.0.borrow_mut();
                scroll.deferred_scroll_to_item = None;
                scroll.base_handle.set_offset(gpui::point(px0(), px0()));
            }
        }
    }

    /// Replaces the rows in `old_range` with `count` new rows, keeping every
    /// other row's measured height and the scroll position (a uniform list
    /// clamps it when the content got shorter).
    pub fn splice(&self, old_range: std::ops::Range<usize>, count: usize) {
        match &self.engine {
            Engine::Measured(state) => state.splice(old_range, count),
            Engine::Uniform(state) => {
                let current = state.count.get();
                let end = old_range.end.min(current);
                let start = old_range.start.min(end);
                state.count.set(current - (end - start) + count);
            }
        }
    }

    /// Discards every measured height, for when row content changed size
    /// without the count changing (a font or density switch). A uniform list
    /// measures its first row every frame, so this does nothing there.
    pub fn remeasure(&self) {
        if let Engine::Measured(state) = &self.engine {
            state.remeasure();
        }
    }

    /// Scrolls so row `ix` is visible, per `strategy`. An index past the end
    /// scrolls to the end. Takes effect on the next frame.
    pub fn scroll_to_item(&self, ix: usize, strategy: VirtualListScroll) {
        let ix = ix.min(self.item_count().saturating_sub(1));
        match &self.engine {
            Engine::Uniform(state) => state.scroll.scroll_to_item(
                ix,
                match strategy {
                    VirtualListScroll::Reveal => ScrollStrategy::Nearest,
                    VirtualListScroll::Top => ScrollStrategy::Top,
                    VirtualListScroll::Center => ScrollStrategy::Center,
                },
            ),
            Engine::Measured(state) => match (strategy, state.bounds_for_item(ix)) {
                (VirtualListScroll::Reveal, Some(_)) => state.scroll_to_reveal_item(ix),
                (VirtualListScroll::Center, Some(row)) => {
                    let viewport = state.viewport_bounds();
                    let fully_visible =
                        row.top() >= viewport.top() && row.bottom() <= viewport.bottom();
                    if !fully_visible {
                        state.scroll_by(row.center().y - viewport.center().y);
                    }
                }
                _ => state.scroll_to(ListOffset {
                    item_ix: ix,
                    offset_in_item: px0(),
                }),
            },
        }
    }

    /// Scrolls by `distance` (positive moves the content up, towards later
    /// rows), clamped to the content.
    pub fn scroll_by(&self, distance: Pixels) {
        match &self.engine {
            Engine::Measured(state) => state.scroll_by(distance),
            Engine::Uniform(state) => {
                let scroll = state.scroll.0.borrow();
                let handle = &scroll.base_handle;
                let max = handle.max_offset().y.max(px0());
                let mut offset = handle.offset();
                offset.y = (offset.y - distance).clamp(-max, px0());
                handle.set_offset(offset);
            }
        }
    }

    /// The index of the first row at the top of the viewport.
    pub fn first_visible_item(&self) -> usize {
        match &self.engine {
            Engine::Measured(state) => state.logical_scroll_top().item_ix,
            Engine::Uniform(state) => {
                let Some(row_height) = state.row_height().filter(|h| *h > px0()) else {
                    return 0;
                };
                let offset = -state.scroll.0.borrow().base_handle.offset().y;
                ((offset / row_height).floor().max(0.) as usize)
                    .min(state.count.get().saturating_sub(1))
            }
        }
    }

    /// Row `ix`'s window-coordinate bounds from the last frame, when it was
    /// rendered.
    pub fn bounds_for_item(&self, ix: usize) -> Option<Bounds<Pixels>> {
        match &self.engine {
            Engine::Measured(state) => state.bounds_for_item(ix),
            Engine::Uniform(state) => {
                if ix >= state.rendered_count.get() {
                    return None;
                }
                let row_height = state.row_height().filter(|h| *h > px0())?;
                let scroll = state.scroll.0.borrow();
                let viewport = scroll.base_handle.bounds();
                let top = viewport.top() + scroll.base_handle.offset().y + row_height * ix as f32;
                let row = Bounds::new(
                    gpui::point(viewport.left(), top),
                    gpui::size(viewport.size.width, row_height),
                );
                (row.bottom() > viewport.top() && row.top() < viewport.bottom()).then_some(row)
            }
        }
    }

    /// The list's viewport in window coordinates, as laid out in the last
    /// frame (zero-sized before the first). This is what PageUp/PageDown
    /// step over: it follows a bounded parent or a resized window, where a
    /// configured height would not.
    pub fn viewport_bounds(&self) -> Bounds<Pixels> {
        match &self.engine {
            Engine::Measured(state) => state.viewport_bounds(),
            Engine::Uniform(state) => state.scroll.0.borrow().base_handle.bounds(),
        }
    }

    /// How much content lies below the viewport's bottom edge after the
    /// last frame, or `None` when that is not known. A uniform list knows it
    /// exactly once laid out: the row count times the measured row, less the
    /// scroll offset and the viewport. A measured list knows it only when
    /// its last row was laid out in the last frame, since the rows it has
    /// not built have no height yet.
    pub fn remaining_below(&self) -> Option<Pixels> {
        match &self.engine {
            Engine::Uniform(state) => {
                let scroll = state.scroll.0.borrow();
                let size = scroll.last_item_size?;
                Some(size.contents.height + scroll.base_handle.offset().y - size.item.height)
            }
            Engine::Measured(state) => {
                let last = state.item_count().checked_sub(1)?;
                let row = state.bounds_for_item(last)?;
                Some((row.bottom() - state.viewport_bounds().bottom()).max(px0()))
            }
        }
    }

    /// Whether the list is scrolled to its top (within half a pixel).
    pub fn is_scrolled_to_top(&self) -> bool {
        match &self.engine {
            Engine::Uniform(state) => {
                state.scroll.0.borrow().base_handle.offset().y >= gpui::px(-HALF_PX)
            }
            Engine::Measured(state) => {
                let top = state.logical_scroll_top();
                top.item_ix == 0 && top.offset_in_item <= gpui::px(HALF_PX)
            }
        }
    }

    /// Whether the list's last row ends inside the viewport. An empty list
    /// answers `true`. A nonempty measured list answers `false` until its
    /// last row has been laid out; uniform mode defaults to `true` before
    /// GPUI reports its scroll extent.
    pub fn is_scrolled_to_end(&self) -> bool {
        match &self.engine {
            Engine::Uniform(state) => state.scroll.is_scrolled_to_end().unwrap_or(true),
            Engine::Measured(state) => {
                let count = state.item_count();
                count == 0
                    || state.bounds_for_item(count - 1).is_some_and(|row| {
                        row.bottom() <= state.viewport_bounds().bottom() + gpui::px(HALF_PX)
                    })
            }
        }
    }
}

const HALF_PX: f32 = 0.5;

fn px0() -> Pixels {
    gpui::px(0.)
}

type RenderRow = dyn FnMut(usize, &mut Window, &mut App) -> AnyElement;

/// A virtualised vertical list, measured or uniform per its handle. See the
/// [module docs](self).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct VirtualList {
    id: ElementId,
    handle: VirtualListHandle,
    render_row: Box<RenderRow>,
    height: Option<Pixels>,
    debug_selector: Option<String>,
    padding: Option<(Pixels, Pixels, Pixels)>,
    restrict_scroll_to_axis: bool,
}

impl VirtualList {
    /// A list over `handle` whose row `ix` is `render_row(ix, ..)`. Only
    /// rows near the viewport are built, each frame, so `render_row` should
    /// be cheap and must not assume it sees every index.
    pub fn new(
        id: impl Into<ElementId>,
        handle: &VirtualListHandle,
        render_row: impl FnMut(usize, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            handle: handle.clone(),
            render_row: Box::new(render_row),
            height: None,
            debug_selector: None,
            padding: None,
            restrict_scroll_to_axis: false,
        }
    }

    /// A fixed viewport height. Without one a measured list fills its
    /// parent's height (`size_full`), which then needs a definite height
    /// itself, and a uniform list sizes to its rows (the row count times the
    /// measured row), capped by the space its parent offers. A uniform list
    /// with a height may still shrink below it as a flex item (`min_h_0`),
    /// so a bounded parent hands it its real viewport.
    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.height = Some(height.into());
        self
    }

    /// Padding inside the scroll viewport (`top`, left and right `x`,
    /// `bottom`). Rows scroll through it, and a row's outset focus ring can
    /// paint into it at the edges, where the viewport would clip it.
    pub(crate) fn padding(mut self, top: Pixels, x: Pixels, bottom: Pixels) -> Self {
        self.padding = Some((top, x, bottom));
        self
    }

    /// Keeps a horizontal wheel from scrolling a uniform list vertically:
    /// gpui's `uniform_list` otherwise reads a gesture with no vertical
    /// component as a vertical scroll. For rows that scroll horizontally
    /// themselves (a `Table` with frozen columns). A measured list only ever
    /// reads the vertical component.
    pub(crate) fn restrict_scroll_to_axis(mut self) -> Self {
        self.restrict_scroll_to_axis = true;
        self
    }

    /// The headless-test probe name for the uniform list's viewport bounds.
    pub(crate) fn debug_selector(mut self, selector: String) -> Self {
        self.debug_selector = Some(selector);
        self
    }
}

impl RenderOnce for VirtualList {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        match self.handle.engine {
            Engine::Measured(state) => {
                let rows = list(state, self.render_row).size_full();
                let root = div().id(self.id).w_full().flex().flex_col();
                let root = match self.padding {
                    Some((top, x, bottom)) => root.pt(top).px(x).pb(bottom),
                    None => root,
                };
                match self.height {
                    Some(height) => root.h(height),
                    None => root.size_full(),
                }
                .child(rows)
                .into_any_element()
            }
            Engine::Uniform(state) => {
                let count = state.count.get();
                state.rendered_count.set(count);
                let render_row = RefCell::new(self.render_row);
                let rows = uniform_list(self.id, count, move |range, window, cx| {
                    let mut render_row = render_row.borrow_mut();
                    range
                        .map(|ix| render_row(ix, window, cx))
                        .collect::<Vec<_>>()
                })
                .track_scroll(&state.scroll)
                .w_full();
                let mut rows = rows;
                if self.restrict_scroll_to_axis {
                    rows.style().restrict_scroll_to_axis = Some(true);
                }
                let rows = match self.height {
                    Some(height) => rows.h(height).min_h_0(),
                    None => rows.with_sizing_behavior(ListSizingBehavior::Infer),
                };
                let rows = match self.padding {
                    Some((top, x, bottom)) => rows.pt(top).px(x).pb(bottom),
                    None => rows,
                };
                match self.debug_selector {
                    Some(selector) => rows.debug_selector(move || selector),
                    None => rows,
                }
                .into_any_element()
            }
        }
    }
}
