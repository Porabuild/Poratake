use std::{cell::RefCell, sync::Arc};

use gpui::{AnyElement, App, ElementId, IntoElement, ParentElement, SharedString, Window};
use herogpui::components::{Tooltip, TooltipPresentationRequest, TooltipPresenter};
use herogpui::gpui;

struct Presentation {
    key: ElementId,
    content: SharedString,
    bounds: gpui::Bounds<gpui::Pixels>,
    scale: f32,
    size: Option<gpui::Size<gpui::Pixels>>,
    placement: herogpui::components::TooltipPlacement,
    offset: gpui::Pixels,
}

#[derive(Default)]
pub struct NativeTooltipPresenter {
    active: RefCell<Option<Presentation>>,
    #[cfg(windows)]
    surface: RefCell<Option<crate::system::native_tooltip::WindowsTooltip>>,
    #[cfg(all(target_os = "macos", not(test)))]
    surface: RefCell<Option<crate::system::tooltip_macos::MacTooltip>>,
    #[cfg(all(test, windows))]
    owner: &'static str,
}

impl NativeTooltipPresenter {
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new(_owner: &'static str) -> Arc<Self> {
        Arc::new(Self {
            #[cfg(all(test, windows))]
            owner: _owner,
            ..Self::default()
        })
    }

    #[cfg(any(not(target_os = "macos"), test))]
    fn attach(self: &Arc<Self>, tooltip: Tooltip) -> Tooltip {
        #[cfg(any(windows, all(test, target_os = "macos")))]
        return tooltip
            .recipe("native")
            .presenter(self.clone())
            .exclude_from_capture(true);
        #[cfg(not(any(windows, all(test, target_os = "macos"))))]
        tooltip
    }

    pub fn wrap(
        self: &Arc<Self>,
        _key: ElementId,
        _content: SharedString,
        tooltip: Tooltip,
        child: impl IntoElement,
    ) -> AnyElement {
        #[cfg(target_os = "macos")]
        return Tooltip::new(_content)
            .id((_key, "appkit-hover-tooltip"))
            .recipe("appkit-hover")
            .trigger(herogpui::components::TooltipTrigger::Hover)
            .presenter(self.clone())
            .child(
                tooltip
                    .trigger(herogpui::components::TooltipTrigger::Focus)
                    .child(child),
            )
            .into_any_element();
        #[cfg(not(target_os = "macos"))]
        self.attach(tooltip).child(child).into_any_element()
    }

    #[cfg(all(any(windows, target_os = "macos"), any(test, feature = "parity")))]
    pub fn active_content(&self) -> Option<SharedString> {
        self.active
            .borrow()
            .as_ref()
            .map(|active| active.content.clone())
    }
}

impl TooltipPresenter for NativeTooltipPresenter {
    fn present(&self, request: &TooltipPresentationRequest, _window: &mut Window, _cx: &mut App) {
        let mut active = self.active.borrow_mut();
        if active.as_ref().is_some_and(|active| {
            active.key == request.key
                && active.content == request.content
                && active.bounds == request.trigger_screen_bounds
                && active.scale == request.scale_factor
                && active.size == request.tip_size
                && active.placement == request.placement
                && active.offset == request.offset
        }) {
            return;
        }
        #[cfg(all(test, windows))]
        crate::system::native_tooltip::record_test_show(self.owner, &request.content);
        #[cfg(windows)]
        {
            let mut surface = self.surface.borrow_mut();
            if surface.is_none() {
                *surface = crate::system::native_tooltip::WindowsTooltip::new(_window);
            }
            if let Some(surface) = surface.as_mut() {
                surface.show(_window, request);
            }
        }
        #[cfg(all(target_os = "macos", not(test)))]
        {
            let mut surface = self.surface.borrow_mut();
            if surface.is_none() {
                *surface = crate::system::tooltip_macos::MacTooltip::new(_window);
            }
            if let Some(surface) = surface.as_mut() {
                surface.show(_window, request);
            }
        }
        #[cfg(all(target_os = "macos", test))]
        crate::windows::scroll_capture::set_native_tooltip(_window, Some(&request.content));
        *active = Some(Presentation {
            key: request.key.clone(),
            content: request.content.clone(),
            bounds: request.trigger_screen_bounds,
            scale: request.scale_factor,
            size: request.tip_size,
            placement: request.placement,
            offset: request.offset,
        });
    }

