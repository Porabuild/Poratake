use gpui::App;

pub fn open_clipboard(cx: &mut App) {
    let image = match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_image()) {
        Ok(image) => image,
        Err(error) => {
            eprintln!("[editor] clipboard has no image: {error}");
            return;
        }
    };

    let Some(buffer) = image::RgbaImage::from_raw(
        image.width as u32,
        image.height as u32,
        image.bytes.into_owned(),
    ) else {
        eprintln!("[editor] clipboard image had an unexpected stride");
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
