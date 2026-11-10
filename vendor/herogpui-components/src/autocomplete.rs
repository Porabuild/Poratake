//! Autocomplete — port of `@heroui/autocomplete`.
//!
//! v3's Autocomplete is a **Select whose popover holds a search field**, not a
//! text field with suggestions under it. `autocomplete.css` says so directly:
//! `.autocomplete__trigger` is a field-shaped box (`min-h-9 rounded-field
//! bg-field px-3 shadow-field`) holding `.autocomplete__value` and a chevron
//! `.autocomplete__indicator`, and `.autocomplete__popover` stacks
//! `[data-slot="search-field"]` above the list. The text field *with* a list is
//! the [`crate::combo_box::ComboBox`], which is a separate component with its
//! own stylesheet.
//!
//! This port had it the other way round -- an
//! [`Input`](crate::input::Input) with a suggestion panel
//! -- which drew none of that sheet and left the trigger nothing to show the
//! selection in. The trigger draws the selection now, and the popover searches.
//!
//! [`InputState`] therefore backs the **search field inside the popover**, which
//! is what `Autocomplete.Filter`'s `inputValue` and `onInputChange` address. The
//! selection is a set of item keys, held by `value` / `defaultValue`.
//!
//! Pinned v3.2.4 / React Aria Components 1.20.0 keep a stable `Key` separate
//! from each item's `textValue`: `value` / `defaultValue` / `disabledKeys`,
//! the selection callbacks and the form value address items by key, while
//! filtering and the visible text use the label. Items are therefore
//! [`crate::PickerItem`]s; using a label as the key made duplicate labels alias
//! each other's selection, disabled state and row identity.
//!
//! Pinned react-stately 3.49.0's `useSelectState` holds `selectedKeys` as a
//! JavaScript `Set`, which iterates in insertion order: `selectedItems`,
//! `selectedText`, the selection callbacks and the form value all follow the
//! *selection's* order, not the collection's. The selection is therefore an
//! ordered unique key list, and toggling removes in place or appends.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

use gpui::{
    prelude::*, px, AnimationExt, App, Entity, IntoElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use herogpui_core::{element_id, FieldVariant, Placement, SelectionMode};
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    icons,
    input::{InputState, SearchField},
    matches::{empty_matches, MatchesCache},
    picker_item::PickerItem,
    selection::{normalize_selection, toggle_key},
    util,
};

type OnSelectionChange = std::sync::Arc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;
type AutocompleteFormState = Rc<RefCell<crate::form::LiveFormFieldState>>;

/// `.autocomplete__clear-button:not([data-empty="true"])` fades in over
/// 150ms with `--ease-smooth`; clearing itself remains an immediate hide.
const CLEAR_OPACITY_TRANSITION_MS: u64 = 150;

thread_local! {
    static AUTOCOMPLETE_FORM_STATES: RefCell<
        HashMap<u64, std::rc::Weak<RefCell<crate::form::LiveFormFieldState>>>,
    > = RefCell::new(HashMap::new());
}

fn autocomplete_form_state(entity_id: u64) -> AutocompleteFormState {
    AUTOCOMPLETE_FORM_STATES.with(|states| {
        let mut states = states.borrow_mut();
        if let Some(state) = states.get(&entity_id).and_then(|state| state.upgrade()) {
            return state;
        }
        let state = Rc::new(RefCell::new(crate::form::LiveFormFieldState {
            commit_validation: None,
            reset_validation: None,
            value: crate::form::FormValue::Keys(Vec::new()),
            is_invalid: false,
            is_successful: true,
            focus: None,
            restore: None,
        }));
        states.insert(entity_id, Rc::downgrade(&state));
        state
    })
}

fn form_selection_value(selected: &[SharedString]) -> crate::form::FormValue {
    crate::form::FormValue::Keys(selected.to_vec())
}

/// The rows the popover's list shows for `query`: the custom filter's own
/// decision — including what an empty query means — or the default
/// case-insensitive substring match. Filtering reads the items' labels, never
/// their keys.
fn compute_matches(
    items: &[PickerItem],
    query: &str,
    max_items: usize,
    filter: Option<&std::sync::Arc<dyn Fn(&str, &str) -> bool + 'static>>,
) -> Vec<PickerItem> {
    if let Some(filter) = filter {
        return items
            .iter()
            .filter(|item| filter(item.label(), query))
            .take(max_items)
            .cloned()
            .collect();
    }
    if query.is_empty() {
        return items.iter().take(max_items).cloned().collect();
    }
    let lowered = query.to_lowercase();
    items
        .iter()
        .filter(|it| it.label().to_lowercase().contains(&lowered))
        .take(max_items)
        .cloned()
        .collect()
}

fn sync_form_state(
    state: &AutocompleteFormState,
    selected: &[SharedString],
    is_disabled: bool,
    is_invalid: bool,
) {
    let mut state = state.borrow_mut();
    state.value = form_selection_value(selected);
    state.is_successful = !is_disabled;
    state.is_invalid = is_invalid;
}

