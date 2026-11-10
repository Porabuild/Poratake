//! ListBox — port of `@heroui/list-box` (v3).
//!
//! A list of options, nonselecting by default: the pinned `useListState`
//! defaults `selectionMode` to `none`. Mirrors the React API: `selectionMode`,
//! `selectedKeys`, `disabledKeys`, `onSelectionChange`, `onAction`, and the
//! `default | danger` item variant. Sections are expressed with
//! [`ListBoxItem::section`] headers and [`ListBoxItem::separator`].

use std::collections::HashSet;
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, App, ElementId, InteractiveElement, IntoElement, RenderOnce, SharedString,
    Styled, Window,
};
use herogpui_core::{element_id, SelectionMode};
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    icons, util, EscapeKeyBehavior,
};

/// Visual variant of a list item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListBoxItemVariant {
    #[default]
    /// The standard appearance.
    Default,
    /// Destructive action — danger text, danger-soft hover.
    Danger,
}

/// One row of a [`ListBox`].
#[must_use = "builder methods return a new value; pass the item to its component"]
#[derive(Clone)]
pub enum ListBoxItem {
    /// A selectable option.
    Option {
        /// Unique key identifying the option.
        key: SharedString,
        /// Text shown for the option.
        label: SharedString,
        /// Optional secondary line beneath the label.
        description: Option<SharedString>,
        /// Asset path of a leading icon.
        icon: Option<SharedString>,
        /// Trailing shortcut hint.
        shortcut: Option<SharedString>,
        /// Visual variant of the option.
        variant: ListBoxItemVariant,
        /// Whether the option is disabled.
        is_disabled: bool,
    },
    /// A non-interactive section header.
    Section(SharedString),
    /// A horizontal rule between groups.
    Separator,
}

impl ListBoxItem {
    /// Creates an option with the given key and label.
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self::Option {
            key: key.into(),
            label: label.into(),
            description: None,
            icon: None,
            shortcut: None,
            variant: ListBoxItemVariant::Default,
            is_disabled: false,
        }
    }

    /// Creates a non-interactive section header.
    pub fn section(label: impl Into<SharedString>) -> Self {
        Self::Section(label.into())
    }

    /// Creates a horizontal separator between groups.
    pub fn separator() -> Self {
        Self::Separator
    }

    /// Secondary line beneath the label.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        if let Self::Option { description, .. } = &mut self {
            *description = Some(text.into());
        }
        self
    }

    /// Sets the leading icon asset path. Ignored for headers and separators.
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        if let Self::Option { icon, .. } = &mut self {
            *icon = Some(path.into());
        }
        self
    }

    /// Sets the trailing shortcut hint. Ignored for headers and separators.
    pub fn shortcut(mut self, text: impl Into<SharedString>) -> Self {
        if let Self::Option { shortcut, .. } = &mut self {
            *shortcut = Some(text.into());
        }
        self
    }

    /// Sets the option's visual variant. Ignored for headers and separators.
    pub fn variant(mut self, v: ListBoxItemVariant) -> Self {
        if let Self::Option { variant, .. } = &mut self {
            *variant = v;
        }
        self
    }

    /// Shorthand for [`ListBoxItemVariant::Danger`].
    pub fn danger(self) -> Self {
        self.variant(ListBoxItemVariant::Danger)
    }

    /// Disables the option (v3 `isDisabled`). Ignored for headers and separators.
    pub fn is_disabled(mut self, v: bool) -> Self {
        if let Self::Option { is_disabled, .. } = &mut self {
            *is_disabled = v;
        }
        self
    }

    /// The item's key, or `None` for headers and separators.
    pub fn key(&self) -> Option<&SharedString> {
        match self {
            Self::Option { key, .. } => Some(key),
            _ => None,
        }
    }
}

type OnSelectionChange = Arc<dyn Fn(&HashSet<SharedString>, &mut Window, &mut App) + 'static>;
type OnAction = Arc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;
/// `ListBox.ItemIndicator`'s render function, handed `isSelected`.
type Indicator = Arc<dyn Fn(bool) -> gpui::AnyElement + 'static>;

/// The anchor and the moving end of a Shift range in a multiple-selection
/// collection (React Stately's `anchorKey` / `currentKey`), and whether the
/// selection is a raw select-all. `TreeView` shares it.
#[derive(Clone, Debug, Default)]
pub(crate) struct ListBoxSelectionRange {
    pub(crate) anchor: Option<SharedString>,
    pub(crate) current: Option<SharedString>,
    pub(crate) is_all: bool,
}

