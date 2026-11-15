//! ComboBox — port of `@heroui/combo-box` (v3).
//!
//! A text input combined with a selectable list. Unlike
//! [`Select`](crate::select::Select) the value is typed, and unlike
//! [`Autocomplete`](crate::autocomplete::Autocomplete) the list can be opened
//! without typing and `allowsCustomValue` decides whether input outside the
//! collection is accepted.
//!
//! Pinned v3.2.4 / React Aria Components 1.20.0 keep a stable `Key` separate
//! from each item's `textValue`: `selectedKey` / `defaultValue` /
//! `disabledKeys`, the selection callbacks and the form value address items
//! by key, while filtering and the visible text use the label. Items are
//! therefore [`crate::PickerItem`]s; using a label as the key made duplicate
//! labels alias each other's selection, disabled state and row identity.
//! Pinned HeroUI's ComboBox composition adds only slots, trigger and popover
//! chrome on the RAC primitive, so it overrides none of this.
//!
//! The input text is the query and the selected item's label, never the key:
//! picking a row fills the field with that row's label, and the cursor and
//! row identities ride the key so duplicate labels stay distinct. A committed
//! custom value carries a null selected key — pinned react-stately 3.49.0's
//! `commitCustomValue` sets the value to `null` and keeps the typed text —
//! which the [`ComboBox::on_selection_change_all`] slice reports as an empty
//! selection, and only when a selection actually existed; the single-key
//! [`ComboBox::on_selection_change`] cannot spell `null` and stays silent there.
//!
//! `formValue` (pinned React Aria Components 1.20.0) defaults to `"key"`: a
//! named field submits the selected key(s). `allowsCustomValue` forces
//! `"text"`, submitting the typed text instead.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
    sync::Arc,
};

use gpui::{
    div, prelude::*, px, App, Entity, InteractiveElement, IntoElement, Pixels, RenderOnce,
    SharedString, Styled, Window,
};
use herogpui_core::{element_id, FieldVariant, Placement, SelectionMode};
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    icons,
    input::{Input, InputState},
    matches::{empty_matches, MatchesCache},
    picker_item::PickerItem,
    selection::{normalize_selection, toggle_key},
    util,
};

/// The port's local floor on the root field stack when `fullWidth` is off:
/// without a placeholder the inner input has no intrinsic width and the
/// trigger would collapse to just its chevron.
const TRIGGER_MIN_WIDTH: Pixels = px(180.);

/// When the suggestion list opens.
///
/// v3's table reads `"focus" | "input" | "manual"` with **`"focus"`** as the
/// default, so a v3 ComboBox shows its list as soon as the field is focused.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuTrigger {
    /// Open as soon as the field gains focus. v3's default.
    #[default]
    Focus,
    /// Open on input, and when the trigger button is pressed.
    Input,
    /// Open only when the trigger button is pressed.
    Manual,
}

/// `formValue` — what a `name`d ComboBox submits.
///
/// Pinned React Aria Components 1.20.0 defaults to key and forces text when
/// `allowsCustomValue` is set: an input that accepts values outside the
/// collection always submits what was typed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ComboBoxFormValue {
    /// The selected item key(s). The pinned default.
    #[default]
    Key,
    /// The typed input text.
    Text,
}

type OnSelectionChange = Arc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;
type OnOpenChange = Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;
type ComboBoxFormState = Rc<RefCell<crate::form::LiveFormFieldState>>;

thread_local! {
    static COMBO_BOX_FORM_STATES: RefCell<HashMap<u64, Weak<RefCell<crate::form::LiveFormFieldState>>>> =
        RefCell::new(HashMap::new());
}

fn combo_box_form_state(entity_id: u64) -> ComboBoxFormState {
    COMBO_BOX_FORM_STATES.with(|states| {
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

/// Whether the pinned `formValue` serialization submits the input text:
/// `formValue="text"` explicitly, or `allowsCustomValue`, which pinned React
/// Aria Components 1.20.0 turns into text regardless of the prop.
fn form_submits_text(form_value: Option<ComboBoxFormValue>, allows_custom_value: bool) -> bool {
    allows_custom_value || form_value == Some(ComboBoxFormValue::Text)
}

/// The one-shot that keeps `MenuTrigger::Focus` from reopening a panel the
/// user dismissed while the field still has the focus.
///
/// `can_open` is true while a fresh focus session may open the list; opening
/// consumes it. `was_open` remembers the flag from the last frame, so a panel
/// that closes while the field stays focused (Escape, a pick, the chevron, a
/// press outside) reads as a dismissal rather than as a new focus to answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FocusOpen {
    can_open: bool,
    was_open: bool,
}

/// The label an item key resolves to, or `None` when the key has no item in
/// the collection (still loading, or already removed).
fn label_of_key<'a>(items: &'a [PickerItem], key: &SharedString) -> Option<&'a SharedString> {
    items
        .iter()
        .find(|item| item.key() == key)
        .map(|item| item.label())
}

/// The rows the panel shows for `query`: the full collection under the
/// full-collection flag (which owns the frame, so no filter runs), otherwise
/// the custom `defaultFilter`'s decision — including what an empty query
/// means — or the default case-insensitive substring match. Filtering reads
/// the items' labels, never their keys.
fn compute_matches(
    items: &[PickerItem],
    query: &str,
    max_items: usize,
    full: bool,
    filter: Option<&Arc<dyn Fn(&str, &str) -> bool + 'static>>,
) -> Vec<PickerItem> {
    if full {
        return items.iter().take(max_items).cloned().collect();
    }
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
        .filter(|item| item.label().to_lowercase().contains(&lowered))
        .take(max_items)
        .cloned()
        .collect()
}

