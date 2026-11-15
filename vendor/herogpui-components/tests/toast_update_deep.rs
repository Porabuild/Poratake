//! HeroUI v3.2.5 `toast.update`: in-place content replacement.
//!
//! Title and visual fields replace the existing card. `timeout` and `onClose`
//! are inherited unless the builder set them. A missing id falls back to a
//! new toast. The updated card keeps its stack position.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use gpui::TestAppContext;
use herogpui_components::{toast_store, Color, Toast};

fn titles(cx: &mut TestAppContext) -> Vec<String> {
    cx.update(|cx| {
        toast_store(cx)
            .read(cx)
            .toasts()
            .iter()
            .map(|toast| toast.title.to_string())
            .collect()
    })
}

fn ids(cx: &mut TestAppContext) -> Vec<u64> {
    cx.update(|cx| {
        toast_store(cx)
            .read(cx)
            .toasts()
            .iter()
            .map(|toast| toast.id)
            .collect()
    })
}

#[gpui::test]
fn update_replaces_content_without_moving_the_stack(cx: &mut TestAppContext) {
    let (older, newer) = cx.update(|cx| {
        (
            Toast::new("Saving…")
                .is_loading(true)
                .timeout(Duration::ZERO)
                .push(None, cx),
            Toast::new("Later").timeout(Duration::ZERO).push(None, cx),
        )
    });
    assert_eq!(ids(cx), [newer, older]);

    let returned = cx.update(|cx| {
        Toast::new("Saved")
            .variant(Color::Success)
            .update(older, cx)
    });
    assert_eq!(returned, older);
    assert_eq!(ids(cx), [newer, older], "update must keep stack order");

    cx.update(|cx| {
        let older = toast_store(cx)
            .read(cx)
            .toasts()
            .iter()
            .find(|toast| toast.id == older)
            .unwrap();
        assert_eq!(older.title.as_ref(), "Saved");
        assert_eq!(older.color, Color::Success);
        assert!(
            !older.is_loading,
            "omitted isLoading must clear the spinner, matching createContent"
        );
    });
    assert_eq!(titles(cx), ["Later", "Saved"]);
}

#[gpui::test]
fn omitted_timeout_keeps_a_persistent_toast(cx: &mut TestAppContext) {
    let id = cx.update(|cx| Toast::new("Wait").timeout(Duration::ZERO).push(None, cx));
    cx.update(|cx| {
        Toast::new("Still waiting").update(id, cx);
    });
    cx.executor().advance_clock(Duration::from_secs(5));
    assert_eq!(
        ids(cx),
        [id],
        "inheriting timeout must not start a countdown"
    );
}

#[gpui::test]
fn explicit_timeout_restarts_the_countdown(cx: &mut TestAppContext) {
    let id = cx.update(|cx| Toast::new("Wait").timeout(Duration::ZERO).push(None, cx));
    cx.update(|cx| {
        Toast::new("Done")
            .timeout(Duration::from_millis(300))
            .update(id, cx);
    });
    cx.executor().advance_clock(Duration::from_millis(500));
    assert!(
        ids(cx).is_empty(),
        "an explicit timeout must restart the auto-dismiss clock"
    );
}

#[gpui::test]
fn missing_id_falls_back_to_a_new_toast(cx: &mut TestAppContext) {
    let created = cx.update(|cx| Toast::new("Recovered").update(9_001, cx));
    assert_ne!(created, 9_001);
    assert_eq!(titles(cx), ["Recovered"]);
}

#[gpui::test]
fn omitted_on_close_is_inherited_and_replaced_on_close_is_not(cx: &mut TestAppContext) {
    let inherited = Arc::new(AtomicUsize::new(0));
    let replaced = Arc::new(AtomicUsize::new(0));
    let first = cx.update(|cx| {
        let inherited = inherited.clone();
        Toast::new("One")
            .timeout(Duration::ZERO)
            .on_close(move |_| {
                inherited.fetch_add(1, Ordering::SeqCst);
            })
            .push(None, cx)
    });
    cx.update(|cx| {
        Toast::new("Two").update(first, cx);
    });
    cx.update(|cx| herogpui_components::dismiss_toast(first, cx));
    assert_eq!(inherited.load(Ordering::SeqCst), 1);

    let second = cx.update(|cx| {
        let inherited = inherited.clone();
        Toast::new("Three")
            .timeout(Duration::ZERO)
            .on_close(move |_| {
                inherited.fetch_add(1, Ordering::SeqCst);
            })
            .push(None, cx)
    });
    cx.update(|cx| {
        let replaced = replaced.clone();
        Toast::new("Four")
            .on_close(move |_| {
                replaced.fetch_add(1, Ordering::SeqCst);
            })
            .update(second, cx);
    });
    cx.update(|cx| herogpui_components::dismiss_toast(second, cx));
    assert_eq!(inherited.load(Ordering::SeqCst), 1);
    assert_eq!(replaced.load(Ordering::SeqCst), 1);
}