/// React Stately's `extendSelection`: drop the keys between the anchor and
/// the previous range end, then add the selectable keys between the anchor
/// and `target`, in `collection` order. A raw select-all collapses to
/// `target`, and with no anchor the range starts at `target`. `TreeView`
/// shares it.
pub(crate) fn extend_selection_range(
    current: &HashSet<SharedString>,
    collection: &[SharedString],
    selectable: &HashSet<SharedString>,
    range: &ListBoxSelectionRange,
    target: &SharedString,
) -> HashSet<SharedString> {
    if range.is_all {
        return HashSet::from([target.clone()]);
    }
    let anchor = range.anchor.as_ref().unwrap_or(target);
    let previous = range.current.as_ref().unwrap_or(target);
    let anchor_at = collection.iter().position(|key| key == anchor);
    let previous_at = collection.iter().position(|key| key == previous);
    let target_at = collection.iter().position(|key| key == target);
    let between = |from: Option<usize>, to: Option<usize>| {
        from.zip(to)
            .map(|(from, to)| if from <= to { from..=to } else { to..=from })
    };
    let mut next = current.clone();
    if let Some(previous_range) = between(anchor_at, previous_at) {
        for index in previous_range {
            next.remove(&collection[index]);
        }
    }
    if let Some(target_range) = between(anchor_at, target_at) {
        for index in target_range {
            let key = &collection[index];
            if selectable.contains(key) {
                next.insert(key.clone());
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

/// HeroUI ListBox.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ListBox {
    id: ElementId,
    items: Vec<ListBoxItem>,
    selection_mode: SelectionMode,
    selected_keys: HashSet<SharedString>,
    default_selected_keys: HashSet<SharedString>,
    is_controlled: bool,
    disallow_empty_selection: bool,
    escape_key_behavior: EscapeKeyBehavior,
    disabled_keys: HashSet<SharedString>,
    /// Applies to every item unless the item overrides it.
    variant: ListBoxItemVariant,
    max_h: Option<gpui::Pixels>,
    /// `shouldFocusWrap` — whether arrow keys wrap at the ends.
    should_focus_wrap: bool,
    /// `ListLayout`'s `rowHeight`. Setting it virtualizes the list: a fixed row
    /// height is what lets the geometry be computed instead of laid out.
    row_height: Option<gpui::Pixels>,
    /// Replaces the option rows' `px-2` horizontal padding.
    row_padding_x: Option<gpui::Pixels>,
    /// Replaces the option rows' `py-1.5` vertical padding.
    row_padding_y: Option<gpui::Pixels>,
    /// The fill a hovered option row takes, in place of `--default`.
    row_hover_bg: Option<gpui::Hsla>,
    /// `ListLayout`'s `estimatedRowHeight` — the estimate that virtualizes a
    /// list whose rows are *not* all one height.
    estimated_row_height: Option<gpui::Pixels>,
    /// `ListLayout`'s `headingHeight` — a section row's height when the list is
    /// virtual.
    heading_height: Option<gpui::Pixels>,
    /// `ListLayout`'s `gap` and `padding`, which override the stylesheet's.
    gap: gpui::Pixels,
    padding: gpui::Pixels,
    /// `ListBox.ItemIndicator` — draw the tick yourself. v3 hands its render
    /// function `isSelected`, so this closure receives it.
    indicator: Option<Indicator>,
    /// `children` on `ListBox.Item` — a render function handed the row's key and
    /// its state.
    item_content:
        Option<Arc<dyn Fn(&SharedString, util::InteractiveState) -> gpui::AnyElement + 'static>>,
    on_selection_change: Option<OnSelectionChange>,
    on_action: Option<OnAction>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl ListBox {
    /// Creates a non-selecting list with the given id and items.
    pub fn new(id: impl Into<ElementId>, items: Vec<ListBoxItem>) -> Self {
        Self {
            id: id.into(),
            items,
            // Pinned `useListState` (`useMultipleSelectionState`) defaults
            // `selectionMode` to `none`; HeroUI's v3 ListBox wrapper forwards
            // props to React Aria untouched, so a plain list is nonselecting.
            // Enable single or multiple selection with `selection_mode`.
            selection_mode: SelectionMode::None,
            selected_keys: HashSet::new(),
            default_selected_keys: HashSet::new(),
            is_controlled: false,
            disallow_empty_selection: false,
            escape_key_behavior: EscapeKeyBehavior::ClearSelection,
            disabled_keys: HashSet::new(),
            variant: ListBoxItemVariant::Default,
            should_focus_wrap: false,
            row_height: None,
            row_padding_x: None,
            row_padding_y: None,
            row_hover_bg: None,
            estimated_row_height: None,
            heading_height: None,
            // `.list-box` is `p-1` with `mt-1` between children.
            gap: px(4.),
            padding: px(4.),
            max_h: None,
            indicator: None,
            item_content: None,
            on_selection_change: None,
            on_action: None,
            sx: None,
        }
    }

    /// Sets the selection mode (v3 `selectionMode`).
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// Sets the controlled selection (v3 `selectedKeys`) from item keys.
    pub fn selected_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.selected_keys = keys.into_iter().collect();
        self.is_controlled = true;
        self
    }

    /// Controlled single-key convenience. Does not change the selection mode:
    /// pair with `selection_mode(SelectionMode::Single)` for interactive picks.
    pub fn selected_key(mut self, key: impl Into<SharedString>) -> Self {
        self.selected_keys = HashSet::from([key.into()]);
        self.is_controlled = true;
        self
    }

    /// `defaultSelectedKeys` — seeds the list's own selection state.
    pub fn default_selected_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.default_selected_keys = keys.into_iter().collect();
        self
    }

    /// `disallowEmptySelection` — keeps the final selected item selected.
    pub fn disallow_empty_selection(mut self, v: bool) -> Self {
        self.disallow_empty_selection = v;
        self
    }

    /// `escapeKeyBehavior` — whether unmodified Escape clears selection.
    pub fn escape_key_behavior(mut self, behavior: EscapeKeyBehavior) -> Self {
        self.escape_key_behavior = behavior;
        self
    }

    /// Disables the given item keys (v3 `disabledKeys`).
    pub fn disabled_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.disabled_keys = keys.into_iter().collect();
        self
    }

    /// Sets the visual variant applied to the list's options.
    pub fn variant(mut self, variant: ListBoxItemVariant) -> Self {
        self.variant = variant;
        self
    }

    /// `shouldFocusWrap` — whether the arrow keys wrap at the ends of the list.
    pub fn should_focus_wrap(mut self, v: bool) -> Self {
        self.should_focus_wrap = v;
        self
    }

    /// Caps the list height and scrolls beyond it.
    pub fn max_h(mut self, h: impl Into<gpui::Pixels>) -> Self {
        self.max_h = Some(h.into());
        self
    }

    /// `ListLayout`'s `rowHeight` — **and** what virtualizes the list.
    ///
    /// v3 wraps the list in `<Virtualizer layout={ListLayout}
    /// layoutOptions={{rowHeight: 50}}>`; the wrapper has no separate identity
    /// here, so the option that defines the layout carries it. a uniform
    /// [`VirtualList`](crate::VirtualList) builds only the rows the viewport shows, and it can do
    /// that because every row is this tall.
    pub fn row_height(mut self, h: impl Into<gpui::Pixels>) -> Self {
        self.row_height = Some(h.into());
        self
    }

    /// `ListLayout`'s `estimatedRowHeight` — virtualize rows that are *not* all
    /// the same height.
    ///
    /// `rowHeight` maps to a uniform `VirtualList`, which measures one row and multiplies;
    /// this maps to gpui's `list`, which measures each row it builds and keeps a
    /// running total, so a described row and a plain one can differ. The estimate
    /// is what it renders beyond the viewport (`overdraw`) while it learns the
    /// real heights.
    pub fn estimated_row_height(mut self, h: impl Into<gpui::Pixels>) -> Self {
        self.estimated_row_height = Some(h.into());
        self
    }

    /// `ListLayout`'s `headingHeight` — how tall a section row is in a virtual
    /// list, where a row cannot size itself.
    pub fn heading_height(mut self, h: impl Into<gpui::Pixels>) -> Self {
        self.heading_height = Some(h.into());
        self
    }

    /// `ListLayout`'s `gap`, overriding the stylesheet's `mt-1`.
    pub fn gap(mut self, gap: impl Into<gpui::Pixels>) -> Self {
        self.gap = gap.into();
        self
    }

    /// `ListLayout`'s `padding`, overriding the stylesheet's `p-1`.
    pub fn padding(mut self, padding: impl Into<gpui::Pixels>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Replaces the option rows' `px-2` horizontal padding. Section headings
    /// keep their own inset.
    pub fn row_padding_x(mut self, p: impl Into<gpui::Pixels>) -> Self {
        self.row_padding_x = Some(p.into());
        self
    }

    /// Replaces the option rows' `py-1.5` vertical padding.
    pub fn row_padding_y(mut self, p: impl Into<gpui::Pixels>) -> Self {
        self.row_padding_y = Some(p.into());
        self
    }

    /// The fill a hovered option row takes, in place of `--default`.
    pub fn row_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.row_hover_bg = Some(color.into());
        self
    }

    /// `ListBox.ItemIndicator` — draw the selected tick yourself.
    ///
    /// The closure is handed `isSelected`, the value v3 passes into the same
    /// render function, so a caller can return its own glyph, or nothing.
    /// `children` on `ListBox.Item` — replaces a row's label.
    ///
    /// The closure is handed the row's key and the state v3 passes into the same
    /// render prop: `isSelected`, `isFocused`, `isPressed` and `isDisabled`. The
    /// press is a frame behind the pointer or activation key, because gpui
    /// reports it to a handler.
    pub fn item_content(
        mut self,
        render: impl Fn(&SharedString, util::InteractiveState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.item_content = Some(Arc::new(render));
        self
    }

    /// Renders a custom indicator for each option; the closure receives whether the option is selected.
    pub fn indicator(mut self, render: impl Fn(bool) -> gpui::AnyElement + 'static) -> Self {
        self.indicator = Some(Arc::new(render));
        self
    }

    /// Called with the full selection after a toggle.
    pub fn on_selection_change(
        mut self,
        handler: impl Fn(&HashSet<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Arc::new(handler));
        self
    }

    /// Called when an item is activated, regardless of selection mode.
    pub fn on_action(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Arc::new(handler));
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the list box's root element after every value the layout
    /// and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }
}

impl RenderOnce for ListBox {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Which row the keyboard is on, and the handle that receives the keys.
        // `use_keyed_state` takes `cx` mutably, so both precede the tokens.
        let base_id = self.id.clone();
        // The `debug_selector` spelling `collections.rs` queries on; a label,
        // not an id.
        let base = format!("{base_id:?}");
        let focus_handle =
            window.use_keyed_state(element_id::scoped(&base_id, "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            });
        let focus_handle = focus_handle.read(cx).clone();
        let cursor = window.use_keyed_state(element_id::scoped(&base_id, "cursor"), cx, |_, _| {
            None::<usize>
        });
        let selection_range = window.use_keyed_state(
            element_id::scoped(&base_id, "selection-range"),
            cx,
            |_, _| ListBoxSelectionRange::default(),
        );
        let mut cursor_at = *cursor.read(cx);
        let (selected_keys, selection_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base_id, "selected"),
            self.is_controlled.then(|| self.selected_keys.clone()),
            self.default_selected_keys.clone(),
        );
        self.selected_keys = selected_keys;
        // React Aria keeps the focused row in view. Two handles, because the
        // virtual list owns its own scrolling and a plain one does not.
        let list_scroll = {
            let count = self.items.len();
            window.use_keyed_state(
                element_id::scoped(&base_id, "list-scroll"),
                cx,
                move |_, _| crate::VirtualListHandle::uniform(count),
            )
        };
        let box_scroll =
            window.use_keyed_state(element_id::scoped(&base_id, "box-scroll"), cx, |_, _| {
                gpui::ScrollHandle::new()
            });
        // `gpui::list`'s state is intrusive -- the caller holds it -- so a
        // variable-height list keeps one here, seeded with the item count and
        // the estimate it overdraws by.
        let list_state = {
            let count = self.items.len();
            let overdraw = self.estimated_row_height.unwrap_or(px(36.)) * 3.;
            window.use_keyed_state(
                element_id::scoped(&base_id, "list-state"),
                cx,
                move |_, _| crate::VirtualListHandle::with_overdraw(count, overdraw),
            )
        };
        let variable_row_heights = if self.row_height.is_none()
            && self.estimated_row_height.is_some()
        {
            let identities: Vec<String> = self
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| match item {
                    ListBoxItem::Option { key, .. } => format!("option:{key}"),
                    ListBoxItem::Section(label) => format!("section:{label}"),
                    ListBoxItem::Separator => format!("separator:{index}"),
                })
                .collect();
            let count = identities.len();
            let heights =
                window.use_keyed_state(element_id::scoped(&base_id, "row-heights"), cx, |_, _| {
                    (Vec::<String>::new(), Vec::<Option<gpui::Pixels>>::new())
                });
            if heights.read(cx).0 != identities {
                heights.update(cx, |stored, _| {
                    *stored = (identities, vec![None; count]);
                });
            }
            Some(heights)
        } else {
            None
        };
        let list_scroll_now = list_scroll.read(cx).clone();
        let box_scroll_now = box_scroll.read(cx).clone();
        let list_handle_now = list_state.read(cx).clone();
        let list_state_now = list_handle_now.list_state().clone();
        // The letters typed so far. A search that reset every frame could only
        // ever match one letter.
        let typed = window.use_keyed_state(element_id::scoped(&base_id, "typed"), cx, |_, _| {
            crate::list_nav::Typeahead::default()
        });
        // One hover/press slot per row. Custom `item_content` render props
        // read the slot's state, and ordinary rows use the same slot for the
        // default HeroUI `scale(0.98)` press skin.
        let interaction: std::rc::Rc<Vec<util::Interaction>> = std::rc::Rc::new(
            (0..self.items.len())
                .map(|index| {
                    util::interaction(
                        element_id::scoped(
                            &element_id::indexed(&self.id, "item", index),
                            "interaction",
                        ),
                        window,
                        cx,
                    )
                })
                .collect(),
        );

        let colors = cx.colors();

        // `.list-box` is `relative w-full overflow-clip p-1` with `mt-1` between
        // children, and nothing else: the popover around it paints the panel.
        // This used to draw its own surface, border and radius, which put a
        // second panel inside every picker.
        let mut list = div()
            .id(self.id.clone())
            // `react-aria/dist/private/listbox/useListBox.mjs` is one literal
            // `role: 'listbox'` with `'aria-orientation': orientation`, which
            // defaults to vertical and is the only axis this port's list
            // lays out on. `aria-multiselectable` is the recorded omission in
            // `crate::a11y`: gpui has no builder for it.
            .a11y(a11y::Role::ListBox)
            .a11y_orientation(herogpui_core::Orientation::Vertical)
            .relative()
            .w_full()
            .flex()
            .flex_col()
            .gap(self.gap)
            .p(self.padding)
            .overflow_hidden()
            .text_color(colors.foreground)
            .track_focus(&focus_handle)
            .key_context("ListBox")
            // A click has to move the keyboard's focus onto the list, or the
            // arrow keys would go nowhere after a pointer selection.
            .on_mouse_down(gpui::MouseButton::Left, {
                let fh = focus_handle.clone();
                move |_, window, cx| window.focus(&fh, cx)
            });

        // A virtualized list scrolls inside its uniform `VirtualList`, which owns the
        // scroll offset it computes the visible range from; a second scroller
        // around it would move the rows without telling it.
        if let (Some(max_h), None) = (self.max_h, self.row_height) {
            list = list
                .max_h(max_h)
                .overflow_y_scroll()
                .track_scroll(&box_scroll_now);
        }

        // The rows a keyboard can land on: an item that is not disabled.
        // Sections and separators are skipped, so the cursor never stops on
        // something that cannot be chosen.
        let stops: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| match item {
                ListBoxItem::Option {
                    key, is_disabled, ..
                } => !is_disabled && !self.disabled_keys.contains(key),
                _ => false,
            })
            .map(|(i, _)| i)
            .collect();

        if let Some(stale) = cursor_at.filter(|index| !stops.contains(index)) {
            cursor_at = stops
                .iter()
                .copied()
                .find(|index| *index > stale)
                .or_else(|| stops.iter().rev().copied().find(|index| *index < stale))
                .or_else(|| stops.first().copied());
            cursor.update(cx, |value, cx| {
                *value = cursor_at;
                cx.notify();
            });
        }

        // React Aria moves collection focus on entry to the first selected
        // option, falling back to the first enabled option. Keep the keyed
        // cursor untouched until the user actually navigates, but use that
        // entry stop for focus styling and immediate Enter/Space activation.
        let has_focus = focus_handle.is_focused(window);
        if cursor_at.is_none() && has_focus {
            cursor_at = stops
                .iter()
                .copied()
                .find(|index| match self.items.get(*index) {
                    Some(ListBoxItem::Option { key, .. }) => self.selected_keys.contains(key),
                    _ => false,
                })
                .or_else(|| stops.first().copied());
        }
        let focused_at = (window.is_window_active() && has_focus)
            .then_some(cursor_at)
            .flatten();
        // The headless window may be inactive while its list still holds
        // keyboard focus. Placement follows the active row regardless of
        // focus-ring modality or window activation.
        let anchor_at = has_focus.then_some(cursor_at).flatten();

        if !stops.is_empty() || !self.selected_keys.is_empty() {
            let held = cursor.clone();
            let stops_for_keys = stops;
            let wrap = self.should_focus_wrap;
            let fixed_virtual = self.row_height.is_some();
            // Pinned `ListKeyboardDelegate` pages by one visible rectangle, so
            // the step reads the virtual list's own laid-out viewport -- the
            // uniform handle's `viewport_bounds()` -- and not the configured
            // `max_h` cap: a bounded parent (or a resized window) shows fewer
            // rows than the cap allows. A zero viewport answers nothing, which
            // the shared resolver turns into no movement.
            let fixed_row_height = self.row_height;
            let variable_scroll = self
                .row_height
                .is_none()
                .then_some(self.estimated_row_height)
                .flatten()
                .map(|_| list_state_now.clone());
            let variable_heights = variable_row_heights.clone();
            let variable_estimate = self.estimated_row_height;
            let plain_rows = self.row_height.is_none() && self.estimated_row_height.is_none();
            let plain_scrollable = plain_rows && self.max_h.is_some();
            let key_list_scroll = list_scroll_now.clone();
            let key_box_scroll = box_scroll_now;
            let keys: Vec<SharedString> = self
                .items
                .iter()
                .map(|item| item.key().cloned().unwrap_or_default())
                .collect();
            // Every row's text, so typeahead can search it. A row that cannot be
            // landed on has no label here, so it is never a match.
            let labels: Vec<String> = self
                .items
                .iter()
                .map(|item| match item {
                    ListBoxItem::Option { label, .. } => label.to_string(),
                    _ => String::new(),
                })
                .collect();
            let typed_keys = typed;
            let mode = self.selection_mode;
            let disallow_empty = self.disallow_empty_selection;
            let escape_key_behavior = self.escape_key_behavior;
            let selected_now = self.selected_keys.clone();
            let on_selection_change = self.on_selection_change.clone();
            let on_action = self.on_action.clone();
            let selection_own_for_keys = selection_own.clone();
            let selection_range_for_keys = selection_range.clone();
            let interaction_for_keys = interaction.clone();
            let entry_at = cursor_at;
            let selectable_keys: HashSet<SharedString> = stops_for_keys
                .iter()
                .filter_map(|index| keys.get(*index).cloned())
                .collect();
            list = list.on_key_down(move |event, window, cx| {
                let from = (*held.read(cx))
                    .filter(|index| stops_for_keys.contains(index))
                    .or(entry_at.filter(|index| stops_for_keys.contains(index)));
                let key_name = event.keystroke.key.as_str();
                let is_mac = cx.platform_info().is_mac();
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
                    && mode == SelectionMode::Multiple
                {
                    let next: HashSet<SharedString> = stops_for_keys
                        .iter()
                        .filter_map(|index| keys.get(*index).cloned())
                        .collect();
                    let all_selected = next.iter().all(|key| selected_now.contains(key));
                    if !all_selected {
                        if let Some(held) = &selection_own_for_keys {
                            held.update(cx, |value, cx| {
                                *value = next.clone();
                                cx.notify();
                            });
                        }
                        if let Some(cb) = &on_selection_change {
                            cb(&next, window, cx);
                        }
                        selection_range_for_keys.update(cx, |range, _| {
                            range.anchor = None;
                            range.current = None;
                            range.is_all = true;
                        });
                    }
                    cx.stop_propagation();
                    return;
                }
                // Pinned `useSelectableCollection` clears a nonempty selection
                // on Escape by default and leaves an empty collection alone.
                if key_name == "escape"
                    && !event.keystroke.modifiers.modified()
                    && escape_key_behavior == EscapeKeyBehavior::ClearSelection
                    && crate::selection::reports_changes(mode)
                    && !disallow_empty
                    && !selected_now.is_empty()
                {
                    let next = HashSet::new();
                    if let Some(held) = &selection_own_for_keys {
                        held.update(cx, |value, cx| {
                            *value = next.clone();
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &on_selection_change {
                        cb(&next, window, cx);
                    }
                    selection_range_for_keys.update(cx, |range, _| {
                        *range = ListBoxSelectionRange::default();
                    });
                    cx.stop_propagation();
                    return;
                }
                let page_by_step = |from: usize, step: usize| match key_name {
                    "pagedown" => {
                        let boundary = from.saturating_add(step).min(keys.len() - 1);
                        stops_for_keys
                            .iter()
                            .copied()
                            .find(|stop| *stop >= boundary)
                            .or_else(|| stops_for_keys.last().copied())
                    }
                    "pageup" => {
                        let boundary = from.saturating_sub(step);
                        stops_for_keys
                            .iter()
                            .rev()
                            .copied()
                            .find(|stop| *stop <= boundary)
                            .or_else(|| stops_for_keys.first().copied())
                    }
                    _ => None,
                };
                let fixed_page_move = from.and_then(|from| {
                    let row_height = fixed_row_height?;
                    let viewport_height = f32::from(key_list_scroll.viewport_bounds().size.height);
                    if viewport_height <= 0. {
                        return None;
                    }
                    let step = ((viewport_height / f32::from(row_height)).ceil() as usize)
                        .saturating_sub(1);
                    page_by_step(from, step)
                });
                let variable_page_move = from.and_then(|from| {
                    let viewport_height = variable_scroll.as_ref()?.viewport_bounds().size.height;
                    let heights = variable_heights.as_ref()?.read(cx);
                    let estimate = variable_estimate?;
                    let height_at =
                        |index: usize| heights.1.get(index).copied().flatten().unwrap_or(estimate);
                    let mut distance = height_at(from);
                    let mut target = from;
                    match key_name {
                        "pagedown" => {
                            if distance >= viewport_height {
                                return Some(target);
                            }
                            let boundary = viewport_height - distance;
                            distance = px(0.);
                            for next in stops_for_keys.iter().copied().filter(|next| *next > from) {
                                for index in target..next {
                                    distance += height_at(index);
                                }
                                target = next;
                                if distance >= boundary {
                                    break;
                                }
                            }
                            Some(target)
                        }
                        "pageup" => {
                            if distance >= viewport_height {
                                return Some(target);
                            }
                            for previous in stops_for_keys
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
                let is_variable_page = fixed_page_move.is_none() && variable_page_move.is_some();
                let plain_page_move = from.filter(|_| plain_rows).and_then(|from| {
                    if !plain_scrollable {
                        return match key_name {
                            "pagedown" => stops_for_keys.last().copied(),
                            "pageup" => stops_for_keys.first().copied(),
                            _ => None,
                        };
                    }
                    let current = key_box_scroll.bounds_for_item(from)?;
                    let viewport_height = key_box_scroll.bounds().size.height;
                    let target = match key_name {
                        "pagedown" => current.top() - current.size.height + viewport_height,
                        "pageup" => current.top() + current.size.height - viewport_height,
                        _ => return None,
                    };
                    match key_name {
                        "pagedown" => stops_for_keys
                            .iter()
                            .copied()
                            .filter(|stop| *stop >= from)
                            .find(|stop| {
                                key_box_scroll
                                    .bounds_for_item(*stop)
                                    .is_some_and(|bounds| bounds.top() >= target)
                            })
                            .or_else(|| stops_for_keys.last().copied()),
                        "pageup" => stops_for_keys
                            .iter()
                            .rev()
                            .copied()
                            .filter(|stop| *stop <= from)
                            .find(|stop| {
                                key_box_scroll
                                    .bounds_for_item(*stop)
                                    .is_some_and(|bounds| bounds.top() <= target)
                            })
                            .or_else(|| stops_for_keys.first().copied()),
                        _ => None,
                    }
                });
                let page_move = fixed_page_move
                    .or(variable_page_move)
                    .or(plain_page_move)
                    .filter(|next| Some(*next) != from);
                let navigation = page_move.map_or_else(
                    || crate::list_nav::resolve(&stops_for_keys, from, key_name, wrap),
                    crate::list_nav::Move::To,
                );
                match navigation {
                    crate::list_nav::Move::To(next) => {
                        let modifiers = event.keystroke.modifiers;
                        // Pinned `useSelectableCollection`: Shift extends a
                        // multiple selection from the anchor with no other
                        // chord, so plain Shift navigation is exact.
                        //
                        // The pinned registrations install no Home/End
                        // handler for an unregistered chord -- Cmd- or
                        // Ctrl-bearing on macOS, Alt- or platform-bearing
                        // elsewhere -- so the whole event stays inert: no
                        // focus move, no selection, no preventDefault.
                        if matches!(key_name, "home" | "end")
                            && !home_end_registered(modifiers, is_mac)
                        {
                            return;
                        }
                        let exact_shift_navigation = if is_mac {
                            !modifiers.control && !modifiers.platform && !modifiers.function
                        } else {
                            !modifiers.alt && !modifiers.platform && !modifiers.function
                        };
                        let extends_selection = modifiers.shift
                            && mode == SelectionMode::Multiple
                            && exact_shift_navigation
                            && shift_home_end_extends(key_name, modifiers.control, is_mac)
                            && Some(next) != from;
                        if extends_selection {
                            if let Some(target) = keys.get(next) {
                                let range = selection_range_for_keys.read(cx).clone();
                                let next_selection = extend_selection_range(
                                    &selected_now,
                                    &keys,
                                    &selectable_keys,
                                    &range,
                                    target,
                                );
                                selection_range_for_keys.update(cx, |range, _| {
                                    if range.anchor.is_none() {
                                        range.anchor = Some(target.clone());
                                    }
                                    range.current = Some(target.clone());
                                    range.is_all = false;
                                });
                                if next_selection != selected_now {
                                    if let Some(held) = &selection_own_for_keys {
                                        held.update(cx, |value, cx| {
                                            *value = next_selection.clone();
                                            cx.notify();
                                        });
                                    }
                                    if let Some(cb) = &on_selection_change {
                                        cb(&next_selection, window, cx);
                                    }
                                }
                            }
                        }
                        held.update(cx, |v, cx| {
                            *v = Some(next);
                            cx.notify();
                        });
                        if fixed_virtual {
                            key_list_scroll.scroll_to_item(next, crate::VirtualListScroll::Center);
                        } else if let Some(state) = &variable_scroll {
                            if is_variable_page || matches!(key_name, "up" | "down") {
                                state.scroll_to_reveal_item(next);
                            } else {
                                state.scroll_to(gpui::ListOffset {
                                    item_ix: next,
                                    offset_in_item: px(0.),
                                });
                            }
                        } else {
                            key_box_scroll.scroll_to_item(next);
                        }
                    }
                    crate::list_nav::Move::Activate => {
                        let Some(index) = from else {
                            return;
                        };
                        let Some(item_key) = keys.get(index).cloned() else {
                            return;
                        };
                        if crate::selection::reports_changes(mode) || on_action.is_some() {
                            if let Some(slot) = interaction_for_keys.get(index) {
                                util::begin_keyboard_press(slot, event, window, cx);
                            }
                        }
                        let action_key = event.keystroke.key == "enter";
                        let has_primary_action = on_action.is_some()
                            && (mode == SelectionMode::None || selected_now.is_empty());
                        if action_key && has_primary_action {
                            if let Some(cb) = &on_action {
                                cb(&item_key, window, cx);
                                return;
                            }
                        }
                        if action_key && on_action.is_some() {
                            return;
                        }
                        if crate::selection::reports_changes(mode) {
                            let was_selected = selected_now.contains(&item_key);
                            let next = match mode {
                                SelectionMode::None => selected_now.clone(),
                                SelectionMode::Single => {
                                    if selected_now.contains(&item_key) && !disallow_empty {
                                        HashSet::new()
                                    } else {
                                        HashSet::from([item_key.clone()])
                                    }
                                }
                                SelectionMode::Multiple => {
                                    let mut set = selected_now.clone();
                                    if set.remove(&item_key) {
                                        if disallow_empty && set.is_empty() {
                                            set.insert(item_key.clone());
                                        }
                                    } else {
                                        set.insert(item_key.clone());
                                    }
                                    set
                                }
                            };
                            if next != selected_now {
                                if let Some(held) = &selection_own_for_keys {
                                    held.update(cx, |value, cx| {
                                        *value = next.clone();
                                        cx.notify();
                                    });
                                }
                                if let Some(cb) = &on_selection_change {
                                    cb(&next, window, cx);
                                }
                            }
                            if mode == SelectionMode::Multiple && !was_selected {
                                selection_range_for_keys.update(cx, |range, _| {
                                    range.anchor = Some(item_key.clone());
                                    range.current = Some(item_key.clone());
                                    range.is_all = false;
                                });
                            } else if mode == SelectionMode::Multiple {
                                selection_range_for_keys.update(cx, |range, _| {
                                    if range.is_all {
                                        *range = ListBoxSelectionRange::default();
                                    }
                                });
                            }
                        }
                    }
                    crate::list_nav::Move::Ignore => {
                        // Typeahead: letters jump to the row that starts with
                        // them, which is the other half of v3's keyboard.
                        if event.keystroke.modifiers.control
                            || event.keystroke.modifiers.platform
                            || event.keystroke.modifiers.alt
                        {
                            return;
                        }
                        let key = key_name;
                        if !crate::list_nav::is_typeahead_key(key) {
                            return;
                        }
                        let now = web_time::Instant::now();
                        let (query, repeat) = typed_keys.update(cx, |t, _| {
                            let query = t.push(key, now);
                            (query, t.is_repeat())
                        });
                        if let Some(found) = crate::list_nav::typeahead(
                            &labels,
                            &stops_for_keys,
                            from,
                            &query,
                            repeat,
                        ) {
                            held.update(cx, |v, cx| {
                                *v = Some(found);
                                cx.notify();
                            });
                        }
                    }
                }
            });
        }

        // The virtual paths below move `self` into their row builders, so the
        // slot comes out first: it refines whichever path returns.
        let sx = self.sx.take();

        // With `rowHeight` set the list is virtual: only the rows the viewport
        // shows are built, which is what makes a thousand of them affordable.
        // A uniform `VirtualList` measures row 0 and multiplies, so the row builder is
        // told the height rather than left to size itself.
        // `estimatedRowHeight` virtualizes a list whose rows differ: gpui's
        // `list` measures each row it builds, where the uniform mode measures one
        // and multiplies. Its state is intrusive -- the caller has to hold it --
        // so it lives in the window's keyed store, and a change in the item
        // count resets it.
        if self.row_height.is_none() && self.estimated_row_height.is_some() {
            let height = self.max_h.unwrap_or(px(400.));
            let count = self.items.len();
            let rows = std::rc::Rc::new(self);
            let handle = list_handle_now;
            if handle.item_count() != count {
                handle.set_item_count(count);
            }
            let interaction = interaction.clone();
            let row_range = selection_range.clone();
            let measured_heights =
                variable_row_heights.expect("estimated row height creates a measurement store");
            return util::apply_sx(
                list.child(
                    crate::VirtualList::new(
                        element_id::scoped(&base_id, "variable-rows"),
                        &handle,
                        move |index, _window, cx| {
                            let row = rows.row(
                                index,
                                focused_at,
                                anchor_at,
                                None,
                                interaction.get(index),
                                &cursor,
                                &row_range,
                                selection_own.as_ref(),
                                _window,
                                cx,
                            );
                            let measured = measured_heights.clone();
                            div()
                                .relative()
                                .w_full()
                                .child(row)
                                .child(
                                    gpui::canvas(
                                        move |bounds: gpui::Bounds<gpui::Pixels>, _, cx| {
                                            measured.update(cx, |(_, heights), cx| {
                                                if heights.get(index).copied().flatten()
                                                    != Some(bounds.size.height)
                                                {
                                                    heights[index] = Some(bounds.size.height);
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
                ),
                &sx,
            )
            .into_any_element();
        }

        if let Some(row_height) = self.row_height {
            let height = self.max_h.unwrap_or(px(400.));
            let list_id = self.id.clone();
            let count = self.items.len();
            let rows = std::rc::Rc::new(self);
            let interaction = interaction.clone();
            let row_range = selection_range.clone();
            // The headless probe name for the virtual viewport's bounds.
            let rows_selector = format!("{base}-rows");
            // The viewport scrolls inside the uniform `VirtualList`, which
            // builds only the shown rows. A fixed height caps the roomy-window viewport
            // at the configured value so an unbounded parent sizes to cap +
            // padding instead of the rows' full natural height; `min_h_0`
            // lets that fixed height shrink as a flex item with a bounded
            // parent, handing the viewport its real height for paging.
            let handle = list_scroll_now;
            if handle.item_count() != count {
                handle.splice(0..handle.item_count(), count);
            }
            return util::apply_sx(
                list.child(
                    crate::VirtualList::new(list_id, &handle, move |i, window, cx| {
                        rows.row(
                            i,
                            focused_at,
                            anchor_at,
                            Some(row_height),
                            interaction.get(i),
                            &cursor,
                            &row_range,
                            selection_own.as_ref(),
                            window,
                            cx,
                        )
                    })
                    .height(height)
                    .debug_selector(rows_selector),
                ),
                &sx,
            )
            .into_any_element();
        }

        let mut items = Vec::with_capacity(self.items.len());
        for index in 0..self.items.len() {
            items.push(self.row(
                index,
                focused_at,
                anchor_at,
                None,
                interaction.get(index),
                &cursor,
                &selection_range,
                selection_own.as_ref(),
                window,
                cx,
            ));
        }
        util::apply_sx(list.children(items), &sx).into_any_element()
    }
}

impl ListBox {
    /// One row, by index.
    ///
    /// Shared by the plain and the virtualized paths so the two cannot drift:
    /// `fixed_h` is `Some` only for the virtual one, where every row -- a
    /// heading and a separator included -- is one `rowHeight` tall because that
    /// is the number the scroll geometry is computed from.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        index: usize,
        cursor_at: Option<usize>,
        anchor_at: Option<usize>,
        fixed_h: Option<gpui::Pixels>,
        interaction: Option<&util::Interaction>,
        cursor: &gpui::Entity<Option<usize>>,
        selection_range: &gpui::Entity<ListBoxSelectionRange>,
        selection_own: Option<&gpui::Entity<HashSet<SharedString>>>,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::AnyElement {
        let colors = cx.colors();
        // `.list-box-item` is `min-h-9`.
        let row_h = fixed_h.unwrap_or(px(36.));
        let text_size = util::FIELD_TEXT;
        let sized = |el: gpui::Div| match fixed_h {
            Some(h) => el.h(h),
            None => el,
        };
        match &self.items[index] {
            ListBoxItem::Separator => sized(
                div()
                    .my(px(4.))
                    .mx(gpui::relative(0.03))
                    .w(gpui::relative(0.94))
                    .h(cx.layout().border_width)
                    .bg(colors.separator),
            )
            .into_any_element(),
            ListBoxItem::Section(label) => sized(
                div()
                    .when_some(self.heading_height, |el, h| el.h(h))
                    .px(px(8.))
                    .pt(px(6.))
                    .pb(px(4.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.muted)
                    .child(label.to_string()),
            )
            .into_any_element(),
            ListBoxItem::Option {
                key,
                label,
                description,
                icon,
                shortcut,
                variant,
                is_disabled,
            } => {
                let variant = if *variant == ListBoxItemVariant::Default {
                    self.variant
                } else {
                    *variant
                };
                let disabled = *is_disabled || self.disabled_keys.contains(key);
                let selected = self.selected_keys.contains(key);
                let pressable = !disabled
                    && (crate::selection::reports_changes(self.selection_mode)
                        || self.on_action.is_some());

                let fg = match variant {
                    ListBoxItemVariant::Default => colors.foreground,
                    ListBoxItemVariant::Danger => colors.danger.color,
                };
                let hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);

                // `useOption.mjs` is `role: 'option'` with
                // `'aria-selected': selectionMode !== 'none' ? isSelected :
                // undefined`, and it adds `aria-posinset`/`aria-setsize`
                // only `if (isVirtualized)` — counted over the *options*
                // (`getItemCount`), which skips the sections and separators
                // that carry no node. The port's virtual paths are the same
                // situation: only a window of rows is built, so the position
                // cannot be counted from the tree.
                let virtualized = self.row_height.is_some() || self.estimated_row_height.is_some();
                let option_count = self
                    .items
                    .iter()
                    .filter(|item| matches!(item, ListBoxItem::Option { .. }))
                    .count();
                let option_index = self.items[..index]
                    .iter()
                    .filter(|item| matches!(item, ListBoxItem::Option { .. }))
                    .count();
                let row_name = a11y::Name::labelled(label.clone()).described(description.clone());
                let mut row = div()
                    .id(element_id::indexed(&self.id, "item", index))
                    // Headless probe: the option row itself, so tests can
                    // measure the row padding on the plain and virtual paths.
                    .debug_selector({
                        let owner = format!("{:?}", self.id);
                        move || format!("{owner}-item-{index}")
                    })
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .px(self.row_padding_x.unwrap_or(px(8.)))
                    .relative()
                    // A virtual row is laid out on its own, so it takes the width
                    // it is given rather than inheriting a stretch.
                    .map(|el| match fixed_h {
                        Some(h) => el.h(h).w_full(),
                        None => el.min_h(row_h),
                    })
                    .py(self.row_padding_y.unwrap_or(px(6.)))
                    .rounded(util::soft_radius(cx))
                    .text_size(text_size)
                    .line_height(px(20.))
                    .text_color(fg);

                if disabled {
                    row = row.opacity(cx.layout().disabled_opacity);
                } else {
                    row = row
                        .cursor(util::interactive_cursor(cx))
                        .hover(move |s| s.bg(hover_bg));
                }

                // `.list-box-item` takes `status-focused` on the row the keyboard
                // is on. A ring rather than a border: a border would move the
                // row's content by two pixels as the cursor arrived. Pointer
                // hover seats the cursor without painting the ring. The ring is
                // an overlay child so its corner stays concentric with the
                // row's `soft_radius`; the shadow variant would keep the row's
                // own radius two pixels further out and blur the edge. The
                // overlay reaches four pixels past the row, which is exactly
                // the list's `p-1` plus its border band, so the enclosing
                // `overflow_hidden` list does not cut it -- and clips the
                // shadow spread to the same box anyway.
                let row = util::with_focus_ring_overlay(
                    row,
                    util::shows_focus_ring(cursor_at == Some(index), cx),
                    true,
                    util::soft_radius(cx),
                    Vec::new(),
                    cx,
                );
                let mut row = row;

                if let Some(path) = icon {
                    row = row.child(
                        gpui::svg()
                            .size(util::FIELD_ICON)
                            .path(path.clone())
                            .flex_shrink_0()
                            .text_color(fg),
                    );
                }

                // Label plus optional description stack -- or the render
                // function, which v3 hands the row's state. The content branch
                // falls through to the click handler below: `ListBox.Item`
                // stays a row whether its children are a node or a function.
                if let Some(render) = &self.item_content {
                    let focused = cursor_at == Some(index);
                    // The slot's press is a frame behind the pointer, because
                    // gpui reports it to a handler rather than to the render
                    // that draws it. v3's `ListBox.Item` render-props table
                    // lists no `isHovered`, so the hover the slot also tracks
                    // is not handed over.
                    let (_, recorded_press) =
                        interaction.map(|slot| *slot.read(cx)).unwrap_or_default();
                    row = row.child(render(
                        key,
                        util::InteractiveState {
                            is_hovered: false,
                            is_pressed: pressable && recorded_press,
                            is_focused: focused,
                            is_focus_visible: focused && util::focus_visible(cx),
                            is_selected: selected,
                            is_disabled: disabled,
                            is_pending: false,
                            is_indeterminate: false,
                        },
                    ));
                } else {
                    // Headless probe: the label column starts at the row's
                    // content edge, so tests can measure row padding.
                    let owner = format!("{:?}", self.id);
                    row = row.child(
                        div()
                            .debug_selector(move || format!("{owner}-item-{index}-label"))
                            .flex()
                            .flex_col()
                            .flex_1()
                            // `[data-slot="description"]` is `text-wrap`, which
                            // needs a column that can shrink below its content.
                            .min_w_0()
                            .child(div().font_weight(gpui::FontWeight::MEDIUM).child(label.to_string()))
                            .when_some(description.clone(), |el, d| {
                                el.child(
                                    div()
                                        .text_size(px(12.)).line_height(px(16.))
                                        .text_color(colors.muted)
                                        .child(d.to_string()),
                                )
                            }),
                    );
                }

                if let Some(render) = &self.indicator {
                    // HeroUI's `.list-box-item__indicator` is absolute at the
                    // inline end, with `pe-7` reserved on the row whenever an
                    // indicator exists. Keeping the slot out of flex flow
                    // prevents a long label from changing its width or
                    // pushing the checkmark away from the row edge.
                    row = row.pr(px(28.)).child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right(px(8.))
                            .w(px(16.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(render(selected)),
                    );
                } else if selected && self.selection_mode != SelectionMode::None {
                    row = row.pr(px(28.)).child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right(px(8.))
                            .w(px(16.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(gpui::svg()
                                    // `.list-box-item__indicator` is `size-4`.
                                    .size(px(16.))
                                    .path(icons::CHECK)
                                    .text_color(match variant {
                                        ListBoxItemVariant::Default => colors.default.foreground,
                                        ListBoxItemVariant::Danger => colors.danger.color,
                                    })),
                    );
                } else if let Some(sc) = shortcut {
                    row = row.child(
                        crate::kbd::Kbd::new()
                            .variant(crate::kbd::KbdVariant::Light)
                            .child(sc.to_string()),
                    );
                }

                if pressable {
                    // Wrap first so the stable press slot owns the hitbox;
                    // then record pointer/keyboard state on that slot for
                    // both built-in rows and caller render props.
                    row = crate::anim::pressed_with_background_ramp(
                        row,
                        crate::anim::PressBox {
                            height: row_h,
                            padding_x: Some(self.row_padding_x.unwrap_or(px(8.))),
                            width: None,
                            min_width: None,
                            text_size,
                            line_height: px(20.),
                            gap: px(12.),
                            radius: util::soft_radius(cx),
                            shrink_x: false,
                            scale: crate::anim::PRESSED_SCALE_SUBTLE,
                        },
                        None,
                        crate::anim::LIST_ITEM_PRESS,
                        interaction,
                        window,
                        cx,
                    );
                    if let Some(slot) = interaction {
                        row = util::track_interaction(row, slot);
                    }
                }

                row = row
                    .a11y_named(a11y::Role::ListBoxOption, &row_name)
                    .when(self.selection_mode != SelectionMode::None, |row| {
                        row.a11y_selected(selected)
                    })
                    .when(virtualized, |row| {
                        row.a11y_set_position(option_index, option_count)
                    })
                    // The list holds one focus handle and moves a cursor
                    // through the rows, which is `shouldUseVirtualFocus` in
                    // upstream's terms. gpui states that relation on the
                    // descendant rather than on the container, so the row the
                    // cursor is on says so itself.
                    .when(cursor_at == Some(index), |row| row.a11y_active_descendant());

                if !disabled {
                    let key = key.clone();
                    let mode = self.selection_mode;
                    let disallow_empty = self.disallow_empty_selection;
                    let current = self.selected_keys.clone();
                    let on_selection_change = self.on_selection_change.clone();
                    let on_action = self.on_action.clone();
                    let moved = cursor.clone();
                    let selection_own = selection_own.cloned();
                    let range_for_click = selection_range.clone();
                    // The collection order the range resolves against -- every
                    // option's key, disabled ones included: they keep their
                    // collection positions so the span traversal preserves
                    // indexes, while the `selectable` filter keeps their
                    // insertions out of the range.
                    let collection: Vec<SharedString> = self
                        .items
                        .iter()
                        .filter_map(|item| item.key().cloned())
                        .collect();
                    let selectable: HashSet<SharedString> = self
                        .items
                        .iter()
                        .filter_map(|item| match item {
                            ListBoxItem::Option {
                                key, is_disabled, ..
                            } => (!is_disabled && !self.disabled_keys.contains(key))
                                .then(|| key.clone()),
                            _ => None,
                        })
                        .collect();
                    row = row
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                            moved.update(cx, |value, cx| {
                                *value = Some(index);
                                cx.notify();
                            });
                        })
                        .on_click(move |ev, window, cx| {
                            let has_primary_action = on_action.is_some()
                                && (mode == SelectionMode::None || current.is_empty());
                            if has_primary_action {
                                if let Some(action) = &on_action {
                                    action(&key, window, cx);
                                }
                                return;
                            }
                            if crate::selection::reports_changes(mode) {
                                let was_selected = current.contains(&key);
                                // A Shift click extends from the anchor in
                                // multiple mode; an ordinary click toggles and
                                // seats the range on itself. A controlled
                                // selection only reports; the owner's prop
                                // stays in charge until it feeds the value
                                // back, while the range keeps advancing.
                                let extends_selection =
                                    ev.modifiers().shift && mode == SelectionMode::Multiple;
                                let next = if extends_selection {
                                    let range = range_for_click.read(cx).clone();
                                    extend_selection_range(
                                        &current,
                                        &collection,
                                        &selectable,
                                        &range,
                                        &key,
                                    )
                                } else {
                                    match mode {
                                        SelectionMode::None => current.clone(),
                                        SelectionMode::Single => {
                                            if current.contains(&key) && !disallow_empty {
                                                HashSet::new()
                                            } else {
                                                HashSet::from([key.clone()])
                                            }
                                        }
                                        SelectionMode::Multiple => {
                                            let mut set = current.clone();
                                            if set.remove(&key) {
                                                if disallow_empty && set.is_empty() {
                                                    set.insert(key.clone());
                                                }
                                            } else {
                                                set.insert(key.clone());
                                            }
                                            set
                                        }
                                    }
                                };
                                if next != current {
                                    if let Some(held) = &selection_own {
                                        held.update(cx, |value, cx| {
                                            *value = next.clone();
                                            cx.notify();
                                        });
                                    }
                                    if let Some(change) = &on_selection_change {
                                        change(&next, window, cx);
                                    }
                                }
                                if extends_selection {
                                    range_for_click.update(cx, |range, _| {
                                        if range.anchor.is_none() {
                                            range.anchor = Some(key.clone());
                                        }
                                        range.current = Some(key.clone());
                                        range.is_all = false;
                                    });
                                } else if mode == SelectionMode::Multiple {
                                    if was_selected {
                                        // A deselect ends a raw `all`, so the
                                        // next Shift click extends instead of
                                        // collapsing to its target.
                                        range_for_click.update(cx, |range, _| {
                                            if range.is_all {
                                                *range = ListBoxSelectionRange::default();
                                            }
                                        });
                                    } else {
                                        range_for_click.update(cx, |range, _| {
                                            range.anchor = Some(key.clone());
                                            range.current = Some(key.clone());
                                            range.is_all = false;
                                        });
                                    }
                                }
                            }
                        });
                }

                if anchor_at == Some(index)
                    && let Some(focus) = window.focused(cx)
                {
                    row = util::record_focus_bounds(row, &focus, window, cx);
                }
                row.into_any_element()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn implementation_source() -> &'static str {
        include_str!("list_box.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present")
    }

    #[test]
    fn selected_indicators_are_absolute_and_reserve_end_padding() {
        let source = implementation_source();
        assert!(source.contains(".relative()"));
        assert!(source.contains(".pr(px(28.))"));
        assert!(source.contains(".absolute()"));
        assert!(source.contains(".right(px(8.))"));
        assert!(source.contains(".top_0()"));
        assert!(source.contains(".bottom_0()"));
    }

    #[test]
    fn ordinary_rows_use_the_pinned_subtle_press_skin() {
        let source = implementation_source();
        assert!(source.contains("PRESSED_SCALE_SUBTLE"));
        assert!(source.contains("util::track_interaction(row, slot)"));
        assert!(source.contains("crate::anim::pressed_with_background_ramp("));
        assert!(source.contains("crate::anim::LIST_ITEM_PRESS"));
        assert!(source.contains("shrink_x: false"));
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

    /// Arrows and page keys never consult the Home/End gate: their forbidden
    /// extra chords are rejected earlier, by `exact_shift_navigation`.
    #[test]
    fn shift_navigation_keys_do_not_consult_the_home_end_gate() {
        for key in ["up", "down", "left", "right", "pageup", "pagedown"] {
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
}

crate::util::impl_component_styled!(ListBox);
