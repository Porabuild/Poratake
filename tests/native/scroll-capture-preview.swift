import Cocoa
import Foundation
import ImageIO

@main
struct ScrollCapturePreviewTest {
    static func frame(start: Int, width: Int, height: Int) -> CGImage {
        var pixels = [UInt8]()
        pixels.reserveCapacity(width * height * 4)
        for y in start..<(start + height) {
            for x in 0..<width {
                pixels.append(UInt8(truncatingIfNeeded: y))
                pixels.append(UInt8(truncatingIfNeeded: y >> 8))
                pixels.append(UInt8(truncatingIfNeeded: x * 17 + y * 31))
                pixels.append(255)
            }
        }
        let provider = CGDataProvider(data: Data(pixels) as CFData)!
        return CGImage(
            width: width,
            height: height,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
            provider: provider,
            decode: nil,
            shouldInterpolate: false,
            intent: .defaultIntent
        )!
    }

    static func pixels(_ image: CGImage) -> Data {
        let context = CGContext(
            data: nil,
            width: image.width,
            height: image.height,
            bitsPerComponent: 8,
            bytesPerRow: image.width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return Data(bytes: context.data!, count: image.width * image.height * 4)
    }

    @MainActor
    static func waitUntil(_ message: String, _ condition: @MainActor () -> Bool) async {
        let deadline = Date(timeIntervalSinceNow: 5)
        while !condition() {
            precondition(Date() < deadline, message)
            await Task.yield()
        }
    }

    @MainActor
    static func main() async throws {
        let screenExclusions = recordingExclusionWindowIDs(
            requested: [31, 41, 31, -1, Int64.max],
            registered: [CGWindowID(41), CGWindowID(52)],
            selectedWindowID: nil
        )
        precondition(screenExclusions == [31, 41, 52], "Display recording should deduplicate valid exact window IDs")
        let selectedWindowExclusions = recordingExclusionWindowIDs(
            requested: [31, 41],
            registered: [CGWindowID(52)],
            selectedWindowID: CGWindowID(31)
        )
        precondition(selectedWindowExclusions.isEmpty, "Window recording must keep the selected Poratake window capturable")
        let resolvedExclusions = try resolvedRecordingExclusionWindowIDs(
            requested: [31],
            registered: [CGWindowID(52)],
            selectedWindowID: nil,
            availableWindowIDs: [CGWindowID(31), CGWindowID(52)]
        )
        precondition(resolvedExclusions == [31, 52])
        let selectedResolvedWindowExclusions = try resolvedRecordingExclusionWindowIDs(
            requested: [31],
            registered: [CGWindowID(52)],
            selectedWindowID: CGWindowID(31),
            availableWindowIDs: []
        )
        precondition(selectedResolvedWindowExclusions.isEmpty, "Selected-window capture must not apply display exclusions")
        var missingWindowFailedClosed = false
        do {
            _ = try resolvedRecordingExclusionWindowIDs(
                requested: [31],
                registered: [],
                selectedWindowID: nil,
                availableWindowIDs: []
            )
        } catch is RecordingCaptureFilterError {
            missingWindowFailedClosed = true
        }
        precondition(missingWindowFailedClosed, "An unresolvable recording control must fail closed")

        var presentation = WindowPresentationGeneration()
        let cancelledPresentation = presentation.begin()
        presentation.cancel()
        precondition(!presentation.isCurrent(cancelledPresentation), "Hide must invalidate a pending show")
        let replacementPresentation = presentation.begin()
        precondition(!presentation.isCurrent(cancelledPresentation))
        precondition(presentation.isCurrent(replacementPresentation))

        var recorderLifecycle = RecorderLifecycleGuard()
        var configuredTake = "existing take"
        precondition(recorderLifecycle.beginStart(state: .idle))
        configuredTake = "active take"
        let duplicateStartAccepted = recorderLifecycle.beginStart(state: .idle)
        if duplicateStartAccepted {
            configuredTake = "rejected take"
        }
        precondition(!duplicateStartAccepted, "Concurrent starts must be rejected")
        precondition(configuredTake == "active take", "A rejected start must preserve the active take configuration")
        recorderLifecycle.finishStart()
        precondition(recorderLifecycle.beginStop(state: .recording))
        let earlyRetryAccepted = recorderLifecycle.beginStart(state: .idle)
        if earlyRetryAccepted {
            configuredTake = "early retry"
        }
        precondition(!earlyRetryAccepted, "Restart must wait for stop finalization")
        precondition(configuredTake == "active take")
        recorderLifecycle.finishStop()
        let retryAccepted = recorderLifecycle.beginStart(state: .idle)
        if retryAccepted {
            configuredTake = "retry take"
        }
        precondition(retryAccepted, "A failed stop must release the lifecycle guard")
        precondition(configuredTake == "retry take")
        recorderLifecycle.finishStart()

        var moduleLifecycle = RecorderModuleLifecycle()
        let failedScreenSession = moduleLifecycle.beginStart(.screen)!
        precondition(moduleLifecycle.beginStart(.ios) == nil, "An iOS start must not overlap a screen start")
        precondition(moduleLifecycle.finishStart(failedScreenSession, success: false))
        precondition(moduleLifecycle.phase == .idle, "A failed start must release the module reservation")
        let iosSession = moduleLifecycle.beginStart(.ios)!
        precondition(moduleLifecycle.finishStart(iosSession, success: true))
        precondition(moduleLifecycle.currentBackend == .ios)
        precondition(moduleLifecycle.beginStop(iosSession) == .ios)
        precondition(moduleLifecycle.beginStop(iosSession) == nil, "A second stop must join the existing teardown")
        precondition(moduleLifecycle.beginStart(.screen) == nil, "A screen start must wait for iOS stop finalization")
        moduleLifecycle.finishStop(iosSession)
        let screenSession = moduleLifecycle.beginStart(.screen)!
        precondition(moduleLifecycle.finishStart(screenSession, success: true))
        precondition(!moduleLifecycle.isCurrent(iosSession), "A delayed error must not affect a newer backend")
        precondition(moduleLifecycle.beginStop(screenSession) == .screen)
        moduleLifecycle.finishStop(screenSession)
        let newerScreenSession = moduleLifecycle.beginStart(.screen)!
        precondition(moduleLifecycle.finishStart(newerScreenSession, success: true))
        precondition(!moduleLifecycle.isCurrent(screenSession), "A delayed same-backend error must not affect a newer session")
        precondition(moduleLifecycle.currentBackend == .screen)
        precondition(moduleLifecycle.beginStop(newerScreenSession) == .screen)
        moduleLifecycle.finishStop(newerScreenSession)

        let stopResult = RecordingResult(
            outputPath: "/tmp/recording-stop-test.poratake",
            cursorPath: nil,
            cameraPath: nil,
            keysPath: nil,
            systemAudioPath: nil,
            micAudioPath: nil,
            duration: 1
        )
        let stopCoordinator = RecorderStopCoordinator()
        var stopOperationStarted = false
        var stopOperationCount = 0
        var releaseStopOperation: CheckedContinuation<RecordingResult, Error>?
        let ownerStop = Task { @MainActor in
            try await stopCoordinator.stop(sessionID: 201) {
                stopOperationCount += 1
                stopOperationStarted = true
                return try await withCheckedThrowingContinuation {
                    releaseStopOperation = $0
                }
            }
        }
        await waitUntil("The shared stop operation did not start") { stopOperationStarted }
        let joinedStop = Task { @MainActor in
            try await stopCoordinator.stop(sessionID: 201) {
                stopOperationCount += 1
                return stopResult
            }
        }
        await waitUntil("A duplicate stop did not join the active teardown") {
            stopCoordinator.testWaiterCount == 1
        }
        var differentSessionRejected = false
        do {
            _ = try await stopCoordinator.stop(sessionID: 202) { stopResult }
        } catch is RecorderStopCoordinatorError {
            differentSessionRejected = true
        }
        precondition(differentSessionRejected)
        releaseStopOperation?.resume(returning: stopResult)
        let ownerResult = try await ownerStop.value
        let joinedResult = try await joinedStop.value
        precondition(stopOperationCount == 1, "Concurrent callers must share one backend stop")
        precondition(ownerResult.outputPath == stopResult.outputPath)
        precondition(joinedResult.outputPath == stopResult.outputPath)
        let retryResult = try await stopCoordinator.stop(sessionID: 202) { stopResult }
        precondition(retryResult.outputPath == stopResult.outputPath, "A completed stop must release coordinator ownership")

        var failedStopReported = false
        do {
            _ = try await stopCoordinator.stop(sessionID: 203) {
                throw RecordingCaptureFilterError.unavailableWindows([203])
            }
        } catch is RecordingCaptureFilterError {
            failedStopReported = true
        }
        precondition(failedStopReported)
        let retryAfterFailure = try await stopCoordinator.stop(sessionID: 203) { stopResult }
        precondition(retryAfterFailure.outputPath == stopResult.outputPath, "A failed stop must release coordinator ownership")

        let startupRegistry = RecordingCaptureWindowRegistry()
        startupRegistry.beginCaptureStart()
        startupRegistry.register(windowID: 61)
        var startupRefreshes: [[CGWindowID]] = []
        startupRegistry.setFilterUpdater { ids in startupRefreshes.append(ids) }
        let startupRefresh = Task { @MainActor in
            try await startupRegistry.updateFilterIfNeeded()
        }
        await waitUntil("The startup filter request did not reach its barrier") {
            startupRegistry.testPendingCaptureStartWaiterCount == 1
        }
        precondition(startupRefreshes.isEmpty, "A newly shown panel must wait until capture startup settles")
        startupRegistry.finishCaptureStart()
        await waitUntil("The startup refresh continuation did not resume") { !startupRefreshes.isEmpty }
        try await startupRefresh.value
        precondition(startupRefreshes == [[61]], "Startup must refresh registrations made after the initial snapshot")

        let serialRegistry = RecordingCaptureWindowRegistry()
        var refreshEvents: [[CGWindowID]] = []
        var firstRefreshStarted = false
        var releaseFirstRefresh: CheckedContinuation<Void, Never>?
        serialRegistry.setFilterUpdater { ids in
            refreshEvents.append(ids)
            if ids == [71] {
                firstRefreshStarted = true
                await withCheckedContinuation { releaseFirstRefresh = $0 }
            }
        }
        serialRegistry.register(windowID: 71)
        var completionOrder: [String] = []
        serialRegistry.testOnRefreshCleared = {
            try? await serialRegistry.updateFilterIfNeeded()
            completionOrder.append("follower-first")
        }
        let firstRefresh = Task { @MainActor in
            try await serialRegistry.updateFilterIfNeeded()
            completionOrder.append("owner")
        }
        await waitUntil("The owner refresh did not start") { firstRefreshStarted }
        let followerRefresh = Task { @MainActor in
            try await serialRegistry.updateFilterIfNeeded()
            completionOrder.append("follower")
        }
        await waitUntil("The follower did not join the held refresh") {
            serialRegistry.testPendingRefreshWaiterCount == 2
        }
        serialRegistry.unregister(windowID: 71)
        serialRegistry.register(windowID: 72)
        releaseFirstRefresh?.resume()
        await waitUntil("The refresh waiters did not resume") { completionOrder.count >= 4 }
        try await firstRefresh.value
        try await followerRefresh.value
        precondition(refreshEvents == [[71], [72]], "A follower must request the newer revision before the original waiters resume")
        precondition(completionOrder.first == "follower-first", "A follower must observe cleared refresh state before the owner resumes")
        precondition(completionOrder.count >= 4)

        let coalescingRegistry = RecordingCaptureWindowRegistry()
        var coalescedEvents: [[CGWindowID]] = []
        var oldRefreshStarted = false
        var releaseOldRefresh: CheckedContinuation<Void, Never>?
        coalescingRegistry.setFilterUpdater { ids in
            coalescedEvents.append(ids)
            if ids == [81] {
                oldRefreshStarted = true
                await withCheckedContinuation { releaseOldRefresh = $0 }
            }
        }
        coalescingRegistry.register(windowID: 81)
        let supersededRefresh = Task { @MainActor in
            try await coalescingRegistry.updateFilterIfNeeded()
        }
        await waitUntil("The old snapshot refresh did not start") { oldRefreshStarted }
        coalescingRegistry.unregister(windowID: 81)
        coalescingRegistry.register(windowID: 82)
        let latestRefresh = Task { @MainActor in
            try await coalescingRegistry.updateFilterIfNeeded()
        }
        await waitUntil("The latest window snapshot was not queued") {
            coalescingRegistry.testPendingRefreshWaiterCount == 2
        }
        releaseOldRefresh?.resume()
        await waitUntil("The updated filter snapshot was not applied") { coalescedEvents.count == 2 }
        try await supersededRefresh.value
        try await latestRefresh.value
        precondition(coalescedEvents == [[81], [82]])

        let restartedRegistry = RecordingCaptureWindowRegistry()
        var oldErrorRelease: CheckedContinuation<Void, Never>?
        var oldStreamStarted = false
        var oldStreamUpdaterFinished = false
        var restartedEvents: [[CGWindowID]] = []
        restartedRegistry.setFilterUpdater { ids in
            if ids == [91] {
                oldStreamStarted = true
                await withCheckedContinuation { oldErrorRelease = $0 }
                oldStreamUpdaterFinished = true
                throw RecordingCaptureFilterError.unavailableWindows([91])
            }
            restartedEvents.append(ids)
        }
        restartedRegistry.register(windowID: 91)
        let oldStreamRefresh = Task { @MainActor in
            try await restartedRegistry.updateFilterIfNeeded()
        }
        await waitUntil("The old stream refresh did not start") { oldStreamStarted }
        restartedRegistry.unregister(windowID: 91)
        restartedRegistry.setFilterUpdater(nil)
        try await oldStreamRefresh.value
        restartedRegistry.beginCaptureStart()
        restartedRegistry.register(windowID: 92)
        restartedRegistry.setFilterUpdater { ids in restartedEvents.append(ids) }
        let newStreamRefresh = Task { @MainActor in
            try await restartedRegistry.updateFilterIfNeeded()
        }
        await waitUntil("The new stream filter did not wait for its start barrier") {
            restartedRegistry.testPendingCaptureStartWaiterCount == 1
        }
        restartedRegistry.finishCaptureStart()
        await waitUntil("The restarted stream filter was not applied") { restartedEvents == [[92]] }
        try await newStreamRefresh.value
        oldErrorRelease?.resume()
        await waitUntil("The invalidated old stream refresh did not finish") { oldStreamUpdaterFinished }
        precondition(restartedEvents == [[92]], "An old stream error must not poison the new filter session")

        let failingRegistry = RecordingCaptureWindowRegistry()
        failingRegistry.setFilterUpdater { _ in
            throw RecordingCaptureFilterError.unavailableWindows([81])
        }
        var refreshFailureReported = false
        do {
            try await failingRegistry.updateFilterIfNeeded()
        } catch is RecordingCaptureFilterError {
            refreshFailureReported = true
        }
        precondition(refreshFailureReported, "Filter update errors must reach the window show operation")
        runExistingScrollChecks()
    }

    static func runExistingScrollChecks() {
        let width = 80
        let height = 160
        let step = 112
        let black = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        black.setFillColor(NSColor.black.cgColor)
        black.fill(CGRect(x: 0, y: 0, width: width, height: height))
        let (hash, results, count, restarted, cleared) = ScrollCaptureModule().testRepeatedFrame(black.makeImage()!)
        precondition(hash == 0)
        precondition(results.count == 4 && results[0] == 0 && results[1] == 0 && results[2] == 0 && results[3] == nil)
        precondition(count == 1, "The first frame must be captured even when its hash is zero")
        precondition(restarted == 0, "A new session must not reuse the previous frame hash")
        precondition(cleared, "Cleanup must clear the previous frame hash")

        let cancelWins = ScrollCaptureModule().testFinishRace(cancelBeforeCompletion: true)
        precondition(cancelWins.0 && cancelWins.1 && cancelWins.2 && cancelWins.3)
        let cancelledResult = ScrollCaptureModule().testCancelledFinishResult(
            outputPath: "/tmp/scroll-cancelled.png",
            frameCount: 3
        )
        precondition((cancelledResult["success"] as? Bool) == false)
        precondition((cancelledResult["outputPath"] as? String) == "/tmp/scroll-cancelled.png")
        precondition((cancelledResult["width"] as? Int) == 0 && (cancelledResult["height"] as? Int) == 0)
        precondition((cancelledResult["frameCount"] as? Int) == 3)
        let completionWins = ScrollCaptureModule().testFinishRace(cancelBeforeCompletion: false)
        precondition(!completionWins.0 && !completionWins.1 && completionWins.2 && completionWins.3)
        let doneCancelHandoff = ScrollCaptureModule().testDoneCancelHandoff()
        precondition(doneCancelHandoff.0 && doneCancelHandoff.1 && doneCancelHandoff.2 && doneCancelHandoff.3)
        let cancelledOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-cancel-" + UUID().uuidString + ".tmp")
        defer { try? FileManager.default.removeItem(at: cancelledOutput) }
        precondition(ScrollCaptureModule().testCancelledStitchPreservesOutputPath(cancelledOutput.path))

        let finishModule = ScrollCaptureModule()
        let finishFrame = frame(start: 0, width: width, height: height)
        finishModule.testPrepareCaptureForFinish(frames: [finishFrame], step: step)
        let workerStarted = DispatchSemaphore(value: 0)
        let releaseWorker = DispatchSemaphore(value: 0)
        finishModule.testHoldFinishWorker(started: workerStarted, release: releaseWorker)
        var responses: [(id: String, result: [String: Any])] = []
        finishModule.testSetFinishResponder { id, result in
            responses.append((id, result))
        }
        finishModule.testSetCancelResponder { id, cancelled in
            responses.append((id, ["cancelled": cancelled]))
        }
        let finishOutput = NSTemporaryDirectory() + "/scroll-finish-race-" + UUID().uuidString + ".png"
        defer { try? FileManager.default.removeItem(atPath: finishOutput) }
        finishModule.handle(
            method: DaemonContract.ScrollCapture.Method.finish.rawValue,
            params: ["outputPath": AnyCodable(finishOutput)],
            requestId: "held-finish"
        )
        precondition(workerStarted.wait(timeout: .now() + 5) == .success, "Finish worker did not start")
        finishModule.handle(
            method: DaemonContract.ScrollCapture.Method.cancel.rawValue,
            params: nil,
            requestId: "held-cancel"
        )
        precondition(responses.isEmpty, "Cancel must wait until the finish worker exits")
        releaseWorker.signal()
        let responseDeadline = Date(timeIntervalSinceNow: 5)
        while responses.count < 2 && RunLoop.current.run(mode: .default, before: responseDeadline) {
        }
        precondition(responses.count == 2, "Finish and Cancel must respond after the worker exits")
        precondition(responses.map(\.id) == ["held-finish", "held-cancel"])
        let heldFinish = responses[0].result
        precondition((heldFinish["success"] as? Bool) == false)
        precondition((heldFinish["outputPath"] as? String) == finishOutput)
        precondition((heldFinish["width"] as? Int) == 0 && (heldFinish["height"] as? Int) == 0)
        precondition((heldFinish["frameCount"] as? Int) == 1)
        precondition((responses[1].result["cancelled"] as? Bool) == true)
        precondition(finishModule.testStartCaptureAfterFinish(), "A capture must be retryable after finish and cancel settle")

        let allFrames = [0, step, step * 2].map { frame(start: $0, width: width, height: height) }
        let alignmentModule = ScrollCaptureModule()
        let alignmentStarted = DispatchSemaphore(value: 0)
        let releaseAlignment = DispatchSemaphore(value: 0)
        let finishWaitingForAlignment = DispatchSemaphore(value: 0)
        let alignmentResponseLock = NSLock()
        var alignmentResponses: [(String, [String: Any])] = []
        var alignmentPublications: [Int] = []
        alignmentModule.testSetPreviewAlignment { _, _, expected, _ in
            alignmentStarted.signal()
            releaseAlignment.wait()
            return expected
        }
        alignmentModule.testSetPreviewEncoder { _ in "preview" }
        alignmentModule.testSetPreviewResponder { _, frameCount, _ in
            alignmentPublications.append(frameCount)
        }
        alignmentModule.testSetFinishAlignmentWaitStarted(finishWaitingForAlignment)
        alignmentModule.testSetFinishResponder { id, result in
            alignmentResponseLock.lock()
            alignmentResponses.append((id, result))
            alignmentResponseLock.unlock()
        }
        alignmentModule.testSetCancelResponder { id, cancelled in
            alignmentResponseLock.lock()
            alignmentResponses.append((id, ["cancelled": cancelled]))
            alignmentResponseLock.unlock()
        }
        alignmentModule.testPreparePreview(step: step)
        alignmentModule.testAppendPreviewFrame(allFrames[0])
        alignmentModule.testAppendPreviewFrame(allFrames[1])
        precondition(alignmentStarted.wait(timeout: .now() + 5) == .success)
        let alignmentOutput = NSTemporaryDirectory() + "/scroll-alignment-cancel-\(UUID().uuidString).png"
        defer { try? FileManager.default.removeItem(atPath: alignmentOutput) }
        alignmentModule.handle(
            method: DaemonContract.ScrollCapture.Method.finish.rawValue,
            params: ["outputPath": AnyCodable(alignmentOutput)],
            requestId: "alignment-finish"
        )
        precondition(finishWaitingForAlignment.wait(timeout: .now() + 5) == .success)
        alignmentModule.handle(
            method: DaemonContract.ScrollCapture.Method.cancel.rawValue,
            params: nil,
            requestId: "alignment-cancel"
        )
        alignmentResponseLock.lock()
        precondition(alignmentResponses.isEmpty, "Finish and Cancel must wait for held alignment")
        alignmentResponseLock.unlock()
        releaseAlignment.signal()
        let alignmentDeadline = Date(timeIntervalSinceNow: 5)
        while Date() < alignmentDeadline {
            alignmentResponseLock.lock()
            let complete = alignmentResponses.count == 2
            alignmentResponseLock.unlock()
            if complete { break }
            _ = RunLoop.current.run(mode: .default, before: alignmentDeadline)
        }
        alignmentResponseLock.lock()
        precondition(alignmentResponses.map { $0.0 } == ["alignment-finish", "alignment-cancel"])
        precondition((alignmentResponses[0].1["success"] as? Bool) == false)
        precondition((alignmentResponses[1].1["cancelled"] as? Bool) == true)
        alignmentResponseLock.unlock()
        precondition(!alignmentPublications.contains(2), "Cancelled alignment must not publish a stale preview")

        let backlogModule = ScrollCaptureModule()
        let firstEncodeStarted = DispatchSemaphore(value: 0)
        let releaseFirstEncode = DispatchSemaphore(value: 0)
        let latestEncodeStarted = DispatchSemaphore(value: 0)
        let releaseLatestEncode = DispatchSemaphore(value: 0)
        let backlogLock = NSLock()
        var backlogEncodeCount = 0
        var backlogPublications: [Int] = []
        backlogModule.testSetPreviewAlignment { _, _, expected, _ in expected }
        backlogModule.testSetPreviewEncoder { _ in
            backlogLock.lock()
            backlogEncodeCount += 1
            let count = backlogEncodeCount
            backlogLock.unlock()
            if count == 1 {
                firstEncodeStarted.signal()
                releaseFirstEncode.wait()
            } else {
                latestEncodeStarted.signal()
                releaseLatestEncode.wait()
            }
            return "preview-\(count)"
        }
        backlogModule.testSetPreviewResponder { _, frameCount, _ in
            backlogPublications.append(frameCount)
        }
        backlogModule.testPreparePreview(step: step)
        backlogModule.testAppendPreviewFrame(allFrames[0])
        precondition(firstEncodeStarted.wait(timeout: .now() + 5) == .success)
        backlogModule.testAppendPreviewFrame(allFrames[1])
        backlogModule.testAppendPreviewFrame(allFrames[2])
        backlogModule.testWaitForPreviewAlignment()
        releaseFirstEncode.signal()
        precondition(latestEncodeStarted.wait(timeout: .now() + 5) == .success)
        let firstBacklogPublicationDeadline = Date(timeIntervalSinceNow: 5)
        while backlogPublications.isEmpty && Date() < firstBacklogPublicationDeadline {
            _ = RunLoop.current.run(mode: .default, before: firstBacklogPublicationDeadline)
        }
        precondition(backlogPublications == [1], "A completed preview must publish while newer frames are pending")
        releaseLatestEncode.signal()
        backlogModule.testWaitForPreviewWork()
        let backlogDeadline = Date(timeIntervalSinceNow: 5)
        while backlogPublications.count < 2 && Date() < backlogDeadline {
            _ = RunLoop.current.run(mode: .default, before: backlogDeadline)
        }
        backlogLock.lock()
        precondition(backlogEncodeCount == 2, "Only the active and latest preview should be encoded")
        backlogLock.unlock()
        precondition(backlogPublications == [1, 3], "A preview backlog must publish monotonically and coalesce pending frames")

        let staleModule = ScrollCaptureModule()
        let staleEncodeStarted = DispatchSemaphore(value: 0)
        let releaseStaleEncode = DispatchSemaphore(value: 0)
        let staleLock = NSLock()
        var staleEncodeCount = 0
        var stalePublications: [Int] = []
        staleModule.testSetPreviewAlignment { _, _, expected, _ in expected }
        staleModule.testSetPreviewEncoder { _ in
            staleLock.lock()
            staleEncodeCount += 1
            let count = staleEncodeCount
            staleLock.unlock()
            if count == 1 {
                staleEncodeStarted.signal()
                releaseStaleEncode.wait()
            }
            return "preview-\(count)"
        }
        staleModule.testSetPreviewResponder { _, frameCount, _ in
            stalePublications.append(frameCount)
        }
        staleModule.testPreparePreview(step: step)
        staleModule.testAppendPreviewFrame(allFrames[0])
        precondition(staleEncodeStarted.wait(timeout: .now() + 5) == .success)
        staleModule.testPreparePreview(step: step)
        staleModule.testAppendPreviewFrame(allFrames[0])
        staleModule.testAppendPreviewFrame(allFrames[1])
        staleModule.testWaitForPreviewWork()
        let freshDeadline = Date(timeIntervalSinceNow: 5)
        while stalePublications.isEmpty && Date() < freshDeadline {
            _ = RunLoop.current.run(mode: .default, before: freshDeadline)
        }
        precondition(stalePublications == [2], "The replacement session should publish its current preview")
        releaseStaleEncode.signal()
        staleModule.testWaitForRetiredPreviewWork()
        let staleCallbackDrained = DispatchSemaphore(value: 0)
        DispatchQueue.main.async {
            staleCallbackDrained.signal()
        }
        let staleCallbackDeadline = Date(timeIntervalSinceNow: 5)
        while staleCallbackDrained.wait(timeout: .now()) != .success {
            precondition(Date() < staleCallbackDeadline, "Main-queue callbacks did not drain")
            _ = RunLoop.current.run(mode: .default, before: staleCallbackDeadline)
        }
        precondition(stalePublications == [2], "A retired session must not publish after restart")

        for count in [1, 2, 3] {
            let output = URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent("poratake-scroll-preview-\(UUID().uuidString).png")
            defer { try? FileManager.default.removeItem(at: output) }
            let (
                preview,
                saved,
                encoded,
                overlaps,
                previewFrameCount,
                estimatedHeight
            ) = ScrollCaptureModule().testPreviewAndSave(
                frames: Array(allFrames.prefix(count)),
                step: step,
                outputPath: output.path
            )
            precondition(overlaps == Array(repeating: height - step, count: count - 1))
            precondition(preview.width == width && preview.height == height + step * (count - 1))
            precondition(saved.width == preview.width && saved.height == preview.height)
            precondition(previewFrameCount == count)
            precondition(estimatedHeight == height + step * (count - 1))
            precondition(pixels(preview) == pixels(saved), "Preview differs from the saved scroll capture")
            let data = Data(base64Encoded: encoded)!
            let source = CGImageSourceCreateWithData(data as CFData, nil)!
            let emitted = CGImageSourceCreateImageAtIndex(source, 0, nil)!
            precondition(pixels(emitted) == pixels(saved), "Emitted preview differs from the saved scroll capture")
        }
        let variableOverlapFrames = [
            frame(start: 0, width: width, height: height),
            frame(start: 96, width: width, height: height)
        ]
        let variableOverlapOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-variable-overlap-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: variableOverlapOutput) }
        let (
            variablePreview,
            variableSaved,
            _,
            variableOverlaps,
            variablePreviewFrameCount,
            variableEstimatedHeight
        ) = ScrollCaptureModule().testPreviewAndSave(
            frames: variableOverlapFrames,
            step: 112,
            outputPath: variableOverlapOutput.path
        )
        precondition(variableOverlaps == [64], "The alignment search must find the 64-pixel overlap")
        precondition(variablePreview.height == 256 && variableSaved.height == 256)
        precondition(variablePreviewFrameCount == 2 && variableEstimatedHeight == 256)
        precondition(pixels(variablePreview) == pixels(variableSaved), "Variable-overlap preview differs from saved pixels")
        print("macOS scroll captures match saved pixels for nominal and measured overlap and suppress stale previews")
    }
}
