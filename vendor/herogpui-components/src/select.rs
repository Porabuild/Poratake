// Modified by Poratake: preserve focus rings inside virtual select viewports.
//! Select — port of `@heroui/select` with single and multiple selection.
//!
//! Pinned v3.2.4 / React Aria Components 1.20.0 keep a stable `Key` separate
//! from each item's `textValue`: `value` / `defaultValue` / `selectedKeys` /
//! `disabledKeys`, the selection callbacks and the form value address items by
//! key, while typeahead and the visible text use the label. Items are
//! therefore [`crate::PickerItem`]s, exactly as in
//! [`crate::Autocomplete`] and [`crate::ComboBox`]: a label cannot serve as
//! that key — two items may share one, and then they alias each other's
//! selection, disabled state and row identity.
//!
//! The selection and the keyboard cursor are held as keys, so both follow an
//! item across a re-render that reorders the collection. Reports and the form
//! value walk the collection, so a selection reads in row order however it
//! was picked — the walk `Select.Value` has always followed.

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
};

use gpui::{
    prelude::*, px, AnimationExt, App, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window,
};
use herogpui_core::{element_id, Color, FieldVariant, Placement, SelectionMode};
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    icons,
    picker_item::PickerItem,
    selection::toggle_key,
    util,
};

type OnSelectionChange =
    std::sync::Arc<dyn Fn(&Option<SharedString>, &mut Window, &mut App) + 'static>;

// This port approximates pinned RAC's en-US `Intl.ListFormat` conjunction.
fn format_selected_names(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [name] => name.clone(),
        [first, second] => format!("{first} and {second}"),
        _ => format!(
            "{}, and {}",
            names[..names.len() - 1].join(", "),
            names.last().expect("the multiple-item branch is non-empty")
        ),
    }
}

/// A selection re-ordered to collection order: every chosen key that still
/// resolves to an item, in row order. Select reports its selection through
/// this walk — the trigger text, the plural callback, the form value and
/// `Select.Value` all read the collection the way this does.
fn in_collection_order(selected: &[SharedString], keys: &[SharedString]) -> Vec<SharedString> {
    keys.iter()
        .filter(|key| selected.contains(*key))
        .cloned()
        .collect()
}

/// The chosen keys that resolve to collection items, in row order — the
/// single key and the multiple set are disjoint channels, so filtering on
/// both is safe. Keys the collection no longer holds resolve to nothing, the
/// way an out-of-range index always did.
fn resolved_keys(
    items: &[PickerItem],
    single: &Option<SharedString>,
    multiple: &[SharedString],
) -> Vec<SharedString> {
    items
        .iter()
        .map(|item| item.key())
        .filter(|key| single.as_ref() == Some(*key) || multiple.contains(*key))
        .cloned()
        .collect()
}

/// Pinned React Stately 3.49.0 `useMultipleSelectionState`'s anchor record,
/// on the option keys a Select's `stops` walk: where a Shift extension
/// reaches from, how far the last one went, and whether the selection is the
/// raw `all` a `selectAll` produced. It lives beside the cursor in keyed
/// state, so it survives closing and reopening the popover the way the
/// pinned hook survives a listbox remount.
#[derive(Clone, Debug, Default)]
struct SelectSelectionRange {
    anchor: Option<SharedString>,
    current: Option<SharedString>,
    is_all: bool,
}

/// The selection after extending to `target` from `range`'s anchor.
///
/// The old anchor..current range is *replaced* by anchor..target, so extending
/// backwards shrinks again; a raw `all` collapses to the new key; a first
/// extension without an anchor selects from the target itself, which is what
/// the pinned SelectionManager does when nothing anchors it yet. Only
/// `selectable` keys enter the range, so disabled options are skipped, and
/// the result walks the collection so it reports in row order.
fn extend_selection_range(
    current: &[SharedString],
    collection: &[SharedString],
    selectable: &[SharedString],
    range: &SelectSelectionRange,
    target: &SharedString,
) -> Vec<SharedString> {
    if range.is_all {
        return vec![target.clone()];
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
    let mut next: Vec<SharedString> = current.to_vec();
    if let Some(previous_range) = between(anchor_at, previous_at) {
        for at in previous_range {
            let stale = &collection[at];
            if let Some(held) = next.iter().position(|key| key == stale) {
                next.remove(held);
            }
        }
    }
    if let Some(target_range) = between(anchor_at, target_at) {
        for at in target_range {
            let key = &collection[at];
            if selectable.contains(key) && !next.contains(key) {
                next.push(key.clone());
            }
        }
    }
    in_collection_order(&next, collection)
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
/// outside the registration is entirely inert: no cursor move, no selection,
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
/// cursor alone -- so the platform is an explicit bool rather than a `cfg!`.
fn shift_home_end_extends(key_name: &str, control: bool, macos: bool) -> bool {
    !matches!(key_name, "home" | "end") || (!macos && control)
}

/// The optional `Select.ClearButton` composition part.
///
/// Passed to [`Select::clear_button`]. It has no separate keyboard focus stop:
/// Backspace and Delete on the closed trigger perform the same clear action.
#[derive(Default)]
pub struct SelectClearButton {
    children: Vec<gpui::AnyElement>,
    on_click: Option<std::sync::Arc<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App)>>,
}

impl ParentElement for SelectClearButton {
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
        self.children.extend(elements);
    }
}

impl SelectClearButton {
    /// Creates a clear button.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the default close icon with caller content.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// Called after the selection request and the root's `on_clear` callback.
    pub fn on_click(
        mut self,
        callback: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(std::sync::Arc::new(callback));
        self
    }
}

/// HeroUI Select (controlled).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Select {
    /// `name` — the name this control submits under; read back by
    /// [`Self::form_field`].
    name: Option<SharedString>,
    id: gpui::ElementId,
    items: Vec<PickerItem>,
    selected: Option<SharedString>,
    /// Whether `value` was supplied. `Option<SharedString>` cannot distinguish
    /// "controlled, nothing selected" from "uncontrolled" on its own.
    is_controlled: bool,
    default_value: Option<SharedString>,
    /// Backs `selectionMode="multiple"`; `selected` backs `single`.
    selected_keys: Vec<SharedString>,
    is_multiple_controlled: bool,
    default_selected_keys: Vec<SharedString>,
    selection_mode: SelectionMode,
    /// `isOpen` — `None` leaves the component holding the flag, seeded from
    /// `defaultOpen`.
    is_open: Option<bool>,
    default_open: bool,
    placement: Placement,
    label: Option<SharedString>,
    /// `aria-label` — the explicit accessible name, which wins over the
    /// label-derived name and renders no visible label of its own.
    aria_label: Option<SharedString>,
    placeholder: SharedString,
    /// Whether `placeholder` came from the caller; otherwise render swaps
    /// in the localized default (`i18n::UiString::SelectPlaceholder`).
    placeholder_is_set: bool,
    description: Option<SharedString>,
    variant: FieldVariant,
    variant_is_set: bool,
    is_disabled: bool,
    is_invalid: bool,
    /// `shouldFocusWrap` — whether the arrow keys wrap at the ends of the list.
    should_focus_wrap: bool,
    /// `ListLayout`'s `rowHeight`, which virtualizes the popover list.
    row_height: Option<Pixels>,
    /// Replaces the list rows' `px-2.5` horizontal padding.
    row_padding_x: Option<Pixels>,
    trigger_text_size: Option<Pixels>,
    row_text_size: Option<Pixels>,
    panel_padding: Option<Pixels>,
    panel_gap: Option<Pixels>,
    panel_min_width: Option<Pixels>,
    /// Replaces the list rows' `py-1.5` vertical padding.
    row_padding_y: Option<Pixels>,
    /// The fill a hovered option row takes, in place of `--default`.
    row_hover_bg: Option<gpui::Hsla>,
    /// The trigger's hover endpoint, in place of the variant's hover token.
    trigger_hover_bg: Option<gpui::Hsla>,
    /// The family the option rows are drawn with; unset keeps the inherited
    /// family. A detached popover does not inherit the trigger's font.
    row_font_family: Option<SharedString>,
    /// The trigger text's family; unset keeps the inherited family.
    font_family: Option<SharedString>,
    /// The corner radius of the detached panel, in place of the owning
    /// `picker_radius` helper.
    radius: Option<Pixels>,
    /// `ListBox.Section` — the heading that precedes an option, by item key.
    sections: Vec<(SharedString, SharedString)>,
    /// `ListBox.ItemIndicator` — draws the tick. The closure is handed whether
    /// the row is selected.
    indicator: Option<Box<dyn Fn(bool) -> gpui::AnyElement + 'static>>,
    #[allow(clippy::type_complexity)]
    item_leading: Option<Box<dyn Fn(&SharedString, bool) -> Option<gpui::AnyElement> + 'static>>,
    item_content: Option<
        std::sync::Arc<dyn Fn(&SharedString, bool, &mut Window, &mut App) -> gpui::AnyElement>,
    >,
    /// `Select.Indicator` — draws the trigger indicator. The closure is handed
    /// whether the popover is open; arbitrary caller content is kept as-is,
    /// while the built-in SVG follows the pinned 150ms rotation.
    trigger_indicator: Option<Box<dyn Fn(bool) -> gpui::AnyElement + 'static>>,
    /// `Select.Value` — draws the trigger's value. The closure is handed the
    /// selected key, or `None` while the placeholder shows.
    value_content: Option<Box<dyn Fn(util::SelectionValue<'_>) -> gpui::AnyElement + 'static>>,
    is_required: bool,
    disabled_keys: HashSet<SharedString>,
    full_width: bool,
    on_open_change: Option<std::sync::Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    on_selection_change: Option<OnSelectionChange>,
    clear_buttons: Vec<SelectClearButton>,
    on_clear: Option<std::sync::Arc<dyn Fn(&mut Window, &mut App)>>,
    on_selection_change_all:
        Option<std::sync::Arc<dyn Fn(&[SharedString], &mut Window, &mut App) + 'static>>,
    /// Optional trigger geometry/chrome overrides; defaults are the stock box.
    field: util::FieldBox,
    /// Mirrors the current selection, validity, successful state, focus and
    /// reset behavior for a live [`crate::form::FormField`].
    form_state: Rc<RefCell<crate::form::LiveFormFieldState>>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
    recipes: Vec<SharedString>,
}

impl Select {
    /// Composes a clear control inside the trigger. Repeated calls add parts.
    /// An empty selection hides the control while retaining its layout space.
    pub fn clear_button(mut self, button: SelectClearButton) -> Self {
        self.clear_buttons.push(button);
        self
    }

    /// Reports a nonempty selection's clear request, including controlled mode.
    pub fn on_clear(mut self, callback: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(std::sync::Arc::new(callback));
        self
    }

    /// `onChange` — the v3 name for [`Select::on_selection_change`].
    pub fn on_change(
        self,
        handler: impl Fn(&Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change(handler)
    }

    /// `disabledKeys` — keys of the items that cannot be chosen. Disabled
    /// state is per key, so one of two same-label items can be disabled
    /// alone.
    pub fn disabled_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.disabled_keys = keys.into_iter().collect();
        self
    }

    /// `shouldFocusWrap` — whether the arrow keys wrap at the ends of the list.
    /// `ListLayout`'s `rowHeight` -- and what virtualizes the popover list.
    ///
    /// v3 wraps the list in `<Virtualizer layout={ListLayout}>` inside
    /// `Select.Popover`; gpui's `uniform_list` builds only the rows in view, and
    /// it can do that because every row is this tall.
    pub fn row_height(mut self, h: impl Into<Pixels>) -> Self {
        self.row_height = Some(h.into());
        self
    }

    /// Sets whether keyboard focus wraps at the ends of the list (v3 `shouldFocusWrap`).
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

    /// Draws a leading element on each option row, before its label.
    ///
    /// v3's `ListBox.Item` takes its children as a render function, so an
    /// option is free to compose an avatar, an icon or a colour swatch beside
    /// its text. [`crate::picker_item::PickerItem`] carries only a key and a
    /// label — it is shared with the combo box, the autocomplete, the tag
    /// filter and the swatch picker, and holds no element — so the per-row
    /// element is supplied here instead, where the select already owns
    /// [`Select::indicator`].
    ///
    /// The closure receives the row's key and whether it is selected, and
    /// returns `None` for a row that takes no leading element. The element
    /// sits in the row's normal `gap-3` flow, before the label, and the
    /// trailing indicator slot is untouched.
    ///
    /// This draws the *rows*. A swatch on the closed trigger is
    /// [`Select::value_content`], which is a separate slot upstream too.
    pub fn item_leading(
        mut self,
        render: impl Fn(&SharedString, bool) -> Option<gpui::AnyElement> + 'static,
    ) -> Self {
        self.item_leading = Some(Box::new(render));
        self
    }

    #[doc = "Replaces an option’s visible content while keeping its label for accessibility and typeahead. The selected indicator remains owned by Select."]
    pub fn item_content(
        mut self,
        render: impl Fn(&SharedString, bool, &mut Window, &mut App) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.item_content = Some(std::sync::Arc::new(render));
        self
    }

