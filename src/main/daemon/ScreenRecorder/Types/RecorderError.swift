import Foundation
import ScreenCaptureKit

enum RecorderError: LocalizedError {
    case invalidState(String)
    case configuration(String)
    case capture(String)
    case notRecording
    case alreadyRecording
    case targetClosed

    static func streamFailure(_ error: Error, capturingWindow: Bool) -> Error {
        let nativeError = error as NSError
        guard capturingWindow,
              nativeError.domain == SCStreamErrorDomain,
              nativeError.code == SCStreamError.Code.noCaptureSource.rawValue
        else { return error }
        return RecorderError.targetClosed
    }

    var errorDescription: String? {
        switch self {
        case .invalidState(let msg): return msg
        case .configuration(let msg): return msg
        case .capture(let msg): return msg
        case .notRecording: return "Not currently recording"
        case .alreadyRecording: return "Already recording"
        case .targetClosed: return "The recorded window was closed"
        }
    }
    
    var code: String {
        switch self {
        case .invalidState: return "INVALID_STATE"
        case .configuration: return "CONFIGURATION_ERROR"
        case .capture: return "CAPTURE_ERROR"
        case .notRecording: return "NOT_RECORDING"
        case .alreadyRecording: return "ALREADY_RECORDING"
        case .targetClosed: return "TARGET_CLOSED"
        }
    }
}