/// HeroUI ComboBox (controlled open state).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ComboBox {
    state: Entity<InputState>,
    items: Vec<PickerItem>,
    /// `isOpen` — `None` leaves the component holding the flag, seeded from
    /// `defaultOpen`.
    is_open: Option<bool>,
    default_open: bool,
    placement: Placement,
    label: Option<SharedString>,
    placeholder: Option<SharedString>,
    description: Option<SharedString>,
    error_message: Option<SharedString>,
    variant: FieldVariant,
    menu_trigger: MenuTrigger,
    /// `defaultFilter` — decides whether an item matches the query. Receives
    /// the item's label; the key never reaches it.
    filter: Option<Arc<dyn Fn(&str, &str) -> bool + 'static>>,
    allows_custom_value: bool,
    max_items: usize,
    full_width: bool,
    /// The root's width floor while not `full_width`; the default is 180px.
    min_width: Option<Pixels>,
    /// Optional trigger geometry/chrome overrides; defaults are the stock box.
    field: util::FieldBox,
    is_disabled: bool,
    is_invalid: bool,
    is_required: bool,
    is_read_only: bool,
    /// `autoFocus` — take focus on the first render.
    auto_focus: bool,
    /// `ListLayout`'s `rowHeight`, which virtualizes the popover list.
    row_height: Option<Pixels>,
    /// Replaces the list rows' `px-2` horizontal padding.
    row_padding_x: Option<Pixels>,
    /// Replaces the list rows' `py-1.5` vertical padding.
    row_padding_y: Option<Pixels>,
    /// The fill a hovered row takes, in place of `--default`.
    row_hover_bg: Option<gpui::Hsla>,
    /// The family the option rows are drawn with; unset keeps the
    /// inherited family. A detached popover does not inherit the trigger's
    /// font.
    row_font_family: Option<SharedString>,
    /// The trigger field text's family, forwarded to the inner `Input`.
    font_family: Option<SharedString>,
    /// The corner radius of the detached panel, in place of the owning
    /// `picker_radius` helper.
    radius: Option<Pixels>,
    /// `validate` — run by the component, not the caller.
    validate: Option<crate::validation::Validator<str>>,
    /// `validationBehavior` — carried on the inner field.
    validation_behavior: Option<crate::form::ValidationBehavior>,
    /// `allowsEmptyCollection` — keeps the panel up with no matches.
    allows_empty_collection: bool,
    /// `name` — the name this field submits under.
    name: Option<SharedString>,
    /// `formValue` — `None` is the pinned `"key"` default; `allowsCustomValue`
    /// forces text whatever this says.
    form_value: Option<ComboBoxFormValue>,
    /// `shouldFocusWrap` — whether the arrow keys wrap at the ends of the list.
    should_focus_wrap: bool,
    /// `ListBox.Section` — a heading above the item with this key.
    sections: Vec<(SharedString, SharedString)>,
    /// `ListBox.ItemIndicator` — draws the tick. The closure is handed whether
    /// the row is the selected one.
    indicator: Option<Box<dyn Fn(bool) -> gpui::AnyElement + 'static>>,
    disabled_keys: std::collections::HashSet<SharedString>,
    selection_mode: SelectionMode,
    /// The selection as ordered unique item keys — pinned react-stately
    /// 3.49.0's `selectedKeys` is a JS `Set`, which iterates in insertion
    /// order, so the callbacks, the form value and `ComboBox.Value` follow
    /// the order the keys were picked (or the owner listed) in.
    selected_keys: Vec<SharedString>,
    /// Whether the caller drives the selection. An unset `selected_keys` is not
    /// an empty controlled selection: without this flag every plain ComboBox
    /// would hand its own picks back to a set nobody owns, and
    /// `ComboBox.Value` would never see them.
    is_controlled: bool,
    /// Whether `selected_key` owns the controlled single key. Only then does
    /// the render sync the input text to the key's label — and only when the
    /// key changes, never on the owner's other re-renders.
    selected_key_sync: bool,
    /// `ComboBox.Value` — draws the chosen item under the field.
    value_content: Option<Box<dyn Fn(util::SelectionValue<'_>) -> gpui::AnyElement + 'static>>,
    /// `defaultValue` — set it to hand this component its own selection.
    default_value: Option<Vec<SharedString>>,
    /// `defaultInputValue` — seeds the text state on the first render only.
    default_input_value: Option<SharedString>,
    /// `value` — the v3 alias of the controlled input text, stored for the
    /// first render only and winning over `defaultInputValue`.
    value: Option<SharedString>,
    on_selection_change_all: Option<Arc<dyn Fn(&[SharedString], &mut Window, &mut App) + 'static>>,
    on_input_change: Option<Arc<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
    on_selection_change: Option<OnSelectionChange>,
    on_open_change: Option<OnOpenChange>,
    form_state: ComboBoxFormState,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl ComboBox {
    /// `selectionMode`
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// `defaultValue` — the uncontrolled initial selection, as item keys.
    ///
    /// Supplying it hands the component its own selection set, seeded once;
    /// [`Self::selected_keys`] is the controlled spelling. The listed order is
    /// the selection's order, exactly as the owner listed it.
    /// `defaultInputValue` — the uncontrolled initial text.
    ///
    /// Written into the state on the first render only; [`Self::input_value`]
    /// is the controlled spelling.
    pub fn default_input_value(mut self, text: impl Into<SharedString>) -> Self {
        self.default_input_value = Some(text.into());
        self
    }

    /// `defaultSelectedKeys` — the uncontrolled initial selection, as item keys.
    pub fn default_value(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.default_value = Some(keys.into_iter().map(Into::into).collect());
        self
    }

    /// `selectedKeys` — the controlled selection, as item keys. The listed
    /// order is the owner's order and is preserved everywhere the selection
    /// is read.
    pub fn selected_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.selected_keys = keys.into_iter().collect();
        self.is_controlled = true;
        self
    }

    /// Reports the whole selection: one key for a changed single pick by
    /// pointer, Enter or Tab, and all keys in multiple mode. Re-picking the
    /// current single key does not report a value change. A cleared input or
    /// committed custom value reports an empty slice only when a selection
    /// existed, matching React Stately 3.50.0's `onChange`/`null` contract.
    pub fn on_selection_change_all(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change_all = Some(Arc::new(handler));
        self
    }

    /// `selectedKey` — the controlled single-selection key, or `null` spelled
    /// as the empty string. The selection rides the key; the input text is
    /// synced to the key's label only when that key actually changes, so an
    /// owner that passes the same key every render never clobbers the text
    /// being typed — pinned react-stately resets the input value when the
    /// selected key changes and leaves the input alone otherwise.
    pub fn selected_key(mut self, key: impl Into<String>) -> Self {
        let key = SharedString::from(key.into());
        self.selected_keys = if key.is_empty() {
            Vec::new()
        } else {
            vec![key]
        };
        self.is_controlled = true;
        self.selected_key_sync = true;
        self
    }

    /// `value` — the v3 alias of [`ComboBox::input_value`], the controlled
    /// input text, as a pure builder.
    pub fn value(self, value: impl Into<String>) -> Self {
        self.input_value(value)
    }

    /// `disabledKeys` — keys of the items that render but cannot be chosen.
    /// Disabled state is per key, so one of two same-label items can be
    /// disabled alone.
    pub fn disabled_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.disabled_keys = keys.into_iter().collect();
        self
    }

    /// `autoFocus` — take focus on the first render.
    /// `ListLayout`'s `rowHeight` -- and what virtualizes the popover list.
    ///
    /// v3 wraps the list in `<Virtualizer layout={ListLayout}>` inside the
    /// popover; a uniform [`VirtualList`](crate::VirtualList) builds only the rows in view, and it can do
    /// that because every row is this tall.
    pub fn row_height(mut self, h: impl Into<Pixels>) -> Self {
        self.row_height = Some(h.into());
        self
    }

    /// Focuses the input when the combo box mounts.
    pub fn auto_focus(mut self, v: bool) -> Self {
        self.auto_focus = v;
        self
    }

    /// `validate` — returns the message to show, or `None` when the text is
    /// fine. The component runs it and surfaces the result.
    pub fn validate(mut self, f: impl Fn(&str) -> Option<SharedString> + 'static) -> Self {
        self.validate = Some(Arc::new(f));
        self
    }

    /// `validationBehavior` — `Allow` shows the message without blocking a
    /// form submission.
    pub fn validation_behavior(mut self, behavior: crate::form::ValidationBehavior) -> Self {
        self.validation_behavior = Some(behavior);
        self
    }

    /// `allowsEmptyCollection` — keeps the panel open when nothing matches.
    pub fn allows_empty_collection(mut self, v: bool) -> Self {
        self.allows_empty_collection = v;
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

    /// `ListBox.ItemIndicator` — draw the selected tick yourself.
    pub fn indicator(mut self, render: impl Fn(bool) -> gpui::AnyElement + 'static) -> Self {
        self.indicator = Some(Box::new(render));
        self
    }

    /// `ComboBox.Value` — draw the chosen item under the field.
    ///
    /// v3's `.combo-box__value` is an optional part (`text-sm
    /// text-field-foreground empty:hidden`), and the closure is handed the same
    /// render props the component passes down: `selectedItems` reaches
    /// `selected_items`, and `defaultChildren` is the row this port would draw.
    pub fn value_content(
        mut self,
        render: impl Fn(util::SelectionValue<'_>) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.value_content = Some(Box::new(render));
        self
    }

    /// `name` — the name this field submits under.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// `formValue` — whether a `name`d field submits the selected key(s) or
    /// the typed text. `allowsCustomValue` forces the text whatever this says,
    /// as pinned React Aria Components 1.20.0 does.
    pub fn form_value(mut self, form_value: ComboBoxFormValue) -> Self {
        self.form_value = Some(form_value);
        self
    }

    /// The `Form` field this control submits, when it has a `name`.
    ///
    /// v3 discovers a field through the DOM; gpui gives a child no way to
    /// reach its ancestor, so the control hands the pair over instead. The
    /// live value follows the pinned `formValue` serialization — the selected
    /// key(s) by default, the typed text under `allowsCustomValue` — and the
    /// input keeps the implicit-Enter submission of the text control it is.
    /// A disabled control stays registered and is omitted from FormData.
    pub fn form_field(&self) -> Option<crate::form::FormField> {
        let name = self.name.clone()?;
        Some(
            crate::form::FormField::live_text(name, self.form_state.clone(), self.state.clone())
                .is_required(self.is_required),
        )
    }

    /// Makes the field read-only (v3 `isReadOnly`).
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.is_read_only = v;
        self
    }

    /// `inputValue` — the controlled input text, as a pure builder.
    ///
    /// The bound [`crate::InputState`] owns the text once the field renders,
    /// so this seeds the state on the first render only, winning over
    /// [`ComboBox::default_input_value`] the way v3's controlled prop outranks
    /// the uncontrolled seed; calling `.input_value(..)` twice keeps the last
    /// call, like every other builder here. [`ComboBox::value`] is v3's alias
    /// and writes the same slot. A later value is an imperative update rather
    /// than a builder: `state.update(cx, |s, _| s.set_value(..))`.
    pub fn input_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(SharedString::from(value.into()));
        self
    }

    /// `onInputChange` — fires as the text changes, where
    /// [`ComboBox::on_selection_change`] fires only on a pick.
    pub fn on_input_change(
        mut self,
        handler: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_input_change = Some(Arc::new(handler));
        self
    }

    /// Single-key convenience alias for [`ComboBox::on_selection_change`].
    /// Use [`Self::on_selection_change_all`] for the complete `onChange` value,
    /// including an empty selection and multiple keys.
    pub fn on_change(
        self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change(handler)
    }

    /// Creates a combo box over the given input state and items.
    pub fn new(state: Entity<InputState>, items: Vec<PickerItem>) -> Self {
        let form_state = combo_box_form_state(state.entity_id().as_u64());
        Self {
            state,
            items,
            is_open: None,
            default_open: false,
            placement: Placement::BottomStart,
            label: None,
            placeholder: None,
            description: None,
            error_message: None,
            variant: FieldVariant::Primary,
            menu_trigger: MenuTrigger::Focus,
            filter: None,
            allows_custom_value: false,
            max_items: 8,
            full_width: false,
            min_width: None,
            is_disabled: false,
            is_invalid: false,
            is_required: false,
            is_read_only: false,
            auto_focus: false,
            row_height: None,
            row_padding_x: None,
            row_padding_y: None,
            row_hover_bg: None,
            row_font_family: None,
            font_family: None,
            radius: None,
            field: util::FieldBox::default(),
            validate: None,
            validation_behavior: None,
            allows_empty_collection: false,
            name: None,
            form_value: None,
            should_focus_wrap: false,
            sections: Vec::new(),
            indicator: None,
            disabled_keys: std::collections::HashSet::new(),
            selection_mode: SelectionMode::Single,
            selected_keys: Vec::new(),
            is_controlled: false,
            selected_key_sync: false,
            value_content: None,
            default_value: None,
            default_input_value: None,
            value: None,
            on_selection_change_all: None,
            on_input_change: None,
            on_selection_change: None,
            on_open_change: None,
            form_state,
            sx: None,
        }
    }

    /// `placement` on `ComboBox.Popover`.
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Sets the controlled open state (v3 `isOpen`).
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

    /// Sets the field label.
    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// Sets the input placeholder text.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Sets the description shown beneath the field.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Sets the error message shown when the field is invalid.
    pub fn error_message(mut self, text: impl Into<SharedString>) -> Self {
        self.error_message = Some(text.into());
        self
    }

    /// Sets the field's visual variant.
    pub fn variant(mut self, variant: FieldVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets when the list opens (v3 `menuTrigger`); the default is on focus.
    pub fn menu_trigger(mut self, trigger: MenuTrigger) -> Self {
        self.menu_trigger = trigger;
        self
    }

    /// `defaultFilter` — replaces the default case-insensitive substring
    /// match.
    ///
    /// Called as `filter(item_label, input)`: v3 filters on the item's
    /// `textValue`, never on its key, and owns the whole decision —
    /// including what an empty query means.
    pub fn filter(mut self, f: impl Fn(&str, &str) -> bool + 'static) -> Self {
        self.filter = Some(Arc::new(f));
        self
    }

    /// `allowsCustomValue` — accept text that matches no item. A committed
    /// custom value keeps the typed text and carries a null selected key.
    pub fn allows_custom_value(mut self, v: bool) -> Self {
        self.allows_custom_value = v;
        self
    }

    /// Caps the number of items shown in the list (minimum 1; default 8).
    pub fn max_items(mut self, n: usize) -> Self {
        self.max_items = n.max(1);
        self
    }

    /// Makes the combo box fill its container's width.
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self
    }

    /// Replaces the 180px width floor the combo box root keeps while it is not
    /// [`ComboBox::full_width`], so a compact toolbar filter can go narrower
    /// or a wide one keep a larger floor. A full-width combo box has no floor
    /// either way, as before. Not a v3 prop; v3 sizes the root with a class.
    pub fn min_width(mut self, width: impl Into<Pixels>) -> Self {
        self.min_width = Some(width.into());
        self
    }

    /// Replaces the trigger's 36px box height (forwarded to the inner field).
    pub fn height(mut self, h: impl Into<Pixels>) -> Self {
        self.field.height = Some(h.into());
        self
    }

    /// Replaces the trigger's `px-3` horizontal padding (forwarded to the
    /// inner field).
    pub fn padding_x(mut self, p: impl Into<Pixels>) -> Self {
        self.field.padding_x = Some(p.into());
        self
    }

    /// Renders the trigger with no background, border, field shadow or focus
    /// ring, for a caller painting around it. The list still opens.
    pub fn is_bare(mut self, v: bool) -> Self {
        self.field.is_bare = v;
        self.field.is_bare_is_set = true;
        self
    }

    /// Shows or hides only the editable trigger's visual focus ring. The
    /// trigger stays focusable and the list still opens when set to `false`.
    pub fn focus_ring(mut self, v: bool) -> Self {
        self.field.focus_ring = Some(v);
        self
    }

    /// Replaces the list rows' `px-2` horizontal padding.
    pub fn row_padding_x(mut self, p: impl Into<Pixels>) -> Self {
        self.row_padding_x = Some(p.into());
        self
    }

    /// Replaces the list rows' `py-1.5` vertical padding.
    pub fn row_padding_y(mut self, p: impl Into<Pixels>) -> Self {
        self.row_padding_y = Some(p.into());
        self
    }

    /// The fill a hovered row takes, in place of `--default`.
    pub fn row_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.row_hover_bg = Some(color.into());
        self
    }

    /// The trigger field's font family, forwarded to the inner
    /// [`crate::Input`] so the query, placeholder and caret measurement all
    /// use it (see [`crate::Input::font_family`]), and set on the root so the
    /// `ComboBox.Value` line inherits it; unset keeps the inherited family.
    /// The detached rows take [`ComboBox::row_font_family`] instead. Not a v3
    /// prop; v3 sets it with a class.
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
    /// The trigger is an inner [`crate::Input`] whose box the shared field
    /// chrome paints — `--field-radius`, not this value.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the combo box's root element after every value the variant
    /// and the active theme chose, so they win. The field paints its own
    /// chrome, so this reaches the box that chrome sits in, not the chrome.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }

    /// Disables the combo box (v3 `isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Marks the field invalid (v3 `isInvalid`).
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// Marks the field required (v3 `isRequired`).
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// Pick-only single-key convenience callback.
    ///
    /// Use [`Self::on_selection_change_all`] for v3's complete
    /// `onChange` domain, including multiple selection and the `null` a
    /// cleared input or a committed custom value reports.
    pub fn on_selection_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Arc::new(handler));
        self
    }

    /// Called with the new open state when the list opens or closes (v3 `onOpenChange`).
    pub fn on_open_change(
        mut self,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Arc::new(handler));
        self
    }
}

type ComboAction = Arc<dyn Fn(&mut Window, &mut App)>;
type OnSelectionChangeAll = Arc<dyn Fn(&[SharedString], &mut Window, &mut App) + 'static>;
type OnInputChange = Arc<dyn Fn(&str, &mut Window, &mut App) + 'static>;
type ComboFilter = Arc<dyn Fn(&str, &str) -> bool + 'static>;

/// The controlled halves `render` resolves first: the frame's base id and
/// shared collection, the selection and the open flag. `controlled` takes
/// `cx` mutably, so these precede every other read.
struct ComboControlled {
    entity_id: u64,
    base_id: gpui::ElementId,
    items: Rc<[PickerItem]>,
    multiple: bool,
    default_selection: Vec<SharedString>,
    selection_own: Option<Entity<Vec<SharedString>>>,
    open_state: bool,
    open_own: Option<Entity<bool>>,
    overlay_phase: util::OverlayPhase,
    dismissal_token: util::OverlayToken,
}