    /// `Select.Indicator` — draw the trigger indicator yourself.
    ///
    /// The closure receives the current open state, which is the GPUI analog
    /// of v3's `data-open` attribute. The default SVG indicator rotates over
    /// the pinned 150ms transition; caller-provided content remains caller
    /// owned and can use this state to render its own visual.
    pub fn trigger_indicator(
        mut self,
        render: impl Fn(bool) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.trigger_indicator = Some(Box::new(render));
        self
    }

    /// `Select.Value` — draw the trigger's value yourself.
    ///
    /// The closure is handed the render props v3 passes into
    /// `<Select.Value>{({defaultChildren, isPlaceholder, selectedItems}) => …}`,
    /// so a `multiple` select can draw all of them and a caller can fall back to
    /// what the trigger would have drawn.
    pub fn value_content(
        mut self,
        render: impl Fn(util::SelectionValue<'_>) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.value_content = Some(Box::new(render));
        self
    }

    /// Sets the invalid state (v3 `isInvalid`).
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// Sets the required state (v3 `isRequired`).
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// `fullWidth` — stretch to the container width.
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

    /// Replaces the trigger's `px-3` horizontal padding.
    pub fn padding_x(mut self, p: impl Into<Pixels>) -> Self {
        self.field.padding_x = Some(p.into());
        self
    }

    /// Adds vertical padding to the trigger, in place of v3's `py-2`.
    ///
    /// The trigger is a `min-h` box, so this changes nothing for a value that
    /// fits on one line: `px(8.)` on the stock 20px line advance resolves to
    /// the same 36px the minimum already imposes. A value that wraps to two
    /// lines then grows the trigger the way upstream's `py-2` does, instead of
    /// keeping the 36px floor and pressing the two lines against its edges.
    ///
    /// Unset leaves the trigger unpadded, which is the pre-0.10.0 geometry.
    /// It is the select that carries this rather than the whole field family:
    /// the other members hold a single-line editor, whose box
    /// [`Select::height`]'s counterparts already name outright.
    pub fn padding_y(mut self, p: impl Into<Pixels>) -> Self {
        self.field.padding_y = Some(p.into());
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

    /// Overrides the trigger text size in pixels; unset preserves the stock metric.
    pub fn trigger_text_size(mut self, value: impl Into<Pixels>) -> Self {
        self.trigger_text_size = Some(value.into());
        self
    }

    /// Overrides the row text size in pixels; unset preserves the stock metric.
    pub fn row_text_size(mut self, value: impl Into<Pixels>) -> Self {
        self.row_text_size = Some(value.into());
        self
    }

    /// Overrides the panel padding in pixels; unset preserves the stock metric.
    pub fn panel_padding(mut self, value: impl Into<Pixels>) -> Self {
        self.panel_padding = Some(value.into());
        self
    }

    #[doc = "Sets spacing between option rows, including fixed-height virtual rows."]
    pub fn panel_gap(mut self, value: impl Into<Pixels>) -> Self {
        self.panel_gap = Some(value.into());
        self
    }

    #[doc = "Sets a minimum width for the detached option panel."]
    pub fn panel_min_width(mut self, value: impl Into<Pixels>) -> Self {
        self.panel_min_width = Some(value.into());
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
    /// unchanged; a focused, invalid, disabled or bare trigger does not hover,
    /// so the override never reaches those states. Not a v3 prop: v3 tints the
    /// trigger with a class.
    pub fn trigger_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.trigger_hover_bg = Some(color.into());
        self
    }

    /// The fill a hovered option row takes, in place of `--default`. v3 tints
    /// the row with a class; this names the colour.
    pub fn row_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.row_hover_bg = Some(color.into());
        self
    }

    /// The family the trigger's value and placeholder are drawn with; unset
    /// keeps the inherited family. The detached rows take
    /// [`Select::row_font_family`] instead. Not a v3 prop; v3 sets it with a
    /// class.
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
    /// applied to the select's root element after every value the variant and
    /// the active theme chose, so they win. The trigger paints its own chrome,
    /// so this reaches the box that chrome sits in, not the chrome.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }

    /// Creates a select from an element id and its items.
    pub fn new(id: impl Into<gpui::ElementId>, items: Vec<PickerItem>) -> Self {
        Self {
            name: None,
            id: id.into(),
            items,
            selected: None,
            is_controlled: false,
            default_value: None,
            selected_keys: Vec::new(),
            is_multiple_controlled: false,
            default_selected_keys: Vec::new(),
            selection_mode: SelectionMode::Single,
            is_open: None,
            default_open: false,
            placement: Placement::BottomStart,
            label: None,
            aria_label: None,
            placeholder: "Select an item".into(),
            placeholder_is_set: false,
            description: None,
            variant: FieldVariant::Primary,
            variant_is_set: false,
            is_disabled: false,
            is_invalid: false,
            should_focus_wrap: false,
            row_height: None,
            row_padding_x: None,
            trigger_text_size: None,
            row_text_size: None,
            panel_padding: None,
            panel_gap: None,
            panel_min_width: None,
            row_padding_y: None,
            row_hover_bg: None,
            trigger_hover_bg: None,
            row_font_family: None,
            font_family: None,
            item_leading: None,
            item_content: None,
            radius: None,
            field: util::FieldBox::default(),
            sections: Vec::new(),
            indicator: None,
            trigger_indicator: None,
            value_content: None,
            is_required: false,
            disabled_keys: HashSet::new(),
            full_width: false,
            on_open_change: None,
            on_selection_change: None,
            clear_buttons: Vec::new(),
            on_clear: None,
            on_selection_change_all: None,
            form_state: live_form_state(),
            sx: None,
            recipes: Vec::new(),
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
    /// its ancestor, so the control hands the pair over instead. Borrows, so the
    /// control is still yours to place:
    ///
    /// ```
    /// # use gpui::{prelude::*, Window};
    /// # use herogpui_components::{Form, PickerItem, Select};
    /// # struct Demo;
    /// # impl Render for Demo {
    /// #     fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    /// #         let form = Form::new();
    /// #         let control =
    /// #             Select::new("city", vec![PickerItem::new("kyiv", "Kyiv")]).name("city");
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
        let selected = if self.is_controlled {
            self.selected.clone()
        } else {
            self.default_value.clone()
        };
        let selected_keys = if self.is_multiple_controlled {
            self.selected_keys.clone()
        } else {
            self.default_selected_keys.clone()
        };
        sync_select_form(
            &self.form_state,
            select_form_value(&self.items, &selected, &selected_keys),
            self.is_invalid,
            !self.is_disabled,
        );
        Some(
            crate::form::FormField::live(name, self.form_state.clone())
                .is_required(self.is_required),
        )
    }

    /// `selectionMode`
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// The chosen item keys under `selectionMode="multiple"` — `selectedKeys`,
    /// the `ListBox` spelling of [`Select::value`]'s controlled set.
    pub fn selected_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.selected_keys = keys.into_iter().collect();
        self.is_multiple_controlled = true;
        self
    }

    /// `defaultSelectedKeys` under `selectionMode="multiple"` — the
    /// uncontrolled initial selection.
    pub fn default_selected_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.default_selected_keys = keys.into_iter().collect();
        self
    }

    /// Reports the whole selection, for `selectionMode="multiple"`.
    pub fn on_selection_change_all(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change_all = Some(std::sync::Arc::new(handler));
        self
    }

    /// `value` — the selected item, by key. Supplying it makes the select
    /// controlled, even with `None`.
    pub fn value(mut self, key: Option<SharedString>) -> Self {
        self.selected = key;
        self.is_controlled = true;
        self
    }

    /// `defaultValue` — the uncontrolled initial selection, by key.
    ///
    /// Only consulted when `value` is not supplied; the select then owns the
    /// selection and choosing an option moves it.
    pub fn default_value(mut self, key: Option<SharedString>) -> Self {
        self.default_value = key;
        self
    }

    /// `placement` on the popover.
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

    /// Sets the label.
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }

    /// `aria-label` — the explicit accessible name, which wins over the
    /// label-derived name and renders no visible label of its own.
    pub fn aria_label(mut self, l: impl Into<SharedString>) -> Self {
        self.aria_label = Some(l.into());
        self
    }

    /// Sets the placeholder text.
    pub fn placeholder(mut self, p: impl Into<SharedString>) -> Self {
        self.placeholder = p.into();
        self.placeholder_is_set = true;
        self
    }

    /// Sets the description.
    pub fn description(mut self, d: impl Into<SharedString>) -> Self {
        self.description = Some(d.into());
        self
    }

    /// Sets the field variant (v3 `variant`).
    pub fn variant(mut self, v: FieldVariant) -> Self {
        self.variant = v;
        self.variant_is_set = true;
        self
    }

    /// Named theme overlay from [`herogpui_theme::ComponentThemes::select`].
    /// Stackable; a missing name adds no override.
    pub fn recipe(mut self, name: impl Into<SharedString>) -> Self {
        self.recipes.push(name.into());
        self
    }

    /// Sets the disabled state (v3 `isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets the handler called when the open state changes (v3 `onOpenChange`).
    pub fn on_open_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_change = Some(std::sync::Arc::new(f));
        self
    }

    /// Sets the handler called when the selection changes (v3 `onSelectionChange`).
    pub fn on_selection_change(
        mut self,
        f: impl Fn(&Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(std::sync::Arc::new(f));
        self
    }
}

impl Select {
    fn value_text_single(&self, selected: &Option<SharedString>) -> SharedString {
        selected
            .as_ref()
            .and_then(|key| self.items.iter().find(|item| item.key() == key))
            .map_or_else(|| self.placeholder.clone(), |item| item.label().clone())
    }

    fn value_text_multiple(&self, selected_keys: &[SharedString]) -> SharedString {
        let names: Vec<String> = self
            .items
            .iter()
            .filter(|item| selected_keys.contains(item.key()))
            .map(|item| item.label().to_string())
            .collect();
        if names.is_empty() {
            self.placeholder.clone()
        } else {
            SharedString::from(format_selected_names(&names))
        }
    }
}

/// The controlled halves `render` resolves first: the open flag, the overlay
/// registration and the selection in whichever mode is active. `controlled`
/// takes `cx` mutably, so these precede every other read.
struct SelectControlled {
    is_open: bool,
    open_own: Option<gpui::Entity<bool>>,
    overlay_phase: util::OverlayPhase,
    dismissal_token: util::OverlayToken,
    multiple: bool,
    selected: Option<SharedString>,
    value_own: Option<gpui::Entity<Option<SharedString>>>,
    selected_keys: Vec<SharedString>,
    indices_own: Option<gpui::Entity<Vec<SharedString>>>,
}

/// Everything one Select frame shares between its painted parts: the
/// controlled state, the keyed handles (focus, cursor, Shift-range anchor,
/// scroll, typeahead) and the clear affordance's derived flags. `render`
/// resolves it once in [`Select::frame`]; the trigger, its key handler, the
/// clear buttons and the popover list all read this one copy.
struct SelectFrame {
    is_open: bool,
    open_own: Option<gpui::Entity<bool>>,
    overlay_phase: util::OverlayPhase,
    dismissal_token: util::OverlayToken,
    resolved_placement: Rc<Cell<Option<Placement>>>,
    entry_placement: Placement,
    multiple: bool,
    selected: Option<SharedString>,
    value_own: Option<gpui::Entity<Option<SharedString>>>,
    selected_keys: Vec<SharedString>,
    indices_own: Option<gpui::Entity<Vec<SharedString>>>,
    keys: Vec<SharedString>,
    focus_handle: gpui::FocusHandle,
    cursor_at: Option<usize>,
    keyboard_press_open: gpui::Entity<Option<bool>>,
    open_focus: gpui::Entity<Option<crate::list_core::OpenFocus>>,
    selection_range: gpui::Entity<SelectSelectionRange>,
    list_scroll_now: gpui::UniformListScrollHandle,
    panel_scroll_now: gpui::ScrollHandle,
    typeahead: gpui::Entity<crate::list_nav::Typeahead>,
    list_core: crate::list_core::ListCore,
    blur_scope: gpui::FocusHandle,
    clear_empty: bool,
    clear_enabled: bool,
    clear_slots: Vec<util::Interaction>,
    clear_hovered: bool,
    clear_selection: SelectAction,
    anchor_bounds: Rc<Cell<Option<gpui::Bounds<Pixels>>>>,
    trigger_pressed: Rc<Cell<bool>>,
}

type SelectAction = std::sync::Arc<dyn Fn(&mut Window, &mut App)>;
type OnOpenChange = std::sync::Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;
type OnSelectionChangeAll =
    std::sync::Arc<dyn Fn(&[SharedString], &mut Window, &mut App) + 'static>;

