//! TagGroup — port of `@heroui/tag-group` (v3).
//!
//! A focusable list of tags with optional selection and removal. Mirrors the
//! React API: `selectionMode`, `selectedKeys`, `disabledKeys`, `isDisabled`,
//! `onRemove`, `onSelectionChange`, `size` and the `default | surface` variant.

use std::collections::HashSet;
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, AnyElement, App, ElementId, InteractiveElement, IntoElement, Pixels,
    RenderOnce, SharedString, Styled, Window,
};
use herogpui_core::{element_id, SelectionMode, Size};
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    icons, EscapeKeyBehavior,
};

/// Visual variant of the tags in a group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TagVariant {
    /// Filled with `default`.
    #[default]
    Default,
    /// Flat on the surface.
    Surface,
}

impl TagVariant {
    /// Every variant, in display order.
    pub const ALL: [TagVariant; 2] = [TagVariant::Default, TagVariant::Surface];

    /// The display name of this variant.
    pub fn label(self) -> &'static str {
        match self {
            TagVariant::Default => "Default",
            TagVariant::Surface => "Surface",
        }
    }
}

/// One tag in a [`TagGroup`].
#[must_use = "builder methods return a new value; pass the tag to its group"]
#[derive(Clone)]
pub struct Tag {
    key: SharedString,
    label: SharedString,
    icon: Option<SharedString>,
    remove_content: Option<Arc<dyn Fn() -> AnyElement + 'static>>,
    is_disabled: bool,
}

impl Tag {
    /// Creates a tag from a key and a label.
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            icon: None,
            remove_content: None,
            is_disabled: false,
        }
    }

    /// Sets the icon asset path shown in the tag.
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    /// `Tag.RemoveButton` children, replacing the default close glyph.
    pub fn remove_content(mut self, render: impl Fn() -> AnyElement + 'static) -> Self {
        self.remove_content = Some(Arc::new(render));
        self
    }

    /// Sets whether the tag is disabled (v3 `isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Returns the tag's key.
    pub fn key(&self) -> &SharedString {
        &self.key
    }
}

type OnSelectionChange = Arc<dyn Fn(&HashSet<SharedString>, &mut Window, &mut App) + 'static>;
type OnRemove = Arc<dyn Fn(&HashSet<SharedString>, &mut Window, &mut App) + 'static>;

/// Pinned React Stately `useMultipleSelectionState`'s anchor record: where a
/// Shift extension reaches from, how far the last one went, and whether the
/// selection is the raw `all` a `selectAll` produced.
#[derive(Clone, Debug, Default)]
struct TagSelectionRange {
    anchor: Option<SharedString>,
    current: Option<SharedString>,
    is_all: bool,
}

impl TagSelectionRange {
    /// Seats the range on `target`: the anchor stays whatever a first
    /// extension chose, the cursor lands on `target`, and a raw `all` ends.
    fn seat(&mut self, target: SharedString) {
        if self.anchor.is_none() {
            self.anchor = Some(target.clone());
        }
        self.current = Some(target);
        self.is_all = false;
    }
}

