//! The shared keyed item for the pickers.
//!
//! HeroUI v3's collection components keep a stable `Key` separate from each
//! item's `textValue`: `value` / `defaultValue` / `disabledKeys`, the selection
//! callbacks and form submission address items by key, while filtering and the
//! visible text use the label. A label cannot serve as that key -- two items
//! may share one, and then they alias each other's selection, disabled state
//! and row identity -- so the keyed pickers take [`PickerItem`]s, following
//! the `ListBoxItem` / `Tag` / `TabItem` precedent.

use gpui::SharedString;

/// One item of a keyed picker collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerItem {
    key: SharedString,
    label: SharedString,
}

impl PickerItem {
    /// Creates an item with the given key and label.
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
        }
    }

    /// The stable `Key` the selection, `disabledKeys` and callbacks address.
    pub fn key(&self) -> &SharedString {
        &self.key
    }

    /// The visible `textValue` the filtering and rendering use.
    pub fn label(&self) -> &SharedString {
        &self.label
    }
}
