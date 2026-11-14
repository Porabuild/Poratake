//! Popover — port of `@heroui/popover`.

use gpui::{
    point, prelude::*, px, AnyElement, App, Bounds, ClickEvent, Display, Element, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, Position, RenderOnce,
    SharedString, Size, StatefulInteractiveElement, Style, Styled, Window,
};
use herogpui_core::element_id;
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};

/// `placement` on `Popover.Content`.
///
/// Shares the one placement vocabulary with the pickers and dropdown.
pub use herogpui_core::Placement as PopoverPlacement;

type OnOpenChange = std::sync::Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy)]
enum PopoverSide {
    Top,
    Bottom,
    Left,
    Right,
}

impl PopoverSide {
    const ALL: [Self; 4] = [Self::Top, Self::Bottom, Self::Left, Self::Right];

    fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Top => 0,
            Self::Bottom => 1,
            Self::Left => 2,
            Self::Right => 3,
        }
    }

    fn arrow_rotation(self) -> f32 {
        match self {
            Self::Top => 0.,
            Self::Bottom => std::f32::consts::PI,
            Self::Left => -std::f32::consts::FRAC_PI_2,
            Self::Right => std::f32::consts::FRAC_PI_2,
        }
    }
}

struct PopoverPositioner {
    trigger: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    resolved: std::rc::Rc<std::cell::Cell<Option<PopoverResolved>>>,
    /// Optional physical placement feedback for components whose entry
    /// animation depends on the side the viewport resolver actually chose.
    /// The positioner owns the measurement; the composing component owns the
    /// keyed cell so the result survives the next render.
    resolved_placement: Option<ResolvedPlacementHandle>,
    placement: PopoverPlacement,
    offset: Pixels,
    should_flip: bool,
    has_arrow: bool,
    constrain_height: bool,
    /// Align a side panel's cross axis to the trigger's start edge instead of
    /// `placement`'s alignment.
    ///
    /// RAC opens each submenu with `placement: 'end top'`: beside the row and
    /// top-aligned. `Placement::Right` alone would centre the panel on the
    /// row, so the submenu positioner carries this explicit override. It only
    /// affects the `Left`/`Right` arms of [`PopoverPositioner::origin`].
    cross_start: bool,
    /// Size the panel from the trigger width instead of `MaxContent`.
    ///
    /// Field panels (Select) are `w_full`: against a `MaxContent` root width
    /// that resolves incorrectly, so both measurements use the measured
    /// trigger width. With the widths equal, start/center/end alignment
    /// coincide and only the flipped side differs.
    match_trigger_width: bool,
    children: Vec<AnyElement>,
}

#[derive(Clone, Copy)]
struct PopoverResolved {
    trigger: Bounds<Pixels>,
    panel: Bounds<Pixels>,
    side: PopoverSide,
}

/// A small cross-component feedback channel for the resolved physical side.
///
/// `PopoverPositioner` learns the side during prepaint, after the composing
/// component has already built its panel. Keeping the side in a keyed cell
/// lets the next render select the correct placement-relative entry offset
/// without exposing the positioner's private geometry type.
pub(crate) type ResolvedPlacementHandle = std::rc::Rc<std::cell::Cell<Option<PopoverPlacement>>>;

/// The v3 `Popover.Arrow` part.
///
/// Upstream v3 draws no arrow unless the part is composed into the panel's
/// children (`<Popover.Arrow />`). Without `.child(..)` the part renders v3's
/// built-in 12px curved arrow, rotated for the resolved side. With children,
/// upstream stamps the single composed element with
/// `data-slot="popover-overlay-arrow"`, and the `.popover` placement CSS
/// (`data-placement="bottom|left|right"`) rotates *that* element — so a custom
/// child is rotated too, not just the built-in curve.
///
/// This port is Partial there, deliberately: GPUI 0.2.2 transforms only `svg()`
/// elements (`with_transformation`), not arbitrary divs, and the resolved side
/// is known only at prepaint while an `Svg`'s transformation can only be set at
/// construction — so a caller-provided element (SVG included) takes the built-in
/// arrow's resolved position but no rotation. The port renders the first
/// composed child; additional children are ignored, where upstream composes
/// exactly one element and falls back to the default curve otherwise.
///
/// The part reads its placement from the popover that composes it, which is
/// discovered among the panel's direct children: a `PopoverArrow` nested inside
/// another element is never resolved and paints nothing, like one rendered
/// outside any popover.
pub struct PopoverArrow {
    resolved: std::rc::Rc<std::cell::Cell<Option<PopoverResolved>>>,
    children: Vec<AnyElement>,
}

impl Default for PopoverArrow {
    fn default() -> Self {
        Self::new()
    }
}

