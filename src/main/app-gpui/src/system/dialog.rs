use rfd::{AsyncMessageDialog, MessageLevel};

pub async fn alert(level: MessageLevel, title: &str, detail: &str) {
    let _ = AsyncMessageDialog::new()
        .set_level(level)
        .set_title(title)
        .set_description(detail)
        .show()
        .await;
}
