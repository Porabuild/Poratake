import Foundation

enum DaemonContract {
    enum AreaSelector {
        static let module = "area-selector"

        enum Method: String {
            case disableWindowTransitions = "disableWindowTransitions"
            case hideWindowWithoutTransitions = "hideWindowWithoutTransitions"
            case showWindowWithoutTransitions = "showWindowWithoutTransitions"
            case setWindowRegion = "setWindowRegion"
            case getForegroundWindow = "getForegroundWindow"
            case setForegroundWindow = "setForegroundWindow"
        }
    }

    enum CameraPreview {
        static let module = "camera-preview"

        enum Method: String {
            case show = "show"
            case hide = "hide"
            case update = "update"
            case setContentProtection = "setContentProtection"
        }
    }

    enum DesktopHelper {
        static let module = "desktop-helper"

        enum Method: String {
            case hide = "hide"
            case show = "show"
        }
    }

    enum DesktopWallpaper {
        static let module = "desktop-wallpaper"

        enum Method: String {
            case get = "get"
        }
    }

    enum DisplaySelector {
        static let module = "display-selector"

        enum Method: String {
            case select = "select"
            case cancel = "cancel"
        }
    }

    enum FreezeScreen {
        static let module = "freeze-screen"

        enum Method: String {
            case freeze = "freeze"
            case release = "release"
            case prewarm = "prewarm"
        }
    }

    enum MediaDevices {
        static let module = "media-devices"

        enum Method: String {
            case list = "list"
            case startMicTest = "startMicTest"
            case stopMicTest = "stopMicTest"
        }
    }

    enum OCR {
        static let module = "ocr"

        enum Method: String {
            case recognize = "recognize"
        }
    }

    enum Print {
        static let module = "print"

        enum Method: String {
            case image = "image"
        }
    }

    enum QRCode {
        static let module = "qrcode"

        enum Method: String {
            case detect = "detect"
        }
    }

    enum RecordingControl {
        static let module = "recording-control"

        enum Method: String {
            case listIosDevices = "listIOSDevices"
        }
    }

    enum RecordingOverlay {
        static let module = "recording-overlay"

        enum Method: String {
            case show = "show"
            case showWindow = "showWindow"
            case hide = "hide"
        }
    }

    enum Screenshot {
        static let module = "screenshot"

        enum Method: String {
            case captureArea = "capture-area"
            case captureWindow = "capture-window"
        }
    }

    enum ScreenRecorder {
        static let module = "screen-recorder"

        enum Method: String {
            case start = "start"
            case pause = "pause"
            case resume = "resume"
            case stop = "stop"
            case status = "status"
            case setMicrophone = "setMicrophone"
            case setSystemAudio = "setSystemAudio"
            case setCamera = "setCamera"
        }
    }

    enum ScrollCapture {
        static let module = "scroll-capture"

        enum Method: String {
            case start = "start"
            case startAutoScroll = "startAutoScroll"
            case stopAutoScroll = "stopAutoScroll"
            case finish = "finish"
            case cancel = "cancel"
        }
    }

    enum TimerControl {
        static let module = "timer-control"

        enum Method: String {
            case show = "show"
            case hide = "hide"
        }
    }

    enum WindowSelector {
        static let module = "window-selector"

        enum Method: String {
            case list = "list"
        }
    }

    static let macOSModules: Set<String> = [
        AreaSelector.module,
        CameraPreview.module,
        DesktopHelper.module,
        DesktopWallpaper.module,
        DisplaySelector.module,
        FreezeScreen.module,
        MediaDevices.module,
        OCR.module,
        Print.module,
        QRCode.module,
        RecordingControl.module,
        RecordingOverlay.module,
        Screenshot.module,
        ScreenRecorder.module,
        ScrollCapture.module,
        TimerControl.module,
        WindowSelector.module,
    ]
}