impl PopoverArrow {
    /// Creates an empty arrow.
    pub fn new() -> Self {
        Self {
            resolved: std::rc::Rc::new(std::cell::Cell::new(None)),
            children: Vec::new(),
        }
    }
}

impl ParentElement for PopoverArrow {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// Layout state of a composed `Popover.Arrow`. Public only because the
/// `Element` associated type leaks; nothing here is a public contract.
pub struct PopoverArrowState {
    leaves: Vec<AnyElement>,
    layouts: Vec<LayoutId>,
    custom: bool,
}

impl Element for PopoverArrow {
    type RequestLayoutState = PopoverArrowState;
    type PrepaintState = Option<usize>;

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let custom = !self.children.is_empty();
        let mut leaves = if custom {
            std::mem::take(&mut self.children)
        } else {
            let color = cx.colors().overlay.background;
            PopoverSide::ALL
                .map(|side| {
                    gpui::div()
                        .absolute()
                        .size(px(12.))
                        .debug_selector(|| "popover-arrow".to_owned())
                        .child(
                            gpui::svg()
                                .size(px(12.))
                                .path(crate::icons::TOOLTIP_ARROW)
                                .text_color(color)
                                .with_transformation(gpui::Transformation::rotate(gpui::radians(
                                    side.arrow_rotation(),
                                ))),
                        )
                        .into_any_element()
                })
                .into_iter()
                .collect()
        };
        let layouts = leaves
            .iter_mut()
            .map(|leaf| leaf.request_layout(window, cx))
            .collect::<Vec<_>>();
        let layout = window.request_layout(
            Style {
                position: Position::Absolute,
                display: Display::Flex,
                ..Style::default()
            },
            layouts.iter().copied(),
            cx,
        );
        (
            layout,
            PopoverArrowState {
                leaves,
                layouts,
                custom,
            },
        )
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let resolved = self.resolved.get()?;
        let index = if state.custom {
            0
        } else {
            resolved.side.index()
        };
        let bounds = window.layout_bounds(state.layouts[index]);
        let offset =
            PopoverPositioner::arrow_origin(resolved.side, resolved.trigger, resolved.panel)
                - bounds.origin;
        let offset = point(offset.x.round(), offset.y.round());
        window.with_element_offset(offset, |window| {
            state.leaves[index].prepaint(window, cx);
        });
        Some(index)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        selected: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(index) = *selected {
            state.leaves[index].paint(window, cx);
        }
    }
}

impl IntoElement for PopoverArrow {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

pub(crate) struct PopoverTriggerMeasure {
    child: AnyElement,
    bounds: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
}

impl PopoverTriggerMeasure {
    pub(crate) fn new(
        child: impl IntoElement,
        bounds: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    ) -> Self {
        Self {
            child: child.into_any_element(),
            bounds,
        }
    }
}

impl Element for PopoverTriggerMeasure {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let next = Some(bounds);
        if self.bounds.get() != next {
            self.bounds.set(next);
        }
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

impl IntoElement for PopoverTriggerMeasure {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl PopoverPositioner {
    fn new(
        trigger: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
        resolved: std::rc::Rc<std::cell::Cell<Option<PopoverResolved>>>,
        placement: PopoverPlacement,
        offset: Pixels,
        should_flip: bool,
        has_arrow: bool,
    ) -> Self {
        Self {
            trigger,
            resolved,
            placement,
            offset,
            should_flip,
            has_arrow,
            constrain_height: false,
            cross_start: false,
            match_trigger_width: false,
            resolved_placement: None,
            children: Vec::new(),
        }
    }

    fn with_resolved_placement(mut self, handle: ResolvedPlacementHandle) -> Self {
        self.resolved_placement = Some(handle);
        self
    }

    fn preferred_side(&self) -> PopoverSide {
        let placement = self.placement;
        if placement.is_side() {
            if placement.is_start_side() {
                PopoverSide::Left
            } else {
                PopoverSide::Right
            }
        } else if placement.is_above() {
            PopoverSide::Top
        } else {
            PopoverSide::Bottom
        }
    }

    fn available(
        &self,
        side: PopoverSide,
        trigger: Bounds<Pixels>,
        viewport: Size<Pixels>,
    ) -> Pixels {
        let available = match side {
            PopoverSide::Top => trigger.top(),
            PopoverSide::Bottom => viewport.height - trigger.bottom(),
            PopoverSide::Left => trigger.left(),
            PopoverSide::Right => viewport.width - trigger.right(),
        };
        available
            - if self.constrain_height {
                px(12.)
            } else {
                px(0.)
            }
    }