/// The selection after extending to `target` from `range`'s anchor.
///
/// The old anchor..current range is *replaced* by anchor..target, so extending
/// backwards shrinks again; a raw `all` collapses to the new key; a first
/// extension without an anchor selects from the target itself, which is what
/// the pinned SelectionManager does when nothing anchors it yet. Only
/// `selectable` keys enter the range, so disabled tags are skipped.
fn extend_selection_range(
    current: &HashSet<SharedString>,
    collection: &[SharedString],
    selectable: &HashSet<SharedString>,
    range: &TagSelectionRange,
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

/// HeroUI TagGroup.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct TagGroup {
    id: ElementId,
    tags: Vec<Tag>,
    label: Option<SharedString>,
    description: Option<SharedString>,
    selection_mode: SelectionMode,
    selected_keys: HashSet<SharedString>,
    default_selected_keys: HashSet<SharedString>,
    is_controlled: bool,
    disallow_empty_selection: bool,
    escape_key_behavior: EscapeKeyBehavior,
    disabled_keys: HashSet<SharedString>,
    is_disabled: bool,
    size: Size,
    /// The tag chip's corner radius, in place of the size step's radius. The
    /// remove button inside stays a circle.
    radius: Option<Pixels>,
    variant: TagVariant,
    /// The fill a hovered selectable tag takes, in place of the variant's
    /// hover colour.
    hover_bg: Option<gpui::Hsla>,
    /// The fill a hovered remove button takes, in place of
    /// `--default-hover`.
    remove_hover_bg: Option<gpui::Hsla>,
    /// `Tag`'s `children`-as-a-function: handed the interactive state and drawn
    /// in place of the label.
    tag_content: Option<Arc<dyn Fn(&Tag, crate::util::InteractiveState) -> AnyElement + 'static>>,
    /// Shown in place of the list when `tags` is empty.
    empty_state: Option<SharedString>,
    on_selection_change: Option<OnSelectionChange>,
    on_remove: Option<OnRemove>,
    /// Expands the root to the available width.
    full_width: bool,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl TagGroup {
    /// Creates a tag group from an element id and its tags.
    pub fn new(id: impl Into<ElementId>, tags: Vec<Tag>) -> Self {
        Self {
            id: id.into(),
            tags,
            tag_content: None,
            label: None,
            description: None,
            selection_mode: SelectionMode::None,
            selected_keys: HashSet::new(),
            default_selected_keys: HashSet::new(),
            is_controlled: false,
            disallow_empty_selection: false,
            escape_key_behavior: EscapeKeyBehavior::ClearSelection,
            disabled_keys: HashSet::new(),
            is_disabled: false,
            size: Size::Md,
            radius: None,
            variant: TagVariant::Default,
            hover_bg: None,
            remove_hover_bg: None,
            empty_state: None,
            on_selection_change: None,
            on_remove: None,
            full_width: false,
            sx: None,
        }
    }

    /// Sets the group label.
    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// Sets the group description.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Sets the selection mode (v3 `selectionMode`).
    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// Sets the selected keys (v3 `selectedKeys`), making the selection controlled.
    pub fn selected_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.selected_keys = keys.into_iter().collect();
        self.is_controlled = true;
        self
    }

    /// `defaultSelectedKeys` — seeds the group's own selection state.
    pub fn default_selected_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.default_selected_keys = keys.into_iter().collect();
        self
    }

    /// Prevents selection from becoming empty through a tag toggle or Escape.
    pub fn disallow_empty_selection(mut self, v: bool) -> Self {
        self.disallow_empty_selection = v;
        self
    }

    /// `escapeKeyBehavior` — whether unmodified Escape clears selection.
    pub fn escape_key_behavior(mut self, behavior: EscapeKeyBehavior) -> Self {
        self.escape_key_behavior = behavior;
        self
    }

    /// Sets the keys of tags that are disabled (v3 `disabledKeys`).
    pub fn disabled_keys(mut self, keys: impl IntoIterator<Item = SharedString>) -> Self {
        self.disabled_keys = keys.into_iter().collect();
        self
    }

    /// Sets whether the whole group is disabled.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets the size (v3 `size`).
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The tag chip's corner radius, in place of the size step's radius. The
    /// step (`rounded-xl`, `rounded-2xl` on `Lg`) stays the fallback, and the
    /// remove button inside a chip is an inner part that stays a circle. Not a
    /// v3 prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// v3's render function for a tag's children, handed `isHovered`,
    /// `isPressed`, `isFocused`, `isFocusVisible` and `isSelected` -- and the tag
    /// itself, which the closure needs to know what it is drawing.
    ///
    /// The hover and the press are a frame behind the pointer: gpui reports both
    /// to a handler, not to the render that draws them.
    pub fn tag_content(
        mut self,
        render: impl Fn(&Tag, crate::util::InteractiveState) -> AnyElement + 'static,
    ) -> Self {
        self.tag_content = Some(Arc::new(render));
        self
    }

    /// Sets the visual variant (v3 `variant`).
    pub fn variant(mut self, variant: TagVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The fill a hovered selectable tag takes, in place of the variant's
    /// hover colour. Resting colours (including a selected tag's soft accent)
    /// are unchanged.
    pub fn hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.hover_bg = Some(color.into());
        self
    }

    /// The fill a hovered remove button takes, in place of
    /// `--default-hover`.
    pub fn remove_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.remove_hover_bg = Some(color.into());
        self
    }

    /// `fullWidth` — expands the root to the available width without
    /// redistributing the children.
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the group's root element after every value the size, the
    /// variant and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// `TagGroup.List` renders this when there is nothing to show.
    pub fn empty_state(mut self, text: impl Into<SharedString>) -> Self {
        self.empty_state = Some(text.into());
        self
    }

    /// Sets the handler called with the new set of selected keys (v3 `onSelectionChange`).
    pub fn on_selection_change(
        mut self,
        handler: impl Fn(&HashSet<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Arc::new(handler));
        self
    }

    /// Adds a remove button to every tag. The button reports its own key;
    /// Delete or Backspace reports the whole selection when the focused tag is
    /// selected, and otherwise reports only the focused key.
    pub fn on_remove(
        mut self,
        handler: impl Fn(&HashSet<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Some(Arc::new(handler));
        self
    }

    /// `(px, py, text, leading)` from `.tag--sm` / `--md` / `--lg`.
    ///
    /// v3 gives a tag no height: it is padding around one line, which is why
    /// this returns a vertical padding rather than the box it used to force.
    fn metrics(size: Size) -> (Pixels, Pixels, Pixels, Pixels) {
        match size {
            Size::Sm => (px(8.), px(2.), px(12.), px(16.)),
            Size::Md => (px(8.), px(4.), px(12.), px(16.)),
            Size::Lg => (px(10.), px(6.), px(14.), px(20.)),
        }
    }

    /// `rounded-xl` on `.tag`, `rounded-2xl` on `.tag--lg`. Named for the step
    /// rather than for what it returns, because [`TagGroup::radius`] is the
    /// builder that replaces its value.
    fn step_radius(size: Size, cx: &App) -> Pixels {
        match size {
            Size::Sm | Size::Md => crate::util::small_radius(cx),
            Size::Lg => crate::util::soft_radius(cx),
        }
    }
}

