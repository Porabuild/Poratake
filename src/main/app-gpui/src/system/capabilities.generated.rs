#[cfg_attr(target_os = "linux", allow(dead_code))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Feature {
    ScreenshotScreen,
    ScreenshotArea,
    ScreenshotWindow,
    Ocr,
    QrCode,
    TimerCapture,
    ScrollCapture,
    AllInOne,
    Recording,
    VideoEditor,
    DesktopIcons,
    FreezeScreen,
    DisplaySelector,
    Print,
    DesktopWallpaper,
    Transcription,
    CaptureSound,
    ColorPicker,
}

#[cfg(target_os = "macos")]
pub const MACOS_FEATURES: &[Feature] = &[
    Feature::ScreenshotScreen,
    Feature::ScreenshotArea,
    Feature::ScreenshotWindow,
    Feature::Ocr,
    Feature::QrCode,
    Feature::TimerCapture,
    Feature::ScrollCapture,
    Feature::AllInOne,
    Feature::Recording,
    Feature::VideoEditor,
    Feature::DesktopIcons,
    Feature::FreezeScreen,
    Feature::DisplaySelector,
    Feature::Print,
    Feature::DesktopWallpaper,
    Feature::Transcription,
    Feature::CaptureSound,
    Feature::ColorPicker,
];

#[cfg(target_os = "windows")]
pub const WINDOWS_FEATURES: &[Feature] = &[
    Feature::ScreenshotScreen,
    Feature::ScreenshotArea,
    Feature::ScreenshotWindow,
    Feature::Ocr,
    Feature::QrCode,
    Feature::TimerCapture,
    Feature::ScrollCapture,
    Feature::AllInOne,
    Feature::Recording,
    Feature::VideoEditor,
    Feature::DesktopIcons,
    Feature::FreezeScreen,
    Feature::DisplaySelector,
    Feature::Print,
    Feature::DesktopWallpaper,
    Feature::Transcription,
    Feature::ColorPicker,
];

#[cfg(target_os = "linux")]
pub const LINUX_X11_FEATURES: &[Feature] = &[
    Feature::ScreenshotScreen,
    Feature::ScreenshotArea,
    Feature::ScreenshotWindow,
    Feature::QrCode,
    Feature::TimerCapture,
    Feature::ScrollCapture,
    Feature::AllInOne,
    Feature::Recording,
    Feature::VideoEditor,
    Feature::FreezeScreen,
    Feature::DisplaySelector,
    Feature::Print,
    Feature::DesktopWallpaper,
    Feature::ColorPicker,
];

#[cfg(target_os = "linux")]
pub const LINUX_WAYLAND_FEATURES: &[Feature] = &[
    Feature::ScreenshotScreen,
    Feature::ScreenshotArea,
    Feature::QrCode,
    Feature::AllInOne,
    Feature::Recording,
    Feature::VideoEditor,
    Feature::Print,
];

#[cfg(target_os = "linux")]
pub const HEADLESS_FEATURES: &[Feature] = &[];
