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
}

const WINDOWS_FEATURES: &[Feature] = &[
    Feature::ScreenshotScreen,
    Feature::ScreenshotArea,
    Feature::ScreenshotWindow,
    Feature::Ocr,
    Feature::QrCode,
    Feature::TimerCapture,
    Feature::DesktopIcons,
    Feature::DisplaySelector,
    Feature::DesktopWallpaper,
    Feature::FreezeScreen,
    Feature::ScrollCapture,
    Feature::AllInOne,
    Feature::Print,
    Feature::Recording,
    Feature::VideoEditor,
    Feature::Transcription,
];

const CROSS_PLATFORM_FEATURES: &[Feature] = &[Feature::ScreenshotScreen, Feature::ScreenshotArea];

pub fn is_supported(feature: Feature) -> bool {
    if cfg!(target_os = "macos") {
        return true;
    }
    if cfg!(target_os = "windows") {
        return WINDOWS_FEATURES.contains(&feature);
    }
    CROSS_PLATFORM_FEATURES.contains(&feature)
}
