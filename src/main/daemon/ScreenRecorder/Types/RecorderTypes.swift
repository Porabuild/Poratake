import CoreMedia
import Foundation

enum RecorderState: String {
    case idle
    case recording
    case paused
}

struct RecordingConfig {
    var x: Int?
    var y: Int?
    var width: Int?
    var height: Int?
    var displayID: CGDirectDisplayID?
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
