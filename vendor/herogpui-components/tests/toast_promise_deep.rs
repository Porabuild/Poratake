//! HeroUI v3.2.5 `toast.promise`: loading toast updates in place.

use std::time::Duration;

use gpui::{SharedString, TestAppContext};
use herogpui_components::{toast_store, Color, Toast, DEFAULT_TOAST_TIMEOUT};

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

fn card(cx: &mut TestAppContext, id: u64) -> (String, Color, bool) {
    cx.update(|cx| {
        let toast = toast_store(cx)
            .read(cx)
            .toasts()
            .iter()
            .find(|toast| toast.id == id)
            .unwrap();
        (toast.title.to_string(), toast.color, toast.is_loading)
    })
}

#[gpui::test]
fn success_updates_the_loading_toast_in_place(cx: &mut TestAppContext) {
    let (promise, later) = cx.update(|cx| {
        let promise = Toast::promise(
            async { Ok::<SharedString, SharedString>("Uploaded".into()) },
            "Uploading…",
            cx,
        );
        let later = Toast::new("Later").timeout(Duration::ZERO).push(None, cx);
        (promise, later)
    });
    assert_eq!(ids(cx), [later, promise]);
    assert_eq!(
        card(cx, promise),
        ("Uploading…".into(), Color::Default, true)
    );

    cx.run_until_parked();
    assert_eq!(ids(cx), [later, promise], "settle must keep stack order");
    assert_eq!(
        card(cx, promise),
        ("Uploaded".into(), Color::Success, false),
        "Ok must replace the same card as success and drop the spinner"
    );
    assert_eq!(titles(cx), ["Later", "Uploaded"]);
}

#[gpui::test]
fn error_updates_the_loading_toast_as_danger(cx: &mut TestAppContext) {
    let id = cx.update(|cx| {
        Toast::promise(
            async { Err::<SharedString, SharedString>("The date is in the past".into()) },
            "Creating event…",
            cx,
        )
    });
    cx.run_until_parked();
    assert_eq!(ids(cx), [id]);
    assert_eq!(
        card(cx, id),
        ("The date is in the past".into(), Color::Danger, false)
    );
}

#[gpui::test]
fn owned_promise_can_cancel_work_without_changing_queue_clear_semantics(cx: &mut TestAppContext) {
    let (cancelled, retained, task) = cx.update(|cx| {
        let executor = cx.background_executor().clone();
        let (cancelled, task) = Toast::promise_task(
            async move {
                executor.timer(Duration::from_millis(400)).await;
                Ok("Cancelled result".into())
            },
            "Owned loading",
            cx,
        );
        let executor = cx.background_executor().clone();
        let retained = Toast::promise(
            async move {
                executor.timer(Duration::from_millis(400)).await;
                Ok("Retained result".into())
            },
            "Detached loading",
            cx,
        );
        (cancelled, retained, task)
    });
    cx.run_until_parked();
    drop(task);
    assert_eq!(
        ids(cx),
        [retained, cancelled],
        "cancellation leaves queue cleanup to the owner"
    );
    cx.update(herogpui_components::clear_toasts);
    cx.executor().advance_clock(Duration::from_millis(400));
    cx.run_until_parked();
    assert_eq!(
        titles(cx),
        ["Retained result"],
        "the detached promise still upserts after clear, as in HeroUI"
    );
}

#[gpui::test]
fn loading_stays_until_the_future_settles(cx: &mut TestAppContext) {
    let id = cx.update(|cx| {
        let executor = cx.background_executor().clone();
        Toast::promise(
            async move {
                executor.timer(Duration::from_millis(400)).await;
                Ok::<SharedString, SharedString>("Done".into())
            },
            "Wait",
            cx,
        )
    });
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    assert_eq!(
        card(cx, id),
        ("Wait".into(), Color::Default, true),
        "the loading toast must remain until the future settles"
    );

    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    assert_eq!(card(cx, id), ("Done".into(), Color::Success, false));
}

#[gpui::test]
fn settle_starts_the_default_dismiss_clock(cx: &mut TestAppContext) {
    let id = cx.update(|cx| {
        Toast::promise(
            async { Ok::<SharedString, SharedString>("Saved".into()) },
            "Saving…",
            cx,
        )
    });
    cx.run_until_parked();
    assert_eq!(ids(cx), [id]);
    assert_eq!(DEFAULT_TOAST_TIMEOUT, Duration::from_secs(4));

    cx.executor().advance_clock(Duration::from_secs(3));
    cx.run_until_parked();
    assert_eq!(ids(cx), [id], "the default clock must still be running");

    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert!(
        ids(cx).is_empty(),
        "the settled toast must auto-dismiss after the default timeout"
    );
}
