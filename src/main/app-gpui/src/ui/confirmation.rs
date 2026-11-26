use gpui::{
    div, prelude::*, px, size, AnyElement, App, Bounds, Context, IntoElement, Render, SharedString,
    Window, WindowBackgroundAppearance,
};
use herogpui::components::{Button, Modal, ModalSize, Size, Variant};
use herogpui::gpui;

use crate::theme::vars::active_theme;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfirmationTone {
    Danger,
    Primary,
}

impl ConfirmationTone {
    fn variant(self) -> Variant {
        match self {
            Self::Danger => Variant::Danger,
            Self::Primary => Variant::Primary,
        }
    }
}

#[derive(Clone)]
struct ConfirmationView {
    title: SharedString,
    detail: SharedString,
    action: SharedString,
    tone: ConfirmationTone,
    sender: smol::channel::Sender<bool>,
}

pub struct Confirmation(ConfirmationView);

impl Confirmation {
    pub fn new(
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        action: impl Into<SharedString>,
        tone: ConfirmationTone,
    ) -> (Self, smol::channel::Receiver<bool>) {
        let (sender, receiver) = smol::channel::bounded(1);
        let confirmation = Self(ConfirmationView {
            title: title.into(),
            detail: detail.into(),
            action: action.into(),
            tone,
            sender,
        });
        (confirmation, receiver)
    }

    fn view(&self) -> ConfirmationView {
        self.0.clone()
    }

    fn resolve(&self, confirmed: bool) {
        let _ = self.0.sender.try_send(confirmed);
        self.0.sender.close();
    }
}

impl Drop for Confirmation {
    fn drop(&mut self) {
        self.resolve(false);
    }
}

pub fn render(
    confirmation: Option<&Confirmation>,
    id: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let held = window.use_keyed_state(
        gpui::ElementId::Name(format!("{id}-view").into()),
        cx,
        |_, _| None::<ConfirmationView>,
    );
    if let Some(confirmation) = confirmation {
        held.update(cx, |held, _| *held = Some(confirmation.view()));
    }
    let Some(view) = held.read(cx).clone() else {
        return Modal::new().id(id).into_any_element();
    };
    let open = confirmation.is_some();
    let escape = view.sender.clone();
    let cancel = view.sender.clone();
    let confirm = view.sender.clone();
    Modal::new()
        .id(id)
        .is_open(open)
        .title(view.title.clone())
        .size(ModalSize::Sm)
        .is_dismissible(false)
        .on_close(move |_, _, _| {
            let _ = escape.try_send(false);
            escape.close();
        })
        .child(div().child(view.detail.clone()))
        .footer_child(
            div().debug_selector(|| "confirmation-cancel".into()).child(
                Button::new(SharedString::from(format!("{id}-cancel")))
                    .label("Cancel")
                    .variant(Variant::Ghost)
                    .recipe("muted-action")
                    .size(Size::Sm)
                    .on_press(move |_, _, _| {
                        let _ = cancel.try_send(false);
                        cancel.close();
                    }),
            ),
        )
        .footer_child(
            div()
                .debug_selector(|| "confirmation-confirm".into())
                .child(
                    Button::new(SharedString::from(format!("{id}-confirm")))
                        .label(view.action.clone())
                        .variant(view.tone.variant())
                        .size(Size::Sm)
                        .on_press(move |_, _, _| {
                            let _ = confirm.try_send(true);
                            confirm.close();
                        }),
                ),
        )
        .into_any_element()
}

