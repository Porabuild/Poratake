//! Dropdown & Menu — port of `@heroui/dropdown`, `@heroui/menu` and
//! `@heroui/listbox`.

use gpui::{
    prelude::FluentBuilder as _, px, AnyElement, App, Bounds, ClickEvent, InteractiveElement,
    IntoElement, ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window,
};
use herogpui_core::{element_id, SelectionMode};
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};
use crate::icons;

/// One entry of a dropdown menu.
pub enum MenuItem {
    /// Section caption (`<MenuSection>` title).
    SectionLabel(SharedString),
    /// A divider row between items.
    Separator,
    /// A menu row.
    ///
    /// `#[non_exhaustive]`: build one through [`MenuItem::new`] and its
    /// builders, which cover every field, so a later row capability is an
    /// additive change rather than a break for literal construction.
    #[non_exhaustive]
    Item {
        /// Stable key identifying the item.
        key: SharedString,
        /// Visible text of the item.
        label: SharedString,
        /// Optional shortcut hint shown on the row.
        shortcut: Option<SharedString>,
        /// Optional icon asset path.
        icon: Option<&'static str>,
        /// Whether the item uses danger styling.
        is_danger: bool,
        /// `Description` inside a `Dropdown.Item` — v3's "With Descriptions".
        description: Option<SharedString>,
        /// `Dropdown.SubmenuTrigger` — the rows this item opens. The row grows a
        /// trailing indicator and the panel appears beside it.
        submenu: Vec<MenuItem>,
        /// Whether the row's own content owns the interaction, in place of the
        /// row acting as one button. See [`MenuItem::is_interactive`].
        is_interactive: bool,
    },
}

impl MenuItem {
    /// Creates a menu item from a key and a label.
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        MenuItem::Item {
            key: key.into(),
            label: label.into(),
            shortcut: None,
            icon: None,
            is_danger: false,
            description: None,
            submenu: Vec::new(),
            is_interactive: false,
        }
    }

    /// `Description` — the second line v3 composes inside an item.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        if let MenuItem::Item { description, .. } = &mut self {
            *description = Some(text.into());
        }
        self
    }

    /// Hands the row's interaction to the element `Menu::item_content` draws
    /// for it, in place of the row behaving as one button.
    ///
    /// HeroUI composes rows that carry an inline secondary control — a small
    /// select, a stepper — on the trailing edge, where pressing that control
    /// must not also pick the row and close the menu. A stock row attaches an
    /// unconditional click that reports the action and dismisses, plus the
    /// `[data-pressed]` 98% scale over the whole row, so a hosted control is
    /// unusable inside one. Setting this drops all three, and Enter and Space
    /// stop activating the row, leaving the hosted element to answer the
    /// pointer and the keyboard itself.
    ///
    /// What it keeps: the row stays a keyboard stop with its hover fill,
    /// highlight and focus ring, and it keeps its `menuitem` role and
    /// accessible name, so arrowing through the menu is unchanged.
    ///
    /// Nothing else is needed to host a popup-opening control such as a
    /// [`crate::select::Select`]. Such a control registers its own panel on the
    /// shared overlay stack, which makes it topmost, and the menu's
    /// outside-press and Escape dismissals are already gated on being topmost —
    /// so they stand down for as long as the inner panel is open.
    ///
    /// Ignored together with [`MenuItem::submenu`]: a submenu trigger already
    /// opts out of the row click, and its flyout is the interaction.
    pub fn is_interactive(mut self, interactive: bool) -> Self {
        if let MenuItem::Item { is_interactive, .. } = &mut self {
            *is_interactive = interactive;
        }
        self
    }

    /// `Dropdown.SubmenuTrigger` — the rows this item opens.
    pub fn submenu(mut self, items: Vec<MenuItem>) -> Self {
        if let MenuItem::Item { submenu, .. } = &mut self {
            *submenu = items;
        }
        self
    }

    /// Sets the shortcut hint shown on the row.
    pub fn shortcut(mut self, s: impl Into<SharedString>) -> Self {
        if let MenuItem::Item { shortcut, .. } = &mut self {
            *shortcut = Some(s.into());
        }
        self
    }

    /// Sets the icon asset path.
    pub fn icon(mut self, path: &'static str) -> Self {
        if let MenuItem::Item { icon, .. } = &mut self {
            *icon = Some(path);
        }
        self
    }

    /// Marks the item as a danger item.
    pub fn danger(mut self) -> Self {
        if let MenuItem::Item { is_danger, .. } = &mut self {
            *is_danger = true;
        }
        self
    }
}

type OnSelect = std::sync::Arc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;

/// Menu panel (`<Menu>` / `<Listbox>`).
/// `type` on `Dropdown.ItemIndicator` — how a selected item is marked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IndicatorKind {
    /// A checkmark indicator.
    #[default]
    Checkmark,
    /// A dot indicator.
    Dot,
}

impl IndicatorKind {
    /// Every indicator kind, in display order.
    pub const ALL: [IndicatorKind; 2] = [IndicatorKind::Checkmark, IndicatorKind::Dot];

    /// The display name of this indicator kind.
    pub fn label(self) -> &'static str {
        match self {
            IndicatorKind::Checkmark => "Checkmark",
            IndicatorKind::Dot => "Dot",
        }
    }
}

/// `onSelectionChange` — the whole selection after an item is activated.
pub type OnSelectionChange =
    std::sync::Arc<dyn Fn(&[SharedString], &mut Window, &mut App) + 'static>;

type ItemContent =
    std::sync::Arc<dyn Fn(&SharedString, crate::util::InteractiveState) -> AnyElement + 'static>;
type ItemIndicatorContent =
    std::sync::Arc<dyn Fn(&SharedString, bool, bool) -> AnyElement + 'static>;
type ItemStartContent = std::sync::Arc<
    dyn Fn(&SharedString, crate::util::InteractiveState) -> Option<AnyElement> + 'static,
>;
type OnDismiss = std::rc::Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;
type PanelBounds = std::rc::Rc<std::cell::RefCell<Vec<Bounds<Pixels>>>>;

/// A menu panel listing `MenuItem` rows.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Menu {
    /// Set by `Dropdown` while the menu is playing its `[data-exiting]` run.
    exiting: bool,
    /// Submenus are already inside their parent's deferred draw and cannot
    /// defer a second time.
    deferred: bool,
    panel_bounds: Option<PanelBounds>,
    focus_first: Option<gpui::Entity<Option<crate::list_core::OpenFocus>>>,
    on_back: Option<std::sync::Arc<dyn Fn(&mut Window, &mut App) + 'static>>,
    /// `children` on `Dropdown.Item` — v3's render prop, handed the item's
    /// key, selection, focus, disabled and pressed state.
    item_content: Option<ItemContent>,
    /// `children` on `Dropdown.ItemIndicator` — handed the item's key,
    /// `isSelected` and `isIndeterminate`.
    indicator_content: Option<ItemIndicatorContent>,
    /// The leading element a row composes before its label, in place of
    /// the item's icon asset.
    item_start_content: Option<ItemStartContent>,
    id: gpui::ElementId,
    items: Vec<MenuItem>,
    selected_key: Option<SharedString>,
    selection_mode: SelectionMode,
    selected_keys: Vec<SharedString>,
    default_selected_keys: Vec<SharedString>,
    selection_is_controlled: bool,
    disallow_empty_selection: bool,
    disabled_keys: Vec<SharedString>,
    indicator: IndicatorKind,
    on_selection_change: Option<OnSelectionChange>,
    on_action: Option<OnSelect>,
    /// The fill a hovered menu row takes, in place of `--default`.
    row_hover_bg: Option<gpui::Hsla>,
    row_hover_foreground: Option<gpui::Hsla>,
    /// The panel's corner radius, in place of the owning `container_radius`
    /// helper. [`Dropdown`] forwards its own override here.
    radius: Option<Pixels>,
    panel_min_width: Option<Pixels>,
    panel_max_width: Option<Pixels>,
    panel_max_height: Option<Pixels>,
    row_height: Option<Pixels>,
    row_padding_x: Option<Pixels>,
    row_padding_y: Option<Pixels>,
    row_text_size: Option<Pixels>,
    row_gap: Option<Pixels>,
    panel_padding: Option<Pixels>,
    panel_gap: Option<Pixels>,
    separator_inset: Option<Pixels>,
    separator_thickness: Option<Pixels>,
    animate_entry: bool,
    animate_entry_is_set: bool,
    focus_handle: Option<gpui::FocusHandle>,
    /// Set by `Dropdown`: the menu panel is where Escape and an outside press
    /// land, and the open state belongs to the wrapper. The `bool` says
    /// whether the trigger should take the focus back: Escape, an outside
    /// press and a mouse pick can, because no key-up follows them; an Enter
    /// pick cannot, because gpui activates a focused element on key up and the
    /// trigger's click listener would reopen the menu it just closed.
    on_dismiss: Option<OnDismiss>,
    overlay_token: Option<crate::util::OverlayToken>,
    dropdown_composition: bool,
    /// Test-only label for the panel, read with `debug_bounds`.
    ///
    /// Not a v3 prop: naming the laid-out panel beats wrapping it, because a
    /// wrapper between the positioner and the menu would measure and cap
    /// while the real panel kept its natural size underneath.
    panel_debug_label: Option<&'static str>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
    recipes: Vec<SharedString>,
}

impl Menu {
    /// Creates a menu from an element id and its items.
    pub fn new(id: impl Into<gpui::ElementId>, items: Vec<MenuItem>) -> Self {
        Self {
            exiting: false,
            deferred: true,
            panel_bounds: None,
            focus_first: None,
            on_back: None,
            item_content: None,
            indicator_content: None,
            item_start_content: None,
            id: id.into(),
            items,
            selected_key: None,
            selection_mode: SelectionMode::None,
            selected_keys: Vec::new(),
            default_selected_keys: Vec::new(),
            selection_is_controlled: false,
            disallow_empty_selection: false,
            disabled_keys: Vec::new(),
            indicator: IndicatorKind::default(),
            on_selection_change: None,
            on_action: None,
            row_hover_bg: None,
            row_hover_foreground: None,
            radius: None,
            panel_min_width: None,
            panel_max_width: None,
            panel_max_height: None,
            row_height: None,
            row_padding_x: None,
            row_padding_y: None,
            row_text_size: None,
            row_gap: None,
            panel_padding: None,
            panel_gap: None,
            separator_inset: None,
            separator_thickness: None,
            animate_entry: true,
            animate_entry_is_set: false,
            focus_handle: None,
            on_dismiss: None,
            overlay_token: None,
            dropdown_composition: false,
            panel_debug_label: None,
            sx: None,
            recipes: Vec::new(),
        }
    }