    fn dismiss(&self, key: &ElementId, _window: Option<&mut Window>, _cx: &mut App) {
        let mut active = self.active.borrow_mut();
        if !active.as_ref().is_some_and(|active| &active.key == key) {
            return;
        }
        active.take();
        #[cfg(all(target_os = "macos", test))]
        if let Some(window) = _window {
            crate::windows::scroll_capture::set_native_tooltip(window, None);
        }
        #[cfg(all(test, windows))]
        crate::system::native_tooltip::record_test_hide(self.owner);
        #[cfg(any(windows, all(target_os = "macos", not(test))))]
        if let Some(surface) = self.surface.borrow_mut().as_mut() {
            surface.hide();
        }
    }
}

#[cfg(all(test, any(windows, target_os = "macos")))]
mod tests {
    use std::time::Duration;

    use gpui::{
        div, point, prelude::*, px, Context, IntoElement, Modifiers, Render, TestAppContext,
    };
    use herogpui::components::Button;
    use herogpui::test::TestWindowExt;

    use super::*;

    struct Host {
        presenter: Arc<NativeTooltipPresenter>,
        visible: bool,
        platform_tooltip: bool,
        label_first: bool,
    }

    impl Render for Host {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let content = div()
                .p(px(20.))
                .flex()
                .gap(px(30.))
                .when(self.visible, |el| {
                    el.children(
                        [("first", "Copy"), ("second", "Delete")].map(|(id, label)| {
                            let tooltip = Tooltip::new(label).id(id);
                            let child = if self.label_first && id == "first" {
                                div().id(id).tab_index(0).child(label).into_any_element()
                            } else {
                                Button::new(id).label(label).into_any_element()
                            };
                            let trigger = div().debug_selector(move || id.into()).child(child);
                            if self.platform_tooltip {
                                self.presenter
                                    .wrap(id.into(), label.into(), tooltip, trigger)
                            } else {
                                self.presenter
                                    .attach(tooltip)
                                    .child(trigger)
                                    .into_any_element()
                            }
                        }),
                    )
                });
            crate::ui::window_root::focus_root(content, window, cx)
        }
    }

    fn settle(visual: &mut gpui::VisualTestContext) {
        for _ in 0..3 {
            visual.settle();
        }
    }

    #[herogpui::test]
    fn upstream_lifecycle_drives_native_tooltip_surfaces(cx: &mut TestAppContext) {
        let presenter = NativeTooltipPresenter::new("test");
        let host_presenter = presenter.clone();
        let (host, visual) = herogpui::test::open_window(cx, move |_, _| Host {
            presenter: host_presenter,
            visible: true,
            platform_tooltip: false,
            label_first: false,
        });
        visual.update(|_, cx| {
            herogpui::theme::set_reduce_motion(true, cx);
            crate::theme::vars::init_theme(
                cx,
                crate::theme::presets::ThemeMode::Light,
                crate::theme::presets::DEFAULT_THEME_ID,
            );
        });
        let first = visual.expect("first").center();
        let second = visual.expect("second").center();
        let outside = point(px(500.), px(500.));
        visual.simulate_mouse_move(first, None, Modifiers::none());
        settle(visual);
        visual.executor().advance_clock(Duration::from_millis(149));
        settle(visual);
        assert!(presenter.active_content().is_none());
        visual.simulate_mouse_move(outside, None, Modifiers::none());
        settle(visual);
        visual.executor().advance_clock(Duration::from_millis(200));
        settle(visual);
        assert!(presenter.active_content().is_none());

        visual.simulate_mouse_move(first, None, Modifiers::none());
        settle(visual);
        visual.executor().advance_clock(Duration::from_millis(150));
        settle(visual);
        assert_eq!(presenter.active_content().as_deref(), Some("Copy"));
        visual.simulate_mouse_move(second, None, Modifiers::none());
        settle(visual);
        assert_eq!(presenter.active_content().as_deref(), Some("Delete"));
        visual.press("escape");
        assert!(presenter.active_content().is_none());
        visual.executor().advance_clock(Duration::from_millis(300));
        settle(visual);
        assert!(presenter.active_content().is_none());

        visual.simulate_mouse_move(outside, None, Modifiers::none());
        visual.press("tab");
        assert_eq!(presenter.active_content().as_deref(), Some("Copy"));
        visual.press("enter");
        assert!(presenter.active_content().is_none());
        visual.press("tab");
        assert_eq!(presenter.active_content().as_deref(), Some("Delete"));
        host.update(visual, |host, cx| {
            host.visible = false;
            cx.notify();
        });
        settle(visual);
        assert!(presenter.active_content().is_none());
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn appkit_hover_uses_upstream_cleanup_and_keeps_focus_in_the_owner(cx: &mut TestAppContext) {
        let presenter = NativeTooltipPresenter::new("test");
        let host_presenter = presenter.clone();
        let (host, visual) = herogpui::test::open_window(cx, move |_, _| Host {
            presenter: host_presenter,
            visible: true,
            platform_tooltip: true,
            label_first: false,
        });
        visual.update(|_, cx| {
            herogpui::theme::set_reduce_motion(true, cx);
            crate::theme::vars::init_theme(
                cx,
                crate::theme::presets::ThemeMode::Light,
                crate::theme::presets::DEFAULT_THEME_ID,
            );
        });
        let first = visual.expect("first").center();
        let second = visual.expect("second").center();
        let outside = point(px(500.), px(500.));
        visual.simulate_mouse_move(first, None, Modifiers::none());
        settle(visual);
        assert_eq!(presenter.active_content().as_deref(), Some("Copy"));
        visual.simulate_mouse_move(second, None, Modifiers::none());
        settle(visual);
        assert_eq!(presenter.active_content().as_deref(), Some("Delete"));
        visual.press("escape");
        assert!(presenter.active_content().is_none());
        visual.simulate_mouse_move(outside, None, Modifiers::none());
        visual.press("tab");
        assert!(presenter.active_content().is_none());
        assert!(visual.update(|window, cx| window.focused(cx).is_some()));
        settle(visual);
        assert!(visual.update(|window, cx| {
            let background = gpui::Background::from(crate::theme::vars::active_theme(cx).overlay);
            window
                .painted_quads()
                .into_iter()
                .any(|quad| quad.background == background)
        }));
        visual.simulate_mouse_move(first, None, Modifiers::none());
        settle(visual);
        assert_eq!(presenter.active_content().as_deref(), Some("Copy"));
        host.update(visual, |host, cx| {
            host.visible = false;
            cx.notify();
        });
        settle(visual);
        assert!(presenter.active_content().is_none());
    }

    #[herogpui::test]
    fn external_presenter_keeps_a_focusable_label_as_the_first_tab_stop(cx: &mut TestAppContext) {
        let presenter = NativeTooltipPresenter::new("test");
        let host_presenter = presenter.clone();
        let (_host, visual) = herogpui::test::open_window(cx, move |_, _| Host {
            presenter: host_presenter,
            visible: true,
            platform_tooltip: false,
            label_first: true,
        });
        visual.press("tab");
        assert_eq!(presenter.active_content().as_deref(), Some("Copy"));
        visual.press("tab");
        assert_eq!(presenter.active_content().as_deref(), Some("Delete"));
    }
}
