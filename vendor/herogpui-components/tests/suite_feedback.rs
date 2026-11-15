//! Integration tests grouped by feature area to keep link time bounded.

#[path = "harness/mod.rs"]
mod harness;
#[path = "source_scan/mod.rs"]
mod source_scan;

#[path = "alert_deep.rs"]
mod alert_deep;
#[path = "avatar_deep.rs"]
mod avatar_deep;
#[path = "avatar_group_deep.rs"]
mod avatar_group_deep;
#[path = "card_deep.rs"]
mod card_deep;
#[path = "feedback.rs"]
mod feedback;
#[path = "feedback_compose.rs"]
mod feedback_compose;
#[path = "icon.rs"]
mod icon;
#[path = "kbd_deep.rs"]
mod kbd_deep;
#[path = "toast_promise_deep.rs"]
mod toast_promise_deep;
#[path = "toast_stack_deep.rs"]
mod toast_stack_deep;
#[path = "toast_update_deep.rs"]
mod toast_update_deep;