    /// What to run when the menu closes: an item activation, Escape, or a
    /// press outside the panel.
    ///
    /// Not a v3 prop: v3's `Dropdown.Menu` is inside the `Dropdown` that owns
    /// `isOpen`, and React Aria's `useOverlay` closes it from there. Standalone
    /// callers remove the menu in this callback. The `bool` is whether to return
    /// the focus to the trigger — see the field docs for why a key pick passes
    /// `false`.
    pub fn on_dismiss(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(std::rc::Rc::new(f));
        self
    }

    /// Overrides the panel min width in pixels, including submenus.
    /// Unset preserves the stock metric.
    pub fn panel_min_width(mut self, value: impl Into<Pixels>) -> Self {
        self.panel_min_width = Some(value.into());
        self
    }

    /// Overrides the panel max width in pixels, including submenus.
    /// Unset preserves the stock metric.
    pub fn panel_max_width(mut self, value: impl Into<Pixels>) -> Self {
        self.panel_max_width = Some(value.into());
        self
    }

    /// Overrides the panel max height in pixels, including submenus.
    /// Unset preserves the stock metric.
    pub fn panel_max_height(mut self, value: impl Into<Pixels>) -> Self {
        self.panel_max_height = Some(value.into());
        self
    }

    /// Overrides the row minimum height in pixels, including submenus.
    /// Described rows may grow to fit their content.
    /// Unset preserves the stock metric.
    pub fn row_height(mut self, value: impl Into<Pixels>) -> Self {
        self.row_height = Some(value.into());
        self
    }

    /// Overrides the row padding x in pixels, including submenus.
    /// Unset preserves the stock metric.
    pub fn row_padding_x(mut self, value: impl Into<Pixels>) -> Self {
        self.row_padding_x = Some(value.into());
        self
    }

    /// Overrides the row padding y in pixels, including submenus.
    /// Unset preserves the stock metric.
    pub fn row_padding_y(mut self, value: impl Into<Pixels>) -> Self {
        self.row_padding_y = Some(value.into());
        self
    }

    /// Overrides the row text size in pixels, including submenus.
    /// Unset preserves the stock metric.
    pub fn row_text_size(mut self, value: impl Into<Pixels>) -> Self {
        self.row_text_size = Some(value.into());
        self
    }

    /// Overrides the row gap in pixels, including submenus.
    /// Unset preserves the stock metric.
    pub fn row_gap(mut self, value: impl Into<Pixels>) -> Self {
        self.row_gap = Some(value.into());
        self
    }

    /// Overrides the panel padding in pixels, including submenus.
    /// Unset preserves the stock metric.
    pub fn panel_padding(mut self, value: impl Into<Pixels>) -> Self {
        self.panel_padding = Some(value.into());
        self
    }

    /// Space between panel entries (default 2px), including submenus.
    /// `row_gap` independently controls spacing inside each item.
    pub fn panel_gap(mut self, gap: impl Into<Pixels>) -> Self {
        self.panel_gap = Some(gap.into());
        self
    }

    /// An absolute horizontal inset on each edge of a
    /// [`MenuItem::Separator`], including submenus.
    ///
    /// Unset, the rule takes v3's own `ms-[3%] w-[94%]`: a proportional inset,
    /// centred in the panel's content box. Set, that is replaced by the same
    /// number of pixels on each edge whatever the panel's width — which is what
    /// a panel sitting beside a platform menu needs, AppKit's own separator
    /// being inset 15pt on each side inside wider panel bounds.
    pub fn separator_inset(mut self, inset: impl Into<Pixels>) -> Self {
        self.separator_inset = Some(inset.into());
        self
    }

    /// Thickness of a [`MenuItem::Separator`], including submenus. Unset keeps
    /// the theme's hairline border width.
    pub fn separator_thickness(mut self, thickness: impl Into<Pixels>) -> Self {
        self.separator_thickness = Some(thickness.into());
        self
    }

    /// Whether the panel and its submenus play their entry animation (default true).
    /// Exit animation and keyboard behavior are unchanged.
    pub fn animate_entry(mut self, animate: bool) -> Self {
        self.animate_entry = animate;
        self.animate_entry_is_set = true;
        self
    }

    /// Named theme overlay from [`herogpui_theme::ComponentThemes::menu`].
    /// Stackable; a missing name adds no override.
    pub fn recipe(mut self, name: impl Into<SharedString>) -> Self {
        self.recipes.push(name.into());
        self
    }

    /// Supplies the root menu's focus handle. Submenus own independent handles
    /// and return focus here on Left/Escape. The menu still focuses on entry.
    pub fn focus_handle(mut self, handle: gpui::FocusHandle) -> Self {
        self.focus_handle = Some(handle);
        self
    }

    pub(crate) fn overlay_token(mut self, token: crate::util::OverlayToken) -> Self {
        self.overlay_token = Some(token);
        self
    }

    pub(crate) fn dropdown_composition(mut self) -> Self {
        self.dropdown_composition = true;
        self
    }

    /// Labels the panel for behavior tests (`debug_bounds`).
    ///
    /// Not a v3 prop: see the field docs for why tests name the panel instead
    /// of wrapping it.
    pub(crate) fn panel_debug_label(mut self, label: &'static str) -> Self {
        self.panel_debug_label = Some(label);
        self
    }

    /// The element id every piece of this menu's state is keyed by.
    ///
    /// Not a v3 prop -- gpui needs an explicit id on a stateful element, and
    /// two menus that share one key share their focus, their cursor and their
    /// typeahead.
    pub fn id(mut self, id: impl Into<gpui::ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Plays the menu's exit instead of its entry.
    ///
    /// Not a v3 prop: v3's menu leaves the tree with a `[data-exiting]`
    /// attribute, and this is the flag that stands in for it.
    pub fn exiting(mut self, v: bool) -> Self {
        self.exiting = v;
        self
    }

    pub(crate) fn embedded(mut self, panel_bounds: PanelBounds) -> Self {
        self.deferred = false;
        self.panel_bounds = Some(panel_bounds);
        self
    }

    pub(crate) fn focus_first(
        mut self,
        state: gpui::Entity<Option<crate::list_core::OpenFocus>>,
    ) -> Self {
        self.focus_first = Some(state);
        self
    }

    pub(crate) fn on_back(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(std::sync::Arc::new(f));
        self
    }

    /// `children` on `Dropdown.Item` — replaces an item's label.
    ///
    /// The closure receives the item's key and the row's state: `isSelected`,
    /// `isIndeterminate`, `isFocused`, `isPressed` and `isDisabled`, which are
    /// the values v3 passes into the same render prop. The press is a frame
    /// behind the pointer or activation key, because gpui reports it to a handler.
    pub fn item_content(
        mut self,
        render: impl Fn(&SharedString, crate::util::InteractiveState) -> AnyElement + 'static,
    ) -> Self {
        self.item_content = Some(std::sync::Arc::new(render));
        self
    }

    /// `children` on `Dropdown.ItemIndicator` — replaces the built-in mark.
    pub fn indicator_content(
        mut self,
        render: impl Fn(&SharedString, bool, bool) -> AnyElement + 'static,
    ) -> Self {
        self.indicator_content = Some(std::sync::Arc::new(render));
        self
    }

    /// The leading element of each row, drawn where [`MenuItem::icon`]'s
    /// asset goes and in its place.
    ///
    /// HeroGPUI extension: v3 composes a leading icon as an ordered child of
    /// `Dropdown.Item`, and an asset path cannot carry an element the caller
    /// draws. The closure receives the item's key and the same row state as
    /// [`Menu::item_content`]; `None` keeps the item's own icon, if any. The
    /// row's text colour is inherited, so text-coloured content follows the
    /// danger, disabled and highlight colours. Forwarded to submenus.
    pub fn item_start_content(
        mut self,
        render: impl Fn(&SharedString, crate::util::InteractiveState) -> Option<AnyElement> + 'static,
    ) -> Self {
        self.item_start_content = Some(std::sync::Arc::new(render));
        self
    }

    /// `type` on `Dropdown.ItemIndicator` — a check mark or a dot.
    pub fn indicator(mut self, kind: IndicatorKind) -> Self {
        self.indicator = kind;
        self
    }

    /// The fill a hovered menu row takes, in place of `--default`.
    pub fn row_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.row_hover_bg = Some(color.into());
        self
    }

    /// Paired highlight text/icon color for hovered, focused and open-submenu
    /// rows. Disabled rows retain their disabled treatment. Forwarded to submenus.
    pub fn row_hover_foreground(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.row_hover_foreground = Some(color.into());
        self
    }

