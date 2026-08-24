use muda::Icon as MenuIcon;

const MENU_ICON_SIZE: u32 = 16;

macro_rules! menu_icons {
    ($($name:literal => $path:literal),* $(,)?) => {
        const MENU_ICON_BYTES: &[(&str, &[u8])] = &[
            $(($name, include_bytes!($path))),*
        ];
    };
}

menu_icons! {
    "aperture" => "../../../../menu/icons/aperture.png",
    "app-window" => "../../../../menu/icons/app-window.png",
    "box" => "../../../../menu/icons/box.png",
    "eye-off" => "../../../../menu/icons/eye-off.png",
    "film" => "../../../../menu/icons/film.png",
    "history" => "../../../../menu/icons/history.png",
    "monitor" => "../../../../menu/icons/monitor.png",
    "monitor-dot" => "../../../../menu/icons/monitor-dot.png",
    "pencil" => "../../../../menu/icons/pencil.png",
    "pin" => "../../../../menu/icons/pin.png",
    "power" => "../../../../menu/icons/power.png",
    "qr-code" => "../../../../menu/icons/qr-code.png",
    "rotate-ccw" => "../../../../menu/icons/rotate-ccw.png",
    "scan" => "../../../../menu/icons/scan.png",
    "scroll" => "../../../../menu/icons/scroll.png",
    "settings" => "../../../../menu/icons/settings.png",
    "text-cursor" => "../../../../menu/icons/text-cursor.png",
    "timer-reset" => "../../../../menu/icons/timer-reset.png",
}

const TRAY_ICON_BYTES: &[u8] = include_bytes!("../../../../../../public/tray-icon.png");

fn decode(bytes: &[u8], size: u32, tint_to_foreground: bool) -> Option<(Vec<u8>, u32, u32)> {
    let decoded = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let mut rgba = image::imageops::resize(
        &decoded.to_rgba8(),
        size,
        size,
        image::imageops::FilterType::CatmullRom,
    );
    if tint_to_foreground {
        for pixel in rgba.pixels_mut() {
            let alpha = pixel.0[3];
            pixel.0[0] = alpha;
            pixel.0[1] = alpha;
            pixel.0[2] = alpha;
        }
    }
    Some((rgba.into_raw(), size, size))
}

pub fn menu_icon(name: &str, dark_mode: bool) -> Option<MenuIcon> {
    let bytes = MENU_ICON_BYTES
        .iter()
        .find(|(id, _)| *id == name)
        .map(|(_, bytes)| *bytes)?;
    let (rgba, width, height) = decode(bytes, MENU_ICON_SIZE, dark_mode)?;
    MenuIcon::from_rgba(rgba, width, height).ok()
}

pub fn tray_icon() -> Option<tray_icon::Icon> {
    let decoded =
        image::load_from_memory_with_format(TRAY_ICON_BYTES, image::ImageFormat::Png).ok()?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height).ok()
}
