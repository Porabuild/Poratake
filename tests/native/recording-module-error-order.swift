import Foundation

@main
struct RecordingModuleErrorOrderTests {
    @MainActor
    static func main() {
        let module = ScreenRecorderModule()
        let session = module.testBeginRecordingSession()
        module.testRaiseRecorderError(sessionID: session, error: RecorderError.capture("Synthetic failure"))
        precondition(module.testErrorEvents == ["CAPTURE_ERROR"], "A current error must be emitted before a concurrent Stop can clear the session")
        module.testFinishRecordingSession(session)
        module.testRaiseRecorderError(sessionID: session, error: RecorderError.targetClosed)
        precondition(module.testErrorEvents == ["CAPTURE_ERROR"], "A stale session must not emit an error")

        let closed = ScreenRecorderModule()
        let windowSession = closed.testBeginRecordingSession()
        closed.testRaiseRecorderError(sessionID: windowSession, error: RecorderError.targetClosed)
        precondition(closed.testErrorEvents == ["TARGET_CLOSED"], "A closed target must be emitted before Stop")
        closed.testFinishRecordingSession(windowSession)

        let stopping = ScreenRecorderModule()
        let stoppingSession = stopping.testBeginRecordingSession()
        stopping.testBeginStoppingSession(stoppingSession)
        stopping.testRaiseRecorderError(sessionID: stoppingSession, error: RecorderError.capture("Synthetic stop failure"))
        precondition(stopping.testErrorEvents == ["CAPTURE_ERROR"], "A stopping session must emit its error before completion")
        stopping.testFinishStoppingSession(stoppingSession)

        let starting = ScreenRecorderModule()
        let startingSession = starting.testBeginStartingSession()
        starting.testRaiseRecorderError(sessionID: startingSession, error: RecorderError.targetClosed)
        starting.testRaiseRecorderError(sessionID: startingSession, error: RecorderError.capture("Startup failure"))
        precondition(starting.testErrorEvents.isEmpty, "An error must wait for startup to resolve")
        starting.testFinishStartingSession(startingSession, success: true)
        precondition(starting.testErrorEvents == ["TARGET_CLOSED", "CAPTURE_ERROR"], "Startup errors must be emitted before a concurrent Stop")
        starting.testFinishRecordingSession(startingSession)

        let failed = ScreenRecorderModule()
        let failedSession = failed.testBeginStartingSession()
        failed.testRaiseRecorderError(sessionID: failedSession, error: RecorderError.targetClosed)
        failed.testFinishStartingSession(failedSession, success: false)
        let nextSession = failed.testBeginStartingSession()
        failed.testFinishStartingSession(nextSession, success: true)
        precondition(failed.testErrorEvents.isEmpty, "A failed startup must not leak errors into its replacement")
        failed.testFinishRecordingSession(nextSession)

        let ios = ScreenRecorderModule()
        let iosSession = ios.testBeginRecordingSession(backend: .ios)
        ios.testRaiseRecorderError(sessionID: iosSession, error: RecorderError.capture("Wrong backend"))
        precondition(ios.testErrorEvents.isEmpty, "An error from the wrong recorder must be ignored")
        ios.testRaiseRecorderError(sessionID: iosSession, error: RecorderError.capture("Synthetic iOS failure"), backend: .ios)
        precondition(ios.testErrorEvents == ["CAPTURE_ERROR"], "An iOS recorder error must be emitted before Stop")
        ios.testFinishRecordingSession(iosSession)
        print("Recorder errors are emitted before a concurrent Stop clears the session")
    }
}
