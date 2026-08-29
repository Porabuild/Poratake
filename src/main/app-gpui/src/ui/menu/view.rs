use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    anchored, canvas, deferred, div, point, prelude::*, px, AnchoredPositionMode, App, Context,
    Corner, ElementId, Entity, FocusHandle, KeyDownEvent, Pixels, Render, SharedString, Styled,
    Window,
};

use crate::theme::vars::{active_theme, ThemeVars};
use crate::ui::icon::icon_element;
use crate::ui::menu::model::{MenuEntry, MenuItem};
use crate::ui::switch::{Switch, SwitchSize};

pub const MENU_MIN_WIDTH: f32 = 128.0;
pub const MENU_MAX_HEIGHT: f32 = 420.0;
/// `.list-box-item { min-h-9 }`, and 28px under `.select__popover--sm`
/// (`min-h-7`).
const ITEM_HEIGHT: f32 = 36.0;
const ITEM_HEIGHT_COMPACT: f32 = 28.0;
/// `.select__popover [data-slot=list-box-item] { px-2.5 }`, `px-2` compact.
const ITEM_PAD_X: f32 = 10.0;
const ITEM_PAD_X_COMPACT: f32 = 8.0;
/// `.list-box-item { gap-3 }`, `gap-2` compact.
const ITEM_GAP: f32 = 12.0;
const ITEM_GAP_COMPACT: f32 = 8.0;
/// `.select__popover [data-slot=list-box] { p-1.5 }`, `p-1` compact.
const LIST_PAD: f32 = 6.0;
const LIST_PAD_COMPACT: f32 = 4.0;
const INDICATOR_INSET: f32 = 32.0;

pub type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// How the menu comes in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MenuEntrance {
    /// `fade-in zoom-in-95 slide-in-from-*-1`, as `popover.css` does. The menu
    /// floats inside a larger surface, so moving and re-laying itself out is
    /// exactly what it should do.
    #[default]
    Overlay,
    /// No entrance at all. For a menu whose window animates the whole surface:
    /// gpui tracks dirtiness per view, so animating here would re-render every
    /// row on every step, while animating the wrapper only re-paints them.
    Instant,
}

pub struct MenuView {
    entries: Vec<MenuEntry>,
    highlighted: Option<usize>,
    /// Whether `highlighted` came from the pointer or the keyboard. A
    /// pointer-driven highlight must also survive the pointer *leaving* the
    /// window without an edge -- gpui never dispatches the exit, so the row
    /// would stay lit forever -- while a keyboard-driven one must stay lit no
    /// matter where the mouse is. See `primitives::hover_is_active`.
    highlighted_by_pointer: bool,
    submenu: Option<(usize, Entity<MenuView>)>,
    /// Where each row was laid out, so an open submenu can be anchored in
    /// window space from outside the scroll container. Anchoring it to the row
    /// itself would let the container's content mask clip it, which is what
    /// stops a long menu from scrolling at all.
    row_bounds: Rc<RefCell<HashMap<usize, gpui::Bounds<Pixels>>>>,
    on_dismiss: DismissHandler,
    min_width: Pixels,
    max_width: Option<Pixels>,
    max_height: Pixels,
    compact: bool,
    neutral_highlight: bool,
    entrance: MenuEntrance,
    /// When this popover opened, so `zoom-in-95` can be applied as a
    /// proportional re-layout over the entrance.
    opened_at: std::time::Instant,
    focus_handle: FocusHandle,
}