impl RenderOnce for TagGroup {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // A tag group is *one* tab stop: React Aria roves the tabindex, so Tab
        // enters the group once and the arrows move inside it. Which tag claims
        // the handle is held here, because a handle's `tab_stop` is fixed where
        // the handle is made. `use_keyed_state` takes `cx` mutably, so both
        // precede the theme.
        let group_focus =
            crate::util::tab_stop_handle(element_id::scoped(&self.id, "focus"), window, cx);
        let cursor =
            window.use_keyed_state(element_id::scoped(&self.id, "cursor"), cx, |_, _| 0usize);
        // The Shift-range anchor lives beside the cursor, keyed off the same
        // instance id so two groups never share an anchor.
        let selection_range =
            window.use_keyed_state(element_id::scoped(&self.id, "range"), cx, |_, _| {
                TagSelectionRange::default()
            });
        let (selected_keys, selection_own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "selected"),
            self.is_controlled.then(|| self.selected_keys.clone()),
            self.default_selected_keys.clone(),
        );
        self.selected_keys = selected_keys;
        // Removing a tag shortens the list and disabled tags take no focus, so
        // the stop lands on the first enabled tag at or after the cursor.
        let enabled: Vec<usize> = self
            .tags
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !(self.is_disabled || t.is_disabled || self.disabled_keys.contains(&t.key))
            })
            .map(|(i, _)| i)
            .collect();
        let at = *cursor.read(cx);
        let cursor_index = enabled
            .iter()
            .copied()
            .find(|i| *i >= at)
            .or_else(|| enabled.first().copied());
        let enabled_keys: HashSet<SharedString> = enabled
            .iter()
            .map(|index| self.tags[*index].key.clone())
            .collect();
        // The collection order the range is resolved against — every tag's
        // key, including disabled ones: disabled keys keep their collection
        // positions so range span traversal preserves indexes, while the
        // `selectable` filter keeps their insertions out of the range.
        let collection_keys: Vec<SharedString> =
            self.tags.iter().map(|tag| tag.key.clone()).collect();
        let owns_focus = window.is_window_active() && group_focus.is_focused(window);
        // One hover/press slot per tag. The slot is also the source for the
        // pinned 100ms background transition, so plain tags and tags with a
        // `tag_content` render prop take exactly the same hover path. Keeping
        // the slot on the stable element lets the animated fill change its id
        // without dropping the interaction listener or focus state.
        let interaction: Vec<crate::util::Interaction> = (0..self.tags.len())
            .map(|index| {
                crate::util::interaction(
                    element_id::scoped(&element_id::indexed(&self.id, "tag", index), "interaction"),
                    window,
                    cx,
                )
            })
            .collect();
        for (index, slot) in interaction.iter().enumerate() {
            let tag = &self.tags[index];
            if (self.is_disabled || tag.is_disabled || self.disabled_keys.contains(&tag.key))
                && *slot.read(cx) != (false, false)
            {
                slot.update(cx, |state, _| *state = (false, false));
            }
        }
        let remove_focus_handles = if self.on_remove.is_some() {
            self.tags
                .iter()
                .map(|tag| {
                    crate::util::tab_stop_handle(
                        element_id::scoped(
                            &element_id::scoped(
                                &element_id::scoped(&self.id, "tag"),
                                tag.key.clone(),
                            ),
                            "remove-focus",
                        ),
                        window,
                        cx,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        let ring_visible = crate::util::focus_visible(cx);
        // The hover animation mutably borrows `cx` while it registers keyed
        // state. Keep a snapshot of the semantic tokens so that borrow does
        // not span that call; theme values are immutable for this render.
        let colors = cx.colors().clone();
        let disabled_opacity = cx.layout().disabled_opacity;
        let (pad_x, pad_y, text_size, leading) = Self::metrics(self.size);
        // The size step stays the fallback; an instance radius replaces it on
        // the chip alone — the remove button inside stays a circle.
        let tag_radius = self
            .radius
            .unwrap_or_else(|| Self::step_radius(self.size, cx));

        // `.tag-group` is `flex flex-col gap-1`: the label, the list and the
        // description.
        let mut root = div().relative().flex().flex_col().gap(px(4.));
        if self.full_width {
            root = root.w_full();
        }

        if let Some(label) = &self.label {
            root = root.child(
                div()
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.foreground)
                    .child(label.to_string()),
            );
        }

        if self.tags.is_empty() {
            let text = self
                .empty_state
                .unwrap_or_else(|| SharedString::from("No tags"));
            root = root.child(
                div()
                    .id(element_id::scoped(&self.id, "list"))
                    // `react-aria/dist/private/tag/useTagGroup.mjs` chooses
                    // the list's role by whether the collection is empty:
                    // `role: state.collection.size ? 'grid' : 'group'`. This
                    // is the empty half. Its `aria-live`/`aria-atomic`/
                    // `aria-relevant` neighbours are recorded omissions —
                    // gpui exposes no live-region builder at all.
                    .a11y_named(a11y::Role::Group, &a11y::Name::maybe(self.label.clone()))
                    // `.empty-state` is `p-2 text-sm text-muted`.
                    .p(px(8.))
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .text_color(colors.muted)
                    .child(text.to_string()),
            );
            return crate::util::apply_sx(root, &self.sx);
        }

        let mut list = div().relative().flex().flex_row().flex_wrap().gap(px(6.));

        for (index, tag) in self.tags.iter().enumerate() {
            let disabled =
                self.is_disabled || tag.is_disabled || self.disabled_keys.contains(&tag.key);
            let selected = self.selected_keys.contains(&tag.key);
            let selectable = self.selection_mode != SelectionMode::None;
            let interactive = selectable && !disabled;
            let tag_foreground = if selected {
                colors.accent.soft_foreground(colors.foreground)
            } else {
                match self.variant {
                    TagVariant::Default => colors.default.foreground,
                    TagVariant::Surface => colors.surface.foreground,
                }
            };

            let mut chip = div()
                .id(element_id::indexed(&self.id, "tag", index))
                // A `Tag` is a grid-list item: `useTag.mjs` builds on
                // `.../gridlist/useGridListItem.mjs`, which is `role: 'row'`
                // with `'aria-label': node['aria-label'] || node.textValue`
                // and `'aria-selected': canSelectItem ? isSelected :
                // undefined`. The `role: 'gridcell'` node that hook also
                // returns has no counterpart element here — this port draws
                // the tag's contents straight into the row — and its
                // `aria-disabled` has no gpui builder.
                .a11y_named(a11y::Role::Row, &a11y::Name::labelled(tag.label.clone()))
                .when(selectable, |c| c.a11y_selected(selected))
                // One handle roves the group and a cursor picks the tag it
                // stands on, which is upstream's virtual focus; gpui states
                // that relation on the descendant.
                .when(cursor_index == Some(index), |c| c.a11y_active_descendant())
                .when(!disabled && cursor_index == Some(index), |c| {
                    c.track_focus(&group_focus)
                })
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .px(pad_x)
                .py(pad_y)
                .rounded(tag_radius)
                .text_size(text_size)
                .line_height(leading)
                .font_weight(gpui::FontWeight::MEDIUM)
                .whitespace_nowrap();

            chip = if selected {
                chip.bg(colors.accent.soft()).text_color(tag_foreground)
            } else {
                match self.variant {
                    TagVariant::Default => chip.bg(colors.default.color).text_color(tag_foreground),
                    TagVariant::Surface => chip
                        .bg(colors.surface.background)
                        .text_color(tag_foreground),
                }
            };

            if disabled {
                chip = chip.opacity(disabled_opacity);
            } else {
                let hover = self.hover_bg.unwrap_or_else(|| {
                    if selected {
                        colors.accent.soft_hover()
                    } else {
                        match self.variant {
                            TagVariant::Default => colors.default.hover(),
                            TagVariant::Surface => colors.surface.hover(),
                        }
                    }
                });
                chip = chip.cursor(crate::util::interactive_cursor(cx));
                // `.tag` declares `background-color 100ms var(--ease-smooth)`.
                // Keep the chip itself as the stable hover listener and put
                // only the interpolated fill in a rounded child. This avoids
                // the keyed animation wrapper resetting the tag's focus and
                // render-prop state, and the child inherits the same radius so
                // a mid-transition frame cannot leak square corners.
                if let Some(slot) = interaction.get(index) {
                    chip = crate::anim::hover_fade_with_duration_and_easing(
                        chip,
                        element_id::scoped(&element_id::indexed(&self.id, "tag", index), "fade"),
                        (
                            if selected {
                                colors.accent.soft()
                            } else {
                                match self.variant {
                                    TagVariant::Default => colors.default.color,
                                    TagVariant::Surface => colors.surface.background,
                                }
                            },
                            hover,
                        ),
                        Some(slot),
                        None,
                        move |fill| fill.rounded(tag_radius),
                        Some(100),
                        crate::anim::HoverFadeEasing::EaseSmooth,
                        window,
                        cx,
                    );
                }
            }

            if let Some(path) = &tag.icon {
                chip = chip.child(
                    gpui::svg()
                        .size(px(12.))
                        .path(path.clone())
                        .flex_shrink_0()
                        .text_color(tag_foreground),
                );
            }

            chip = match &self.tag_content {
                Some(render) => {
                    let (is_hovered, is_pressed) = if interactive {
                        interaction
                            .get(index)
                            .map(|slot| *slot.read(cx))
                            .unwrap_or_default()
                    } else {
                        (false, false)
                    };
                    let focused = !disabled && owns_focus && cursor_index == Some(index);
                    chip.child(render(
                        tag,
                        crate::util::InteractiveState {
                            is_hovered,
                            is_pressed,
                            is_focused: focused,
                            is_focus_visible: focused && ring_visible,
                            is_selected: selected,
                            is_disabled: disabled,
                            is_pending: false,
                            is_indeterminate: false,
                        },
                    ))
                }
                None => chip.child(tag.label.to_string()),
            };
            if !disabled {
                if let Some(slot) = interaction.get(index) {
                    chip = crate::util::track_interaction(chip, slot);
                }
            }

            if let Some(on_remove) = self.on_remove.clone() {
                let key = tag.key.clone();
                let remove_content = tag.remove_content.as_ref().map_or_else(
                    || {
                        gpui::svg()
                            .size(px(12.))
                            .path(icons::CLOSE)
                            .text_color(tag_foreground)
                            .into_any_element()
                    },
                    |render| render(),
                );
                // `.tag__remove-button` is `size-3`.
                let mut remove_visual = div()
                    .size(px(12.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .child(remove_content);
                let mut close = div()
                    .id(element_id::scoped(
                        &element_id::indexed(&self.id, "tag", index),
                        "remove",
                    ))
                    // `useTag.mjs`'s `removeButtonProps` is
                    // `'aria-label': stringFormatter.format(
                    // 'removeButtonLabel')` — `Remove` in the pinned en-US
                    // strings (`react-aria/dist/private/intl/tag/en-US.mjs`)
                    // — plus an `aria-labelledby` pointing at the button and
                    // the row together, which needs the id graph gpui does
                    // not have. The button itself is an RAC `Button`.
                    .a11y_named(a11y::Role::Button, &a11y::Name::labelled(crate::i18n::ui_string(crate::i18n::UiString::Remove, cx)))
                    .group("tag-remove")
                    .flex()
                    .items_center()
                    .justify_center()
                    // v3.2.5's `touch-target` extends 6px on each side of
                    // the glyph. A 12px parent retains the layout footprint.
                    .size(px(24.))
                    .absolute()
                    .top(px(-6.))
                    .left(px(-6.))
                    .flex_shrink_0();
                if !disabled {
                    let hover_bg = self.remove_hover_bg.unwrap_or(colors.default.hover());
                    let remove_focus = &remove_focus_handles[index];
                    let focus_for_remove = group_focus.clone();
                    let cursor_for_remove = cursor.clone();
                    remove_visual =
                        remove_visual.group_hover("tag-remove", move |s| s.bg(hover_bg));
                    close = close
                        .track_focus(remove_focus)
                        .cursor(crate::util::interactive_cursor(cx))
                        // React Aria's grid-list stops row key handling while
                        // a child button owns the focus, except for Tab.
                        .on_key_down(|event, _, cx| {
                            if event.keystroke.key != "tab" {
                                cx.stop_propagation();
                            }
                        })
                        // The press belongs to the button. Stopping it here
                        // keeps the tag body's mouse-down -- which seats the
                        // group's focus and cursor -- out of a remove press.
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .on_click(move |_, window, cx| {
                            // The remove button is an action inside a
                            // selectable tag. Its press belongs to the button,
                            // never to the tag behind it.
                            cx.stop_propagation();
                            on_remove(&HashSet::from([key.clone()]), window, cx);
                            // Pinned `useSelectableItem` only isolates the
                            // child's press -- DOM focus goes to the button.
                            // This port seats the owning tag itself because
                            // the report-only Rust model has no persisting
                            // native child and keyboard continuity needs a
                            // stable roving target. Removal only reports;
                            // the selection is not this click's to change.
                            window.focus(&focus_for_remove, cx);
                            cursor_for_remove.update(cx, |v, cx| {
                                *v = index;
                                cx.notify();
                            });
                        });
                    // The ring rides the 12px glyph rather than the 24px
                    // touch target that owns the focus handle: the glyph is
                    // the element carrying `rounded_full`, and the ring is
                    // concentric only with the shape it is drawn around. A
                    // ring on the touch target would circle a footprint the
                    // pinned sheet never paints. `rounded_full` on a 12px box
                    // resolves to a 6px radius.
                    remove_visual = crate::util::ring_overlay_if_focused(
                        remove_visual,
                        remove_focus,
                        true,
                        px(6.),
                        Vec::new(),
                        window,
                        cx,
                    );
                    close = crate::util::record_focus_bounds(close, remove_focus, window, cx);
                }
                chip = chip.child(
                    div()
                        .relative()
                        .size(px(12.))
                        .flex_shrink_0()
                        .child(close.child(remove_visual)),
                );
            }

            // React Aria's TagGroup: the arrows move between tags. Delete or
            // Backspace removes the selection when the focused tag belongs to
            // it, and otherwise removes only the focused tag.
            if !disabled {
                let stops = enabled.clone();
                let moved = cursor.clone();
                let remove = self.on_remove.clone();
                let key_for_remove = tag.key.clone();
                let mode = self.selection_mode;
                let disallow_empty = self.disallow_empty_selection;
                let escape_key_behavior = self.escape_key_behavior;
                let selected_now = self.selected_keys.clone();
                let selectable_keys = enabled_keys.clone();
                let collection_for_keys = collection_keys.clone();
                let range_for_keys = selection_range.clone();
                let on_selection_change = self.on_selection_change.clone();
                let selection_own_for_keys = selection_own.clone();
                let range_for_all = selection_range.clone();
                let range_for_escape = selection_range.clone();
                chip = chip.on_key_down(move |event, window, cx| {
                    let key_name = event.keystroke.key.as_str();
                    let is_mac = cx.platform_info().is_mac();
                    // Pinned React Aria 3.51 routes TagGroup through
                    // `useSelectableCollection`, including Mod+A select-all.
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
                        let all_selected =
                            selectable_keys.iter().all(|key| selected_now.contains(key));
                        if !all_selected {
                            if let Some(held) = &selection_own_for_keys {
                                held.update(cx, |value, cx| {
                                    *value = selectable_keys.clone();
                                    cx.notify();
                                });
                            }
                            if let Some(change) = &on_selection_change {
                                change(&selectable_keys, window, cx);
                            }
                            // The raw `all`: the next Shift move collapses to
                            // its target instead of extending across
                            // everything. A redundant Mod+A over an already
                            // complete selection is idempotent and keeps the
                            // anchor, like pinned `SelectionManager::selectAll`.
                            range_for_all.update(cx, |range, _| {
                                *range = TagSelectionRange {
                                    is_all: true,
                                    ..TagSelectionRange::default()
                                };
                            });
                        }
                        cx.stop_propagation();
                        return;
                    }
                    // `useSelectableCollection` also clears a nonempty
                    // selection on unmodified Escape by default.
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
                        if let Some(change) = &on_selection_change {
                            change(&next, window, cx);
                        }
                        range_for_escape.update(cx, |range, _| {
                            *range = TagSelectionRange::default();
                        });
                        cx.stop_propagation();
                        return;
                    }
                    match key_name {
                        "delete" | "backspace" => {
                            if let Some(cb) = &remove {
                                cx.stop_propagation();
                                if selected_now.contains(&key_for_remove) {
                                    cb(&selected_now, window, cx);
                                } else {
                                    cb(&HashSet::from([key_for_remove.clone()]), window, cx);
                                }
                            }
                        }
                        key @ ("left" | "right" | "home" | "end") => {
                            // The pinned registrations install no Home/End
                            // handler for an unregistered chord -- Cmd- or
                            // Ctrl-bearing on macOS, Alt- or platform-bearing
                            // elsewhere -- so the whole event stays inert: no
                            // focus move, no selection, no preventDefault,
                            // and no consumption of the key either.
                            if matches!(key, "home" | "end")
                                && !home_end_registered(event.keystroke.modifiers, is_mac)
                            {
                                return;
                            }
                            let key = match key {
                                "right" => "down",
                                "left" => "up",
                                other => other,
                            };
                            // React Aria gives TagGroup a horizontal
                            // keyboard delegate. The list owns its axis
                            // and Home/End; Up/Down fall through to an
                            // enclosing scroller.
                            cx.stop_propagation();
                            let crate::list_nav::Move::To(next) =
                                crate::list_nav::resolve(&stops, Some(index), key, true)
                            else {
                                return;
                            };
                            // Pinned `useSelectableCollection`: Shift extends a
                            // multiple selection from the anchor with no other
                            // chord, so plain Shift navigation is exact. A
                            // wrap-to-self move or a Home/End
                            // already at its target still extends: pinned
                            // `extendSelection` replaces the anchor..target
                            // range even when the cursor does not move, and
                            // the unchanged-selection guard below is what
                            // keeps true no-ops silent.
                            let modifiers = event.keystroke.modifiers;
                            let exact_shift_navigation = if is_mac {
                                !modifiers.control && !modifiers.platform && !modifiers.function
                            } else {
                                !modifiers.alt && !modifiers.platform && !modifiers.function
                            };
                            let extends_selection = modifiers.shift
                                && mode == SelectionMode::Multiple
                                && exact_shift_navigation
                                && shift_home_end_extends(key_name, modifiers.control, is_mac);
                            if extends_selection {
                                if let Some(target) = collection_for_keys.get(next) {
                                    let range = range_for_keys.read(cx).clone();
                                    let next_selection = extend_selection_range(
                                        &selected_now,
                                        &collection_for_keys,
                                        &selectable_keys,
                                        &range,
                                        target,
                                    );
                                    range_for_keys
                                        .update(cx, |range, _| range.seat(target.clone()));
                                    if next_selection != selected_now {
                                        if let Some(held) = &selection_own_for_keys {
                                            held.update(cx, |value, cx| {
                                                *value = next_selection.clone();
                                                cx.notify();
                                            });
                                        }
                                        if let Some(change) = &on_selection_change {
                                            change(&next_selection, window, cx);
                                        }
                                    }
                                }
                            }
                            // No refocusing: the next render has the tag
                            // at `next` claim the group's handle, so the
                            // focus goes with it.
                            moved.update(cx, |v, cx| {
                                *v = next;
                                cx.notify();
                            });
                        }
                        _ => {}
                    }
                });
                // React Aria seats a collection on pointer-down: pressing a
                // tag's body moves the roving cursor to it and takes the
                // group's focus, so the arrows and Space answer the tag the
                // user pressed with no Tab first. A child that prevented the
                // press -- the remove button -- keeps the body out of it, and
                // preventing the default here is the other half of the seat:
                // gpui's own focus-on-press for `track_focus` elements works
                // the same way, so an ancestor's -- the app root's -- press
                // focus cannot steal the handle back in the same dispatch.
                let focus_for_seat = group_focus.clone();
                let cursor_for_seat = cursor.clone();
                chip = chip.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                    if window.default_prevented() {
                        return;
                    }
                    window.focus(&focus_for_seat, cx);
                    window.prevent_default();
                    cursor_for_seat.update(cx, |v, cx| {
                        *v = index;
                        cx.notify();
                    });
                });
            }

            if selectable && !disabled {
                let key = tag.key.clone();
                let mode = self.selection_mode;
                let disallow_empty = self.disallow_empty_selection;
                let current = self.selected_keys.clone();
                let on_change = self.on_selection_change.clone();
                let selection_own = selection_own.clone();
                let collection_for_click = collection_keys.clone();
                let selectable_for_click = enabled_keys.clone();
                let range_for_click = selection_range.clone();
                chip = chip.on_click(move |ev, window, cx| {
                    let was_selected = current.contains(&key);
                    // A Shift click extends from the anchor in multiple mode;
                    // an ordinary click toggles and re-anchors instead.
                    let extends_selection = ev.modifiers().shift && mode == SelectionMode::Multiple;
                    let next = if extends_selection {
                        let range = range_for_click.read(cx).clone();
                        extend_selection_range(
                            &current,
                            &collection_for_click,
                            &selectable_for_click,
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
                    // A controlled selection only reports; the owner's prop
                    // stays in charge until it feeds the value back.
                    if next != current {
                        if let Some(held) = &selection_own {
                            held.update(cx, |value, cx| {
                                *value = next.clone();
                                cx.notify();
                            });
                        }
                        if let Some(change) = &on_change {
                            change(&next, window, cx);
                        }
                    }
                    if extends_selection {
                        range_for_click.update(cx, |range, _| range.seat(key.clone()));
                    } else if mode == SelectionMode::Multiple && !was_selected {
                        range_for_click.update(cx, |range, _| {
                            range.anchor = Some(key.clone());
                            range.current = Some(key.clone());
                            range.is_all = false;
                        });
                    } else if mode == SelectionMode::Multiple {
                        range_for_click.update(cx, |range, _| {
                            if range.is_all {
                                *range = TagSelectionRange::default();
                            }
                        });
                    }
                });
            }

            // `.tag:focus-visible` is `status-focused`.
            let mut chip = crate::util::with_focus_ring_overlay(
                chip,
                !disabled && ring_visible && cursor_index == Some(index) && owns_focus,
                true,
                tag_radius,
                Vec::new(),
                cx,
            );
            if !disabled && cursor_index == Some(index) {
                chip = crate::util::record_focus_bounds(chip, &group_focus, window, cx);
            }
            list = list.child(chip);
        }

        // The id and the role go on at the end rather than in the chain
        // above: `.shots/design_audit.py` reads `.tag-group__list`'s gap with
        // a regex spelling `let mut list = div().relative().flex()...` with
        // no room between the calls, and it correctly reported the line as
        // unreadable when they were spliced in.
        //
        // This is the populated half of `useTagGroup.mjs`'s role switch
        // (`role: state.collection.size ? 'grid' : 'group'`); the grid comes
        // from `useGridList` underneath it, and RAC's `TagGroup.mjs` spreads
        // those props onto the `TagList` unmodified.
        root = root.child(
            list.id(element_id::scoped(&self.id, "list"))
                .a11y_named(a11y::Role::Grid, &a11y::Name::maybe(self.label.clone())),
        );

        if let Some(description) = &self.description {
            root = root.child(
                div()
                    .p(px(4.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(colors.muted)
                    .child(description.to_string()),
            );
        }

        root = crate::util::apply_sx(root, &self.sx);
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The horizontal delegate's arrows never consult the Home/End gate:
    /// their forbidden extra chords are rejected earlier, by
    /// `exact_shift_navigation`.
    #[test]
    fn shift_navigation_keys_do_not_consult_the_home_end_gate() {
        for key in ["left", "right"] {
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

crate::util::impl_component_styled!(TagGroup);