/// Everything one ComboBox frame shares between its painted parts: the
/// controlled state, the resolved matches and cursor, the keyed handles and
/// the shared close/commit actions. `render` resolves it once in
/// [`ComboBox::frame`]; the chevron, the input, the field row, the key
/// handler and the popover list all read this one copy.
struct ComboFrame {
    entity_id: u64,
    base_id: gpui::ElementId,
    items: Rc<[PickerItem]>,
    multiple: bool,
    selection_own: Option<Entity<Vec<SharedString>>>,
    open_state: bool,
    open_own: Option<Entity<bool>>,
    overlay_phase: util::OverlayPhase,
    dismissal_token: util::OverlayToken,
    resolved_placement: Rc<Cell<Option<Placement>>>,
    entry_placement: Placement,
    anchor_bounds: Rc<Cell<Option<gpui::Bounds<Pixels>>>>,
    colors: herogpui_theme::ThemeColors,
    layout: herogpui_theme::LayoutTheme,
    container_radius: Pixels,
    close_open: ComboAction,
    raw_query: String,
    is_invalid: bool,
    focus_handle: gpui::FocusHandle,
    show_all_items: Entity<bool>,
    display_full_collection: bool,
    core: crate::list_core::ListCore,
    retained: Entity<Option<(SharedString, String)>>,
    matches: Rc<[PickerItem]>,
    focus_open: Option<Entity<FocusOpen>>,
    open_focus: Entity<Option<crate::list_core::OpenFocus>>,
    inside_pressed: Rc<Cell<bool>>,
    cursor_at: Option<usize>,
    list_scroll_now: crate::VirtualListHandle,
    panel_scroll_now: gpui::ScrollHandle,
    commit_value: ComboAction,
    blur_scope: util::FocusLeave,
    blur_focus: gpui::FocusHandle,
}

impl RenderOnce for ComboBox {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // One structured base id for the whole frame: every part below hangs
        // off it, so a hyphenated part name can never fold into a sibling's
        // key. Its `Display` is `combobox-<entity id>`, the spelling the parts
        // used to build with `format!`.
        let entity_id = self.state.entity_id().as_u64();
        let base_id = gpui::ElementId::named_usize("combobox", entity_id as usize);
        // One shared collection for the frame: the `'static` panel and event
        // closures clone the `Rc`, never the rows.
        let items: Rc<[PickerItem]> = std::mem::take(&mut self.items).into();

        // `value` / `defaultInputValue` seed the text once, before anything
        // reads it. `value` is v3's controlled spelling, so it outranks the
        // uncontrolled seed; the state owns the text afterwards, and
        // `InputState::set_value` is the imperative update.
        let seed = self.value.clone().or(self.default_input_value.clone());
        if let Some(text) = seed {
            let state = self.state.clone();
            util::seed_once(
                window,
                cx,
                element_id::scoped(&base_id, "default-text"),
                move |cx| {
                    state.update(cx, |s, cx| {
                        s.set_value(text.to_string());
                        cx.notify();
                    });
                },
            );
        }

        // `defaultValue` opts into the component holding its own selection;
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        // Both spellings normalize to the `Set` shape first: duplicates
        // collapse to their first insertion, and single mode keeps one key.
        // A controlled order is the owner's order — nothing is sorted.
        let multiple = self.selection_mode == SelectionMode::Multiple;
        self.selected_keys = normalize_selection(self.selected_keys.clone(), multiple);
        let default_selection =
            normalize_selection(self.default_value.clone().unwrap_or_default(), multiple);
        let (selection, selection_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base_id, "selection"),
            self.is_controlled.then(|| self.selected_keys.clone()),
            default_selection.clone(),
        );
        self.selected_keys = selection;

        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (open_state, open_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base_id, "open"),
            self.is_open,
            self.default_open,
        );
        let (overlay_phase, dismissal_token) = util::overlay_scope(
            window,
            cx,
            element_id::scoped(&base_id, "overlay"),
            open_state,
            true,
        );
        let overlay_active = overlay_phase != util::OverlayPhase::Closed;
        let frame = self.frame(
            ComboControlled {
                entity_id,
                base_id,
                items,
                multiple,
                default_selection,
                selection_own,
                open_state,
                open_own,
                overlay_phase,
                dismissal_token,
            },
            window,
            cx,
        );
        let trigger = self.trigger(&frame, window, cx);
        let mut input = self.input(&frame, trigger);
        let ComboFrame {
            entity_id,
            ref base_id,
            ref anchor_bounds,
            ref inside_pressed,
            ..
        } = frame;
        // `.combo-box__input-group` is the field itself (`relative
        // inline-flex items-center`). The popup anchors to the Input's 36px
        // field row — not to the label-to-value wrapper root — the way RAC's
        // `triggerRef` reads `groupRef.current || inputRef.current` (pinned
        // 1.20.0 `dist/private/ComboBox.js`). `.combo-box__value` sits below
        // the field inside the root, so a value row that appears under it
        // must not push the popover down. `scrollable_field_popover` below
        // reads these bounds to flip and cap the panel; the measure element
        // inside `Input` only records them.
        let inside_pressed_for_group = inside_pressed.clone();
        let field_selector = format!("combobox-field-{entity_id}");
        input = input.field_anchor(anchor_bounds.clone(), field_selector);
        let input_group = div()
            .id(element_id::scoped(base_id, "field"))
            // `combo-box/combo-box.js`'s `ComboBox.InputGroup` renders RAC
            // `Group`, and `react-aria-components/dist/private/Group.mjs` is
            // `role: props.role ?? 'group'`.
            //
            // The `role="combobox"` that `useComboBox` puts on the *input*
            // does not appear anywhere in this port: the field is a
            // `crate::input::Input`, whose role is decided inside its own
            // render from its `InputType` and which has no override prop.
            // See the picker note in `crate::a11y`.
            .a11y(a11y::Role::Group)
            .relative()
            .capture_any_mouse_down(move |_, _, cx| {
                inside_pressed_for_group.set(true);
                let pressed = inside_pressed_for_group.clone();
                cx.defer(move |_| pressed.set(false));
            })
            .child(input.render(window, cx));
        let mut root = div()
            .when(!self.full_width, |e| {
                e.min_w(self.min_width.unwrap_or(TRIGGER_MIN_WIDTH))
            })
            .relative()
            .flex()
            .flex_col()
            .gap(px(4.))
            // The value line and label inherit the family; the field takes it
            // explicitly above for its caret measurement.
            .when_some(self.font_family.clone(), |e, family| e.font_family(family));
        let value_content = self.value_row(&frame);

        // `allowsEmptyCollection` keeps the panel up with no matches. Without
        // it an empty result closes the list; `allowsCustomValue` only changes
        // what Enter commits while the list is closed.
        // Up, down, Home, End and Enter walk the suggestions; the inner input
        // keeps left and right for the caret.
        if !self.is_disabled && !self.is_read_only {
            root = self.root_keys(root, &frame);
        }

        let show_list = overlay_active
            && !self.is_disabled
            && (!frame.matches.is_empty() || self.allows_empty_collection);
        root = self.root_dismissals(root, &frame, overlay_active && !show_list);
        // The popup anchors to the Input's field-row bounds — not to the
        // label-to-value wrapper root — the way RAC's `triggerRef` reads
        // `groupRef.current || inputRef.current`. The field joins ahead of
        // the panel below so the positioner reads settled trigger bounds.
        root = root.child(input_group);
        let blur_focus = frame.blur_focus.clone();
        if show_list {
            root = root.child(self.popover(frame, cx));
        }

        // The popover hangs off the field it belongs to, so the field — not
        // the panel — is the root's first child; the deferred panel still
        // paints over the value row below it.
        root = util::apply_sx(root, &self.sx);
        root.when_some(value_content, |root, value| root.child(value))
            .track_focus(&blur_focus)
    }
}

impl ComboBox {
    /// Resolves the frame's keyed handles, matches, cursor and shared actions
    /// on top of the controlled state.
    fn frame(&self, controlled: ComboControlled, window: &mut Window, cx: &mut App) -> ComboFrame {
        let ComboControlled {
            ref base_id,
            ref items,
            open_state,
            ref open_own,
            ..
        } = controlled;
        // The field positioner discovers flips during prepaint. Keep its
        // resolved physical side keyed to this ComboBox so entry motion starts
        // from the side that is actually painted.
        let (resolved_placement, entry_placement) =
            crate::popover::field_placement_feedback(window, cx, base_id, self.placement);

        // Owned copies: `input.render` below needs `cx` mutably.
        let anchor_bounds: Rc<Cell<Option<gpui::Bounds<Pixels>>>> = window
            .use_keyed_state(element_id::scoped(base_id, "anchor-bounds"), cx, |_, _| {
                Rc::new(Cell::new(None))
            })
            .read(cx)
            .clone();
        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        // One binding feeds every panel surface below: the painted panel and
        // the entry zoom that animates it. The trigger's own box is the held
        // field's, and the shared field chrome paints that one.
        let container_radius = self.radius.unwrap_or_else(|| util::picker_radius(cx));
        let close_open = util::shared({
            let own = open_own.clone();
            let callback = self.on_open_change.clone();
            move |window: &mut Window, cx: &mut App| {
                let is_open = own.as_ref().map_or(open_state, |held| *held.read(cx));
                if !is_open {
                    return;
                }
                if let Some(held) = &own {
                    held.update(cx, |value, cx| {
                        *value = false;
                        cx.notify();
                    });
                }
                if let Some(callback) = &callback {
                    callback(&false, window, cx);
                }
            }
        });
        let raw_query = self.state.read(cx).value().to_owned();
        let is_invalid = self.is_invalid || self.error_message.is_some();
        let focus_handle = self.state.read(cx).focus_handle.clone();

        self.sync_form(&controlled, &raw_query, &focus_handle, window, cx);

        let show_all_items =
            window.use_keyed_state(element_id::scoped(base_id, "show-all-items"), cx, |_, _| {
                false
            });
        let last_query =
            window.use_keyed_state(element_id::scoped(base_id, "last-query"), cx, |_, _| {
                raw_query.clone()
            });
        if *last_query.read(cx) != raw_query {
            last_query.update(cx, |value, _| value.clone_from(&raw_query));
            show_all_items.update(cx, |value, _| *value = false);
        }
        let display_full_collection =
            *show_all_items.read(cx) || (self.menu_trigger == MenuTrigger::Manual && !open_state);

        // Which suggestion the keyboard is on, held as the item's *key* so the
        // cursor stays on the same item when the query filters or the caller
        // reorders the collection. Input edits clear it, matching React
        // Stately's focused-key reset before the filtered list changes.
        // Created ahead of the matches: the idle gate below reads it.
        let cursor = crate::list_core::ListCore::cursor_state(base_id, window, cx);
        // A cursor picked in `multiple` mode survives the query reset its pick
        // performs, but only for that one edit: the pair `(key, query)` says
        // "this key was retained while the query read `query`". Any other
        // cursor write clears it, so a stale pair cannot resurrect a cursor
        // the user has since moved.
        let retained = window.use_keyed_state(
            element_id::scoped(base_id, "cursor-retained"),
            cx,
            |_, _| None::<(SharedString, String)>,
        );
        // The previous frame's row keys, which the removed-row repair walks.
        let previous_rows = crate::list_core::ListCore::rows_state(base_id, window, cx);
        // The open strategy the trigger left for this open -- ArrowDown
        // `First`, ArrowUp `Last`, a button press or typed query `None` --
        // consumed by `seat_on_open` on the first open render.
        let open_focus =
            window.use_keyed_state(element_id::scoped(base_id, "open-focus"), cx, |_, _| {
                None::<crate::list_core::OpenFocus>
            });

        let matches = self.matches(
            &controlled,
            &raw_query,
            display_full_collection,
            &cursor,
            window,
            cx,
        );
        let focus_open = self.focus_open(&controlled, &show_all_items, window, cx);

        // Whether the pointer went down inside the input-plus-panel subtree.
        // The panel's outside-press listener reads only the panel bounds, so
        // the input is geometrically outside it even though both belong to the
        // same ComboBox. Capture the whole subtree for one dispatch: input
        // presses keep the list open, and the chevron or a row owns its click.
        let inside_pressed = Rc::new(Cell::new(false));

        // A cursor is stale when its key is disabled, or when it is neither a
        // visible match nor retained across the current query by the
        // `(key, query)` pair a multiple-mode pick left behind. A key the
        // caller removed from `items` is not stale: it is left for
        // `repair_removed` to walk to a neighbour, the way Select does.
        let stale_cursor = cursor.read(cx).as_ref().is_some_and(|key| {
            let visible = matches.iter().any(|item| item.key() == key);
            let retained_hidden = retained
                .read(cx)
                .as_ref()
                .is_some_and(|(retained_key, query)| retained_key == key && query == &raw_query)
                && items.iter().any(|item| item.key() == key);
            self.disabled_keys.contains(key)
                || (!visible && !retained_hidden && items.iter().any(|item| item.key() == key))
        });
        // React Aria keeps the focused row in view; the panel scrolls and the
        // virtual list scrolls itself. `use_keyed_state` takes `cx` mutably.
        let list_scroll =
            window.use_keyed_state(element_id::scoped(base_id, "list-scroll"), cx, |_, _| {
                crate::VirtualListHandle::uniform(0)
            });
        let panel_scroll =
            window.use_keyed_state(element_id::scoped(base_id, "panel-scroll"), cx, |_, _| {
                gpui::ScrollHandle::new()
            });
        let list_scroll_now = list_scroll.read(cx).clone();
        let panel_scroll_now = panel_scroll.read(cx).clone();
        // The list's shared navigation core owns the keyed cursor, the move
        // resolution, and the reveal scroll. ComboBox keeps its own gates in
        // front of it -- the page keys are gated on the list being open, Tab
        // maps to Enter, and a custom value commits on Enter -- but every
        // cursor read and write, every move, and every reveal routes through
        // here. Its text field owns the query,
        // so the list carries no typeahead buffer; its page keys take the
        // enabled ends, matching the non-scrolling suggestion list.
        let list_core = crate::list_core::ListCore::new(
            cursor.clone(),
            None,
            self.should_focus_wrap,
            crate::list_core::PageKeys::Ends,
            if self.row_height.is_some() {
                crate::list_core::ListScroll::Virtual(list_scroll_now.clone())
            } else {
                crate::list_core::ListScroll::Panel(panel_scroll_now.clone())
            },
        );
        // A stale cursor clears through the core, ahead of the resolution
        // below that reads it.
        if stale_cursor {
            list_core.set_cursor(None, cx);
            retained.update(cx, |value, _| *value = None);
        }
        // The row the cursor's key lands on in the rendered matches, resolved
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
        // An open the trigger armed seats the cursor through the shared core:
        // the selected key wins when it is a stop in the filtered rows, else
        // the trigger's First/Last/None strategy applies. Consumed once.
        if open_state {
            if let Some(strategy) = *open_focus.read(cx) {
                list_core.seat_on_open(&match_rows, &self.selected_keys, strategy, cx);
                open_focus.update(cx, |value, _| *value = None);
            }
        }
        let cursor_at = list_core.cursor_at(&match_rows, cx);

        let commit_value = self.commit_action(&controlled);
        let blur_close = close_open.clone();
        let blur_commit = commit_value.clone();
        let blur_scope =
            util::on_focus_leave(window, cx, base_id, !self.is_disabled, move |window, cx| {
                blur_commit(window, cx);
                blur_close(window, cx);
            });
        let blur_focus = blur_scope.focus_handle();

        let ComboControlled {
            entity_id,
            base_id,
            items,
            multiple,
            selection_own,
            open_state,
            open_own,
            overlay_phase,
            dismissal_token,
            ..
        } = controlled;
        ComboFrame {
            entity_id,
            base_id,
            items,
            multiple,
            selection_own,
            open_state,
            open_own,
            overlay_phase,
            dismissal_token,
            resolved_placement,
            entry_placement,
            anchor_bounds,
            colors,
            layout,
            container_radius,
            close_open,
            raw_query,
            is_invalid,
            focus_handle,
            show_all_items,
            display_full_collection,
            core: list_core,
            retained,
            matches,
            focus_open,
            open_focus,
            inside_pressed,
            cursor_at,
            list_scroll_now,
            panel_scroll_now,
            commit_value,
            blur_scope,
            blur_focus,
        }
    }

