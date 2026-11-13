import Cocoa
import CoreGraphics
import CoreMedia
import CoreVideo
import Foundation
import ImageIO
import ScreenCaptureKit

private struct CapturedScreenFrame: Sendable {
    let width: Int
    let height: Int
    let bytesPerRow: Int
    let pixels: Data

    func color(x: Double, topY: Double) -> (UInt8, UInt8, UInt8)? {
        guard width > 0, height > 0, x >= 0, x < 1, topY >= 0, topY < 1 else {
            return nil
        }
        let pixelX = min(Int(x * Double(width)), width - 1)
        let pixelY = min(Int(topY * Double(height)), height - 1)
        let offset = pixelY * bytesPerRow + pixelX * 4
        guard offset + 2 < pixels.count else { return nil }
        return (pixels[offset + 2], pixels[offset + 1], pixels[offset])
    }
}

private final class CaptureFrameOutput: NSObject, SCStreamOutput, @unchecked Sendable {
    let frames: AsyncStream<CapturedScreenFrame>
    private let continuation: AsyncStream<CapturedScreenFrame>.Continuation

    override init() {
        let stream = AsyncStream<CapturedScreenFrame>.makeStream(bufferingPolicy: .bufferingNewest(8))
        frames = stream.stream
        continuation = stream.continuation
    }

    func stream(
        _ stream: SCStream,
        didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of type: SCStreamOutputType
    ) {
        guard type == .screen,
              let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer),
              CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly) == kCVReturnSuccess
        else {
            return
        }
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }
        guard let baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer) else { return }
        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)
        let bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer)
        continuation.yield(CapturedScreenFrame(
            width: width,
            height: height,
            bytesPerRow: bytesPerRow,
            pixels: Data(bytes: baseAddress, count: bytesPerRow * height)
        ))
    }

    func finish() {
        continuation.finish()
    }

    func frame(
        timeoutNanoseconds: UInt64 = 5_000_000_000,
        matching predicate: @escaping @Sendable (CapturedScreenFrame) -> Bool
    ) async throws -> CapturedScreenFrame {
        try await withThrowingTaskGroup(of: CapturedScreenFrame.self) { group in
            group.addTask {
                for await frame in self.frames where predicate(frame) {
                    return frame
                }
                throw CaptureFrameError.streamEnded
            }
            group.addTask {
                try await Task.sleep(nanoseconds: timeoutNanoseconds)
                throw CaptureFrameError.timedOut
            }
            defer { group.cancelAll() }
            guard let frame = try await group.next() else {
                throw CaptureFrameError.streamEnded
            }
            return frame
        }
    }
}

