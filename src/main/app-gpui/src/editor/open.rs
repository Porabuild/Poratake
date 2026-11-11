use gpui::App;
use herogpui::gpui;

pub fn open_clipboard(cx: &mut App) {
    let Some(image) = crate::system::clipboard::ClipboardService::read_image(cx) else {
        eprintln!("[editor] clipboard has no image");
        return;
    };
    let Ok(buffer) = image::load_from_memory(&image.bytes) else {
        eprintln!("[editor] clipboard image could not be decoded");
        return;
    };

    let path = crate::state::state(cx).generate_screenshot_path();
    let temporary = match crate::capture::prepare_screenshot(&path) {
        Ok(temporary) => temporary,
        Err(error) => {
            eprintln!("[editor] failed to prepare {}: {error}", path.display());
            return;
        }
    };
    if let Err(error) = buffer.save(&temporary) {
        eprintln!("[editor] failed to write {}: {error}", temporary.display());
        let _ = std::fs::remove_file(temporary);
        return;
    }
    let path = match crate::capture::publish_screenshot(&temporary, &path) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("[editor] failed to save {}: {error}", temporary.display());
            crate::windows::toast::Toast::show(
                cx,
                "Clipboard Image Not Saved",
                format!("Capture data is available at {}", temporary.display()),
            );
            return;
        }
    };

    crate::open_editor_for(cx, path.to_string_lossy().as_ref());
}