/// HeroUI Autocomplete.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Autocomplete {
    /// `name` — the name this control submits under; read back by
    /// [`Self::form_field`].
    name: Option<SharedString>,
    /// The search field's text state — `Autocomplete.Filter`'s `inputValue`.
    state: Entity<InputState>,
    items: Vec<PickerItem>,
    max_items: usize,
    /// `ListLayout`'s `rowHeight`, which virtualizes the popover list.
    row_height: Option<Pixels>,
    /// Replaces the list rows' `px-2.5` horizontal padding.
    row_padding_x: Option<Pixels>,
    /// Replaces the list rows' `py-1.5` vertical padding.
    row_padding_y: Option<Pixels>,
    /// The fill a hovered row takes, in place of `--default`.
    row_hover_bg: Option<gpui::Hsla>,
    /// The trigger's hover endpoint, in place of the variant's hover token.
    trigger_hover_bg: Option<gpui::Hsla>,
    /// The clear button's hover fill, in place of `--default-hover`.
    clear_hover_bg: Option<gpui::Hsla>,
    /// The family the option rows are drawn with; unset keeps the
    /// inherited family. A detached popover does not inherit the trigger's
    /// font.
    row_font_family: Option<SharedString>,
    /// The trigger's and filter field's family; unset keeps the inherited one.
    font_family: Option<SharedString>,
    /// The corner radius of the detached panel, in place of the owning
    /// `picker_radius` helper.
    radius: Option<Pixels>,
    label: Option<SharedString>,
    placeholder: Option<SharedString>,
    description: Option<SharedString>,
    error_message: Option<SharedString>,
    variant: FieldVariant,
    full_width: bool,
    /// Optional trigger geometry/chrome overrides; defaults are the stock box.
    field: util::FieldBox,
    is_disabled: bool,
    is_read_only: bool,
    is_invalid: bool,
    is_required: bool,
    /// `disabledKeys` — suggestions that render but cannot be chosen,
    /// addressed by item key so a disabled item never disables its
    /// same-label sibling.
    disabled_keys: std::collections::HashSet<SharedString>,
    /// `shouldFocusWrap` — whether the arrow keys wrap at the ends of the list.
    should_focus_wrap: bool,
    /// `ListBox.Section` — a heading above the item with this key.
    sections: Vec<(SharedString, SharedString)>,
    /// `Autocomplete.Indicator` — replaces the trigger chevron. The closure is
    /// handed whether the popover is open.
    indicator: Option<Box<dyn Fn(bool) -> gpui::AnyElement + 'static>>,
    /// Composed `ListBox.ItemIndicator` — draws the selection tick. The closure
    /// is handed whether the row is selected.
    item_indicator: Option<Box<dyn Fn(bool) -> gpui::AnyElement + 'static>>,
    /// `Autocomplete.Value` — draws the trigger's value.
    value_content: Option<Box<dyn Fn(util::SelectionValue<'_>) -> gpui::AnyElement + 'static>>,
    /// `allowsEmptyCollection` — whether the autocomplete may function when
    /// the collection has no items at all. It is the `useSelectState` open
    /// gate, not a close-on-filtered-empty flag: filtering an open popover
    /// to zero keeps it mounted with the empty state either way.
    allows_empty_collection: bool,
    selection_mode: SelectionMode,
    /// The selection as ordered unique item keys — react-stately 3.49.0's
    /// `selectedKeys` is a JS `Set`, which iterates in insertion order, so
    /// callbacks, the form value and the trigger all follow the order the
    /// keys were picked (or the owner listed) in.
    selected_keys: Vec<SharedString>,
    /// Whether the caller drives the selection. An unset `value` is not an empty
    /// controlled selection: without this flag every uncontrolled Autocomplete
    /// would hand its own clicks back to a set nobody owns, and picking an item
    /// would do nothing.
    is_controlled: bool,
    /// `defaultValue` — set it to hand this component its own selection.
    default_value: Option<Vec<SharedString>>,
    on_selection_change_all:
        Option<std::sync::Arc<dyn Fn(&[SharedString], &mut Window, &mut App) + 'static>>,
    /// `isOpen`. `None` lets the trigger own it, seeded from `defaultOpen`.
    is_open: Option<bool>,
    default_open: bool,
    placement: Placement,
    on_open_change: Option<std::sync::Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    on_selection_change: Option<OnSelectionChange>,
    /// `filter` — decides whether an item matches the query. Defaults to a
    /// case-insensitive substring test.
    filter: Option<std::sync::Arc<dyn Fn(&str, &str) -> bool + 'static>>,
    input_value: Option<String>,
    on_input_change: Option<std::sync::Arc<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
    on_clear: Option<std::sync::Arc<dyn Fn(&mut Window, &mut App) + 'static>>,
    form_state: AutocompleteFormState,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Autocomplete {
    /// `selectionMode`
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// `defaultValue` — the uncontrolled initial selection.
    ///
    /// Supplying it hands the component its own selection set, seeded once;
    /// [`Self::value`] is the controlled spelling. The listed order is the
    /// selection's order, exactly as the owner listed it.
    pub fn default_value(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.default_value = Some(keys.into_iter().map(Into::into).collect());
        self
    }

    /// `value` — the controlled selection, as item keys. The listed order is
    /// the owner's order and is preserved everywhere the selection is read.
    pub fn value(mut self, keys: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.selected_keys = keys.into_iter().map(Into::into).collect();
        self.is_controlled = true;
        self
    }

    /// The `ListBox`'s spelling of [`Self::value`], for a caller that has a set.
    pub fn selected_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.selected_keys = keys.into_iter().collect();
        self.is_controlled = true;
        self
    }

    /// `onChange`'s complete `Key | Key[] | null` domain as a selection slice.
    /// Single selection reports zero or one key; multiple reports every key.
    pub fn on_selection_change_all(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change_all = Some(std::sync::Arc::new(handler));
        self
    }

    /// `placement` on `Autocomplete.Popover`.
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// `defaultOpen` — the popover starts open.
    pub fn default_open(mut self, v: bool) -> Self {
        self.default_open = v;
        self
    }

    /// `isOpen` — the controlled popover state.
    pub fn is_open(mut self, v: bool) -> Self {
        self.is_open = Some(v);
        self
    }

    /// `onOpenChange`
    pub fn on_open_change(
        mut self,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(std::sync::Arc::new(handler));
        self
    }

    /// `filter` on `Autocomplete.Filter` — replaces the default
    /// case-insensitive substring match.
    ///
    /// Called as `filter(item_label, input)`: v3 filters on the item's
    /// `textValue`, not on its key.
    pub fn filter(mut self, f: impl Fn(&str, &str) -> bool + 'static) -> Self {
        self.filter = Some(std::sync::Arc::new(f));
        self
    }

    /// `inputValue` on `Autocomplete.Filter` — the controlled search text.
    ///
    /// Unlike the bound [`InputState`] this does not write through, so a caller
    /// can hold the query itself.
    pub fn input_value(mut self, value: impl Into<String>) -> Self {
        self.input_value = Some(value.into());
        self
    }

    /// `onInputChange` on `Autocomplete.Filter` — every keystroke in the search
    /// field.
    pub fn on_input_change(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_input_change = Some(std::sync::Arc::new(f));
        self
    }

    /// `onClear` — called after `Autocomplete.ClearButton` clears selection.
    /// The button's clearing behavior does not depend on this callback.
    pub fn on_clear(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(std::sync::Arc::new(f));
        self
    }

    /// Pick-only single-key convenience callback.
    ///
    /// Use [`Self::on_selection_change_all`] for v3's complete `onChange`
    /// domain, including multiple selection and the empty value from clear.
    pub fn on_change(
        self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change(handler)
    }

    /// Creates an autocomplete over `items`, with `state` holding the input text.
    pub fn new(state: Entity<InputState>, items: Vec<PickerItem>) -> Self {
        let form_state = autocomplete_form_state(state.entity_id().as_u64());
        Self {
            name: None,
            state,
            items,
            max_items: 100,
            row_height: None,
            row_padding_x: None,
            row_padding_y: None,
            row_hover_bg: None,
            trigger_hover_bg: None,
            clear_hover_bg: None,
            row_font_family: None,
            font_family: None,
            radius: None,
            field: util::FieldBox::default(),
            label: None,
            placeholder: None,
            description: None,
            error_message: None,
            variant: FieldVariant::Primary,
            full_width: false,
            is_disabled: false,
            is_read_only: false,
            is_invalid: false,
            is_required: false,
            disabled_keys: std::collections::HashSet::new(),
            should_focus_wrap: false,
            sections: Vec::new(),
            indicator: None,
            item_indicator: None,
            value_content: None,
            allows_empty_collection: false,
            selection_mode: SelectionMode::Single,
            selected_keys: Vec::new(),
            is_controlled: false,
            default_value: None,
            on_selection_change_all: None,
            filter: None,
            input_value: None,
            on_input_change: None,
            on_clear: None,
            is_open: None,
            default_open: false,
            placement: Placement::BottomStart,
            on_open_change: None,
            on_selection_change: None,
            form_state,
            sx: None,
        }
    }

    /// `name` — the name this control submits under.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// The `Form` field this control submits, when it has a `name`.
    ///
    /// v3 discovers a field through the DOM; gpui gives a child no way to reach
    /// its ancestor, so the control hands the pair over instead. The live
    /// selection survives the next `Autocomplete::new` because it is keyed by
    /// the search-field entity, the way DateField keys its form state. A
    /// disabled control stays registered and is omitted from FormData.
    ///
    /// ```
    /// # use gpui::{prelude::*, Window};
    /// # use herogpui_components::{Autocomplete, Form, InputState, PickerItem};
    /// # struct Demo;
    /// # impl Render for Demo {
    /// #     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    /// #         let form = Form::new();
    /// #         let state = cx.new(|cx| InputState::new(cx));
    /// #         let control = Autocomplete::new(state, vec![PickerItem::new("rust", "Rust")])
    /// #             .name("lang");
    /// let field = control.form_field();
    /// form.field(field.unwrap()).child(control)
    /// #     }
    /// # }
    /// # let mut tcx = gpui::TestAppContext::single();
    /// # tcx.update(herogpui_theme::ThemeProvider::init);
    /// # let _ = tcx.add_window_view(|_, _| Demo);
    /// ```
    pub fn form_field(&self) -> Option<crate::form::FormField> {
        let name = self.name.clone()?;
        {
            let mut state = self.form_state.borrow_mut();
            state.is_successful = !self.is_disabled;
            state.is_invalid = self.is_invalid || self.error_message.is_some();
        }
        Some(
            crate::form::FormField::live(name, self.form_state.clone())
                .is_required(self.is_required),
        )
    }

    /// `ListLayout`'s `rowHeight` -- and what virtualizes the popover list.
    ///
    /// v3 wraps the list in `<Virtualizer layout={ListLayout}>` inside
    /// `Autocomplete.Popover`; gpui's `uniform_list` builds only the rows in
    /// view, and it can do that because every row is this tall.
    pub fn row_height(mut self, h: impl Into<Pixels>) -> Self {
        self.row_height = Some(h.into());
        self
    }

    /// Sets the maximum number of suggestions shown; values below 1 are treated as 1.
    pub fn max_items(mut self, n: usize) -> Self {
        self.max_items = n.max(1);
        self
    }

    /// Sets the label shown above the field.
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }

    /// Sets the input placeholder.
    pub fn placeholder(mut self, p: impl Into<SharedString>) -> Self {
        self.placeholder = Some(p.into());
        self
    }

    /// Sets the description shown below the field.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Sets the error message shown when the field is invalid.
    pub fn error_message(mut self, text: impl Into<SharedString>) -> Self {
        self.error_message = Some(text.into());
        self
    }

    /// Sets the field variant.
    pub fn variant(mut self, variant: FieldVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets whether the field fills the available width.
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self
    }

    /// Fixes the trigger box at `h`. Unset keeps the 36px `min-h-9`; content
    /// taller than an explicit height overflows the box.
    pub fn height(mut self, h: impl Into<Pixels>) -> Self {
        self.field.height = Some(h.into());
        self
    }

    /// Replaces the trigger's `px-3` horizontal padding. The trailing 28px
    /// keeps its room for the indicator.
    pub fn padding_x(mut self, p: impl Into<Pixels>) -> Self {
        self.field.padding_x = Some(p.into());
        self
    }

    /// Renders the trigger with no background, border, field shadow, ring,
    /// focus fill or hover fill, for a caller painting around it. The list
    /// still opens and selects.
    pub fn is_bare(mut self, v: bool) -> Self {
        self.field.is_bare = v;
        self.field.is_bare_is_set = true;
        self
    }

    /// Shows or hides only the trigger's visual focus ring. The trigger stays
    /// keyboard focusable and the list still opens when set to `false`.
    pub fn focus_ring(mut self, v: bool) -> Self {
        self.field.focus_ring = Some(v);
        self
    }

    /// Replaces the list rows' `px-2.5` horizontal padding.
    pub fn row_padding_x(mut self, p: impl Into<Pixels>) -> Self {
        self.row_padding_x = Some(p.into());
        self
    }

    /// Replaces the list rows' `py-1.5` vertical padding.
    pub fn row_padding_y(mut self, p: impl Into<Pixels>) -> Self {
        self.row_padding_y = Some(p.into());
        self
    }

    /// The trigger's fill while hovered, in place of `--field-hover`
    /// (`--default-hover` on the secondary variant). The 150ms ease-smooth
    /// fade, the border hover and the clear button's suppression are
    /// unchanged; a disabled or bare trigger does not hover. Not a v3 prop:
    /// v3 tints the trigger with a class.
    pub fn trigger_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.trigger_hover_bg = Some(color.into());
        self
    }

    /// The clear button's fill while hovered, in place of `--default-hover`.
    /// Its press scale is unchanged. Not a v3 prop: v3 tints the button with
    /// a class.
    pub fn clear_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.clear_hover_bg = Some(color.into());
        self
    }

    /// The fill a hovered row takes, in place of `--default`.
    pub fn row_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.row_hover_bg = Some(color.into());
        self
    }

    /// The family the trigger's value and placeholder, and the popover's
    /// filter field (query, placeholder and caret measurement), are drawn
    /// with; unset keeps the inherited family. The detached rows take
    /// [`Autocomplete::row_font_family`] instead. Not a v3 prop; v3 sets it
    /// with a class.
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// The family the option rows are drawn with; unset keeps the inherited
    /// family. A detached popover does not inherit the trigger's font.
    pub fn row_font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.row_font_family = Some(family.into());
        self
    }

    /// The corner radius of the detached panel, in place of the owning
    /// `picker_radius` helper. The panel's entry zoom interpolates the same
    /// value, so both follow the override. Not a v3 prop; the removed v2
    /// `radius` prop is prohibited and this is a per-component repository
    /// extension.
    ///
    /// The trigger is a field box of its own, painted by the shared field
    /// chrome — `--field-radius`, not this value.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the autocomplete's root element after every value the
    /// variant and the active theme chose, so they win. The trigger paints its
    /// own chrome, so this reaches the box that chrome sits in, not the chrome.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }

    /// Sets whether the field is disabled (`isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// A read-only Autocomplete shows its selection and does not open. Pinned
    /// v3 puts no read-only gate on the clear button part (only `disabled`),
    /// so a read-only control keeps a working clear button.
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.is_read_only = v;
        self
    }

    /// Sets whether the field is invalid (`isInvalid`).
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// Sets whether the field is required (`isRequired`).
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// `disabledKeys` — keys of the suggestions that render but cannot be
    /// chosen. Disabled state is per key, so one of two same-label items can
    /// be disabled alone.
    pub fn disabled_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.disabled_keys = keys.into_iter().collect();
        self
    }

    /// `shouldFocusWrap` — whether the arrow keys wrap at the ends of the list.
    pub fn should_focus_wrap(mut self, v: bool) -> Self {
        self.should_focus_wrap = v;
        self
    }

    /// `ListBox.Section` — a heading rendered above the item with this key.
    pub fn section_before(
        mut self,
        item: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        self.sections.push((item.into(), label.into()));
        self
    }

    /// `Autocomplete.Indicator` — draw the trigger indicator yourself.
    ///
    /// The closure receives the current open state, which is the GPUI analog of
    /// v3's `data-open` attribute on this part.
    pub fn indicator(mut self, render: impl Fn(bool) -> gpui::AnyElement + 'static) -> Self {
        self.indicator = Some(Box::new(render));
        self
    }

    /// Composed `ListBox.ItemIndicator` — draw the selected row tick yourself.
    pub fn item_indicator(mut self, render: impl Fn(bool) -> gpui::AnyElement + 'static) -> Self {
        self.item_indicator = Some(Box::new(render));
        self
    }

    /// `Autocomplete.Value` — draw the trigger's value yourself.
    ///
    /// The closure is handed the render props v3 passes into
    /// `<Autocomplete.Value>{({defaultChildren, isPlaceholder, selectedItems,
    /// selectedText}) => …}`, so a multiple selection can be drawn as tags.
    pub fn value_content(
        mut self,
        render: impl Fn(util::SelectionValue<'_>) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.value_content = Some(Box::new(render));
        self
    }

    /// `allowsEmptyCollection` — whether the autocomplete may function with a
    /// collection that has no items at all (v3: *"When true, the autocomplete
    /// can function even with no items."*).
    ///
    /// react-stately 3.49.0's `useSelectState` reads it as the `open`/`toggle`
    /// gate: a truly empty collection refuses to open without it. Filtering an
    /// open popover to zero is a different layer (the ListBox's empty-state
    /// slot), so this is not a close-on-filtered-empty flag.
    pub fn allows_empty_collection(mut self, v: bool) -> Self {
        self.allows_empty_collection = v;
        self
    }

    /// Sets the handler called with the chosen suggestion (`onSelectionChange`).
    pub fn on_selection_change(
        mut self,
        f: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(std::sync::Arc::new(f));
        self
    }
}

type OnOpenChange = std::sync::Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;
type OnSelectionChangeAll =
    std::sync::Arc<dyn Fn(&[SharedString], &mut Window, &mut App) + 'static>;

/// The controlled halves `render` resolves first: the frame's ids and shared
/// collection, the selection and the open flag. `controlled` takes `cx`
/// mutably, so these precede every other read.
struct AutoControlled {
    base: String,
    base_id: gpui::ElementId,
    items: Rc<[PickerItem]>,
    multiple: bool,
    selection_own: Option<Entity<Vec<SharedString>>>,
    open: bool,
    open_own: Option<Entity<bool>>,
    overlay_phase: util::OverlayPhase,
    dismissal_token: util::OverlayToken,
}