pub fn present(
    cx: &mut App,
    id: &'static str,
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    action: impl Into<SharedString>,
    tone: ConfirmationTone,
    on_resolved: impl FnOnce(bool, &mut App) + 'static,
) {
    let (confirmation, receiver) = Confirmation::new(title, detail, action, tone);
    let bounds = Bounds::centered(None, size(px(480.0), px(360.0)), cx);
    let options = crate::windows::popup_window_options(
        bounds,
        crate::windows::PopupWindowConfig {
            focus: true,
            show: true,
            movable: false,
            resizable: false,
            display_id: None,
            background: WindowBackgroundAppearance::Opaque,
        },
    );
    let opened = cx.open_window(options, |_, cx| {
        cx.new(|_| ConfirmationWindow { id, confirmation })
    });
    let handle = match opened {
        Ok(handle) => handle,
        Err(_) => {
            on_resolved(false, cx);
            return;
        }
    };
    cx.spawn(async move |cx| {
        let confirmed = receiver.recv().await.unwrap_or(false);
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        cx.update(|cx| on_resolved(confirmed, cx));
    })
    .detach();
}

struct ConfirmationWindow {
    id: &'static str,
    confirmation: Confirmation,
}

impl Render for ConfirmationWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let content = crate::ui::font::root()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(render(Some(&self.confirmation), self.id, window, cx));
        crate::ui::window_root::focus_root(content, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use herogpui::test::TestWindowExt;

    struct Host {
        confirmation: Option<Confirmation>,
    }

    impl Render for Host {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let modal = render(self.confirmation.as_ref(), "test-confirmation", window, cx);
            div().size_full().child(modal)
        }
    }

    fn reopen(
        host: &gpui::Entity<Host>,
        cx: &mut gpui::VisualTestContext,
    ) -> smol::channel::Receiver<bool> {
        let (confirmation, receiver) = Confirmation::new(
            "Delete Video",
            "Delete Sample Recording? This removes the recording and all of its assets.",
            "Delete",
            ConfirmationTone::Danger,
        );
        host.update(cx, |host, cx| {
            host.confirmation = Some(confirmation);
            cx.notify();
        });
        receiver
    }

    #[herogpui::test]
    fn footer_controls_escape_cancel_and_confirm_use_the_shared_modal(cx: &mut TestAppContext) {
        let (confirmation, _) = Confirmation::new(
            "Delete Video",
            "Delete Sample Recording? This removes the recording and all of its assets.",
            "Delete",
            ConfirmationTone::Danger,
        );
        let (host, cx) = herogpui::test::open_window(cx, move |_, _| Host {
            confirmation: Some(confirmation),
        });
        cx.activate_test_a11y();
        cx.refresh().unwrap();
        cx.run_until_parked();

        let tree = cx.test_a11y_tree().expect("accessibility tree");
        let labels: Vec<&str> = tree
            .nodes
            .iter()
            .filter_map(|(_, node)| node.label())
            .collect();
        assert!(labels.contains(&"Delete Video"), "{labels:?}");
        assert!(labels.contains(&"Cancel"), "{labels:?}");
        assert!(labels.contains(&"Delete"), "{labels:?}");
        assert!(
            tree.nodes.iter().any(|(_, node)| {
                node.role() == gpui::accesskit::Role::Dialog && node.label() == Some("Delete Video")
            }),
            "the modal dialog must be named by its title"
        );
        assert!(cx.find("confirmation-cancel").is_some());
        assert!(cx.find("confirmation-confirm").is_some());

        let cancel = reopen(&host, cx);
        cx.click("confirmation-cancel");
        assert_eq!(cancel.try_recv(), Ok(false));

        let escape = reopen(&host, cx);
        cx.press("escape");
        assert_eq!(escape.try_recv(), Ok(false));

        let confirm = reopen(&host, cx);
        cx.click("confirmation-confirm");
        assert_eq!(confirm.try_recv(), Ok(true));
    }

    #[test]
    fn dropping_the_owner_resolves_cancel() {
        let (confirmation, receiver) = Confirmation::new(
            "Delete Video",
            "Delete Sample Recording?",
            "Delete",
            ConfirmationTone::Danger,
        );
        drop(confirmation);
        assert_eq!(smol::block_on(receiver.recv()), Ok(false));
    }
}
