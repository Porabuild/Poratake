//! Select — HeroUI `select.css` trigger (field surface, chevron indicator)
//! backed by the shared menu layer for its option list.

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, AnimationExt, App, ElementId, Pixels, SharedString, Styled, Window,
};

use crate::theme::vars::active_theme;
use crate::ui::chrome;
use crate::ui::icon::chevron_element;
use crate::ui::menu::{MenuBuilder, MenuHandle, MenuItem, MenuPlacement};

#[derive(Clone, PartialEq)]
pub struct SelectOption {
    pub value: SharedString,
    pub label: SharedString,
    pub icon: Option<SharedString>,
    pub disabled: bool,
}

impl SelectOption {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            icon: None,
            disabled: false,
        }
    }

    #[allow(dead_code)]
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    #[allow(dead_code)]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    owner: SharedString,
    menu: MenuHandle,
    selected: Option<SharedString>,
    placeholder: SharedString,
    options: Vec<SelectOption>,
    full_width: bool,
    width: Option<Pixels>,
    disabled: bool,
    small: bool,
    on_select: Option<SelectHandler>,
}

impl Select {
    pub fn new(id: impl Into<SharedString>, menu: MenuHandle) -> Self {
        let owner = id.into();
        Self {
            id: ElementId::Name(owner.clone()),
            owner,
            menu,
            selected: None,
            placeholder: SharedString::default(),
            options: Vec::new(),
            full_width: false,
            width: None,
            disabled: false,
            small: false,
            on_select: None,
        }
    }

    pub fn selected(mut self, value: impl Into<SharedString>) -> Self {
        self.selected = Some(value.into());
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn options(mut self, options: Vec<SelectOption>) -> Self {
        self.options = options;
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    #[allow(dead_code)]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    fn selected_label(&self) -> Option<SharedString> {
        let selected = self.selected.as_ref()?;
        self.options
            .iter()
            .find(|option| &option.value == selected)
            .map(|option| option.label.clone())
    }
}

impl RenderOnce for Select {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = active_theme(cx);
        let owner_key = self.owner.to_string();
        let focus = crate::ui::primitives::control_focus(
            &owner_key,
            self.disabled || self.options.is_empty(),
            window,
            cx,
        );
        let resolved = self.selected_label();
        let is_placeholder = resolved.is_none();
        let label = resolved.unwrap_or_else(|| self.placeholder.clone());
        let is_open = self.menu.is_open_for(&self.owner);

        let (min_h, pad_y, text) = if self.small {
            (
                px(chrome::SELECT_SM_HEIGHT),
                px(0.0),
                px(chrome::SELECT_SM_TEXT),
            )
        } else {
            (
                px(chrome::FIELD_MIN_HEIGHT),
                px(chrome::FIELD_PAD_Y),
                px(chrome::FIELD_TEXT),
            )
        };
        // `ui/select.tsx` passes `variant="secondary"`, so the trigger sits on
        // `--default` (not `--field-background`, which only matches in dark
        // mode) and hovers to `--default-hover`. There is no border: the app
        // pins `--field-border-width: 0px`.
        let mut trigger = div()
            .id(self.id)
            .track_focus(&focus)
            .focus(|style| style.shadow(crate::ui::primitives::focus_ring(&theme, 0.0)))
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .min_h(min_h)
            .rounded(px(chrome::FIELD_RADIUS))
            .pl(px(chrome::FIELD_PAD_X))
            // `pe-7` reserves room for the absolutely placed indicator.
            .pr(px(chrome::SELECT_INDICATOR_PAD_END))
            .py(pad_y)
            .text_size(text)
            .bg(theme.default)
            .when(self.full_width, |el| el.w_full())
            .when_some(self.width, |el, width| el.w(width))
            .text_color(if self.disabled {
                theme.field_placeholder.opacity(0.5)
            } else if is_placeholder {
                theme.field_placeholder
            } else {
                theme.field_foreground
            })
            .when(self.disabled, |el| el.opacity(0.5))
            // `settings-select.tsx` is `w-40 shrink-0` with no clamp, so a long
            // value wraps and the trigger grows taller rather than being cut off
            // with an ellipsis.
            .child(div().flex_1().min_w_0().child(label))
            .child(
                div()
                    .absolute()
                    .right(px(chrome::SELECT_INDICATOR_INSET))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .text_color(theme.field_placeholder)
                    .child(chevron_element(px(chrome::SELECT_INDICATOR_SIZE), is_open)),
            )
            .child(self.menu.render_dropdown(&self.owner));

        if self.disabled || self.options.is_empty() {
            return trigger.into_any_element();
        }

        let menu = self.menu.clone();
        let owner = self.owner.clone();
        let options = self.options.clone();
        let selected = self.selected.clone();
        let handler = self.on_select.clone();
        let min_width = self.width;
        let compact = self.small;

        trigger = trigger.on_click(move |_event, window, cx| {
            let mut builder = MenuBuilder::new();
            for option in &options {
                let value = option.value.clone();
                let handler = handler.clone();
                let mut entry = MenuItem::new(option.label.clone())
                    .trailing_check(selected.as_ref() == Some(&option.value))
                    .disabled(option.disabled)
                    .on_select(move |window, cx| {
                        if let Some(handler) = &handler {
                            handler(&value, window, cx);
                        }
                    });
                if let Some(icon) = &option.icon {
                    entry = entry.icon(icon.clone());
                }
                builder = builder.item(entry);
            }
            let mut placement = MenuPlacement::below(owner.clone()).compact(compact);
            if let Some(width) = min_width {
                placement = placement.min_width(width);
            }
            menu.toggle(placement, builder.build(), window, cx);
            cx.stop_propagation();
        });

        // `.select__trigger { transition: background-color 150ms }`.
        let (hover, hovered, _) = crate::ui::primitives::hover_fade(&owner_key, window, cx);
        let (from, to) = hover.read(cx).range(theme.default, theme.default_hover);
        trigger
            .on_hover({
                let hover = hover.clone();
                move |over: &bool, _window, cx| {
                    crate::ui::primitives::track_hover(&hover, *over, cx);
                }
            })
            .with_animation(
                gpui::ElementId::Name(format!("{owner_key}-hover-{hovered}").into()),
                gpui::Animation::new(std::time::Duration::from_millis(chrome::FIELD_HOVER_MS))
                    .with_easing(crate::ui::primitives::ease_smooth()),
                move |trigger, delta| trigger.bg(crate::theme::color::lerp_srgb(from, to, delta)),
            )
            .into_any_element()
    }
}
