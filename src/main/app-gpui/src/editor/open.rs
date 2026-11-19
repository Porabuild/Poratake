use anyhow::Context as _;
use gpui::App;
use herogpui::gpui;

use crate::editor::window::{PersistedEditorState, PreparedScreenshot};
use crate::system::clipboard::ClipboardService;
use crate::windows::toast::Toast;

pub fn open_clipboard(cx: &mut App) {
    let Some(image) = ClipboardService::read_image(cx) else {
        eprintln!("[editor] clipboard has no image");
        return;
    };
    let service = crate::state::state(cx);
    cx.spawn(async move |cx| {
        let saved: anyhow::Result<_> = cx
            .background_executor()
            .spawn(async move {
                let buffer = crate::render::images::decode(&image.bytes)
                    .context("The clipboard image could not be decoded")?;
                let path = service.generate_screenshot_path();
                let temporary = crate::capture::prepare_screenshot(&path)
                    .with_context(|| format!("Could not prepare {}", path.display()))?;
                if let Err(error) = buffer.save(&temporary) {
                    let _ = std::fs::remove_file(&temporary);
                    return Err(error).context("Could not write the clipboard image");
                }
                let path = crate::capture::publish_screenshot(&temporary, &path)
                    .with_context(|| {
                        format!("Capture data is available at {}", temporary.display())
                    })?
                    .to_string_lossy()
                    .into_owned();
                let prepared = PreparedScreenshot::open(&path);
                Ok((path, prepared))
            })
            .await;
        cx.update(|cx| match saved {
            Ok((path, prepared)) => {
                crate::open_editor_with_image(
                    cx,
                    &path,
                    Some(PersistedEditorState::default()),
                    prepared,
                );
            }
            Err(error) => {
                eprintln!("[editor] {error:#}");
                Toast::show(cx, "Clipboard Image Not Saved", format!("{error:#}"));
            }
        });
    })
    .detach();
}