    /// The panel's corner radius, in place of the owning `container_radius`
    /// helper. The panel's entry zoom interpolates the same value, so both
    /// follow the override.
    ///
    /// Internal: only [`Dropdown`] reaches it, by forwarding its own
    /// [`Dropdown::radius`], so the standalone menu keeps the helper. Not a v3
    /// prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    pub(crate) fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// `selectionMode` — `None` (the default) makes items pure actions.
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// `selectedKeys` — the controlled selection.
    pub fn selected_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.selected_keys = keys.into_iter().map(Into::into).collect();
        self.selection_is_controlled = true;
        self
    }

    /// `defaultSelectedKeys` — seeds the menu's own selection state.
    pub fn default_selected_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.default_selected_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// `disallowEmptySelection` — prevents removing the last selected item.
    pub fn disallow_empty_selection(mut self, value: bool) -> Self {
        self.disallow_empty_selection = value;
        self
    }

    /// `disabledKeys` — items that cannot be activated.
    pub fn disabled_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.disabled_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// `onSelectionChange` — the whole selection after an item is activated.
    pub fn on_selection_change(
        mut self,
        f: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(std::sync::Arc::new(f));
        self
    }

    /// `onAction` — an item was activated, independent of any selection.
    pub fn on_action(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::sync::Arc::new(f));
        self
    }

    /// Sets the key of the selected item.
    pub fn selected_key(mut self, key: impl Into<SharedString>) -> Self {
        self.selected_key = Some(key.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the menu's root element after every value the composition
    /// and the active theme chose, so they win. The panel inside paints its
    /// own chrome, so this reaches the surface it floats in, not the panel.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl RenderOnce for Menu {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base_id = self.id.clone();
        let overlay_token = if let Some(token) = self.overlay_token.clone() {
            Some(token)
        } else if self.on_dismiss.is_some() {
            let (_, token) = crate::util::overlay_scope(
                window,
                cx,
                element_id::scoped(&base_id, "overlay"),
                true,
                self.exiting,
            );
            Some(token)
        } else {
            None
        };
        let (selected_keys, selection_own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&base_id, "selected"),
            self.selection_is_controlled
                .then(|| self.selected_keys.clone()),
            self.default_selected_keys.clone(),
        );
        self.selected_keys = selected_keys;
        // Which submenu is open, if any -- held as the open child's own
        // `ElementId` (`{id}-sub-{key}`), which is also the id the child menu
        // renders under. `use_keyed_state` takes `cx` mutably, so it precedes
        // everything that borrows the theme.
        let submenu_state =
            window.use_keyed_state(element_id::scoped(&base_id, "submenu"), cx, |_, _| {
                None::<gpui::ElementId>
            });
        let mut submenu_open = submenu_state.read(cx).clone();
        let submenu_focus =
            window.use_keyed_state(element_id::scoped(&base_id, "submenu-focus"), cx, |_, _| {
                None::<crate::list_core::OpenFocus>
            });
        // The open strategy the trigger left for this open, if any: `Some` is
        // an open that should seat the cursor (a pointer open carries
        // `OpenFocus::None`), `None` is no pending open.
        let open_focus = self.focus_first.as_ref().and_then(|state| *state.read(cx));
        let keyboard_open =
            open_focus.is_some_and(|focus| focus != crate::list_core::OpenFocus::None);
        let dismiss = self.on_dismiss.clone().map(|cb| {
            let submenu_state = submenu_state.clone();
            let submenu_focus = submenu_focus.clone();
            std::rc::Rc::new(move |refocus: &bool, window: &mut Window, cx: &mut App| {
                submenu_state.update(cx, |value, cx| {
                    if value.is_some() {
                        *value = None;
                        cx.notify();
                    }
                });
                submenu_focus.update(cx, |value, _| *value = None);
                cb(refocus, window, cx);
            }) as OnDismiss
        });
        // The keyboard's own state: which row it is on, the handle that receives
        // the keys, and the letters typed so far.
        let focus_handle = self.focus_handle.clone().unwrap_or_else(|| {
            window
                .use_keyed_state(element_id::scoped(&base_id, "focus"), cx, |_, cx| {
                    cx.focus_handle().tab_stop(true)
                })
                .read(cx)
                .clone()
        });
        // The keyboard's cursor is the shared list core's keyed cursor: it
        // holds the focused item's *key*, so it follows the item across a
        // reorder and clears when the item is disabled rather than repairing
        // to a neighbour.
        let cursor = crate::list_core::ListCore::cursor_state(&base_id, window, cx);
        // `.dropdown__popover` is `overflow-y-auto`, and React Aria keeps the
        // focused row in view. `use_keyed_state` takes `cx` mutably, so the
        // handle precedes the theme.
        let menu_scroll =
            window.use_keyed_state(element_id::scoped(&base_id, "scroll"), cx, |_, _| {
                gpui::ScrollHandle::new()
            });
        let menu_scroll_now = menu_scroll.read(cx).clone();
        // The menu does not wrap its arrows and pages to the enabled ends;
        // its letters typeahead through the shared buffer.
        let core = crate::list_core::ListCore::new(
            cursor.clone(),
            Some(crate::list_core::ListCore::typeahead_state(
                &base_id, window, cx,
            )),
            false,
            crate::list_core::PageKeys::Ends,
            crate::list_core::ListScroll::Panel(menu_scroll_now.clone()),
        );
        // One hover/press slot per item, for an `item_content` or
        // `item_start_content` closure. The slots exist only when one is set:
        // `track_interaction`'s handlers cost a frame of state, and the
        // closures are the only readers (the press v3's `Dropdown.Item`
        // render props document).
        let interaction: Vec<crate::util::Interaction> = if self.item_content.is_some()
            || self.item_start_content.is_some()
            || self.row_hover_foreground.is_some()
        {
            (0..self.items.len())
                .map(|i| {
                    crate::util::interaction(
                        element_id::scoped(
                            &element_id::indexed(&base_id, "item", i),
                            "interaction",
                        ),
                        window,
                        cx,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        // A menu takes focus when it opens, which is what makes the arrows work
        // without a click first. The one-shot re-arms while the menu plays its
        // exit, so a menu that reopens after a dismissal -- a pick or Escape
        // hands the focus back to the trigger -- is keyboard-driven again.
        let autofocus = element_id::scoped(&base_id, "autofocus");
        if self.exiting {
            let done = window.use_keyed_state(autofocus, cx, |_, _| false);
            done.update(cx, |d, _| *d = false);
            // A trigger toggle or a controlled close shuts the menu without
            // running `dismiss`, so an open child would paint full-size
            // beside its exiting parent and greet the next open. Exiting is
            // the close every path funnels through, which is where the child
            // goes quiet.
            submenu_state.update(cx, |value, cx| {
                if value.is_some() {
                    *value = None;
                    cx.notify();
                }
            });
            submenu_focus.update(cx, |value, _| *value = None);
            submenu_open = None;
        } else if keyboard_open {
            window.focus(&focus_handle, cx);
        } else if self.on_back.is_none() {
            crate::util::focus_once(window, cx, autofocus, &focus_handle);
        }

        // The rows a keyboard can land on -- an item that is not disabled -- and
        // the text a typed letter searches.
        let stops: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| match item {
                MenuItem::Item { key, .. } => !self.disabled_keys.contains(key),
                _ => false,
            })
            .map(|(i, _)| i)
            .collect();
        let labels: Vec<String> = self
            .items
            .iter()
            .map(|item| match item {
                MenuItem::Item { label, .. } => label.to_string(),
                _ => String::new(),
            })
            .collect();
        let item_keys: Vec<SharedString> = self
            .items
            .iter()
            .map(|item| match item {
                MenuItem::Item { key, .. } => key.clone(),
                _ => SharedString::default(),
            })
            .collect();
        // Whether each row is a submenu trigger. Such a row opens a child
        // panel instead of ending the menu, so activating it must not close
        // the parent -- React Aria returns before the close for a trigger.
        let item_has_submenu: Vec<bool> = self
            .items
            .iter()
            .map(|item| match item {
                MenuItem::Item { submenu, .. } => !submenu.is_empty(),
                _ => false,
            })
            .collect();
        // Whether each row hands its interaction to its own content. Enter and
        // Space must leave such a row alone: the hosted control answers them.
        let item_is_interactive: Vec<bool> = self
            .items
            .iter()
            .map(|item| match item {
                MenuItem::Item {
                    is_interactive,
                    submenu,
                    ..
                } => *is_interactive && submenu.is_empty(),
                _ => false,
            })
            .collect();
        // The cursor resolves through the shared core: its key locates the
        // row, and a key that is no longer a stop -- its item was disabled or
        // vanished -- resolves to no cursor rather than repairing to the
        // nearest enabled row the way an index did.
        let rows_view = crate::list_core::ListRows {
            keys: &item_keys,
            stops: &stops,
            labels: &labels,
        };
        let mut cursor_at = core.cursor_at(&rows_view, cx);
        if cursor.read(cx).is_some() && cursor_at.is_none() {
            core.set_cursor(None, cx);
        }
        // Every open seats through the shared core: a selected row that is a
        // stop wins, otherwise the trigger's strategy (First/Last/None for a
        // keyboard or pointer open) applies.
        if let Some(strategy) = open_focus {
            let selected: &[SharedString] =
                if crate::selection::reports_changes(self.selection_mode) {
                    self.selected_keys.as_slice()
                } else {
                    &[]
                };
            cursor_at = core.seat_on_open(&rows_view, selected, strategy, cx);
            if let Some(state) = &self.focus_first {
                state.update(cx, |value, _| *value = None);
            }
        }
        let item_bounds = self
            .items
            .iter()
            .map(|item| match item {
                MenuItem::Item { key, submenu, .. } if !submenu.is_empty() => {
                    Some(window.use_keyed_state(
                        element_id::scoped(
                            &element_id::scoped(&element_id::scoped(&base_id, "item"), key.clone()),
                            "bounds",
                        ),
                        cx,
                        |_, _| None::<Bounds<Pixels>>,
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let all_panel_bounds = self
            .panel_bounds
            .clone()
            .unwrap_or_else(|| std::rc::Rc::new(std::cell::RefCell::new(Vec::new())));
        // The union Vec's lifetime is one frame by construction: the top-level
        // menu allocates it here, in its own render, and gpui re-renders every
        // frame the panel is mounted. Both canvases below push once per frame
        // and the outside-press listener captured in this same render reads it,
        // so the union is always exactly this frame's composite panels -- no
        // stale bounds can outlive the frame that drew them. Never hoist this
        // Rc into keyed state: a Vec that survived frames would accumulate
        // every panel position it ever had and swallow outside presses near
        // old panel locations.

        let colors = cx.colors();
        let menu_theme = cx.theme().components.menu.resolve(&self.recipes);
        self.panel_min_width = self.panel_min_width.or(menu_theme.panel_min_width);
        self.panel_max_width = self.panel_max_width.or(menu_theme.panel_max_width);
        self.panel_max_height = self.panel_max_height.or(menu_theme.panel_max_height);
        self.panel_padding = self.panel_padding.or(menu_theme.panel_padding);
        self.panel_gap = self.panel_gap.or(menu_theme.panel_gap);
        self.row_height = self.row_height.or(menu_theme.row_height);
        self.row_padding_x = self.row_padding_x.or(menu_theme.row_padding_x);
        self.row_padding_y = self.row_padding_y.or(menu_theme.row_padding_y);
        self.row_text_size = self.row_text_size.or(menu_theme.row_text_size);
        self.row_gap = self.row_gap.or(menu_theme.row_gap);
        self.radius = self.radius.or(menu_theme.radius);
        self.separator_inset = self.separator_inset.or(menu_theme.separator_inset);
        self.separator_thickness = self.separator_thickness.or(menu_theme.separator_thickness);
        if !self.animate_entry_is_set {
            if let Some(animate) = menu_theme.animate_entry {
                self.animate_entry = animate;
            }
        }
        if self.row_hover_bg.is_none() {
            self.row_hover_bg = menu_theme.row_hover_bg.map(|color| color.resolve(colors));
        }
        if self.row_hover_foreground.is_none() {
            self.row_hover_foreground = menu_theme
                .row_hover_foreground
                .map(|color| color.resolve(colors));
        }
        let row_hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);
        let dropdown_composition = self.dropdown_composition;
        // The panel's entry zoom interpolates the panel's own radius, so one
        // binding feeds both the painted shape and the animation.
        let radius = self.radius.unwrap_or_else(|| {
            if dropdown_composition {
                crate::util::picker_radius(cx)
            } else {
                crate::util::container_radius(cx)
            }
        });

        let row_height = self.row_height.unwrap_or(px(36.));
        let row_padding_x = self.row_padding_x.unwrap_or(if dropdown_composition {
            px(10.)
        } else {
            px(8.)
        });
        let row_text_size = self.row_text_size.unwrap_or(px(14.));
        let row_gap = self.row_gap.unwrap_or(px(12.));
        let separator_inset = self.separator_inset;
        let separator_thickness = self
            .separator_thickness
            .unwrap_or_else(|| cx.layout().border_width);
        let panel_padding =
            self.panel_padding
                .unwrap_or(if dropdown_composition { px(6.) } else { px(4.) });
        let focus_gutter = panel_padding.max(px(0.)).min(px(4.));
        let viewport_width = (window.viewport_size().width - px(32.)).max(px(0.));
        let panel = gpui::div()
            .relative()
            .flex()
            .flex_col()
            // `.dropdown__popover` is `min(220px, 100%, 100svw - 2rem)` and the menu inside
            // it is `gap-0.5 p-1` -- `.dropdown__menu` overrides `.menu`'s
            // `gap-1` with half a step.
            .min_w(self.panel_min_width.unwrap_or(if dropdown_composition {
                px(220.).min(viewport_width)
            } else { px(220.) }))
            // v3.2.7 keeps 16px on each viewport edge; narrow windows must
            // preserve the normal minimum instead of collapsing to 48svw.
            .max_w(self.panel_max_width.unwrap_or(if dropdown_composition { viewport_width } else { window.viewport_size().width * 0.48 }))
            .gap(self.panel_gap.unwrap_or(px(2.)))
            .px(panel_padding)
            .py((panel_padding - focus_gutter).max(px(0.)))
            .bg(colors.overlay.background)
            .rounded(radius)
            .shadow(cx.layout().overlay_shadow.clone());
        let mut panel = panel
            // A long menu scrolls rather than being clipped, and gpui needs an
            // id for that. Pinned `.dropdown__popover` owns the overflow
            // (`overflow-y-auto`) while the menu inside is `overflow-clip`, and
            // React Aria caps the popover at the space the viewport leaves
            // (`calculatePosition`'s `getMaxHeight`). Inside a
            // height-constraining positioner -- the dropdown root and each
            // submenu popover -- the panel carries a viewport-relative bound so
            // the positioner's capped pass sizes it, and a short menu keeps its
            // natural height. A standalone menu keeps the old window share.
            .id(element_id::scoped(&base_id, "list"))
            .overflow_y_scroll()
            .track_scroll(&menu_scroll_now)
            .track_focus(&focus_handle)
            .key_context("Menu");
        // `menu/menu.js` renders the RAC `Menu`, and
        // `react-aria/dist/private/menu/useMenu.js` is a flat `role: 'menu'` on
        // the list element — which is this panel, the element that holds the
        // rows and the keyboard focus. Upstream names it through
        // `aria-labelledby` pointing at the trigger and warns when neither
        // `aria-label` nor `aria-labelledby` is supplied; the port has no id
        // graph and `Menu` has no label prop in v3's API, so it is unnamed.
        panel = panel.a11y(a11y::Role::Menu);
        if dropdown_composition || !self.deferred {
            // The surface already carries the positioner's bound; the panel
            // stretches to the surface's resolved height. A percentage `max_h`
            // cannot tunnel through the auto-height surface -- it would keep
            // its natural height with padding-only scroll room -- while flex
            // stretch follows the resolved size.
            panel = panel.self_stretch();
        } else {
            panel = panel.max_h(window.viewport_size().height * 0.6);
        }
        if let Some(max_height) = self.panel_max_height {
            panel = panel.max_h(max_height);
        }
        if let Some(label) = self.panel_debug_label {
            panel = panel.debug_selector(move || label.to_owned());
        }

        // v3 gives a floating panel no border: it is `bg-overlay shadow-overlay`
        // and a radius, and dark mode's inset hairline is what separates the
        // panel from the page.
        if let Some(hairline) = cx.layout().overlay_hairline {
            panel = panel
                .border(cx.layout().border_width)
                .border_color(hairline);
        }

        if !stops.is_empty() {
            let core_for_keys = core.clone();
            let stops_for_keys = stops;
            let labels_for_keys = labels;
            let on_action = self.on_action.clone();
            let on_selection_change = self.on_selection_change.clone();
            let selection_own_for_keys = selection_own.clone();
            let mode = self.selection_mode;
            let disallow_empty = self.disallow_empty_selection;
            let selected_now = self.selected_keys.clone();
            let keys = item_keys;
            let has_submenu = item_has_submenu;
            let is_interactive_for_keys = item_is_interactive;
            let submenu_open_for_keys = submenu_state.clone();
            let submenu_focus_for_keys = submenu_focus.clone();
            let submenu_base_for_keys = base_id.clone();
            let on_back = self.on_back.clone();
            let local_submenu = submenu_state.clone();
            let local_submenu_focus = submenu_focus.clone();
            let dismiss = dismiss.clone();
            let interaction_for_keys = interaction.clone();
            panel = panel.on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.as_str();
                // The rows the shared core answers to, borrowed for this key.
                let rows_view = crate::list_core::ListRows {
                    keys: &keys,
                    stops: &stops_for_keys,
                    labels: &labels_for_keys,
                };
                let from = core_for_keys.cursor_at(&rows_view, cx);
                if key == "left" {
                    if let Some(cb) = &on_back {
                        local_submenu.update(cx, |value, cx| {
                            if value.is_some() {
                                *value = None;
                                cx.notify();
                            }
                        });
                        local_submenu_focus.update(cx, |value, _| *value = None);
                        cb(window, cx);
                        cx.stop_propagation();
                    }
                    return;
                }
                if key == "right" {
                    let Some(i) = from else {
                        return;
                    };
                    if has_submenu.get(i).copied().unwrap_or(false) {
                        let Some(item_key) = keys.get(i) else {
                            return;
                        };
                        let open_key = element_id::scoped(
                            &element_id::scoped(&submenu_base_for_keys, "sub"),
                            item_key.clone(),
                        );
                        submenu_open_for_keys.update(cx, |value, cx| {
                            *value = Some(open_key);
                            cx.notify();
                        });
                        submenu_focus_for_keys.update(cx, |value, _| {
                            *value = Some(crate::list_core::OpenFocus::First);
                        });
                        cx.stop_propagation();
                    }
                    return;
                }
                // The page keys, arrows, Home/End, activation and the
                // typeahead buffer are the shared list core's. Pinned React
                // Aria 3.51.0 binds PageUp/PageDown through the menu's
                // `useSelectableCollection`, which only runs while the panel
                // is mounted, and its handlers require `manager.focusedKey !=
                // null` -- a mouse-opened menu has a null cursor until an
                // arrow (or a keyboard open's focus-first) seats one, so the
                // page keys stay inert until then. The menu is non-scrollable
                // (`PageKeys::Ends`), so a page takes the enabled end:
                // `stops` already omits disabled rows.
                match core_for_keys.key_down(&rows_view, key, &event.keystroke.modifiers, cx) {
                    crate::list_core::ListKey::Moved { .. } => {}
                    crate::list_core::ListKey::Activate(at) => {
                        let Some(i) = at else {
                            return;
                        };
                        let Some(item_key) = keys.get(i).cloned() else {
                            return;
                        };
                        // An interactive row is not a button: the element its
                        // `item_content` drew owns Enter and Space, exactly as
                        // it owns the pointer.
                        if is_interactive_for_keys.get(i).copied().unwrap_or(false) {
                            return;
                        }
                        if let Some(slot) = interaction_for_keys.get(i) {
                            crate::util::begin_keyboard_press(slot, event, window, cx);
                        }
                        let has_submenu = has_submenu[i];
                        // A submenu trigger opens its child; it is neither a
                        // selection nor a menu-level action in React Aria.
                        if has_submenu {
                            let open_key = element_id::scoped(
                                &element_id::scoped(&submenu_base_for_keys, "sub"),
                                item_key,
                            );
                            submenu_open_for_keys.update(cx, |value, cx| {
                                *value = Some(open_key);
                                cx.notify();
                            });
                            submenu_focus_for_keys.update(cx, |value, _| {
                                *value = Some(crate::list_core::OpenFocus::First);
                            });
                            return;
                        }
                        if crate::selection::reports_changes(mode) {
                            let next = crate::selection::next_selection(
                                &selected_now,
                                &item_key,
                                mode,
                                disallow_empty,
                            );
                            let blocked_last_removal = disallow_empty
                                && selected_now.len() == 1
                                && selected_now.contains(&item_key);
                            if !blocked_last_removal {
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
                        }
                        if let Some(cb) = &on_action {
                            cb(&item_key, window, cx);
                        }
                        // React Aria always closes for Enter. Space stays open
                        // only in multiple mode, so another item can be ticked.
                        // The focus is deliberately not sent back to the
                        // trigger from a key: gpui activates a focused element
                        // on key up, which would reopen the menu.
                        if key == "enter" || mode != SelectionMode::Multiple {
                            if let Some(cb) = &dismiss {
                                cb(&false, window, cx);
                            }
                        }
                    }
                    crate::list_core::ListKey::Unhandled => {}
                }
            });
        }

        let mut open_submenu = None;
        let item_count = self.items.len();
        let guttered = |index: usize, child: AnyElement| {
            gpui::div()
                .py(focus_gutter)
                .mt(if index == 0 { px(0.) } else { -focus_gutter })
                .mb(if index + 1 == item_count { px(0.) } else { -focus_gutter })
                .child(child)
        };
        for (i, item) in self.items.into_iter().enumerate() {
            match item {
                MenuItem::Separator => {
                    // `menu.css:8-11` and `dropdown.css:127-130` both inset the
                    // rule: `[data-slot="separator"] { @apply ms-[3%] w-[94%] }`,
                    // centred inside the panel's content box. `list_box.rs`
                    // already ports the identical rule; this panel used to draw
                    // the band full bleed.
                    //
                    // `separator_inset` replaces the proportional inset with an
                    // absolute one on each edge, for a panel that has to sit
                    // beside a platform menu whose own rule is a fixed inset
                    // rather than a share of the width.
                    let separator = match separator_inset {
                        Some(inset) => gpui::div().w_full().my(px(4.)).px(inset).child(
                            gpui::div()
                                .w_full()
                                .h(separator_thickness)
                                .bg(colors.separator),
                        ),
                        None => gpui::div()
                            .my(px(4.))
                            .mx(gpui::relative(0.03))
                            .w(gpui::relative(0.94))
                            .h(separator_thickness)
                            .bg(colors.separator),
                    };
                    panel = panel.child(guttered(i, separator.into_any_element()));
                }
                MenuItem::SectionLabel(label) => {
                    panel = panel.child(guttered(
                        i,
                        gpui::div()
                            .px(px(8.))
                            .pt(px(6.))
                            .pb(px(4.))
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(colors.muted)
                            .child(label.to_string())
                            .into_any_element(),
                    ));
                }
                MenuItem::Item {
                    key,
                    label,
                    shortcut,
                    icon,
                    is_danger,
                    description,
                    submenu,
                    is_interactive,
                } => {
                    // Either the single controlled key or membership of the
                    // selection set marks an item.
                    let is_selected = self.selected_key.as_ref() == Some(&key)
                        || self.selected_keys.contains(&key);
                    let is_item_disabled = self.disabled_keys.contains(&key);
                    let has_submenu = !submenu.is_empty();
                    // A submenu trigger already skips the row click; the flag
                    // only has meaning on a plain row.
                    let row_is_interactive = is_interactive && !has_submenu;
                    let open_key =
                        element_id::scoped(&element_id::scoped(&base_id, "sub"), key.clone());
                    let highlighted = !is_item_disabled
                        && (cursor_at == Some(i)
                            || submenu_open.as_ref() == Some(&open_key)
                            || interaction.get(i).is_some_and(|slot| slot.read(cx).0));
                    let highlight_foreground = self.row_hover_foreground.filter(|_| highlighted);
                    let text_color = if let Some(color) = highlight_foreground {
                        color
                    } else if is_item_disabled {
                        colors.muted
                    } else if is_danger {
                        colors.danger.color
                    } else {
                        colors.foreground
                    };
                    let mut row = gpui::div()
                        .id(element_id::indexed(&base_id, "item", i))
                        .relative()
                        .flex()
                        // `.menu-item` is `w-full`: the row takes the menu's
                        // width rather than its own content's.
                        .w_full()
                        .items_center()
                        .gap(row_gap)
                        .px(row_padding_x)
                        .rounded(crate::util::soft_radius(cx))
                        .text_size(row_text_size)
                        .line_height(px(20.))
                        .text_color(text_color);
                    if let Some(recorded_item_bounds) = item_bounds[i].clone() {
                        row = row.child(
                            gpui::canvas(
                                move |bounds, _, cx| {
                                    recorded_item_bounds.update(cx, |value, cx| {
                                        if value.as_ref() != Some(&bounds) {
                                            *value = Some(bounds);
                                            cx.notify();
                                        }
                                    });
                                    bounds
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .inset_0(),
                        );
                    }
                    // `.menu-item` is `min-h-9 py-1.5`; a described row grows
                    // past the minimum instead of clipping its second line.
                    row = row
                        .min_h(row_height)
                        .py(self.row_padding_y.unwrap_or(px(6.)));
                    // `react-aria/dist/private/menu/useMenuItem.js` decides the
                    // row's role in one place: `let role = 'menuitem'`, and
                    // then, *only when the row is not a submenu trigger*,
                    // `menuitemradio` in single-selection mode and
                    // `menuitemcheckbox` in multiple. `aria-checked` follows the
                    // same guard (`selectionMode !== 'none' && !isTrigger`), and
                    // a submenu trigger takes `aria-expanded` instead. Its
                    // `aria-haspopup` and `aria-controls` companions have no
                    // gpui builder (see `crate::a11y`).
                    let item_role = match (has_submenu, self.selection_mode) {
                        (true, _) | (false, SelectionMode::None) => a11y::Role::MenuItem,
                        (false, SelectionMode::Single) => a11y::Role::MenuItemRadio,
                        (false, SelectionMode::Multiple) => a11y::Role::MenuItemCheckBox,
                    };
                    // `aria-describedby` joins the description node and the
                    // keyboard-shortcut node, in that order.
                    let described = match (&description, &shortcut) {
                        (None, None) => None,
                        (Some(d), None) => Some(d.clone()),
                        (None, Some(k)) => Some(k.clone()),
                        (Some(d), Some(k)) => Some(SharedString::from(format!("{d} {k}"))),
                    };
                    if is_item_disabled {
                        // `status-disabled` is `--disabled-opacity`; the muted
                        // text alone was this port's own idea of the state.
                        row = row.opacity(cx.layout().disabled_opacity);
                    } else {
                        row = crate::util::cursor_interactive(row, cx);
                        // `.menu-item:hover` fills with `bg-default`, the full
                        // token, not the soft wash.
                        row = row.hover(move |s| s.bg(row_hover_bg));
                        // A press seats the keyboard cursor on this item's key.
                        let click_core = core.clone();
                        let click_key = key.clone();
                        let pointer_focus = focus_handle.clone();
                        row = row.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                            window.focus(&pointer_focus, cx);
                            click_core.set_cursor(Some(click_key.clone()), cx);
                        });
                        // Pointer hover moves real focus to the panel and, via
                        // the shared core, seats the cursor on this item's key
                        // for the next arrow. `seat_on_hover_with` keeps the
                        // focus call and the seat in one gated listener --
                        // GPUI runs same-element bubble listeners in reverse
                        // registration order, so a separate focus listener
                        // would see the cursor already seated and skip.
                        let hover_focus = focus_handle.clone();
                        row = core.seat_on_hover_with(row, key.clone(), move |window, cx| {
                            window.focus(&hover_focus, cx);
                        });
                    }
                    row = when_selected(row, is_selected, sem_primary(cx));
                    if highlighted && self.row_hover_foreground.is_some() {
                        row = row.bg(row_hover_bg).text_color(text_color);
                    }
                    // `.menu-item` takes `status-focused` on the row the keyboard
                    // is on -- a ring, not a border, which would shift the row.
                    // Pointer hover seats the cursor for the next arrow; it
                    // must not paint the ring. The ring rides as an overlay
                    // child so its corner is concentric with the row's
                    // `soft_radius`, which a spread shadow cannot be: the
                    // shadow keeps the row's own radius at a band two pixels
                    // wider, and needs a blur to paint at all.
                    row = crate::util::with_focus_ring_overlay(
                        row,
                        crate::util::shows_focus_ring(cursor_at == Some(i), cx),
                        true,
                        crate::util::soft_radius(cx),
                        Vec::new(),
                        cx,
                    );

                    // HeroUI passes React Aria's raw MenuItem state to the
                    // indicator. The pinned 1.20.0 state has no indeterminate
                    // member, so the documented value is always falsy.
                    let is_indeterminate = false;
                    let indicator_content = if let Some(render) = &self.indicator_content {
                        Some(render(&key, is_selected, is_indeterminate))
                    } else if is_selected && self.selection_mode != SelectionMode::None {
                        Some(match self.indicator {
                            IndicatorKind::Checkmark => gpui::svg()
                                .size(px(13.))
                                .path(icons::CHECK)
                                // svg() never inherits text colour.
                                .text_color(highlight_foreground.unwrap_or_else(|| sem_primary(cx)))
                                .into_any_element(),
                            IndicatorKind::Dot => gpui::div()
                                .size(px(6.))
                                .rounded_full()
                                .bg(highlight_foreground.unwrap_or_else(|| sem_primary(cx)))
                                .into_any_element(),
                        })
                    } else {
                        None
                    };
                    let mut indicator = if self.indicator_content.is_some()
                        || self.selection_mode != SelectionMode::None
                    {
                        let mut cell = gpui::div()
                            .size(px(16.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center();
                        // The row's normal gap is 12px. v3 positions the 16px
                        // cell 4px from its content, so cancel the extra 8px.
                        cell = if has_submenu {
                            cell.ml(px(-8.))
                        } else {
                            cell.mr(px(-8.))
                        };
                        if let Some(content) = indicator_content {
                            cell = cell.child(content);
                        }
                        Some(cell.into_any_element())
                    } else {
                        None
                    };
                    // The indicator owns v3's leading 16px cell. Submenu rows
                    // move it beside the trailing submenu chevron instead.
                    if !has_submenu {
                        if let Some(content) = indicator.take() {
                            row = row.child(content);
                        }
                    }
                    // The slot's press is a frame behind the pointer, because
                    // gpui reports it to a handler rather than to the render
                    // that draws it. v3's `Dropdown.Item` render-props table
                    // lists no `isHovered`, so the hover the slot also tracks
                    // is not handed over; a row is focused when the keyboard
                    // cursor is on it.
                    let (_, recorded_press) = interaction
                        .get(i)
                        .map(|slot| *slot.read(cx))
                        .unwrap_or_default();
                    let row_state = crate::util::InteractiveState {
                        is_hovered: false,
                        is_pressed: !is_item_disabled && recorded_press,
                        is_focused: cursor_at == Some(i),
                        is_focus_visible: cursor_at == Some(i) && crate::util::focus_visible(cx),
                        is_selected,
                        is_disabled: is_item_disabled,
                        is_pending: false,
                        is_indeterminate,
                    };
                    let start_content = self
                        .item_start_content
                        .as_ref()
                        .and_then(|render| render(&key, row_state));
                    match (start_content, icon) {
                        (Some(content), _) => {
                            row = row.child(
                                gpui::div().flex_none().flex().items_center().child(content),
                            );
                        }
                        (None, Some(icon_path)) => {
                            row = row.child(
                                gpui::svg()
                                    // `.menu-item__indicator` is `size-4`.
                                    .size(px(16.))
                                    .path(icon_path)
                                    .text_color(text_color),
                            );
                        }
                        (None, None) => {}
                    }
                    // `children` on `Dropdown.Item` is a render function in
                    // v3, handed the row's state.
                    row = row.child(
                        gpui::div().flex().flex_col().flex_1().min_w_0().child(
                            match &self.item_content {
                                Some(render) => render(&key, row_state),
                                None => match &description {
                                    // `Label` over `Description`, which is how v3
                                    // composes a described item.
                                    Some(text) => gpui::div()
                                        .flex()
                                        .flex_col()
                                        .min_w_0()
                                        .child(
                                            gpui::div()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .child(label.to_string()),
                                        )
                                        .child(
                                            gpui::div()
                                            // A described row composes a
                                            // `Description`, which is `text-xs`.
                                            .text_size(px(12.))
                                            .line_height(px(16.))
                                            .text_color(highlight_foreground.unwrap_or(colors.muted))
                                            // `[data-slot="description"]` is
                                            // `text-wrap`, so its box is the
                                            // column's width, not its own
                                            // content's.
                                            .w_full()
                                            .child(text.to_string()),
                                        )
                                        .into_any_element(),
                                    None => gpui::div()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(label.to_string())
                                        .into_any_element(),
                                },
                            },
                        ),
                    );
                    // The slot's hover and press handlers keep the press the
                    // closure reads current. Disabled rows expose idle state.
                    if !is_item_disabled {
                        if let Some(slot) = interaction.get(i) {
                            row = crate::util::track_interaction(row, slot);
                        }
                    }
                    if let Some(sc) = shortcut {
                        row = row.child(
                            crate::kbd::Kbd::new()
                                .variant(crate::kbd::KbdVariant::Light)
                                .map(|kbd| match highlight_foreground {
                                    Some(color) => kbd.sx(move |el| el.text_color(color)),
                                    None => kbd,
                                })
                                .child(sc.to_string()),
                        );
                    }
                    // `Dropdown.SubmenuIndicator` — the chevron that says a row
                    // opens another panel.
                    if has_submenu {
                        row = row.child(
                            gpui::svg()
                                .size(px(13.))
                                .path(icons::CHEVRON_RIGHT)
                                .text_color(highlight_foreground.unwrap_or(colors.muted)),
                        );
                        if let Some(content) = indicator {
                            row = row.child(content);
                        }
                    }

                    // `.menu-item[data-pressed]` is `scale(0.98)`. The press
                    // wrap comes after every visual child: children added
                    // after `pressed` land on the slot and fight the skin for
                    // width. The row is `w-full`, so its slot is too.
                    // The 98% press scale shrinks every child, a hosted
                    // control included, and it is keyed off a row press the
                    // interactive row no longer claims.
                    if !is_item_disabled && !row_is_interactive {
                        row = crate::anim::pressed(
                            row,
                            crate::anim::PressBox {
                                height: row_height,
                                padding_x: Some(row_padding_x),
                                width: None,
                                min_width: None,
                                text_size: row_text_size,
                                line_height: px(20.),
                                gap: row_gap,
                                radius: crate::util::soft_radius(cx),
                                shrink_x: false,
                                scale: crate::anim::PRESSED_SCALE_SUBTLE,
                            },
                            cx,
                        );
                    }

                    row = row.a11y_named(
                        item_role,
                        &a11y::Name::labelled(label.clone()).described(described),
                    );
                    if has_submenu {
                        let open_key =
                            element_id::scoped(&element_id::scoped(&base_id, "sub"), key.clone());
                        row = row.a11y_expanded(submenu_open.as_ref() == Some(&open_key));
                    } else if self.selection_mode != SelectionMode::None {
                        row = row.a11y_checked(is_selected, false);
                    }

                    if !is_item_disabled && !has_submenu && !row_is_interactive {
                        let on_action = self.on_action.clone();
                        let on_selection_change = self.on_selection_change.clone();
                        let selection_own = selection_own.clone();
                        let dismiss = dismiss.clone();
                        // Attached even with no callback to run, because v3's
                        // close happens on the click, not in a handler.
                        let key2 = key.clone();
                        let mode = self.selection_mode;
                        let disallow_empty = self.disallow_empty_selection;
                        let current = self.selected_keys.clone();
                        row = row.on_click(move |_, window, cx| {
                            if crate::selection::reports_changes(mode) {
                                let next = crate::selection::next_selection(
                                    &current,
                                    &key2,
                                    mode,
                                    disallow_empty,
                                );
                                let blocked_last_removal =
                                    disallow_empty && current.len() == 1 && current.contains(&key2);
                                if !blocked_last_removal {
                                    if let Some(held) = &selection_own {
                                        held.update(cx, |value, cx| {
                                            *value = next.clone();
                                            cx.notify();
                                        });
                                    }
                                    if let Some(cb) = &on_selection_change {
                                        cb(&next, window, cx);
                                    }
                                }
                            }
                            if let Some(cb) = &on_action {
                                cb(&key2, window, cx);
                            }
                            // A pointer pick stays open only in multiple mode.
                            // The trigger gets focus back from a click; only a
                            // keyboard key-up cannot safely refocus it.
                            if mode != SelectionMode::Multiple {
                                if let Some(cb) = &dismiss {
                                    cb(&true, window, cx);
                                }
                            }
                        });
                    }

                    // `Dropdown.SubmenuTrigger`: the child panel is anchored to
                    // the row and opens while the row is hovered. gpui paints in
                    // tree order, so it goes through `util::floating` like every
                    // other floating surface.
                    if has_submenu {
                        let open_key =
                            element_id::scoped(&element_id::scoped(&base_id, "sub"), key.clone());
                        let is_sub_open = submenu_open.as_ref() == Some(&open_key);
                        let held = submenu_state.clone();
                        let hover_focus = submenu_focus.clone();
                        let open_key2 = open_key.clone();
                        // The hover that opens the child panel lives on the
                        // wrapper, not the row: `track_interaction` above has
                        // claimed the row's single `on_hover` when an
                        // `item_content` closure is set, and gpui refuses a
                        // second listener on one element.
                        let mut slot = gpui::div()
                            .id(element_id::scoped(&open_key, "wrap"))
                            .relative()
                            .child(row);
                        if !is_item_disabled {
                            let click_held = submenu_state.clone();
                            let click_focus = submenu_focus.clone();
                            let click_key = open_key.clone();
                            slot = slot
                                .on_hover(move |hovered, _window, cx| {
                                    if *hovered {
                                        held.update(cx, |value, cx| {
                                            if value.as_ref() != Some(&open_key2) {
                                                *value = Some(open_key2.clone());
                                                cx.notify();
                                            }
                                        });
                                        hover_focus.update(cx, |value, _| *value = None);
                                    }
                                })
                                .on_click(move |_, _, cx| {
                                    click_held.update(cx, |value, cx| {
                                        *value = Some(click_key.clone());
                                        cx.notify();
                                    });
                                    click_focus.update(cx, |value, _| *value = None);
                                });
                        }
                        if is_sub_open {
                            open_submenu = Some((i, open_key, submenu));
                        }
                        panel = panel.child(guttered(i, slot.into_any_element()));
                    } else {
                        panel = panel.child(guttered(i, row.into_any_element()));
                    }
                }
            }
        }

        // The flex surface's rectangular hull includes blank space between
        // unequal-height panels. The outside listener lives on the root panel
        // and checks the union of the actual parent and descendant bounds.
        if self.deferred {
            if let Some(cb) = dismiss.clone() {
                if open_submenu.is_some() {
                    // A submenu is a sibling of the parent panel. Keep the
                    // union-boundary check so its real bounds remain inside
                    // the menu surface; the shared token still gates Escape.
                    let panel_union = all_panel_bounds.clone();
                    if let Some(token) = overlay_token.clone() {
                        panel = crate::util::dismiss_on_press_outside_with_token_event(
                            panel,
                            token,
                            move |event, window, cx| {
                                let inside = panel_union.borrow().iter().any(|bounds| {
                                    event.position.x >= bounds.origin.x
                                        && event.position.x <= bounds.origin.x + bounds.size.width
                                        && event.position.y >= bounds.origin.y
                                        && event.position.y <= bounds.origin.y + bounds.size.height
                                });
                                if inside {
                                    crate::util::DismissResult::Declined
                                } else {
                                    cb(&true, window, cx);
                                    crate::util::DismissResult::Handled
                                }
                            },
                        );
                    }
                } else if let Some(token) = overlay_token.clone() {
                    // The explicit token gates a simple menu against any
                    // overlay above it.
                    panel = crate::util::dismiss_on_press_outside_with_token(
                        panel,
                        token,
                        move |window, cx| {
                            cb(&true, window, cx);
                            crate::util::DismissResult::Handled
                        },
                    );
                }
            }
        }

        // The zoom grows the panel's own vertical padding, so the border box
        // the positioner places IS the painted panel: at rest the animated
        // padding equals the panel's natural padding (`p-1.5` under the
        // dropdown composition, `p-1` otherwise) and adds nothing, and
        // mid-animation only the internal padding flexes, never the origin.
        // Animating the surface instead would hang its `py` between the
        // placed box and the painted panel (one RAC gap plus ~6px).
        let zoom = crate::anim::ZoomBox::panel((panel_padding - focus_gutter).max(px(0.)), radius);
        let panel = if self.exiting {
            crate::anim::exiting(
                panel,
                element_id::scoped(&base_id, "panel-out"),
                zoom,
                crate::anim::Motion::LIST_OUT,
                cx,
            )
        } else if self.animate_entry {
            crate::anim::entering_zoom(
                panel,
                element_id::scoped(&base_id, "panel"),
                zoom,
                crate::anim::Motion::POPOVER_IN,
                cx,
            )
        } else {
            panel.into_any_element()
        };

        // Parent and child menus share one deferred surface. The submenu is its
        // own popover positioned against its row -- pinned RAC 1.20.0 renders
        // each submenu in a separate `Popover` with `placement: 'end top'` --
        // so it is a sibling of the parent's overflow scroller, never clipped
        // by it, and it flips and caps against the viewport on its own. The
        // positioner element itself is absolute, so it takes no flex space.
        // The surface carries the positioner's relative bound through to the
        // panel: `layout_as_root` only offers definite space, and the cap
        // resolves through `max_h_full` at every level -- a bare flex surface
        // would size to its content and shield the panel, leaving it at its
        // natural height with padding-only scroll room.
        let mut surface = gpui::div()
            .relative()
            .flex()
            .items_start()
            .gap(px(4.))
            .max_h_full()
            .child(panel);
        if let Some((index, submenu_id, submenu)) = open_submenu {
            // The row canvas records these bounds every frame, so the bridge
            // carries last frame's row rect and the positioner settles the
            // same frame the submenu opens.
            let row_trigger = std::rc::Rc::new(std::cell::Cell::new(
                item_bounds[index]
                    .as_ref()
                    .and_then(|bounds| bounds.read(cx).to_owned()),
            ));
            let mut sub = Menu::new(submenu_id.clone(), submenu)
                .id(element_id::scoped(&submenu_id, "menu"))
                .panel_debug_label("dropdown-submenu")
                .indicator(self.indicator)
                .disabled_keys(self.disabled_keys)
                .embedded(all_panel_bounds.clone())
                .focus_first(submenu_focus.clone());
            sub.panel_min_width = self.panel_min_width;
            sub.panel_max_width = self.panel_max_width;
            sub.panel_max_height = self.panel_max_height;
            sub.row_height = self.row_height;
            sub.row_padding_x = self.row_padding_x;
            sub.row_padding_y = self.row_padding_y;
            sub.row_text_size = self.row_text_size;
            sub.row_gap = self.row_gap;
            sub.panel_padding = self.panel_padding;
            sub.panel_gap = self.panel_gap;
            sub.separator_inset = self.separator_inset;
            sub.separator_thickness = self.separator_thickness;
            sub.animate_entry = self.animate_entry;
            sub.row_hover_bg = self.row_hover_bg;
            sub.row_hover_foreground = self.row_hover_foreground;
            sub.radius = self.radius;
            sub.recipes = self.recipes.clone();
            sub.animate_entry_is_set = self.animate_entry_is_set;
            sub.item_content = self.item_content.clone();
            sub.indicator_content = self.indicator_content.clone();
            sub.item_start_content = self.item_start_content.clone();
            if let Some(token) = overlay_token.clone() {
                sub = sub.overlay_token(token);
            }
            let close_state = submenu_state;
            let close_focus_state = submenu_focus;
            let parent_focus = focus_handle.clone();
            sub = sub.on_back(move |window, cx| {
                close_state.update(cx, |value, cx| {
                    *value = None;
                    cx.notify();
                });
                close_focus_state.update(cx, |value, _| *value = None);
                window.focus(&parent_focus, cx);
            });
            if let Some(cb) = self.on_action.clone() {
                sub = sub.on_action(move |key, window, cx| cb(key, window, cx));
            }
            if let Some(cb) = dismiss.clone() {
                sub = sub.on_dismiss(move |refocus, window, cx| cb(refocus, window, cx));
            }
            surface = surface.child(crate::popover::scrollable_submenu_popover(row_trigger, sub));
        }

        // The union outside-press check reads this frame's panel frames. The
        // canvas lives on the surface, not the panel: the panel owns the
        // vertical scroll container, so a canvas inside it reports scrolled
        // content-space bounds after a wheel (its origin walks up with the
        // rows) and a press on a scrolled-into-view row reads as outside.
        // The surface never scrolls -- its only other child is the absolute
        // submenu positioner, which takes no flex space -- so its frame
        // coincides with the panel's.
        let registered_panel_bounds = all_panel_bounds;
        surface = surface.child(
            gpui::canvas(
                move |bounds, _, _| {
                    registered_panel_bounds.borrow_mut().push(bounds);
                    bounds
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        );

        // Escape bubbles from the focused descendant to this root surface.
        let surface = if self.deferred {
            match dismiss {
                Some(cb) => match overlay_token {
                    Some(token) => crate::util::dismiss_on_escape_with_token(
                        surface,
                        token,
                        move |window, cx| {
                            cb(&true, window, cx);
                            crate::util::DismissResult::Handled
                        },
                    ),
                    None => surface,
                },
                None => surface,
            }
        } else {
            surface
        };
        let surface = crate::util::apply_sx(surface, &self.sx);

        if self.deferred {
            crate::util::floating(surface).into_any_element()
        } else {
            surface.into_any_element()
        }
    }
}

fn sem_primary(cx: &App) -> gpui::Hsla {
    cx.colors().accent.color
}

fn when_selected(
    el: gpui::Stateful<gpui::Div>,
    selected: bool,
    color: gpui::Hsla,
) -> gpui::Stateful<gpui::Div> {
    if selected {
        el.bg(color.alpha(0.14)).text_color(color)
    } else {
        el
    }
}

/// `trigger` — what opens the menu.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum DropdownTrigger {
    /// A press opens it, which is v3's default.
    #[default]
    Press,
    /// A press held for the theme's `long_press_ms` opens it (500ms by
    /// default, matching React Aria).
    LongPress,
}

/// Dropdown wrapper: trigger + floating menu panel (`Dropdown/DropdownTrigger/
/// DropdownMenu` composition).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Dropdown {
    /// Keys this dropdown's own state; see [`Dropdown::id`].
    id: gpui::ElementId,
    trigger: AnyElement,
    /// `trigger` — press (the default) or long press.
    trigger_kind: DropdownTrigger,
    /// `isOpen` — `None` leaves the component holding the flag, seeded from
    /// `defaultOpen`.
    is_open: Option<bool>,
    default_open: bool,
    on_open_change: Option<std::sync::Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    items: Vec<MenuItem>,
    item_content: Option<ItemContent>,
    indicator_content: Option<ItemIndicatorContent>,
    item_start_content: Option<ItemStartContent>,
    selection_mode: SelectionMode,
    selected_keys: Vec<SharedString>,
    default_selected_keys: Vec<SharedString>,
    selection_is_controlled: bool,
    disallow_empty_selection: bool,
    disabled_keys: Vec<SharedString>,
    indicator: IndicatorKind,
    on_selection_change: Option<OnSelectionChange>,
    on_action: Option<OnSelect>,
    /// The fill a hovered menu row takes, in place of `--default`.
    row_hover_bg: Option<gpui::Hsla>,
    /// The panel's corner radius, forwarded onto the menu that paints it.
    radius: Option<Pixels>,
    placement: DropdownPlacement,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
    recipes: Vec<SharedString>,
}

/// `placement` on `Dropdown.Popover`.
///
/// Shares the one placement vocabulary with the pickers and popover; it
/// previously offered only the two bottom-aligned values.
pub use herogpui_core::Placement as DropdownPlacement;

impl Dropdown {
    /// The element id this dropdown's state is keyed by.
    ///
    /// Not a v3 prop. It matters more here than it looks: with one shared key,
    /// pressing any trigger on a page opened *every* menu on it, because they
    /// were all reading the same uncontrolled open flag.
    pub fn id(mut self, id: impl Into<gpui::ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// `onOpenChange` — reports the open state the trigger moves to.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(std::sync::Arc::new(handler));
        self
    }

    /// `isOpen` — also accepted positionally by [`Dropdown::new`].
    pub fn is_open(mut self, v: bool) -> Self {
        self.is_open = Some(v);
        self
    }
    /// `defaultOpen` — the uncontrolled initial state.
    ///
    /// Only consulted when `is_open` is not supplied; the component then owns
    /// the flag and its trigger toggles it.
    pub fn default_open(mut self, v: bool) -> Self {
        self.default_open = v;
        self
    }

    /// An uncontrolled dropdown: the menu holds its own open state, seeded
    /// from [`Dropdown::default_open`], and the trigger toggles it.
    /// `trigger` — `Press` (default) or `LongPress`.
    pub fn trigger(mut self, kind: DropdownTrigger) -> Self {
        self.trigger_kind = kind;
        self
    }

    /// Creates a dropdown that owns its open state.
    pub fn uncontrolled(
        id: impl Into<gpui::ElementId>,
        trigger: impl IntoElement,
        items: Vec<MenuItem>,
    ) -> Self {
        let mut dd = Self::new(id, trigger, items, false);
        dd.is_open = None;
        dd
    }

    /// Creates a dropdown whose open state is supplied by the caller.
    pub fn new(
        id: impl Into<gpui::ElementId>,
        trigger: impl IntoElement,
        items: Vec<MenuItem>,
        is_open: bool,
    ) -> Self {
        Self {
            id: id.into(),
            trigger: trigger.into_any_element(),
            trigger_kind: DropdownTrigger::default(),
            is_open: Some(is_open),
            default_open: false,
            on_open_change: None,
            items,
            item_content: None,
            indicator_content: None,
            item_start_content: None,
            selection_mode: SelectionMode::None,
            selected_keys: Vec::new(),
            default_selected_keys: Vec::new(),
            selection_is_controlled: false,
            disallow_empty_selection: false,
            disabled_keys: Vec::new(),
            indicator: IndicatorKind::default(),
            on_selection_change: None,
            on_action: None,
            row_hover_bg: None,
            radius: None,
            placement: DropdownPlacement::BottomStart,
            sx: None,
            recipes: Vec::new(),
        }
    }

    /// Named theme overlay forwarded onto the painted [`Menu`].
    /// Stackable; a missing name adds no override.
    pub fn recipe(mut self, name: impl Into<SharedString>) -> Self {
        self.recipes.push(name.into());
        self
    }

    /// `type` on `Dropdown.ItemIndicator`.
    pub fn indicator(mut self, kind: IndicatorKind) -> Self {
        self.indicator = kind;
        self
    }

    /// The fill a hovered menu row takes, in place of `--default`. v3 tints
    /// the row with a class; this names the colour.
    pub fn row_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.row_hover_bg = Some(color.into());
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

    /// `children` on `Dropdown.Item` — replaces each item's label with a
    /// render closure receiving its key and interactive state.
    pub fn item_content(
        mut self,
        render: impl Fn(&SharedString, crate::util::InteractiveState) -> AnyElement + 'static,
    ) -> Self {
        self.item_content = Some(std::sync::Arc::new(render));
        self
    }

    /// `children` on `Dropdown.ItemIndicator` — replaces the built-in mark.
    pub fn indicator_content(
        mut self,
        render: impl Fn(&SharedString, bool, bool) -> AnyElement + 'static,
    ) -> Self {
        self.indicator_content = Some(std::sync::Arc::new(render));
        self
    }

    /// The leading element of each row, forwarded onto the painted [`Menu`].
    /// See [`Menu::item_start_content`]; a HeroGPUI extension.
    pub fn item_start_content(
        mut self,
        render: impl Fn(&SharedString, crate::util::InteractiveState) -> Option<AnyElement> + 'static,
    ) -> Self {
        self.item_start_content = Some(std::sync::Arc::new(render));
        self
    }

    /// `selectionMode` on `Dropdown.Menu`.
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// `selectedKeys` on `Dropdown.Menu`.
    pub fn selected_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.selected_keys = keys.into_iter().map(Into::into).collect();
        self.selection_is_controlled = true;
        self
    }

    /// `defaultSelectedKeys` on `Dropdown.Menu` — seeds uncontrolled selection.
    pub fn default_selected_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.default_selected_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// `disallowEmptySelection` on `Dropdown.Menu`.
    pub fn disallow_empty_selection(mut self, value: bool) -> Self {
        self.disallow_empty_selection = value;
        self
    }

    /// `disabledKeys` on `Dropdown.Menu`.
    pub fn disabled_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.disabled_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// `onSelectionChange` on `Dropdown.Menu`.
    pub fn on_selection_change(
        mut self,
        f: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(std::sync::Arc::new(f));
        self
    }

    /// `onAction` on `Dropdown.Menu`.
    pub fn on_action(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::sync::Arc::new(f));
        self
    }

    /// Sets where the menu opens relative to the trigger.
    pub fn placement(mut self, p: DropdownPlacement) -> Self {
        self.placement = p;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the dropdown's root element after every value the
    /// composition and the active theme chose, so they win. The trigger is
    /// the caller's own element and the panel paints its own chrome, so this
    /// reaches the wrapper they sit in.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl RenderOnce for Dropdown {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let _ = icons::CHEVRON_DOWN;
        let wrap_base_id = self.id.clone();

        // `isOpen` wins; without it the menu holds the flag itself, which is
        // what `defaultOpen` promises. See `Dropdown::uncontrolled`.
        let (is_open, open_own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&wrap_base_id, "open"),
            self.is_open,
            self.default_open,
        );
        let (selected_keys, selection_own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&wrap_base_id, "selected"),
            self.selection_is_controlled
                .then(|| self.selected_keys.clone()),
            self.default_selected_keys.clone(),
        );
        self.selected_keys = selected_keys;
        // `overlay_scope` takes `cx` mutably too, so it goes here.
        let (phase, overlay_token) = crate::util::overlay_scope(
            window,
            cx,
            element_id::scoped(&wrap_base_id, "phase"),
            is_open,
            true,
        );
        let focus_first = window.use_keyed_state(
            element_id::scoped(&wrap_base_id, "focus-first"),
            cx,
            |_, _| None::<crate::list_core::OpenFocus>,
        );

        // `trigger="longPress"` needs to know whether the button is still down
        // when the timer fires, so the press is a piece of state rather than a
        // local.
        let holding =
            window.use_keyed_state(element_id::scoped(&wrap_base_id, "holding"), cx, |_, _| {
                false
            });

        // Where the focus goes when the menu closes. React Aria hands it back
        // to the trigger, and the trigger element is the caller's, so the
        // wrapper holds the handle. It is deliberately *not* a tab stop: gpui
        // keeps any tracked handle in the tab order, so Tab carries on from here
        // instead of starting the page over.
        let trigger_focus = window.use_keyed_state(
            element_id::scoped(&wrap_base_id, "trigger-focus"),
            cx,
            |_, cx| cx.focus_handle(),
        );
        let trigger_handle = trigger_focus.read(cx).clone();
        let anchor_bounds = window
            .use_keyed_state(
                element_id::scoped(&wrap_base_id, "anchor-bounds"),
                cx,
                |_, _| std::rc::Rc::new(std::cell::Cell::new(None::<Bounds<Pixels>>)),
            )
            .read(cx)
            .clone();
        let mut trigger_wrap = gpui::div()
            .id(element_id::scoped(&wrap_base_id, "trigger"))
            .track_focus(&trigger_handle)
            .cursor(crate::util::interactive_cursor(cx));
        let dismiss_own = open_own.clone();
        let on_open_change = self.on_open_change.clone();
        if on_open_change.is_some() || open_own.is_some() {
            let next_open = !is_open;
            let own = open_own;
            match self.trigger_kind {
                DropdownTrigger::Press => {
                    let key_own = own.clone();
                    let key_open_change = on_open_change.clone();
                    let key_focus_first = focus_first.clone();
                    trigger_wrap = trigger_wrap.on_key_down(move |event, window, cx| {
                        let key = event.keystroke.key.as_str();
                        if is_open {
                            return;
                        }
                        // React Aria's `useMenuTrigger`: plain Enter, Space
                        // and ArrowDown open `press` onto the first row,
                        // ArrowUp onto the last; Alt+ArrowDown/ArrowUp open for
                        // both trigger modes while Alt+Enter/Space are the
                        // longPress chords and stay inert here.
                        let modifiers = event.keystroke.modifiers;
                        let clean = !modifiers.control && !modifiers.platform && !modifiers.shift;
                        let strategy = match key {
                            "enter" | "space" if clean && !modifiers.alt => {
                                crate::list_core::OpenFocus::First
                            }
                            "down" if clean => crate::list_core::OpenFocus::First,
                            "up" if clean => crate::list_core::OpenFocus::Last,
                            _ => return,
                        };
                        key_focus_first.update(cx, |focus, _| *focus = Some(strategy));
                        if let Some(held) = &key_own {
                            held.update(cx, |value, cx| {
                                *value = true;
                                cx.notify();
                            });
                        }
                        if let Some(cb) = &key_open_change {
                            cb(&true, window, cx);
                        }
                        cx.stop_propagation();
                    });
                    let focus_first = focus_first.clone();
                    trigger_wrap = trigger_wrap.on_click(move |ev: &ClickEvent, w, cx| {
                        focus_first.update(cx, |focus, _| {
                            *focus = Some(if matches!(ev, ClickEvent::Keyboard(_)) {
                                crate::list_core::OpenFocus::First
                            } else {
                                crate::list_core::OpenFocus::None
                            });
                        });
                        // Uncontrolled: flip our own copy, or the trigger would
                        // be inert without a caller handler.
                        if let Some(held) = &own {
                            held.update(cx, |v, cx| {
                                *v = next_open;
                                cx.notify();
                            });
                        }
                        if let Some(cb) = &on_open_change {
                            cb(&next_open, w, cx);
                        }
                    });
                }
                DropdownTrigger::LongPress => {
                    let up_holding = holding.clone();
                    let pointer_focus_first = focus_first.clone();
                    // `longPress` opens only through the Alt chords upstream:
                    // Alt+Enter/Space/ArrowDown `first`, Alt+ArrowUp `last`;
                    // plain keys stay inert.
                    let key_focus_first = focus_first.clone();
                    let key_own = own.clone();
                    let key_open_change = on_open_change.clone();
                    trigger_wrap = trigger_wrap.on_key_down(move |event, window, cx| {
                        if is_open {
                            return;
                        }
                        let modifiers = event.keystroke.modifiers;
                        if !modifiers.alt
                            || modifiers.control
                            || modifiers.platform
                            || modifiers.shift
                        {
                            return;
                        }
                        let strategy = match event.keystroke.key.as_str() {
                            "enter" | "space" | "down" => crate::list_core::OpenFocus::First,
                            "up" => crate::list_core::OpenFocus::Last,
                            _ => return,
                        };
                        key_focus_first.update(cx, |focus, _| *focus = Some(strategy));
                        if let Some(held) = &key_own {
                            held.update(cx, |value, cx| {
                                *value = true;
                                cx.notify();
                            });
                        }
                        if let Some(cb) = &key_open_change {
                            cb(&true, window, cx);
                        }
                        cx.stop_propagation();
                    });
                    trigger_wrap = trigger_wrap
                        .on_mouse_down(gpui::MouseButton::Left, {
                            let holding = holding;
                            move |_, window, cx| {
                                pointer_focus_first.update(cx, |focus, _| {
                                    *focus = Some(crate::list_core::OpenFocus::None);
                                });
                                holding.update(cx, |v, _| *v = true);
                                let holding = holding.clone();
                                let own = own.clone();
                                let on_open_change = on_open_change.clone();
                                let long_press_ms = cx.layout().long_press_ms;
                                // Open only if the button is still down when the
                                // timer expires; a quick click leaves it shut.
                                // `window.spawn` rather than `cx.spawn`: the
                                // callback needs a `Window`, and only a window
                                // async context can hand one back.
                                window
                                    .spawn(cx, async move |cx| {
                                        cx.background_executor()
                                            .timer(std::time::Duration::from_millis(long_press_ms))
                                            .await;
                                        cx.update(|window, cx| {
                                            if !*holding.read(cx) {
                                                return;
                                            }
                                            if let Some(held) = &own {
                                                held.update(cx, |v, cx| {
                                                    *v = true;
                                                    cx.notify();
                                                });
                                            }
                                            if let Some(cb) = &on_open_change {
                                                cb(&true, window, cx);
                                            }
                                        })
                                        .ok();
                                    })
                                    .detach();
                            }
                        })
                        .on_mouse_up(gpui::MouseButton::Left, move |_, _window, cx| {
                            up_holding.update(cx, |v, _| *v = false);
                        });
                }
            }
        }

        // A flex column with `items_start` keeps the trigger at its natural
        // width; a plain block root would stretch it (gpui divs are
        // Display::Block, so a block-level flex child fills the line). The
        // trigger is measured the way RAC's `useOverlayPosition` positions
        // against the trigger rect -- the measure element only records the
        // bounds the popover below reads to flip and cap the panel.
        let trigger = crate::popover::PopoverTriggerMeasure::new(
            trigger_wrap.child(self.trigger),
            anchor_bounds.clone(),
        );
        let mut root = gpui::div()
            .relative()
            .flex()
            .flex_col()
            // `.dropdown` is `flex flex-col gap-1`.
            .gap(px(4.))
            .items_start()
            .child(trigger);

        // v3 keeps a closing menu on screen for its `[data-exiting]` run.
        if phase != crate::util::OverlayPhase::Closed {
            let mut menu = Menu::new(
                element_id::scoped(&wrap_base_id, "menu-content"),
                self.items,
            )
            .id(element_id::scoped(&wrap_base_id, "menu"))
            .dropdown_composition()
            .focus_first(focus_first)
            .exiting(phase == crate::util::OverlayPhase::Exiting)
            .selection_mode(self.selection_mode)
            .selected_keys(self.selected_keys.clone())
            .disallow_empty_selection(self.disallow_empty_selection)
            .disabled_keys(self.disabled_keys.clone())
            .indicator(self.indicator);
            menu.item_content = self.item_content.clone();
            menu.indicator_content = self.indicator_content.clone();
            menu.item_start_content = self.item_start_content.clone();
            menu.recipes = self.recipes.clone();
            if let Some(row_hover_bg) = self.row_hover_bg {
                menu = menu.row_hover_bg(row_hover_bg);
            }
            if let Some(radius) = self.radius {
                menu = menu.radius(radius);
            }
            menu = menu.overlay_token(overlay_token);
            if let Some(on_action) = self.on_action.clone() {
                menu = menu.on_action(move |k, w, cx| on_action(k, w, cx));
            }
            let selection_cb = self.on_selection_change.clone();
            if selection_cb.is_some() || selection_own.is_some() {
                menu = menu.on_selection_change(move |keys, w, cx| {
                    if let Some(held) = &selection_own {
                        held.update(cx, |value, cx| {
                            *value = keys.to_vec();
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &selection_cb {
                        cb(keys, w, cx);
                    }
                });
            }
            let dismiss_cb = self.on_open_change.clone();
            if dismiss_cb.is_some() || dismiss_own.is_some() {
                let back_to_trigger = trigger_handle;
                menu = menu.on_dismiss(move |refocus, window, cx| {
                    if let Some(held) = &dismiss_own {
                        held.update(cx, |v, cx| {
                            *v = false;
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &dismiss_cb {
                        cb(&false, window, cx);
                    }
                    // The menu held the focus for its arrows; hand it back.
                    // An Enter pick runs this inside the key event, where gpui
                    // would activate the trigger on key up and reopen the menu
                    // -- the keyboard path asks for no refocus for that reason.
                    if *refocus {
                        window.focus(&back_to_trigger, cx);
                    }
                });
            }
            // RAC renders the menu in a `Popover` against the trigger: an 8px
            // gap (`offset ?? 8` in pinned RAC 1.20.0's `Popover`, which `Menu`
            // passes no offset to), flipping to the side with more room when
            // the menu cannot fit and capping at the available viewport height
            // past the 12px container padding. The shared `scrollable_popover`
            // owns that contract -- including `Left`/`Right`, which the old
            // cross-axis-only shift left to overflow -- and the panel's
            // `max_h_full` keeps short menus at their natural height. The menu
            // itself is the positioner's child: a wrapper between them would
            // measure and cap while the panel kept its natural size (and its
            // scroll range would collapse to the padding).
            root = root.child(crate::util::floating(crate::popover::scrollable_popover(
                anchor_bounds,
                self.placement,
                menu.panel_debug_label("dropdown-menu"),
            )));
        }

        crate::util::apply_sx(root, &self.sx)
    }
}

// The pinned `.menu-item:hover` fills with `bg-default`, the full token.
// `soft()` is a lighter wash that looks plausible on screen, so the check is
// mechanical.
#[cfg(test)]
mod hover_tokens {
    #[test]
    fn menu_rows_hover_the_full_default() {
        // Scan the implementation only.
        let source = include_str!("dropdown.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source
                .contains("let row_hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);"),
            "menu rows must default to the full `bg-default` and honor the \
             named override (pinned `.menu-item:hover`)"
        );
        assert!(
            source.contains("row = row.hover(move |s| s.bg(row_hover_bg));"),
            "the row hover must consume the resolved fill"
        );
        assert!(
            !source.contains("colors.default.soft()"),
            "no menu surface may hover a soft token"
        );
    }
}

crate::util::impl_component_styled!(Menu, Dropdown);