/// The clear button's keyed interaction and derived flags.
struct AutoClear {
    clear_slot: util::Interaction,
    clear_active: bool,
    clear_hovered: bool,
    clear_pressed: bool,
    reduce_motion: bool,
    clear_opacity: crate::anim::Tween<f32>,
}

/// Everything one Autocomplete frame shares between its painted parts: the
/// controlled state, the keyed handles, the resolved matches and cursor, the
/// clear button's state and the theme tokens. `render` resolves it once in
/// [`Autocomplete::frame`]; the trigger, its clear button, the key handler
/// and the popover all read this one copy.
struct AutoFrame {
    base: String,
    base_id: gpui::ElementId,
    items: Rc<[PickerItem]>,
    multiple: bool,
    selection_own: Option<Entity<Vec<SharedString>>>,
    open: bool,
    open_own: Option<Entity<bool>>,
    overlay_phase: util::OverlayPhase,
    dismissal_token: util::OverlayToken,
    resolved_placement: Rc<Cell<Option<Placement>>>,
    entry_placement: Placement,
    blur_scope: gpui::FocusHandle,
    focus_handle: Option<gpui::FocusHandle>,
    list_scroll_now: gpui::UniformListScrollHandle,
    panel_scroll_now: gpui::ScrollHandle,
    list_core: crate::list_core::ListCore,
    query_edit: Entity<Option<bool>>,
    plain_edit_key: Entity<bool>,
    clear: AutoClear,
    raw_query: String,
    matches: Rc<[PickerItem]>,
    cursor_at: Option<usize>,
    anchor_bounds: Rc<Cell<Option<gpui::Bounds<Pixels>>>>,
    colors: herogpui_theme::ThemeColors,
    layout: herogpui_theme::LayoutTheme,
    is_invalid: bool,
    can_open: bool,
    toggle_allowed: bool,
    trigger_pressed: Rc<Cell<bool>>,
}

impl RenderOnce for Autocomplete {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // One shared collection for the frame: the `'static` panel and event
        // closures clone the `Rc`, never the rows.
        let items: Rc<[PickerItem]> = std::mem::take(&mut self.items).into();
        let base = format!("autocomplete-{}", self.state.entity_id().as_u64());
        // The same identity as `base`, kept as structure for the ids that key
        // state rather than name a debug selector.
        let base_id =
            gpui::ElementId::named_usize("autocomplete", self.state.entity_id().as_u64() as usize);
        // `Autocomplete.Filter.inputValue` is controlled state. Keep the bound
        // search field on the owner's value while still reporting proposed
        // edits through `onInputChange`.
        if let Some(input_value) = self.input_value.clone() {
            let current = self.state.read(cx).value().to_owned();
            if current != input_value {
                self.state.update(cx, |state, cx| {
                    state.set_value(input_value);
                    cx.notify();
                });
            }
        }
        // `defaultValue` opts into the component holding its own selection;
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        // Both spellings normalize to the `Set` shape first: duplicates
        // collapse to their first insertion, and single mode keeps one key.
        // A controlled order is the owner's order — nothing is sorted.
        let multiple = self.selection_mode == SelectionMode::Multiple;
        self.selected_keys = normalize_selection(self.selected_keys.clone(), multiple);
        let (selection, selection_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base_id, "selection"),
            self.is_controlled.then(|| self.selected_keys.clone()),
            normalize_selection(self.default_value.clone().unwrap_or_default(), multiple),
        );
        self.selected_keys = selection;

        // `isOpen` / `defaultOpen`: the trigger owns the popover, the way
        // `.autocomplete__trigger` does in v3 -- the old port opened on focus,
        // which is a ComboBox's behaviour, not this one's.
        let (is_open, open_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base_id, "open"),
            self.is_open,
            self.default_open,
        );
        let open = is_open && !self.is_disabled;
        let (overlay_phase, dismissal_token) = util::overlay_scope(
            window,
            cx,
            element_id::scoped(&base_id, "overlay"),
            open,
            true,
        );
        let frame = self.frame(
            AutoControlled {
                base,
                base_id,
                items,
                multiple,
                selection_own,
                open,
                open_own,
                overlay_phase,
                dismissal_token,
            },
            window,
            cx,
        );

        // --- the trigger ----------------------------------------------------
        let mut field = self.trigger_field(&frame, window, cx);
        let (value_slot, selected_text) = self.trigger_value(&frame, cx);
        // `@heroui/react/dist/components/autocomplete/autocomplete.js` builds
        // this trigger from RAC `Select` + `Button` — v3's Autocomplete is a
        // Select whose popover holds a search field, not a ComboBox — so
        // `react-aria/.../select/useSelect.mjs` decides its contract through
        // `useMenuTrigger({type: 'listbox'})`: a native `<button>` with
        // `'aria-haspopup': 'listbox'`, `'aria-expanded': isOpen`,
        // `'aria-controls'`, named by its label and the drawn value. Only the
        // role, the name and `aria-expanded` have gpui builders.
        //
        // Stated here rather than at the head of the chain because
        // `selected_text` is what names it, and that is not known until the
        // value slot is resolved.
        field = field
            .a11y_named(
                a11y::Role::Button,
                &a11y::Name::maybe(self.label.clone())
                    .described(Some(SharedString::from(selected_text))),
            )
            .a11y_expanded(frame.open);
        field = field.child(value_slot);
        field = field.child(self.clear_button(&frame, cx));
        field = self.trigger_indicator_slot(field, &frame, window, cx);
        field = self.trigger_toggle(field, &frame);
        if let Some(handle) = &frame.focus_handle {
            field = util::record_focus_bounds(field, handle, window, cx);
        }

        // The popup anchors to the trigger bounds -- not to the
        // label-to-description wrapper root -- the way RAC's
        // `useOverlayPosition` positions against the trigger rect.
        // `scrollable_field_popover` below reads these bounds to flip and
        // cap the panel; the measure element itself only records them.
        let field = crate::popover::PopoverTriggerMeasure::new(field, frame.anchor_bounds.clone());

        let mut root = self.field_root(field, &frame);
        if frame.can_open {
            root = self.root_keys(root, &frame);
        }
        root = self.root_escape(root, &frame);

        // --- the popover ----------------------------------------------------
        // The popover's presence is the Select's open state and nothing else.
        // Filtering happens inside `Autocomplete.Filter`, which prunes only
        // the ListBox's rows. At zero this port draws the "No results found"
        // empty state used by v3's examples; `allowsEmptyCollection` is not a
        // close-on-filtered-empty flag. The panel carries its own
        // outside-press dismissal, so there is nothing to attach to the root
        // when it is unmounted.
        if frame.overlay_phase != util::OverlayPhase::Closed {
            root = root.child(self.popover(frame, cx));
        }

        util::apply_sx(root, &self.sx)
    }
}

impl Autocomplete {
    /// Resolves the frame's keyed handles, matches and cursor on top of the
    /// controlled state.
    fn frame(&self, controlled: AutoControlled, window: &mut Window, cx: &mut App) -> AutoFrame {
        let AutoControlled {
            ref base_id,
            ref items,
            open,
            ref open_own,
            ..
        } = controlled;
        // Field popover placement can flip during prepaint. Feed the resolved
        // physical side back into the next entry frame, just like Popover.
        let (resolved_placement, entry_placement) =
            crate::popover::field_placement_feedback(window, cx, base_id, self.placement);

        // `usePopover` closes when focus leaves the trigger-plus-panel scope.
        // Unlike Escape, blur leaves focus on its destination.
        let blur_close_own = open_own.clone();
        let blur_open_change = self.on_open_change.clone();
        let blur_scope = util::close_on_blur(window, cx, base_id, open, move |window, cx| {
            if let Some(held) = &blur_close_own {
                held.update(cx, |v, cx| {
                    *v = false;
                    cx.notify();
                });
            }
            if let Some(cb) = &blur_open_change {
                cb(&false, window, cx);
            }
        });

        // The trigger is what holds focus, so the open list can be walked with
        // the arrows. A disabled control leaves the tab order.
        let focus_handle = if self.is_disabled {
            None
        } else {
            Some(util::tab_stop_handle(
                element_id::scoped(base_id, "focus"),
                window,
                cx,
            ))
        };
        // Which row the keyboard is on, held as the item's *key* so the cursor
        // stays on the same item when the query filters or the caller reorders
        // the collection.
        let cursor = crate::list_core::ListCore::cursor_state(base_id, window, cx);
        // The previous frame's row keys, which the removed-row repair walks.
        let previous_rows = crate::list_core::ListCore::rows_state(base_id, window, cx);
        // React Aria keeps the focused row in view, and v3's list is
        // `overflow-y-auto`. The virtual list has its own handle kind, a
        // scrolling div the other. `use_keyed_state` takes `cx` mutably, so both
        // precede the theme tokens.
        let list_scroll =
            window.use_keyed_state(element_id::scoped(base_id, "list-scroll"), cx, |_, _| {
                gpui::UniformListScrollHandle::new()
            });
        let panel_scroll =
            window.use_keyed_state(element_id::scoped(base_id, "panel-scroll"), cx, |_, _| {
                gpui::ScrollHandle::new()
            });
        let list_scroll_now = list_scroll.read(cx).clone();
        let panel_scroll_now = panel_scroll.read(cx).clone();
        // The list's shared navigation core. Autocomplete is the one popup
        // that pages by viewport: its list element is its own scroller, so
        // PageUp/PageDown walk a one-viewport boundary rather than jumping to
        // an end. Its search field owns the typed text, so the list carries
        // no typeahead buffer. Built here so the cursor resolution and the
        // forward-typing seat below read and write through it.
        let list_core = crate::list_core::ListCore::new(
            cursor,
            None,
            self.should_focus_wrap,
            crate::list_core::PageKeys::Viewport,
            if self.row_height.is_some() {
                crate::list_core::ListScroll::Uniform {
                    handle: list_scroll_now.clone(),
                    row_height: self.row_height.unwrap_or_default(),
                }
            } else {
                crate::list_core::ListScroll::Panel(panel_scroll_now.clone())
            },
        );
        // v3 writes `<SearchField autoFocus>` inside `Autocomplete.Filter`, so
        // the query field takes the focus as the popover opens -- once per
        // opening, or it would take the focus back on every frame.
        let autofocused =
            window.use_keyed_state(element_id::scoped(base_id, "autofocus"), cx, |_, _| false);
        // SearchField's text callback and the bubbling key event cooperate to
        // classify the pending edit; see the block after `matches` below.
        let query_edit =
            window.use_keyed_state(element_id::scoped(base_id, "query-edit"), cx, |_, _| {
                None::<bool>
            });
        let plain_edit_key =
            window.use_keyed_state(element_id::scoped(base_id, "plain-edit-key"), cx, |_, _| {
                false
            });
        let clear = self.clear_state(base_id, window, cx);
        let search_focus = self.state.read(cx).focus_handle.clone();
        if open && !*autofocused.read(cx) {
            window.focus(&search_focus, cx);
            autofocused.update(cx, |v, _| *v = true);
        } else if !open && *autofocused.read(cx) {
            autofocused.update(cx, |v, _| *v = false);
        }

        // A controlled `inputValue` wins over whatever the search field holds.
        let raw_query = match &self.input_value {
            Some(v) => v.clone(),
            None => self.state.read(cx).value().to_owned(),
        };

        let matches = self.matches(&controlled, &raw_query, &query_edit, window, cx);
        // The stored cursor is the focused item's key; the row it lands on is
        // wherever that key sits in the filtered collection now, resolved
        // through the same core the keys use.
        let match_keys: Vec<SharedString> = matches.iter().map(|item| item.key().clone()).collect();
        let match_stops: Vec<usize> = (0..matches.len())
            .filter(|i| {
                matches
                    .get(*i)
                    .is_some_and(|item| !self.disabled_keys.contains(item.key()))
            })
            .collect();
        let match_labels: Vec<String> = matches
            .iter()
            .map(|item| item.label().to_string())
            .collect();
        let match_rows = crate::list_core::ListRows {
            keys: &match_keys,
            stops: &match_stops,
            labels: &match_labels,
        };
        // A highlighted row the caller removed repairs to the nearest enabled
        // neighbour. `all_keys` is the full unfiltered set, so a row hidden by
        // the query is not treated as removed.
        let all_keys: Vec<SharedString> = items.iter().map(|item| item.key().clone()).collect();
        list_core.repair_removed(&match_rows, &all_keys, &previous_rows, cx);
        let cursor_at = list_core.cursor_at(&match_rows, cx);

        // Forward typing while the popover is open puts the collection cursor
        // on its first enabled filtered row. react-aria 3.51.0 does this from
        // `useAutocomplete.onChange` only for forward input types, and clears
        // virtual focus for deletion, paste and history edits. GPUI exposes
        // neither a DOM input type nor one combined callback, so the actual
        // SearchField change and its bubbling unmodified character key mark
        // the edit together. A controlled prop update fires neither and cannot
        // masquerade as typing.
        if let Some(forward) = *query_edit.read(cx) {
            let next = if open && forward {
                matches
                    .iter()
                    .position(|item| !self.disabled_keys.contains(item.key()))
            } else {
                None
            };
            if cursor_at != next {
                let next_key = next.and_then(|i| matches.get(i)).map(|it| it.key().clone());
                list_core.set_cursor(next_key, cx);
                if let Some(next) = next {
                    list_core.reveal(next);
                }
            }
            query_edit.update(cx, |v, cx| {
                *v = None;
                cx.notify();
            });
        }
        if *plain_edit_key.read(cx) {
            plain_edit_key.update(cx, |v, _| *v = false);
        }

        let anchor_bounds: Rc<Cell<Option<gpui::Bounds<Pixels>>>> = window
            .use_keyed_state(element_id::scoped(base_id, "anchor-bounds"), cx, |_, _| {
                Rc::new(Cell::new(None))
            })
            .read(cx)
            .clone();

        // The theme tokens borrow `cx`, so they are copied out only after the
        // keyed-state updates above.
        let colors = cx.colors().clone();
        let layout = cx.layout().clone();

        let is_invalid = self.is_invalid || self.error_message.is_some();
        self.sync_form(&controlled, is_invalid, &focus_handle);
        let can_open = !self.is_disabled;
        // Whether the trigger's open acts are allowed at all: react-stately
        // 3.49.0's `useSelectState` guards `open`/`toggle` — *"Don't open if
        // the collection is empty"* — and v3's Autocomplete root is a RAC
        // `Select`, whose trigger calls `state.toggle()`. A collection with no
        // items therefore refuses every trigger open/toggle unless
        // `allowsEmptyCollection` lets the autocomplete function with no
        // items. This is the *unfiltered* collection: a query that prunes an
        // open popover to zero never reaches this gate.
        let toggle_allowed = self.allows_empty_collection || !items.is_empty();

        // Whether the pointer went down on the trigger (or on the clear
        // button inside it). The panel's outside-press dismissal treats the
        // trigger as outside its own bounds, so a press on an *open* popover's
        // trigger would dismiss it on the mouse-down *and* toggle it back open
        // through the trigger's own click on the mouse-up -- one press, two
        // contradictory reports. The trigger's capture-phase handler runs
        // before the panel's `on_mouse_down_out` in the same dispatch, so the
        // dismissal can see it and leave the close to the trigger's click.
        let trigger_pressed = Rc::new(Cell::new(false));
        let AutoControlled {
            base,
            base_id,
            items,
            multiple,
            selection_own,
            open,
            open_own,
            overlay_phase,
            dismissal_token,
        } = controlled;
        AutoFrame {
            base,
            base_id,
            items,
            multiple,
            selection_own,
            open,
            open_own,
            overlay_phase,
            dismissal_token,
            resolved_placement,
            entry_placement,
            blur_scope,
            focus_handle,
            list_scroll_now,
            panel_scroll_now,
            list_core,
            query_edit,
            plain_edit_key,
            clear,
            raw_query,
            matches,
            cursor_at,
            anchor_bounds,
            colors,
            layout,
            is_invalid,
            can_open,
            toggle_allowed,
            trigger_pressed,
        }
    }