impl RenderOnce for Select {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if !self.placeholder_is_set {
            self.placeholder = crate::i18n::ui_string(crate::i18n::UiString::SelectPlaceholder, cx);
        }
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (is_open, open_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "open"),
            self.is_open,
            self.default_open,
        );
        let (overlay_phase, dismissal_token) = util::overlay_scope(
            window,
            cx,
            element_id::scoped(&self.id, "overlay"),
            is_open,
            true,
        );
        let overlay_active = overlay_phase != util::OverlayPhase::Closed;
        let multiple = self.selection_mode == SelectionMode::Multiple;
        let (selected, value_own) = if multiple {
            (None, None)
        } else {
            util::controlled(
                window,
                cx,
                element_id::scoped(&self.id, "value"),
                self.is_controlled.then_some(self.selected.clone()),
                self.default_value.clone(),
            )
        };
        let (selected_keys, indices_own) = if multiple {
            util::controlled(
                window,
                cx,
                element_id::scoped(&self.id, "values"),
                self.is_multiple_controlled.then(|| {
                    crate::selection::normalize_selection(self.selected_keys.clone(), true)
                }),
                crate::selection::normalize_selection(self.default_selected_keys.clone(), true),
            )
        } else {
            (Vec::new(), None)
        };
        let frame = self.frame(
            SelectControlled {
                is_open,
                open_own,
                overlay_phase,
                dismissal_token,
                multiple,
                selected,
                value_own,
                selected_keys,
                indices_own,
            },
            window,
            cx,
        );
        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        self.apply_theme(&colors, cx);
        let mut field = self.trigger_field(&frame, &colors, &layout, window, cx);

        // Down or Enter on a closed Select opens it, and the arrows then walk
        // the options -- the same keys React Aria binds.
        if !self.is_disabled {
            field = self.trigger_keys(field, &frame);
        }

        let (value_text, value_slot) = self.trigger_value(&frame, &colors);
        // `react-aria/dist/private/select/useSelect.mjs` derives the trigger
        // from `useMenuTrigger({type: 'listbox'})`, so
        // `.../overlays/useOverlayTrigger.mjs` gives it
        // `'aria-haspopup': 'listbox'`, `'aria-expanded': isOpen` and
        // `'aria-controls': isOpen ? overlayId : undefined`; HeroUI renders it
        // as an RAC `Button` (`select/select.js`), i.e. a native `<button>`.
        // Only `aria-expanded` ports: `aria-haspopup` and `aria-controls` have
        // no gpui builder and no id graph to point at (see `crate::a11y`).
        // `useSelect` names the trigger `aria-labelledby: [valueId, label]`,
        // which resolves to the field's label followed by the drawn value.
        field = field
            .a11y_named(
                a11y::Role::Button,
                &a11y::Name::maybe(self.aria_label.clone().or(self.label.clone()))
                    .described(Some(value_text)),
            )
            .a11y_expanded(frame.is_open);
        field = field.child(value_slot);
        field = self.clear_buttons(field, &frame, &colors, window, cx);
        field = self.trigger_indicator_slot(field, &frame, &colors, window, cx);
        field = self.trigger_toggle(field, &frame);
        if !self.is_disabled {
            field = util::record_focus_bounds(field, &frame.focus_handle, window, cx);
        }

        // The popup anchors to the trigger bounds — not to the
        // label-to-description wrapper root — the way RAC's
        // `useOverlayPosition` positions against the trigger rect.
        // `scrollable_field_popover` below reads these bounds to flip and
        // cap the panel; the measure element itself only records them.
        let field = crate::popover::PopoverTriggerMeasure::new(field, frame.anchor_bounds.clone());

        let mut root = self.field_root(field);
        root = self.root_dismissals(root, &frame);
        let blur_scope = frame.blur_scope.clone();
        if overlay_active && !self.items.is_empty() {
            root = root.child(self.popover(frame, &colors, &layout, window, cx));
        }

        root = util::apply_sx(root, &self.sx);
        root.track_focus(&blur_scope)
    }
}

impl Select {
    /// Mirrors the selection into the live form state and installs the
    /// reset that restores the default selection.
    fn sync_form(&self, controlled: &SelectControlled, window: &mut Window, cx: &mut App) {
        let SelectControlled {
            multiple,
            ref selected,
            ref selected_keys,
            ref value_own,
            ref indices_own,
            ..
        } = *controlled;
        let form_default_keys = if multiple {
            let reset_keys = if self.is_multiple_controlled {
                self.selected_keys.clone()
            } else {
                self.default_selected_keys.clone()
            };
            let slot = window.use_keyed_state(
                element_id::scoped(&self.id, "form-default"),
                cx,
                move |_, _| reset_keys,
            );
            slot.read(cx).clone()
        } else {
            Vec::new()
        };
        sync_select_form(
            &self.form_state,
            select_form_value(&self.items, selected, selected_keys),
            self.is_invalid,
            !self.is_disabled,
        );
        let reset_own = value_own.clone();
        let reset_keys_own = indices_own.clone();
        let reset_state = Rc::downgrade(&self.form_state);
        let reset_change = self
            .is_controlled
            .then(|| self.on_selection_change.clone())
            .flatten();
        let reset_change_all = self
            .is_multiple_controlled
            .then(|| self.on_selection_change_all.clone())
            .flatten();
        let reset_key = self.default_value.clone();
        let reset_items = self.items.clone();
        let reset_mode = self.selection_mode;
        self.form_state.borrow_mut().restore = (reset_own.is_some()
            || (multiple && reset_keys_own.is_some())
            || reset_change.is_some()
            || (multiple && reset_change_all.is_some()))
        .then(|| {
            util::shared(move |window: &mut Window, cx: &mut App| {
                if reset_mode == SelectionMode::Multiple {
                    if let Some(state) = reset_state.upgrade() {
                        state.borrow_mut().value =
                            select_form_value(&reset_items, &None, &form_default_keys);
                    }
                    if let Some(held) = &reset_keys_own {
                        held.update(cx, |selected, cx| {
                            *selected = form_default_keys.clone();
                            cx.notify();
                        });
                    }
                    if let Some(on_change) = &reset_change_all {
                        on_change(&form_default_keys, window, cx);
                    }
                } else {
                    if let Some(state) = reset_state.upgrade() {
                        state.borrow_mut().value = select_form_value(&reset_items, &reset_key, &[]);
                    }
                    if let Some(held) = &reset_own {
                        held.update(cx, |selected, cx| {
                            *selected = reset_key.clone();
                            cx.notify();
                        });
                    }
                    if let Some(on_change) = &reset_change {
                        on_change(&reset_key, window, cx);
                    }
                }
            }) as std::sync::Arc<dyn Fn(&mut Window, &mut App)>
        });
    }