    fn resolved_side(
        &self,
        trigger: Bounds<Pixels>,
        popup: Size<Pixels>,
        viewport: Size<Pixels>,
    ) -> PopoverSide {
        let preferred = self.preferred_side();
        if !self.should_flip {
            return preferred;
        }
        let opposite = preferred.opposite();
        let arrow = if self.has_arrow { px(12.) } else { px(0.) };
        let extent = match preferred {
            PopoverSide::Top | PopoverSide::Bottom => popup.height + self.offset,
            PopoverSide::Left | PopoverSide::Right => popup.width + self.offset,
        } + arrow;
        if extent <= self.available(preferred, trigger, viewport) {
            preferred
        } else if extent <= self.available(opposite, trigger, viewport) {
            opposite
        } else if self.available(preferred, trigger, viewport)
            >= self.available(opposite, trigger, viewport)
        {
            preferred
        } else {
            opposite
        }
    }

    fn origin(
        &self,
        side: PopoverSide,
        trigger: Bounds<Pixels>,
        popup: Size<Pixels>,
        viewport: Size<Pixels>,
    ) -> gpui::Point<Pixels> {
        use herogpui_core::PlacementAlign;

        let gap = self.offset + if self.has_arrow { px(12.) } else { px(0.) };
        let align = self.placement.align();
        let aligned_x = match align {
            PlacementAlign::Start => trigger.left(),
            PlacementAlign::Center => trigger.center().x - px(f32::from(popup.width) / 2.0),
            PlacementAlign::End => trigger.right() - popup.width,
        };
        let aligned_y = if self.cross_start {
            trigger.top()
        } else {
            match align {
                PlacementAlign::Start => trigger.top(),
                PlacementAlign::Center => trigger.center().y - px(f32::from(popup.height) / 2.0),
                PlacementAlign::End => trigger.bottom() - popup.height,
            }
        };
        let mut origin = match side {
            PopoverSide::Top => point(aligned_x, trigger.top() - popup.height - gap),
            PopoverSide::Bottom => point(aligned_x, trigger.bottom() + gap),
            PopoverSide::Left => point(trigger.left() - popup.width - gap, aligned_y),
            PopoverSide::Right => point(trigger.right() + gap, aligned_y),
        };

        let inset = if self.constrain_height {
            px(12.)
        } else {
            px(0.)
        };
        let max_x = (viewport.width - popup.width - inset).max(inset);
        let max_y = (viewport.height - popup.height - inset).max(inset);
        if matches!(side, PopoverSide::Top | PopoverSide::Bottom) {
            origin.x = origin.x.max(inset).min(max_x);
            if self.should_flip && !self.constrain_height {
                origin.y = origin.y.max(inset).min(max_y);
            }
        } else {
            origin.y = origin.y.max(inset).min(max_y);
            if self.should_flip && !self.constrain_height {
                origin.x = origin.x.max(inset).min(max_x);
            }
        }
        origin
    }

    fn arrow_origin(
        side: PopoverSide,
        trigger: Bounds<Pixels>,
        panel: Bounds<Pixels>,
    ) -> gpui::Point<Pixels> {
        let size = px(12.);
        let max_x = (panel.right() - size).max(panel.left());
        let max_y = (panel.bottom() - size).max(panel.top());
        match side {
            PopoverSide::Top => point(
                (trigger.center().x - size / 2.)
                    .max(panel.left())
                    .min(max_x),
                panel.bottom(),
            ),
            PopoverSide::Bottom => point(
                (trigger.center().x - size / 2.)
                    .max(panel.left())
                    .min(max_x),
                panel.top() - size,
            ),
            PopoverSide::Left => point(
                panel.right(),
                (trigger.center().y - size / 2.).max(panel.top()).min(max_y),
            ),
            PopoverSide::Right => point(
                panel.left() - size,
                (trigger.center().y - size / 2.).max(panel.top()).min(max_y),
            ),
        }
    }
}

impl ParentElement for PopoverPositioner {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

struct PopoverPositionerState {
    children: Vec<LayoutId>,
}

impl Element for PopoverPositioner {
    type RequestLayoutState = PopoverPositionerState;
    type PrepaintState = bool;

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let children = self
            .children
            .iter_mut()
            .map(|child| child.request_layout(window, cx))
            .collect::<Vec<_>>();
        let layout = window.request_layout(
            Style {
                position: Position::Absolute,
                display: Display::Flex,
                ..Style::default()
            },
            children.iter().copied(),
            cx,
        );
        (layout, PopoverPositionerState { children })
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        if state.children.is_empty() {
            return false;
        }
        let Some(trigger) = self.trigger.get() else {
            return false;
        };
        let viewport = window.viewport_size();
        // A trigger-width panel resolves `w_full` against this width, not
        // against `MaxContent`.
        let width_space = if self.match_trigger_width {
            gpui::AvailableSpace::Definite(trigger.size.width)
        } else {
            gpui::AvailableSpace::MaxContent
        };
        let mut popup = window.layout_bounds(state.children[0]).size;
        if self.constrain_height {
            popup = self.children[0].layout_as_root(
                gpui::size(width_space, gpui::AvailableSpace::MaxContent),
                window,
                cx,
            );
        }
        let side = self.resolved_side(trigger, popup, viewport);
        if let Some(handle) = &self.resolved_placement {
            let resolved = match side {
                PopoverSide::Top => PopoverPlacement::Top,
                PopoverSide::Bottom => PopoverPlacement::Bottom,
                PopoverSide::Left => PopoverPlacement::Left,
                PopoverSide::Right => PopoverPlacement::Right,
            };
            if handle.get() != Some(resolved) {
                handle.set(Some(resolved));
                // The first prepaint discovers a flip only after the panel
                // has been built. Re-render once so an entry animation can
                // use the resolved physical side on its next frame.
                window.defer(cx, |window, _| window.refresh());
            }
        }
        if self.constrain_height {
            let max_height = match side {
                PopoverSide::Top | PopoverSide::Bottom => {
                    self.available(side, trigger, viewport) - self.offset
                }
                PopoverSide::Left | PopoverSide::Right => {
                    viewport.height - self.origin(side, trigger, popup, viewport).y - px(12.)
                }
            }
            .max(px(0.));
            popup = self.children[0].layout_as_root(
                gpui::size(width_space, gpui::AvailableSpace::Definite(max_height)),
                window,
                cx,
            );
        }
        let origin = self.origin(side, trigger, popup, viewport);
        self.resolved.set(Some(PopoverResolved {
            trigger,
            panel: Bounds {
                origin,
                size: popup,
            },
            side,
        }));
        let layout_origin = if self.constrain_height {
            window.layout_bounds(state.children[0]).origin
        } else {
            bounds.origin
        };
        let offset = origin - layout_origin;
        let offset = point(offset.x.round(), offset.y.round());
        window.with_element_offset(offset, |window| {
            self.children[0].prepaint(window, cx);
        });

