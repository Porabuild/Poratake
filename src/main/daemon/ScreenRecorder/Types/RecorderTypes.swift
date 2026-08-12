import CoreMedia
import Foundation

enum RecorderState: String {
    case idle
    case recording
    case paused
}

/// Places `source` inside `target` at the largest scale that keeps its aspect
/// ratio, centring the letterbox. A recorded window keeps the video size it
/// started with, so every later window size is fitted into that frame.
func fitRect(source: CGSize, target: CGSize) -> CGRect {
    guard source.width > 0, source.height > 0, target.width > 0, target.height > 0 else {
        return .zero
    }

    let scale = min(target.width / source.width, target.height / source.height)
    let size = CGSize(width: source.width * scale, height: source.height * scale)

    return CGRect(
        x: (target.width - size.width) / 2,
        y: (target.height - size.height) / 2,
        width: size.width,
        height: size.height
    )
}

struct RecordingConfig {
    var x: Int?
    var y: Int?
    var width: Int?
    var height: Int?
    var displayID: CGDirectDisplayID?
    var windowID: CGWindowID?
    var includeAudio: Bool
    var micEnabled: Bool
    var micDeviceId: String?
    var micDeviceName: String?
    var cameraEnabled: Bool
    var cameraDeviceId: String?
    var cameraDeviceName: String?
    var keyboardEnabled: Bool
    var frameRate: Int
    var outputPath: String
    var iosDeviceId: String?
    var iosDeviceName: String?
    
    var captureRect: CGRect? {
        guard let x = x, let y = y, let width = width, let height = height else {
            return nil
        }
        return CGRect(x: x, y: y, width: width, height: height)
    }
    
    var isAreaRecording: Bool {
        captureRect != nil
    }
    
    var isIOSDeviceRecording: Bool {
        iosDeviceId != nil
    }
}

struct RecordingResult {
    let outputPath: String
    let cursorPath: String?
    let cameraPath: String?
    let keysPath: String?
    let systemAudioPath: String?
    let micAudioPath: String?
    let duration: Double
}

protocol SyncableTracker {
    func syncWithVideoStart()
    func pause()
    func resume()
    func stop() -> String?
}

protocol PausableRecorder {
    var isRecording: Bool { get }
    var isPaused: Bool { get }
    func pause()
    func resume()
}