    /// Resolves the frame's keyed handles and derived flags on top of the
    /// controlled state.
    fn frame(
        &self,
        controlled: SelectControlled,
        window: &mut Window,
        cx: &mut App,
    ) -> SelectFrame {
        self.sync_form(&controlled, window, cx);
        let SelectControlled {
            is_open,
            open_own,
            overlay_phase,
            dismissal_token,
            multiple,
            selected,
            value_own,
            selected_keys,
            indices_own,
        } = controlled;
        // The field popover resolves flips during prepaint. Keep the
        // requested placement and the physical side in keyed state so the
        // next entry frame uses the side that was actually painted.
        let (resolved_placement, entry_placement) =
            crate::popover::field_placement_feedback(window, cx, &self.id, self.placement);

        // The collection's keys in row order — the channel the selection,
        // the cursor and every report address.
        let keys: Vec<SharedString> = self.items.iter().map(|item| item.key().clone()).collect();

        // The trigger is what holds focus, so the open list can be walked with
        // the arrows the way v3's is.
        let focus_handle =
            window.use_keyed_state(element_id::scoped(&self.id, "focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            });
        let focus_handle = focus_handle.read(cx).clone();
        self.form_state.borrow_mut().focus = Some(focus_handle.clone());
        // Which row the keyboard is on, held as the item's *key* so the cursor
        // stays on the same item when the caller reorders the collection.
        let cursor = crate::list_core::ListCore::cursor_state(&self.id, window, cx);
        let keyboard_press_open = window.use_keyed_state(
            element_id::scoped(&self.id, "keyboard-press"),
            cx,
            |_, _| None::<bool>,
        );
        // The open strategy the trigger left for this open -- Enter/Space/
        // ArrowDown `First`, ArrowUp `Last`, a pointer press `None` -- consumed
        // by `seat_on_open` on the first open render, the way Dropdown's
        // `focus_first` is.
        let open_focus =
            window.use_keyed_state(element_id::scoped(&self.id, "open-focus"), cx, |_, _| {
                None::<crate::list_core::OpenFocus>
            });
        // The Shift-range anchor lives beside the cursor, keyed off the same
        // instance id, so two selects never share an anchor and a closed
        // popover leaves its anchor standing for the reopen.
        let selection_range =
            window.use_keyed_state(element_id::scoped(&self.id, "range"), cx, |_, _| {
                SelectSelectionRange::default()
            });
        // v3's list is `overflow-y-auto`, and React Aria keeps the focused
        // option in view. Both need a handle: the virtual list has its own kind,
        // and a plain scrolling div has the other. `use_keyed_state` takes `cx`
        // mutably, so they precede the theme.
        let list_scroll =
            window.use_keyed_state(element_id::scoped(&self.id, "list-scroll"), cx, |_, _| {
                gpui::UniformListScrollHandle::new()
            });
        let panel_scroll =
            window.use_keyed_state(element_id::scoped(&self.id, "panel-scroll"), cx, |_, _| {
                gpui::ScrollHandle::new()
            });
        let list_scroll_now = list_scroll.read(cx).clone();
        let panel_scroll_now = panel_scroll.read(cx).clone();
        // The letters typed so far, which a search resetting every frame could
        // not accumulate.
        let typeahead = crate::list_core::ListCore::typeahead_state(&self.id, window, cx);
        // The previous frame's row keys, which the removed-row repair walks.
        let previous_rows = crate::list_core::ListCore::rows_state(&self.id, window, cx);

        // Pinned `usePopover` closes when focus leaves the trigger-plus-list
        // scope. Blur deliberately leaves focus on its destination.
        let blur_base = element_id::scoped(&self.id, "select");
        let blur_close_own = open_own.clone();
        let blur_open_change = self.on_open_change.clone();
        let blur_scope = util::close_on_blur(window, cx, &blur_base, is_open, move |window, cx| {
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

        // `setSelectedKeys(empty)` clears even a required Select. A controlled
        // owner's value and live form data stay authoritative until it updates.
        let clear_empty = if multiple {
            selected_keys.is_empty()
        } else {
            selected.is_none()
        };
        let has_clear = !self.clear_buttons.is_empty();
        let clear_enabled = has_clear && !self.is_disabled && !clear_empty;
        let clear_slots: Vec<_> = (0..self.clear_buttons.len())
            .map(|index| {
                util::interaction(
                    element_id::scoped(&self.id, format!("clear-{index}")),
                    window,
                    cx,
                )
            })
            .collect();
        let clear_hovered = clear_enabled && clear_slots.iter().any(|slot| slot.read(cx).0);
        let clear_own = value_own.clone();
        let clear_keys_own = indices_own.clone();
        let clear_form = self.form_state.clone();
        let clear_single = self.on_selection_change.clone();
        let clear_multiple = self.on_selection_change_all.clone();
        let clear_callback = self.on_clear.clone();
        let clear_range = selection_range.clone();
        let clear_open = open_own.clone();
        let clear_open_callback = self.on_open_change.clone();
        let clear_selection = util::shared(move |window: &mut Window, cx: &mut App| {
            if !clear_enabled {
                return;
            }
            if multiple {
                if let Some(own) = &clear_keys_own {
                    clear_form.borrow_mut().value = crate::form::FormValue::Keys(Vec::new());
                    own.update(cx, |value, cx| {
                        value.clear();
                        cx.notify();
                    });
                }
                if let Some(callback) = &clear_multiple {
                    callback(&[], window, cx);
                }
            } else {
                if let Some(own) = &clear_own {
                    clear_form.borrow_mut().value = crate::form::FormValue::Keys(Vec::new());
                    own.update(cx, |value, cx| {
                        *value = None;
                        cx.notify();
                    });
                }
                if let Some(callback) = &clear_single {
                    callback(&None, window, cx);
                }
                // React Stately's single-selection callback closes an open list.
                if is_open {
                    if let Some(own) = &clear_open {
                        own.update(cx, |value, cx| {
                            *value = false;
                            cx.notify();
                        });
                    }
                    if let Some(callback) = &clear_open_callback {
                        callback(&false, window, cx);
                    }
                }
            }
            clear_range.update(cx, |range, _| *range = SelectSelectionRange::default());
            if let Some(callback) = &clear_callback {
                callback(window, cx);
            }
        });

        let anchor_bounds: Rc<Cell<Option<gpui::Bounds<Pixels>>>> = window
            .use_keyed_state(element_id::scoped(&self.id, "anchor-bounds"), cx, |_, _| {
                Rc::new(Cell::new(None))
            })
            .read(cx)
            .clone();
        // Whether the pointer went down on the trigger. The panel's
        // outside-press dismissal treats the trigger as outside its own bounds,
        // so a press on the trigger of an *open* list would dismiss it on the
        // mouse-down *and* toggle it through the trigger's own click on the
        // mouse-up -- one press, two contradictory reports, and the list ended
        // up open. The trigger's capture-phase handler runs before the panel's
        // `on_mouse_down_out` in the same dispatch, so the dismissal can see it
        // and leave the close to the trigger's click.
        let trigger_pressed = Rc::new(Cell::new(false));
        // The open list's shared navigation core: the keyed cursor, the
        // typeahead buffer, the arrow wrap, the end-taking page keys, and the
        // scroller the cursor reveals through. `uniform_list` rows page by
        // ends like the panel ones -- the list is not a measured scroller.
        let list_core = crate::list_core::ListCore::new(
            cursor,
            Some(typeahead.clone()),
            self.should_focus_wrap,
            crate::list_core::PageKeys::Ends,
            if self.row_height.is_some() {
                crate::list_core::ListScroll::Uniform {
                    handle: list_scroll_now.clone(),
                    row_height: self.row_height.unwrap_or_default(),
                }
            } else {
                crate::list_core::ListScroll::Panel(panel_scroll_now.clone())
            },
        );
        // The cursor's row index resolves through the same core the keys use:
        // its key locates the row and only an enabled stop answers.
        let cursor_stops: Vec<usize> = (0..self.items.len())
            .filter(|i| !self.disabled_keys.contains(&keys[*i]))
            .collect();
        let cursor_labels: Vec<String> = self
            .items
            .iter()
            .map(|item| item.label().to_string())
            .collect();
        let cursor_rows_view = crate::list_core::ListRows {
            keys: &keys,
            stops: &cursor_stops,
            labels: &cursor_labels,
        };
        // A highlighted row the caller removed repairs to the nearest enabled
        // neighbour before the cursor resolves -- `keys` is the full set, so a
        // disabled row is not treated as removed.
        list_core.repair_removed(&cursor_rows_view, &keys, &previous_rows, cx);
        // An open the trigger armed seats the cursor through the shared core:
        // a selected stop wins, else the trigger's First/Last/None strategy
        // applies. Consumed once, on the first open render.
        if is_open {
            if let Some(strategy) = *open_focus.read(cx) {
                if multiple {
                    list_core.seat_on_open(&cursor_rows_view, &selected_keys, strategy, cx);
                } else {
                    let seat_selected: Vec<SharedString> = selected.clone().into_iter().collect();
                    list_core.seat_on_open(&cursor_rows_view, &seat_selected, strategy, cx);
                }
                open_focus.update(cx, |value, _| *value = None);
            }
        }
        let cursor_at = list_core.cursor_at(&cursor_rows_view, cx);
        SelectFrame {
            is_open,
            open_own,
            overlay_phase,
            dismissal_token,
            resolved_placement,
            entry_placement,
            multiple,
            selected,
            value_own,
            selected_keys,
            indices_own,
            keys,
            focus_handle,
            cursor_at,
            keyboard_press_open,
            open_focus,
            selection_range,
            list_scroll_now,
            panel_scroll_now,
            typeahead,
            list_core,
            blur_scope,
            clear_empty,
            clear_enabled,
            clear_slots,
            clear_hovered,
            clear_selection,
            anchor_bounds,
            trigger_pressed,
        }
    }

    /// Folds the theme's `select` recipe under the builder values.
    fn apply_theme(&mut self, colors: &herogpui_theme::ThemeColors, cx: &App) {
        let select_theme = cx.theme().components.select.resolve(&self.recipes);
        if !self.variant_is_set {
            if let Some(variant) = select_theme.variant {
                self.variant = variant;
            }
        }
        self.field.height = self.field.height.or(select_theme.height);
        self.field.padding_x = self.field.padding_x.or(select_theme.padding_x);
        self.field.padding_y = self.field.padding_y.or(select_theme.padding_y);
        if !self.field.is_bare_is_set {
            if let Some(is_bare) = select_theme.is_bare {
                self.field.is_bare = is_bare;
            }
        }
        self.trigger_text_size = self.trigger_text_size.or(select_theme.trigger_text_size);
        self.row_height = self.row_height.or(select_theme.row_height);
        self.row_padding_x = self.row_padding_x.or(select_theme.row_padding_x);
        self.row_padding_y = self.row_padding_y.or(select_theme.row_padding_y);
        self.row_text_size = self.row_text_size.or(select_theme.row_text_size);
        self.panel_padding = self.panel_padding.or(select_theme.panel_padding);
        self.panel_gap = self.panel_gap.or(select_theme.panel_gap);
        self.panel_min_width = self.panel_min_width.or(select_theme.panel_min_width);
        self.radius = self.radius.or(select_theme.radius);
        if self.row_hover_bg.is_none() {
            self.row_hover_bg = select_theme.row_hover_bg.map(|color| color.resolve(colors));
        }
    }

    /// The trigger box: geometry, field chrome, focus ring and hover fade.
    fn trigger_field(
        &self,
        frame: &SelectFrame,
        colors: &herogpui_theme::ThemeColors,
        layout: &herogpui_theme::LayoutTheme,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let SelectFrame {
            is_open,
            ref focus_handle,
            clear_hovered,
            ..
        } = *frame;
        let sem = *cx.role(Color::Accent);
        // `.select__trigger` is `min-h-9 ... text-sm`.
        let field_box = self.field;
        let (h, text) = (
            field_box.resolved_height(),
            self.trigger_text_size.unwrap_or(util::FIELD_TEXT),
        );

        let trigger_id = element_id::scoped(&self.id, "trigger");
        let trigger_selector = format!("select-trigger-{}", id_debug(&self.id));
        let trigger_radius = util::field_radius(cx);
        let trigger_focused = focus_handle.is_focused(window);
        let mut field = gpui::div()
            .id(trigger_id)
            .debug_selector(move || trigger_selector)
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .when_some(self.font_family.clone(), |field, family| field.font_family(family))
            .min_h(h)
            .when_some(field_box.height, |el, h| el.h(h))
            .px(field_box.resolved_padding_x())
            .when_some(field_box.padding_y, |el, p| el.py(p))
            // HeroUI's trigger reserves `pe-7` for its always-present
            // `.select__indicator`, whether or not a clear button is composed.
            // The indicator is taken out of flex flow below, so the value and
            // clear slots cannot push it or overlap its hit box.
            .relative()
            .pr(px(28.))
            .text_size(text)
            .line_height(px(20.))
            .cursor(util::interactive_cursor(cx));

        let _border_color = if is_open { sem.color } else { colors.separator };
        // `.select__trigger:focus-visible` is `status-focused` -- the offset
        // ring, not a field's flush one, which is why the chrome is not told
        // about the focus here.
        if !field_box.is_bare {
            field = util::apply_field_chrome(field, self.variant, self.is_invalid, false, None, cx);
        }
        if !field_box.is_bare && field_box.focus_ring.unwrap_or(true) && !self.is_disabled {
            // The trigger hosts the ring as an overlay child: concentric with
            // its own `trigger_radius`, where a spread shadow would keep the
            // trigger's radius on a band two pixels further out and blur it.
            field = util::ring_overlay_if_focused(
                field,
                focus_handle,
                true,
                trigger_radius,
                Vec::new(),
                window,
                cx,
            );
        }

        if !field_box.is_bare && (self.is_invalid || (trigger_focused && util::focus_visible(cx))) {
            field = field.bg(match self.variant {
                FieldVariant::Primary => colors.field.focus(),
                FieldVariant::Secondary => colors.default.color,
            });
        }

        if self.is_disabled {
            field = field.opacity(layout.disabled_opacity);
        } else if !field_box.is_bare && !self.is_invalid && !trigger_focused {
            let idle_bg = match self.variant {
                FieldVariant::Primary => colors.field.background,
                FieldVariant::Secondary => colors.default.color,
            };
            let hover_bg = self.trigger_hover_bg.unwrap_or(match self.variant {
                FieldVariant::Primary => colors.field.hover(),
                // `.select--secondary` hovers `--select-trigger-bg-hover: var(--default-hover)`.
                FieldVariant::Secondary => colors.default.hover(),
            });
            let hover_border = colors.field.border_hover();
            // Keep clear-button ownership and trigger focus stable while the
            // field surface eases over HeroUI's 150ms ease-smooth transition.
            // The nested clear affordance suppresses the trigger endpoint.
            field = crate::anim::hover_fade_with_duration_and_easing_suppressed(
                field,
                element_id::scoped(&self.id, "trigger-hover-fade"),
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

        if self.full_width {
            field = field.w_full();
        }
        field
    }

    /// Focus tracking and the trigger's key handler.
    fn trigger_keys(
        &self,
        field: gpui::Stateful<gpui::Div>,
        frame: &SelectFrame,
    ) -> gpui::Stateful<gpui::Div> {
        let SelectFrame {
            is_open,
            ref open_own,
            multiple,
            ref selected,
            ref value_own,
            ref selected_keys,
            ref indices_own,
            ref keys,
            ref focus_handle,
            ref keyboard_press_open,
            ref open_focus,
            ref selection_range,
            ref typeahead,
            ref list_core,
            clear_enabled,
            ref clear_selection,
            ..
        } = *frame;
        let stops: Vec<usize> = (0..self.items.len())
            .filter(|i| !self.disabled_keys.contains(&keys[*i]))
            .collect();
        // The full key list the range is resolved against -- disabled
        // keys keep their positions so range spans stay indexable, while
        // `stops` keeps their insertions out of the range.
        let collection: Vec<SharedString> = keys.clone();
        let selectable: Vec<SharedString> = stops.iter().map(|i| keys[*i].clone()).collect();
        let labels: Vec<String> = self
            .items
            .iter()
            .map(|item| item.label().to_string())
            .collect();
        let typed = typeahead.clone();
        let core = list_core.clone();
        let open_own_keys = open_own.clone();
        let value_own_keys = value_own.clone();
        let indices_own_keys = indices_own.clone();
        let selected_held = selected.clone();
        let selected_keys_held = selected_keys.clone();
        let form_state_keys = self.form_state.clone();
        let on_open_change = self.on_open_change.clone();
        let on_select = self.on_selection_change.clone();
        let on_select_all = self.on_selection_change_all.clone();
        let range_keys = selection_range.clone();
        let was_open = is_open;
        let fh = focus_handle.clone();
        let press_open = keyboard_press_open.clone();
        let open_focus_keys = open_focus.clone();
        let row_keys = keys.clone();
        let clear_keys = clear_selection.clone();
        let key_focus = focus_handle.clone();
        let handler = SelectTriggerKeys {
            stops,
            collection,
            selectable,
            core,
            labels,
            typed,
            open_own_keys,
            value_own_keys,
            indices_own_keys,
            selected_held,
            selected_keys_held,
            form_state_keys,
            on_open_change,
            on_select,
            on_select_all,
            range_keys,
            was_open,
            press_open,
            open_focus: open_focus_keys,
            row_keys,
            clear_keys,
            key_focus,
            clear_enabled,
            multiple,
        };
        field
            .track_focus(focus_handle)
            .key_context("Select")
            .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                window.focus(&fh, cx);
            })
            .on_key_down(move |event, window, cx| handler.on_key_down(event, window, cx))
    }

    /// The trigger's value text and the slot that draws it.
    fn trigger_value(
        &self,
        frame: &SelectFrame,
        colors: &herogpui_theme::ThemeColors,
    ) -> (SharedString, gpui::AnyElement) {
        let SelectFrame {
            multiple,
            ref selected,
            ref selected_keys,
            ..
        } = *frame;
        let value_text = if multiple {
            self.value_text_multiple(selected_keys)
        } else {
            self.value_text_single(selected)
        };
        let chosen = resolved_keys(&self.items, selected, selected_keys);
        let has_value = !chosen.is_empty();
        // The chosen rows' labels and positions, walked in collection order so
        // the value slot's `selectedItems` and `selectedIndices` agree.
        let mut chosen_items: Vec<SharedString> = Vec::with_capacity(chosen.len());
        let mut chosen_at: Vec<usize> = Vec::with_capacity(chosen.len());
        for (at, item) in self.items.iter().enumerate() {
            if chosen.contains(item.key()) {
                chosen_items.push(item.label().clone());
                chosen_at.push(at);
            }
        }

        // What the trigger draws when the caller does not: v3's
        // `defaultChildren`, which a `Select.Value` closure can hand straight
        // back for the placeholder case.
        let default_children = gpui::div()
            // HeroUI's `.select__value` uses `wrap-break-word`: a long
            // selected label grows the trigger instead of disappearing into
            // an ellipsis. `min_w_0` keeps the value slot from forcing the
            // chevron/clear affordance out of the trigger.
            .flex_1()
            .min_w_0()
            .whitespace_normal()
            .text_color(if has_value {
                colors.foreground
            } else {
                colors.muted
            })
            .child(value_text.to_string())
            .into_any_element();

        // `Select.Value` — a caller-drawn value replaces the trigger's text.
        let value_slot = match &self.value_content {
            Some(render) => {
                let names = chosen_items
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                let text = format_selected_names(&names);
                gpui::div()
                    .flex_1()
                    .min_w_0()
                    .child(render(util::SelectionValue {
                        selected_items: &chosen_items,
                        selected_indices: &chosen_at,
                        selected_keys: Some(&chosen),
                        selected_text: &text,
                        is_placeholder: !has_value,
                        default_children,
                    }))
                    .into_any_element()
            }
            None => default_children,
        };
        (value_text, value_slot)
    }

    /// The composed `Select.ClearButton`s, each a stable pointer target over
    /// an animated visual.
    fn clear_buttons(
        &mut self,
        mut field: gpui::Stateful<gpui::Div>,
        frame: &SelectFrame,
        colors: &herogpui_theme::ThemeColors,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let SelectFrame {
            ref focus_handle,
            clear_empty,
            clear_enabled,
            ref clear_slots,
            ref clear_selection,
            ..
        } = *frame;
        for (index, (button, slot)) in std::mem::take(&mut self.clear_buttons)
            .into_iter()
            .zip(clear_slots)
            .enumerate()
        {
            let id = element_id::scoped(&self.id, format!("clear-button-{index}"));
            let pressed = clear_enabled && slot.read(cx).1;
            let hovered = clear_enabled && slot.read(cx).0;
            let scale = if pressed { 0.93 } else { 1. };
            let mut visual = gpui::div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(20. * scale))
                .rounded(px(f32::from(util::small_radius(cx)) * scale))
                .when(hovered, |el| el.bg(colors.default.hover()));
            visual = if button.children.is_empty() {
                visual.child(
                    gpui::svg()
                        .w(px(12. * scale))
                        .h(px(14. * scale))
                        .path(icons::CLOSE)
                        .text_color(colors.muted),
                )
            } else {
                visual.children(button.children)
            };
            let mut opacity = crate::anim::Tween::keyed(
                &id,
                "opacity",
                if clear_empty { 0. } else { 1. },
                window,
                cx,
            );
            // Upstream hides immediately, but fades in when selection returns.
            let reduced = ActiveTheme::reduce_motion(cx);
            opacity.snap_if_reduced(reduced || clear_empty);
            let visual = if opacity.animates(reduced) {
                let from = opacity.from();
                let value = opacity.value();
                visual
                    .with_animation(
                        element_id::scoped(&id, format!("opacity-{}", opacity.generation())),
                        gpui::Animation::new(std::time::Duration::from_millis(150))
                            .with_easing(crate::anim::ease_smooth()),
                        move |el, progress| {
                            let alpha = from + (1. - from) * progress;
                            value.set(alpha);
                            el.opacity(alpha)
                        },
                    )
                    .into_any_element()
            } else {
                opacity.settle();
                visual.opacity(opacity.target()).into_any_element()
            };
            // `.select__clear-button`: keep its 20px layout slot and
            // 24px pointer target independent.
            // Only the target owns listeners; the animated visual never changes
            // their element-id path during a press or an opacity transition.
            // `&:active, &[data-pressed="true"]` applies
            // `transform: scale(0.93)` to the whole control about its center,
            // and CSS transforms rescale hit testing without touching layout,
            // so the absolute target tracks the same centered box — 24px at
            // rest, 22.32px inset by 1.16px while the press lasts.
            let target_size = px(24. * scale);
            let target_inset = px((20. - 24. * scale) / 2.);
            let mut target = gpui::div()
                .id(id.clone())
                .absolute()
                .left(target_inset)
                .top(target_inset)
                .size(target_size)
                .flex()
                .items_center()
                .justify_center()
                .child(visual)
                .debug_selector(move || format!("select-clear-{index}"));
            if clear_enabled {
                let focus = focus_handle.clone();
                let clear = clear_selection.clone();
                target = util::track_interaction_on_mouse_down(target, slot, move |window, cx| {
                    window.focus(&focus, cx);
                    cx.stop_propagation();
                })
                .cursor(util::interactive_cursor(cx))
                .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(gpui::MouseButton::Middle, |_, _, cx| cx.stop_propagation())
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    window.prevent_default();
                    clear(window, cx);
                    if let Some(callback) = &button.on_click {
                        callback(event, window, cx);
                    }
                });
            }
            field = field.child(
                gpui::div()
                    .relative()
                    .size(px(20.))
                    .flex_shrink_0()
                    .child(target),
            );
        }
        field
    }

    /// The absolute `.select__indicator` end slot.
    fn trigger_indicator_slot(
        &mut self,
        mut field: gpui::Stateful<gpui::Div>,
        frame: &SelectFrame,
        colors: &herogpui_theme::ThemeColors,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let is_open = frame.is_open;
        // `.select__indicator`: clear compositions reserve its absolute end
        // slot. HeroUI keeps one down-chevron in the tree and rotates it over
        // 150ms; do the same for the built-in SVG. A caller-provided trigger
        // indicator receives the live open state and owns its own pixels.
        let indicator = match self.trigger_indicator.take() {
            Some(render) => render(is_open),
            None => crate::anim::rotating_indicator_with_duration(
                &element_id::scoped(&self.id, "trigger-indicator"),
                is_open,
                gpui::svg()
                    .size(px(16.))
                    .path(icons::CHEVRON_DOWN)
                    .text_color(colors.muted)
                    .flex_shrink_0(),
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
                .w(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .child(indicator),
        );
        field
    }

    /// The trigger's press: toggles the popover and reports the change.
    fn trigger_toggle(
        &self,
        mut field: gpui::Stateful<gpui::Div>,
        frame: &SelectFrame,
    ) -> gpui::Stateful<gpui::Div> {
        let SelectFrame {
            is_open,
            ref open_own,
            multiple,
            ref keyboard_press_open,
            ref open_focus,
            ref trigger_pressed,
            ..
        } = *frame;
        if !self.is_disabled && (self.on_open_change.is_some() || open_own.is_some()) {
            let on_open_change = self.on_open_change.clone();
            let own = open_own.clone();
            let open = is_open;
            let pressed = trigger_pressed.clone();
            let keyboard_press_open = keyboard_press_open.clone();
            let open_focus = open_focus.clone();
            field = field
                .capture_any_mouse_down(move |_, _, cx| {
                    pressed.set(true);
                    let pressed = pressed.clone();
                    cx.defer(move |_| pressed.set(false));
                })
                .on_click(move |event, window, cx| {
                    // Enter and Space activate the highlighted option before
                    // gpui synthesizes this trigger click. Multiple selection
                    // keeps the popover open for the next pick.
                    let keyboard = matches!(event, gpui::ClickEvent::Keyboard(_));
                    let press_open = keyboard_press_open.update(cx, |value, _| value.take());
                    let started_open = if keyboard {
                        press_open.unwrap_or(open)
                    } else {
                        open
                    };
                    if multiple && started_open && keyboard {
                        return;
                    }
                    // A selection callback can close a controlled popup and
                    // redraw between key-down and key-up. Finish that press's
                    // close rather than toggling the new frame back open.
                    let next_open = !started_open;
                    if next_open == open {
                        return;
                    }
                    // An opening press arms the open strategy the next render's
                    // `seat_on_open` consumes: a keyboard activation (Enter or
                    // Space) opens onto the first row, a pointer press leaves
                    // the seat to a selected row or none.
                    if next_open {
                        open_focus.update(cx, |focus, _| {
                            *focus = Some(if keyboard {
                                crate::list_core::OpenFocus::First
                            } else {
                                crate::list_core::OpenFocus::None
                            });
                        });
                    }
                    // Uncontrolled: flip our own copy, or the trigger would be
                    // inert without a caller handler.
                    if let Some(held) = &own {
                        held.update(cx, |v, cx| {
                            *v = next_open;
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &on_open_change {
                        cb(&next_open, window, cx);
                    }
                    // A pointer press the trigger consumed must not also
                    // activate whatever encloses it: gpui bubbles a click to
                    // every ancestor listener, so a select embedded in a
                    // clickable row -- a menu row hosting an inline select,
                    // a pressable card -- opened its popup and fired the
                    // enclosing handler from one press, which typically
                    // dismissed the surface the popup had just opened over.
                    // Only the pointer is stopped: a keyboard activation is
                    // synthesized on the focused element, so there is no
                    // ancestor press to suppress, and stopping it would take
                    // the key away from a parent's own bindings.
                    if !keyboard {
                        cx.stop_propagation();
                    }
                });
        }
        field
    }

    /// The root: the trigger alone, or wrapped with its label and description.
    fn field_root(&self, field: impl IntoElement) -> gpui::Div {
        // listbox panel
        let mut root = gpui::div().relative();
        root = if self.full_width {
            root.w_full()
        } else {
            root.max_w(px(320.))
        };
        if self.label.is_some() || self.description.is_some() {
            let mut wrapper = gpui::div().flex().flex_col().gap(px(4.)).w_full();
            // Reuse the shared field slots so the required/invalid/disabled
            // treatments match every other control.
            if let Some(label) = &self.label {
                wrapper = wrapper.child(
                    crate::field::Label::new(label.clone())
                        .is_required(self.is_required)
                        .is_disabled(self.is_disabled)
                        .is_invalid(self.is_invalid),
                );
            }
            wrapper = wrapper.child(field);
            if !self.is_invalid {
                if let Some(desc) = &self.description {
                    wrapper = wrapper.child(crate::field::Description::new(desc.clone()));
                }
            }
            root = root.child(wrapper);
        } else {
            root = root.child(field);
        }
        root
    }

    /// Escape, and — for an empty list — the outside press.
    fn root_dismissals(&self, mut root: gpui::Div, frame: &SelectFrame) -> gpui::Div {
        let SelectFrame {
            ref open_own,
            ref dismissal_token,
            ..
        } = *frame;
        let overlay_active = frame.overlay_phase != util::OverlayPhase::Closed;
        let escape_own = open_own.clone();
        let escape_cb = self.on_open_change.clone();
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
                util::DismissResult::Handled
            });

        if overlay_active && self.items.is_empty() {
            let dismiss_own = open_own.clone();
            let dismiss_cb = self.on_open_change.clone();
            root = util::dismiss_on_press_outside_with_token(
                root,
                dismissal_token.clone(),
                move |window, cx| {
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
                },
            );
        }
        root
    }

    /// The popover's scrolling `ListBox` surface: `bg-overlay`, the panel
    /// radius, the dark-mode hairline and the overlay shadow.
    fn panel_surface(
        &self,
        base: &str,
        base_id: &gpui::ElementId,
        radius: Pixels,
        panel_scroll_now: &gpui::ScrollHandle,
        colors: &herogpui_theme::ThemeColors,
        layout: &herogpui_theme::LayoutTheme,
    ) -> gpui::Stateful<gpui::Div> {
        // `useListBox` is named through `useField`, i.e. by the same
        // `<Label>` the trigger points at.
        let list_name = self.aria_label.clone().or(self.label.clone());
        gpui::div()
                .w_full()
                .when_some(self.panel_min_width, |panel, width| panel.min_w(width))
                .flex()
                .flex_col()
                .p(self.panel_padding.unwrap_or(px(6.)))
                .gap(self.panel_gap.unwrap_or(px(0.)))
                .bg(colors.overlay.background)
                .rounded(radius)
                // v3 gives a floating panel no border: `.popover` and friends are
                // `bg-overlay shadow-overlay` and a radius, and dark mode's
                // inset hairline is what separates the panel from the page.
                .when_some(layout.overlay_hairline, |el, hairline| {
                el.border(layout.border_width).border_color(hairline)
                })
                .shadow(layout.overlay_shadow.clone())
                // `.select__popover` is `overflow-y-auto`: a long list scrolls
                // rather than being clipped. gpui needs an id for that.
                .id(element_id::scoped(base_id, "scroll"))
                // RAC's `Select` puts a `ListBox` inside the popover, and
                // `react-aria/dist/private/listbox/useListBox.mjs` is one
                // literal `role: 'listbox'` with `'aria-orientation'`
                // defaulting to vertical. The scroller *is* that list here:
                // the rows are its children. `aria-multiselectable` has no
                // gpui builder (see `crate::a11y`).
                .a11y_named(a11y::Role::ListBox, &a11y::Name::maybe(list_name))
                .a11y_orientation(herogpui_core::Orientation::Vertical)
                .debug_selector({
                    let base = base.to_owned();
                    move || format!("{base}-panel")
                })
                .overflow_y_scroll()
                // `overscroll-contain` upstream: a wheel over the popup must
                // not scroll the page behind it (ColorPicker pattern).
                .occlude()
                .track_scroll(panel_scroll_now)
                // RAC caps the popover at the available viewport height
                // (`calculatePosition`'s `getMaxHeight`); the positioner
                // below re-lays the panel out with that cap, so the panel
                // carries a viewport-relative bound rather than a fixed one.
                .max_h_full()
    }

    /// The listbox popover: panel surface, dismissal, rows and motion.
    fn popover(
        &mut self,
        frame: SelectFrame,
        colors: &herogpui_theme::ThemeColors,
        layout: &herogpui_theme::LayoutTheme,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Deferred {
        let SelectFrame {
            open_own,
            overlay_phase,
            dismissal_token,
            resolved_placement,
            entry_placement,
            multiple,
            selected,
            value_own,
            selected_keys,
            indices_own,
            keys,
            focus_handle,
            cursor_at,
            selection_range,
            list_scroll_now,
            panel_scroll_now,
            list_core,
            anchor_bounds,
            trigger_pressed,
            ..
        } = frame;
        // The `debug_selector` spellings `pickers_deep.rs` queries on --
        // labels, not ids; the ids beside them derive from `self.id`.
        let base = format!("select-list-{}", id_debug(&self.id));
        let base_id = element_id::scoped(&self.id, "list");
        let options_len = self.items.len();
        let panel_interactive = overlay_phase == util::OverlayPhase::Open;
        // The entry zoom interpolates the panel's own radius, so one
        // binding feeds both the painted shape and the animation.
        let radius = self.radius.unwrap_or_else(|| util::picker_radius(cx));
        let panel = self.panel_surface(&base, &base_id, radius, &panel_scroll_now, colors, layout);

        // React Aria dismisses the list on a press outside it. Escape is
        // already read by the trigger's key handler, so only the press half
        // is added here. A press that started on the trigger is not an
        // outside press: the trigger's own click owns the close (and the
        // click only fires because the down was not stolen as a dismissal).
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

        // `useOption` adds the set position only under virtualization;
        // `row_height` is what turns this list into a windowed one.
        let row_virtualized = self.row_height.is_some();
        let row_disabled_opacity = layout.disabled_opacity;
        // Everything a row reads, owned: `uniform_list`'s callback is
        // `'static` and is called again on every scroll, so it cannot
        // borrow `self` -- and one row builder for both paths is what keeps
        // a virtual list drawing the same row as a short one.
        let items = self.items.clone();
        let sections = self.sections.clone();
        let opt_disabled_keys = self.disabled_keys.clone();
        // The key list the range is resolved against and the enabled
        // keys that may join it, for the Shift-click extension.
        let collection_rows: Vec<SharedString> = keys.clone();
        let selectable_rows: Vec<SharedString> = (0..options_len)
            .filter(|i| !self.disabled_keys.contains(&keys[*i]))
            .map(|i| keys[i].clone())
            .collect();
        let range_rows = selection_range;
        let focus_rows = focus_handle;
        let indicator: Option<Rc<dyn Fn(bool) -> gpui::AnyElement>> =
            self.indicator.take().map(Rc::from);
        #[allow(clippy::type_complexity)]
        let item_leading: Option<
            Rc<dyn Fn(&SharedString, bool) -> Option<gpui::AnyElement>>,
        > = self.item_leading.take().map(Rc::from);
        let on_change_all = self.on_selection_change_all.clone();
        let on_change_one = self.on_selection_change.clone();
        let value_own = value_own;
        let open_own = open_own;
        let form_state_rows = self.form_state.clone();
        let on_close = self.on_open_change.clone();
        let base_row = base;
        let base_row_id = base_id.clone();
        let row_text_size = self.row_text_size.unwrap_or(util::FIELD_TEXT);
        let row_padding_x = self.row_padding_x.unwrap_or(px(10.));
        let row_font_family = self.row_font_family.clone();
        let row_padding_y = self.row_padding_y.unwrap_or(px(6.));
        let rows = SelectRows {
            colors: colors.clone(),
            row_hover_bg: self.row_hover_bg,
            multiple,
            selected,
            selected_keys,
            cursor_at,
            panel_interactive,
            options_len,
            row_virtualized,
            row_disabled_opacity,
            items,
            sections,
            opt_disabled_keys,
            collection_rows,
            selectable_rows,
            range_rows,
            core: list_core,
            focus_rows,
            indicator,
            item_leading,
            item_content: self.item_content.clone(),
            on_change_all,
            on_change_one,
            indices_own,
            value_own,
            open_own,
            form_state_rows,
            on_close,
            base_row,
            base_row_id,
            row_text_size,
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
                let gap = self.panel_gap.unwrap_or(px(0.));
                let padding = self.panel_padding.unwrap_or(px(6.));
                panel = panel.child(
                    gpui::uniform_list(
                        element_id::scoped(&base_id, "rows"),
                        options_len,
                        move |range, _window, cx| {
                            range
                                .map(|i| {
                                    let row = rows.row(i, Some(row_height), _window, cx);
                                    if gap <= px(0.) {
                                        return row;
                                    }
                                    gpui::div()
                                        .h(row_height + gap)
                                        .child(row)
                                        .into_any_element()
                                })
                                .collect::<Vec<_>>()
                        },
                    )
                    .track_scroll(&list_scroll_now)
                    .with_sizing_behavior(gpui::ListSizingBehavior::Infer)
                    .p(padding)
                    .m(-padding)
                    .mb(-padding - gap),
                );
            }
            None => {
                for i in 0..options_len {
                    panel = panel.child(rows.row(i, None, window, cx));
                }
            }
        }

        let (slide_x, slide_y) = crate::popover::placement_entry_offset(entry_placement);
        let zoom = crate::anim::ZoomBox::panel(self.panel_padding.unwrap_or(px(6.)), radius);
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
                crate::anim::Motion::LIST_OUT,
                cx,
            )
        } else {
            crate::anim::entering_zoom(
                panel,
                element_id::scoped(&base_id, "panel"),
                zoom,
                crate::anim::Motion::LIST_IN,
                cx,
            )
        };
        // RAC positions the popover against the trigger with an 8px gap,
        // flips it when the other side has more room, and caps it at the
        // available viewport height past a 12px inset — which `Select`
        // inherits unchanged from `useOverlayPosition`/`Popover`.
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

/// The trigger's key handler, holding everything it reads. The handler is
/// `'static`, so it owns copies of the frame's state rather than borrowing
/// the Select; each branch of the pinned keyboard contract is one method.
struct SelectTriggerKeys {
    stops: Vec<usize>,
    collection: Vec<SharedString>,
    selectable: Vec<SharedString>,
    core: crate::list_core::ListCore,
    labels: Vec<String>,
    typed: gpui::Entity<crate::list_nav::Typeahead>,
    open_own_keys: Option<gpui::Entity<bool>>,
    value_own_keys: Option<gpui::Entity<Option<SharedString>>>,
    indices_own_keys: Option<gpui::Entity<Vec<SharedString>>>,
    selected_held: Option<SharedString>,
    selected_keys_held: Vec<SharedString>,
    form_state_keys: Rc<RefCell<crate::form::LiveFormFieldState>>,
    on_open_change: Option<OnOpenChange>,
    on_select: Option<OnSelectionChange>,
    on_select_all: Option<OnSelectionChangeAll>,
    range_keys: gpui::Entity<SelectSelectionRange>,
    was_open: bool,
    press_open: gpui::Entity<Option<bool>>,
    open_focus: gpui::Entity<Option<crate::list_core::OpenFocus>>,
    row_keys: Vec<SharedString>,
    clear_keys: SelectAction,
    key_focus: gpui::FocusHandle,
    clear_enabled: bool,
    multiple: bool,
}

impl SelectTriggerKeys {
    fn on_key_down(&self, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut App) {
        let Self {
            ref stops,
            ref core,
            ref labels,
            ref indices_own_keys,
            ref selected_keys_held,
            ref form_state_keys,
            ref range_keys,
            was_open,
            ref press_open,
            ref row_keys,
            ref clear_keys,
            ref key_focus,
            clear_enabled,
            multiple,
            ..
        } = *self;
        if !key_focus.is_focused(window) {
            return;
        }
        let key = event.keystroke.key.as_str();
        if !was_open && clear_enabled && matches!(key, "backspace" | "delete") {
            cx.stop_propagation();
            window.prevent_default();
            // This key is consumed before the app root sees it.
            util::set_focus_visible(true, cx);
            clear_keys(window, cx);
            return;
        }
        if matches!(key, "enter" | "space") {
            // The browser's default newline can synthesize another
            // Enter through beforeinput, including while held.
            cx.stop_propagation();
            if event.is_held {
                return;
            }
            press_open.update(cx, |value, _| *value = Some(was_open));
        }
        if !was_open {
            self.on_closed_key(key, window, cx);
            return;
        }
        let modifiers = event.keystroke.modifiers;
        let is_mac = cx.platform_info().is_mac();
        // Pinned React Aria 3.51.0 `useSelectableCollection`
        // answers `Mod+A` with `selectAll` -- multiple mode only,
        // and only while the list is open. Pinned SelectState
        // drops the symbolic `all`: the uncontrolled set becomes
        // every enabled key while `on_selection_change_all` stays
        // silent, a repeat over a complete selection is not a
        // toggle, and a controlled owner's state is not this
        // keystroke's to mutate.
        if key == "a"
            && modifiers.secondary(cx)
            && !modifiers.shift
            && !modifiers.alt
            && if is_mac {
                !modifiers.control
            } else {
                !modifiers.platform
            }
            && multiple
        {
            let all: Vec<SharedString> = stops.iter().map(|i| row_keys[*i].clone()).collect();
            let complete = all.iter().all(|key| selected_keys_held.contains(key));
            if !complete {
                if let Some(held) = &indices_own_keys {
                    form_state_keys.borrow_mut().value = crate::form::FormValue::Keys(all.clone());
                    held.update(cx, |selected, cx| {
                        *selected = all.clone();
                        cx.notify();
                    });
                    range_keys.update(cx, |range, _| {
                        *range = SelectSelectionRange {
                            is_all: true,
                            ..SelectSelectionRange::default()
                        };
                    });
                }
            }
            cx.stop_propagation();
            return;
        }
        // The open list's navigation -- page keys, arrows, Home/End, and
        // typeahead -- is the shared list core's. It moves the cursor and
        // reveals the row; what a move *means* (the Shift range extension)
        // and what activation does stay here.
        let rows = crate::list_core::ListRows {
            keys: row_keys,
            stops,
            labels,
        };
        let from = core.cursor_at(&rows, cx);
        // The pinned registrations install no Home/End handler for an
        // unregistered chord -- Cmd- or Ctrl-bearing on macOS, Alt- or
        // platform-bearing elsewhere -- so in either mode the whole event
        // stays inert: no cursor move, no selection, no preventDefault. A
        // Shift extension also reaches from the cursor, and a null cursor
        // is nothing to reach from, so Shift+Home/End is inert too. Both
        // gates run before the core, which would otherwise seat the move.
        if matches!(key, "home" | "end") {
            if !home_end_registered(modifiers, is_mac) {
                return;
            }
            if modifiers.shift && from.is_none() {
                return;
            }
        }
        match core.key_down(&rows, key, &modifiers, cx) {
            crate::list_core::ListKey::Moved { from, to, typed } => {
                self.moved(to, from, typed, key, modifiers, window, cx);
            }
            crate::list_core::ListKey::Activate(at) => {
                self.activate(at, window, cx);
            }
            crate::list_core::ListKey::Unhandled => {}
        }
    }

    /// A closed trigger: Down and Up open the list; a single select answers
    /// letters where it stands.
    fn on_closed_key(&self, key: &str, window: &mut Window, cx: &mut App) {
        let Self {
            ref labels,
            ref stops,
            ref typed,
            ref open_own_keys,
            ref value_own_keys,
            ref selected_held,
            ref form_state_keys,
            ref on_open_change,
            ref on_select,
            ref open_focus,
            ref row_keys,
            multiple,
            ..
        } = *self;
        // Closed: Down and Up open the list. Enter and Space are
        // *not* handled here -- the trigger has a click listener
        // and gpui fires those on Enter and Space for a focused
        // element, so handling them again would open and close
        // the list in one keystroke.
        if matches!(key, "down" | "up") {
            // React Aria's `useMenuTrigger`: ArrowDown opens onto the first
            // row, ArrowUp onto the last.
            open_focus.update(cx, |focus, _| {
                *focus = Some(if key == "down" {
                    crate::list_core::OpenFocus::First
                } else {
                    crate::list_core::OpenFocus::Last
                });
            });
            if let Some(held) = &open_own_keys {
                held.update(cx, |v, cx| {
                    *v = true;
                    cx.notify();
                });
            }
            if let Some(cb) = &on_open_change {
                cb(&true, window, cx);
            }
            return;
        }
        // A closed select still answers letters in single
        // mode: React Aria picks the matching option where it
        // stands rather than opening the list. A multiple
        // select answers no typeahead on the closed trigger --
        // the closed pick reports through the single-key
        // callback, which a set-valued selection has no use
        // for. Open, the RAC ListBox keeps its type-select and
        // moves the cursor without selecting, exactly as in
        // single mode.
        if multiple {
            return;
        }
        if !crate::list_nav::is_typeahead_key(key) {
            return;
        }
        let now = web_time::Instant::now();
        let (query, repeat) = typed.update(cx, |t, _| {
            let query = t.push(key, now);
            (query, t.is_repeat())
        });
        let selected_at = selected_held
            .as_ref()
            .and_then(|k| row_keys.iter().position(|key| key == k));
        let Some(found) = crate::list_nav::typeahead(labels, stops, selected_at, &query, repeat)
        else {
            return;
        };
        let found = row_keys[found].clone();
        if let Some(held) = &value_own_keys {
            form_state_keys.borrow_mut().value = crate::form::FormValue::Keys(vec![found.clone()]);
            held.update(cx, |v, cx| {
                *v = Some(found.clone());
                cx.notify();
            });
        }
        if let Some(cb) = &on_select {
            cb(&Some(found), window, cx);
        }
    }

    /// A cursor move the core already applied -- an arrow, Home/End, a page
    /// key, or a typeahead match. In multiple mode a registered Shift chord
    /// extends the selection over the range from the anchor to the row the
    /// cursor landed on; the move itself selected nothing.
    #[allow(clippy::too_many_arguments)] // the core's `Moved` fields plus the key and its modifiers
    fn moved(
        &self,
        to: usize,
        from: Option<usize>,
        typed: bool,
        key: &str,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Self {
            ref collection,
            ref selectable,
            ref indices_own_keys,
            ref selected_keys_held,
            ref form_state_keys,
            ref on_select_all,
            ref range_keys,
            ref row_keys,
            multiple,
            ..
        } = *self;
        let is_mac = cx.platform_info().is_mac();
        let next = row_keys[to].clone();
        // Pinned `useSelectableCollection`: Shift extends a
        // multiple selection over exactly the chords the
        // platform's registration admits, so the extension
        // gate reuses the Home/End registration map -- the
        // pinned matcher reads the browser's canonical
        // modifier flags only, and GPUI's `function` flag
        // never reaches it. The pinned arrow delegates
        // return null at an enabled boundary, so a
        // Shift+Arrow that held ran no extension at all
        // and must not report; Home and End resolve their
        // end key again, so their repeated registered
        // extension does report; and the page keys only
        // reach here off the unchanged filter above.
        let exact_shift_navigation = home_end_registered(modifiers, is_mac);
        // A typeahead match is `useTypeSelect`'s `setFocusedKey`: it moves the
        // cursor but never extends the selection, however Shift is held.
        let extends_selection = !typed
            && multiple
            && modifiers.shift
            && exact_shift_navigation
            && shift_home_end_extends(key, modifiers.control, is_mac)
            && (matches!(key, "home" | "end") || Some(to) != from);
        if extends_selection {
            let range = range_keys.read(cx).clone();
            let next_selection =
                extend_selection_range(selected_keys_held, collection, selectable, &range, &next);
            range_keys.update(cx, |range, _| {
                if range.anchor.is_none() {
                    range.anchor = Some(next.clone());
                }
                range.current = Some(next);
                range.is_all = false;
            });
            // The uncontrolled set and the form only move
            // when the extension actually changed something,
            // but pinned `useMultipleSelectionState` with
            // Select's `allowDuplicateSelectionEvents`
            // reports every extension it is handed -- so a
            // repeated registered Shift+Home/End that
            // resolved the end already held still reports.
            if next_selection != *selected_keys_held {
                if let Some(held) = &indices_own_keys {
                    form_state_keys.borrow_mut().value =
                        crate::form::FormValue::Keys(next_selection.clone());
                    held.update(cx, |selected, cx| {
                        *selected = next_selection.clone();
                        cx.notify();
                    });
                }
            }
            if let Some(cb) = &on_select_all {
                cb(&next_selection, window, cx);
            }
        }
    }

    /// Enter or Space on the cursor row: single mode selects the key and
    /// reports it; multiple mode toggles it in the set and reports the set.
    /// The trigger click owns closing on key-up.
    fn activate(&self, at: Option<usize>, window: &mut Window, cx: &mut App) {
        let Self {
            ref value_own_keys,
            ref indices_own_keys,
            ref selected_keys_held,
            ref form_state_keys,
            ref on_select,
            ref on_select_all,
            ref range_keys,
            ref row_keys,
            multiple,
            ..
        } = *self;
        let Some(at) = at else { return };
        let key = &row_keys[at];
        if multiple {
            let added = !selected_keys_held.contains(key);
            let mut next = selected_keys_held.clone();
            toggle_key(&mut next, key);
            let next = in_collection_order(&next, row_keys);
            // Pinned `toggleSelection` re-anchors on the
            // add, and a deselect only ends a raw `all`
            // so the next Shift move extends instead of
            // collapsing to its target.
            if added {
                range_keys.update(cx, |range, _| {
                    range.anchor = Some(key.clone());
                    range.current = Some(key.clone());
                    range.is_all = false;
                });
            } else {
                range_keys.update(cx, |range, _| {
                    if range.is_all {
                        *range = SelectSelectionRange::default();
                    }
                });
            }
            if let Some(held) = &indices_own_keys {
                form_state_keys.borrow_mut().value = crate::form::FormValue::Keys(next.clone());
                held.update(cx, |selected, cx| {
                    *selected = next.clone();
                    cx.notify();
                });
            }
            if let Some(cb) = &on_select_all {
                cb(&next, window, cx);
            }
            return;
        }
        if let Some(held) = &value_own_keys {
            form_state_keys.borrow_mut().value = crate::form::FormValue::Keys(vec![key.clone()]);
            held.update(cx, |v, cx| {
                *v = Some(key.clone());
                cx.notify();
            });
        }
        if let Some(cb) = &on_select {
            cb(&Some(key.clone()), window, cx);
        }
    }
}

/// Everything an option row reads, owned: `uniform_list`'s callback is
/// `'static` and is called again on every scroll, so it cannot borrow the
/// Select -- and one row builder for both paths is what keeps a virtual list
/// drawing the same row as a short one.
struct SelectRows {
    /// The theme tokens the row draws with, copied out: `cx.colors()` hands
    /// back a borrow of the app, which a `'static` closure cannot hold.
    colors: herogpui_theme::ThemeColors,
    row_hover_bg: Option<gpui::Hsla>,
    multiple: bool,
    selected: Option<SharedString>,
    selected_keys: Vec<SharedString>,
    cursor_at: Option<usize>,
    panel_interactive: bool,
    options_len: usize,
    row_virtualized: bool,
    row_disabled_opacity: f32,
    items: Vec<PickerItem>,
    sections: Vec<(SharedString, SharedString)>,
    opt_disabled_keys: HashSet<SharedString>,
    collection_rows: Vec<SharedString>,
    selectable_rows: Vec<SharedString>,
    range_rows: gpui::Entity<SelectSelectionRange>,
    core: crate::list_core::ListCore,
    focus_rows: gpui::FocusHandle,
    indicator: Option<Rc<dyn Fn(bool) -> gpui::AnyElement>>,
    #[allow(clippy::type_complexity)]
    item_leading: Option<Rc<dyn Fn(&SharedString, bool) -> Option<gpui::AnyElement>>>,
    item_content: Option<
        std::sync::Arc<dyn Fn(&SharedString, bool, &mut Window, &mut App) -> gpui::AnyElement>,
    >,
    on_change_all: Option<OnSelectionChangeAll>,
    on_change_one: Option<OnSelectionChange>,
    indices_own: Option<gpui::Entity<Vec<SharedString>>>,
    value_own: Option<gpui::Entity<Option<SharedString>>>,
    open_own: Option<gpui::Entity<bool>>,
    form_state_rows: Rc<RefCell<crate::form::LiveFormFieldState>>,
    on_close: Option<OnOpenChange>,
    base_row: String,
    base_row_id: gpui::ElementId,
    row_text_size: Pixels,
    row_padding_x: Pixels,
    row_font_family: Option<SharedString>,
    row_padding_y: Pixels,
}

impl SelectRows {
    /// One option row, with its section header when one precedes it.
    fn row(
        &self,
        i: usize,
        fixed_h: Option<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::AnyElement {
        let Self {
            ref colors,
            multiple,
            ref selected,
            ref selected_keys,
            cursor_at,
            panel_interactive,
            options_len,
            row_virtualized,
            row_disabled_opacity,
            ref items,
            ref sections,
            ref opt_disabled_keys,
            ref focus_rows,
            ref indicator,
            ref item_leading,
            ref item_content,
            ref base_row,
            ref base_row_id,
            row_text_size,
            row_padding_x,
            ref row_font_family,
            row_padding_y,
            ..
        } = *self;
        let row_muted = colors.muted;
        let row_fg = colors.foreground;
        let row_hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);
        let base = &base_row;
        let base_id = &base_row_id;
        let opt = &items[i];
        let row_key = opt.key().clone();
        let focus_click = focus_rows.clone();
        let mut rows = Vec::new();
        // `ListBox.Section`'s `Header`: `text-xs` in the muted colour,
        // above the option it introduces.
        if let Some((_, label)) = sections.iter().find(|(at, _)| at == &row_key) {
            rows.push(
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
        let is_sel = if multiple {
            selected_keys.contains(&row_key)
        } else {
            selected.as_ref() == Some(&row_key)
        };
        let has_indicator_slot = indicator.is_some() || is_sel;
        let opt_disabled = opt_disabled_keys.contains(&row_key);
        let row_selector = format!("{base}-opt-{i}");
        let mut item = gpui::div()
                        .id(element_id::indexed(base_id, "opt", i))
                        // `useOption.mjs`: `role: 'option'` with
                        // `'aria-selected': selectionMode !== 'none' ?
                        // isSelected : undefined`. A Select's list always
                        // selects, so the flag is unconditional here. Its
                        // `aria-posinset`/`aria-setsize` pair is set only
                        // `if (isVirtualized)`, which is exactly this port's
                        // `row_height` path.
                        .debug_selector(move || row_selector)
                        .flex()
                        .items_center()
                        .justify_between()
                        // Every menu row in v3 is a `.list-box-item`: `min-h-9
                        // rounded-2xl px-2 py-1.5 gap-3` at `text-sm`. A
                        // `row_height` list is virtualized, and its rows are
                        // exactly that tall -- the 36px floor would otherwise
                        // overflow any shorter row it was asked for.
                        .map(|item| match fixed_h {
                            Some(row_h) => item.h(row_h),
                            None => item.min_h(util::FIELD_HEIGHT),
                        })
                        .rounded(util::soft_radius(cx))
                        .px(row_padding_x)
                        .py(row_padding_y)
                        .gap(px(12.))
                        .text_size(row_text_size)
                        .line_height(px(20.))
                        // HeroUI's list-box item reserves `pe-7` whenever its
                        // indicator slot is present; the indicator itself is
                        // absolute at the inline end. Keeping it out of flex
                        // flow prevents long labels from pushing the checkmark.
                        .relative()
                        .when(has_indicator_slot, |row| row.pr(px(28.)));
        if let Some(family) = row_font_family.clone() {
            item = item.font_family(family);
        }

        if opt_disabled {
            item = item.opacity(row_disabled_opacity);
        } else if panel_interactive {
            // `useOption.onHoverStart`: pointer hover seats the virtual
            // cursor for the next arrow key — `shouldFocusOnHover` — while
            // a keyboard-active session (`focus_visible`) keeps it. Real
            // focus stays on the trigger; only the cursor moves, and the
            // row ring stays gated on `shows_focus_ring`.
            item = self.core.seat_on_hover(
                item.cursor(util::interactive_cursor(cx))
                    .hover(move |s| s.bg(row_hover_bg)),
                row_key.clone(),
            );
        }

        // `.list-box-item[data-selected]` is indicator-only in v3:
        // selection does not recolour or bold the option label.
        item = item.text_color(row_fg);

        // `status-focused` is an overlay shadow in v3. A border
        // changes the row's content geometry when the cursor moves.
        // The ring goes on `item` rather than on anything the press
        // ramp below produces: `anim::pressed_with_background_ramp`
        // refines this same element, and `item` is the one that both
        // carries `soft_radius` and holds the cursor the ring reports.
        item = util::with_focus_ring_overlay(
            item,
            util::shows_focus_ring(cursor_at == Some(i), cx),
            true,
            util::soft_radius(cx),
            Vec::new(),
            cx,
        );

        // HeroUI's ListBox.Item does not add an ellipsis rule. Keep
        // normal text flow in both natural and virtual rows; the
        // caller owns the fixed row geometry when `row_height` is
        // supplied, just as the upstream Virtualizer owns its
        // `rowHeight` layout.
        // `ListBox.Item`'s render function draws whatever precedes the
        // text. It goes in before the label so the row's `gap-3` sets
        // it off, and it is `flex_none` so a long label cannot squeeze
        // a swatch or avatar out of its own size.
        if let Some(render) = item_content {
            item = item.child(render(&row_key, is_sel, window, cx));
        } else {
            if let Some(render) = &item_leading {
                if let Some(content) = render(&row_key, is_sel) {
                    item = item.child(gpui::div().flex_none().flex().items_center().child(content));
                }
            }
            let label = gpui::div().flex_1().min_w_0().whitespace_normal();
            item = item.child(label.child(opt.label().to_string()));
        }

        match &indicator {
            Some(render) => {
                item = item.child(
                    gpui::div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right(px(8.))
                        .w(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(render(is_sel)),
                );
            }
            None if is_sel => {
                item = item.child(
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
                                        // HeroUI's default list-item indicator is
                                        // `text-default-foreground`, independent of
                                        // the accent selection role.
                                        .text_color(row_fg),
                        ),
                );
            }
            None => {}
        }

        // `.list-box-item:active` scales an option to 98% over the
        // pinned 250ms ease-out-quart transition. Wrap before the
        // selection listener is attached so the stable slot owns the
        // row's hit target and the click remains reachable after the
        // visual skin moves inside it.
        if panel_interactive && !opt_disabled {
            item = crate::anim::pressed_with_background_ramp(
                item,
                crate::anim::PressBox {
                    height: fixed_h.unwrap_or(util::FIELD_HEIGHT),
                    padding_x: Some(row_padding_x),
                    width: None,
                    min_width: None,
                    text_size: row_text_size,
                    line_height: px(20.),
                    gap: px(12.),
                    radius: util::soft_radius(cx),
                    shrink_x: false,
                    scale: crate::anim::PRESSED_SCALE_SUBTLE,
                },
                None,
                crate::anim::LIST_ITEM_PRESS,
                None,
                window,
                cx,
            );
        }

        item = item
                        .a11y_named(
                            a11y::Role::ListBoxOption,
                            &a11y::Name::labelled(opt.label().clone()),
                        )
                        .a11y_selected(is_sel)
                        .when(row_virtualized, |item| {
                            item.a11y_set_position(i, options_len)
                        })
                        // The trigger keeps the real focus while the list is
                        // open, and a cursor walks the rows -- upstream's
                        // `shouldUseVirtualFocus`. gpui states that relation
                        // on the descendant rather than on the container.
                        .when(cursor_at == Some(i), |item| item.a11y_active_descendant());

        if panel_interactive && !opt_disabled {
            item = self.attach_pick(item, row_key, focus_click);
        }

        rows.push(item.into_any_element());
        gpui::div()
                    .flex()
                    .flex_col()
                    // Keep the row wrapper at the panel's inline extent in
                    // both the natural and virtual paths. The stable press
                    // slot below uses `w_full`; without the same constraint
                    // on natural rows, a long label expands the wrapper to
                    // its min-content width and the option stops wrapping.
                    .w_full()
                    .when_some(fixed_h, |el, h| el.h(h))
                    .children(rows)
                    .into_any_element()
    }

    /// The row's pointer pick: a toggle or Shift extension in multiple mode,
    /// a select-and-close in single mode.
    fn attach_pick(
        &self,
        mut item: gpui::Stateful<gpui::Div>,
        row_key: SharedString,
        focus_click: gpui::FocusHandle,
    ) -> gpui::Stateful<gpui::Div> {
        let Self {
            multiple,
            ref selected_keys,
            ref collection_rows,
            ref selectable_rows,
            ref range_rows,
            ref on_change_all,
            ref on_change_one,
            ref indices_own,
            ref value_own,
            ref open_own,
            ref form_state_rows,
            ref on_close,
            ..
        } = *self;
        if multiple {
            if indices_own.is_some() || on_change_all.is_some() {
                let current = selected_keys.clone();
                let own = indices_own.clone();
                let cb = on_change_all.clone();
                let form_state_pick = form_state_rows.clone();
                let range_click = range_rows.clone();
                let collection_click = collection_rows.clone();
                let selectable_click = selectable_rows.clone();
                let click_core = self.core.clone();
                let picked_key = row_key;
                item = item.on_click(move |ev, window, cx| {
                    // A pointer pick must not also activate whatever
                    // encloses the select: gpui bubbles a click to
                    // every ancestor listener, so a select hosted in
                    // a clickable row -- an `is_interactive` menu
                    // row, a pressable card -- fired the enclosing
                    // handler from the same press that picked an
                    // option. Only the pointer is stopped, matching
                    // the trigger: a keyboard activation is
                    // synthesized on the focused element, so there
                    // is no ancestor press to suppress, and stopping
                    // it would take the key away from a parent's own
                    // bindings.
                    if !matches!(ev, gpui::ClickEvent::Keyboard(_)) {
                        cx.stop_propagation();
                    }
                    // Pinned `useSelectableItem` seats the cursor
                    // on pointer press, so a Shift+Arrow, page, or
                    // Enter that follows starts from the clicked
                    // row rather than from a null or stale cursor.
                    click_core.set_cursor(Some(picked_key.clone()), cx);
                    // gpui's own focus-on-press would park focus
                    // on the row and deafen the trigger's key
                    // handler; the trigger is what holds focus so
                    // the open list stays keyboard-walkable.
                    window.focus(&focus_click, cx);
                    let mut next = current.clone();
                    // A Shift click extends from the anchor
                    // through `extendSelection`; an ordinary or
                    // platform-Mod click toggles and re-anchors on
                    // the add, the way pinned `toggleSelection`
                    // does -- a deselect only ends a raw `all`.
                    if ev.modifiers().shift {
                        let range = range_click.read(cx).clone();
                        next = extend_selection_range(
                            &current,
                            &collection_click,
                            &selectable_click,
                            &range,
                            &picked_key,
                        );
                        range_click.update(cx, |range, _| {
                            if range.anchor.is_none() {
                                range.anchor = Some(picked_key.clone());
                            }
                            range.current = Some(picked_key.clone());
                            range.is_all = false;
                        });
                    } else {
                        let added = !next.contains(&picked_key);
                        toggle_key(&mut next, &picked_key);
                        next = in_collection_order(&next, &collection_click);
                        if added {
                            range_click.update(cx, |range, _| {
                                range.anchor = Some(picked_key.clone());
                                range.current = Some(picked_key.clone());
                                range.is_all = false;
                            });
                        } else {
                            range_click.update(cx, |range, _| {
                                if range.is_all {
                                    *range = SelectSelectionRange::default();
                                }
                            });
                        }
                    }
                    if let Some(held) = &own {
                        form_state_pick.borrow_mut().value =
                            crate::form::FormValue::Keys(next.clone());
                        held.update(cx, |selected, cx| {
                            *selected = next.clone();
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &cb {
                        cb(&next, window, cx);
                    }
                });
            }
        } else if on_change_one.is_some() || value_own.is_some() || open_own.is_some() {
            let on_select = on_change_one.clone();
            let on_close = on_close.clone();
            let value_own = value_own.clone();
            let open_own = open_own.clone();
            let form_state_pick = form_state_rows.clone();
            let picked_key = row_key;
            item = item.on_click(move |ev, window, cx| {
                // The same stop as the multi pick above: the pick
                // that closes the list must not fire the clickable
                // ancestor a hosted select sits in. Keyboard stays
                // live for the reason recorded there.
                if !matches!(ev, gpui::ClickEvent::Keyboard(_)) {
                    cx.stop_propagation();
                }
                // Uncontrolled: take the selection and close, or
                // choosing an option would do nothing.
                if let Some(held) = &value_own {
                    form_state_pick.borrow_mut().value =
                        crate::form::FormValue::Keys(vec![picked_key.clone()]);
                    held.update(cx, |v, cx| {
                        *v = Some(picked_key.clone());
                        cx.notify();
                    });
                }
                if let Some(held) = &open_own {
                    held.update(cx, |v, cx| {
                        *v = false;
                        cx.notify();
                    });
                }
                // A single-mode pick closes the popover, and a
                // caller who drives `isOpen` has to hear about it:
                // without this the keyed flag flipped while the
                // callback stayed silent, so a controlled caller
                // still saw the panel open and reopened it on the
                // next render. Reported exactly here by a pointer
                // pick; the Enter path's close is the trigger's
                // own click listener (gpui fires a focused
                // element's click on Enter), so it never reaches
                // this closure.
                if let Some(cb) = &on_close {
                    cb(&false, window, cx);
                }
                if let Some(f) = &on_select {
                    f(&Some(picked_key.clone()), window, cx);
                }
            });
        }
        item
    }
}

fn live_form_state() -> Rc<RefCell<crate::form::LiveFormFieldState>> {
    Rc::new(RefCell::new(crate::form::LiveFormFieldState {
        commit_validation: None,
        reset_validation: None,
        value: crate::form::FormValue::Text(SharedString::default()),
        is_invalid: false,
        is_successful: true,
        focus: None,
        restore: None,
    }))
}

/// The form value both modes submit: the selection's keys, each resolved to a
/// collection item, in row order — the channel `@heroui/autocomplete` submits
/// the same way, and what a keyed collection's hidden input carries upstream.
fn select_form_value(
    items: &[PickerItem],
    single: &Option<SharedString>,
    multiple: &[SharedString],
) -> crate::form::FormValue {
    crate::form::FormValue::Keys(resolved_keys(items, single, multiple))
}

fn sync_select_form(
    state: &Rc<RefCell<crate::form::LiveFormFieldState>>,
    value: crate::form::FormValue,
    is_invalid: bool,
    is_successful: bool,
) {
    let mut state = state.borrow_mut();
    state.value = value;
    state.is_invalid = is_invalid;
    state.is_successful = is_successful;
}

fn id_debug(id: &gpui::ElementId) -> String {
    format!("{id:?}").trim_matches('"').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Home/End gate takes the platform as an explicit bool, so this
    /// truth table is free of `cfg!` and mechanically proves both maps from
    /// any host: no macOS chord ever extends -- Shift and Alt+Shift move the
    /// cursor alone -- while Windows and Linux extend exactly from
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
                "plain Shift+{key} must only move the cursor"
            );
        }
    }

    /// Arrows and page keys never consult the Home/End gate: their forbidden
    /// extra chords are rejected earlier, by `exact_shift_navigation`.
    #[test]
    fn arrows_and_pages_skip_the_home_end_gate() {
        assert!(shift_home_end_extends("down", false, true));
        assert!(shift_home_end_extends("up", true, false));
        assert!(shift_home_end_extends("pagedown", false, false));
        assert!(shift_home_end_extends("pageup", true, true));
    }

    #[test]
    fn home_end_registration_follows_the_platform_maps() {
        let none = gpui::Modifiers::none();
        let mut shift = none;
        shift.shift = true;
        let mut alt = none;
        alt.alt = true;
        let mut alt_shift = alt;
        alt_shift.shift = true;
        let mut control = none;
        control.control = true;
        let mut control_shift = control;
        control_shift.shift = true;
        let mut platform = none;
        platform.platform = true;
        let mut platform_shift = platform;
        platform_shift.shift = true;
        let mut function = none;
        function.function = true;
        let mut function_alt = function;
        function_alt.alt = true;

        for modifiers in [none, shift, alt, alt_shift, function, function_alt] {
            assert!(
                home_end_registered(modifiers, true),
                "macOS must register {modifiers:?}"
            );
        }
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

    #[test]
    fn default_value_text_matches_pinned_select_value() {
        let select = Select::new(
            "select-default-text",
            vec![
                PickerItem::new("alpha", "Alpha"),
                PickerItem::new("beta", "Beta"),
                PickerItem::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .selected_keys(["alpha", "beta", "gamma"].map(SharedString::from));

        assert_eq!(
            select.value_text_multiple(&select.selected_keys),
            "Alpha, Beta, and Gamma"
        );
        assert_eq!(
            format_selected_names(&["Alpha".into(), "Beta".into()]),
            "Alpha and Beta"
        );
        assert_eq!(
            Select::new("select-default-placeholder", Vec::new()).placeholder,
            "Select an item"
        );
    }

    /// The keyed selection channel: keys, not labels, address items, so two
    /// items may share a label without aliasing each other, the reads walk
    /// the collection however the keys were picked or listed, and keys the
    /// collection does not hold resolve to nothing.
    #[test]
    fn keys_address_items_and_reads_walk_the_collection() {
        let keys = |names: &[&str]| -> Vec<SharedString> {
            names.iter().copied().map(SharedString::from).collect()
        };
        let items = vec![
            PickerItem::new("a", "Paris"),
            PickerItem::new("b", "London"),
            PickerItem::new("c", "Paris"),
        ];
        // Same label, distinct keys: each key resolves to its own item.
        let select = Select::new("select-keyed", items).value(Some(SharedString::from("c")));
        assert_eq!(
            select.value_text_single(&select.selected),
            "Paris",
            "the key must resolve to its own item, never a same-label sibling"
        );
        // A multiple selection reports in collection order however it was
        // listed, and an unknown key resolves to nothing.
        assert_eq!(
            in_collection_order(&keys(&["c", "a", "zz"]), &keys(&["a", "b", "c"])),
            keys(&["a", "c"]),
            "reads must walk the collection and drop unresolvable keys"
        );
        assert_eq!(
            resolved_keys(&select.items, &None, &keys(&["c", "b"])),
            keys(&["b", "c"]),
            "the resolved set follows the collection's row order"
        );
        assert_eq!(
            resolved_keys(&select.items, &Some(SharedString::from("gone")), &[]),
            Vec::<SharedString>::new(),
            "a key with no item resolves to no selection"
        );
    }

    // The pinned `.select--secondary` hover fill is
    // `--select-trigger-bg-hover: var(--default-hover)` and the popup rows
    // fill with the full `bg-default`. The two accessors differ by one word
    // and the wrong one still looks plausible on screen, so the check is
    // mechanical.
    #[test]
    fn secondary_trigger_and_menu_rows_use_the_pinned_hover_tokens() {
        // Scan the implementation only; this test's own text names the
        // forbidden accessors.
        let source = include_str!("select.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("FieldVariant::Secondary => colors.default.hover()"),
            "the secondary trigger hover must read `colors.default.hover()` \
             (pinned `--select-trigger-bg-hover: var(--default-hover)`)"
        );
        assert!(
            source
                .contains("let row_hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);"),
            "the popup rows must hover the full `bg-default` \
             (pinned `.list-box-item:hover`)"
        );
        assert!(
            !source.contains("soft_hover()") && !source.contains("default.soft()"),
            "no select surface may hover a soft token"
        );
    }

    #[test]
    fn trigger_hover_uses_the_pinned_smooth_transition() {
        let source = include_str!("select.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("hover_fade_with_duration_and_easing_suppressed")
                && source.contains("Some(150)")
                && source.contains("HoverFadeEasing::EaseSmooth")
                && source.contains("|fill| fill.rounded(trigger_radius)"),
            "Select trigger hover must animate with the pinned 150ms smooth \
             transition and preserve the trigger radius"
        );
        assert!(
            source.contains("!field_box.is_bare && !self.is_invalid && !trigger_focused"),
            "invalid and focused triggers must keep ownership of their chrome"
        );
    }

    #[test]
    fn option_rows_use_the_pinned_subtle_press_transition() {
        let source = include_str!("select.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("crate::anim::pressed_with_background_ramp(")
                && source.contains("crate::anim::LIST_ITEM_PRESS")
                && source.contains("PRESSED_SCALE_SUBTLE")
                && source.contains("radius: util::soft_radius(cx)"),
            "Select options must use the shared 98% quart press ramp and keep their row radius"
        );
    }
}

crate::util::impl_component_styled!(Select);
