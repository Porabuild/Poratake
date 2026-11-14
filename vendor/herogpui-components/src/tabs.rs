//! Tabs — port of `@heroui/tabs`.

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    prelude::*, px, Animation, AnimationExt, AnyElement, App, InteractiveElement, IntoElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
};
use herogpui_core::{element_id, Orientation};
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};

/// Tab bar style (`variant`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabsVariant {
    /// Filled indicator behind the selected tab.
    #[default]
    Primary,
    /// Underline indicator beneath the selected tab.
    Secondary,
}

impl TabsVariant {
    /// Every variant, in display order.
    pub const ALL: [TabsVariant; 2] = [TabsVariant::Primary, TabsVariant::Secondary];

    /// The display name of this variant.
    pub fn label(self) -> &'static str {
        match self {
            TabsVariant::Primary => "Primary",
            TabsVariant::Secondary => "Secondary",
        }
    }
}

/// Content alignment inside each tab (`align`), independent of orientation.
/// Start and end follow the port's left-to-right layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabsAlign {
    /// Aligns tab content to the start.
    Start,
    /// Centers tab content.
    #[default]
    Center,
    /// Aligns tab content to the end.
    End,
}

impl TabsAlign {
    /// Every alignment, in display order.
    pub const ALL: [Self; 3] = [Self::Start, Self::Center, Self::End];

    /// The display name of this alignment.
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Center => "Center",
            Self::End => "End",
        }
    }

    fn apply<T: Styled>(self, tab: T) -> T {
        match self {
            Self::Start => tab.justify_start().text_left(),
            Self::Center => tab.justify_center().text_center(),
            Self::End => tab.justify_end().text_right(),
        }
    }
}

/// `.tabs__separator` transitions its opacity for 150ms with `--ease-smooth`.
const SEPARATOR_TRANSITION_MS: u64 = 150;
/// `.tabs__indicator` transitions translate, width and height for 250ms with
/// `--ease-out-fluid`.
const INDICATOR_TRANSITION_MS: u64 = 250;
/// v3's vertical-list tab floor (`.tabs__list[data-orientation="vertical"]`
/// sizes each tab `min-w-20`): a vertical tab never narrows below 80px, so a
/// short label still leaves the list a usable width.
const VERTICAL_TAB_MIN_WIDTH: gpui::Pixels = px(80.);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct IndicatorRect {
    x: gpui::Pixels,
    y: gpui::Pixels,
    width: gpui::Pixels,
    height: gpui::Pixels,
}

#[derive(Clone)]
struct IndicatorMotion {
    target: IndicatorRect,
    generation: usize,
    from: IndicatorRect,
    rect: Rc<Cell<IndicatorRect>>,
}

struct IndicatorMotionFrame {
    id: gpui::ElementId,
    generation: usize,
    from: IndicatorRect,
    to: IndicatorRect,
    rect: Rc<Cell<IndicatorRect>>,
    animate: bool,
}

impl IndicatorMotionFrame {
    fn render(self, indicator: gpui::Div) -> AnyElement {
        if !self.animate {
            self.rect.set(self.to);
            return place_indicator(indicator, self.to).into_any_element();
        }

        let rect = self.rect;
        let from = self.from;
        let to = self.to;
        indicator
            .with_animation(
                element_id::indexed(&self.id, "slide", self.generation),
                Animation::new(Duration::from_millis(INDICATOR_TRANSITION_MS))
                    .with_easing(|t| crate::anim::Curve::OutFluid.at(t)),
                move |indicator, delta| {
                    let next = IndicatorRect {
                        x: from.x + (to.x - from.x) * delta,
                        y: from.y + (to.y - from.y) * delta,
                        width: from.width + (to.width - from.width) * delta,
                        height: from.height + (to.height - from.height) * delta,
                    };
                    rect.set(next);
                    place_indicator(indicator, next)
                },
            )
            .into_any_element()
    }
}

fn place_indicator(indicator: gpui::Div, rect: IndicatorRect) -> gpui::Div {
    indicator
        .left(rect.x)
        .top(rect.y)
        .w(rect.width)
        .h(rect.height)
}

fn indicator_motion(
    id: gpui::ElementId,
    target: IndicatorRect,
    window: &mut Window,
    cx: &mut App,
) -> IndicatorMotionFrame {
    let state = window.use_keyed_state(id.clone(), cx, |_, _| IndicatorMotion {
        target,
        generation: 0,
        from: target,
        rect: Rc::new(Cell::new(target)),
    });
    let mut current = state.read(cx).clone();
    if current.target != target {
        current.target = target;
        current.generation = current.generation.wrapping_add(1);
        current.from = current.rect.get();
        state.update(cx, |stored, _| *stored = current.clone());
    }
    if ActiveTheme::reduce_motion(cx) && current.rect.get() != target {
        current.from = target;
        current.rect.set(target);
        state.update(cx, |stored, _| *stored = current.clone());
    }
    IndicatorMotionFrame {
        id,
        generation: current.generation,
        from: current.from,
        to: target,
        rect: current.rect,
        animate: current.generation != 0
            && !ActiveTheme::reduce_motion(cx)
            && current.from != target,
    }
}

#[derive(Clone, Debug, Default)]
struct TabsGeometry {
    list: Option<gpui::Bounds<gpui::Pixels>>,
    tabs: Vec<(SharedString, gpui::Bounds<gpui::Pixels>)>,
}

fn indicator_target(
    geometry: &TabsGeometry,
    selected_key: &SharedString,
    vertical: bool,
    secondary: bool,
) -> Option<IndicatorRect> {
    let list = geometry.list?;
    let tab = geometry
        .tabs
        .iter()
        .find_map(|(key, bounds)| (key == selected_key).then_some(*bounds))?;
    let x = tab.origin.x - list.origin.x;
    let y = tab.origin.y - list.origin.y;
    Some(if secondary && vertical {
        IndicatorRect {
            x: px(0.),
            y,
            width: px(2.),
            height: tab.size.height,
        }
    } else if secondary {
        IndicatorRect {
            x,
            y: list.size.height - px(2.),
            width: tab.size.width,
            height: px(2.),
        }
    } else {
        IndicatorRect {
            x,
            y,
            width: tab.size.width,
            height: tab.size.height,
        }
    })
}

fn scroll_tab_into_view(
    scroll: &gpui::ScrollHandle,
    geometry: &TabsGeometry,
    key: &SharedString,
    vertical: bool,
) {
    let Some(tab) = geometry
        .tabs
        .iter()
        .find_map(|(held, bounds)| (held == key).then_some(*bounds))
    else {
        return;
    };
    let viewport = scroll.bounds();
    let at = scroll.offset();
    let next = if vertical {
        let before = viewport.top() - tab.top();
        let after = tab.bottom() - viewport.bottom();
        if before > px(0.) && after > px(0.) {
            if before <= after {
                gpui::point(at.x, at.y + before)
            } else {
                gpui::point(at.x, at.y - after)
            }
        } else if before > px(0.) {
            gpui::point(at.x, at.y + viewport.top() - tab.top())
        } else if after > px(0.) {
            gpui::point(at.x, at.y - (tab.bottom() - viewport.bottom()))
        } else {
            return;
        }
    } else {
        let before = viewport.left() - tab.left();
        let after = tab.right() - viewport.right();
        if before > px(0.) && after > px(0.) {
            if before <= after {
                gpui::point(at.x + before, at.y)
            } else {
                gpui::point(at.x - after, at.y)
            }
        } else if before > px(0.) {
            gpui::point(at.x + viewport.left() - tab.left(), at.y)
        } else if after > px(0.) {
            gpui::point(at.x - (tab.right() - viewport.right()), at.y)
        } else {
            return;
        }
    };
    scroll.set_offset(next);
}

#[derive(Clone)]
struct SeparatorMotion {
    hidden: bool,
    generation: usize,
    from: f32,
    opacity: Rc<Cell<f32>>,
}

struct SeparatorMotionFrame {
    id: gpui::ElementId,
    generation: usize,
    from: f32,
    to: f32,
    opacity: Rc<Cell<f32>>,
    animate: bool,
}

impl SeparatorMotionFrame {
    fn render(self, separator: gpui::Div) -> AnyElement {
        if !self.animate {
            self.opacity.set(self.to);
            return separator.opacity(self.to).into_any_element();
        }

        let opacity = self.opacity;
        let from = self.from;
        let to = self.to;
        separator
            .with_animation(
                element_id::indexed(&self.id, "fade", self.generation),
                Animation::new(Duration::from_millis(SEPARATOR_TRANSITION_MS))
                    .with_easing(|t| crate::anim::Curve::Smooth.at(t)),
                move |separator, delta| {
                    let next = from + (to - from) * delta;
                    opacity.set(next);
                    separator.opacity(next)
                },
            )
            .into_any_element()
    }
}