        true
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepainted: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !*prepainted {
            return;
        }
        self.children[0].paint(window, cx);
    }
}

/// A field popover's placement feedback, held in keyed state under `base`:
/// the placement last requested and the physical side the positioner
/// resolved during the last prepaint. A changed request forgets the resolved
/// side. Returns the handle the positioner writes and the side the next
/// entry motion starts from, so a viewport flip animates from the side that
/// was actually painted.
pub(crate) fn field_placement_feedback(
    window: &mut Window,
    cx: &mut App,
    base: &gpui::ElementId,
    placement: PopoverPlacement,
) -> (ResolvedPlacementHandle, PopoverPlacement) {
    let requested_placement = window.use_keyed_state(
        element_id::scoped(base, "requested-placement"),
        cx,
        |_, _| placement,
    );
    let resolved_placement = window.use_keyed_state(
        element_id::scoped(base, "resolved-placement"),
        cx,
        |_, _| std::rc::Rc::new(std::cell::Cell::new(None::<PopoverPlacement>)),
    );
    if *requested_placement.read(cx) != placement {
        requested_placement.update(cx, |requested, _| *requested = placement);
        resolved_placement.read(cx).set(None);
    }
    let resolved_placement = resolved_placement.read(cx).clone();
    let entry_placement = resolved_placement.get().unwrap_or(placement);
    (resolved_placement, entry_placement)
}

/// HeroUI's placement-specific `slide-in-from-*` entry offsets for popovers.
/// The positioner feeds the resolved physical side back into the composing
/// component, so a viewport flip animates from the side actually painted.
///
/// The offset follows the physical side only: aligned and logical spellings
/// share the motion of the centered form they hang beside, exactly as the
/// pinned `data-placement` CSS keys animation to the four sides.
pub(crate) fn placement_entry_offset(placement: PopoverPlacement) -> (f32, f32) {
    if placement.is_above() {
        (0.0, 4.0)
    } else if placement.is_side() {
        if placement.is_start_side() {
            (4.0, 0.0)
        } else {
            (-4.0, 0.0)
        }
    } else {
        (0.0, -4.0)
    }
}

impl IntoElement for PopoverPositioner {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// The panel must use `max_h_full()` and own its vertical scroll container.
pub(crate) fn scrollable_popover(
    trigger: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    placement: PopoverPlacement,
    panel: impl IntoElement,
) -> impl IntoElement {
    scrollable_popover_with_resolved_placement(trigger, placement, None, panel)
}

/// Scrollable content-sized positioner for composed surfaces that need the
/// default viewport flip and a viewport-bounded panel. DatePicker and
/// DateRangePicker use this path so their calendar panels share the same
/// trigger measurement, resolved-side feedback and short-viewport scrolling as
/// the other picker overlays.
pub(crate) fn popover_with_resolved_placement(
    trigger: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    placement: PopoverPlacement,
    offset: Pixels,
    resolved_placement: Option<ResolvedPlacementHandle>,
    panel: impl IntoElement,
) -> impl IntoElement {
    let mut positioner = PopoverPositioner::new(
        trigger,
        std::rc::Rc::new(std::cell::Cell::new(None)),
        placement,
        offset,
        true,
        false,
    );
    // The picker panel owns the scroll container. The positioner supplies the
    // available height from the resolved physical side, then lays the panel
    // out again with that definite cap so the calendar can scroll instead of
    // painting outside the viewport.
    positioner.constrain_height = true;
    if let Some(handle) = resolved_placement {
        positioner = positioner.with_resolved_placement(handle);
    }
    positioner.child(panel)
}

/// The ColorPicker variant of [`scrollable_popover`] that feeds the resolved
/// physical side back to its placement-aware entry animation.
pub(crate) fn scrollable_popover_with_resolved_placement(
    trigger: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    placement: PopoverPlacement,
    resolved_placement: Option<ResolvedPlacementHandle>,
    panel: impl IntoElement,
) -> impl IntoElement {
    let mut positioner = PopoverPositioner::new(
        trigger,
        std::rc::Rc::new(std::cell::Cell::new(None)),
        placement,
        px(8.),
        true,
        false,
    );
    positioner.constrain_height = true;
    if let Some(handle) = resolved_placement {
        positioner = positioner.with_resolved_placement(handle);
    }
    positioner.child(panel)
}

/// Trigger-width field panel that feeds the resolved physical side back to a
/// picker entry animation. Select, Autocomplete and ComboBox share this path;
/// keeping the feedback here prevents their flip logic from drifting.
pub(crate) fn scrollable_field_popover_with_resolved_placement(
    trigger: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    placement: PopoverPlacement,
    resolved_placement: Option<ResolvedPlacementHandle>,
    panel: impl IntoElement,
) -> impl IntoElement {
    let mut positioner = PopoverPositioner::new(
        trigger,
        std::rc::Rc::new(std::cell::Cell::new(None)),
        placement,
        px(8.),
        true,
        false,
    );
    positioner.constrain_height = true;
    positioner.match_trigger_width = true;
    if let Some(handle) = resolved_placement {
        positioner = positioner.with_resolved_placement(handle);
    }
    positioner.child(panel)
}

/// A submenu variant of [`scrollable_popover`].
///
/// RAC renders each submenu in its own `Popover` against the parent row with
/// `placement: 'end top'` (pinned RAC 1.20.0 `Menu.js`): beside the row,
/// top-aligned, with the default 8px gap, flipping to the other side when it
/// has more room and capping at the available viewport height past the 12px
/// container padding. The panel must use `max_h_full()` and own its vertical
/// scroll container, as with [`scrollable_popover`].
pub(crate) fn scrollable_submenu_popover(
    trigger: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    panel: impl IntoElement,
) -> impl IntoElement {
    let mut positioner = PopoverPositioner::new(
        trigger,
        std::rc::Rc::new(std::cell::Cell::new(None)),
        PopoverPlacement::Right,
        px(8.),
        true,
        false,
    );
    positioner.constrain_height = true;
    positioner.cross_start = true;
    positioner.child(panel)
}

/// HeroUI Popover (controlled).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Popover {
    /// Distinguishes this popover's uncontrolled state from its neighbours'.
    id: gpui::ElementId,
    trigger: AnyElement,
    /// `isOpen` — `None` leaves the component holding the state, seeded
    /// from `defaultOpen`.
    is_open: Option<bool>,
    default_open: bool,
    placement: PopoverPlacement,
    title: Option<SharedString>,
    aria_label: Option<SharedString>,
    panel_width: Option<Pixels>,
    show_close_button: bool,
    offset: Pixels,
    should_flip: bool,
    on_open_change: Option<OnOpenChange>,
    children: Vec<AnyElement>,
    /// Both panel padding axes; unset keeps v3's 16px `p-4` insets.
    padding: Option<Pixels>,
    /// The panel's corner radius, in place of the owning `container_radius`
    /// helper.
    radius: Option<Pixels>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Popover {
    /// Creates a popover around the given trigger.
    pub fn new(trigger: impl IntoElement) -> Self {
        Self {
            id: gpui::ElementId::Name("popover".into()),
            trigger: trigger.into_any_element(),
            is_open: None,
            default_open: false,
            placement: PopoverPlacement::Bottom,
            offset: px(8.),
            should_flip: true,
            title: None,
            aria_label: None,
            panel_width: None,
            show_close_button: false,
            on_open_change: None,
            children: Vec::new(),
            padding: None,
            radius: None,
            sx: None,
        }
    }

    /// Distinguishes this popover from its neighbours.
    ///
    /// Only matters in the uncontrolled mode, where the open flag lives in
    /// element state: two popovers sharing a key would open together.
    pub fn id(mut self, id: impl Into<gpui::ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Sets whether the popover is open, making it controlled.
    pub fn is_open(mut self, v: bool) -> Self {
        self.is_open = Some(v);
        self
    }

    /// `defaultOpen` — the uncontrolled initial state.
    ///
    /// Only consulted when `is_open` is not supplied; the component then owns
    /// the flag and the trigger toggles it.
    pub fn default_open(mut self, v: bool) -> Self {
        self.default_open = v;
        self
    }

    /// `offset` — distance from the trigger, 8px in v3.
    pub fn offset(mut self, offset: impl Into<Pixels>) -> Self {
        self.offset = offset.into();
        self
    }

    /// `shouldFlip` — lets the panel reposition to stay inside the window.
    pub fn should_flip(mut self, v: bool) -> Self {
        self.should_flip = v;
        self
    }

    /// Sets both panel padding axes, feeding the resting panel and its entry
    /// animation the same value. Unset keeps v3's 16px `p-4` insets.
    pub fn padding(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding = Some(padding.into());
        self
    }

    /// The panel's corner radius, in place of the owning `container_radius`
    /// helper. The panel's entry zoom interpolates the same value, so both
    /// follow the override. Not a v3 prop; the removed v2 `radius` prop is
    /// prohibited and this is a per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    #[doc = "Sets the detached dialog width without sizing the trigger."]
    pub fn panel_width(mut self, width: impl Into<Pixels>) -> Self {
        self.panel_width = Some(width.into());
        self
    }

    #[doc = "Names the dialog without adding a visible heading."]
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Sets where the popover sits relative to its trigger.
    pub fn placement(mut self, p: PopoverPlacement) -> Self {
        self.placement = p;
        self
    }

    /// Optional bold header inside the panel.
    pub fn title(mut self, t: impl Into<SharedString>) -> Self {
        self.title = Some(t.into());
        self
    }

    /// Sets whether the popover shows a close button.
    pub fn show_close_button(mut self, v: bool) -> Self {
        self.show_close_button = v;
        self
    }

    /// Toggle handler wired to the trigger click.
    pub fn on_open_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_change = Some(std::sync::Arc::new(f));
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the popover's root element — the wrapper the trigger and the
    /// floating panel sit in — after every value the placement and the active
    /// theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl ParentElement for Popover {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Popover {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (is_open, open_own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "open"),
            self.is_open,
            self.default_open,
        );
        // v3 keeps a closing panel on screen for its `[data-exiting]` run.
        // `overlay_phase` takes `cx` mutably too, so it goes here.
        let phase_key = element_id::scoped(&self.id, "popover-phase");
        let (phase, dismissal_token) =
            crate::util::overlay_scope(window, cx, phase_key, is_open, true);
        let exiting = phase == crate::util::OverlayPhase::Exiting;
        // Positioning resolves flips during prepaint, after this render has
        // already chosen the panel's entry direction. Keep the requested
        // placement and the resolved physical side in keyed state so a flip,
        // resize, or initially-open mount can feed the next entry frame.
        let requested_placement = window.use_keyed_state(
            element_id::scoped(&self.id, "requested-placement"),
            cx,
            |_, _| self.placement,
        );
        let resolved_placement = window.use_keyed_state(
            element_id::scoped(&self.id, "resolved-placement"),
            cx,
            |_, _| std::rc::Rc::new(std::cell::Cell::new(None::<PopoverPlacement>)),
        );
        if *requested_placement.read(cx) != self.placement {
            requested_placement.update(cx, |placement, _| *placement = self.placement);
            resolved_placement.read(cx).set(None);
        }
        let resolved_placement = resolved_placement.read(cx).clone();
        let entry_placement = resolved_placement.get().unwrap_or(self.placement);
        // Escape is read on the root, and a key event only reaches an element
        // that is on the focused element's path -- so an open panel needs
        // *something* inside this root to hold the focus. A click on the
        // trigger does that by itself, which is why the keyboard appeared to
        // work; a caller that drives `isOpen` focuses nothing, and Escape went
        // to the app root instead. The root handle is how the component asks
        // whether anything inside it already has the focus, and it is
        // deliberately not a tab stop: the popover adds no stop of its own.
        // Both `use_keyed_state` calls take `cx` mutably, so they precede the
        // theme tokens.
        let anchor_bounds = window
            .use_keyed_state(element_id::scoped(&self.id, "anchor-bounds"), cx, |_, _| {
                std::rc::Rc::new(std::cell::Cell::new(None::<Bounds<Pixels>>))
            })
            .read(cx)
            .clone();
        let resolved = std::rc::Rc::new(std::cell::Cell::new(None::<PopoverResolved>));
        let root_focus = window
            .use_keyed_state(element_id::scoped(&self.id, "root-focus"), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        // A v3 popover is a dialog focus scope. It claims focus on every open
        // transition, contains Tab inside the panel, and restores the handle
        // that opened it when it closes.
        let claim = is_open;
        let trigger_focus = crate::util::panel_restore_focus(window, cx, &self.id);
        let panel_focus = crate::util::panel_focus(window, cx, &self.id, claim);
        // The panel's outside-press capture runs on mouse-down, before the
        // trigger's click listener runs on mouse-up. Keep this latch for one
        // dispatch so an open trigger is owned by its own toggle, not by both
        // dismissal paths.
        let trigger_pressed = std::rc::Rc::new(std::cell::Cell::new(false));
        let colors = cx.colors();
        let layout = cx.layout();

        let mut trigger_wrap = gpui::div()
            .id(element_id::scoped(&self.id, "trigger"))
            .flex()
            .track_focus(&trigger_focus)
            .cursor(crate::util::interactive_cursor(cx));
        if self.on_open_change.is_some() || open_own.is_some() {
            let on_open_change = self.on_open_change.clone();
            let own = open_own.clone();
            let open = is_open;
            let capture_pressed = trigger_pressed.clone();
            let click_pressed = trigger_pressed.clone();
            let toggle = crate::util::shared(move |window: &mut Window, cx: &mut App| {
                if let Some(held) = &own {
                    held.update(cx, |value, cx| {
                        *value = !open;
                        cx.notify();
                    });
                }
                if let Some(cb) = &on_open_change {
                    cb(&!open, window, cx);
                }
            });
            trigger_wrap = trigger_wrap
                .capture_any_mouse_down(move |_, _, cx| {
                    capture_pressed.set(true);
                    let clear = capture_pressed.clone();
                    cx.defer(move |_| clear.set(false));
                })
                .on_key_down({
                    let toggle = toggle.clone();
                    move |event, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space")
                            && trigger_focus.contains_focused(window, cx)
                        {
                            // Moving focus between key-down and key-up cancels
                            // GPUI's keyboard click. Activate without transferring it.
                            if !event.is_held {
                                toggle(window, cx);
                            }
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    }
                })
                .on_click(move |_: &ClickEvent, window, cx| {
                    toggle(window, cx);
                    click_pressed.set(false);
                });
        }

        let trigger =
            PopoverTriggerMeasure::new(trigger_wrap.child(self.trigger), anchor_bounds.clone());
        let mut root = gpui::div()
            .track_focus(&root_focus)
            .when(is_open, |root| root.key_context("Popover"))
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .child(trigger);

        if phase == crate::util::OverlayPhase::Closed {
            return crate::util::apply_sx(root, &self.sx);
        }

        let close = crate::util::shared({
            let own = open_own;
            let cb = self.on_open_change.clone();
            move |window: &mut Window, cx: &mut App| -> crate::util::DismissResult {
                if let Some(held) = &own {
                    held.update(cx, |v, cx| {
                        *v = false;
                        cx.notify();
                    });
                }
                if let Some(cb) = &cb {
                    cb(&false, window, cx);
                }
                crate::util::DismissResult::Handled
            }
        });

        // Panel
        // `.popover__heading` is the title beside the close button.
        let mut header_row = gpui::div().flex().items_center().justify_between();
        if let Some(title) = &self.title {
            header_row = header_row.child(
                gpui::div()
                    .text_size(px(14.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.foreground)
                    .child(title.to_string()),
            );
        } else {
            header_row = header_row.child("");
        }
        if self.show_close_button {
            let close_button = close.clone();
            header_row = header_row.child(
                crate::close_button::CloseButton::new(element_id::scoped(&self.id, "close"))
                    .on_press(move |_, window, cx| {
                        close_button(window, cx);
                    }),
            );
        }

        // The panel's entry zoom interpolates the panel's own radius, so one
        // binding feeds both the painted shape and the animation. Read off
        // `self` before the children are moved into the panel.
        let radius = self
            .radius
            .unwrap_or_else(|| crate::util::container_radius(cx));
        // The padding pair is read off `self` here for the same reason: the
        // resting panel's chain consumes it and so does the entry zoom below.
        // Under reduced motion `entering_zoom` returns the element untouched,
        // so the chain is then the only consumer — the override still reaches
        // the resting panel on every motion path. The defaults are v3's
        // `.popover__dialog` `p-4` on both axes either way.
        let panel_padding_y = self.padding.unwrap_or(px(16.));
        let panel_padding_x = self.padding.unwrap_or(px(16.));
        let panel_width = self
            .panel_width
            .unwrap_or(px(260.))
            .min(window.viewport_size().width);
        let mut panel = gpui::div()
            // `popover/popover.js` composes RAC `Popover` around a `Dialog`,
            // and `react-aria/dist/private/dialog/useDialog.js` is what gives
            // that dialog `role="dialog"` — the RAC `Popover` wrapper itself
            // reports nothing. `useDialog` names it from the composed
            // `Heading` through `aria-labelledby`, which this port inlines as
            // the title text; a popover with no title warns upstream ("A
            // dialog must have a title for accessibility") and is unnamed here
            // for the same reason.
            .id(element_id::scoped(&self.id, "dialog"))
            .a11y_named(
                a11y::Role::Dialog,
                &a11y::Name::maybe(self.aria_label.clone().or(self.title.clone())),
            )
            .w(panel_width)
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(panel_padding_x)
            .py(panel_padding_y)
            .bg(colors.overlay.background)
            .text_color(colors.surface.foreground)
            // `.popover` is `text-sm`.
            .text_size(px(14.))
            .line_height(px(20.))
            .rounded(radius)
            // v3 gives a floating panel no border: `.popover` and friends are
            // `bg-overlay shadow-overlay` and a radius, and dark mode's
            // inset hairline is what separates the panel from the page.
            .when_some(layout.overlay_hairline, |el, hairline| {
            el.border(layout.border_width).border_color(hairline)
            })
            .shadow(layout.overlay_shadow.clone());

        if self.title.is_some() || self.show_close_button {
            panel = panel.child(header_row);
        }
        // v3 composes the arrow as a part inside the panel's children. Hand
        // every composed `Popover.Arrow` the resolved placement it needs to
        // draw, and let the positioner reserve the arrow's 12px of gap.
        let mut has_arrow = false;
        for child in &mut self.children {
            if let Some(arrow) = child.downcast_mut::<PopoverArrow>() {
                arrow.resolved = resolved.clone();
                has_arrow = true;
            }
        }
        panel = panel.children(self.children);

        // The panel owns the dialog scope. Its handle is not a tab stop, but
        // the close affordance and child controls are, and Tab wraps among
        // those descendants without reaching the trigger or the page behind.
        let panel = crate::util::trap_tab(panel.track_focus(&panel_focus), &panel_focus);

        // React Aria dismisses a popover on Escape and on a press outside it.
        //
        let outside_close = close.clone();
        let trigger_pressed_for_dismissal = trigger_pressed;
        let panel = crate::util::dismiss_on_press_outside_with_token(
            panel,
            dismissal_token.clone(),
            move |window, cx| {
                if trigger_pressed_for_dismissal.get() {
                    return crate::util::DismissResult::Declined;
                }
                outside_close(window, cx);
                crate::util::DismissResult::Handled
            },
        );
        root =
            crate::util::dismiss_on_escape_with_token(root, dismissal_token, move |window, cx| {
                close(window, cx);
                crate::util::DismissResult::Handled
            });

        // v3 fades the panel in on `[data-entering]` and shifts it four
        // pixels from the resolved placement side.
        let (slide_x, slide_y) = placement_entry_offset(entry_placement);
        let zoom = crate::anim::ZoomBox::panel(panel_padding_y, radius)
            .padding_x(panel_padding_x)
            .sized(panel_width);
        let zoom = crate::anim::ZoomBox {
            slide_x: (slide_x != 0.0).then(|| px(slide_x)),
            slide_y: (slide_y != 0.0).then(|| px(slide_y)),
            ..zoom
        };
        let panel = if exiting {
            crate::anim::exiting(
                panel,
                "popover-panel-out",
                zoom,
                crate::anim::Motion::LIST_OUT,
                cx,
            )
        } else {
            crate::anim::entering_zoom(
                panel,
                "popover-panel",
                zoom,
                crate::anim::Motion::POPOVER_IN,
                cx,
            )
        };

        let positioner = PopoverPositioner::new(
            anchor_bounds,
            resolved,
            self.placement,
            self.offset,
            self.should_flip,
            has_arrow,
        )
        .with_resolved_placement(resolved_placement)
        .child(panel);
        // gpui paints in tree order, so a later sibling would overdraw the
        // open panel — every floating surface goes through `util::floating`
        // (v3 portals the popover above the page).
        root = root.child(crate::util::floating(positioner));
        root = crate::util::apply_sx(root, &self.sx);
        root
    }
}

#[cfg(test)]
mod tests {
    use super::{placement_entry_offset, PopoverPlacement};

    #[test]
    fn placement_entry_offsets_follow_the_painted_side() {
        assert_eq!(placement_entry_offset(PopoverPlacement::Top), (0.0, 4.0));
        assert_eq!(
            placement_entry_offset(PopoverPlacement::BottomStart),
            (0.0, -4.0)
        );
        assert_eq!(placement_entry_offset(PopoverPlacement::Left), (4.0, 0.0));
        assert_eq!(placement_entry_offset(PopoverPlacement::Right), (-4.0, 0.0));
    }
}

crate::util::impl_component_styled!(Popover);