    /// The clear button's keyed (hovered, pressed) slot and its fade.
    fn clear_state(
        &self,
        base_id: &gpui::ElementId,
        window: &mut Window,
        cx: &mut App,
    ) -> AutoClear {
        // The pinned trigger hover carries
        // `:not(:has(.autocomplete__clear-button:hover))`: while the pointer is
        // on the clear button inside the trigger, the trigger's own hover fill
        // is suppressed so the affordance does not double-hover the whole
        // field. gpui 0.2.2 has no `:has` analog and a parent hitbox stays
        // hovered while a child's is, so the clear button feeds its own hover
        // into this keyed (hovered, pressed) slot (`on_hover` dispatches with
        // the moved position, before the next paint) and the trigger's
        // refinement reads it. The same slot carries the press, for the pinned
        // `:active, &[data-pressed] { transform: scale(0.93) }`.
        let clear_slot = util::interaction(element_id::scoped(base_id, "clear-ix"), window, cx);
        // The slot is read before the theme tokens for the same reason the
        // other keyed states are: the normalization below takes `cx` mutably.
        // `.autocomplete__clear-button` stays mounted for as long as it is
        // composed: pinned v3 gates the part only through `disabled={isDisabled}`
        // and its own `data-empty` (`pointer-events-none opacity-0`), and
        // neither the part nor pinned react-stately 3.49.0's
        // `selectionManager.setSelectedKeys` knows a read-only gate (RAC
        // 1.20.0's `Select` has no `isReadOnly` at all), so a read-only
        // control keeps a working clear button.
        let clear_empty = self.selected_keys.is_empty();
        let clear_active = !clear_empty && !self.is_disabled;
        // A hover or press recorded on the button outlives the listener that
        // would clear it when the button goes inert (cleared, disabled): the
        // pointer can leave and the selection can flip with no event reaching
        // the detached handler. The inert frames normalize the slot back to
        // rest, so a stale flag cannot survive a clear-and-reselect.
        if !clear_active && *clear_slot.read(cx) != (false, false) {
            clear_slot.update(cx, |state, _| *state = (false, false));
        }
        let (clear_hovered, clear_pressed) = if clear_active {
            *clear_slot.read(cx)
        } else {
            (false, false)
        };
        let reduce_motion = ActiveTheme::reduce_motion(cx);
        let mut clear_opacity = crate::anim::Tween::keyed(
            base_id,
            "clear-opacity",
            if clear_empty { 0.0 } else { 1.0 },
            window,
            cx,
        );
        // HeroUI hides the clear button immediately when the selection is
        // emptied, but lets a newly visible button fade in. Keeping the
        // transition on a listener-free child preserves the stable 20px hit
        // target and lets a clear/reselect reversal resume from its painted
        // opacity.
        if clear_empty {
            clear_opacity.settle();
        } else {
            clear_opacity.snap_if_reduced(reduce_motion);
        }
        AutoClear {
            clear_slot,
            clear_active,
            clear_hovered,
            clear_pressed,
            reduce_motion,
            clear_opacity,
        }
    }