    /// Mirrors the value into the live form state, applies a controlled
    /// `selected_key`, and installs the reset that restores the default.
    fn sync_form(
        &self,
        controlled: &ComboControlled,
        raw_query: &str,
        focus_handle: &gpui::FocusHandle,
        window: &mut Window,
        cx: &mut App,
    ) {
        let ComboControlled {
            ref base_id,
            ref items,
            ref default_selection,
            ref selection_own,
            ..
        } = *controlled;
        // The live form value follows the pinned `formValue` serialization:
        // the selected key(s) by default, the typed text when
        // `allowsCustomValue` forces text mode. Success and validity are the
        // inner input's own mirrors (`live_text` reads them there), so only
        // the value and the focus handle — the handle a blocked submit must
        // reach — live in this shared state.
        let form_text = form_submits_text(self.form_value, self.allows_custom_value);
        {
            let mut form = self.form_state.borrow_mut();
            form.value = if form_text {
                crate::form::FormValue::Text(SharedString::from(raw_query))
            } else {
                crate::form::FormValue::Keys(self.selected_keys.clone())
            };
            form.focus = Some(focus_handle.clone());
        }

        // `selected_key`'s controlled sync: the owner hands the same key to
        // the builder every render, so the label may move into the input only
        // when that key actually changes — writing it every frame would
        // clobber the text being typed. The empty string is v3's `null` and
        // clears the input; a key with no item resolves to no label.
        if self.selected_key_sync {
            let applied_key =
                window.use_keyed_state(element_id::scoped(base_id, "applied-key"), cx, |_, _| {
                    None::<SharedString>
                });
            let owned_key = self.selected_keys.first().cloned().unwrap_or_default();
            let first_apply = applied_key.read(cx).is_none();
            if applied_key.read(cx).clone() != Some(owned_key.clone()) {
                // Pinned `getDefaultInputValue` derives the text from the
                // selected key only when no `defaultInputValue` was given, so
                // the first application must leave the seeded text in place —
                // `value` seeds the same slot and is owed the same leave-alone;
                // later key changes still move their labels in.
                if !(first_apply && (self.default_input_value.is_some() || self.value.is_some())) {
                    let label = label_of_key(items, &owned_key).cloned().unwrap_or_default();
                    self.state.update(cx, |state, cx| {
                        state.set_value(label.to_string());
                        cx.notify();
                    });
                }
                applied_key.update(cx, |v, _| *v = Some(owned_key));
            }
        }

        // A reset restores the default selection and the label it resolves
        // to, the way pinned react-stately resets `selectedKey` and lets
        // `resetInputValue` re-derive the text — reporting the restored text
        // through `onInputChange` when it actually changed, as the pinned
        // controlled input state does.
        let restore_own = selection_own.clone();
        let restore_state = Rc::downgrade(&self.form_state);
        let restore_input = self.state.clone();
        let restore_items = items.clone();
        let restore_default = default_selection.clone();
        let restore_input_change = self.on_input_change.clone();
        let restore_all = self.on_selection_change_all.clone();
        self.form_state.borrow_mut().restore = (restore_own.is_some() || restore_all.is_some())
            .then(|| {
                util::shared(move |window: &mut Window, cx: &mut App| {
                    let default_text = restore_default
                        .first()
                        .and_then(|key| label_of_key(&restore_items, key))
                        .cloned()
                        .unwrap_or_default();
                    if let Some(state) = restore_state.upgrade() {
                        state.borrow_mut().value =
                            crate::form::FormValue::Keys(restore_default.clone());
                    }
                    let input_changed = restore_input.read(cx).value() != default_text.as_ref();
                    restore_input.update(cx, |state, cx| {
                        state.set_value(default_text.to_string());
                        cx.notify();
                    });
                    if input_changed {
                        if let Some(cb) = &restore_input_change {
                            cb(&default_text, window, cx);
                        }
                    }
                    if let Some(held) = &restore_own {
                        let keys = restore_default.clone();
                        held.update(cx, |v, cx| {
                            *v = keys;
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &restore_all {
                        cb(&restore_default, window, cx);
                    }
                }) as Arc<dyn Fn(&mut Window, &mut App)>
            });
    }

    /// The rows the list draws this frame: cached, retained for the exit
    /// frame, or skipped entirely while nothing consumes them.
    fn matches(
        &self,
        controlled: &ComboControlled,
        raw_query: &str,
        display_full_collection: bool,
        cursor: &Entity<Option<SharedString>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Rc<[PickerItem]> {
        let ComboControlled {
            ref base_id,
            ref items,
            overlay_phase,
            ..
        } = *controlled;
        let overlay_active = overlay_phase != util::OverlayPhase::Closed;
        // Closed and idle frames draw no rows, so they skip the match work
        // entirely; a consuming filtered frame shares one cached list until
        // the query, collection or cap changes. Full-collection and
        // empty-query frames copy their capped prefix directly: no per-row
        // matching runs for them, so the cache's element-by-element key
        // comparison would cost more than the work it saves. A custom
        // `defaultFilter` owns the whole filtering decision, including an
        // empty query, and its configuration cannot join a cache key — its
        // results are never cached and it only runs while the matches are
        // consumed.
        let matches_cache =
            window.use_keyed_state(element_id::scoped(base_id, "matches"), cx, |_, _| {
                MatchesCache::default()
            });
        // Keep the last open collection for the retained exit frame. Closing
        // a row can update the input text in the same dispatch; React Aria
        // keeps the old rows under its fade and does not re-run a custom
        // filter for that committed label.
        let retained_matches = window.use_keyed_state(
            element_id::scoped(base_id, "retained-matches"),
            cx,
            |_, _| empty_matches(),
        );
        let consume_matches = overlay_active
            || cursor.read(cx).is_some()
            || (self.allows_custom_value && !raw_query.is_empty());
        let matches: Rc<[PickerItem]> = if overlay_phase == util::OverlayPhase::Exiting {
            retained_matches.read(cx).clone()
        } else if !consume_matches {
            empty_matches()
        } else {
            let filter = self.filter.clone();
            match &filter {
                Some(f) => Rc::from(compute_matches(
                    items,
                    raw_query,
                    self.max_items,
                    display_full_collection,
                    Some(f),
                )),
                None if display_full_collection => Rc::from(compute_matches(
                    items,
                    raw_query,
                    self.max_items,
                    true,
                    None,
                )),
                None if raw_query.is_empty() => Rc::from(compute_matches(
                    items,
                    raw_query,
                    self.max_items,
                    false,
                    None,
                )),
                None => matches_cache.update(cx, |cache, _| {
                    cache.get(items.clone(), raw_query, self.max_items, |items| {
                        compute_matches(items, raw_query, self.max_items, false, None)
                    })
                }),
            }
        };
        if overlay_phase == util::OverlayPhase::Open {
            retained_matches.update(cx, |value, _| *value = matches.clone());
        }
        matches
    }

    /// `MenuTrigger::Focus`'s one-shot open on the field taking focus.
    fn focus_open(
        &self,
        controlled: &ComboControlled,
        show_all_items: &Entity<bool>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<FocusOpen>> {
        let ComboControlled {
            ref base_id,
            ref items,
            open_state,
            ref open_own,
            ..
        } = *controlled;
        // `MenuTrigger::Focus` opens the list when the field takes focus. The
        // check reads the focus handle every frame; the keyed state is the
        // one-shot that stops the panel from reopening behind a dismissal
        // (Escape, a pick, the chevron, a press outside) while the field keeps
        // the focus -- the frame after closing would otherwise re-answer the
        // still-held focus. The focus leaving resets it, so the *next* focus
        // session opens again.
        let focus_open =
            if self.menu_trigger == MenuTrigger::Focus && !self.is_disabled && !self.is_read_only {
                Some(window.use_keyed_state(
                    element_id::scoped(base_id, "focus-open"),
                    cx,
                    |_, _| FocusOpen {
                        can_open: true,
                        was_open: false,
                    },
                ))
            } else {
                None
            };
        if let Some(focus_open) = &focus_open {
            let focused = self.state.read(cx).focus_handle.is_focused(window);
            let mut held = *focus_open.read(cx);
            let mut now_open = open_state;
            if focused && !open_state && held.was_open {
                // The list closed while the field still has the focus: the
                // user dismissed it, so it must not come back.
                held.can_open = false;
            } else if focused && !open_state && held.can_open {
                // A fresh focus session: the field taking focus is the
                // gesture, when there is something to show -- the panel's own
                // gate of a non-empty result or an allowed empty state.
                let can_show = !items.is_empty() || self.allows_empty_collection;
                if can_show {
                    show_all_items.update(cx, |v, _| *v = true);
                    // Opening from focus writes both halves, the way the
                    // chevron's handler does: the uncontrolled flag, and the
                    // report to a controlled caller.
                    if let Some(open) = &open_own {
                        open.update(cx, |v, cx| {
                            *v = true;
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &self.on_open_change {
                        cb(&true, window, cx);
                    }
                    held.can_open = false;
                    now_open = true;
                }
            }
            if !focused {
                // The focus left; the next focus session may open again.
                held.can_open = true;
            }
            held.was_open = now_open;
            focus_open.update(cx, |v, _| *v = held);
        }
        focus_open
    }

    /// The commit a focus loss or an outside press runs.
    fn commit_action(&self, controlled: &ComboControlled) -> ComboAction {
        let ComboControlled {
            ref items,
            multiple,
            ref selection_own,
            ..
        } = *controlled;
        // ComboBox commits its text/selection when focus leaves even while the
        // list is closed. This is `useComboBoxState.setFocused(false)`, not the
        // generic popover close used by Select-family controls.
        util::shared({
            let state = self.state.clone();
            let selection_own = selection_own.clone();
            let selected = self.selected_keys.clone();
            let items = items.clone();
            let selection_change = self.on_selection_change_all.clone();
            let input_change = self.on_input_change.clone();
            let allows_custom = self.allows_custom_value;
            move |window: &mut Window, cx: &mut App| {
                let current = state.read(cx).value().to_owned();
                let selected_text = selected
                    .first()
                    .and_then(|key| label_of_key(&items, key))
                    .cloned()
                    .unwrap_or_default();

                if allows_custom {
                    // Pinned react-stately's commit path: text that no longer
                    // matches the selected item's label is a custom value,
                    // which carries a null selected key.
                    if !multiple && current != selected_text && !selected.is_empty() {
                        if let Some(held) = &selection_own {
                            held.update(cx, |value, cx| {
                                value.clear();
                                cx.notify();
                            });
                        }
                        if let Some(callback) = &selection_change {
                            callback(&[], window, cx);
                        }
                    }
                    return;
                }

                let committed = if multiple {
                    String::new()
                } else {
                    selected_text.to_string()
                };
                if current != committed {
                    state.update(cx, |state, cx| {
                        state.set_value(committed.clone());
                        cx.notify();
                    });
                    if let Some(callback) = &input_change {
                        callback(&committed, window, cx);
                    }
                }
            }
        })
    }

    /// The `.combo-box__trigger` chevron button.
    fn trigger(
        &self,
        frame: &ComboFrame,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let ComboFrame {
            ref base_id,
            open_state,
            ref open_own,
            ref colors,
            ref close_open,
            ref focus_handle,
            ref show_all_items,
            ref focus_open,
            ref open_focus,
            ..
        } = *frame;
        let on_open_change = self.on_open_change.clone();
        let is_open = open_state;
        // `.combo-box__trigger` is `border-none bg-transparent
        // text-field-placeholder`, and its hover recolours the text to
        // `text-field-foreground`, filling nothing. `svg()` paints from its own
        // style rather than the inherited text color, so the hover refinement
        // goes on the glyph as well as the trigger that wraps it.
        let trigger_hover_fg = colors.field.foreground;
        let trigger_indicator = crate::anim::rotating_indicator_with_duration(
            &element_id::scoped(base_id, "trigger-indicator"),
            is_open,
            gpui::svg()
                .size(util::FIELD_ICON)
                .path(icons::CHEVRON_DOWN)
                .text_color(colors.muted)
                .when(!self.is_disabled && !self.is_read_only, |glyph| {
                    glyph.hover(move |st| st.text_color(trigger_hover_fg))
                }),
            150,
            window,
            cx,
        );
        let mut trigger = div()
            .id(element_id::scoped(base_id, "trigger"))
            // `combo-box/combo-box.js` composes an RAC `Button` here, whose
            // props come from `useComboBox`'s `buttonProps` — i.e.
            // `useMenuTrigger({type: 'listbox'})`, so
            // `.../overlays/useOverlayTrigger.mjs` gives it
            // `'aria-haspopup': 'listbox'`, `'aria-expanded': isOpen` and
            // `'aria-controls'`. Only the expansion flag ports; the other two
            // are recorded omissions in `crate::a11y`. RAC names the button
            // `buttonLabel` ("Show suggestions"); the chevron is the only
            // child and carries no text of its own.
            .a11y_named(
                a11y::Role::Button,
                &a11y::Name::labelled(crate::i18n::ui_string(
                    crate::i18n::UiString::ShowSuggestions,
                    cx,
                )),
            )
            .a11y_expanded(is_open)
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .size(px(20.))
            .rounded(px(6.))
            .text_color(colors.muted)
            .child(trigger_indicator);
        if !self.is_disabled && !self.is_read_only {
            trigger = trigger
                .cursor(util::interactive_cursor(cx))
                .hover(move |s| s.text_color(trigger_hover_fg));
            if on_open_change.is_some() || open_own.is_some() {
                let own = open_own.clone();
                let focus_open = focus_open.clone();
                let open_focus = open_focus.clone();
                let show_all_items = show_all_items.clone();
                let close = close_open.clone();
                let focus_handle = focus_handle.clone();
                trigger = trigger
                    // The chevron is a separate React Aria button. Focus the
                    // input on press start, but do not let its down bubble into
                    // the input row and also trigger the default focus-open path.
                    .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                        if let Some(focus_open) = &focus_open {
                            focus_open.update(cx, |v, _| v.can_open = false);
                        }
                        window.focus(&focus_handle, cx);
                        cx.stop_propagation();
                    })
                    .on_click(move |ev: &gpui::ClickEvent, window, cx| {
                        if is_open {
                            close(window, cx);
                            return;
                        }
                        // A keyboard press on the button opens `first`, a
                        // pointer press leaves the seat to a selected row.
                        open_focus.update(cx, |focus, _| {
                            *focus = Some(if matches!(ev, gpui::ClickEvent::Keyboard(_)) {
                                crate::list_core::OpenFocus::First
                            } else {
                                crate::list_core::OpenFocus::None
                            });
                        });
                        show_all_items.update(cx, |v, _| *v = true);
                        if let Some(held) = &own {
                            held.update(cx, |v, cx| {
                                *v = true;
                                cx.notify();
                            });
                        }
                        if let Some(cb) = &on_open_change {
                            cb(&true, window, cx);
                        }
                    });
            }
        }
        trigger
    }

    /// The held `Input` with the chevron in its end slot and the edit
    /// handler that opens, filters and closes the list.
    fn input(&self, frame: &ComboFrame, trigger: gpui::Stateful<gpui::Div>) -> Input {
        let ComboFrame {
            ref items,
            ref selection_own,
            open_state,
            ref open_own,
            ref close_open,
            ref raw_query,
            is_invalid,
            ref show_all_items,
            ref core,
            ref retained,
            ref focus_open,
            ref open_focus,
            ..
        } = *frame;
        let core_on_change = core.clone();
        let retained_on_change = retained.clone();
        let validate = self.validate.clone();
        let mut input = Input::new(self.state.clone())
            .variant(self.variant)
            .is_disabled(self.is_disabled)
            .is_invalid(is_invalid)
            .is_required(self.is_required)
            .is_read_only(self.is_read_only)
            .auto_focus(self.auto_focus)
            .when_some(self.font_family.clone(), |i, family| i.font_family(family))
            .when_some(self.validation_behavior, |i, b| i.validation_behavior(b))
            .when_some(validate, |i, f| i.validate(move |v| f(v)))
            .end_content(trigger);
        input = input.with_field_box(self.field);
        // Edits open Focus and Input triggers only when there is something to
        // show, and close an already-open filtered collection when it empties.
        // Manual stays closed until its trigger opens it, then edits switch it
        // from the full collection to filtered rows.
        let input_change = self.on_input_change.clone();
        let open_own_on_change = open_own.clone();
        let open_change_cb = self.on_open_change.clone();
        let focus_open_on_change = focus_open.clone();
        let open_focus_on_change = open_focus.clone();
        let show_all_items_on_change = show_all_items.clone();
        let items_on_change = items.clone();
        let filter_on_change = self.filter.clone();
        let menu_trigger_on_change = self.menu_trigger;
        let allows_empty_collection = self.allows_empty_collection;
        let was_open = open_state;
        let selection_own_on_change = selection_own.clone();
        let selected_on_change = self.selected_keys.clone();
        let selection_mode_on_change = self.selection_mode;
        let selection_change_on_input = self.on_selection_change_all.clone();
        let input_value_on_change = raw_query.clone();
        let close_on_empty = close_open.clone();
        input = input.on_change(move |text, window, cx| {
            if let Some(cb) = &input_change {
                cb(text, window, cx);
            }
            core_on_change.set_cursor(None, cx);
            retained_on_change.update(cx, |v, _| *v = None);

            if selection_mode_on_change == SelectionMode::Single
                && !input_value_on_change.is_empty()
                && text.is_empty()
                && !selected_on_change.is_empty()
            {
                if let Some(held) = &selection_own_on_change {
                    held.update(cx, |value, cx| {
                        value.clear();
                        cx.notify();
                    });
                }
                if let Some(cb) = &selection_change_on_input {
                    cb(&[], window, cx);
                }
            }

            let is_open = open_own_on_change
                .as_ref()
                .map_or(was_open, |held| *held.read(cx));

            // A row pick closes the uncontrolled popup before copying the
            // selected label into the input. React Aria batches those updates
            // and does not run the custom filter for the value that is being
            // committed into a now-closing field. Preserve that boundary so
            // the retained exit frame stays visual-only and the filter's
            // callback count does not change on a pick.
            let closing_after_pick = was_open && !is_open;
            let has_matches = if closing_after_pick {
                true
            } else {
                match &filter_on_change {
                    Some(f) => items_on_change.iter().any(|item| f(item.label(), text)),
                    None if text.is_empty() => !items_on_change.is_empty(),
                    None => {
                        let query = text.to_lowercase();
                        items_on_change
                            .iter()
                            .any(|item| item.label().to_lowercase().contains(&query))
                    }
                }
            };
            let can_show = has_matches || allows_empty_collection;

            if !is_open && menu_trigger_on_change != MenuTrigger::Manual && can_show {
                if let Some(focus_open) = &focus_open_on_change {
                    focus_open.update(cx, |v, _| v.can_open = false);
                }
                // A typed open carries no keyboard strategy: pinned React Aria
                // opens it with `open(null, 'input')`, so only a selected row
                // that is still a stop claims the cursor.
                open_focus_on_change.update(cx, |focus, _| {
                    *focus = Some(crate::list_core::OpenFocus::None);
                });
                if let Some(held) = &open_own_on_change {
                    held.update(cx, |v, cx| {
                        *v = true;
                        cx.notify();
                    });
                }
                if let Some(cb) = &open_change_cb {
                    cb(&true, window, cx);
                }
            } else if is_open && !can_show {
                close_on_empty(window, cx);
            }

            show_all_items_on_change.update(cx, |v, cx| {
                *v = false;
                cx.notify();
            });
            if menu_trigger_on_change == MenuTrigger::Manual {
                cx.refresh_windows();
            }
        });

        if self.full_width {
            input = input.full_width();
        }
        if let Some(label) = self.label.clone() {
            input = input.label(label);
        }
        if let Some(placeholder) = self.placeholder.clone() {
            input = input.placeholder(placeholder);
        }
        if is_invalid {
            if let Some(message) = self.error_message.clone() {
                input = input.error_message(message);
            }
        } else if let Some(description) = self.description.clone() {
            input = input.description(description);
        }
        input
    }

    /// `ComboBox.Value`, drawn below the field once something is chosen.
    fn value_row(&mut self, frame: &ComboFrame) -> Option<gpui::AnyElement> {
        let ComboFrame {
            entity_id,
            ref items,
            ref colors,
            ..
        } = *frame;
        // `ComboBox.Value` — `.combo-box__value` is `text-sm
        // text-field-foreground empty:hidden`, so it shows only once something
        // is chosen. The selection is read in its own order — pinned
        // react-stately 3.49.0's `selectedKeys` is a JS `Set`, which iterates
        // in insertion order — and each key resolves to its item wherever that
        // item now sits; a key with no item renders nothing.
        let mut value_content = None;
        if let Some(render) = self.value_content.take() {
            let mut labels: Vec<SharedString> = Vec::new();
            let mut indices: Vec<usize> = Vec::new();
            for key in &self.selected_keys {
                if let Some((index, item)) =
                    items.iter().enumerate().find(|(_, it)| it.key() == key)
                {
                    labels.push(item.label().clone());
                    indices.push(index);
                }
            }
            let text = labels
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let value_selector = format!("combobox-value-{entity_id}");
            let default_children = div()
                .debug_selector(move || value_selector)
                .w_full()
                .min_w_0()
                .whitespace_normal()
                .text_size(util::FIELD_TEXT)
                .line_height(px(20.))
                .text_color(colors.field.foreground)
                .child(text.clone())
                .into_any_element();
            value_content = Some(render(util::SelectionValue {
                selected_items: &labels,
                selected_indices: &indices,
                selected_keys: Some(&self.selected_keys),
                selected_text: &text,
                is_placeholder: labels.is_empty(),
                default_children,
            }));
        }
        value_content
    }

    /// The root's key handler: the list walk, Enter's commit or revert, and
    /// `allowsCustomValue`'s custom commit.
    fn root_keys(&self, root: gpui::Div, frame: &ComboFrame) -> gpui::Div {
        let ComboFrame {
            ref items,
            multiple,
            ref selection_own,
            open_state,
            ref open_own,
            ref close_open,
            ref raw_query,
            ref show_all_items,
            display_full_collection,
            ref core,
            ref retained,
            ref matches,
            ref open_focus,
            ref blur_scope,
            ..
        } = *frame;
        let key_rows: Rc<[PickerItem]> =
            if open_state || (self.allows_custom_value && !raw_query.is_empty()) {
                Rc::clone(matches)
            } else {
                items
                    .iter()
                    .take(self.max_items)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into()
            };
        let stops: Vec<usize> = (0..key_rows.len())
            .filter(|i| {
                key_rows
                    .get(*i)
                    .is_some_and(|item| !self.disabled_keys.contains(item.key()))
            })
            .collect();
        let key_core = core.clone();
        let key_retained = retained.clone();
        let key_open_focus = open_focus.clone();
        let rows = key_rows;
        let state = self.state.clone();
        let allows_custom_value = self.allows_custom_value;
        let was_open = open_state;
        let show_all_items = show_all_items.clone();
        let on_selection_change = self.on_selection_change.clone();
        let on_selection_change_all = self.on_selection_change_all.clone();
        let on_input_change = self.on_input_change.clone();
        let selected_now = self.selected_keys.clone();
        let key_query = raw_query.clone();
        let key_display_full = display_full_collection;
        let key_items = items.clone();
        let key_filter = self.filter.clone();
        let key_max_items = self.max_items;
        let key_disabled = self.disabled_keys.clone();
        let key_menu_trigger = self.menu_trigger;
        let open_own_keys = open_own.clone();
        let on_open_change = self.on_open_change.clone();
        let key_selection_own = selection_own.clone();
        let key_close = close_open.clone();
        let key_blur = blur_scope.clone();
        let key_multiple = multiple;
        let handler = ComboKeys {
            rows,
            stops,
            core: key_core,
            retained: key_retained,
            state,
            allows_custom_value,
            was_open,
            show_all_items,
            on_selection_change,
            on_selection_change_all,
            on_input_change,
            selected_now,
            key_query,
            key_display_full,
            key_items,
            key_filter,
            key_max_items,
            key_disabled,
            key_menu_trigger,
            open_own_keys,
            open_focus: key_open_focus,
            on_open_change,
            key_selection_own,
            key_close,
            key_blur,
            key_multiple,
        };
        root.on_key_down(move |event, window, cx| handler.on_key_down(event, window, cx))
    }

    /// Escape on the root, and the outside press when no list is drawn.
    fn root_dismissals(
        &self,
        mut root: gpui::Div,
        frame: &ComboFrame,
        dismiss_without_list: bool,
    ) -> gpui::Div {
        let ComboFrame {
            ref dismissal_token,
            ref close_open,
            ref commit_value,
            ref blur_scope,
            ..
        } = *frame;
        let escape_close = close_open.clone();
        root =
            util::dismiss_on_escape_with_token(root, dismissal_token.clone(), move |window, cx| {
                escape_close(window, cx);
                util::DismissResult::Handled
            });
        if dismiss_without_list {
            let dismiss_close = close_open.clone();
            let dismiss_commit = commit_value.clone();
            let dismiss_blur = blur_scope.clone();
            // ComboBox's popover is `isNonModal` in React Aria: no underlay,
            // so the outside press that closes the list also reaches the
            // control under the pointer.
            root = util::dismiss_on_press_outside_passthrough_with_token(
                root,
                dismissal_token.clone(),
                move |window, cx| {
                    dismiss_blur.consume(cx);
                    dismiss_commit(window, cx);
                    dismiss_close(window, cx);
                    util::DismissResult::Handled
                },
            );
        }
        root
    }

    /// The popover's scrolling `ListBox` surface: `bg-overlay`, the panel
    /// radius, the dark-mode hairline and the overlay shadow.
    fn panel_surface(&self, frame: &ComboFrame) -> gpui::Stateful<gpui::Div> {
        let ComboFrame {
            entity_id,
            ref base_id,
            overlay_phase,
            ref colors,
            ref layout,
            container_radius,
            ref panel_scroll_now,
            ..
        } = *frame;
        let panel_selector = format!("combobox-panel-{entity_id}");
        div()
                .id(element_id::scoped(base_id, "panel"))
                // `useComboBox` hands `listBoxProps` to the popover's RAC
                // `ListBox`, which is `useListBox`'s literal `role: 'listbox'`
                // with `'aria-orientation'` defaulting to vertical. The rows
                // are this panel's children, so the panel is that list.
                .a11y(a11y::Role::ListBox)
                .a11y_orientation(herogpui_core::Orientation::Vertical)
                .debug_selector(move || panel_selector)
                .w_full()
                .flex()
                .flex_col()
                .gap(px(2.))
                .p(px(4.))
                .overflow_y_scroll()
                // `overscroll-contain` upstream: a wheel over the popup must
                // not scroll the page behind it (Select/ColorPicker pattern).
                // A retained exit panel is visual-only and must not block the
                // next pointer target behind its old bounds.
                .when(overlay_phase == util::OverlayPhase::Open, |el| el.occlude())
                .track_scroll(panel_scroll_now)
                // RAC caps the popover at the available viewport height
                // (`calculatePosition`'s `getMaxHeight`); the positioner
                // below re-lays the panel out with that cap, so the panel
                // carries a viewport-relative bound rather than a fixed one.
                .max_h_full()
                .rounded(container_radius)
                .bg(colors.overlay.background)
                // v3 gives a floating panel no border: `.popover` and friends are
                // `bg-overlay shadow-overlay` and a radius, and dark mode's
                // inset hairline is what separates the panel from the page.
                .when_some(layout.overlay_hairline, |el, hairline| {
                el.border(layout.border_width).border_color(hairline)
                })
                .text_color(colors.overlay.foreground)
                .when(
                    !layout.overlay_shadow.is_empty(),
                    |e: gpui::Stateful<gpui::Div>| e.shadow(layout.overlay_shadow.clone()),
                )
    }

    /// The suggestion popover: surface, dismissal, empty state, rows and motion.
    fn popover(&mut self, frame: ComboFrame, cx: &mut App) -> gpui::Deferred {
        let panel = self.panel_surface(&frame);
        let ComboFrame {
            entity_id,
            base_id,
            multiple,
            selection_own,
            open_state,
            open_own,
            overlay_phase,
            dismissal_token,
            resolved_placement,
            entry_placement,
            anchor_bounds,
            colors,
            layout,
            container_radius,
            close_open,
            core,
            retained,
            matches,
            inside_pressed,
            cursor_at,
            list_scroll_now,
            commit_value,
            blur_scope,
            ..
        } = frame;
        // React Aria dismisses the list on a press outside it; Escape is
        // read in the field's own key handler above. A press that started
        // in the input-plus-panel subtree is not outside the ComboBox;
        // the input keeps the list open, while the chevron and rows own
        // their respective clicks.
        let dismiss_close = close_open.clone();
        let dismiss_commit = commit_value.clone();
        let dismiss_blur = blur_scope;
        let mut panel = util::dismiss_on_press_outside_passthrough_with_token(
            panel,
            dismissal_token,
            move |window, cx| {
                if inside_pressed.get() {
                    return util::DismissResult::Declined;
                }
                dismiss_blur.consume(cx);
                dismiss_commit(window, cx);
                dismiss_close(window, cx);
                util::DismissResult::Handled
            },
        );

        if matches.is_empty() {
            // `allowsCustomValue` means an unmatched query is still valid,
            // so the empty state has to say something different.
            let message = if self.allows_custom_value {
                "Press Enter to use this value"
            } else {
                "No matching options"
            };
            panel = panel.child(
                div()
                    .px(px(8.))
                    .py(px(6.))
                    .text_size(util::FIELD_TEXT)
                    .line_height(px(20.))
                    .text_color(colors.muted)
                    .child(message),
            );
        }

        // Everything a row reads, owned: the virtual list's row callback is
        // `'static` and runs again on every scroll, so it cannot borrow
        // `self` or the theme -- and one row builder for both paths is what
        // keeps a virtual list drawing the same row as a short one.
        let matches_len = matches.len();
        let rows = matches;
        let sections = self.sections.clone();
        let row_disabled_keys = self.disabled_keys.clone();
        let row_read_only = self.is_read_only;
        let row_selected_keys = self.selected_keys.clone();
        let indicator: Option<Rc<dyn Fn(bool) -> gpui::AnyElement>> =
            self.indicator.take().map(Rc::from);
        let on_change_all = self.on_selection_change_all.clone();
        let row_input_change = self.on_input_change.clone();
        let on_change_one = self.on_selection_change.clone();
        let row_state = self.state.clone();
        let row_core = core;
        let row_retained = retained;
        let selection_own = selection_own;
        let row_open_own = open_own;
        let row_open_state = open_state;
        let row_close_open = close_open.clone();
        let row_disabled_opacity = layout.disabled_opacity;
        // The rows' shared parent id, owned by the `'static` row builder:
        // each row adds its item key as its own segment below it.
        let row_base = element_id::scoped(&base_id, "item");
        // `useOption` adds `aria-posinset`/`aria-setsize` only
        // `if (isVirtualized)`; `row_height` is what windows this list.
        let row_virtualized = self.row_height.is_some();
        let panel_interactive = overlay_phase == util::OverlayPhase::Open;
        let row_count = rows.len();
        let row_padding_x = self.row_padding_x.unwrap_or(px(8.));
        let row_font_family = self.row_font_family.clone();
        let row_padding_y = self.row_padding_y.unwrap_or(px(6.));
        let rows = ComboRows {
            entity_id,
            multiple,
            cursor_at,
            colors,
            row_hover_bg: self.row_hover_bg,
            rows,
            sections,
            row_disabled_keys,
            row_read_only,
            row_selected_keys,
            indicator,
            on_change_all,
            row_input_change,
            on_change_one,
            row_state,
            row_core,
            row_retained,
            selection_own,
            row_open_own,
            row_open_state,
            row_close_open,
            row_disabled_opacity,
            row_base,
            row_virtualized,
            panel_interactive,
            row_count,
            row_padding_x,
            row_font_family,
            row_padding_y,
        };

        match self.row_height {
            // Virtual: only the rows in view are built, which is what makes
            // a thousand options affordable. The list itself is the scroll
            // container: `Infer` sizes it from its rows — the full
            // natural height on the positioner's measure pass (so the
            // flip sees the real extent, like upstream's `overlaySize`),
            // capped to the available height on the capped pass — while
            // the ListBox stays `overflow-clip`, as in v3. A fixed inner
            // height plus an outer scroller would nest two scroll
            // containers and strand rows between them.
            Some(row_height) => {
                // No `height`: a uniform `VirtualList` then sizes to its
                // rows (`ListSizingBehavior::Infer`).
                let handle = list_scroll_now;
                if handle.item_count() != matches_len {
                    handle.splice(0..handle.item_count(), matches_len);
                }
                panel = panel.child(crate::VirtualList::new(
                    element_id::scoped(&base_id, "rows"),
                    &handle,
                    move |i, _window, cx| rows.row(i, Some(row_height), cx),
                ));
            }
            None => {
                for index in 0..matches_len {
                    panel = panel.child(rows.row(index, None, cx));
                }
            }
        }

        let (slide_x, slide_y) = crate::popover::placement_entry_offset(entry_placement);
        let zoom = crate::anim::ZoomBox::panel(px(4.), container_radius).padding_x(px(4.));
        let zoom = crate::anim::ZoomBox {
            slide_x: (slide_x != 0.0).then(|| px(slide_x)),
            slide_y: (slide_y != 0.0).then(|| px(slide_y)),
            ..zoom
        };
        let panel = if overlay_phase == util::OverlayPhase::Exiting {
            crate::anim::exiting(
                panel,
                element_id::scoped(&base_id, "anim-out"),
                zoom,
                crate::anim::Motion::LIST_OUT,
                cx,
            )
        } else {
            crate::anim::entering_zoom(
                panel,
                element_id::scoped(&base_id, "anim"),
                zoom,
                crate::anim::Motion::LIST_IN,
                cx,
            )
        };
        // RAC positions the popover against the field with an 8px gap,
        // flips it when the other side has more room, and caps it at the
        // available viewport height past a 12px inset.
        util::floating(
            crate::popover::scrollable_field_popover_with_resolved_placement(
                anchor_bounds,
                self.placement,
                Some(resolved_placement),
                panel,
            ),
        )
    }
}

/// The root's key handler, holding everything it reads. The handler is
/// `'static`, so it owns copies of the frame's state rather than borrowing
/// the ComboBox.
struct ComboKeys {
    rows: Rc<[PickerItem]>,
    stops: Vec<usize>,
    core: crate::list_core::ListCore,
    retained: Entity<Option<(SharedString, String)>>,
    state: Entity<InputState>,
    allows_custom_value: bool,
    was_open: bool,
    show_all_items: Entity<bool>,
    on_selection_change: Option<OnSelectionChange>,
    on_selection_change_all: Option<OnSelectionChangeAll>,
    on_input_change: Option<OnInputChange>,
    selected_now: Vec<SharedString>,
    key_query: String,
    key_display_full: bool,
    key_items: Rc<[PickerItem]>,
    key_filter: Option<ComboFilter>,
    key_max_items: usize,
    key_disabled: std::collections::HashSet<SharedString>,
    key_menu_trigger: MenuTrigger,
    open_own_keys: Option<Entity<bool>>,
    open_focus: Entity<Option<crate::list_core::OpenFocus>>,
    on_open_change: Option<OnOpenChange>,
    key_selection_own: Option<Entity<Vec<SharedString>>>,
    key_close: ComboAction,
    key_blur: util::FocusLeave,
    key_multiple: bool,
}

impl ComboKeys {
    fn on_key_down(&self, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut App) {
        // An owned handle, so the row collection reads the way the closure
        // that held it did.
        let rows = Rc::clone(&self.rows);
        let Self {
            ref stops,
            ref core,
            ref retained,
            ref state,
            allows_custom_value,
            was_open,
            ref show_all_items,
            ref on_selection_change_all,
            ref on_input_change,
            ref selected_now,
            ref key_query,
            key_display_full,
            ref key_items,
            ref key_filter,
            key_max_items,
            ref key_disabled,
            key_menu_trigger,
            ref open_own_keys,
            ref open_focus,
            ref key_selection_own,
            ref key_close,
            key_multiple,
            ..
        } = *self;
        let key = event.keystroke.key.as_str();
        let is_open = open_own_keys
            .as_ref()
            .map_or(was_open, |held| *held.read(cx));
        let stale_cursor = core.cursor_key(cx).is_some_and(|key| {
            let current_query = state.read(cx).value().to_owned();
            let display_full = (current_query == *key_query && *show_all_items.read(cx))
                || (key_menu_trigger == MenuTrigger::Manual && !is_open);
            let current_rows: Rc<[PickerItem]> =
                if !(is_open || (allows_custom_value && !current_query.is_empty())) {
                    key_items
                        .iter()
                        .take(key_max_items)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into()
                } else if current_query == *key_query
                    && is_open == was_open
                    && display_full == key_display_full
                {
                    Rc::clone(&rows)
                } else {
                    compute_matches(
                        key_items,
                        &current_query,
                        key_max_items,
                        display_full,
                        key_filter.as_ref(),
                    )
                    .into()
                };
            let visible = current_rows.iter().any(|item| item.key() == &key);
            let retained_hidden =
                retained
                    .read(cx)
                    .as_ref()
                    .is_some_and(|(retained_key, query)| {
                        *retained_key == key && query == &current_query
                    })
                    && key_items.iter().any(|item| item.key() == &key);
            key_disabled.contains(&key) || (!visible && !retained_hidden)
        });
        if stale_cursor {
            core.set_cursor(None, cx);
            retained.update(cx, |value, _| *value = None);
        }
        // `allowsCustomValue` is the promise behind the drawn hint
        // "Press Enter to use this value". A no-match query has no
        // cursor row at all (an empty stop list makes `resolve` report
        // Ignore, so the Activate arm never runs). Pinned
        // react-stately's `commitCustomValue` keeps the typed text and
        // sets the value to null: the selection clears, and the slice
        // callback reports it only when a selection actually existed.
        // Text that still matches the selected item's label is not a
        // custom value at all — pinned `commitValue` re-runs
        // `commitSelection` there, so the selection stands and the
        // callbacks stay silent. The existing single-value commit
        // remains single-mode only; React Aria keeps multiple-mode
        // custom input independent from the selected items.
        if allows_custom_value
            && key == "enter"
            && !state.read(cx).value().is_empty()
            && core
                .cursor_key(cx)
                .is_none_or(|key| stale_cursor || !rows.iter().any(|item| item.key() == &key))
        {
            let selected_label = selected_now
                .first()
                .and_then(|item| label_of_key(key_items, item))
                .cloned()
                .unwrap_or_default();
            if !key_multiple && selected_label != state.read(cx).value() {
                let had_selection = !selected_now.is_empty();
                if let Some(held) = &key_selection_own {
                    held.update(cx, |v, cx| {
                        v.clear();
                        cx.notify();
                    });
                }
                if had_selection {
                    if let Some(cb) = &on_selection_change_all {
                        cb(&[], window, cx);
                    }
                }
            }
            core.set_cursor(None, cx);
            retained.update(cx, |value, _| *value = None);
            key_close(window, cx);
            // Pinned React Aria 3.51.0's `Enter` shortcut prevents
            // the default only while the menu is open
            // (`shouldPreventDefault = state.isOpen`): a commit on a
            // closed field — custom value or not — leaves Enter to
            // bubble into an enclosing form's implicit submission.
            if is_open {
                cx.stop_propagation();
            }
            return;
        }
        if key == "enter" && (stale_cursor || core.cursor_key(cx).is_none()) {
            let reset_value = if key_multiple {
                String::new()
            } else {
                selected_now
                    .first()
                    .and_then(|item| label_of_key(key_items, item))
                    .cloned()
                    .unwrap_or_default()
                    .to_string()
            };
            let input_changed = state.read(cx).value() != reset_value;
            state.update(cx, |value, cx| {
                value.set_value(reset_value.clone());
                cx.notify();
            });
            if input_changed {
                if let Some(cb) = &on_input_change {
                    cb(&reset_value, window, cx);
                }
            }
            key_close(window, cx);
            // Only an Enter the list acted on is kept from an
            // enclosing form: an open list reverted, or a typed
            // query discarded. A closed field with nothing to revert
            // answers Enter with nothing here, and the keystroke may
            // still bubble into the form's implicit submission.
            if is_open || stale_cursor || input_changed {
                cx.stop_propagation();
            }
            return;
        }
        // The rows the key event resolves over: the rendered collection's
        // keys, its enabled stops, and each row's typeahead label. The core
        // resolves the cursor's key to a stop the same way the frame does.
        let row_keys: Vec<SharedString> = rows.iter().map(|item| item.key().clone()).collect();
        let row_labels: Vec<String> = rows.iter().map(|item| item.label().to_string()).collect();
        let rows_view = crate::list_core::ListRows {
            keys: &row_keys,
            stops,
            labels: &row_labels,
        };
        let nav_key = if key == "tab" && is_open && core.cursor_key(cx).is_some() {
            "enter"
        } else {
            key
        };
        // A closed list's arrow opens it. The arrow's own move already seated
        // the cursor (first on a fresh field, last on ArrowUp, a step from a
        // retained one), so the open render uses `Keep`: the selected key
        // still wins when it is a stop, but no selection leaves the moved
        // cursor alone rather than resetting it to an end.
        if !is_open {
            if matches!(nav_key, "down" | "up") {
                open_focus.update(cx, |focus, _| {
                    *focus = Some(crate::list_core::OpenFocus::Keep);
                });
            }
        }
        // Pinned React Aria 3.51.0 binds PageUp/PageDown through the
        // listbox's `useSelectableCollection`, which a closed field
        // never runs: the suggestion list is not mounted, so the page
        // keys must not open it and must not move a retained cursor.
        // Open, those handlers require `manager.focusedKey != null` --
        // a mouse-opened, selection-less ComboBox has a null cursor
        // and must answer nothing until an arrow establishes one (the
        // core's page keys answer only with a cursor). With a cursor
        // the list is non-scrollable -- HeroUI v3.2.4 puts the
        // overflow scrolling on the Popover while the ListBox element
        // is `overflow-clip` -- so a page takes the enabled end
        // (`PageKeys::Ends`): `stops` already omits disabled rows,
        // whatever the list's length or scroll state.
        if matches!(nav_key, "pagedown" | "pageup") && !is_open {
            return;
        }
        match core.key_down(&rows_view, nav_key, &event.keystroke.modifiers, cx) {
            crate::list_core::ListKey::Moved { .. } => self.moved(window, cx),
            crate::list_core::ListKey::Activate(_) => self.activate(key, window, cx),
            crate::list_core::ListKey::Unhandled => {}
        }
    }

    /// The list's answer to a cursor the core walked (and already wrote and
    /// revealed): drop any retention and open the list.
    fn moved(&self, window: &mut Window, cx: &mut App) {
        let Self {
            ref retained,
            was_open,
            ref show_all_items,
            ref open_own_keys,
            ref on_open_change,
            ..
        } = *self;
        // A fresh seat clears any retention a multiple-mode pick left:
        // the cursor now answers for the row it visibly sits on.
        retained.update(cx, |value, _| *value = None);
        // Walking the list opens it, which is what typing does.
        if let Some(held) = &open_own_keys {
            held.update(cx, |v, cx| {
                *v = true;
                cx.notify();
            });
        }
        if !was_open {
            show_all_items.update(cx, |v, _| *v = true);
            if let Some(cb) = &on_open_change {
                cb(&true, window, cx);
            }
        }
    }

    /// The list's answer to Enter (or a Tab mapped to it): take the cursor
    /// row.
    fn activate(&self, key: &str, window: &mut Window, cx: &mut App) {
        let Self {
            ref core,
            ref retained,
            ref state,
            ref on_selection_change,
            ref on_selection_change_all,
            ref on_input_change,
            ref selected_now,
            ref key_items,
            ref key_selection_own,
            ref key_close,
            ref key_blur,
            key_multiple,
            ..
        } = *self;
        // The activation reads the cursor's key, not the
        // rendered row: a cursor retained across a query reset
        // (the multiple-mode pick above) can sit on an item
        // the capped or filtered list no longer renders, and
        // Enter must still toggle it.
        let Some(item_key) = core.cursor_key(cx) else {
            return;
        };
        let item_label = label_of_key(key_items, &item_key)
            .cloned()
            .unwrap_or_default();
        // An Enter that picks a row is the list's, not an
        // enclosing form's. A Tab mapped here keeps its
        // native motion: only Enter is stopped.
        if key == "enter" {
            cx.stop_propagation();
        }
        if key_multiple {
            let had_query = !state.read(cx).value().is_empty();
            state.update(cx, |st, cx| {
                st.set_value(String::new());
                cx.notify();
            });
            // The pick resets the query; retain the cursor on this
            // key across that one edit so the next arrow still knows
            // where it was.
            retained.update(cx, |value, _| {
                *value = Some((item_key.clone(), String::new()));
            });
            let mut next = selected_now.clone();
            toggle_key(&mut next, &item_key);
            if let Some(held) = &key_selection_own {
                let next = next.clone();
                held.update(cx, |v, cx| {
                    *v = next;
                    cx.notify();
                });
            }
            if let Some(cb) = &on_selection_change_all {
                cb(&next, window, cx);
            }
            if had_query {
                if let Some(cb) = &on_input_change {
                    cb("", window, cx);
                }
            }
            return;
        }
        // Taking a suggestion fills the field with the row's
        // label and closes the list, the way a click does.
        // Close first so the retained exit frame does not
        // re-run the custom filter for the value we are about
        // to copy into the input.
        key_close(window, cx);
        state.update(cx, |st, cx| {
            st.set_value(item_label.to_string());
            cx.notify();
        });
        // Uncontrolled: record the pick in the selection too,
        // or `ComboBox.Value` would never see it.
        if let Some(held) = &key_selection_own {
            let next = vec![item_key.clone()];
            held.update(cx, |v, cx| {
                *v = next;
                cx.notify();
            });
        }
        core.set_cursor(None, cx);
        retained.update(cx, |value, _| *value = None);
        if key == "tab" {
            key_blur.consume(cx);
        }
        if selected_now.first() != Some(&item_key) {
            if let Some(cb) = &on_selection_change_all {
                cb(std::slice::from_ref(&item_key), window, cx);
            }
        }
        if let Some(cb) = &on_selection_change {
            cb(&item_key, window, cx);
        }
    }
}

/// Everything a suggestion row reads, owned: the virtual list's row callback
/// is `'static` and runs again on every scroll, so it cannot borrow the
/// ComboBox or the theme -- and one row builder for both paths is what keeps
/// a virtual list drawing the same row as a short one.
struct ComboRows {
    entity_id: u64,
    multiple: bool,
    cursor_at: Option<usize>,
    colors: herogpui_theme::ThemeColors,
    row_hover_bg: Option<gpui::Hsla>,
    rows: Rc<[PickerItem]>,
    sections: Vec<(SharedString, SharedString)>,
    row_disabled_keys: std::collections::HashSet<SharedString>,
    row_read_only: bool,
    row_selected_keys: Vec<SharedString>,
    indicator: Option<Rc<dyn Fn(bool) -> gpui::AnyElement>>,
    on_change_all: Option<OnSelectionChangeAll>,
    row_input_change: Option<OnInputChange>,
    on_change_one: Option<OnSelectionChange>,
    row_state: Entity<InputState>,
    row_core: crate::list_core::ListCore,
    row_retained: Entity<Option<(SharedString, String)>>,
    selection_own: Option<Entity<Vec<SharedString>>>,
    row_open_own: Option<Entity<bool>>,
    row_open_state: bool,
    row_close_open: ComboAction,
    row_disabled_opacity: f32,
    /// The rows' shared parent id: each row adds its item key as its own
    /// segment below it.
    row_base: gpui::ElementId,
    row_virtualized: bool,
    panel_interactive: bool,
    row_count: usize,
    row_padding_x: Pixels,
    row_font_family: Option<SharedString>,
    row_padding_y: Pixels,
}

impl ComboRows {
    /// One suggestion row, with its section header when one precedes it.
    fn row(&self, index: usize, fixed_h: Option<Pixels>, cx: &mut App) -> gpui::AnyElement {
        let Self {
            entity_id,
            multiple,
            cursor_at,
            ref colors,
            ref rows,
            ref sections,
            ref row_disabled_keys,
            row_read_only,
            ref row_selected_keys,
            ref indicator,
            ref row_core,
            ref row_retained,
            row_disabled_opacity,
            ref row_base,
            row_virtualized,
            panel_interactive,
            row_count,
            row_padding_x,
            ref row_font_family,
            row_padding_y,
            ..
        } = *self;
        let row_muted = colors.muted;
        let row_hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);
        let row_focus = colors.focus;
        let row_accent = colors.accent.color;
        let item = &rows[index];
        // A section header rides above the row it introduces, so the two
        // are one element -- a virtual row is one slot tall.
        let mut head: Vec<gpui::AnyElement> = Vec::new();
        let done = |head: Vec<gpui::AnyElement>, row: gpui::AnyElement| {
            div()
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
                div()
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
        let hover_bg = row_hover_bg;
        let row_selector = format!("combobox-{entity_id}-item-{}", item.key());
        let row_selected = row_selected_keys.contains(item.key());
        let has_indicator_slot = indicator.is_some() || (multiple && row_selected);
        let mut row = div()
                        .id(element_id::scoped(row_base, item.key().clone()))
                        // `useOption.mjs`: `role: 'option'` plus
                        // `'aria-selected'` whenever the list selects at all.
                        // The highlighted row is what `useComboBox` points its
                        // input's `aria-activedescendant` at; gpui states that
                        // relation on the descendant, so the row can say it
                        // even though this component does not own the input.
                        .a11y_named(
                            a11y::Role::ListBoxOption,
                            &a11y::Name::labelled(item.label().clone()),
                        )
                        .a11y_selected(row_selected_keys.contains(item.key()))
                        .when(cursor_at == Some(index), |row| row.a11y_active_descendant())
                        .when(row_virtualized, |row| {
                            row.a11y_set_position(index, row_count)
                        })
                        .debug_selector(move || row_selector)
                        // `.list-box-item`: `min-h-9 rounded-2xl px-2 py-1.5 gap-3`.
                        .min_h(util::FIELD_HEIGHT)
                        .px(row_padding_x)
                        .py(row_padding_y)
                        .gap(px(12.))
                        .rounded(util::soft_radius(cx))
                        .flex()
                        .items_center()
                        .justify_between()
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
        } else if !row_read_only {
            row = row
                .cursor(util::interactive_cursor(cx))
                .hover(move |s| s.bg(hover_bg));
            // `useOption.onHoverStart` with `shouldFocusOnHover`: pointer
            // hover seats the virtual cursor for the next arrow key, but a
            // keyboard-active session (`focus_visible`) keeps it. The input
            // keeps real focus — only the cursor moves. A fresh seat clears
            // any retention a multiple-mode pick left behind.
            if panel_interactive {
                let retained_for_hover = row_retained.clone();
                row = row_core.seat_on_hover_with(row, item.key().clone(), move |_, cx| {
                    retained_for_hover.update(cx, |value, _| *value = None);
                });
            }
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
        let label = div().flex_1().min_w_0().whitespace_normal();
        row = row.child(label.child(item.label().to_string()));

        // `ListBox.ItemIndicator`: a caller-drawn tick replaces the
        // check glyph, and is asked for on every row so it can draw the
        // unselected state too.
        match &indicator {
            Some(render) => {
                row = row.child(
                    div()
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
            None if multiple && row_selected => {
                row = row.child(
                    div()
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

        if item_disabled || row_read_only {
            return done(head, row.into_any_element());
        }

        // Multiple mode toggles membership and leaves the panel open.
        if multiple {
            row = self.attach_toggle(row, index);
            return done(head, row.into_any_element());
        }

        // Taking a suggestion fills the field with the row's label and
        // closes the list, the way a click does; the selection rides
        // the row's key.
        row = self.attach_pick(row, index);

        done(head, row.into_any_element())
    }

    /// A multiple-mode row's pointer toggle: the panel stays open.
    fn attach_toggle(
        &self,
        mut row: gpui::Stateful<gpui::Div>,
        index: usize,
    ) -> gpui::Stateful<gpui::Div> {
        let rows = Rc::clone(&self.rows);
        let item = &rows[index];
        let Self {
            panel_interactive,
            ref row_selected_keys,
            ref on_change_all,
            ref row_input_change,
            ref row_state,
            ref row_core,
            ref row_retained,
            ref selection_own,
            ..
        } = *self;
        if panel_interactive && (on_change_all.is_some() || selection_own.is_some()) {
            let cb = on_change_all.clone();
            let own = selection_own.clone();
            let current = row_selected_keys.clone();
            let value = item.key().clone();
            let state = row_state.clone();
            let core = row_core.clone();
            let retained = row_retained.clone();
            let input_change = row_input_change.clone();
            row = row.on_click(move |_, window, cx| {
                let had_query = !state.read(cx).value().is_empty();
                let focus_handle = state.read(cx).focus_handle.clone();
                focus_handle.focus(window, cx);
                state.update(cx, |st, cx| {
                    st.set_value(String::new());
                    cx.notify();
                });
                // The pick resets the query; seat the cursor on this key and
                // retain it across that one edit.
                core.set_cursor(Some(value.clone()), cx);
                retained.update(cx, |v, _| *v = Some((value.clone(), String::new())));
                let mut next = current.clone();
                toggle_key(&mut next, &value);
                // Uncontrolled: keep the new set, or picking an
                // item would do nothing.
                if let Some(held) = &own {
                    let set = next.clone();
                    held.update(cx, |v, cx| {
                        *v = set;
                        cx.notify();
                    });
                }
                if let Some(cb) = &cb {
                    cb(&next, window, cx);
                }
                if had_query {
                    if let Some(cb) = &input_change {
                        cb("", window, cx);
                    }
                }
            });
        }
        row
    }

    /// A single-mode row's pointer pick: fills the field and closes the list.
    fn attach_pick(
        &self,
        mut row: gpui::Stateful<gpui::Div>,
        index: usize,
    ) -> gpui::Stateful<gpui::Div> {
        let item = &self.rows[index];
        let Self {
            panel_interactive,
            ref row_selected_keys,
            ref on_change_all,
            ref on_change_one,
            ref row_state,
            ref selection_own,
            ref row_open_own,
            row_open_state,
            ref row_close_open,
            ..
        } = *self;
        let value = item.key().clone();
        let label = item.label().clone();
        let state = row_state.clone();
        let on_selection_change = on_change_one.clone();
        let on_selection_change_all = on_change_all.clone();
        let selection_changed = row_selected_keys.first() != Some(&value);
        let own = selection_own.clone();
        let open_own = row_open_own.clone();
        let close_open = row_close_open.clone();
        if panel_interactive {
            row = row.on_click(move |_, window, cx| {
                let is_open = open_own
                    .as_ref()
                    .map_or(row_open_state, |held| *held.read(cx));
                if !is_open {
                    return;
                }
                // Close first so the retained exit frame does not
                // re-run the custom filter for the selected label.
                close_open(window, cx);
                state.update(cx, |s, cx| {
                    s.set_value(label.to_string());
                    cx.notify();
                });
                // Uncontrolled: record the pick in the selection too, or
                // `ComboBox.Value` would never see it.
                if let Some(held) = &own {
                    let next = vec![value.clone()];
                    held.update(cx, |v, cx| {
                        *v = next;
                        cx.notify();
                    });
                }
                if selection_changed {
                    if let Some(cb) = &on_selection_change_all {
                        cb(std::slice::from_ref(&value), window, cx);
                    }
                }
                if let Some(cb) = &on_selection_change {
                    cb(&value, window, cx);
                }
            });
        }
        row
    }
}

// The pinned `.combo-box__trigger` hovers `text-field-foreground` and stays
// `bg-transparent`; a filled hover looks plausible on screen, so the check is
// mechanical.
#[cfg(test)]
mod hover_tokens {
    #[test]
    fn the_trigger_hover_recolours_the_text_and_fills_nothing() {
        // Scan the implementation only; this test's own text names the
        // forbidden accessor.
        let source = include_str!("combo_box.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains(".hover(move |s| s.text_color(trigger_hover_fg))"),
            "the trigger hover must recolour the text to `text-field-foreground` \
             (pinned `.combo-box__trigger:hover`)"
        );
        assert!(
            !source.contains("let hover_bg = colors.default.hover();"),
            "the trigger hover must not fill a background \
             (pinned `.combo-box__trigger` is `bg-transparent`)"
        );
    }
}

crate::util::impl_component_styled!(ComboBox);