fn separator_motion(
    id: gpui::ElementId,
    hidden: bool,
    window: &mut Window,
    cx: &mut App,
) -> SeparatorMotionFrame {
    let target = if hidden { 0. } else { 1. };
    let state = window.use_keyed_state(id.clone(), cx, |_, _| SeparatorMotion {
        hidden,
        generation: 0,
        from: target,
        opacity: Rc::new(Cell::new(target)),
    });
    let mut current = state.read(cx).clone();
    if current.hidden != hidden {
        current.hidden = hidden;
        current.generation = current.generation.wrapping_add(1);
        current.from = current.opacity.get();
        state.update(cx, |stored, _| *stored = current.clone());
    }
    if ActiveTheme::reduce_motion(cx) && (current.opacity.get() - target).abs() > f32::EPSILON {
        current.from = target;
        current.opacity.set(target);
        state.update(cx, |stored, _| *stored = current.clone());
    }
    SeparatorMotionFrame {
        id,
        generation: current.generation,
        from: current.from,
        to: target,
        opacity: current.opacity,
        animate: current.generation != 0
            && !ActiveTheme::reduce_motion(cx)
            && (current.from - target).abs() > f32::EPSILON,
    }
}

/// When arrow-key focus changes become selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyboardActivation {
    /// Select as focus moves, which is React Aria's default.
    #[default]
    Automatic,
    /// Move focus only; Enter or Space selects the focused tab.
    Manual,
}

/// One tab: key + label + panel content.
#[must_use = "builder methods return a new value; pass it on to its component"]
#[non_exhaustive]
pub struct TabItem {
    /// Stable key identifying the tab.
    pub key: SharedString,
    /// Visible text and accessible name of the tab.
    pub label: SharedString,
    /// Replacement content for the segment trigger itself; the label text is
    /// not rendered when it is set. `label` stays the tab's accessible name —
    /// an svg or icon div child carries no accessible name of its own.
    pub trigger: Option<AnyElement>,
    /// Content shown in the panel when the tab is selected.
    pub content: Option<AnyElement>,
    /// `Tabs.Tab.isDisabled` — removes this tab from activation and the roving
    /// keyboard stops without disabling its siblings.
    pub is_disabled: bool,
    /// `<Tabs.Separator />` inside this tab. v3 made the hairline between
    /// segments opt-in per tab in 3.0.0-beta.12, replacing the automatic
    /// pseudo-element and the `hideSeparator` prop it deleted.
    pub separator: bool,
    /// Fixed segment width; `None` keeps the content-hugging box. Set with
    /// [`TabItem::width`].
    pub width: Option<gpui::Pixels>,
    /// Fixed segment height; `None` keeps the size step's tab height. Set
    /// with [`TabItem::height`].
    pub height: Option<gpui::Pixels>,
    /// Fixed horizontal padding; `None` keeps the size step's tab padding.
    /// Set with [`TabItem::padding_x`].
    pub padding_x: Option<gpui::Pixels>,
}

impl TabItem {
    /// Creates a tab from a key and a label.
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            trigger: None,
            content: None,
            is_disabled: false,
            separator: false,
            width: None,
            height: None,
            padding_x: None,
        }
    }

    /// Renders this element as the segment trigger instead of the label text.
    ///
    /// This is how the icon-only segmented controls in the reference
    /// application are built: every trigger stays the same tab box with its
    /// id, hit area, hover, focus ring and keyboard activation, while the
    /// content becomes an icon. The trigger is presentational — keep it a
    /// plain `svg()` or `div()` so presses keep reaching the tab — and
    /// `svg()` does not inherit the tab's `text_color`, so set it explicitly.
    /// `label` is still the accessible name; an icon child carries none.
    pub fn trigger(mut self, el: impl IntoElement) -> Self {
        self.trigger = Some(el.into_any_element());
        self
    }

    /// Sets the panel content of the tab.
    pub fn content(mut self, el: impl IntoElement) -> Self {
        self.content = Some(el.into_any_element());
        self
    }

    /// Sets whether the tab is disabled (v3 `Tabs.Tab.isDisabled`).
    pub fn is_disabled(mut self, value: bool) -> Self {
        self.is_disabled = value;
        self
    }

    /// Composes a `Tabs.Separator` into this tab: a hairline before it.
    pub fn separator(mut self) -> Self {
        self.separator = true;
        self
    }

    /// Fixes the segment's width in both variants, beating the box the tab
    /// would otherwise hug its content into. This is the reference capture
    /// toolbar's `TabsTrigger className="size-8"`: a square 32px segment
    /// whose icon is narrower than the box. The sliding indicator measures
    /// the fixed box, so its geometry follows the override exactly.
    ///
    /// An explicit width also wins over [`Tabs::full_width`] for this tab
    /// alone — its siblings still divide the remaining width equally — and
    /// replaces the vertical list's 80px minimum, which would otherwise
    /// clamp a narrow segment back up.
    ///
    /// Not a v3 prop; it stands in for the `className` a caller passes to
    /// `Tabs.Tab`.
    pub fn width(mut self, w: impl Into<gpui::Pixels>) -> Self {
        self.width = Some(w.into());
        self
    }

    /// Fixes the segment's height in both variants, replacing the size
    /// step's tab height (32px on the pinned `Md`, 28px on `Sm`) instead of
    /// flooring it. The box stays fixed, so a wrapped label keeps painting
    /// its extra lines past it exactly as it does at the pinned height.
    ///
    /// Not a v3 prop; it stands in for the `className` a caller passes to
    /// `Tabs.Tab`.
    pub fn height(mut self, h: impl Into<gpui::Pixels>) -> Self {
        self.height = Some(h.into());
        self
    }

    /// Replaces the size step's horizontal padding (`px-4` on the pinned
    /// `Md` box, `px-3` on `Sm`). `0` means no horizontal padding — the
    /// fixed icon segment whose whole box is already sized by
    /// [`TabItem::width`] and [`TabItem::height`].
    ///
    /// Not a v3 prop; it stands in for the `className` a caller passes to
    /// `Tabs.Tab`.
    pub fn padding_x(mut self, p: impl Into<gpui::Pixels>) -> Self {
        self.padding_x = Some(p.into());
        self
    }
}

type OnChange = std::sync::Arc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;

#[derive(Clone)]
struct TabFocusState {
    key: SharedString,
    selected_key: SharedString,
    enabled_keys: Vec<SharedString>,
}

/// HeroGPUI-only compact step for [`Tabs`].
///
/// | Metric (pixels) | Sm | Md (pinned default) |
/// | --- | --- | --- |
/// | Tab height / horizontal padding | 28 / 12 | 32 / 16 |
/// | Label text / line height | 12 / 16 | 14 / 20 |
///
/// Both steps have zero vertical tab padding; the whole tab box is its hitbox.
/// Primary tabs and their full-box indicator keep `control_radius`, with a 4px
/// list padding that [`Tabs::list_padding`] overrides. Secondary tabs stay
/// square with a 2px accent indicator and no list padding. The content panel
/// keeps 8px padding. Horizontal tabs adjoin
/// and can scroll; `full_width` divides the available width equally. Vertical
/// lists retain a 4px gap and each tab's 80px minimum width. Explicit `sx`
/// pixel corners refine the indicator independently of its default shape.
///
/// v3.2.4 has no Tabs `size` prop (the box is `h-8 px-4 text-sm` through
/// classes), so this is additive: `Md` is byte-identical to the pinned box and
/// `Sm` is HeroGPUI's own 28px step. The secondary underline keeps its 2px
/// thickness at both steps. Not a v3 prop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabsSize {
    /// The small size.
    Sm,
    /// The medium size.
    #[default]
    Md,
}

impl TabsSize {
    /// Every size, in display order.
    pub const ALL: [TabsSize; 2] = [Self::Sm, Self::Md];

    /// `(height, horizontal padding, label text)` for this step; the label
    /// leading follows [`crate::util::leading_for`].
    fn metrics(self) -> (gpui::Pixels, gpui::Pixels, gpui::Pixels) {
        match self {
            Self::Sm => (px(28.), px(12.), px(12.)),
            Self::Md => (px(32.), px(16.), px(14.)),
        }
    }

    /// The display name of this size.
    pub fn label(self) -> &'static str {
        match self {
            Self::Sm => "Small",
            Self::Md => "Medium",
        }
    }
}

