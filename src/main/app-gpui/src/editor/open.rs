use gpui::App;

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
    if let Some(parent) = path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            eprintln!("[editor] failed to create {}: {error}", parent.display());
            return;
        }
    }
    if let Err(error) = buffer.save(&path) {
        eprintln!("[editor] failed to write {}: {error}", path.display());
        return;
    }

    crate::open_editor_for(cx, path.to_string_lossy().as_ref());
}
