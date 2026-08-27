use std::rc::Rc;

use gpui::{AnyElement, App, SharedString, Window};

pub type MenuAction = Rc<dyn Fn(&mut Window, &mut App)>;

/// Renders custom leading content for an item (thickness bars, arrow and
/// number previews) the way the renderer's `ListBox.Item` children do.
pub type MenuDecoration = Rc<dyn Fn(&mut App) -> AnyElement>;

#[derive(Clone)]
pub struct MenuItem {
    pub label: SharedString,
    pub icon: Option<SharedString>,
    pub shortcut: Option<SharedString>,
    pub disabled: bool,
    pub danger: bool,
    pub toggle: Option<bool>,
    pub radio: Option<bool>,
    pub trailing_check: bool,
    pub leading: Option<MenuDecoration>,
    pub row_label: Option<SharedString>,
    pub trailing_switch: Option<bool>,
    pub inset: bool,
    pub submenu: Vec<MenuEntry>,
    pub action: Option<MenuAction>,
}

impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            shortcut: None,
            disabled: false,
            danger: false,
            toggle: None,
            radio: None,
            trailing_check: false,
            leading: None,
            row_label: None,
            trailing_switch: None,
            inset: false,
            submenu: Vec::new(),
            action: None,
        }
    }

    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    #[allow(dead_code)]
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        let shortcut = shortcut.into();
        if !shortcut.is_empty() {
            self.shortcut = Some(shortcut);
        }
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    #[allow(dead_code)]
    pub fn toggle(mut self, checked: bool) -> Self {
        self.toggle = Some(checked);
        self
    }

    pub fn radio(mut self, selected: bool) -> Self {
        self.radio = Some(selected);
        self
    }

    /// A trailing check indicator, matching HeroUI's `ListBox.ItemIndicator`
    /// used by the renderer's select popover.
    pub fn trailing_check(mut self, checked: bool) -> Self {
        self.trailing_check = checked;
        self
    }

    pub fn leading(mut self, render: impl Fn(&mut App) -> AnyElement + 'static) -> Self {
        self.leading = Some(Rc::new(render));
        self
    }

    /// Renders the entry as a settings row: a muted label on the left and this
    /// item's label as a compact pill on the right, matching the small selects
    /// inside the renderer's tool-option popovers.
    pub fn row(mut self, label: impl Into<SharedString>) -> Self {
        self.row_label = Some(label.into());
        self
    }

    pub fn trailing_switch(mut self, checked: bool) -> Self {
        self.trailing_switch = Some(checked);
        self
    }

    #[allow(dead_code)]
    pub fn inset(mut self) -> Self {
        self.inset = true;
        self
    }

    pub fn submenu(mut self, entries: Vec<MenuEntry>) -> Self {
        self.submenu = entries;
        self
    }

    pub fn on_select(mut self, action: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.action = Some(Rc::new(action));
        self
    }

    pub fn is_interactive(&self) -> bool {
        !self.disabled && (self.action.is_some() || !self.submenu.is_empty())
    }

    pub fn is_row(&self) -> bool {
        self.row_label.is_some()
    }
}

#[derive(Clone)]
pub enum MenuEntry {
    Item(MenuItem),
    Separator,
    #[allow(dead_code)]
    Label(SharedString),
}

#[derive(Default)]
pub struct MenuBuilder {
    entries: Vec<MenuEntry>,
}

impl MenuBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn item(mut self, item: MenuItem) -> Self {
        self.entries.push(MenuEntry::Item(item));
        self
    }

    #[allow(dead_code)]
    pub fn item_when(self, condition: bool, item: MenuItem) -> Self {
        if condition {
            self.item(item)
        } else {
            self
        }
    }

    #[allow(dead_code)]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.entries.push(MenuEntry::Label(label.into()));
        self
    }

    pub fn separator(mut self) -> Self {
        self.entries.push(MenuEntry::Separator);
        self
    }

    #[allow(dead_code)]
    pub fn extend(mut self, entries: impl IntoIterator<Item = MenuEntry>) -> Self {
        self.entries.extend(entries);
        self
    }

    pub fn build(self) -> Vec<MenuEntry> {
        prune(self.entries)
    }
}

pub fn prune(entries: Vec<MenuEntry>) -> Vec<MenuEntry> {
    let mut result: Vec<MenuEntry> = Vec::with_capacity(entries.len());
    for entry in entries {
        if matches!(entry, MenuEntry::Separator)
            && matches!(result.last(), None | Some(MenuEntry::Separator))
        {
            continue;
        }
        result.push(entry);
    }
    while matches!(result.last(), Some(MenuEntry::Separator)) {
        result.pop();
    }
    result
}