/// HeroUI Tabs (controlled).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Tabs {
    id: gpui::ElementId,
    items: Vec<TabItem>,
    /// `selectedKey` — `None` leaves the tabs holding the selection, seeded
    /// from `defaultSelectedKey`.
    selected_key: Option<SharedString>,
    default_selected_key: Option<SharedString>,
    variant: TabsVariant,
    align: TabsAlign,
    is_disabled: bool,
    /// The compact step; `Md` is the pinned default.
    size: TabsSize,
    /// The tab labels' font size, in place of the size step's.
    text_size: Option<gpui::Pixels>,
    full_width: bool,
    orientation: Orientation,
    keyboard_activation: KeyboardActivation,
    on_selection_change: Option<OnChange>,
    /// Corner radius override for the list container and the indicator.
    radius: Option<gpui::Pixels>,
    /// List container fill override; the primary tray is `bg-default` by
    /// default and the secondary tray is transparent.
    list_bg: Option<gpui::Hsla>,
    /// Indicator fill override; the primary segment token and the secondary
    /// accent line are the defaults.
    indicator_bg: Option<gpui::Hsla>,
    /// Gates the indicator's `shadow-surface`; the default keeps it.
    indicator_shadow: bool,
    /// The primary tray's uniform inset override; the pinned default is 4px.
    list_padding: Option<gpui::Pixels>,
    /// A vertical list's per-tab width floor; the pinned default is 80px.
    vertical_tab_min_width: Option<gpui::Pixels>,
    /// Gates the unselected-tab hover wash; the default keeps it.
    hover_fill: bool,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Tabs {
    /// `orientation` — a vertical tab list stacks its tabs.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Aligns content within each tab. The default is center. This does not
    /// reposition the list, its indicator, panel content, or nested tabs.
    pub fn align(mut self, align: TabsAlign) -> Self {
        self.align = align;
        self
    }

    /// React Aria's inherited `keyboardActivation`.
    pub fn keyboard_activation(mut self, activation: KeyboardActivation) -> Self {
        self.keyboard_activation = activation;
        self
    }

    /// `selectedKey` — drives the tabs from outside; without it they hold
    /// their own selection, seeded positionally by [`Tabs::new`].
    pub fn selected_key(mut self, key: impl Into<SharedString>) -> Self {
        self.selected_key = Some(key.into());
        self
    }

    /// `defaultSelectedKey` — the uncontrolled initial tab, also accepted
    /// positionally by [`Tabs::new`].
    ///
    /// Only consulted when `selectedKey` is not supplied; the tabs then own the
    /// selection and switch themselves on press.
    pub fn default_selected_key(mut self, key: impl Into<SharedString>) -> Self {
        self.default_selected_key = Some(key.into());
        self
    }

    /// The positional key is `defaultSelectedKey`, not `selectedKey`: seeding
    /// the *controlled* prop leaves the tabs unable to switch themselves, so
    /// every demo that passed a literal was inert. Pass
    /// [`Tabs::selected_key`] to drive them from outside.
    pub fn new(
        id: impl Into<gpui::ElementId>,
        items: Vec<TabItem>,
        default_selected_key: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            items,
            selected_key: None,
            default_selected_key: Some(default_selected_key.into()),
            variant: TabsVariant::Primary,
            align: TabsAlign::default(),
            is_disabled: false,
            full_width: false,
            size: TabsSize::default(),
            text_size: None,
            orientation: Orientation::Horizontal,
            keyboard_activation: KeyboardActivation::Automatic,
            on_selection_change: None,
            radius: None,
            list_bg: None,
            indicator_bg: None,
            indicator_shadow: true,
            list_padding: None,
            vertical_tab_min_width: None,
            hover_fill: true,
            sx: None,
        }
    }

    /// Sets the visual variant (v3 `variant`).
    pub fn variant(mut self, v: TabsVariant) -> Self {
        self.variant = v;
        self
    }

    /// Sets whether the whole tab list is disabled (v3 `isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets the compact step. `Md` is the default and byte-identical to the
    /// pinned box; `Sm` is a 28px box with 12px padding and a 12px label
    /// (16px leading). Not a v3 prop.
    pub fn size(mut self, size: TabsSize) -> Self {
        self.size = size;
        self
    }

    /// The tab labels' font size, in place of the size step's. A 12/14/16px size
    /// takes v3's leading pair (16/20/24); any other keeps the 20px leading.
    /// Each tab keeps its fixed height and inline padding, so a larger type wants a matching `TabItem::height`. Not a v3 prop: v3 sets it with a class on `Tabs.Tab`.
    pub fn text_size(mut self, size: impl Into<gpui::Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// v3's `<Tabs.List className="w-full">` with `<Tabs.Trigger
    /// className="flex-1">`: the list fills its parent and the tabs divide that
    /// width equally instead of hugging their labels.
    ///
    /// Horizontal only. A vertical list already stretches its tabs across the
    /// full width and sizes itself from the tallest content, so there is
    /// nothing for this to change and it is ignored there.
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self
    }

    /// Overrides the corner radius of the list container and the sliding
    /// indicator: both take this exact value instead of the derived container
    /// radius and the `control_radius` pill. The reference capture toolbar
    /// sets `rounded-3xl` on both to read as one segmented control. The tab
    /// boxes, their hover fades and their focus rings keep the pinned radius.
    ///
    /// Not a v3 prop; it stands in for the `className` the reference passes
    /// to `Tabs.ListContainer` and `Tabs.Indicator`.
    pub fn radius(mut self, radius: impl Into<gpui::Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// Overrides the list container's fill. The primary tray is `bg-default`
    /// and the secondary tray transparent by default; the reference
    /// segmented control tints the tray with `bg-muted-foreground/10`.
    ///
    /// Not a v3 prop; it stands in for the `className` the reference passes
    /// to `Tabs.ListContainer`.
    pub fn list_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.list_bg = Some(color.into());
        self
    }

    /// Overrides the indicator's fill in both variants — the primary segment
    /// token and the secondary accent line. The reference segmented control
    /// tints the indicator with `bg-muted-foreground/25`.
    ///
    /// Not a v3 prop; it stands in for the `className` the reference passes
    /// to `Tabs.Indicator`.
    pub fn indicator_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.indicator_bg = Some(color.into());
        self
    }

    /// `false` drops the indicator's surface shadow, and the pre-measurement
    /// active-tab fallback's, so an overridden indicator reads flat the way
    /// the reference's `shadow-none` does. The default keeps the pinned
    /// `shadow-surface`.
    pub fn indicator_shadow(mut self, v: bool) -> Self {
        self.indicator_shadow = v;
        self
    }

    /// Overrides the primary tray's inset on every side. The pinned default
    /// is the `p-1` the stylesheet puts on `.tabs__list`; the reference
    /// capture toolbar removes it with `Tabs.List className="p-0"` so its
    /// fixed 32px segments tile the tray edge to edge.
    ///
    /// Primary only: the Secondary tray has no inset of its own — v3's
    /// secondary block already replaces the list's `p-1` with `p-0` — so
    /// there is nothing for this to override there and the builder is
    /// ignored in that variant.
    ///
    /// Not a v3 prop; it stands in for the `className` a caller passes to
    /// `Tabs.List`.
    pub fn list_padding(mut self, padding: impl Into<gpui::Pixels>) -> Self {
        self.list_padding = Some(padding.into());
        self
    }

    /// Replaces the width floor every tab of a vertical list keeps (v3's
    /// `min-w-20`, 80px), so a narrow icon rail can go below it or a wide
    /// one can set a larger floor. Only the vertical orientation reads it —
    /// a horizontal list has no per-tab floor — and a tab's own
    /// [`TabItem::width`] still wins over it. Not a v3 prop; it stands in for
    /// the `className` a caller passes to `Tabs.Tab`.
    pub fn vertical_tab_min_width(mut self, width: impl Into<gpui::Pixels>) -> Self {
        self.vertical_tab_min_width = Some(width.into());
        self
    }

    /// `false` drops the unselected-tab hover wash and replaces it with a
    /// text-colour change: a hovered unselected tab keeps its resting fill
    /// and its label moves to the theme foreground, while the selected
    /// fill, the indicator, the focus rings and every tab's activation stay
    /// exactly as they are. The wash is the port's translation of the
    /// stylesheet's `opacity-70` dim on a hovered unselected tab — the same
    /// tray-dimmed overlay for `list_bg` and a root `sx` background alike —
    /// so with it off, none is painted regardless of what the tray resolves
    /// to. The overflow chevrons keep their own hover dim. The default
    /// keeps the wash.
    ///
    /// A `trigger` svg still needs its own `text_color`: gpui svgs do not
    /// inherit the tab's, so this hover cannot recolour an icon the caller
    /// painted explicitly. Unset the svg colour only when the rest colour
    /// is acceptable; otherwise the caller owns both rest and hover.
    ///
    /// Not a v3 prop; v3's hover dim is stylesheet work, and turning it off
    /// there is className work on the tab too.
    pub fn hover_fill(mut self, v: bool) -> Self {
        self.hover_fill = v;
        self
    }

    /// `onSelectionChange` — reports the key of the tab the press moves to.
    pub fn on_selection_change(
        mut self,
        f: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(std::sync::Arc::new(f));
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the tabs' root element after every value the variant and the
    /// active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl RenderOnce for Tabs {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = self.id.clone();
        // Only the `debug_selector` below still needs the id as a string, and
        // it keeps that string's exact original spelling.
        let base_id = format!("{:?}", self.id);

        // `selectedKey` wins; without it the tabs hold the selection, seeded
        // from `defaultSelectedKey` (falling back to the first tab so something
        // is always active). `controlled` takes `cx` mutably, so it precedes
        // the theme tokens.
        let first_enabled = self
            .items
            .iter()
            .find(|item| !item.is_disabled)
            .or_else(|| self.items.first())
            .map(|item| item.key.clone());
        let fallback = self
            .default_selected_key
            .clone()
            .or(first_enabled.clone())
            .unwrap_or_default();
        // One handle for the list: a tab list is one tab stop and the focused
        // tab claims it, which is how the stop roves. Flipping a handle's
        // `tab_stop` cannot do that -- it is fixed where the handle is made.
        let list_focus =
            crate::util::tab_stop_handle(element_id::scoped(&base, "focus"), window, cx);
        let (mut selected_key, selection_own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&base, "selected"),
            self.selected_key.clone(),
            fallback,
        );
        // Pinned `useTabListState` repairs an invalid uncontrolled default and
        // a selection whose item disappears from the collection. Use the
        // repaired key in this frame as well as storing it, so the replacement
        // panel and roving stop are never absent for a frame.
        if self.selected_key.is_none() && !self.items.iter().any(|item| item.key == selected_key) {
            if let (Some(next), Some(held)) = (first_enabled.clone(), selection_own.as_ref()) {
                selected_key = next.clone();
                held.update(cx, |value, cx| {
                    *value = next.clone();
                    cx.notify();
                });
                if let Some(cb) = &self.on_selection_change {
                    cb(&next, window, cx);
                }
            }
        }

        // React Aria keeps the roving focus key separate from selectedKey.
        // That distinction matters for controlled tabs: arrow keys move focus
        // and report each newly focused key without mutating the selection the
        // caller still owns.
        let focus_seed = self
            .items
            .iter()
            .find(|item| item.key == selected_key && !item.is_disabled)
            .map(|item| item.key.clone())
            .or(first_enabled)
            .unwrap_or_else(|| selected_key.clone());
        let enabled_keys: Vec<SharedString> = self
            .items
            .iter()
            .filter(|item| !item.is_disabled)
            .map(|item| item.key.clone())
            .collect();
        let focus_state =
            window.use_keyed_state(element_id::scoped(&base, "focused-key"), cx, |_, _| {
                TabFocusState {
                    key: focus_seed.clone(),
                    selected_key: selected_key.clone(),
                    enabled_keys: enabled_keys.clone(),
                }
            });
        let mut focus_now = focus_state.read(cx).clone();
        let selection_changed = focus_now.selected_key != selected_key;
        let focus_valid = enabled_keys.contains(&focus_now.key);
        if selection_changed && !list_focus.is_focused(window) {
            focus_now.key = focus_seed;
        } else if !focus_valid {
            let old_index = focus_now
                .enabled_keys
                .iter()
                .position(|key| key == &focus_now.key);
            focus_now.key = old_index
                .and_then(|index| {
                    focus_now.enabled_keys[index + 1..]
                        .iter()
                        .find(|key| enabled_keys.contains(key))
                        .or_else(|| {
                            focus_now.enabled_keys[..index]
                                .iter()
                                .rev()
                                .find(|key| enabled_keys.contains(key))
                        })
                        .cloned()
                })
                .unwrap_or(focus_seed);
        }
        if selection_changed || focus_now.enabled_keys != enabled_keys {
            focus_now.selected_key = selected_key.clone();
            focus_now.enabled_keys = enabled_keys;
            focus_state.update(cx, |state, _| *state = focus_now.clone());
        }
        let focused_key = focus_now.key;

        // `.tabs__list-container__scroller` is the box `.tabs__list` scrolls
        // inside; the handle is what says how far it has, which is what decides
        // whether each chevron shows.
        let scroll = window
            .use_keyed_state(element_id::scoped(&base, "scroll"), cx, |_, _| {
                gpui::ScrollHandle::new()
            })
            .read(cx)
            .clone();

        // The two chevrons' visibility, measured a frame ago; `use_keyed_state`
        // takes `cx` mutably, so it precedes the theme borrow.
        let arrows = window.use_keyed_state(element_id::scoped(&base, "arrows"), cx, |_, _| {
            (false, false)
        });
        let vertical = self.orientation == Orientation::Vertical;
        let vertical_tab_min_width = self
            .vertical_tab_min_width
            .unwrap_or(VERTICAL_TAB_MIN_WIDTH);
        // `full_width` is a horizontal contract: equal shares of the list's
        // width. A vertical list's tabs are already `w_full`.
        let stretch = self.full_width && !vertical;
        let secondary = self.variant == TabsVariant::Secondary;
        // The instance overrides refine the theme's resolved values; the
        // reference segmented control uses all of them at once.
        let radius_override = self.radius;
        let list_bg = self.list_bg;
        let indicator_bg = self.indicator_bg;
        let indicator_shadow = self.indicator_shadow;
        let hover_fill = self.hover_fill;
        let list_padding = self.list_padding;
        let (tab_h, tab_padding_x, tab_text) = self.size.metrics();
        let tab_text = self.text_size.unwrap_or(tab_text);
        let geometry = window.use_keyed_state(element_id::scoped(&base, "geometry"), cx, |_, _| {
            TabsGeometry::default()
        });
        let keyboard_focus = list_focus.is_focused(window) && crate::util::focus_visible(cx);
        let focus_presence =
            window.use_keyed_state(element_id::scoped(&base, "keyboard-focus"), cx, |_, _| {
                false
            });
        if *focus_presence.read(cx) != keyboard_focus {
            focus_presence.update(cx, |focused, _| *focused = keyboard_focus);
            if keyboard_focus {
                scroll_tab_into_view(&scroll, geometry.read(cx), &focused_key, vertical);
            }
        }
        let indicator_frame =
            indicator_target(geometry.read(cx), &selected_key, vertical, secondary).map(|target| {
                indicator_motion(
                    element_id::scoped(&base, "indicator-motion"),
                    target,
                    window,
                    cx,
                )
            });
        let active_idx = self.items.iter().position(|item| item.key == selected_key);
        let active_has_content =
            active_idx.is_some_and(|index| self.items[index].content.is_some());
        let panel_focus =
            crate::util::tab_stop_handle(element_id::scoped(&base, "panel-focus"), window, cx);
        let recover_replaced_panel_focus =
            selection_changed && panel_focus.contains_focused(window, cx);
        let panel_focus = active_has_content.then_some(panel_focus);
        let recovery_focus = window
            .use_keyed_state(element_id::scoped(&base, "focus-recovery"), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        if recover_replaced_panel_focus {
            window.focus(&recovery_focus, cx);
        }
        let has_enabled_tab = !self.is_disabled && self.items.iter().any(|item| !item.is_disabled);
        let mut separator_motions = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                (self.variant == TabsVariant::Primary && index > 0 && item.separator).then(|| {
                    let hidden =
                        item.key == selected_key || self.items[index - 1].key == selected_key;
                    separator_motion(
                        element_id::scoped(
                            &element_id::scoped(&base, "separator"),
                            item.key.clone(),
                        ),
                        hidden,
                        window,
                        cx,
                    )
                })
            })
            .collect::<Vec<_>>();

        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        let tabs_hover_opacity = layout.tabs_hover_opacity;
        // The pinned stylesheet dims unselected tabs and overflow arrows to
        // `tabs_hover_opacity`. GPUI cannot animate an element's opacity
        // without moving its listener path, so a same-colour overlay is faded
        // over the finished content while the stable tab/arrow element keeps
        // focus and pointer ownership. The overlay's colour is the resolved
        // tray fill — a state fill fades from the resting background, which
        // `list_bg` overrides and a root `sx` background refines last, the
        // same resting value `scrollbar.rs` resolves — not the stock token
        // those overrides replaced.
        let sx_background = crate::util::sx_background(&self.sx);
        let tray = if secondary {
            sx_background.or(list_bg).unwrap_or(colors.background)
        } else {
            sx_background.or(list_bg).unwrap_or(colors.default.color)
        };
        let tab_overlay = tray.alpha(1.0 - tabs_hover_opacity);
        let hover_fg = colors.foreground;
        let key_stops: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (!item.is_disabled).then_some(index))
            .collect();
        let key_keys: Vec<SharedString> = self.items.iter().map(|item| item.key.clone()).collect();

        // `.tabs__list` is `w-max min-w-full`: it grows with its content, which is
        // what lets the scroller overflow -- a shrinking row always fits and
        // never scrolls.
        // `react-aria/dist/private/tabs/useTabList.mjs` is
        // `role: 'tablist'` with `'aria-orientation': orientation`, always
        // present. The list had no id of its own, and a role on an element
        // with no `GlobalElementId` produces no node at all.
        let mut list = gpui::div()
            .id(element_id::scoped(&base, "tablist"))
            .a11y(a11y::Role::TabList)
            .a11y_orientation(self.orientation)
            .relative()
            .flex()
            .flex_shrink_0()
            // A vertical list is a flex item beside the panel; releasing its
            // cross-axis min-content width is what lets long labels wrap.
            // Keep the zero shrink on the column's main axis so a bounded
            // vertical scroller still retains its intrinsic content height.
            .when(vertical, |list| list.h_auto().min_w(px(0.)))
            .child({
                let measured = geometry.clone();
                gpui::canvas(
                    move |bounds, _window, cx| {
                        if measured.read(cx).list != Some(bounds) {
                            measured.update(cx, |geometry, cx| {
                                geometry.list = Some(bounds);
                                cx.notify();
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0()
            });
        if vertical {
            list = list.flex_col().items_start().gap(px(4.));
        } else if stretch {
            // `w_full`, not `min_w_full`: the stretched row is exactly the
            // viewport, so the scroller keeps no horizontal range and the
            // equal shares never turn into overflow.
            list = list.w_full();
        } else {
            list = list.min_w_full();
        }

        // v3 keeps two indicator styles: `primary` fills a segment behind the
        // selected tab, `secondary` underlines it.
        let sx_corners = crate::util::sx_radius(&self.sx);
        let indicator_ready = indicator_frame.is_some();
        if let Some(frame) = indicator_frame {
            // `.tabs__indicator` is the absolute `rounded-3xl bg-segment
            // shadow-surface` pill; Secondary flattens it into an accent line.
            let mut indicator = gpui::div()
                .absolute()
                .debug_selector(|| format!("{base_id}-indicator"));
            indicator = if !secondary {
                indicator
                    .rounded(crate::util::control_radius(cx))
                    .when_some(radius_override, |indicator, radius| {
                        indicator.rounded(radius)
                    })
                    .bg(indicator_bg.unwrap_or(colors.segment.background))
                    .when(
                        indicator_shadow && !layout.surface_shadow.is_empty(),
                        |indicator| indicator.shadow(layout.surface_shadow.clone()),
                    )
            } else {
                indicator.bg(indicator_bg.unwrap_or(colors.accent.color))
            };
            // The overrides refine the resolved defaults; `sx` corners refine
            // the result independently of the chosen radius.
            indicator = crate::util::round_sx_corners(indicator, &sx_corners);
            list = list.child(frame.render(indicator));
        }
        let measure_tab = |key: SharedString| {
            let measured = geometry.clone();
            gpui::canvas(
                move |bounds, _window, cx| {
                    let current = measured
                        .read(cx)
                        .tabs
                        .iter()
                        .find_map(|(held, bounds)| (held == &key).then_some(*bounds));
                    if current != Some(bounds) {
                        measured.update(cx, |geometry, cx| {
                            if let Some((_, held)) =
                                geometry.tabs.iter_mut().find(|(held, _)| held == &key)
                            {
                                *held = bounds;
                            } else {
                                geometry.tabs.push((key.clone(), bounds));
                            }
                            cx.notify();
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0()
        };
        // A constrained label slot releases its min-content width so normal
        // whitespace wraps inside the tab share; the tab keeps its fixed
        // `h-8` box and the wrapped lines paint past it like the stylesheet's
        // default overflow.
        let label_constrained = vertical || stretch;
        let tab_label = move |label: SharedString| {
            gpui::div()
                .min_w(px(0.))
                .when(label_constrained, |slot| slot.flex_1())
                .whitespace_normal()
                .child(label.to_string())
        };
        match self.variant {
            TabsVariant::Primary => {
                // `.tabs__list` is `p-1` and nothing else: the tabs sit
                // shoulder to shoulder, with no gap between them.
                list = list.p(px(4.));
                // `list_padding` refines the pinned inset; Primary only, the
                // secondary tray has no inset of its own to override.
                list = list.when_some(list_padding, |list, inset| list.p(inset));
                // `trigger` is single-use, so the loop consumes it out of each
                // item; the panel below reads `content` the same way.
                for (index, item) in self.items.iter_mut().enumerate() {
                    let active = item.key == selected_key;
                    let focused = item.key == focused_key;
                    let disabled = self.is_disabled || item.is_disabled;
                    let mut tab = gpui::div()
                        .id(element_id::scoped(
                            &element_id::scoped(&base, "tab"),
                            item.key.clone(),
                        ))
                        // `useTab.mjs`: `role: 'tab'` with
                        // `'aria-selected': isSelected`. Its
                        // `'aria-controls': isSelected ? tabPanelId :
                        // undefined` half needs an id graph gpui does not
                        // have, and `aria-disabled` has no builder; both are
                        // recorded omissions in `crate::a11y`. A gpui text
                        // child carries no name, so the label is restated.
                        .a11y_named(a11y::Role::Tab, &a11y::Name::labelled(item.label.clone()))
                        .a11y_selected(active)
                        .relative()
                        .when(!disabled && focused, |t| t.track_focus(&list_focus))
                        // `.tabs__tab` is `h-8 px-4 rounded-3xl text-sm
                        // font-medium` — `h-8` is a fixed box, not a floor.
                        // Upstream ships no truncate, nowrap or overflow
                        // utility on the tab, so a wrapped label keeps its
                        // extra lines and paints them past the pill the way
                        // CSS `overflow: visible` does; gpui paints every
                        // wrapped line too, giving the same rendering. Those
                        // lines stay pointer-inert here, while a browser
                        // hit-tests painted text as the tab.
                        .h(item.height.unwrap_or(tab_h))
                        .px(item.padding_x.unwrap_or(tab_padding_x))
                        // A fixed per-tab width beats every stretch source:
                        // the vertical list's `w_full` and its 80px minimum,
                        // and `full_width`'s equal shares below, whose
                        // `flex_1` basis of zero would ignore the width.
                        .when(vertical && item.width.is_none(), |t| {
                            t.w_full().min_w(vertical_tab_min_width)
                        })
                        .when_some(item.width, |t, w| t.w(w))
                        // Stretched tabs take an equal share: `flex_1` zeroes
                        // the flex basis and `min_w(0)` releases the label's
                        // min-content floor, so every share is the same and
                        // none of them widens the row.
                        .when(stretch && item.width.is_none(), |t| t.flex_1().min_w(px(0.)))
                        .when(!stretch || item.width.is_some(), |t| t.flex_shrink_0())
                        .flex()
                        .items_center()
                        .map(|tab| self.align.apply(tab))
                        .rounded(crate::util::control_radius(cx))
                        .text_size(tab_text)
                        .line_height(
                            crate::util::leading_for(tab_text).unwrap_or(px(20.)),
                        )
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .when(!disabled, |t| t.cursor(crate::util::interactive_cursor(cx)))
                        // `status-disabled` is `--disabled-opacity`.
                        .when(disabled, |t| t.opacity(cx.layout().disabled_opacity));
                    tab = tab.child(measure_tab(item.key.clone()));
                    if active {
                        tab = tab
                            .text_color(colors.segment.foreground)
                            .font_weight(gpui::FontWeight::MEDIUM)
                            // Before the indicator has measured geometry this
                            // active tab paints its fill itself, so it reads
                            // through the same overrides the indicator will.
                            .when(!indicator_ready, |tab| {
                                tab.bg(indicator_bg.unwrap_or(colors.segment.background))
                                    .when(
                                        indicator_shadow
                                            && !layout.surface_shadow.is_empty(),
                                        |tab| tab.shadow(layout.surface_shadow.clone()),
                                    )
                            });
                    } else {
                        tab = tab.text_color(colors.muted);
                    }
                    if !disabled {
                        // A tab list is one stop and the arrows move within
                        // it. Automatic activation selects as focus moves;
                        // manual activation waits for a press.
                        let key_stops = key_stops.clone();
                        let key_keys = key_keys.clone();
                        let key_cb = self.on_selection_change.clone();
                        let key_own = selection_own.clone();
                        let key_focus = focus_state.clone();
                        let key_geometry = geometry.clone();
                        let key_scroll = scroll.clone();
                        let automatic = self.keyboard_activation == KeyboardActivation::Automatic;
                        tab = tab.on_key_down(move |event, window, cx| {
                            let key = match (vertical, event.keystroke.key.as_str()) {
                                (_, "right") | (true, "down") => "down",
                                (_, "left") | (true, "up") => "up",
                                (_, other @ ("home" | "end")) => other,
                                _ => return,
                            };
                            // The list owns its axis and Home/End. Cross-axis
                            // keys returned above remain available to an
                            // enclosing scroller or navigation control.
                            cx.stop_propagation();
                            let crate::list_nav::Move::To(next) =
                                crate::list_nav::resolve(&key_stops, Some(index), key, true)
                            else {
                                return;
                            };
                            let Some(next_key) = key_keys.get(next).cloned() else {
                                return;
                            };
                            key_focus.update(cx, |state, cx| {
                                state.key = next_key.clone();
                                cx.notify();
                            });
                            scroll_tab_into_view(
                                &key_scroll,
                                key_geometry.read(cx),
                                &next_key,
                                vertical,
                            );
                            if automatic {
                                if let Some(held) = &key_own {
                                    let next_key = next_key.clone();
                                    held.update(cx, |v, cx| {
                                        *v = next_key;
                                        cx.notify();
                                    });
                                }
                                if let Some(f) = &key_cb {
                                    f(&next_key, window, cx);
                                }
                            }
                            // No refocusing: the next render has the newly
                            // focused tab claim the list's handle.
                        });
                        let key = item.key.clone();
                        let cb = self.on_selection_change.clone();
                        let own = selection_own.clone();
                        let focus = focus_state.clone();
                        let list_focus_for_click = list_focus.clone();
                        let list_focus_after_pointer = list_focus_for_click.clone();
                        let select: Rc<dyn Fn(&mut Window, &mut App)> =
                            Rc::new(move |window, cx| {
                                window.focus(&list_focus_for_click, cx);
                                focus.update(cx, |state, cx| {
                                    state.key = key.clone();
                                    cx.notify();
                                });
                                // Uncontrolled: move our own selection, or pressing
                                // a tab would do nothing.
                                if let Some(held) = &own {
                                    held.update(cx, |v, cx| {
                                        *v = key.clone();
                                        cx.notify();
                                    });
                                }
                                if let Some(f) = &cb {
                                    f(&key, window, cx);
                                }
                            });
                        let pointer_select = select.clone();
                        tab = tab
                            .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                                pointer_select(window, cx);
                                let list_focus = list_focus_after_pointer.clone();
                                window.defer(cx, move |window, cx| window.focus(&list_focus, cx));
                            })
                            .on_click(move |event, window, cx| {
                                if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                    select(window, cx);
                                }
                            });
                    }
                    // `.tab:focus-visible` is `status-focused`. The ring is an
                    // overlay child rather than a spread shadow so its corner
                    // stays concentric with the tab's own `control_radius`; a
                    // shadow would keep the tab's radius at a band two pixels
                    // further out and blur the edge. The tab is the element
                    // that both carries the rounding and shows the list's
                    // focus, and `anim::hover_fade` below refines that same
                    // element rather than wrapping it, so the overlay rides
                    // along untouched.
                    let mut tab = crate::util::with_focus_ring_overlay(
                        tab,
                        focused
                            && list_focus.is_focused(window)
                            && crate::util::focus_visible(cx)
                            && !disabled,
                        true,
                        crate::util::control_radius(cx),
                        Vec::new(),
                        cx,
                    );
                    if !active && !disabled {
                        if hover_fill {
                            let radius = crate::util::control_radius(cx);
                            tab = crate::anim::hover_fade(
                                tab,
                                element_id::scoped(&base, format!("tab-hover-{}", item.key)),
                                (colors.default.color.alpha(0.0), tab_overlay),
                                None,
                                None,
                                move |fill| fill.rounded(radius),
                                window,
                                cx,
                            );
                        } else {
                            tab = tab.hover(move |style| style.text_color(hover_fg));
                        }
                    }
                    // `.tabs__separator` is the absolute `w-px h-1/2
                    // rounded-sm` hairline inside the tab that carries it. It
                    // turns into `h-px w-[90%]` in a vertical list and hides
                    // beside the selected segment. It joins after the fade's
                    // fill — the fill is appended and would paint over a
                    // child already present.
                    if index > 0 && item.separator {
                        let separator = gpui::div()
                            .absolute()
                            .rounded(crate::util::hairline_radius(cx))
                            .bg(colors.muted.alpha(0.25));
                        let separator = if vertical {
                            separator
                                .left(gpui::relative(0.05))
                                .top(px(0.))
                                .w(gpui::relative(0.9))
                                .h(cx.layout().border_width)
                        } else {
                            separator
                                .left(px(0.))
                                .top(gpui::relative(0.25))
                                .w(cx.layout().border_width)
                                .h(gpui::relative(0.5))
                        };
                        let separator = separator_motions[index]
                            .take()
                            .expect("primary separators have motion state")
                            .render(separator);
                        tab = tab.child(separator);
                    }
                    // The trigger element replaces the label text when one is
                    // set; `label` keeps naming the tab through the
                    // `a11y_named` restatement above either way. It joins
                    // after the fade's fill — the fill is appended and would
                    // paint over a child already present.
                    tab = match item.trigger.take() {
                        Some(trigger) => tab.child(trigger),
                        None => tab.child(tab_label(item.label.clone())),
                    };
                    if focused && !disabled {
                        tab = crate::util::record_focus_bounds(tab, &list_focus, window, cx);
                    }
                    list = list.child(tab);
                }
            }
            TabsVariant::Secondary => {
                // `.tabs--secondary` gives the container a trailing-axis
                // border: bottom when horizontal, start when vertical.
                // `trigger` is single-use, so the loop consumes it out of each
                // item, as in the primary branch.
                for (index, item) in self.items.iter_mut().enumerate() {
                    let active = item.key == selected_key;
                    let focused = item.key == focused_key;
                    let disabled = self.is_disabled || item.is_disabled;
                    let mut tab = gpui::div()
                        .id(element_id::scoped(
                            &element_id::scoped(&base, "tab"),
                            item.key.clone(),
                        ))
                        // `useTab.mjs`: `role: 'tab'` with
                        // `'aria-selected': isSelected`. Its
                        // `'aria-controls': isSelected ? tabPanelId :
                        // undefined` half needs an id graph gpui does not
                        // have, and `aria-disabled` has no builder; both are
                        // recorded omissions in `crate::a11y`. A gpui text
                        // child carries no name, so the label is restated.
                        .a11y_named(a11y::Role::Tab, &a11y::Name::labelled(item.label.clone()))
                        .a11y_selected(active)
                        .relative()
                        .when(!disabled && focused, |t| t.track_focus(&list_focus))
                        // The same fixed `h-8 px-4 text-sm` box,
                        // `rounded-none`, with the indicator as a 2px bar
                        // along the bottom; wrapped label lines paint past it
                        // as in the primary variant.
                        .h(item.height.unwrap_or(tab_h))
                        .px(item.padding_x.unwrap_or(tab_padding_x))
                        // The same fixed-width precedence as the primary
                        // branch: a per-tab width beats the vertical `w_full`
                        // and its 80px minimum, and beats the stretch share
                        // whose `flex_1` basis of zero would ignore it.
                        .when(vertical && item.width.is_none(), |t| {
                            t.w_full().min_w(vertical_tab_min_width)
                        })
                        .when_some(item.width, |t, w| t.w(w))
                        // Stretched tabs take an equal share: `flex_1` zeroes
                        // the flex basis and `min_w(0)` releases the label's
                        // min-content floor, so every share is the same and
                        // none of them widens the row.
                        .when(stretch && item.width.is_none(), |t| t.flex_1().min_w(px(0.)))
                        .when(!stretch || item.width.is_some(), |t| t.flex_shrink_0())
                        .flex()
                        .items_center()
                        .map(|tab| self.align.apply(tab))
                        .text_size(tab_text)
                        .line_height(
                            crate::util::leading_for(tab_text).unwrap_or(px(20.)),
                        )
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .when(!indicator_ready && vertical, |t| t.border_l_2())
                        .when(!indicator_ready && !vertical, |t| t.border_b_2())
                        .when(!disabled, |t| t.cursor(crate::util::interactive_cursor(cx)))
                        // `status-disabled` is `--disabled-opacity`.
                        .when(disabled, |t| t.opacity(cx.layout().disabled_opacity));
                    tab = tab.child(measure_tab(item.key.clone()));
                    tab = if active {
                        tab.text_color(colors.foreground)
                            .font_weight(gpui::FontWeight::MEDIUM)
                            // Same pre-measurement fallback as the primary
                            // branch: the underline honors `indicator_bg`.
                            .when(!indicator_ready, |tab| {
                                tab.border_color(indicator_bg.unwrap_or(colors.accent.color))
                            })
                    } else {
                        tab.text_color(colors.muted).when(!indicator_ready, |tab| {
                            tab.border_color(gpui::transparent_black())
                        })
                    };
                    if !disabled {
                        // A tab list is one stop and the arrows move within
                        // it. Automatic activation selects as focus moves;
                        // manual activation waits for a press.
                        let key_stops = key_stops.clone();
                        let key_keys = key_keys.clone();
                        let key_cb = self.on_selection_change.clone();
                        let key_own = selection_own.clone();
                        let key_focus = focus_state.clone();
                        let key_geometry = geometry.clone();
                        let key_scroll = scroll.clone();
                        let automatic = self.keyboard_activation == KeyboardActivation::Automatic;
                        tab = tab.on_key_down(move |event, window, cx| {
                            let key = match (vertical, event.keystroke.key.as_str()) {
                                (_, "right") | (true, "down") => "down",
                                (_, "left") | (true, "up") => "up",
                                (_, other @ ("home" | "end")) => other,
                                _ => return,
                            };
                            // The list owns its axis and Home/End. Cross-axis
                            // keys returned above remain available to an
                            // enclosing scroller or navigation control.
                            cx.stop_propagation();
                            let crate::list_nav::Move::To(next) =
                                crate::list_nav::resolve(&key_stops, Some(index), key, true)
                            else {
                                return;
                            };
                            let Some(next_key) = key_keys.get(next).cloned() else {
                                return;
                            };
                            key_focus.update(cx, |state, cx| {
                                state.key = next_key.clone();
                                cx.notify();
                            });
                            scroll_tab_into_view(
                                &key_scroll,
                                key_geometry.read(cx),
                                &next_key,
                                vertical,
                            );
                            if automatic {
                                if let Some(held) = &key_own {
                                    let next_key = next_key.clone();
                                    held.update(cx, |v, cx| {
                                        *v = next_key;
                                        cx.notify();
                                    });
                                }
                                if let Some(f) = &key_cb {
                                    f(&next_key, window, cx);
                                }
                            }
                            // No refocusing: the next render has the newly
                            // focused tab claim the list's handle.
                        });
                        let key = item.key.clone();
                        let cb = self.on_selection_change.clone();
                        let own = selection_own.clone();
                        let focus = focus_state.clone();
                        let list_focus_for_click = list_focus.clone();
                        let list_focus_after_pointer = list_focus_for_click.clone();
                        let select: Rc<dyn Fn(&mut Window, &mut App)> =
                            Rc::new(move |window, cx| {
                                window.focus(&list_focus_for_click, cx);
                                focus.update(cx, |state, cx| {
                                    state.key = key.clone();
                                    cx.notify();
                                });
                                // Uncontrolled: move our own selection, or pressing
                                // a tab would do nothing.
                                if let Some(held) = &own {
                                    held.update(cx, |v, cx| {
                                        *v = key.clone();
                                        cx.notify();
                                    });
                                }
                                if let Some(f) = &cb {
                                    f(&key, window, cx);
                                }
                            });
                        let pointer_select = select.clone();
                        tab = tab
                            .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                                pointer_select(window, cx);
                                let list_focus = list_focus_after_pointer.clone();
                                window.defer(cx, move |window, cx| window.focus(&list_focus, cx));
                            })
                            .on_click(move |event, window, cx| {
                                if matches!(event, gpui::ClickEvent::Keyboard(_)) {
                                    select(window, cx);
                                }
                            });
                    }
                    // `.tab:focus-visible` is `status-focused`, painted as an
                    // overlay child. A secondary tab is `rounded-none`, so the
                    // ring's own corner is the square one the zero radius
                    // implies; the overlay is still the crisper of the two,
                    // since the shadow variant needs a non-zero blur. As in
                    // the primary branch the tab carries both the rounding and
                    // the list's focus, and `anim::hover_fade` refines it in
                    // place rather than wrapping it.
                    let mut tab = crate::util::with_focus_ring_overlay(
                        tab,
                        focused
                            && list_focus.is_focused(window)
                            && crate::util::focus_visible(cx)
                            && !disabled,
                        true,
                        px(0.),
                        Vec::new(),
                        cx,
                    );
                    if !active && !disabled {
                        if hover_fill {
                            let radius = crate::util::control_radius(cx);
                            tab = crate::anim::hover_fade(
                                tab,
                                element_id::scoped(&base, format!("tab-hover-{}", item.key)),
                                (colors.background.alpha(0.0), tab_overlay),
                                None,
                                None,
                                move |fill| fill.rounded(radius),
                                window,
                                cx,
                            );
                        } else {
                            tab = tab.hover(move |style| style.text_color(hover_fg));
                        }
                    }
                    // As in the primary branch: the trigger element replaces
                    // the label text; `label` keeps naming the tab. It joins
                    // after the fade's fill — the fill is appended and would
                    // paint over a child already present.
                    tab = match item.trigger.take() {
                        Some(trigger) => tab.child(trigger),
                        None => tab.child(tab_label(item.label.clone())),
                    };
                    if focused && !disabled {
                        tab = crate::util::record_focus_bounds(tab, &list_focus, window, cx);
                    }
                    list = list.child(tab);
                }
            }
        }

        if let Some(panel_focus) = panel_focus.clone() {
            list = list.on_key_down(move |event, window, cx| {
                if event.keystroke.key != "tab" || event.keystroke.modifiers.shift {
                    return;
                }
                cx.stop_propagation();
                crate::util::set_focus_visible(true, cx);
                // The wrapper is a real stop so reverse traversal can reach a
                // plain panel. Probe one stop past it: a child stays inside;
                // otherwise reclaim the wrapper as the correct destination.
                window.focus_next(cx);
                if panel_focus.is_focused(window) {
                    window.focus_next(cx);
                }
                if !panel_focus.contains_focused(window, cx) {
                    window.focus(&panel_focus, cx);
                }
            });
        }

        // Active panel
        let mut items = self.items;
        // `.tabs__list-container` is `relative`, holds the scroller, and hangs
        // the two `size-4` chevrons off its edges -- `hidden` until there is
        // something to scroll to in that direction (`start-1`/`end-1`, centred
        // on the cross axis).
        let (before, after) = *arrows.read(cx);
        // `.tabs__list-container__scroll-prev` and
        // `.tabs__list-container__scroll-next` are `size-4` circles at the
        // edges, shown only when there is something that way to scroll to.
        let mut arrow =
            |id: &str, icon: &'static str, direction: f32, handle: gpui::ScrollHandle| {
                let arrow_id = element_id::scoped(&base, id);
                let arrow = gpui::div()
                    .id(arrow_id.clone())
                    // gpui has no hitbox occlusion, so a chevron floating over
                    // the list hands its click to the tab underneath as well.
                    // v3's chevron is `z-2` above the `z-index: 1` tabs exactly
                    // so it takes the press; `occlude` stops the hit test at
                    // the button, which is that on-top layer.
                    .occlude()
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .cursor(crate::util::interactive_cursor(cx))
                    .text_color(colors.foreground)
                    .on_click(move |_, window, _| {
                        let at = handle.offset();
                        let viewport = handle.bounds().size;
                        let step = if vertical {
                            viewport.height * 0.8
                        } else {
                            viewport.width * 0.8
                        };
                        let delta = step * direction;
                        let next = if vertical {
                            gpui::point(at.x, at.y + delta)
                        } else {
                            gpui::point(at.x + delta, at.y)
                        };
                        handle.set_offset(next);
                        window.refresh();
                    });
                let arrow = crate::anim::hover_fade(
                    arrow,
                    element_id::scoped(&arrow_id, "hover-fade"),
                    (
                        if secondary {
                            colors.background.alpha(0.0)
                        } else {
                            colors.default.color.alpha(0.0)
                        },
                        tab_overlay,
                    ),
                    None,
                    None,
                    move |fill| fill.rounded_full(),
                    window,
                    cx,
                );
                // The chevron joins the skin after the fade's fill — the
                // fill is appended and would paint over a child already
                // present.
                let arrow = arrow.child(
                    gpui::svg()
                        .size(px(12.))
                        .path(icon)
                        .text_color(colors.foreground),
                );
                // Keep the occlusion on the outer animated node as well as the
                // inner control so the list tab underneath cannot win a hit test.
                arrow.absolute().occlude()
            };
        let container_radius = layout.radius_lg() * 2.5;
        // `list_bg` and `radius` refine the resolved tray fill and corner on
        // both variants; the reference segmented control sets all three of
        // fill, radius and indicator treatment this way.
        let container = gpui::div()
            .relative()
            .when(!secondary, |c| {
                c.bg(list_bg.unwrap_or(colors.default.color))
                    .rounded(container_radius.min(px(32.)))
                    .when_some(radius_override, |c, radius| c.rounded(radius))
            })
            .when(secondary, |c| {
                c.border_color(colors.border)
                    .when_some(list_bg, |c, bg| c.bg(bg))
                    .when_some(radius_override, |c, radius| c.rounded(radius))
                    .when(vertical, |c| c.border_l_1())
                    .when(!vertical, |c| c.border_b_1())
            })
            // A scroller only overflows if it is bounded: without `w_full` the
            // box grows to fit every tab and nothing ever scrolls.
            .when(!vertical, |c| c.w_full())
            // In a vertical row the list container is the horizontal flex
            // item beside the panel. Allow that item to shrink below a long
            // label's max-content width while keeping the list column's
            // vertical main-axis sizing intact.
            .when(vertical, |c| c.h_full().min_w(px(0.)).flex_shrink(1.))
            .child(
                crate::scroll_shadow::ScrollShadow::new(element_id::scoped(
                    &base, "scroller",
                ))
                    .orientation(if vertical {
                        Orientation::Vertical
                    } else {
                        Orientation::Horizontal
                    })
                    .size(px(64.))
                    .gap(px(0.))
                    .hide_scroll_bar(true)
                    .scroll_handle(scroll.clone())
                    .fill_axis(true)
                    .when(vertical, |shadow| shadow.unbounded_h())
                    .sx(move |root| {
                        if vertical {
                            root.h_full()
                        } else {
                            root.w_full()
                        }
                    })
                    .child(list),
            )
            .child({
                // `max_offset` is written during prepaint, so the render that
                // decided whether to draw an arrow read the frame before. This
                // canvas reads it in place and stores what it found; the entity
                // update is what asks for the frame that draws them.
                let measured = arrows;
                let handle = scroll.clone();
                gpui::canvas(
                    move |_bounds, _window, cx| {
                        let offset = handle.offset();
                        let max = handle.max_offset();
                        let next = if vertical {
                            (
                                f32::from(offset.y) < -0.5,
                                f32::from(offset.y) - 0.5 > -f32::from(max.y),
                            )
                        } else {
                            (
                                f32::from(offset.x) < -0.5,
                                f32::from(offset.x) - 0.5 > -f32::from(max.x),
                            )
                        };
                        if *measured.read(cx) != next {
                            measured.update(cx, |flags, cx| {
                                *flags = next;
                                cx.notify();
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size(px(0.))
            })
            .when(before, |c| {
                let a = arrow(
                    "scroll-prev",
                    if vertical {
                        crate::icons::CHEVRON_UP
                    } else {
                        crate::icons::CHEVRON_LEFT
                    },
                    1.,
                    scroll.clone(),
                );
                c.child(if vertical {
                    a.top(px(4.)).left(gpui::relative(0.5)).ml(px(-8.))
                } else {
                    // `start-1 top-1/2 -translate-y-1/2`.
                    a.left(px(4.)).top(gpui::relative(0.5)).mt(px(-8.))
                })
            })
            .when(after, |c| {
                let a = arrow(
                    "scroll-next",
                    if vertical {
                        crate::icons::CHEVRON_DOWN
                    } else {
                        crate::icons::CHEVRON_RIGHT
                    },
                    -1.,
                    scroll.clone(),
                );
                c.child(if vertical {
                    a.bottom(px(4.)).left(gpui::relative(0.5)).ml(px(-8.))
                } else {
                    a.right(px(4.)).top(gpui::relative(0.5)).mt(px(-8.))
                })
            });

        // `.tabs` is `flex gap-2`: horizontal tabs stack their panel below;
        // vertical tabs place it beside the list.
        let mut el = gpui::div()
            .track_focus(&recovery_focus)
            .flex()
            .when(vertical, |root| root.flex_row().h_full())
            .when(!vertical, |root| root.flex_col())
            .gap(px(8.))
            .child(container);

        if let Some(idx) = active_idx {
            // `useTabPanel.mjs` labels the panel by the selected tab
            // (`aria-labelledby: generateId(state, state.selectedKey,
            // 'tab')`); with no id graph the tab's own text is inlined.
            let panel_name = a11y::Name::labelled(items[idx].label.clone());
            if let Some(content) = items.swap_remove(idx).content {
                let panel_focus = panel_focus.expect("a rendered panel has a focus handle");
                let panel_focus_for_keys = panel_focus.clone();
                let panel_focus_for_key_up = panel_focus.clone();
                let list_focus_for_panel = list_focus;
                let probe_on_key_up = !has_enabled_tab;
                // `.tabs__panel` is `w-full p-2`, with `mt-4` horizontally or
                // `ms-4` vertically.
                el = el.child(
                    gpui::div()
                        .w_full()
                        .p(px(8.))
                        .when(vertical, |panel| panel.ml(px(16.)))
                        .when(!vertical, |panel| panel.mt(px(16.)))
                        .track_focus(&panel_focus)
                        .on_key_down(move |event, window, cx| {
                            if event.keystroke.key != "tab" {
                                return;
                            }
                            if event.keystroke.modifiers.shift {
                                cx.stop_propagation();
                                crate::util::set_focus_visible(true, cx);
                                // Skip the wrapper only from a real child stop.
                                // A programmatically focused non-stop descendant
                                // should land on it when moving backward.
                                let skip_panel = !panel_focus_for_keys.is_focused(window)
                                    && window.focused(cx).is_some_and(|handle| handle.tab_stop);
                                window.focus_prev(cx);
                                if skip_panel && panel_focus_for_keys.is_focused(window) {
                                    window.focus_prev(cx);
                                    if has_enabled_tab && !list_focus_for_panel.is_focused(window) {
                                        window.focus(&list_focus_for_panel, cx);
                                    }
                                }
                            } else if probe_on_key_up
                                && event.is_held
                                && panel_focus_for_keys.is_focused(window)
                            {
                                // The first held repeat arrives before key-up
                                // has removed the synthetic wrapper stop. Walk
                                // the child it stands in for and then one real
                                // step, skipping the wrapper again on wrap.
                                cx.stop_propagation();
                                window.focus_next(cx);
                                if panel_focus_for_keys.contains_focused(window, cx)
                                    && !panel_focus_for_keys.is_focused(window)
                                {
                                    window.focus_next(cx);
                                    if panel_focus_for_keys.is_focused(window) {
                                        window.focus_next(cx);
                                    }
                                }
                            }
                        })
                        .on_key_up(move |event, window, cx| {
                            if !probe_on_key_up
                                || event.keystroke.key != "tab"
                                || event.keystroke.modifiers.shift
                                || !panel_focus_for_key_up.is_focused(window)
                            {
                                return;
                            }
                            // An all-disabled list has no focused tab whose
                            // key-down can run the forward probe. Tab key-up is
                            // delivered to the wrapper it just reached.
                            window.focus_next(cx);
                            if !panel_focus_for_key_up.contains_focused(window, cx) {
                                window.focus(&panel_focus_for_key_up, cx);
                            }
                        })
                        .child(content)
                        // Stated after the layout chain rather than inside it:
                        // `.shots/design_audit.py` reads `.tabs__panel`'s
                        // padding through a character-windowed regex anchored
                        // on the comment above the box, and an id plus a role
                        // pushed the `.p(..)` out of that window.
                        .id(element_id::scoped(&base, "tabpanel"))
                        .a11y_named(a11y::Role::TabPanel, &panel_name),
                );
            }
        }

        crate::util::apply_sx(el, &self.sx)
    }
}

#[cfg(test)]
mod tabs_size_tests {
    use super::*;

    #[test]
    fn md_is_the_pinned_geometry_and_sm_scales_together() {
        assert_eq!(TabsSize::default(), TabsSize::Md);
        assert_eq!(TabsSize::Md.metrics(), (px(32.), px(16.), px(14.)));
        assert_eq!(TabsSize::Sm.metrics(), (px(28.), px(12.), px(12.)));
        assert_eq!(
            crate::util::leading_for(TabsSize::Sm.metrics().2),
            Some(px(16.))
        );
    }
}

crate::util::impl_component_styled!(Tabs);
