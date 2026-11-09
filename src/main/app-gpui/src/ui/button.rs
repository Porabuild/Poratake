use herogpui::components::{Button, Size};
use herogpui::gpui::px;

use crate::ui::chrome;

pub trait CompactButton {
    fn compact(self) -> Self;
}

impl CompactButton for Button {
    fn compact(self) -> Self {
        self.size(Size::Sm)
            .height(px(chrome::TOOL_OPTION_HEIGHT))
            .padding_x(px(10.0))
            .text_size(px(chrome::BUTTON_XS_TEXT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{div, point, prelude::*, Context, Modifiers, MouseButton, Render, Window};
    use herogpui::components::Variant;
    use herogpui::gpui;
    use herogpui::test::TestWindowExt;

    struct Toolbar;

    impl Render for Toolbar {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let buttons = div()
                .flex()
                .gap(px(4.0))
                .children(
                    [false, false, true]
                        .into_iter()
                        .enumerate()
                        .map(|(i, disabled)| {
                            crate::ui::icon_button::compact(
                                gpui::ElementId::Integer(i as u64),
                                "mouse-pointer-2",
                                "Select",
                            )
                            .variant(Variant::Tertiary)
                            .is_disabled(disabled)
                        }),
                )
                .child(Button::new("labelled").compact().label("Save"))
                .child(
                    crate::ui::icon_button::chip_icon("chip", "settings", "Settings")
                        .variant(Variant::Tertiary),
                );
            crate::ui::window_root::focus_root(buttons, window, cx)
        }
    }

    fn assert_geometry(cx: &mut gpui::VisualTestContext) {
        cx.settle();
        let (quads, scale) = cx.update(|window, _| (window.painted_quads(), window.scale_factor()));
        let controls: Vec<_> = quads
            .iter()
            .filter(|quad| quad.bounds.size.height.0 / scale > 20.0)
            .collect();
        assert!(controls.len() >= 3, "the button skins must be painted");
        for quad in controls {
            assert!(
                quad.bounds.size.height.0 / scale <= 28.0,
                "a compact control must paint inside its 28px slot: {:?}",
                quad.bounds
            );
            if quad.bounds.origin.x.0 / scale < 96.0 {
                assert!(
                    quad.bounds.size.width.0 / scale <= 28.0,
                    "the icon skin must fit between its neighbours: {:?}",
                    quad.bounds
                );
            }
        }
    }

    #[herogpui::test]
    fn compact_skins_fit_their_slots_when_idle_hovered_pressed_and_disabled(
        cx: &mut gpui::TestAppContext,
    ) {
        let (_view, cx) = herogpui::test::open_window(cx, |_, _| Toolbar);
        cx.update(|_, cx| {
            herogpui::theme::set_theme(
                crate::theme::bridge::to_herogpui(&crate::theme::vars::ThemeVars::default(), true),
                cx,
            );
        });
        assert_geometry(cx);
        let at = point(px(14.0), px(14.0));
        cx.simulate_mouse_move(at, None, Modifiers::none());
        assert_geometry(cx);
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        for _ in 0..30 {
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(10));
            cx.update(|window, cx| window.simulate_next_frame(cx));
        }
        assert_geometry(cx);
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        assert_geometry(cx);
    }
}