private enum CaptureFrameError: Error {
    case missingFixtureDisplay
    case missingFixtureWindow
    case fixtureControlWasNotHidden
    case hiddenFixtureWindowUnavailable
    case fixtureControlDidNotBecomeVisible
    case streamEnded
    case timedOut
}

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

    static func fixedFooterFrame(index: Int, periodic: Bool, width: Int, height: Int, footerHeight: Int, step: Int, blankTail: Bool = false, changingTail: Bool = false, bodyNoise: Int = 0, changingFooter: Bool = false) -> CGImage {
        var pixels = [UInt8]()
        pixels.reserveCapacity(width * height * 4)
        for y in 0..<height {
            for x in 0..<width {
                var pixel: [UInt8]
                if y >= height - footerHeight {
                    let offset = changingFooter ? 10 + index : 0
                    pixel = blankTail
                        ? [0, 0, 0, 255]
                        : [
                            UInt8(truncatingIfNeeded: 62 + offset),
                            UInt8(truncatingIfNeeded: (y - (height - footerHeight)) % 251 + offset),
                            UInt8(truncatingIfNeeded: x % 251 + offset),
                            255
                        ]
                } else if periodic {
                    let absoluteY = y + index * step
                    if x == 0 && absoluteY % 256 < 32 {
                        let value = UInt32(truncatingIfNeeded: absoluteY) &* 0x45d9f3b
                        pixel = [
                            UInt8(truncatingIfNeeded: value),
                            UInt8(truncatingIfNeeded: value << 9 | value >> 23),
                            UInt8(truncatingIfNeeded: value << 17 | value >> 15),
                            255
                        ]
                    } else {
                        pixel = [UInt8((absoluteY % 8) * 30), 0, 0, 255]
                    }
                } else {
                    let absoluteY = UInt32(truncatingIfNeeded: y + index * step)
                    let value = absoluteY &* 0x45d9f3b &+ UInt32(x) &* 0x119de1f3
                    pixel = [
                        UInt8(value % 200) + UInt8(index * bodyNoise),
                        UInt8((value << 9 | value >> 23) % 200) + UInt8(index * bodyNoise),
                        UInt8((value << 17 | value >> 15) % 200) + UInt8(index * bodyNoise),
                        255
                    ]
                }
                if periodic && (1..<11).contains(x) && (32..<42).contains(y) {
                    pixel[0] ^= UInt8(truncatingIfNeeded: 51 * index)
                }
                if changingTail && footerHeight == 0 && y >= height - 20 && x == width - 1 {
                    pixel[2] ^= UInt8(truncatingIfNeeded: index)
                }
                pixels.append(contentsOf: pixel)
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

    static func expectedFooterDocument(width: Int, contentHeight: Int, footerHeight: Int, blankTail: Bool = false, footerOffset: Int = 0) -> CGImage {
        let totalHeight = contentHeight + footerHeight
        var pixels = [UInt8]()
        pixels.reserveCapacity(width * totalHeight * 4)
        for y in 0..<totalHeight {
            for x in 0..<width {
                if y >= contentHeight {
                    let footerY = y - contentHeight
                    let pixel: [UInt8] = blankTail
                        ? [0, 0, 0, 255]
                        : [
                            UInt8(truncatingIfNeeded: 62 + footerOffset),
                            UInt8(truncatingIfNeeded: footerY % 251 + footerOffset),
                            UInt8(truncatingIfNeeded: x % 251 + footerOffset),
                            255
                        ]
                    pixels.append(contentsOf: pixel)
                    continue
                }
                let absoluteY = UInt32(truncatingIfNeeded: y)
                let value = absoluteY &* 0x45d9f3b &+ UInt32(x) &* 0x119de1f3
                pixels.append(contentsOf: [
                    UInt8(value % 200),
                    UInt8((value << 9 | value >> 23) % 200),
                    UInt8((value << 17 | value >> 15) % 200),
                    255
                ])
            }
        }
        let provider = CGDataProvider(data: Data(pixels) as CFData)!
        return CGImage(
            width: width,
            height: totalHeight,
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

    static func sparseContentFrame(start: Int, width: Int, height: Int) -> CGImage {
        var pixels = [UInt8]()
        pixels.reserveCapacity(width * height * 4)
        for y in start..<(start + height) {
            for x in 0..<width {
                if x == 0 {
                    let absoluteY = UInt32(truncatingIfNeeded: y)
                    pixels.append(UInt8(truncatingIfNeeded: absoluteY))
                    pixels.append(UInt8(truncatingIfNeeded: absoluteY >> 8))
                    pixels.append(UInt8(truncatingIfNeeded: absoluteY &* 37))
                    pixels.append(255)
                } else {
                    pixels.append(contentsOf: [0, 0, 0, 255])
                }
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

    static func monotoneFrame(start: Int, width: Int, height: Int) -> CGImage {
        var pixels = [UInt8]()
        pixels.reserveCapacity(width * height * 4)
        for y in start..<(start + height) {
            let value = UInt8(truncatingIfNeeded: y)
            for _ in 0..<width {
                pixels.append(contentsOf: [value, value, value, 255])
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

    static func main() {
        FileHandle.standardError.write(Data("existing scroll checks: start\n".utf8))
        runExistingScrollChecks()
        FileHandle.standardError.write(Data("existing scroll checks: complete\n".utf8))
        Task { @MainActor in
            do {
                try await runRecordingCoordinationChecks()
                exit(EXIT_SUCCESS)
            } catch {
                FileHandle.standardError.write(Data("Recording coordination checks failed: \(error)\n".utf8))
                exit(EXIT_FAILURE)
            }
        }
        dispatchMain()
    }

    @MainActor
    static func runRecordingCoordinationChecks() async throws {
        try await runScreenCaptureExclusionCheck()
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

        FileHandle.standardError.write(Data("recording stop coordinator: start\n".utf8))
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
        FileHandle.standardError.write(Data("recording stop coordinator: complete\n".utf8))

        FileHandle.standardError.write(Data("capture filter startup: start\n".utf8))
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
        FileHandle.standardError.write(Data("capture filter startup: complete\n".utf8))

        FileHandle.standardError.write(Data("capture filter serial refresh: start\n".utf8))
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
        FileHandle.standardError.write(Data("capture filter serial refresh: complete\n".utf8))

        FileHandle.standardError.write(Data("capture filter coalescing: start\n".utf8))
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
        FileHandle.standardError.write(Data("capture filter coalescing: complete\n".utf8))

        FileHandle.standardError.write(Data("capture filter restart: start\n".utf8))
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
        FileHandle.standardError.write(Data("capture filter restart: complete\n".utf8))

        FileHandle.standardError.write(Data("capture filter failure: start\n".utf8))
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
        FileHandle.standardError.write(Data("capture filter failure: complete\n".utf8))
    }

    @MainActor
    static func runScreenCaptureExclusionCheck() async throws {
        guard CommandLine.arguments.contains("--screen-capture-exclusion") else {
            FileHandle.standardError.write(Data("SKIP ScreenCaptureKit exclusion check: pass --screen-capture-exclusion to opt in\n".utf8))
            return
        }
        guard CGPreflightScreenCaptureAccess() else {
            FileHandle.standardError.write(Data("SKIP ScreenCaptureKit exclusion check: Screen Recording permission was not already granted\n".utf8))
            return
        }

        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: start\n".utf8))
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: AppKit initialization begin\n".utf8))
        let application = NSApplication.shared
        application.setActivationPolicy(.accessory)
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: AppKit initialization complete\n".utf8))
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: display lookup begin\n".utf8))
        guard let screen = NSScreen.main ?? NSScreen.screens.first,
              let displayID = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID
        else {
            throw CaptureFrameError.missingFixtureDisplay
        }
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: display lookup complete\n".utf8))
        let screenFrame = screen.frame
        let sourceFrame = NSRect(
            x: screenFrame.midX - 160,
            y: screenFrame.midY - 120,
            width: 320,
            height: 240
        )
        let controlFrame = NSRect(
            x: sourceFrame.midX - 48,
            y: sourceFrame.midY - 32,
            width: 96,
            height: 64
        )
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: window creation begin\n".utf8))
        let sourceWindow = fixtureWindow(
            frame: sourceFrame,
            color: NSColor(calibratedRed: 0.04, green: 0.84, blue: 0.12, alpha: 1),
            level: .normal
        )
        let controlWindow = fixtureWindow(
            frame: controlFrame,
            color: NSColor(calibratedRed: 0.9, green: 0.04, blue: 0.04, alpha: 1),
            level: .floating
        )
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: window creation complete\n".utf8))
        defer {
            controlWindow.orderOut(nil)
            sourceWindow.orderOut(nil)
            controlWindow.close()
            sourceWindow.close()
        }

        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: window show begin\n".utf8))
        sourceWindow.orderFrontRegardless()
        controlWindow.orderFrontRegardless()
        application.updateWindows()
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: window show complete\n".utf8))
        guard let controlWindowID = CGWindowID(exactly: controlWindow.windowNumber) else {
            throw CaptureFrameError.missingFixtureWindow
        }

        let sourceSample = normalizedScreenPoint(
            x: Double(sourceFrame.minX + 24),
            y: Double(sourceFrame.minY + 24),
            screenFrame: screenFrame
        )
        let controlSample = normalizedScreenPoint(
            x: Double(controlFrame.midX),
            y: Double(controlFrame.midY),
            screenFrame: screenFrame
        )
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: control hide begin\n".utf8))
        controlWindow.orderOut(nil)
        application.updateWindows()
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: control hide complete\n".utf8))
        guard !controlWindow.isVisible else {
            throw CaptureFrameError.fixtureControlWasNotHidden
        }
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: hidden content query begin\n".utf8))
        let hiddenContent = try await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: false
        )
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: hidden content query complete\n".utf8))
        guard let hiddenApplication = hiddenContent.applications.first(where: {
            $0.processID == ProcessInfo.processInfo.processIdentifier
        }),
              let hiddenDisplay = hiddenContent.displays.first(where: { $0.displayID == displayID }),
              let hiddenControl = hiddenContent.windows.first(where: {
                  $0.windowID == controlWindowID
              })
        else {
            throw CaptureFrameError.hiddenFixtureWindowUnavailable
        }
        guard !hiddenControl.isOnScreen else {
            throw CaptureFrameError.fixtureControlWasNotHidden
        }
        let configuration = SCStreamConfiguration()
        configuration.width = hiddenDisplay.width
        configuration.height = hiddenDisplay.height
        configuration.pixelFormat = kCVPixelFormatType_32BGRA
        configuration.queueDepth = 3
        configuration.showsCursor = false
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: filtered stream setup begin\n".utf8))
        let exclusionFilter = SCContentFilter(
            display: hiddenDisplay,
            including: [hiddenApplication],
            exceptingWindows: [hiddenControl]
        )
        let exclusionOutput = CaptureFrameOutput()
        let exclusionStream = SCStream(
            filter: exclusionFilter,
            configuration: configuration,
            delegate: nil
        )
        try exclusionStream.addStreamOutput(
            exclusionOutput,
            type: .screen,
            sampleHandlerQueue: DispatchQueue(label: "poratake.capture-exclusion-filtered")
        )
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: filtered stream setup complete\n".utf8))
        do {
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: filtered stream start begin\n".utf8))
            try await exclusionStream.startCapture()
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: filtered stream start complete\n".utf8))
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: control show begin\n".utf8))
            controlWindow.orderFrontRegardless()
            application.updateWindows()
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: control show complete\n".utf8))
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: visible content query begin\n".utf8))
            let shownContent = try await SCShareableContent.excludingDesktopWindows(
                false,
                onScreenWindowsOnly: true
            )
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: visible content query complete\n".utf8))
            guard controlWindow.isVisible,
                  shownContent.windows.contains(where: { $0.windowID == controlWindowID })
            else {
                throw CaptureFrameError.fixtureControlDidNotBecomeVisible
            }
            setFixtureWindowColor(
                sourceWindow,
                NSColor(calibratedRed: 0.04, green: 0.12, blue: 0.9, alpha: 1)
            )
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: filtered frame wait begin\n".utf8))
            let filtered = try await exclusionOutput.frame { frame in
                isFixtureBlue(frame.color(x: sourceSample.0, topY: sourceSample.1))
                    && isFixtureBlue(frame.color(x: controlSample.0, topY: controlSample.1))
            }
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: filtered frame wait complete\n".utf8))
            precondition(
                isFixtureBlue(filtered.color(x: sourceSample.0, topY: sourceSample.1)),
                "The fixture source must remain visible through the app-only filter"
            )
            precondition(
                isFixtureBlue(filtered.color(x: controlSample.0, topY: controlSample.1)),
                "The control hidden at filter creation must remain excluded after showing"
            )
            exclusionOutput.finish()
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: filtered stream stop begin\n".utf8))
            try await exclusionStream.stopCapture()
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: filtered stream stop complete\n".utf8))
        } catch {
            exclusionOutput.finish()
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: filtered stream error: \(error)\n".utf8))
            try? await exclusionStream.stopCapture()
            throw error
        }

        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: positive-control filter setup begin\n".utf8))
        let visibleControlFilter = SCContentFilter(
            display: hiddenDisplay,
            including: [hiddenApplication],
            exceptingWindows: []
        )
        let visibleControlOutput = CaptureFrameOutput()
        let visibleControlStream = SCStream(
            filter: visibleControlFilter,
            configuration: configuration,
            delegate: nil
        )
        try visibleControlStream.addStreamOutput(
            visibleControlOutput,
            type: .screen,
            sampleHandlerQueue: DispatchQueue(label: "poratake.capture-control-positive")
        )
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: positive-control filter setup complete\n".utf8))
        do {
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: positive-control stream start begin\n".utf8))
            try await visibleControlStream.startCapture()
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: positive-control stream start complete\n".utf8))
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: positive-control frame wait begin\n".utf8))
            let visibleControl = try await visibleControlOutput.frame { frame in
                isFixtureBlue(frame.color(x: sourceSample.0, topY: sourceSample.1))
                    && isFixtureRed(frame.color(x: controlSample.0, topY: controlSample.1))
            }
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: positive-control frame wait complete\n".utf8))
            precondition(
                isFixtureBlue(visibleControl.color(x: sourceSample.0, topY: sourceSample.1)),
                "The fixture source must remain visible through its app-only filter"
            )
            precondition(
                isFixtureRed(visibleControl.color(x: controlSample.0, topY: controlSample.1)),
                "The shown control must appear when the app-only filter has no exception"
            )
            visibleControlOutput.finish()
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: positive-control stream stop begin\n".utf8))
            try await visibleControlStream.stopCapture()
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: positive-control stream stop complete\n".utf8))
        } catch {
            visibleControlOutput.finish()
            FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: positive-control stream error: \(error)\n".utf8))
            try? await visibleControlStream.stopCapture()
            throw error
        }
        FileHandle.standardError.write(Data("ScreenCaptureKit hidden-window exclusion: complete\n".utf8))
    }

    @MainActor
    static func fixtureWindow(frame: NSRect, color: NSColor, level: NSWindow.Level) -> NSWindow {
        let window = NSWindow(
            contentRect: frame,
            styleMask: .borderless,
            backing: .buffered,
            defer: false
        )
        window.isOpaque = true
        window.hasShadow = false
        window.ignoresMouseEvents = true
        window.level = level
        setFixtureWindowColor(window, color)
        return window
    }

    @MainActor
    static func setFixtureWindowColor(_ window: NSWindow, _ color: NSColor) {
        window.backgroundColor = color
        window.contentView?.wantsLayer = true
        window.contentView?.layer?.backgroundColor = color.cgColor
        window.contentView?.needsDisplay = true
        window.displayIfNeeded()
    }

    static func normalizedScreenPoint(
        x: Double,
        y: Double,
        screenFrame: NSRect
    ) -> (Double, Double) {
        (
            (x - Double(screenFrame.minX)) / Double(screenFrame.width),
            (Double(screenFrame.maxY) - y) / Double(screenFrame.height)
        )
    }

    static func isFixtureRed(_ color: (UInt8, UInt8, UInt8)?) -> Bool {
        guard let (red, green, blue) = color else { return false }
        return red > 150 && Int(red) > Int(green) * 2 && Int(red) > Int(blue) * 2
    }

    static func isFixtureBlue(_ color: (UInt8, UInt8, UInt8)?) -> Bool {
        guard let (red, green, blue) = color else { return false }
        return blue > 150 && Int(blue) > Int(red) * 2 && Int(blue) > Int(green) * 2
    }

    static func runExistingScrollChecks() {
        let width = 80
        let height = 160
        let step = 112
        let mismatchedOverlap = ScrollCaptureModule().testFindOverlap(
            topFrame: frame(start: 0, width: width, height: height),
            bottomFrame: frame(start: 20, width: width - 1, height: height),
            expectedOverlap: height - step
        )
        precondition(mismatchedOverlap == nil, "Frames with different dimensions must fail closed before full-row matching")
        let cancellationFooterFrames = (0..<2).map {
            fixedFooterFrame(index: $0, periodic: false, width: 640, height: height, footerHeight: 20, step: 96)
        }
        precondition(ScrollCaptureModule().testFooterExtractionFailure(
            topFrame: cancellationFooterFrames[0],
            bottomFrame: cancellationFooterFrames[1]
        ), "Footer extraction failure must not be treated as an absent footer")
        var footerCancellationChecks = 0
        let cancelledFooterScan = ScrollCaptureModule().testFixedFooterHeight(
            topFrame: cancellationFooterFrames[0],
            bottomFrame: cancellationFooterFrames[1]
        ) {
            footerCancellationChecks += 1
            return footerCancellationChecks == 10
        }
        precondition(cancelledFooterScan == nil && footerCancellationChecks == 10, "Footer scanning must observe cancellation between rows")

        let cancellationSparseFrames = [
            sparseContentFrame(start: 0, width: 1000, height: height),
            sparseContentFrame(start: 96, width: 1000, height: height)
        ]
        var overlapCancellationChecks = 0
        let cancelledOverlapScan = ScrollCaptureModule().testExactFullImageOverlap(
            topFrame: cancellationSparseFrames[0],
            bottomFrame: cancellationSparseFrames[1],
            contentHeight: height,
            minimumOverlap: 8
        ) {
            overlapCancellationChecks += 1
            return overlapCancellationChecks == 20
        }
        precondition(cancelledOverlapScan == nil && overlapCancellationChecks == 20, "Full-row overlap arbitration must observe cancellation during prefix scanning")

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
        while responses.count < 2 && Date() < responseDeadline {
            _ = RunLoop.current.run(mode: .default, before: responseDeadline)
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

        let drainModule = ScrollCaptureModule()
        let firstDrainAlignmentStarted = DispatchSemaphore(value: 0)
        let releaseFirstDrainAlignment = DispatchSemaphore(value: 0)
        let finishWaitingForDrain = DispatchSemaphore(value: 0)
        var drainAlignmentCount = 0
        var drainFinishResult: [String: Any]?
        drainModule.testSetPreviewAlignment { _, _, expected, _ in
            drainAlignmentCount += 1
            if drainAlignmentCount == 1 {
                firstDrainAlignmentStarted.signal()
                releaseFirstDrainAlignment.wait()
            }
            return expected
        }
        drainModule.testSetPreviewEncoder { _ in "preview" }
        drainModule.testSetFinishAlignmentWaitStarted(finishWaitingForDrain)
        drainModule.testSetFinishResponder { _, result in drainFinishResult = result }
        drainModule.testPreparePreview(step: 112)
        drainModule.testAppendPreviewFrame(frame(start: 0, width: width, height: height))
        drainModule.testAppendPreviewFrame(frame(start: 112, width: width, height: height))
        precondition(firstDrainAlignmentStarted.wait(timeout: .now() + 5) == .success)
        drainModule.testAppendPreviewFrame(frame(start: 224, width: width, height: height))
        let drainOutput = NSTemporaryDirectory() + "/scroll-alignment-drain-\(UUID().uuidString).png"
        defer { try? FileManager.default.removeItem(atPath: drainOutput) }
        drainModule.handle(
            method: DaemonContract.ScrollCapture.Method.finish.rawValue,
            params: ["outputPath": AnyCodable(drainOutput)],
            requestId: "alignment-drain-finish"
        )
        precondition(finishWaitingForDrain.wait(timeout: .now() + 5) == .success)
        releaseFirstDrainAlignment.signal()
        let drainDeadline = Date(timeIntervalSinceNow: 5)
        while drainFinishResult == nil && Date() < drainDeadline {
            _ = RunLoop.current.run(mode: .default, before: drainDeadline)
        }
        precondition((drainFinishResult?["success"] as? Bool) == true)
        precondition((drainFinishResult?["frameCount"] as? Int) == 3)
        precondition(drainAlignmentCount == 2, "Finish must reuse both alignments drained after preview rendering stopped")
        let drainSource = CGImageSourceCreateWithURL(URL(fileURLWithPath: drainOutput) as CFURL, nil)!
        let drainSaved = CGImageSourceCreateImageAtIndex(drainSource, 0, nil)!
        precondition(drainSaved.height == height + 224)
        precondition(pixels(drainSaved) == pixels(frame(start: 0, width: width, height: height + 224)))

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

        let closeScrollFrames = [
            monotoneFrame(start: 0, width: width, height: height),
            monotoneFrame(start: 30, width: width, height: height)
        ]
        let closeScrollOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-close-overlap-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: closeScrollOutput) }
        let (
            closeScrollPreview,
            closeScrollSaved,
            _,
            closeScrollOverlaps,
            closeScrollFrameCount,
            closeScrollEstimatedHeight
        ) = ScrollCaptureModule().testPreviewAndSave(
            frames: closeScrollFrames,
            step: 30,
            outputPath: closeScrollOutput.path
        )
        precondition(closeScrollOverlaps == [130], "Small scrolls must include overlaps above the former search limit")
        precondition(closeScrollFrameCount == 2 && closeScrollEstimatedHeight == 190)
        precondition(pixels(closeScrollPreview) == pixels(closeScrollSaved))
        precondition(pixels(closeScrollSaved) == pixels(monotoneFrame(start: 0, width: width, height: 190)))

        let shortOverlapFrames = [
            monotoneFrame(start: 0, width: width, height: height),
            monotoneFrame(start: 128, width: width, height: height)
        ]
        let shortOverlapOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-short-overlap-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: shortOverlapOutput) }
        let (
            shortOverlapPreview,
            shortOverlapSaved,
            _,
            shortOverlaps,
            shortOverlapFrameCount,
            shortOverlapEstimatedHeight
        ) = ScrollCaptureModule().testPreviewAndSave(
            frames: shortOverlapFrames,
            step: 128,
            outputPath: shortOverlapOutput.path
        )
        precondition(shortOverlaps == [32], "A sub-40px seam must be accepted only when full rows prove it")
        precondition(shortOverlapFrameCount == 2 && shortOverlapEstimatedHeight == 288)
        precondition(pixels(shortOverlapPreview) == pixels(shortOverlapSaved))
        precondition(pixels(shortOverlapSaved) == pixels(monotoneFrame(start: 0, width: width, height: 288)))

        let sparseFrames = [
            sparseContentFrame(start: 0, width: 1000, height: height),
            sparseContentFrame(start: 96, width: 1000, height: height)
        ]
        let sparseOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-sparse-overlap-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: sparseOutput) }
        let (
            sparsePreview,
            sparseSaved,
            _,
            sparseOverlaps,
            sparseFrameCount,
            sparseEstimatedHeight
        ) = ScrollCaptureModule().testPreviewAndSave(
            frames: sparseFrames,
            step: 96,
            outputPath: sparseOutput.path
        )
        precondition(sparseOverlaps == [64], "Full-width row evidence must resolve a blank, repeated 40-row sample")
        precondition(sparseFrameCount == 2 && sparseEstimatedHeight == 256)
        precondition(pixels(sparsePreview) == pixels(sparseSaved))
        precondition(pixels(sparseSaved) == pixels(sparseContentFrame(start: 0, width: 1000, height: 256)))

        let zeroStepFrames = [frame(start: 0, width: width, height: height), frame(start: 48, width: width, height: height)]
        let zeroStepOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-zero-step-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: zeroStepOutput) }
        let (
            zeroStepPreview,
            zeroStepSaved,
            _,
            zeroStepOverlaps,
            zeroStepFrameCount,
            zeroStepEstimatedHeight
        ) = ScrollCaptureModule().testPreviewAndSave(
            frames: zeroStepFrames,
            step: 0,
            outputPath: zeroStepOutput.path
        )
        precondition(zeroStepOverlaps == [height - 48], "A zero requested step should still use measured content alignment")
        precondition(zeroStepFrameCount == 2 && zeroStepEstimatedHeight == height + 48)
        precondition(pixels(zeroStepPreview) == pixels(zeroStepSaved))

        let noisySeamFrames = (0..<2).map {
            fixedFooterFrame(
                index: $0,
                periodic: false,
                width: 640,
                height: height,
                footerHeight: 0,
                step: 96,
                bodyNoise: 18
            )
        }
        let noisySeamOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-noisy-seam-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: noisySeamOutput) }
        let (
            noisySeamPreview,
            noisySeamSaved,
            _,
            noisySeamOverlaps,
            noisySeamFrameCount,
            noisySeamEstimatedHeight
        ) = ScrollCaptureModule().testPreviewAndSave(
            frames: noisySeamFrames,
            step: 96,
            outputPath: noisySeamOutput.path
        )
        precondition(noisySeamOverlaps == [64], "A unique seam at the shared 18-level per-channel confidence boundary should be accepted")
        precondition(noisySeamFrameCount == 2 && noisySeamEstimatedHeight == 256)
        precondition(pixels(noisySeamPreview) == pixels(noisySeamSaved))

        let overThresholdFrames = (0..<2).map {
            fixedFooterFrame(
                index: $0,
                periodic: false,
                width: 640,
                height: height,
                footerHeight: 0,
                step: 96,
                bodyNoise: 19
            )
        }
        let overThresholdOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-over-threshold-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: overThresholdOutput) }
        let overThresholdResult = ScrollCaptureModule().testFinishReportsAcceptedStitchPrefix(
            frames: overThresholdFrames,
            step: 96,
            outputPath: overThresholdOutput.path
        )
        precondition((overThresholdResult?["success"] as? Bool) == true)
        precondition((overThresholdResult?["frameCount"] as? Int) == 1, "A seam above the confidence boundary must preserve only the valid prefix")

        let noOverlapFrames = [frame(start: 0, width: width, height: height), frame(start: 180, width: width, height: height)]
        let noOverlapOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-no-overlap-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: noOverlapOutput) }
        let noOverlapResult = ScrollCaptureModule().testFinishReportsAcceptedStitchPrefix(
            frames: noOverlapFrames,
            step: step,
            outputPath: noOverlapOutput.path
        )
        precondition((noOverlapResult?["success"] as? Bool) == true)
        precondition((noOverlapResult?["frameCount"] as? Int) == 1, "A poor unique score must not discard rows when the frames have no true overlap")

        let footerWidth = 640
        let footerHeight = 160
        let footerSize = 20
        let footerStep = 96
        let periodicFooterFrames = (0..<2).map {
            fixedFooterFrame(
                index: $0,
                periodic: true,
                width: footerWidth,
                height: footerHeight,
                footerHeight: footerSize,
                step: footerStep
            )
        }
        let periodicFooterOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-periodic-footer-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: periodicFooterOutput) }
        let periodicFooterResult = ScrollCaptureModule().testFinishReportsAcceptedStitchPrefix(
            frames: periodicFooterFrames,
            step: footerStep,
            outputPath: periodicFooterOutput.path
        )
        precondition((periodicFooterResult?["success"] as? Bool) == true)
        precondition((periodicFooterResult?["frameCount"] as? Int) == 1, "A rejected ambiguous frame must not be reported as saved")
        precondition((periodicFooterResult?["width"] as? Int) == footerWidth)
        precondition((periodicFooterResult?["height"] as? Int) == footerHeight)
        let periodicFooterSource = CGImageSourceCreateWithURL(periodicFooterOutput as CFURL, nil)!
        let periodicFooterSaved = CGImageSourceCreateImageAtIndex(periodicFooterSource, 0, nil)!
        precondition(pixels(periodicFooterSaved) == pixels(periodicFooterFrames[0]), "Ambiguous alignment must save the previous accepted frame")

        let autoStopModule = ScrollCaptureModule()
        var autoStopPreviewCounts: [Int] = []
        var noOverlapEvents: [[String: Any]] = []
        autoStopModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        autoStopModule.testSetPreviewEncoder { _ in "preview" }
        autoStopModule.testSetPreviewResponder { _, frameCount, _ in
            autoStopPreviewCounts.append(frameCount)
        }
        autoStopModule.testSetScrollEndedResponder { data in noOverlapEvents.append(data) }
        autoStopModule.testAppendPreviewFrame(periodicFooterFrames[0])
        autoStopModule.testWaitForPreviewWork()
        let firstAutoPreviewDeadline = Date(timeIntervalSinceNow: 5)
        while autoStopPreviewCounts.isEmpty && Date() < firstAutoPreviewDeadline {
            _ = RunLoop.current.run(mode: .default, before: firstAutoPreviewDeadline)
        }
        precondition(autoStopPreviewCounts == [1], "The last accepted preview must be visible before an ambiguous frame")
        autoStopModule.testAppendPreviewFrame(periodicFooterFrames[1])
        autoStopModule.testWaitForPreviewAlignment()
        let autoStopDeadline = Date(timeIntervalSinceNow: 5)
        while noOverlapEvents.isEmpty && Date() < autoStopDeadline {
            _ = RunLoop.current.run(mode: .default, before: autoStopDeadline)
        }
        precondition(!autoStopModule.testIsAutoScrolling(), "Ambiguous alignment must stop auto-scroll")
        precondition((noOverlapEvents.first?["reason"] as? String) == "no-overlap")
        precondition((noOverlapEvents.first?["frameCount"] as? Int) == 1)
        precondition(autoStopPreviewCounts == [1], "The ambiguous frame must not replace the last accepted preview")
        let autoStopOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-auto-stop-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: autoStopOutput) }
        let autoStopResult = autoStopModule.testFinishCurrentCapture(outputPath: autoStopOutput.path)
        precondition((autoStopResult?["success"] as? Bool) == true)
        precondition((autoStopResult?["frameCount"] as? Int) == 1)
        precondition(noOverlapEvents.count == 1, "Finish must not duplicate the auto-stop notification")
        let autoStopSource = CGImageSourceCreateWithURL(autoStopOutput as CFURL, nil)!
        let autoStopSaved = CGImageSourceCreateImageAtIndex(autoStopSource, 0, nil)!
        precondition(pixels(autoStopSaved) == pixels(periodicFooterFrames[0]))

        let repeatedNoFooterFrames = (0..<2).map {
            fixedFooterFrame(
                index: $0,
                periodic: true,
                width: footerWidth,
                height: footerHeight,
                footerHeight: 0,
                step: footerStep,
                changingTail: true
            )
        }
        let manualStopModule = ScrollCaptureModule()
        var manualStopPreviewCounts: [Int] = []
        var manualNoOverlapEvents: [[String: Any]] = []
        manualStopModule.testPreparePreview(step: footerStep, width: footerWidth, height: footerHeight)
        manualStopModule.testSetPreviewEncoder { _ in "preview" }
        manualStopModule.testSetPreviewResponder { _, frameCount, _ in
            manualStopPreviewCounts.append(frameCount)
        }
        manualStopModule.testSetScrollEndedResponder { data in manualNoOverlapEvents.append(data) }
        manualStopModule.testAppendPreviewFrame(repeatedNoFooterFrames[0])
        manualStopModule.testWaitForPreviewWork()
        let firstManualPreviewDeadline = Date(timeIntervalSinceNow: 5)
        while manualStopPreviewCounts.isEmpty && Date() < firstManualPreviewDeadline {
            _ = RunLoop.current.run(mode: .default, before: firstManualPreviewDeadline)
        }
        precondition(manualStopPreviewCounts == [1])
        manualStopModule.testAppendPreviewFrame(repeatedNoFooterFrames[1])
        manualStopModule.testWaitForPreviewAlignment()
        let manualStopDeadline = Date(timeIntervalSinceNow: 5)
        while manualNoOverlapEvents.isEmpty && Date() < manualStopDeadline {
            _ = RunLoop.current.run(mode: .default, before: manualStopDeadline)
        }
        precondition((manualNoOverlapEvents.first?["reason"] as? String) == "no-overlap", "Manual scrolling must report an ambiguous repeated-content seam")
        precondition((manualNoOverlapEvents.first?["frameCount"] as? Int) == 1)
        precondition(manualStopPreviewCounts == [1], "Manual scrolling must keep the last accepted preview")
        let manualStopOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-manual-stop-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: manualStopOutput) }
        let manualStopResult = manualStopModule.testFinishCurrentCapture(outputPath: manualStopOutput.path)
        precondition((manualStopResult?["success"] as? Bool) == true)
        precondition((manualStopResult?["frameCount"] as? Int) == 1)
        precondition(manualNoOverlapEvents.count == 1)
        let manualStopSource = CGImageSourceCreateWithURL(manualStopOutput as CFURL, nil)!
        let manualStopSaved = CGImageSourceCreateImageAtIndex(manualStopSource, 0, nil)!
        precondition(pixels(manualStopSaved) == pixels(repeatedNoFooterFrames[0]))

        let uniqueFooterFrames = (0..<2).map {
            fixedFooterFrame(
                index: $0,
                periodic: false,
                width: footerWidth,
                height: footerHeight,
                footerHeight: footerSize,
                step: footerStep
            )
        }
        let uniqueFooterOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-unique-footer-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: uniqueFooterOutput) }
        let (
            uniqueFooterPreview,
            uniqueFooterSaved,
            _,
            uniqueFooterOverlaps,
            uniqueFooterFrameCount,
            uniqueFooterEstimatedHeight
        ) = ScrollCaptureModule().testPreviewAndSave(
            frames: uniqueFooterFrames,
            step: footerStep,
            outputPath: uniqueFooterOutput.path
        )
        precondition(uniqueFooterOverlaps == [footerHeight - footerSize - footerStep], "Unique content should align after excluding the fixed footer")
        precondition(uniqueFooterFrameCount == 2 && uniqueFooterEstimatedHeight == footerHeight + footerStep)
        precondition(uniqueFooterSaved.height == footerHeight + footerStep)
        precondition(pixels(uniqueFooterPreview) == pixels(uniqueFooterSaved), "Fixed-footer preview must match the saved capture")
        let expectedUniqueFooter = expectedFooterDocument(
            width: footerWidth,
            contentHeight: footerHeight - footerSize + footerStep,
            footerHeight: footerSize
        )
        precondition(pixels(uniqueFooterSaved) == pixels(expectedUniqueFooter), "The reopened PNG must contain every document row and one footer")

        let changingFooterFrames = (0..<2).map {
            fixedFooterFrame(
                index: $0,
                periodic: false,
                width: footerWidth,
                height: footerHeight,
                footerHeight: footerSize,
                step: footerStep,
                changingFooter: true
            )
        }
        let changingFooterDetection = ScrollCaptureModule().testFooterDetection(
            topFrame: changingFooterFrames[0],
            bottomFrame: changingFooterFrames[1]
        )
        precondition(changingFooterDetection?.height == footerSize && changingFooterDetection?.isExact == false)
        let changingFooterOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-changing-footer-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: changingFooterOutput) }
        let (
            changingFooterPreview,
            changingFooterSaved,
            _,
            changingFooterOverlaps,
            changingFooterFrameCount,
            changingFooterEstimatedHeight
        ) = ScrollCaptureModule().testPreviewAndSave(
            frames: changingFooterFrames,
            step: footerStep,
            outputPath: changingFooterOutput.path
        )
        precondition(changingFooterOverlaps == [footerHeight - footerSize - footerStep])
        precondition(changingFooterFrameCount == 2 && changingFooterEstimatedHeight == footerHeight + footerStep)
        precondition(pixels(changingFooterPreview) == pixels(changingFooterSaved))
        precondition(
            pixels(changingFooterSaved) == pixels(expectedFooterDocument(
                width: footerWidth,
                contentHeight: footerHeight - footerSize + footerStep,
                footerHeight: footerSize,
                footerOffset: 11
            )),
            "A one-level animated footer must not replace document rows or appear twice"
        )

        let unprovableFooterFrames = (0..<2).map {
            fixedFooterFrame(
                index: $0,
                periodic: false,
                width: footerWidth,
                height: footerHeight,
                footerHeight: footerSize,
                step: 180,
                changingFooter: true
            )
        }
        let unprovableFooterOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-unprovable-footer-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: unprovableFooterOutput) }
        let unprovableFooterResult = ScrollCaptureModule().testFinishReportsAcceptedStitchPrefix(
            frames: unprovableFooterFrames,
            step: footerStep,
            outputPath: unprovableFooterOutput.path
        )
        precondition((unprovableFooterResult?["success"] as? Bool) == true)
        precondition((unprovableFooterResult?["frameCount"] as? Int) == 1, "A near-static footer without a proven document seam must preserve only the accepted prefix")
        let unprovableFooterSource = CGImageSourceCreateWithURL(unprovableFooterOutput as CFURL, nil)!
        let unprovableFooterSaved = CGImageSourceCreateImageAtIndex(unprovableFooterSource, 0, nil)!
        precondition(pixels(unprovableFooterSaved) == pixels(unprovableFooterFrames[0]))

        let blankTailFrames = (0..<2).map {
            fixedFooterFrame(
                index: $0,
                periodic: false,
                width: footerWidth,
                height: footerHeight,
                footerHeight: footerSize,
                step: footerStep,
                blankTail: true
            )
        }
        let blankTailOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-blank-tail-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: blankTailOutput) }
        let (
            blankTailPreview,
            blankTailSaved,
            _,
            blankTailOverlaps,
            blankTailFrameCount,
            blankTailEstimatedHeight
        ) = ScrollCaptureModule().testPreviewAndSave(
            frames: blankTailFrames,
            step: footerStep,
            outputPath: blankTailOutput.path
        )
        precondition(blankTailOverlaps == [footerHeight - footerSize - footerStep])
        precondition(blankTailFrameCount == 2 && blankTailEstimatedHeight == footerHeight + footerStep)
        precondition(pixels(blankTailPreview) == pixels(blankTailSaved))
        let expectedBlankTail = expectedFooterDocument(
            width: footerWidth,
            contentHeight: footerHeight - footerSize + footerStep,
            footerHeight: footerSize,
            blankTail: true
        )
        precondition(pixels(blankTailSaved) == pixels(expectedBlankTail), "Identical blank tail rows must occur once in the output")
        print("macOS scroll captures match saved pixels for nominal and measured overlap and suppress stale previews")
    }
}
