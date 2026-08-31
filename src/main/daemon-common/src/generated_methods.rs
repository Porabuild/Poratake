pub const AREA_SELECTOR_MODULE: &str = "area-selector";
pub const CAMERA_PREVIEW_MODULE: &str = "camera-preview";
pub const DESKTOP_HELPER_MODULE: &str = "desktop-helper";
pub const DESKTOP_WALLPAPER_MODULE: &str = "desktop-wallpaper";
pub const DISPLAY_SELECTOR_MODULE: &str = "display-selector";
pub const FREEZE_SCREEN_MODULE: &str = "freeze-screen";
pub const MEDIA_DEVICES_MODULE: &str = "media-devices";
pub const OCR_MODULE: &str = "ocr";
pub const PRINT_MODULE: &str = "print";
pub const QRCODE_MODULE: &str = "qrcode";
pub const RECORDING_CONTROL_MODULE: &str = "recording-control";
pub const RECORDING_OVERLAY_MODULE: &str = "recording-overlay";
pub const SCREENSHOT_MODULE: &str = "screenshot";
pub const SCREEN_RECORDER_MODULE: &str = "screen-recorder";
pub const SCROLL_CAPTURE_MODULE: &str = "scroll-capture";
pub const TIMER_CONTROL_MODULE: &str = "timer-control";
pub const WINDOW_SELECTOR_MODULE: &str = "window-selector";

pub const MACOS_MODULES: &[&str] = &[
    AREA_SELECTOR_MODULE,
    CAMERA_PREVIEW_MODULE,
    DESKTOP_HELPER_MODULE,
    DESKTOP_WALLPAPER_MODULE,
    DISPLAY_SELECTOR_MODULE,
    FREEZE_SCREEN_MODULE,
    MEDIA_DEVICES_MODULE,
    OCR_MODULE,
    PRINT_MODULE,
    QRCODE_MODULE,
    RECORDING_CONTROL_MODULE,
    RECORDING_OVERLAY_MODULE,
    SCREENSHOT_MODULE,
    SCREEN_RECORDER_MODULE,
    SCROLL_CAPTURE_MODULE,
    TIMER_CONTROL_MODULE,
    WINDOW_SELECTOR_MODULE,
];

pub const WINDOWS_MODULES: &[&str] = &[
    AREA_SELECTOR_MODULE,
    CAMERA_PREVIEW_MODULE,
    DESKTOP_HELPER_MODULE,
    DESKTOP_WALLPAPER_MODULE,
    DISPLAY_SELECTOR_MODULE,
    FREEZE_SCREEN_MODULE,
    MEDIA_DEVICES_MODULE,
    OCR_MODULE,
    PRINT_MODULE,
    QRCODE_MODULE,
    RECORDING_CONTROL_MODULE,
    RECORDING_OVERLAY_MODULE,
    SCREENSHOT_MODULE,
    SCREEN_RECORDER_MODULE,
    SCROLL_CAPTURE_MODULE,
    TIMER_CONTROL_MODULE,
    WINDOW_SELECTOR_MODULE,
];

pub const LINUX_MODULES: &[&str] = &[
    DESKTOP_WALLPAPER_MODULE,
    FREEZE_SCREEN_MODULE,
    PRINT_MODULE,
    QRCODE_MODULE,
    SCREENSHOT_MODULE,
    SCREEN_RECORDER_MODULE,
    SCROLL_CAPTURE_MODULE,
    TIMER_CONTROL_MODULE,
    WINDOW_SELECTOR_MODULE,
];

macro_rules! daemon_methods {
    ($name:ident, { $($variant:ident => $id:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: [Self; [$(stringify!($variant)),+].len()] = [$(Self::$variant),+];

            pub const fn id(self) -> &'static str {
                match self {
                    $(Self::$variant => $id),+
                }
            }

            pub fn parse(method: &str) -> Option<Self> {
                Self::ALL.into_iter().find(|item| item.id() == method)
            }
        }
    };
}

daemon_methods!(AreaSelectorMethod, {
    DisableWindowTransitions => "disableWindowTransitions",
    HideWindowWithoutTransitions => "hideWindowWithoutTransitions",
    ShowWindowWithoutTransitions => "showWindowWithoutTransitions",
    SetWindowRegion => "setWindowRegion",
    GetForegroundWindow => "getForegroundWindow",
    SetForegroundWindow => "setForegroundWindow",
});
daemon_methods!(CameraPreviewMethod, {
    Show => "show",
    Hide => "hide",
    Update => "update",
    SetContentProtection => "setContentProtection",
});
daemon_methods!(DesktopHelperMethod, {
    Hide => "hide",
    Show => "show",
});
daemon_methods!(DesktopWallpaperMethod, {
    Get => "get",
});
daemon_methods!(DisplaySelectorMethod, {
    Select => "select",
    Cancel => "cancel",
});
daemon_methods!(FreezeScreenMethod, {
    Freeze => "freeze",
    Release => "release",
    Prewarm => "prewarm",
});
daemon_methods!(MediaDevicesMethod, {
    List => "list",
    StartMicTest => "startMicTest",
    StopMicTest => "stopMicTest",
});
daemon_methods!(OcrMethod, {
    Recognize => "recognize",
});
daemon_methods!(PrintMethod, {
    Image => "image",
});
daemon_methods!(QrCodeMethod, {
    Detect => "detect",
});
daemon_methods!(RecordingControlMethod, {
    ListIosDevices => "listIOSDevices",
});
daemon_methods!(RecordingOverlayMethod, {
    Show => "show",
    ShowWindow => "showWindow",
    Hide => "hide",
});
daemon_methods!(ScreenshotMethod, {
    CaptureArea => "capture-area",
    CaptureWindow => "capture-window",
});
daemon_methods!(ScreenshotLinuxMethod, {
    ListDisplays => "list-displays",
});
daemon_methods!(ScreenRecorderMethod, {
    Start => "start",
    Pause => "pause",
    Resume => "resume",
    Stop => "stop",
    Status => "status",
    SetMicrophone => "setMicrophone",
    SetSystemAudio => "setSystemAudio",
    SetCamera => "setCamera",
});
daemon_methods!(ScrollCaptureMethod, {
    Start => "start",
    StartAutoScroll => "startAutoScroll",
    StopAutoScroll => "stopAutoScroll",
    Finish => "finish",
    Cancel => "cancel",
});
daemon_methods!(TimerControlMethod, {
    Show => "show",
    Hide => "hide",
});
daemon_methods!(WindowSelectorMethod, {
    List => "list",
});