    /// The rows the list draws this frame, or none while nothing consumes
    /// them.
    fn matches(
        &self,
        controlled: &AutoControlled,
        raw_query: &str,
        query_edit: &Entity<Option<bool>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Rc<[PickerItem]> {
        let AutoControlled {
            ref base_id,
            ref items,
            overlay_phase,
            ..
        } = *controlled;
        let overlay_active = overlay_phase != util::OverlayPhase::Closed;
        // The list starts unfiltered: v3's popover shows the whole collection
        // until something is typed into the search field. Closed and idle
        // frames draw no rows, so they skip the match work entirely; a
        // consuming frame with a typed query shares one cached list until the
        // query, the collection or the cap changes. An empty query copies the
        // capped prefix directly: no per-row matching runs for it, so the
        // cache's element-by-element key comparison would cost more than the
        // work it saves. A custom filter owns the whole decision, including
        // what an empty query means, and its configuration cannot join a
        // cache key — its results are never cached and it only runs while the
        // matches are consumed.
        let matches_cache =
            window.use_keyed_state(element_id::scoped(base_id, "matches"), cx, |_, _| {
                MatchesCache::default()
            });
        let consume_matches = overlay_active || query_edit.read(cx).is_some();
        let matches: Rc<[PickerItem]> = if !consume_matches {
            empty_matches()
        } else {
            let filter = self.filter.clone();
            match &filter {
                Some(f) => Rc::from(compute_matches(items, raw_query, self.max_items, Some(f))),
                None if raw_query.is_empty() => {
                    Rc::from(compute_matches(items, raw_query, self.max_items, None))
                }
                None => matches_cache.update(cx, |cache, _| {
                    cache.get(items.clone(), raw_query, self.max_items, |items| {
                        compute_matches(items, raw_query, self.max_items, None)
                    })
                }),
            }
        };
        matches
    }

    /// Mirrors the selection into the live form state and installs the
    /// reset that restores the default selection.
    fn sync_form(
        &self,
        controlled: &AutoControlled,
        is_invalid: bool,
        focus_handle: &Option<gpui::FocusHandle>,
    ) {
        let AutoControlled {
            multiple,
            ref selection_own,
            ..
        } = *controlled;
        sync_form_state(
            &self.form_state,
            &self.selected_keys,
            self.is_disabled,
            is_invalid,
        );
        self.form_state.borrow_mut().focus = focus_handle.clone();
        let restore_own = selection_own.clone();
        let restore_state = Rc::downgrade(&self.form_state);
        let restore_default =
            normalize_selection(self.default_value.clone().unwrap_or_default(), multiple);
        let restore_all = self.on_selection_change_all.clone();
        self.form_state.borrow_mut().restore = (restore_own.is_some() || restore_all.is_some())
            .then(|| {
                util::shared(move |window: &mut Window, cx: &mut App| {
                    if let Some(state) = restore_state.upgrade() {
                        state.borrow_mut().value = form_selection_value(&restore_default);
                    }
                    if let Some(held) = &restore_own {
                        let set = restore_default.clone();
                        held.update(cx, |v, cx| {
                            *v = set;
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &restore_all {
                        cb(&restore_default, window, cx);
                    }
                }) as std::sync::Arc<dyn Fn(&mut Window, &mut App)>
            });
    }

    /// The `.autocomplete__trigger` box: geometry, field chrome, focus ring
    /// and hover fade.
    fn trigger_field(
        &self,
        frame: &AutoFrame,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let AutoFrame {
            ref base,
            ref base_id,
            ref focus_handle,
            clear: AutoClear { clear_hovered, .. },
            ref colors,
            ref layout,
            is_invalid,
            ..
        } = *frame;
        let field_box = self.field;
        // `radius` overrides the detached panel only; the trigger is a field
        // box painted by the shared field chrome.
        let trigger_radius = util::field_radius(cx);
        // `.autocomplete__trigger` is `relative isolate inline-flex min-h-9
        // rounded-field border bg-field px-3 py-2 text-sm shadow-field`, plus
        // `pe-7` because the indicator sits inside it.
        let mut field = gpui::div()
            .id(element_id::scoped(base_id, "trigger"))
            .when_some(self.font_family.clone(), |field, family| field.font_family(family))
            // Headless probe: the decision that gates the hover refinement
            // above, so a test can drive real hover coordinates and read the
            // rendered state without painted-color access.
            .debug_selector({
                let base = base.clone();
                move || format!("{base}-trigger-suppressed-{clear_hovered}")
            })
            .relative()
            .flex()
            .items_center()
            .gap(px(8.))
            .min_h(field_box.resolved_height())
            .when_some(field_box.height, |el, h| el.h(h))
            .px(field_box.resolved_padding_x())
            .pr(px(28.))
            .text_size(util::FIELD_TEXT)
            .line_height(px(20.));
        if !field_box.is_bare {
            field = util::apply_field_chrome(
                field,
                self.variant,
                is_invalid,
                false,
                Some(trigger_radius),
                cx,
            );
            // `.autocomplete__trigger:focus-visible` is `status-focused` -- the
            // offset ring, not a field's flush one, which is why the chrome
            // above is not told about the focus.
            if field_box.focus_ring.unwrap_or(true) {
                if let Some(handle) = &focus_handle {
                    // Overlay rather than spread shadow: the ring's corner is
                    // then concentric with `trigger_radius` instead of
                    // repeating the trigger's own radius four pixels out, and
                    // its edge is crisp instead of blurred.
                    field = util::ring_overlay_if_focused(
                        field,
                        handle,
                        true,
                        trigger_radius,
                        Vec::new(),
                        window,
                        cx,
                    );
                }
            }
        }
        if self.is_disabled {
            field = field.opacity(layout.disabled_opacity);
        } else {
            // The cursor is an affordance, not chrome: a bare trigger stays
            // clickable and keeps the themed pointer.
            field = field.cursor(util::interactive_cursor(cx));
            if !field_box.is_bare {
                let idle_bg = match self.variant {
                    FieldVariant::Primary => colors.field.background,
                    FieldVariant::Secondary => colors.default.color,
                };
                let hover_bg = self.trigger_hover_bg.unwrap_or(match self.variant {
                    FieldVariant::Primary => colors.field.hover(),
                    // `.autocomplete--secondary` hovers
                    // `--autocomplete-trigger-bg-hover: var(--default-hover)`.
                    FieldVariant::Secondary => colors.default.hover(),
                });
                let hover_border = colors.field.border_hover();
                // The trigger owns the stable focus/clear listeners. Animate
                // only its background fill with the pinned 150ms
                // `ease-smooth` curve; while the nested clear affordance is
                // hovered, suppress the parent endpoint just like v3's
                // `:not(:has(.autocomplete__clear-button:hover))` rule.
                field = crate::anim::hover_fade_with_duration_and_easing_suppressed(
                    field,
                    element_id::scoped(base_id, "trigger-hover-fade"),
                    (idle_bg, hover_bg),
                    None,
                    None,
                    (!clear_hovered).then_some(hover_border),
                    clear_hovered,
                    |fill| fill.rounded(trigger_radius),
                    Some(150),
                    crate::anim::HoverFadeEasing::EaseSmooth,
                    window,
                    cx,
                );
            }
        }
        if self.full_width {
            field = field.w_full();
        } else {
            // v3's trigger is `inline-flex` and every documented example sizes
            // it from the outside (`<Autocomplete className="w-[256px]">`).
            // There is no `className` here, so the trigger keeps a floor of its
            // own rather than collapsing onto the placeholder -- the same choice
            // `ComboBox` makes.
            field = field.min_w(px(180.));
        }
        field
    }

    /// `.autocomplete__value`: the selection's text, or the caller's slot.
    fn trigger_value(&mut self, frame: &AutoFrame, cx: &App) -> (gpui::AnyElement, String) {
        let AutoFrame {
            ref items,
            ref colors,
            ..
        } = *frame;
        // --- `.autocomplete__value` -----------------------------------------
        // The trigger renders the selection in the selection set's own order —
        // pinned react-stately 3.49.0's `selectedKeys` is a JS `Set`, whose
        // iteration order is insertion order, so `selectedItems`, the render
        // props' keys and `selectedText` follow the pick (or owner) order, not
        // the collection's. Each key resolves to its item wherever that item
        // now sits; a key whose item is not in the collection (still loading)
        // renders nothing, which is what v3's `selectedItems` does too.
        let mut selected_items: Vec<SharedString> = Vec::new();
        let mut selected_indices: Vec<usize> = Vec::new();
        for key in &self.selected_keys {
            if let Some((index, item)) = items.iter().enumerate().find(|(_, it)| it.key() == key) {
                selected_items.push(item.label().clone());
                selected_indices.push(index);
            }
        }
        let selected_key_order = self.selected_keys.clone();
        // `selectedText` — v3 joins with locale-aware separators; without CLDR
        // data this is a comma and a space.
        let selected_text = selected_items
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let is_placeholder = selected_items.is_empty();
        let placeholder = self
            .placeholder
            .clone()
            // v3's own default for this prop.
            .unwrap_or_else(|| crate::i18n::ui_string(crate::i18n::UiString::SelectPlaceholder, cx));
        // `.autocomplete__value` is `flex-1 text-start text-sm
        // wrap-break-word`, and `text-field-placeholder` while nothing is
        // chosen. Keep the value slot's min-content floor released so a
        // narrow trigger grows vertically for a long selected label.
        let default_children = gpui::div()
            .flex_1()
            .min_w_0()
            .whitespace_normal()
            .text_size(util::FIELD_TEXT)
            .line_height(px(20.))
            .text_color(if is_placeholder {
                colors.field.placeholder
            } else {
                colors.field.foreground
            })
            .child(if is_placeholder {
                placeholder.to_string()
            } else {
                selected_text.clone()
            })
            .into_any_element();
        let value_slot = match self.value_content.take() {
            Some(render) => gpui::div()
                .flex_1()
                .min_w_0()
                .child(render(util::SelectionValue {
                    selected_items: &selected_items,
                    selected_indices: &selected_indices,
                    selected_keys: Some(&selected_key_order),
                    selected_text: &selected_text,
                    is_placeholder,
                    default_children,
                }))
                .into_any_element(),
            None => default_children,
        };
        (value_slot, selected_text)
    }

    /// `.autocomplete__clear-button`: a stable 20px hit box over a scaled,
    /// fading visual.
    fn clear_button(&self, frame: &AutoFrame, cx: &App) -> gpui::Stateful<gpui::Div> {
        let AutoFrame {
            ref base,
            ref base_id,
            ref selection_own,
            clear:
                AutoClear {
                    ref clear_slot,
                    clear_active,
                    clear_pressed,
                    reduce_motion,
                    ref clear_opacity,
                    ..
                },
            ref colors,
            ..
        } = *frame;
        // `.autocomplete__clear-button` — mounted whenever the trigger is:
        // `data-empty` only makes it invisible and pointer-inert, and
        // `disabled={isDisabled}` only disables it. The pinned part is a
        // 20px `rounded-xl p-1` box holding the `size-3.5` (14px) glyph, and
        // `:active, &[data-pressed]` scales the whole button to 0.93 about
        // its center. gpui 0.2.2 cannot scale a div, so the 20px hit box
        // stays put for the pointer and a centered *visual* box carries the
        // scale while pressed.
        let clear_scale = if clear_pressed { 0.93 } else { 1. };
        let clear_visual = px(20. * clear_scale);
        let clear_glyph = px(14. * clear_scale);
        let clear_radius = px(f32::from(util::small_radius(cx)) * clear_scale);
        // `.autocomplete__clear-button:hover` fills with `bg-default-hover`,
        // the role-hover mix -- not the lighter soft-hover wash.
        let hover_bg = self.clear_hover_bg.unwrap_or(colors.default.hover());
        let clear_visual = gpui::div()
            .debug_selector({
                let base = base.clone();
                move || format!("{base}-clear-visual")
            })
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .size(clear_visual)
            .rounded(clear_radius)
            .when(clear_active, |el| el.hover(move |st| st.bg(hover_bg)))
            .child(
                gpui::svg()
                    .size(clear_glyph)
                    .path(icons::CLOSE)
                    .text_color(colors.muted),
            );
        let clear_visual = if clear_opacity.animates(reduce_motion) {
            let from = clear_opacity.from();
            let to = clear_opacity.target();
            let value = clear_opacity.value();
            clear_visual
                .with_animation(
                    element_id::indexed(base_id, "clear-opacity", clear_opacity.generation()),
                    gpui::Animation::new(Duration::from_millis(CLEAR_OPACITY_TRANSITION_MS))
                        .with_easing(crate::anim::ease_smooth()),
                    move |visual, delta| {
                        let next = from + (to - from) * delta;
                        value.set(next);
                        visual.opacity(next)
                    },
                )
                .into_any_element()
        } else {
            clear_opacity.settle();
            clear_visual
                .opacity(clear_opacity.target())
                .into_any_element()
        };
        let mut clear = gpui::div()
            .id(element_id::scoped(base_id, "clear"))
            // `autocomplete.js` hard-codes `"aria-label": "Clear selection"`
            // on this RAC `Button`, beside an `aria-hidden` that only hides
            // it while the selection is empty — and gpui has no
            // `aria-hidden`, so the port leaves the empty button in the tree
            // where upstream removes it from it.
            .a11y_named(
                a11y::Role::Button,
                &a11y::Name::labelled(crate::i18n::ui_string(
                    crate::i18n::UiString::ClearSelection,
                    cx,
                )),
            )
            // `.autocomplete__clear-button` is `h-6 w-6`
            // and then `size-5`, so 20px, `rounded-xl` and `p-1`
            // -- with the pinned `size-3.5` (14px) glyph inside.
            .size(px(20.))
            .p(px(4.))
            .rounded(util::small_radius(cx))
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .when(clear_active, |el| el.cursor(util::interactive_cursor(cx)))
            .debug_selector({
                let base = base.clone();
                move || format!("{base}-clear")
            })
            .child(clear_visual);
        if clear_active {
            let own = selection_own.clone();
            let selection_cb = self.on_selection_change_all.clone();
            let clear_cb = self.on_clear.clone();
            let clear_form_state = self.form_state.clone();
            clear = util::track_interaction(clear, clear_slot).on_click(move |_, window, cx| {
                // The button sits *inside* the trigger, so gpui
                // dispatches its click up to the trigger's own
                // `on_click` too -- and clearing is not an open
                // gesture (React Aria's trigger press is
                // pointer-bound, so a bubbled DOM click is inert
                // there).
                cx.stop_propagation();
                // Uncontrolled: drop our own selection too, or the
                // button would clear nothing.
                if let Some(held) = &own {
                    held.update(cx, |v, cx| {
                        v.clear();
                        cx.notify();
                    });
                    clear_form_state.borrow_mut().value = crate::form::FormValue::Keys(Vec::new());
                }
                if let Some(cb) = &selection_cb {
                    cb(&[], window, cx);
                }
                if let Some(cb) = &clear_cb {
                    cb(window, cx);
                }
            });
        }
        clear
    }

    /// The absolute `.autocomplete__indicator` end slot.
    fn trigger_indicator_slot(
        &mut self,
        mut field: gpui::Stateful<gpui::Div>,
        frame: &AutoFrame,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let AutoFrame {
            ref base_id,
            open,
            ref colors,
            ..
        } = *frame;
        // `.autocomplete__indicator` is `absolute inset-y-0 end-2 my-auto`, and
        // its glyph is `size-4`. HeroUI keeps one down-chevron in the tree and
        // rotates it over 150ms. Caller content still receives the live open
        // state and remains caller-owned.
        let trigger_indicator = match self.indicator.take() {
            Some(render) => render(open),
            None => crate::anim::rotating_indicator_with_duration(
                &element_id::scoped(base_id, "trigger-indicator"),
                open,
                gpui::svg()
                    .size(util::FIELD_ICON)
                    .path(icons::CHEVRON_DOWN)
                    .text_color(colors.field.placeholder),
                150,
                window,
                cx,
            ),
        };
        field = field.child(
            gpui::div()
                .absolute()
                .right(px(8.))
                .top_0()
                .bottom_0()
                .flex()
                .items_center()
                .justify_center()
                .text_color(colors.field.placeholder)
                .child(trigger_indicator),
        );
        field
    }

    /// The trigger's press: toggles the popover and reports the change.
    fn trigger_toggle(
        &self,
        mut field: gpui::Stateful<gpui::Div>,
        frame: &AutoFrame,
    ) -> gpui::Stateful<gpui::Div> {
        let AutoFrame {
            open,
            ref open_own,
            ref focus_handle,
            can_open,
            toggle_allowed,
            ref trigger_pressed,
            ..
        } = *frame;
        // Clicking the trigger opens and closes the popover. The toggle is
        // the `useSelectState.toggle()` act: an empty collection without the
        // prop refuses it in *both* directions, and the refusal reports
        // nothing (the guard sits before `triggerState.toggle()`, so
        // `onOpenChange` never fires).
        if can_open {
            let own = open_own.clone();
            let cb = self.on_open_change.clone();
            let was_open = open;
            let pressed = trigger_pressed.clone();
            let may_toggle = toggle_allowed;
            field = field
                .capture_any_mouse_down(move |_, _, cx| {
                    pressed.set(true);
                    let pressed = pressed.clone();
                    cx.defer(move |_| pressed.set(false));
                })
                .when_some(focus_handle.as_ref(), |el, handle| el.track_focus(handle))
                .on_click(move |_, window, cx| {
                    if !may_toggle {
                        return;
                    }
                    if let Some(held) = &own {
                        held.update(cx, |v, cx| {
                            *v = !was_open;
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &cb {
                        cb(&!was_open, window, cx);
                    }
                });
        }
        field
    }

    /// The `.autocomplete` wrapper column and the root that scopes blur.
    fn field_root(&self, field: impl IntoElement, frame: &AutoFrame) -> gpui::Div {
        let AutoFrame {
            ref blur_scope,
            is_invalid,
            ..
        } = *frame;
        // --- the wrapper: `.autocomplete` is `flex flex-col gap-1` -----------
        let mut wrapper = gpui::div().flex().flex_col().gap(px(4.)).w_full();
        if let Some(label) = &self.label {
            wrapper = wrapper.child(
                crate::field::Label::new(label.clone())
                    .is_required(self.is_required)
                    .is_disabled(self.is_disabled)
                    .is_invalid(is_invalid),
            );
        }
        wrapper = wrapper.child(field);
        if is_invalid {
            if let Some(message) = &self.error_message {
                wrapper = wrapper.child(crate::field::ErrorMessage::new(message.clone()));
            }
        } else if let Some(desc) = &self.description {
            wrapper = wrapper.child(crate::field::Description::new(desc.clone()));
        }

        let mut root = gpui::div().relative().child(wrapper);
        root = if self.full_width {
            root.w_full()
        } else {
            root.max_w(px(320.))
        };
        // The blur scope spans this one root, so a focus move between the
        // trigger and the search field inside the panel stays inside it.
        root = root.track_focus(blur_scope);
        root
    }

    /// The root's key handler: the list walk while the search field holds
    /// the focus, and the closed trigger's Down/Up open.
    fn root_keys(&self, root: gpui::Div, frame: &AutoFrame) -> gpui::Div {
        let AutoFrame {
            multiple,
            ref selection_own,
            open,
            ref open_own,
            ref list_core,
            ref query_edit,
            ref plain_edit_key,
            ref matches,
            toggle_allowed,
            ..
        } = *frame;
        let stops: Vec<usize> = (0..matches.len())
            .filter(|i| {
                matches
                    .get(*i)
                    .is_some_and(|item| !self.disabled_keys.contains(item.key()))
            })
            .collect();
        let keys: Vec<SharedString> = matches.iter().map(|item| item.key().clone()).collect();
        let labels: Vec<String> = matches
            .iter()
            .map(|item| item.label().to_string())
            .collect();
        let key_query_edit = query_edit.clone();
        let key_plain_edit = plain_edit_key.clone();
        let core = list_core.clone();
        let rows = matches.clone();
        let key_open_own = open_own.clone();
        let key_open_change = self.on_open_change.clone();
        let may_open = toggle_allowed;
        let on_change_all = self.on_selection_change_all.clone();
        let on_change_one = self.on_selection_change.clone();
        let key_selection_own = selection_own.clone();
        let key_form_state = self.form_state.clone();
        let selected_now = self.selected_keys.clone();
        let was_open = open;
        let handler = AutoKeys {
            stops,
            keys,
            labels,
            core,
            key_query_edit,
            key_plain_edit,
            rows,
            key_open_own,
            key_open_change,
            may_open,
            on_change_all,
            on_change_one,
            key_selection_own,
            key_form_state,
            selected_now,
            was_open,
            multiple,
        };
        root.on_key_down(move |event, window, cx| handler.on_key_down(event, window, cx))
    }

    /// Escape closes the popover and hands the focus back to the trigger.
    fn root_escape(&self, mut root: gpui::Div, frame: &AutoFrame) -> gpui::Div {
        let AutoFrame {
            ref open_own,
            ref dismissal_token,
            ref focus_handle,
            ..
        } = *frame;
        let escape_own = open_own.clone();
        let escape_cb = self.on_open_change.clone();
        let escape_focus = focus_handle.clone();
        root =
            util::dismiss_on_escape_with_token(root, dismissal_token.clone(), move |window, cx| {
                if let Some(held) = &escape_own {
                    held.update(cx, |v, cx| {
                        *v = false;
                        cx.notify();
                    });
                }
                if let Some(cb) = &escape_cb {
                    cb(&false, window, cx);
                }
                if let Some(handle) = &escape_focus {
                    window.focus(handle, cx);
                }
                util::DismissResult::Handled
            });
        root
    }

    /// The popover's clipping surface: `bg-overlay`, the panel radius, the
    /// dark-mode hairline and the overlay shadow.
    fn panel_surface(&self, base: &str, radius: Pixels, frame: &AutoFrame) -> gpui::Div {
        let AutoFrame {
            ref colors,
            ref layout,
            ..
        } = *frame;
        let panel_selector = format!("{base}-panel");
        gpui::div()
                .w_full()
                .flex()
                .flex_col()
                // `.autocomplete__popover` is `p-0 pt-2`: the search field and
                // the list bring their own padding.
                .pt(px(8.))
                .bg(colors.overlay.background)
                .rounded(radius)
                // v3 gives a floating panel no border: `.popover` and friends are
                // `bg-overlay shadow-overlay` and a radius, and dark mode's
                // inset hairline is what separates the panel from the page.
                .when_some(layout.overlay_hairline, |el, hairline| {
                    el.border(layout.border_width).border_color(hairline)
                })
                .shadow(layout.overlay_shadow.clone())
                .debug_selector(move || panel_selector)
                // `.autocomplete__popover` is `overflow-hidden
                // overscroll-contain`: the panel clips while the inner list
                // owns the scrolling, and a wheel over it never reaches the
                // page behind. RAC caps the popover at the available viewport
                // height past a 12px inset; the positioner below re-lays the
                // panel out with that cap, so the panel carries a
                // viewport-relative bound rather than a fixed one.
                .max_h_full()
                .overflow_hidden()
                .occlude()
    }

    /// The popover's search header: v3's `[data-slot="search-field"]`.
    fn search_row(&self, frame: &AutoFrame) -> gpui::Div {
        let AutoFrame {
            ref base,
            ref raw_query,
            ref query_edit,
            ref plain_edit_key,
            ..
        } = *frame;
        // The search field: v3's `[data-slot="search-field"]` inside the
        // popover is `shrink-0 px-3 py-1`, and `variant="secondary"` so it
        // reads as part of the panel rather than as a second field.
        let query_before_edit = raw_query.clone();
        let edit_query = query_edit.clone();
        let edit_key = plain_edit_key.clone();
        let input_change = self.on_input_change.clone();
        let search = SearchField::new(self.state.clone());
        let search = match self.font_family.clone() {
            Some(family) => search.font_family(family),
            None => search,
        };
        let search = search
            .variant(FieldVariant::Secondary)
            .placeholder("Search...")
            .is_read_only(self.is_read_only)
            .on_change(move |text, window, cx| {
                if text != query_before_edit {
                    let forward = *edit_key.read(cx);
                    edit_query.update(cx, |edit, cx| {
                        *edit = Some(forward);
                        cx.notify();
                    });
                }
                if let Some(cb) = &input_change {
                    cb(text, window, cx);
                }
            });
        gpui::div()
            .flex_shrink_0()
            .px(px(12.))
            .py(px(4.))
            .debug_selector({
                let base = base.clone();
                move || format!("{base}-search")
            })
            .child(search)
    }

    /// The popover: surface, dismissal, search header, rows and motion.
    fn popover(&mut self, frame: AutoFrame, cx: &mut App) -> gpui::Deferred {
        // The entry zoom interpolates the panel's own radius, so one
        // binding feeds both the painted shape and the animation.
        let radius = self.radius.unwrap_or_else(|| util::picker_radius(cx));
        let panel = self.panel_surface(&frame.base, radius, &frame);
        let search = self.search_row(&frame);
        let AutoFrame {
            base,
            base_id,
            multiple,
            selection_own,
            open_own,
            overlay_phase,
            dismissal_token,
            resolved_placement,
            entry_placement,
            focus_handle,
            list_scroll_now,
            panel_scroll_now,
            list_core,
            matches,
            cursor_at,
            anchor_bounds,
            colors,
            layout,
            trigger_pressed,
            ..
        } = frame;
        let overlay_exiting = overlay_phase == util::OverlayPhase::Exiting;
        // React Aria dismisses the popover on a press outside it; Escape is
        // read by the key handler above. A press that started on the
        // trigger (or its clear button) is not an outside press: the
        // trigger's own click owns the close, and the click only fires
        // because the down was not stolen as a dismissal.
        let dismiss_own = open_own.clone();
        let dismiss_cb = self.on_open_change.clone();
        let mut panel =
            util::dismiss_on_press_outside_with_token(panel, dismissal_token, move |window, cx| {
                if trigger_pressed.get() {
                    return util::DismissResult::Declined;
                }
                if let Some(held) = &dismiss_own {
                    held.update(cx, |v, cx| {
                        *v = false;
                        cx.notify();
                    });
                }
                if let Some(cb) = &dismiss_cb {
                    cb(&false, window, cx);
                }
                util::DismissResult::Handled
            });

        panel = panel.child(search);

        // Everything a row reads, owned: `uniform_list`'s callback is
        // `'static` and runs again on every scroll, so it cannot borrow
        // `self` or the theme -- and one row builder for both paths is what
        // keeps a virtual list drawing the same row as a short one.
        let matches_len = matches.len();
        // `useOption` adds `aria-posinset`/`aria-setsize` only
        // `if (isVirtualized)`; `row_height` is what windows this list.
        let row_virtualized = self.row_height.is_some();
        let rows = matches.clone();
        let sections = self.sections.clone();
        let row_disabled_keys = self.disabled_keys.clone();
        let row_selected_keys = self.selected_keys.clone();
        let indicator: Option<Rc<dyn Fn(bool) -> gpui::AnyElement>> =
            self.item_indicator.take().map(Rc::from);
        let on_change_all = self.on_selection_change_all.clone();
        let on_change_one = self.on_selection_change.clone();
        let row_selection_own = selection_own;
        let row_form_state = self.form_state.clone();
        let row_open_own = open_own;
        let row_open_change = self.on_open_change.clone();
        let row_trigger_focus = focus_handle;
        let base_row = format!("{base}-list");
        let base_row_id = element_id::scoped(&base_id, "list");
        let row_disabled_opacity = layout.disabled_opacity;
        // v3's `EmptyState` inside the popover is `text-center text-sm
        // text-overlay-foreground/60`. Copied out here because the row
        // builder below takes `cx` mutably, which ends the theme borrow.
        let mut empty_fg = colors.overlay.foreground;
        empty_fg.a *= 0.6;
        let row_padding_x = self.row_padding_x.unwrap_or(px(10.));
        let row_font_family = self.row_font_family.clone();
        let row_padding_y = self.row_padding_y.unwrap_or(px(6.));
        let rows = AutoRows {
            base_row,
            base_row_id,
            rows,
            sections,
            row_disabled_keys,
            row_selected_keys,
            indicator,
            on_change_all,
            on_change_one,
            row_selection_own,
            row_form_state,
            row_open_own,
            row_open_change,
            row_trigger_focus,
            colors,
            row_hover_bg: self.row_hover_bg,
            row_disabled_opacity,
            row_padding_x,
            row_font_family,
            row_padding_y,
            overlay_exiting,
            cursor_at,
            core: list_core,
            row_virtualized,
            matches_len,
            multiple,
        };

        // The list: `[data-slot="list-box"]` inside the popover is
        // `max-h-[320px] min-h-0 p-1.5 overflow-y-auto`. The search header
        // above stays fixed (`shrink-0`) while this list shrinks with the
        // capped panel and owns the scrolling -- the outer panel only
        // clips (`overflow-hidden`).
        panel = panel.child(self.rows_list(
            rows,
            &base,
            &base_id,
            &list_scroll_now,
            &panel_scroll_now,
            cx,
        ));

        if matches.is_empty() {
            panel = panel.child(
                gpui::div()
                        .w_full()
                        .px(px(12.))
                        .py(px(12.))
                        .text_center()
                        .text_size(util::FIELD_TEXT)
                        .line_height(px(20.))
                        .text_color(empty_fg)
                        // "No results found" in en-US.
                        .child(crate::i18n::ui_string(crate::i18n::UiString::NoResults, cx)),
            );
        }

        let (slide_x, slide_y) = crate::popover::placement_entry_offset(entry_placement);
        let zoom = crate::anim::ZoomBox::panel(px(6.), radius);
        let zoom = crate::anim::ZoomBox {
            slide_x: (slide_x != 0.0).then(|| px(slide_x)),
            slide_y: (slide_y != 0.0).then(|| px(slide_y)),
            ..zoom
        };
        let panel = if overlay_phase == util::OverlayPhase::Exiting {
            crate::anim::exiting(
                panel,
                element_id::scoped(&base_id, "panel-out"),
                zoom,
                crate::anim::Motion::FLUID_OUT,
                cx,
            )
        } else {
            crate::anim::entering_zoom(
                panel,
                element_id::scoped(&base_id, "panel"),
                zoom,
                crate::anim::Motion::FLUID_IN,
                cx,
            )
        };
        // RAC positions the popover against the trigger with an 8px gap,
        // flips it when the other side has more room, and caps it at the
        // available viewport height past a 12px inset -- which
        // `scrollable_field_popover` reads from the measured trigger
        // bounds above.
        util::floating(
            crate::popover::scrollable_field_popover_with_resolved_placement(
                anchor_bounds,
                self.placement,
                Some(resolved_placement),
                panel,
            ),
        )
    }

    /// The `[data-slot="list-box"]` rows: a windowed list under
    /// `row_height`, a scrolling column otherwise.
    fn rows_list(
        &self,
        rows: AutoRows,
        base: &str,
        base_id: &gpui::ElementId,
        list_scroll_now: &gpui::UniformListScrollHandle,
        panel_scroll_now: &gpui::ScrollHandle,
        cx: &mut App,
    ) -> gpui::AnyElement {
        let matches_len = rows.matches_len;
        match self.row_height {
            // Virtual: only the rows in view are built, which is what makes
            // a thousand options affordable. The list itself is the scroll
            // container: `Infer` sizes it from its rows -- the full
            // natural height on the positioner's measure pass (so the
            // flip sees the real extent, like upstream's `overlaySize`),
            // capped to the available height on the capped pass -- while
            // `max-h-[320px]` keeps the roomy-window height at the
            // upstream maximum and `min-h-0` lets it shrink with the
            // panel. A fixed inner height would strand rows outside a
            // capped panel.
            Some(row_height) => {
                let rows_selector = format!("{base}-rows");
                // The virtual half of the same `[data-slot="list-box"]`
                // the branch below draws. `gpui::uniform_list` returns a
                // `UniformList`, which is not a
                // `StatefulInteractiveElement` and so cannot carry a role
                // however many ids it has; the role goes on a wrapper that
                // adds no box of its own — `flex flex-col min-h-0` around
                // a `w-full` child lays out exactly as the child did.
                gpui::div()
                    .id(element_id::scoped(base_id, "list"))
                    .a11y(a11y::Role::ListBox)
                    .a11y_orientation(herogpui_core::Orientation::Vertical)
                    .flex()
                    .flex_col()
                    .w_full()
                    .min_h_0()
                    .child(
                        gpui::uniform_list(
                            element_id::scoped(base_id, "rows"),
                            matches_len,
                            move |range, _window, cx| {
                                range
                                    .map(|i| rows.row(i, Some(row_height), cx))
                                    .collect::<Vec<_>>()
                            },
                        )
                        .track_scroll(list_scroll_now)
                        .with_sizing_behavior(gpui::ListSizingBehavior::Infer)
                        .w_full()
                        .max_h(px(320.))
                        .min_h_0()
                        .p(px(6.))
                        .debug_selector(move || rows_selector),
                    )
                    .into_any_element()
            }
            None => {
                let list_selector = format!("{base}-list-scroll");
                let mut list = gpui::div()
                        .id(element_id::scoped(base_id, "list"))
                        // `[data-slot="list-box"]` is RAC's `ListBox`, i.e.
                        // `useListBox.mjs`'s literal `role: 'listbox'` with
                        // `'aria-orientation'` defaulting to vertical. The
                        // search field above it is outside the list upstream
                        // too, which is why the role is here and not on the
                        // popover panel.
                        .a11y(a11y::Role::ListBox)
                        .a11y_orientation(herogpui_core::Orientation::Vertical)
                        .debug_selector(move || list_selector)
                        .flex()
                        .flex_col()
                        .w_full()
                        .p(px(6.))
                        .max_h(px(320.))
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(panel_scroll_now);
                for index in 0..matches_len {
                    list = list.child(rows.row(index, None, cx));
                }
                list.into_any_element()
            }
        }
    }
}

/// The root's key handler, holding everything it reads. The handler is
/// `'static`, so it owns copies of the frame's state rather than borrowing
/// the Autocomplete.
struct AutoKeys {
    stops: Vec<usize>,
    keys: Vec<SharedString>,
    labels: Vec<String>,
    core: crate::list_core::ListCore,
    key_query_edit: Entity<Option<bool>>,
    key_plain_edit: Entity<bool>,
    rows: Rc<[PickerItem]>,
    key_open_own: Option<Entity<bool>>,
    key_open_change: Option<OnOpenChange>,
    may_open: bool,
    on_change_all: Option<OnSelectionChangeAll>,
    on_change_one: Option<OnSelectionChange>,
    key_selection_own: Option<Entity<Vec<SharedString>>>,
    key_form_state: AutocompleteFormState,
    selected_now: Vec<SharedString>,
    was_open: bool,
    multiple: bool,
}

impl AutoKeys {
    fn on_key_down(&self, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut App) {
        let Self {
            ref stops,
            ref keys,
            ref labels,
            ref core,
            ref key_query_edit,
            ref key_plain_edit,
            ref key_open_own,
            ref key_open_change,
            may_open,
            was_open,
            ..
        } = *self;
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let mut chars = key.chars();
        let plain_insert = was_open
            && (key == "space"
                || matches!(
                    (chars.next(), chars.next()),
                    (Some(ch), None) if !ch.is_control()
                ))
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.platform
            && !modifiers.function;
        key_plain_edit.update(cx, |v, cx| {
            if *v != plain_insert {
                *v = plain_insert;
                cx.notify();
            }
        });
        if plain_insert {
            key_query_edit.update(cx, |edit, cx| {
                if edit.is_some() {
                    *edit = Some(true);
                    cx.notify();
                }
            });
        }
        if !was_open {
            // Closed: Down and Up open it. Enter and Space are *not*
            // handled here -- the trigger has a click listener and gpui
            // fires those for a focused element, so answering them again
            // would open and close the popover in one keystroke.
            if matches!(key, "down" | "up") {
                // The keyboard open is the same
                // `useSelectState.open()` act: an empty collection
                // without `allowsEmptyCollection` refuses it and
                // reports nothing.
                if !may_open {
                    return;
                }
                if let Some(held) = &key_open_own {
                    held.update(cx, |v, cx| {
                        *v = true;
                        cx.notify();
                    });
                }
                if let Some(cb) = &key_open_change {
                    cb(&true, window, cx);
                }
            }
            return;
        }
        // The focused search field owns inserted characters. In
        // particular, the shared list navigator treats Space as an
        // activation key, but Autocomplete must insert it into the
        // query rather than select the current virtual row.
        if plain_insert || key == "space" {
            return;
        }
        // The list's navigation -- arrows, Home/End, and the viewport page
        // keys -- is the shared list core's; it moves the cursor and reveals
        // the row. Autocomplete is the one popup that pages by viewport (its
        // list element is its own scroller) and its search field owns the
        // typed text, so the core carries no typeahead buffer. Activation's
        // selection semantics stay here.
        let rows_view = crate::list_core::ListRows {
            keys,
            stops,
            labels,
        };
        match core.key_down(&rows_view, key, &modifiers, cx) {
            crate::list_core::ListKey::Moved { .. } => {}
            crate::list_core::ListKey::Activate(at) => self.activate(at, window, cx),
            crate::list_core::ListKey::Unhandled => {}
        }
    }

    /// Enter or Space on the cursor row: toggle the item in the selection
    /// (single mode replaces it) and report through the change callbacks; a
    /// single pick closes the popover, a multiple one stays open for the
    /// next pick.
    fn activate(&self, at: Option<usize>, window: &mut Window, cx: &mut App) {
        let Self {
            ref rows,
            ref key_open_own,
            ref key_open_change,
            ref on_change_all,
            ref on_change_one,
            ref key_selection_own,
            ref key_form_state,
            ref selected_now,
            multiple,
            ..
        } = *self;
        let Some(item) = at.and_then(|i| rows.get(i)) else {
            return;
        };
        let item_key = item.key().clone();
        let mut next = selected_now.clone();
        if multiple {
            toggle_key(&mut next, &item_key);
        } else {
            next.clear();
            next.push(item_key.clone());
        }
        if let Some(own) = &key_selection_own {
            let set = next.clone();
            own.update(cx, |v, cx| {
                *v = set;
                cx.notify();
            });
            key_form_state.borrow_mut().value = form_selection_value(&next);
        }
        if let Some(cb) = &on_change_one {
            cb(&item_key, window, cx);
        }
        if let Some(cb) = &on_change_all {
            cb(&next, window, cx);
        }
        // A single selection closes the popover, as v3's does;
        // a multiple one stays open for the next pick.
        if !multiple {
            if let Some(own) = &key_open_own {
                own.update(cx, |v, cx| {
                    *v = false;
                    cx.notify();
                });
            }
            if let Some(cb) = &key_open_change {
                cb(&false, window, cx);
            }
            // The focus is *not* moved back to the trigger here.
            // gpui activates a focused element on Enter, so
            // focusing the trigger inside this very keystroke
            // fires its click listener and the popover reopens --
            // observed, not theorised.
        }
    }
}

/// Everything an option row reads, owned: `uniform_list`'s callback is
/// `'static` and runs again on every scroll, so it cannot borrow the
/// Autocomplete or the theme -- and one row builder for both paths is what
/// keeps a virtual list drawing the same row as a short one.
struct AutoRows {
    base_row: String,
    base_row_id: gpui::ElementId,
    rows: Rc<[PickerItem]>,
    sections: Vec<(SharedString, SharedString)>,
    row_disabled_keys: std::collections::HashSet<SharedString>,
    row_selected_keys: Vec<SharedString>,
    indicator: Option<Rc<dyn Fn(bool) -> gpui::AnyElement>>,
    on_change_all: Option<OnSelectionChangeAll>,
    on_change_one: Option<OnSelectionChange>,
    row_selection_own: Option<Entity<Vec<SharedString>>>,
    row_form_state: AutocompleteFormState,
    row_open_own: Option<Entity<bool>>,
    row_open_change: Option<OnOpenChange>,
    row_trigger_focus: Option<gpui::FocusHandle>,
    colors: herogpui_theme::ThemeColors,
    row_hover_bg: Option<gpui::Hsla>,
    row_disabled_opacity: f32,
    row_padding_x: Pixels,
    row_font_family: Option<SharedString>,
    row_padding_y: Pixels,
    overlay_exiting: bool,
    cursor_at: Option<usize>,
    core: crate::list_core::ListCore,
    row_virtualized: bool,
    matches_len: usize,
    multiple: bool,
}

impl AutoRows {
    /// One option row, with its section header when one precedes it.
    fn row(&self, index: usize, fixed_h: Option<Pixels>, cx: &mut App) -> gpui::AnyElement {
        let Self {
            ref base_row,
            ref base_row_id,
            ref rows,
            ref sections,
            ref row_disabled_keys,
            ref row_selected_keys,
            ref indicator,
            ref colors,
            row_disabled_opacity,
            row_padding_x,
            ref row_font_family,
            row_padding_y,
            overlay_exiting,
            cursor_at,
            row_virtualized,
            matches_len,
            ..
        } = *self;
        let row_muted = colors.muted;
        let row_fg = colors.foreground;
        let row_hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);
        let row_focus = colors.focus;
        let row_accent = colors.accent.color;
        let base = base_row.as_str();
        let base_id = &base_row_id;
        let item = &rows[index];
        // A section header rides above the row it introduces, so the two
        // are one element -- a virtual row is one slot tall.
        let mut head: Vec<gpui::AnyElement> = Vec::new();
        let done = |head: Vec<gpui::AnyElement>, row: gpui::AnyElement| {
            gpui::div()
                .flex()
                .flex_col()
                .when_some(fixed_h, |el, h| el.h(h).w_full())
                .children(head)
                .child(row)
                .into_any_element()
        };
        // `ListBox.Section`'s `Header`, above the item it introduces.
        if let Some((_, label)) = sections.iter().find(|(at, _)| at == item.key()) {
            head.push(
                gpui::div()
                    .px(px(8.))
                    .pt(px(6.))
                    .pb(px(4.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(row_muted)
                    .child(label.to_string())
                    .into_any_element(),
            );
        }
        // The row's element id comes from the item's key, so two items
        // that share a label never share an interactive row.
        let item_disabled = row_disabled_keys.contains(item.key());
        let item_interactive = !item_disabled && !overlay_exiting;
        let row_selected = row_selected_keys.contains(item.key());
        let has_indicator_slot = indicator.is_some() || row_selected;
        let row_selector = format!("{base}-{}", item.key());
        let mut row = gpui::div()
                    .id(element_id::scoped(
                        &element_id::scoped(base_id, "opt"),
                        item.key().clone(),
                    ))
                    // `useOption.mjs`: `role: 'option'` plus `'aria-selected'`
                    // whenever the list selects at all. The search field keeps
                    // the real focus while a cursor walks the rows, which is
                    // upstream's virtual focus; gpui states that relation on
                    // the descendant, so the row can carry it even though this
                    // component does not own the field.
                    .a11y_named(
                        a11y::Role::ListBoxOption,
                        &a11y::Name::labelled(item.label().clone()),
                    )
                    .a11y_selected(row_selected)
                    .when(cursor_at == Some(index), |row| row.a11y_active_descendant())
                    .when(row_virtualized, |row| {
                        row.a11y_set_position(index, matches_len)
                    })
                    .debug_selector(move || row_selector)
                    .flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    // Every menu row in v3 is a `.list-box-item`: `min-h-9
                    // rounded-2xl py-1.5 gap-3` at `text-sm`, and the
                    // Autocomplete's popover restates the padding as `px-2.5`.
                    .min_h(util::FIELD_HEIGHT)
                    .rounded(util::soft_radius(cx))
                    .px(row_padding_x)
                    .py(row_padding_y)
                    .gap(px(12.))
                    .text_size(util::FIELD_TEXT)
                    .line_height(px(20.))
                    // HeroUI's list-box item reserves `pe-7` whenever its
                    // indicator slot is present; the indicator itself is
                    // absolute at the inline end. Keeping it out of flex
                    // flow prevents long labels from pushing the checkmark.
                    .relative()
                    .when(has_indicator_slot, |row| row.pr(px(28.)));
        if let Some(family) = row_font_family.clone() {
            row = row.font_family(family);
        }

        if item_disabled {
            row = row.opacity(row_disabled_opacity);
        } else if item_interactive {
            // Pointer hover seats the cursor for the next arrow key, the
            // `shouldFocusOnHover` the other popup lists share; the search
            // field keeps the real focus and only the cursor moves.
            row = self.core.seat_on_hover(
                row.cursor(util::interactive_cursor(cx))
                    .hover(move |s| s.bg(row_hover_bg)),
                item.key().clone(),
            );
        }
        if row_selected {
            row = row.text_color(row_accent);
        } else {
            row = row.text_color(row_fg);
        }
        // `status-focused` on the row the keyboard is on.
        if util::shows_focus_ring(cursor_at == Some(index), cx) {
            row = row.border_2().border_color(row_focus);
        }

        // HeroUI's ListBox.Item does not add an ellipsis rule. Keep
        // normal text flow in both natural and virtual rows; the
        // caller owns the fixed row geometry when `row_height` is
        // supplied, just as the upstream Virtualizer owns its
        // `rowHeight` layout.
        let label = gpui::div().flex_1().min_w_0().whitespace_normal();
        row = row.child(label.child(item.label().to_string()));

        // The chosen rows are ticked, unless `ListBox.ItemIndicator` is
        // drawn by the caller.
        match &indicator {
            Some(render) => {
                row = row.child(
                    gpui::div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right(px(8.))
                        .w(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(render(row_selected)),
                );
            }
            None if row_selected => {
                row = row.child(
                    gpui::div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right(px(8.))
                        .w(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            gpui::svg()
                                .size(px(13.))
                                .path(icons::CHECK)
                                .text_color(row_accent),
                        ),
                );
            }
            None => {}
        }

        if item_interactive {
            row = self.attach_pick(row, item.key().clone());
        }

        done(head, row.into_any_element())
    }

    /// The row's pointer pick: a toggle in multiple mode, a select-and-close
    /// that hands the focus back to the trigger in single mode.
    fn attach_pick(
        &self,
        mut row: gpui::Stateful<gpui::Div>,
        value: SharedString,
    ) -> gpui::Stateful<gpui::Div> {
        let Self {
            ref row_selected_keys,
            ref row_selection_own,
            ref row_form_state,
            ref on_change_all,
            ref on_change_one,
            ref row_open_own,
            ref row_open_change,
            ref row_trigger_focus,
            multiple,
            ..
        } = *self;
        let current = row_selected_keys.clone();
        let own = row_selection_own.clone();
        let row_form_state = row_form_state.clone();
        let cb_all = on_change_all.clone();
        let cb_one = on_change_one.clone();
        let open_own = row_open_own.clone();
        let open_cb = row_open_change.clone();
        let trigger_focus = row_trigger_focus.clone();
        row = row.on_click(move |_, window, cx| {
            let mut next = current.clone();
            if multiple {
                toggle_key(&mut next, &value);
            } else {
                next.clear();
                next.push(value.clone());
            }
            // Uncontrolled: keep the new set, or picking an item
            // would do nothing.
            if let Some(held) = &own {
                let set = next.clone();
                held.update(cx, |v, cx| {
                    *v = set;
                    cx.notify();
                });
                row_form_state.borrow_mut().value = form_selection_value(&next);
            }
            if let Some(cb) = &cb_one {
                cb(&value, window, cx);
            }
            if let Some(cb) = &cb_all {
                cb(&next, window, cx);
            }
            // A single selection closes the popover; a multiple one
            // stays open for the next pick.
            if !multiple {
                if let Some(held) = &open_own {
                    held.update(cx, |v, cx| {
                        *v = false;
                        cx.notify();
                    });
                }
                if let Some(cb) = &open_cb {
                    cb(&false, window, cx);
                }
                if let Some(handle) = &trigger_focus {
                    window.focus(handle, cx);
                }
            }
        });
        row
    }
}

// The pinned `.autocomplete--secondary` hover fill is
// `--autocomplete-trigger-bg-hover: var(--default-hover)` and the popup rows
// fill with the full `bg-default`. The two accessors differ by one word and
// the wrong one still looks plausible on screen, so the check is mechanical.
#[cfg(test)]
mod hover_tokens {
    #[test]
    fn secondary_trigger_and_menu_rows_use_the_pinned_hover_tokens() {
        // Scan the implementation only.
        let source = include_str!("autocomplete.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("FieldVariant::Secondary => colors.default.hover()"),
            "the secondary trigger hover must read `colors.default.hover()` \
             (pinned `--autocomplete-trigger-bg-hover: var(--default-hover)`)"
        );
        assert!(
            source
                .contains("let row_hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);"),
            "the popup rows must hover the full `bg-default` \
             (pinned `.list-box-item:hover`)"
        );
    }

    #[test]
    fn the_clear_button_hovers_the_role_hover_token() {
        // Scan the implementation only; this test's own text names the
        // forbidden accessor.
        let source = include_str!("autocomplete.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source
                .contains("let hover_bg = self.clear_hover_bg.unwrap_or(colors.default.hover());"),
            "the clear button must hover `bg-default-hover` \
             (pinned `.autocomplete__clear-button:hover`)"
        );
        assert!(
            !source.contains("colors.default.soft_hover()"),
            "the clear button must not hover the lighter soft-hover wash"
        );
    }

    #[test]
    fn the_trigger_hover_defers_to_a_hovered_clear_button() {
        // Painted-only claim, so it is pinned by source shape: headless tests
        // prove the wiring (real coordinates flip the keyed flag) but cannot
        // read the painted quad. The refinement must leave the trigger's
        // resting style untouched while the clear button is hovered, which is
        // what `:not(:has(.autocomplete__clear-button:hover))` does upstream.
        let source = include_str!("autocomplete.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("hover_fade_with_duration_and_easing_suppressed")
                && source.contains("clear_hovered,"),
            "the trigger hover must be suppressed while the clear button is hovered \
             (pinned `.autocomplete__trigger:hover:not(\
             :has(.autocomplete__clear-button:hover))`)"
        );
    }

    #[test]
    fn trigger_hover_uses_the_pinned_smooth_transition() {
        let source = include_str!("autocomplete.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("Some(150)")
                && source.contains("HoverFadeEasing::EaseSmooth")
                && source.contains("colors.field.border_hover()"),
            "the trigger hover must animate its background while retaining the border endpoint"
        );
    }

    #[test]
    fn clear_button_visibility_fades_in_on_a_listener_free_visual_child() {
        let source = include_str!("autocomplete.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("const CLEAR_OPACITY_TRANSITION_MS: u64 = 150;"),
            "the clear button must retain HeroUI's 150ms visibility duration"
        );
        assert!(
            source.contains("\"clear-opacity\"")
                && source.contains("clear_opacity.animates(reduce_motion)"),
            "clear visibility must use keyed reduced-motion-aware state"
        );
        assert!(
            source.contains("clear_opacity.settle();") && source.contains(".with_animation("),
            "clearing must hide immediately while appearance animates on the visual child"
        );
    }
}

crate::util::impl_component_styled!(Autocomplete);
