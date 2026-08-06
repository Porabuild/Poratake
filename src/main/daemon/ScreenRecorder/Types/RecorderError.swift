import Foundation

enum RecorderError: LocalizedError {
    case invalidState(String)
    case configuration(String)
    case capture(String)
    case notRecording
    case alreadyRecording

    var errorDescription: String? {
        switch self {
        case .invalidState(let msg): return msg
        case .configuration(let msg): return msg
        case .capture(let msg): return msg
        case .notRecording: return "Not currently recording"
        case .alreadyRecording: return "Already recording"
        }
    }
    
    var code: String {
        switch self {
        case .invalidState: return "INVALID_STATE"
        case .configuration: return "CONFIGURATION_ERROR"
        case .capture: return "CAPTURE_ERROR"
        case .notRecording: return "NOT_RECORDING"
        case .alreadyRecording: return "ALREADY_RECORDING"
        }
    }
}