impl MenuView {
    pub fn new(
        entries: Vec<MenuEntry>,
        on_dismiss: DismissHandler,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            entries,
            highlighted: None,
            highlighted_by_pointer: false,
            submenu: None,
            row_bounds: Rc::new(RefCell::new(HashMap::new())),
            on_dismiss,
            min_width: px(MENU_MIN_WIDTH),
            max_width: None,
            max_height: px(MENU_MAX_HEIGHT),
            compact: false,
            neutral_highlight: false,
            entrance: MenuEntrance::default(),
            opened_at: std::time::Instant::now(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn min_width(mut self, width: Pixels) -> Self {
        self.min_width = width;
        self
    }

    pub fn max_width(mut self, width: Pixels) -> Self {
        self.max_width = Some(width);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn neutral_highlight(mut self, neutral: bool) -> Self {
        self.neutral_highlight = neutral;
        self
    }

    pub fn entrance(mut self, entrance: MenuEntrance) -> Self {
        self.entrance = entrance;
        self
    }

    /// `zoom-in-95`, applied to every length the popover lays out with.
    fn scale(&self) -> f32 {
        match self.entrance {
            MenuEntrance::Instant => 1.0,
            MenuEntrance::Overlay => crate::ui::primitives::enter_scale(
                self.opened_at,
                crate::ui::primitives::OVERLAY_ENTER_ZOOM_95,
            ),
        }
    }

    fn scaled(&self, value: f32) -> f32 {
        value * self.scale()
    }

    fn entrance_frame(&self) -> (f32, f32) {
        match self.entrance {
            MenuEntrance::Instant => (1.0, 0.0),
            MenuEntrance::Overlay => {
                let progress = crate::ui::primitives::enter_progress(self.opened_at);
                (
                    progress,
                    -crate::ui::primitives::OVERLAY_ENTER_SLIDE * (1.0 - progress),
                )
            }
        }
    }

    fn item_height(&self) -> f32 {
        self.scaled(if self.compact {
            ITEM_HEIGHT_COMPACT
        } else {
            ITEM_HEIGHT
        })
    }

    fn item_pad_x(&self) -> f32 {
        self.scaled(if self.compact {
            ITEM_PAD_X_COMPACT
        } else {
            ITEM_PAD_X
        })
    }

    fn item_gap(&self) -> f32 {
        self.scaled(if self.compact {
            ITEM_GAP_COMPACT
        } else {
            ITEM_GAP
        })
    }

    fn list_pad(&self) -> f32 {
        self.scaled(if self.compact {
            LIST_PAD_COMPACT
        } else {
            LIST_PAD
        })
    }

    /// Deliberately *not* scaled. A CSS transform scales the rasterised glyphs
    /// uniformly; scaling the font size instead re-shapes the text every frame,
    /// which reads as a shimmer over the 150ms the box geometry is animating.
    /// Holding the size fixed keeps the visible behaviour -- content settling
    /// into place -- without the artifact the transform does not have.
    fn item_text(&self) -> f32 {
        if self.compact {
            crate::ui::chrome::TEXT_XS
        } else {
            crate::ui::chrome::TEXT_SM
        }
    }

    #[allow(dead_code)]
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = height;
        self
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    fn item_at(&self, index: usize) -> Option<&MenuItem> {
        match self.entries.get(index) {
            Some(MenuEntry::Item(item)) => Some(item),
            _ => None,
        }
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self.entries.len();
        if count == 0 {
            return;
        }
        let mut index = match self.highlighted {
            Some(current) => current as isize,
            None if delta > 0 => -1,
            None => count as isize,
        };
        for _ in 0..count {
            index = (index + delta).rem_euclid(count as isize);
            if self
                .item_at(index as usize)
                .is_some_and(MenuItem::is_interactive)
            {
                self.highlighted = Some(index as usize);
                self.highlighted_by_pointer = false;
                self.submenu = None;
                cx.notify();
                return;
            }
        }
    }

    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.item_at(index) else {
            return;
        };
        if item.disabled {
            return;
        }
        if !item.submenu.is_empty() {
            self.open_submenu(index, cx);
            return;
        }
        let Some(action) = item.action.clone() else {
            return;
        };
        let dismiss = self.on_dismiss.clone();
        action(window, cx);
        dismiss(window, cx);
    }

    fn open_submenu(&mut self, index: usize, cx: &mut Context<Self>) {
        if self
            .submenu
            .as_ref()
            .is_some_and(|(open, _)| *open == index)
        {
            return;
        }
        let Some(item) = self.item_at(index) else {
            return;
        };
        if item.submenu.is_empty() {
            self.submenu = None;
            cx.notify();
            return;
        }
        let entries = item.submenu.clone();
        let dismiss = self.on_dismiss.clone();
        let neutral_highlight = self.neutral_highlight;
        let entrance = self.entrance;
        let view = cx.new(|cx| {
            MenuView::new(entries, dismiss, cx)
                .neutral_highlight(neutral_highlight)
                .entrance(entrance)
        });
        self.submenu = Some((index, view));
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "down" => self.step(1, cx),
            "up" => self.step(-1, cx),
            "right" => {
                if let Some(index) = self.highlighted {
                    self.open_submenu(index, cx);
                }
            }
            "left" => {
                self.submenu = None;
                cx.notify();
            }
            "enter" | "space" => {
                if let Some(index) = self.highlighted {
                    self.activate(index, window, cx);
                }
            }
            "escape" => {
                let dismiss = self.on_dismiss.clone();
                dismiss(window, cx);
            }
            _ => {}
        }
    }

    fn item_row(
        &self,
        index: usize,
        item: &MenuItem,
        theme: &ThemeVars,
        has_indicator: bool,
        pointer_inside: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if item.is_row() {
            return self.settings_row(index, item, theme, window, cx);
        }
        // A pointer-driven highlight is gated on the window still holding the
        // pointer: gpui never reports the exit, so `highlighted` alone would
        // keep the last row lit after the pointer flicks away. The row that
        // owns the open submenu stays lit either way -- that is the design.
        let pointer_highlight = if self.highlighted_by_pointer && !pointer_inside {
            None
        } else {
            self.highlighted
        };
        let highlighted = pointer_highlight == Some(index)
            || self
                .submenu
                .as_ref()
                .is_some_and(|(open, _)| *open == index);
        let interactive = item.is_interactive();
        let foreground = if item.disabled {
            theme.muted_foreground.opacity(0.5)
        } else if item.danger {
            theme.danger
        } else {
            theme.popover_foreground
        };

        let mut row = div()
            .id(ElementId::Name(SharedString::from(format!(
                "menu-item-{index}"
            ))))
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(self.item_gap()))
            .min_h(px(self.item_height()))
            .px(px(self.item_pad_x()))
            .rounded(px(self.scaled(crate::ui::chrome::RADIUS_2XL)))
            .text_size(px(self.item_text()))
            .text_color(foreground)
            .when(
                highlighted && interactive && !self.neutral_highlight,
                |el| el.bg(theme.accent).text_color(theme.accent_foreground),
            )
            .when(highlighted && interactive && self.neutral_highlight, |el| {
                el.bg(theme.default_hover)
            })
            .when(has_indicator, |el| el.pl(px(INDICATOR_INSET)));

        if has_indicator {
            let indicator = if item.toggle == Some(true) {
                Some(icon_element("check", px(14.0)))
            } else if item.radio == Some(true) {
                Some(icon_element("circle", px(8.0)))
            } else {
                None
            };
            if let Some(indicator) = indicator {
                row = row.child(
                    div()
                        .absolute()
                        .left(px(10.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(indicator),
                );
            }
        }

        if let Some(leading) = &item.leading {
            row = row.child(div().flex_shrink_0().child(leading(cx)));
        } else if let Some(icon) = &item.icon {
            row = row.child(
                div()
                    .flex_shrink_0()
                    .child(icon_element(icon.as_ref(), px(14.0))),
            );
        }

        row = row.child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(item.label.clone()),
        );

        if let Some(shortcut) = &item.shortcut {
            row = row.child(
                div()
                    .flex_shrink_0()
                    .pl(px(16.0))
                    .text_size(px(crate::ui::chrome::TEXT_XS))
                    .text_color(theme.muted_foreground)
                    .child(shortcut.clone()),
            );
        }

        if item.trailing_check {
            row = row.child(div().flex_shrink_0().child(icon_element("check", px(14.0))));
        }

        if !item.submenu.is_empty() {
            row = row.child(
                div()
                    .flex_shrink_0()
                    .child(icon_element("chevron-right", px(14.0))),
            );
        }

        if !interactive {
            return row.into_any_element();
        }

        row = row
            .on_hover(cx.listener(move |this, hovered: &bool, _window, cx| {
                if !*hovered {
                    if this.highlighted == Some(index)
                        && this.submenu.as_ref().is_none_or(|(open, _)| *open != index)
                    {
                        this.highlighted = None;
                        cx.notify();
                    }
                    return;
                }
                this.highlighted = Some(index);
                this.highlighted_by_pointer = true;
                let has_submenu = this
                    .item_at(index)
                    .is_some_and(|item| !item.submenu.is_empty());
                if has_submenu {
                    this.open_submenu(index, cx);
                } else if this.submenu.is_some() {
                    this.submenu = None;
                }
                cx.notify();
            }))
            .on_click(cx.listener(move |this, _event, window, cx| {
                this.activate(index, window, cx);
            }));

        row.child(self.bounds_recorder(index)).into_any_element()
    }

    /// A zero-size canvas that records where its row ended up.
    fn bounds_recorder(&self, index: usize) -> gpui::AnyElement {
        let bounds = self.row_bounds.clone();
        canvas(
            move |laid_out, _window, _cx| {
                bounds.borrow_mut().insert(index, laid_out);
            },
            |_, (), _, _| {},
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    /// The open submenu, anchored beside its row in window space.
    fn submenu_layer(&self) -> Option<gpui::AnyElement> {
        let (open, view) = self.submenu.as_ref()?;
        let bounds = self.row_bounds.borrow().get(open).copied()?;
        Some(
            deferred(
                anchored()
                    .anchor(Corner::TopLeft)
                    .position_mode(AnchoredPositionMode::Window)
                    .position(point(bounds.right() + px(6.0), bounds.top() - px(5.0)))
                    .snap_to_window_with_margin(px(8.0))
                    .child(view.clone()),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    fn settings_row(
        &self,
        index: usize,
        item: &MenuItem,
        theme: &ThemeVars,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let label = item.row_label.clone().unwrap_or_default();
        let mut row = div()
            .id(ElementId::Name(SharedString::from(format!(
                "menu-row-{index}"
            ))))
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .px(px(8.0))
            .py(px(6.0))
            .child(
                div()
                    .text_size(px(crate::ui::chrome::TEXT_XS))
                    .text_color(theme.muted_foreground)
                    .child(label),
            );

        if let Some(checked) = item.trailing_switch {
            let action = item.action.clone();
            row = row.child(
                Switch::new(
                    ElementId::Name(SharedString::from(format!("menu-row-switch-{index}"))),
                    checked,
                )
                .size(SwitchSize::Sm)
                .on_change(move |_value, window, cx| {
                    if let Some(action) = &action {
                        action(window, cx);
                    }
                }),
            );
            return row.into_any_element();
        }

        let open = self
            .submenu
            .as_ref()
            .is_some_and(|(current, _)| *current == index);
        // Gated hover flag instead of a `.hover()` style, which gpui paints
        // against the window's last mouse position and so survives the
        // pointer leaving the window.
        let pill_key = format!("menu-row-pill-{index}");
        let (pill_hover, pill_hovered) = crate::ui::primitives::hover_flag(&pill_key, window, cx);
        let mut pill = div()
            .id(ElementId::Name(SharedString::from(format!(
                "menu-row-pill-{index}"
            ))))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(4.0))
            .h(px(24.0))
            .min_w(px(56.0))
            .rounded(px(self.scaled(crate::ui::chrome::RADIUS_3XL)))
            .px(px(8.0))
            .text_size(px(crate::ui::chrome::TEXT_XS))
            .font_weight(gpui::FontWeight::MEDIUM)
            .bg(if open || pill_hovered {
                theme.default_hover
            } else {
                theme.default
            })
            .text_color(theme.foreground)
            .on_hover({
                let pill_hover = pill_hover.clone();
                move |over: &bool, _window, cx| {
                    crate::ui::primitives::track_hover(&pill_hover, *over, cx);
                }
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(item.label.clone()),
            )
            .child(icon_element(
                "chevron-down",
                px(crate::ui::chrome::SELECT_INDICATOR_SIZE),
            ));

        if !item.submenu.is_empty() {
            pill = pill.on_click(cx.listener(move |this, _event, _window, cx| {
                if this
                    .submenu
                    .as_ref()
                    .is_some_and(|(current, _)| *current == index)
                {
                    this.submenu = None;
                    cx.notify();
                    return;
                }
                this.open_submenu(index, cx);
            }));
        }

        row.child(pill)
            .child(self.bounds_recorder(index))
            .into_any_element()
    }
}

impl Render for MenuView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let pointer_inside = window.is_window_hovered();
        let overlay = matches!(self.entrance, MenuEntrance::Overlay);
        let (enter_opacity, enter_offset) = self.entrance_frame();
        let has_indicator = self.entries.iter().any(|entry| match entry {
            MenuEntry::Item(item) => {
                !item.is_row() && (item.toggle.is_some() || item.radio.is_some() || item.inset)
            }
            _ => false,
        });

        let mut list = div()
            .id("menu-content")
            .key_context("Menu")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _event, window, cx| {
                let dismiss = this.on_dismiss.clone();
                dismiss(window, cx);
            }))
            .flex()
            .flex_col()
            .min_w(self.min_width)
            .when_some(self.max_width, |el, width| el.max_w(width))
            .rounded(px(self.scaled(crate::ui::chrome::RADIUS_3XL)))
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .shadow_md()
            .p(px(self.list_pad()))
            .when(overlay && enter_opacity < 1.0, |el| {
                crate::ui::primitives::request_animation_frame(window);
                el
            })
            .when(overlay, |el| el.opacity(enter_opacity).mt(px(enter_offset)))
            .max_h(self.max_height)
            .overflow_y_scroll();

        for (index, entry) in self.entries.iter().enumerate() {
            list = list.child(match entry {
                MenuEntry::Separator => separator(&theme),
                MenuEntry::Label(text) => label_row(text.clone(), &theme, has_indicator),
                MenuEntry::Item(item) => self.item_row(
                    index,
                    item,
                    &theme,
                    has_indicator,
                    pointer_inside,
                    window,
                    cx,
                ),
            });
        }

        div().relative().child(list).children(self.submenu_layer())
    }
}

fn separator(theme: &ThemeVars) -> gpui::AnyElement {
    div()
        .my(px(4.0))
        .mx(px(-4.0))
        .h(px(1.0))
        .bg(theme.separator)
        .into_any_element()
}

fn label_row(text: SharedString, theme: &ThemeVars, has_indicator: bool) -> gpui::AnyElement {
    div()
        .px(px(8.0))
        .py(px(6.0))
        .when(has_indicator, |el| el.pl(px(INDICATOR_INSET)))
        .text_size(px(12.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.muted_foreground)
        .child(text)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn completed_menu_entrance_stays_settled_across_pointer_highlight_updates(
        cx: &mut gpui::TestAppContext,
    ) {
        let menu = cx.update(|cx| {
            cx.new(|cx| {
                let mut menu = MenuView::new(Vec::new(), Rc::new(|_, _| {}), cx);
                menu.opened_at = std::time::Instant::now()
                    - std::time::Duration::from_millis(crate::ui::primitives::OVERLAY_ENTER_MS * 2);
                menu
            })
        });
        let before = menu.read_with(cx, |menu, _| menu.entrance_frame());

        menu.update(cx, |menu, cx| {
            menu.highlighted = Some(0);
            menu.highlighted_by_pointer = true;
            cx.notify();
        });
        let after = menu.read_with(cx, |menu, _| menu.entrance_frame());

        assert_eq!((before, after), ((1.0, 0.0), (1.0, 0.0)));
    }
}
