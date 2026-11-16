import Foundation
import ScreenCaptureKit

@main
struct RecordingStartStreamErrorTests {
    @MainActor
    static func main() {
        let config = RecordingConfig(
            x: nil, y: nil, width: nil, height: nil, displayID: nil, windowID: 42,
            excludedWindowIDs: [], includeAudio: false, micEnabled: false,
            micDeviceId: nil, micDeviceName: nil, cameraEnabled: false,
            cameraDeviceId: nil, cameraDeviceName: nil, keyboardEnabled: false,
            frameRate: 30, outputPath: "/tmp/unused-recording", iosDeviceId: nil,
            iosDeviceName: nil
        )
        let recorder = ScreenCaptureRecorder()
        var reports: [(RecorderSessionID, Error)] = []
        recorder.onError = { reports.append(($0, $1)) }
        let closed = NSError(domain: SCStreamErrorDomain, code: SCStreamError.Code.noCaptureSource.rawValue)

        recorder.testBeginStreamStartup(config: config, sessionID: 73)
        recorder.testStreamFailure(closed, isCurrentStream: false)
        precondition(reports.isEmpty, "A stale stream must not report errors")
        recorder.testStreamFailure(closed, isCurrentStream: true)
        precondition(reports.count == 1 && reports[0].0 == 73, "A startup stream error must be reported")
        guard case RecorderError.targetClosed = reports[0].1 else {
            fatalError("A closed startup target must retain its target-closed classification")
        }

        recorder.testFinishStreamStartup(recording: true)
        let failure = NSError(domain: "synthetic", code: 1)
        recorder.testStreamFailure(failure, isCurrentStream: true)
        precondition(reports.count == 2 && (reports[1].1 as NSError) === failure)
        recorder.testStreamFailure(failure, isCurrentStream: false)
        precondition(reports.count == 2, "A stale stream must remain ignored after startup")

        var displayConfig = config
        displayConfig.windowID = nil
        let displayRecorder = ScreenCaptureRecorder()
        var displayFailure: Error?
        displayRecorder.onError = { _, error in displayFailure = error }
        displayRecorder.testBeginStreamStartup(config: displayConfig, sessionID: 75)
        displayRecorder.testStreamFailure(closed, isCurrentStream: true)
        precondition((displayFailure as NSError?) === closed, "A missing display source must not be classified as a closed window")

        let idleRecorder = ScreenCaptureRecorder()
        idleRecorder.onError = { _, _ in fatalError("An inactive session must not report errors") }
        idleRecorder.testBeginStreamStartup(config: config, sessionID: 74)
        idleRecorder.testFinishStreamStartup(recording: false)
        idleRecorder.testStreamFailure(failure, isCurrentStream: true)
        print("Startup stream errors preserve session and target-closed classification")
    }
}
