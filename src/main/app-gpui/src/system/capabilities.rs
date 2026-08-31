include!("capabilities.generated.rs");

pub fn is_supported(feature: Feature) -> bool {
    #[cfg(target_os = "macos")]
    return MACOS_FEATURES.contains(&feature);
    #[cfg(target_os = "windows")]
    return WINDOWS_FEATURES.contains(&feature);
    #[cfg(target_os = "linux")]
    return match crate::system::linux_session::current() {
        crate::system::linux_session::LinuxSession::X11 => x11_supported(feature, || {
            crate::system::linux_session::capabilities().ffmpeg_encoder
        }),
        crate::system::linux_session::LinuxSession::Wayland => wayland_supported(
            feature,
            crate::system::linux_session::capabilities().screen_cast,
        ),
        crate::system::linux_session::LinuxSession::Headless => {
            HEADLESS_FEATURES.contains(&feature)
        }
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    false
}

#[cfg(target_os = "linux")]
fn wayland_supported(feature: Feature, screen_cast: bool) -> bool {
    if !LINUX_WAYLAND_FEATURES.contains(&feature) {
        return false;
    }
    // Print and the video editor never touch the screen — they work from
    // files — so they are the only features exempt from the portal probe.
    if matches!(feature, Feature::Print | Feature::VideoEditor) {
        return true;
    }
    screen_cast
}

/// X11 capture reaches the daemon's native X11 paths, but recording also
/// needs an H.264-capable FFmpeg, which minimal distributions may not ship.
/// The encoder verdict is a closure so routine feature checks never pay for
/// the probe subprocess.
#[cfg(target_os = "linux")]
fn x11_supported(feature: Feature, ffmpeg_encoder: impl FnOnce() -> bool) -> bool {
    if !LINUX_X11_FEATURES.contains(&feature) {
        return false;
    }
    if feature == Feature::Recording {
        return ffmpeg_encoder();
    }
    true
}

pub fn has_native_daemon() -> bool {
    cfg!(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    ))
}

pub fn global_shortcuts_supported() -> bool {
    #[cfg(target_os = "linux")]
    return crate::system::linux_session::current()
        == crate::system::linux_session::LinuxSession::X11;
    #[cfg(not(target_os = "linux"))]
    true
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn linux_capabilities_match_the_session_backends() {
        assert!(LINUX_X11_FEATURES.contains(&Feature::ScreenshotWindow));
        assert!(!LINUX_WAYLAND_FEATURES.contains(&Feature::ScreenshotWindow));
        assert!(LINUX_WAYLAND_FEATURES.contains(&Feature::ScreenshotScreen));
        assert!(LINUX_X11_FEATURES.contains(&Feature::QrCode));
        assert!(LINUX_WAYLAND_FEATURES.contains(&Feature::QrCode));
        assert!(LINUX_X11_FEATURES.contains(&Feature::TimerCapture));
        assert!(!LINUX_WAYLAND_FEATURES.contains(&Feature::TimerCapture));
        assert!(LINUX_X11_FEATURES.contains(&Feature::ScrollCapture));
        assert!(!LINUX_WAYLAND_FEATURES.contains(&Feature::ScrollCapture));
        assert!(LINUX_X11_FEATURES.contains(&Feature::FreezeScreen));
        assert!(!LINUX_WAYLAND_FEATURES.contains(&Feature::FreezeScreen));
        assert!(LINUX_X11_FEATURES.contains(&Feature::DisplaySelector));
        assert!(!LINUX_WAYLAND_FEATURES.contains(&Feature::DisplaySelector));
        assert!(LINUX_X11_FEATURES.contains(&Feature::Print));
        assert!(LINUX_WAYLAND_FEATURES.contains(&Feature::Print));
        assert!(LINUX_X11_FEATURES.contains(&Feature::DesktopWallpaper));
        assert!(!LINUX_WAYLAND_FEATURES.contains(&Feature::DesktopWallpaper));
        assert!(wayland_supported(Feature::ScreenshotArea, true));
        assert!(!wayland_supported(Feature::ScreenshotArea, false));
        assert!(wayland_supported(Feature::Print, false));
        assert!(wayland_supported(Feature::Recording, true));
        assert!(!wayland_supported(Feature::Recording, false));
        assert!(wayland_supported(Feature::VideoEditor, false));
        assert!(LINUX_WAYLAND_FEATURES.contains(&Feature::VideoEditor));
        assert!(LINUX_X11_FEATURES.contains(&Feature::VideoEditor));
        assert!(x11_supported(Feature::Recording, || true));
        assert!(!x11_supported(Feature::Recording, || false));
        assert!(x11_supported(Feature::Print, || false));
        // Wayland recording is in the matrix; the runtime ScreenCast probe
        // above is what keeps it honest per session.
        assert!(LINUX_WAYLAND_FEATURES.contains(&Feature::Recording));
    }
}
