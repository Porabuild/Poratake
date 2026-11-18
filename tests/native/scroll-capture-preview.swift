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

    static func fixedFooterFrame(index: Int, periodic: Bool, width: Int, height: Int, footerHeight: Int, step: Int, blankTail: Bool = false, changingTail: Bool = false, bodyNoise: Int = 0, changingFooter: Bool = false, headerHeight: Int = 0) -> CGImage {
        var pixels = [UInt8]()
        pixels.reserveCapacity(width * height * 4)
        for y in 0..<height {
            for x in 0..<width {
                var pixel: [UInt8]
                if y < headerHeight {
                    let noise = x == 0 ? index * bodyNoise : 0
                    pixel = [255, UInt8(truncatingIfNeeded: y + noise), UInt8(truncatingIfNeeded: x), 255]
                } else if y >= height - footerHeight {
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
                if periodic && y >= headerHeight && (1..<11).contains(x) && (32..<42).contains(y) {
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
        let application = NSApplication.shared
        application.setActivationPolicy(.accessory)
        Task { @MainActor in
            do {
                try await runRecordingCoordinationChecks()
                exit(EXIT_SUCCESS)
            } catch {
                FileHandle.standardError.write(Data("Recording coordination checks failed: \(error)\n".utf8))
                exit(EXIT_FAILURE)
            }
        }
        application.run()
    }

    @MainActor
    static func runRecordingCoordinationChecks() async throws {
        await runLegacyScrollCoordinationCheck()
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
            cameraTrackMissing: false,
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

        var startingLifecycle = RecorderModuleLifecycle()
        let startingSession = startingLifecycle.beginStart(.screen)!
        var releaseStart: CheckedContinuation<Void, Never>?
        let startup = Task { @MainActor in
            await withCheckedContinuation { releaseStart = $0 }
            precondition(startingLifecycle.finishStart(startingSession, success: true))
        }
        let waitingCoordinator = RecorderStopCoordinator()
        var delayedStopCount = 0
        let waitingStop = Task { @MainActor in
            try await waitingCoordinator.stop(sessionID: startingSession, startup: startup) {
                precondition(startingLifecycle.beginStop(startingSession) == .screen)
                delayedStopCount += 1
                startingLifecycle.finishStop(startingSession)
                return stopResult
            }
        }
        await waitUntil("Stop did not wait for startup") {
            waitingCoordinator.testWaitingForStart && releaseStart != nil
        }
        let joiningStop = Task { @MainActor in
            try await waitingCoordinator.stop(sessionID: startingSession, startup: startup) {
                delayedStopCount += 1
                return stopResult
            }
        }
        await waitUntil("The second stop did not join startup finalization") {
            waitingCoordinator.testWaiterCount == 1
        }
        precondition(delayedStopCount == 0)
        releaseStart?.resume()
        let waitingResult = try await waitingStop.value
        let joiningResult = try await joiningStop.value
        precondition(waitingResult.outputPath == stopResult.outputPath)
        precondition(joiningResult.outputPath == stopResult.outputPath)
        precondition(delayedStopCount == 1)
        precondition(startingLifecycle.phase == .idle)

        let failedStartingSession = startingLifecycle.beginStart(.ios)!
        let failedStartup = Task { @MainActor in
            precondition(startingLifecycle.finishStart(failedStartingSession, success: false))
        }
        var failedStartupStopRejected = false
        do {
            _ = try await waitingCoordinator.stop(sessionID: failedStartingSession, startup: failedStartup) {
                guard startingLifecycle.beginStop(failedStartingSession) != nil else {
                    throw RecordingCaptureFilterError.unavailableWindows([203])
                }
                return stopResult
            }
        } catch is RecordingCaptureFilterError {
            failedStartupStopRejected = true
        }
        precondition(failedStartupStopRejected)
        precondition(startingLifecycle.phase == .idle)

        let staleSession = startingLifecycle.beginStart(.screen)!
        var replacementSession: RecorderSessionID?
        let replacedStartup = Task { @MainActor in
            precondition(startingLifecycle.finishStart(staleSession, success: false))
            replacementSession = startingLifecycle.beginStart(.ios)!
            precondition(startingLifecycle.finishStart(replacementSession!, success: true))
        }
        var staleStopRejected = false
        do {
            _ = try await waitingCoordinator.stop(sessionID: staleSession, startup: replacedStartup) {
                guard startingLifecycle.beginStop(staleSession) != nil else {
                    throw RecordingCaptureFilterError.unavailableWindows([204])
                }
                return stopResult
            }
        } catch is RecordingCaptureFilterError {
            staleStopRejected = true
        }
        precondition(staleStopRejected)
        precondition(startingLifecycle.isCurrent(replacementSession!))
        precondition(startingLifecycle.currentBackend == .ios)
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

        FileHandle.standardError.write(Data("capture screenshot barrier: start\n".utf8))
        let screenshotRegistry = RecordingCaptureWindowRegistry()
        screenshotRegistry.register(windowID: 61)
        let firstSnapshot = screenshotRegistry.beginScreenshot()
        screenshotRegistry.register(windowID: 62)
        let secondSnapshot = screenshotRegistry.beginScreenshot()
        precondition(firstSnapshot == [61])
        precondition(secondSnapshot == [61, 62])
        var mayShow = false
        let barrierPresentation = Task { @MainActor in
            await screenshotRegistry.waitForScreenshots()
            mayShow = true
        }
        await waitUntil("The panel did not wait for existing screenshots") {
            screenshotRegistry.testPendingScreenshotWaiterCount == 1
        }
        screenshotRegistry.finishScreenshot()
        precondition(!mayShow)
        screenshotRegistry.finishScreenshot()
        let thirdSnapshot = screenshotRegistry.beginScreenshot()
        precondition(thirdSnapshot == [61, 62])
        await waitUntil("The panel did not wait for a newly started screenshot") {
            screenshotRegistry.testPendingScreenshotWaiterCount == 1
        }
        precondition(!mayShow)
        screenshotRegistry.finishScreenshot()
        await barrierPresentation.value
        precondition(mayShow)
        FileHandle.standardError.write(Data("capture screenshot barrier: complete\n".utf8))

        FileHandle.standardError.write(Data("direct screenshot coordination: start\n".utf8))
        let directRegistry = RecordingCaptureWindowRegistry()
        var directStarted = false
        var releaseDirect: CheckedContinuation<Void, Never>?
        let directCapture = Task { @MainActor in
            await coordinateAreaCapture(
                registry: directRegistry,
                excludedWindowIDs: [],
                unfiltered: {
                    directStarted = true
                    await withCheckedContinuation { releaseDirect = $0 }
                    return "direct"
                },
                filtered: { _ in "unexpected" }
            )
        }
        await waitUntil("The direct screenshot did not start") { directStarted }
        directRegistry.register(windowID: 73)
        var directMayShow = false
        let directPresentation = Task { @MainActor in
            await directRegistry.waitForScreenshots()
            directMayShow = true
        }
        await waitUntil("The panel did not wait for the direct screenshot") {
            directRegistry.testPendingScreenshotWaiterCount == 1
        }
        precondition(!directMayShow)
        releaseDirect?.resume()
        let directResult = await directCapture.value
        await directPresentation.value
        precondition(directResult == "direct" && directMayShow)
        let filteredIDs = await coordinateAreaCapture(
            registry: directRegistry,
            excludedWindowIDs: [74],
            unfiltered: { Set<CGWindowID>() },
            filtered: { $0 }
        )
        precondition(filteredIDs == [73, 74])
        var filteredFailed = false
        do {
            _ = try await coordinateAreaCapture(
                registry: directRegistry,
                excludedWindowIDs: [],
                unfiltered: { 0 },
                filtered: { _ in throw RecordingCaptureFilterError.unavailableWindows([73]) }
            )
        } catch is RecordingCaptureFilterError {
            filteredFailed = true
        }
        precondition(filteredFailed)
        await directRegistry.waitForScreenshots()
        FileHandle.standardError.write(Data("direct screenshot coordination: complete\n".utf8))

        FileHandle.standardError.write(Data("screenshot frame deadline: start\n".utf8))
        var lateFrame: (@Sendable (Int?, Error?) -> Void)?
        do {
            let unexpectedFrame: Int = try await coordinateAreaCapture(
                registry: directRegistry,
                excludedWindowIDs: [],
                unfiltered: { preconditionFailure("The registered window must stay excluded") },
                filtered: { _ in
                    try await awaitScreenshotFrame(timeout: .zero) { lateFrame = $0 }
                }
            )
            preconditionFailure("A screenshot without a frame returned \(unexpectedFrame)")
        } catch ScreenshotFrameError.timedOut {}
        guard let lateFrame else { preconditionFailure("The screenshot must have requested a frame") }
        lateFrame(41, nil)
        await directRegistry.waitForScreenshots()
        let nextFrame: Int = try await awaitScreenshotFrame { $0(42, nil) }
        precondition(nextFrame == 42)
        FileHandle.standardError.write(Data("screenshot frame deadline: complete\n".utf8))

        FileHandle.standardError.write(Data("protected tooltip lifecycle: start\n".utf8))
        _ = NSApplication.shared
        if let screen = NSScreen.screens.first {
            let point = NSPoint(x: screen.visibleFrame.minX + 10, y: screen.visibleFrame.minY + 10)
            let tooltipRegistry = RecordingCaptureWindowRegistry()
            var updates: [[CGWindowID]] = []
            tooltipRegistry.setFilterUpdater { updates.append($0) }
            let tooltip = ProtectedTooltip(registry: tooltipRegistry)
            _ = tooltipRegistry.beginScreenshot()
            precondition(tooltip.accept(sequence: 1, owner: 101, text: "Copy"))
            let pendingTooltip = Task { @MainActor in
                try await tooltip.show(sequence: 1, owner: 101, text: "Copy", dark: false, at: point)
            }
            await waitUntil("The tooltip did not wait for the screenshot") {
                tooltipRegistry.testPendingScreenshotWaiterCount == 1 && tooltip.testPanel != nil
            }
            precondition(tooltip.testPanel?.alphaValue == 0)
            precondition(tooltip.testPanel?.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .aqua)
            precondition(tooltipRegistry.currentWindowIDs().count == 1)
            precondition(!updates.isEmpty && updates.last == tooltipRegistry.currentWindowIDs())
            precondition(!tooltip.accept(sequence: 2, owner: 101, text: nil))
            tooltipRegistry.finishScreenshot()
            let cancelledTooltipVisible = try await pendingTooltip.value
            precondition(!cancelledTooltipVisible)
            await waitUntil("The cancelled tooltip was not unregistered") {
                tooltipRegistry.currentWindowIDs().isEmpty
            }
            precondition(tooltip.accept(sequence: 3, owner: 102, text: "Stop auto-scroll · Esc"))
            let shownTooltip = try await tooltip.show(sequence: 3, owner: 102, text: "Stop auto-scroll · Esc", dark: true, at: point)
            precondition(shownTooltip)
            precondition(tooltip.testPanel?.alphaValue == 1)
            precondition(tooltip.testPanel?.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua)
            precondition(tooltipRegistry.currentWindowIDs().count == 1)
            precondition(!tooltip.accept(sequence: 2, owner: 102, text: nil))
            precondition(tooltip.testPanel?.alphaValue == 1)
            precondition(!tooltip.accept(sequence: 5, owner: 102, text: nil))
            await waitUntil("The hidden tooltip was not unregistered") {
                tooltipRegistry.currentWindowIDs().isEmpty
            }
            precondition(tooltip.accept(sequence: 6, owner: 101, text: "First"))
            let firstVisible = try await tooltip.show(sequence: 6, owner: 101, text: "First", dark: false, at: point)
            precondition(firstVisible)
            let firstPanel = tooltip.testPanel
            precondition(!tooltip.accept(sequence: 8, owner: 102, text: nil))
            precondition(firstPanel?.alphaValue == 0)
            precondition(!tooltip.accept(sequence: 7, owner: 102, text: "Second"))
            await waitUntil("The superseded owner's tooltip was not unregistered") {
                tooltipRegistry.currentWindowIDs().isEmpty
            }
            precondition(tooltip.accept(sequence: 9, owner: 102, text: "Second"))
            precondition(!tooltip.accept(sequence: 8, owner: 102, text: nil))
            let secondVisible = try await tooltip.show(sequence: 9, owner: 102, text: "Second", dark: true, at: point)
            precondition(secondVisible)
            precondition(!tooltip.accept(sequence: 10, owner: 102, text: nil))
            await waitUntil("The replacement tooltip was not unregistered") {
                tooltipRegistry.currentWindowIDs().isEmpty
            }
            let longTooltipTexts = [
                "Copy with a very long preset name that should wrap across multiple lines and remain readable for the user",
                "Copy with 日本語の長いプリセット名と多言語の注釈を保存する設定日本語の長いプリセット名と多言語の注釈を保存する設定"
            ]
            for (index, item) in longTooltipTexts.flatMap({ text in [(text, false), (text, true)] }).enumerated() {
                let (longText, dark) = item
                let sequence = UInt64(11 + index * 2)
                precondition(tooltip.accept(sequence: sequence, owner: 104, text: longText))
                let visible = try await tooltip.show(sequence: sequence, owner: 104, text: longText, dark: dark, at: point)
                precondition(visible)
                guard let panel = tooltip.testPanel,
                      let label = panel.contentView?.subviews.compactMap({ $0 as? NSTextField }).first,
                      let bitmap = label.bitmapImageRepForCachingDisplay(in: label.bounds) else {
                    preconditionFailure("The tooltip label could not be rendered")
                }
                panel.displayIfNeeded()
                label.cacheDisplay(in: label.bounds, to: bitmap)
                let midpoint = bitmap.pixelsHigh / 2
                func hasInk(_ rows: Range<Int>) -> Bool {
                    for y in rows {
                        for x in 0..<bitmap.pixelsWide {
                            if (bitmap.colorAt(x: x, y: y)?.alphaComponent ?? 0) > 0.1 { return true }
                        }
                    }
                    return false
                }
                precondition(hasInk(0..<midpoint) && hasInk(midpoint..<bitmap.pixelsHigh),
                             "Long tooltip text must paint on both lines")
                precondition(!tooltip.accept(sequence: sequence + 1, owner: 104, text: nil))
                await waitUntil("The wrapped tooltip was not unregistered") {
                    tooltipRegistry.currentWindowIDs().isEmpty
                }
            }
            let edgeText = "FIRST LINE\nSECOND LINE\nTHIRD LINE\nFOURTH LINE\nFIFTH LINE\nSIXTH LINE"
            let visibleFrame = screen.visibleFrame
            let edgePoints = [
                NSPoint(x: visibleFrame.minX + 1, y: visibleFrame.minY + 1),
                NSPoint(x: visibleFrame.maxX - 1, y: visibleFrame.minY + 1),
                NSPoint(x: visibleFrame.minX + 1, y: visibleFrame.maxY - 1),
                NSPoint(x: visibleFrame.maxX - 1, y: visibleFrame.maxY - 1)
            ]
            for (index, edgePoint) in edgePoints.enumerated() {
                let sequence = UInt64(19 + index * 2)
                precondition(tooltip.accept(sequence: sequence, owner: 105, text: edgeText))
                let visible = try await tooltip.show(sequence: sequence, owner: 105, text: edgeText, dark: index % 2 == 1, at: edgePoint)
                precondition(visible)
                guard let panel = tooltip.testPanel,
                      let label = panel.contentView?.subviews.compactMap({ $0 as? NSTextField }).first,
                      let font = label.font,
                      let bitmap = label.bitmapImageRepForCachingDisplay(in: label.bounds) else {
                    preconditionFailure("The edge tooltip could not be rendered")
                }
                precondition(panel.frame.minX >= visibleFrame.minX && panel.frame.maxX <= visibleFrame.maxX)
                precondition(panel.frame.minY >= visibleFrame.minY && panel.frame.maxY <= visibleFrame.maxY)
                let lineHeight = NSLayoutManager().defaultLineHeight(for: font)
                precondition(panel.frame.height <= ceil(lineHeight * 4) + 12)
                precondition(label.cell?.truncatesLastVisibleLine == true)
                panel.displayIfNeeded()
                label.cacheDisplay(in: label.bounds, to: bitmap)
                let quarter = bitmap.pixelsHigh / 4
                for line in 0..<4 {
                    let rows = (line * quarter)..<(line == 3 ? bitmap.pixelsHigh : (line + 1) * quarter)
                    let ink = rows.contains { y in
                        (0..<bitmap.pixelsWide).contains { x in
                            (bitmap.colorAt(x: x, y: y)?.alphaComponent ?? 0) > 0.1
                        }
                    }
                    precondition(ink, "Every visible tooltip line must be painted")
                }
                precondition(!tooltip.accept(sequence: sequence + 1, owner: 105, text: nil))
                await waitUntil("The edge tooltip was not unregistered") {
                    tooltipRegistry.currentWindowIDs().isEmpty
                }
            }
            let recordingFrame = NSRect(
                x: visibleFrame.midX - 130,
                y: visibleFrame.minY + 100,
                width: 260,
                height: 44
            )
            let recordingPoint = NSPoint(x: recordingFrame.minX + 22, y: recordingFrame.midY)
            precondition(tooltip.accept(sequence: 27, owner: 106, text: "Start recording"))
            let recordingTooltipVisible = try await tooltip.show(
                sequence: 27,
                owner: 106,
                text: "Start recording",
                dark: true,
                at: recordingPoint,
                ownerFrame: recordingFrame
            )
            precondition(recordingTooltipVisible)
            guard let recordingTooltipPanel = tooltip.testPanel else {
                preconditionFailure("The recording tooltip panel was not created")
            }
            precondition(!recordingTooltipPanel.frame.intersects(recordingFrame))
            precondition(recordingTooltipPanel.frame.maxY <= recordingFrame.minY - 4)
            precondition(!tooltip.accept(sequence: 28, owner: 106, text: nil))
            await waitUntil("The recording tooltip was not unregistered") {
                tooltipRegistry.currentWindowIDs().isEmpty
            }

            let bottomFrame = NSRect(
                x: visibleFrame.midX - 130,
                y: visibleFrame.minY + 2,
                width: 260,
                height: 44
            )
            let bottomPoint = NSPoint(x: bottomFrame.minX + 22, y: bottomFrame.midY)
            precondition(tooltip.accept(sequence: 29, owner: 107, text: "Close preview"))
            let bottomTooltipVisible = try await tooltip.show(
                sequence: 29,
                owner: 107,
                text: "Close preview",
                dark: false,
                at: bottomPoint,
                ownerFrame: bottomFrame
            )
            precondition(bottomTooltipVisible)
            guard let bottomTooltipPanel = tooltip.testPanel else {
                preconditionFailure("The bottom-edge tooltip panel was not created")
            }
            precondition(!bottomTooltipPanel.frame.intersects(bottomFrame))
            precondition(bottomTooltipPanel.frame.minY >= bottomFrame.maxY + 4)
            precondition(!tooltip.accept(sequence: 30, owner: 107, text: nil))
            await waitUntil("The bottom-edge tooltip was not unregistered") {
                tooltipRegistry.currentWindowIDs().isEmpty
            }

            let centeredTexts = ["Start recording", longTooltipTexts[0]]
            for (index, item) in centeredTexts.flatMap({ text in [(text, false), (text, true)] }).enumerated() {
                let (text, dark) = item
                let sequence = UInt64(31 + index * 2)
                precondition(tooltip.accept(sequence: sequence, owner: 108, text: text))
                let visible = try await tooltip.show(sequence: sequence, owner: 108, text: text, dark: dark, at: point)
                precondition(visible)
                guard let panel = tooltip.testPanel,
                      let content = panel.contentView,
                      let label = content.subviews.compactMap({ $0 as? NSTextField }).first,
                      let bitmap = NSBitmapImageRep(
                        bitmapDataPlanes: nil,
                        pixelsWide: Int(label.bounds.width * 2),
                        pixelsHigh: Int(label.bounds.height * 2),
                        bitsPerSample: 8,
                        samplesPerPixel: 4,
                        hasAlpha: true,
                        isPlanar: false,
                        colorSpaceName: .deviceRGB,
                        bytesPerRow: 0,
                        bitsPerPixel: 0
                      ) else {
                    preconditionFailure("The tooltip label could not be rendered at 2x")
                }
                bitmap.size = label.bounds.size
                panel.displayIfNeeded()
                label.cacheDisplay(in: label.bounds, to: bitmap)
                let inkRows = (0..<bitmap.pixelsHigh).filter { y in
                    (0..<bitmap.pixelsWide).contains { x in
                        (bitmap.colorAt(x: x, y: y)?.alphaComponent ?? 0) > 0.1
                    }
                }
                guard let first = inkRows.first, let last = inkRows.last else {
                    preconditionFailure("The tooltip label did not paint")
                }
                let top = Int((content.bounds.height - label.frame.maxY) * 2) + first
                let bottom = Int(label.frame.minY * 2) + bitmap.pixelsHigh - 1 - last
                precondition(abs(top - bottom) <= 3, "Tooltip label ink is not vertically centered: top \(top), bottom \(bottom)")
                precondition(!tooltip.accept(sequence: sequence + 1, owner: 108, text: nil))
                await waitUntil("The centered tooltip was not unregistered") {
                    tooltipRegistry.currentWindowIDs().isEmpty
                }
            }

            let failureRegistry = RecordingCaptureWindowRegistry()
            failureRegistry.setFilterUpdater { _ in throw RecordingCaptureFilterError.unavailableWindows([1]) }
            let failedTooltip = ProtectedTooltip(registry: failureRegistry)
            precondition(failedTooltip.accept(sequence: 1, owner: 103, text: "Copy"))
            var tooltipFailed = false
            do {
                _ = try await failedTooltip.show(sequence: 1, owner: 103, text: "Copy", dark: false, at: point)
            } catch is RecordingCaptureFilterError {
                tooltipFailed = true
            }
            precondition(tooltipFailed && failedTooltip.testPanel == nil && failureRegistry.currentWindowIDs().isEmpty)
            FileHandle.standardError.write(Data("protected tooltip lifecycle: complete\n".utf8))
        } else {
            FileHandle.standardError.write(Data("SKIP protected tooltip lifecycle: no AppKit display\n".utf8))
        }

        FileHandle.standardError.write(Data("scroll capture UI cleanup: start\n".utf8))
        if let screen = NSScreen.screens.first {
            let module = ScrollCaptureModule()
            let frame = screen.visibleFrame
            let windows = module.testShowCaptureUI(rect: NSRect(x: frame.minX + 100, y: frame.minY + 100,
                                                                 width: 80, height: 100))
            let ids = windows.map { CGWindowID($0.windowNumber) }
            func onScreenAlphas() -> [CGWindowID: Double] {
                let entries = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] ?? []
                var alphas: [CGWindowID: Double] = [:]
                for entry in entries {
                    if let id = entry[kCGWindowNumber as String] as? UInt32,
                       let alpha = entry[kCGWindowAlpha as String] as? Double, ids.contains(id) {
                        alphas[id] = alpha
                    }
                }
                return alphas
            }
            await waitUntil("Scroll controls were not visible to the window server") {
                let alphas = onScreenAlphas()
                return ids.allSatisfy { alphas[$0] == 1 }
            }
            module.testHideCaptureUI()
            let alphas = onScreenAlphas()
            precondition(ids.allSatisfy { alphas[$0].map { $0 == 0 } ?? true },
                         "Scroll controls remained opaque after cleanup")
            FileHandle.standardError.write(Data("scroll capture UI cleanup: complete\n".utf8))
        } else {
            FileHandle.standardError.write(Data("SKIP scroll capture UI cleanup: no AppKit display\n".utf8))
        }

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
    static func runLegacyScrollCoordinationCheck() async {
        FileHandle.standardError.write(Data("legacy scroll capture coordination: start\n".utf8))
        let availableIDs = Set(NSScreen.screens.compactMap {
            $0.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID
        })
        guard let availableID = availableIDs.first else { preconditionFailure("The fixture needs a display") }
        let unavailableID = availableIDs.contains(CGDirectDisplayID.max) ? CGDirectDisplayID(0) : CGDirectDisplayID.max
        precondition(!availableIDs.contains(unavailableID))
        let selectionModule = ScrollCaptureModule()
        func startSelection(_ extra: [String: AnyCodable], id: String) {
            var params: [String: AnyCodable] = [
                "x": AnyCodable(0), "y": AnyCodable(0), "width": AnyCodable(80), "height": AnyCodable(160)
            ]
            params.merge(extra) { _, new in new }
            selectionModule.handle(method: DaemonContract.ScrollCapture.Method.start.rawValue, params: params, requestId: id)
        }
        startSelection(["displayId": AnyCodable(Int(unavailableID))], id: "unavailable-display")
        precondition(!selectionModule.testIsCapturing(), "An unavailable display must not start capture")
        startSelection(["displayOriginX": AnyCodable(Int(Int32.max)), "displayOriginY": AnyCodable(Int(Int32.max))], id: "unavailable-origin")
        precondition(!selectionModule.testIsCapturing(), "An unavailable origin must not start capture")
        startSelection(["displayId": AnyCodable(Int(availableID))], id: "available-display")
        precondition(selectionModule.testIsCapturing(), "An available display must start capture")
        selectionModule.testCancelCapture()
        await Task.yield()
        precondition(!selectionModule.testIsCapturing())
        guard let selectedScreen = NSScreen.screens.first(where: {
            ($0.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID) == availableID
        }), let primaryScreen = NSScreen.screens.first(where: {
            $0.frame.minX.rounded() == 0 && $0.frame.minY.rounded() == 0
        }) else { preconditionFailure("The fixture needs screen geometry") }
        let originX = Int(selectedScreen.frame.minX.rounded())
        let originY = Int((primaryScreen.frame.height - selectedScreen.frame.maxY).rounded())
        startSelection(["displayOriginX": AnyCodable(originX), "displayOriginY": AnyCodable(originY)], id: "available-origin")
        precondition(selectionModule.testIsCapturing(), "An available display origin must start capture")
        startSelection(["displayId": AnyCodable(Int(unavailableID))], id: "unavailable-during-capture")
        precondition(selectionModule.testIsCapturing(), "A rejected request must preserve the active capture")
        selectionModule.testCancelCapture()
        await Task.yield()
        precondition(!selectionModule.testIsCapturing())

        let footerWidth = 80
        let footerHeight = 160
        let footerStep = 32
        let generatedFrame = frame(start: 0, width: footerWidth, height: footerHeight)
        let coordinatedModule = ScrollCaptureModule()
        coordinatedModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        coordinatedModule.testSetPreviewEncoder { _ in "preview" }
        var coordinatedRequests: [Set<CGWindowID>?] = []
        var coordinatedCallbacks: [(CGImage?) -> Void] = []
        var coordinatedSchedules = 0
        var coordinatedEvents: [[String: Any]] = []
        coordinatedModule.testSetCoordinatedCaptureProvider { ids, callback in
            coordinatedRequests.append(ids)
            coordinatedCallbacks.append(callback)
        }
        coordinatedModule.testSetScheduleAutoScroll { coordinatedSchedules += 1 }
        coordinatedModule.testSetScrollEndedResponder { coordinatedEvents.append($0) }
        var coordinatedGeneration = coordinatedModule.testCaptureGeneration()
        coordinatedModule.testCaptureAfterScroll(generation: coordinatedGeneration)
        await waitUntil("The direct scroll frame did not start") { coordinatedRequests.count == 1 }
        precondition(coordinatedRequests[0] == nil)
        let scrollRegistry = RecordingCaptureWindowRegistry.shared
        let protectedID: CGWindowID = 812345678
        scrollRegistry.register(windowID: protectedID)
        var overlayMayShow = false
        let overlayPresentation = Task { @MainActor in
            await scrollRegistry.waitForScreenshots()
            overlayMayShow = true
        }
        await waitUntil("The overlay did not wait for the scroll frame") {
            scrollRegistry.testPendingScreenshotWaiterCount > 0
        }
        precondition(!overlayMayShow)
        coordinatedCallbacks[0](generatedFrame)
        await waitUntil("The direct frame did not finish") { coordinatedSchedules == 1 }
        await overlayPresentation.value
        precondition(overlayMayShow)
        coordinatedModule.testCaptureAfterScroll(generation: coordinatedGeneration)
        coordinatedModule.testStopAutoScroll()
        coordinatedModule.testResumeAutoScroll()
        await Task.yield()
        precondition(coordinatedRequests.count == 1, "A stale scroll task must not start a capture")
        coordinatedGeneration = coordinatedModule.testCaptureGeneration()
        let scrollWindows = coordinatedModule.testShowCaptureUI(rect: NSRect(x: 10, y: 10, width: footerWidth, height: footerHeight))
        let scrollWindowIDs = Set(scrollWindows.compactMap { CGWindowID(exactly: $0.windowNumber) })
        precondition(scrollWindowIDs.count == scrollWindows.count)
        let expectedExclusions = scrollWindowIDs.union([protectedID])
        coordinatedModule.testCaptureAfterScroll(generation: coordinatedGeneration)
        await waitUntil("The filtered scroll frame did not start") { coordinatedRequests.count == 2 }
        precondition(coordinatedRequests[1] == expectedExclusions)
        coordinatedCallbacks[1](frame(start: footerStep, width: footerWidth, height: footerHeight))
        await waitUntil("The filtered frame did not finish") { coordinatedSchedules == 2 }
        coordinatedModule.testCaptureAfterScroll(generation: coordinatedGeneration)
        await waitUntil("The next filtered frame did not start") { coordinatedRequests.count == 3 }
        precondition(coordinatedRequests[2] == expectedExclusions)
        coordinatedCallbacks[2](nil)
        await waitUntil("The failed filtered frame did not stop capture") { !coordinatedModule.testIsAutoScrolling() }
        precondition((coordinatedEvents.first?["reason"] as? String) == "capture-error")
        let coordinatedOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-coordinated-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: coordinatedOutput) }
        var coordinatedResult: [String: Any]?
        coordinatedModule.testSetFinishResponder { _, result in coordinatedResult = result }
        coordinatedModule.handle(
            method: DaemonContract.ScrollCapture.Method.finish.rawValue,
            params: ["outputPath": AnyCodable(coordinatedOutput.path)],
            requestId: "coordinated-scroll-finish"
        )
        await waitUntil("The coordinated scroll capture did not finish") { coordinatedResult != nil }
        precondition((coordinatedResult?["frameCount"] as? Int) == 2)
        let coordinatedSource = CGImageSourceCreateWithURL(coordinatedOutput as CFURL, nil)!
        let coordinatedSaved = CGImageSourceCreateImageAtIndex(coordinatedSource, 0, nil)!
        let expectedCapture = frame(start: 0, width: footerWidth, height: footerHeight + footerStep)
        precondition(pixels(coordinatedSaved) == pixels(expectedCapture))
        coordinatedModule.testHideCaptureUI()
        scrollRegistry.unregister(windowID: protectedID)
        await scrollRegistry.waitForScreenshots()
        FileHandle.standardError.write(Data("legacy scroll capture coordination: complete\n".utf8))
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
        let blackFrame = black.makeImage()!
        let (hash, results, count, restarted, cleared) = ScrollCaptureModule()
            .testFrameSequence(Array(repeating: blackFrame, count: 4))
        precondition(hash == 0)
        precondition(results.count == 4 && results[0] == 0 && results[1] == 0 && results[2] == 0 && results[3] == nil)
        precondition(count == 1, "The first frame must be captured even when its hash is zero")
        precondition(restarted == 0, "A new session must not reuse the previous frame hash")
        precondition(cleared, "Cleanup must clear the previous frame hash")

        func frameBelowFixedHeader(green: CGFloat) -> CGImage {
            let context = CGContext(
                data: nil,
                width: 80,
                height: 240,
                bitsPerComponent: 8,
                bytesPerRow: 80 * 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            )!
            context.setFillColor(red: 0.25, green: 0.5, blue: 0.5, alpha: 1)
            context.fill(CGRect(x: 0, y: 0, width: 80, height: 240))
            context.setFillColor(red: 0.25, green: green, blue: 0.5, alpha: 1)
            context.fill(CGRect(x: 0, y: 0, width: 80, height: 140))
            return context.makeImage()!
        }
        let (_, headerResults, headerCount, _, _) = ScrollCaptureModule().testFrameSequence(
            [0.2, 0.3, 0.4, 0.6, 0.6, 0.6, 0.6].map { frameBelowFixedHeader(green: $0) }
        )
        precondition(headerResults == [0, 1, 2, 3, 3, 3, nil] && headerCount == 4,
                     "Changed body pixels below a fixed header must survive duplicate detection")
        black.clear(CGRect(x: 0, y: 0, width: width, height: height))
        let (_, alphaResults, alphaCount, _, _) = ScrollCaptureModule()
            .testFrameSequence([blackFrame, black.makeImage()!])
        precondition(alphaResults == [0, 1] && alphaCount == 2,
                     "An RGB fingerprint collision must not discard changed alpha")

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

        let headerRows = 46
        let bodyShift = 86
        for footerRows in [0, 12] {
            let viewportHeight = 160
            let frameCount = 3
            let frames = (0..<frameCount).map { index in
                fixedFooterFrame(
                    index: index, periodic: false, width: width, height: viewportHeight,
                    footerHeight: footerRows, step: bodyShift, headerHeight: headerRows
                )
            }
            let expectedOverlap = viewportHeight - footerRows - bodyShift
            precondition(ScrollCaptureModule().testFindOverlap(
                topFrame: frames[0], bottomFrame: frames[1], expectedOverlap: viewportHeight - bodyShift + 11
            ) == expectedOverlap, "An exact body match below the fixed header must align the whole viewport")
            let output = URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent("poratake-scroll-fixed-header-seam-\(UUID().uuidString).png")
            defer { try? FileManager.default.removeItem(at: output) }
            let (preview, saved, _, overlaps, count, estimatedHeight) = ScrollCaptureModule().testPreviewAndSave(
                frames: frames, step: bodyShift - 11, outputPath: output.path
            )
            let expectedHeight = viewportHeight + (frameCount - 1) * bodyShift
            let document = fixedFooterFrame(
                index: 0, periodic: false, width: width, height: expectedHeight,
                footerHeight: footerRows, step: bodyShift, headerHeight: headerRows
            )
            precondition(overlaps == Array(repeating: expectedOverlap, count: frameCount - 1))
            precondition(count == frameCount && estimatedHeight == expectedHeight)
            precondition(pixels(preview) == pixels(saved))
            precondition(pixels(saved) == pixels(document), "The saved scroll capture must contain all body rows and one header")

            let noisyFrames = (0..<2).map { index in
                fixedFooterFrame(
                    index: index, periodic: false, width: width, height: viewportHeight,
                    footerHeight: footerRows, step: bodyShift, bodyNoise: 1, headerHeight: headerRows
                )
            }
            precondition(ScrollCaptureModule().testFindOverlap(
                topFrame: noisyFrames[0], bottomFrame: noisyFrames[1], expectedOverlap: viewportHeight - bodyShift + 11
            ) == expectedOverlap, "A short visible seam with capture noise must align below the header")
        }
        for shift in [84, 85] {
            let repeatedBodyFrames = (0..<2).map { index in
                fixedFooterFrame(
                    index: index, periodic: true, width: width, height: 160,
                    footerHeight: 0, step: shift, headerHeight: headerRows
                )
            }
            precondition(ScrollCaptureModule().testFindOverlap(
                topFrame: repeatedBodyFrames[0], bottomFrame: repeatedBodyFrames[1], expectedOverlap: 160 - shift
            ) == nil, "A fixed header must not make multiple equally valid body overlaps acceptable")
        }

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

        let asyncCaptureModule = ScrollCaptureModule()
        asyncCaptureModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        asyncCaptureModule.testSetPreviewEncoder { _ in "preview" }
        let staleCaptureGeneration = asyncCaptureModule.testCaptureGeneration()
        asyncCaptureModule.testStopAutoScroll()
        asyncCaptureModule.testResumeAutoScroll()
        let currentCaptureGeneration = asyncCaptureModule.testCaptureGeneration()
        precondition(staleCaptureGeneration != currentCaptureGeneration)
        precondition(asyncCaptureModule.testCompleteExcludedCapture(periodicFooterFrames[1], generation: staleCaptureGeneration) == nil)
        precondition(asyncCaptureModule.testCompleteExcludedCapture(periodicFooterFrames[0], generation: currentCaptureGeneration) == true)
        let asyncCaptureOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-async-capture-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: asyncCaptureOutput) }
        let asyncCaptureResult = asyncCaptureModule.testFinishCurrentCapture(outputPath: asyncCaptureOutput.path)
        precondition((asyncCaptureResult?["success"] as? Bool) == true)
        precondition((asyncCaptureResult?["frameCount"] as? Int) == 1)
        precondition(asyncCaptureModule.testCompleteExcludedCapture(periodicFooterFrames[1], generation: currentCaptureGeneration) == nil)
        let asyncCaptureSource = CGImageSourceCreateWithURL(asyncCaptureOutput as CFURL, nil)!
        let asyncCaptureSaved = CGImageSourceCreateImageAtIndex(asyncCaptureSource, 0, nil)!
        precondition(pixels(asyncCaptureSaved) == pixels(periodicFooterFrames[0]))

        let delayedCaptureModule = ScrollCaptureModule()
        delayedCaptureModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        delayedCaptureModule.testSetPreviewEncoder { _ in "preview" }
        var pendingCaptures: [(CGImage?) -> Void] = []
        var scheduledScrollSteps = 0
        var delayedCaptureEvents: [[String: Any]] = []
        delayedCaptureModule.testSetCaptureProvider { pendingCaptures.append($0) }
        delayedCaptureModule.testSetScheduleAutoScroll { scheduledScrollSteps += 1 }
        delayedCaptureModule.testSetScrollEndedResponder { delayedCaptureEvents.append($0) }
        let delayedOldGeneration = delayedCaptureModule.testCaptureGeneration()
        delayedCaptureModule.testStopAutoScroll()
        delayedCaptureModule.testResumeAutoScroll()
        let delayedCurrentGeneration = delayedCaptureModule.testCaptureGeneration()
        delayedCaptureModule.testCaptureAfterScroll(generation: delayedOldGeneration)
        precondition(pendingCaptures.isEmpty, "A delayed request from a stopped run must not capture after restart")
        delayedCaptureModule.testCaptureAfterScroll(generation: delayedCurrentGeneration)
        precondition(pendingCaptures.count == 1)
        delayedCaptureModule.testStopAutoScroll()
        delayedCaptureModule.testResumeAutoScroll()
        pendingCaptures[0](periodicFooterFrames[1])
        precondition(scheduledScrollSteps == 0, "A stale frame must not schedule a scroll timer")
        let delayedNewGeneration = delayedCaptureModule.testCaptureGeneration()
        delayedCaptureModule.testCaptureAfterScroll(generation: delayedNewGeneration)
        precondition(pendingCaptures.count == 2)
        pendingCaptures[1](periodicFooterFrames[0])
        precondition(scheduledScrollSteps == 1)
        delayedCaptureModule.testCaptureAfterScroll(generation: delayedNewGeneration)
        precondition(pendingCaptures.count == 3)
        pendingCaptures[2](nil)
        precondition(!delayedCaptureModule.testIsAutoScrolling())
        precondition(scheduledScrollSteps == 1)
        precondition((delayedCaptureEvents.first?["reason"] as? String) == "capture-error")
        precondition((delayedCaptureEvents.first?["frameCount"] as? Int) == 1)
        let delayedCaptureOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-delayed-capture-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: delayedCaptureOutput) }
        let delayedCaptureResult = delayedCaptureModule.testFinishCurrentCapture(outputPath: delayedCaptureOutput.path)
        precondition((delayedCaptureResult?["success"] as? Bool) == true)
        precondition((delayedCaptureResult?["frameCount"] as? Int) == 1)
        delayedCaptureModule.testCaptureAfterScroll(generation: delayedNewGeneration)
        precondition(pendingCaptures.count == 3)
        let delayedCaptureSource = CGImageSourceCreateWithURL(delayedCaptureOutput as CFURL, nil)!
        let delayedCaptureSaved = CGImageSourceCreateImageAtIndex(delayedCaptureSource, 0, nil)!
        precondition(pixels(delayedCaptureSaved) == pixels(periodicFooterFrames[0]))

        let staleStartModule = ScrollCaptureModule()
        staleStartModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        staleStartModule.testStopAutoScroll()
        var staleStartCaptures = 0
        staleStartModule.testSetCaptureProvider { _ in staleStartCaptures += 1 }
        staleStartModule.testEnqueueAutoScrollStart()
        staleStartModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        staleStartModule.testStopAutoScroll()
        var staleStartDrained = false
        DispatchQueue.main.async { staleStartDrained = true }
        let staleStartDeadline = Date(timeIntervalSinceNow: 5)
        while !staleStartDrained && Date() < staleStartDeadline {
            _ = RunLoop.current.run(mode: .default, before: staleStartDeadline)
        }
        precondition(staleStartDrained)
        precondition(!staleStartModule.testIsAutoScrolling(), "An old start request must not start a new capture session")
        precondition(staleStartCaptures == 0)

        let staleStopModule = ScrollCaptureModule()
        staleStopModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        staleStopModule.testEnqueueAutoScrollStop()
        staleStopModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        var staleStopDrained = false
        DispatchQueue.main.async { staleStopDrained = true }
        let staleStopDeadline = Date(timeIntervalSinceNow: 5)
        while !staleStopDrained && Date() < staleStopDeadline {
            _ = RunLoop.current.run(mode: .default, before: staleStopDeadline)
        }
        precondition(staleStopDrained)
        precondition(staleStopModule.testIsAutoScrolling(), "An old stop request must not stop a new capture session")

        let rapidStopModule = ScrollCaptureModule()
        rapidStopModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        rapidStopModule.testStopAutoScroll()
        var rapidStopCaptures = 0
        rapidStopModule.testSetCaptureProvider { _ in rapidStopCaptures += 1 }
        rapidStopModule.testEnqueueAutoScrollStart()
        rapidStopModule.testEnqueueAutoScrollStop()
        var rapidStopDrained = false
        DispatchQueue.main.async { rapidStopDrained = true }
        let rapidStopDeadline = Date(timeIntervalSinceNow: 5)
        while !rapidStopDrained && Date() < rapidStopDeadline {
            _ = RunLoop.current.run(mode: .default, before: rapidStopDeadline)
        }
        precondition(rapidStopDrained)
        precondition(!rapidStopModule.testIsAutoScrolling(), "A stop must cancel a queued auto-scroll start")
        precondition(rapidStopCaptures == 0)

        let staleSessionModule = ScrollCaptureModule()
        var staleSessionStarts: [Bool] = []
        staleSessionModule.testSetCaptureStartResult { staleSessionStarts.append($0) }
        staleSessionModule.testEnqueueCaptureStart(width: footerWidth, height: footerHeight)
        staleSessionModule.testCancelCapture()
        staleSessionModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        var staleSessionDrained = false
        DispatchQueue.main.async { staleSessionDrained = true }
        let staleSessionDeadline = Date(timeIntervalSinceNow: 5)
        while !staleSessionDrained && Date() < staleSessionDeadline {
            _ = RunLoop.current.run(mode: .default, before: staleSessionDeadline)
        }
        precondition(staleSessionDrained)
        precondition(staleSessionStarts == [false], "A cancelled session must not finish setup in a later session")

        let firstFrameFailureModule = ScrollCaptureModule()
        var firstFrameFailureEvents: [[String: Any]] = []
        firstFrameFailureModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        firstFrameFailureModule.testSetScrollEndedResponder { firstFrameFailureEvents.append($0) }
        precondition(!firstFrameFailureModule.testConsumeCaptureImage(nil))
        precondition(!firstFrameFailureModule.testIsAutoScrolling())
        precondition((firstFrameFailureEvents.first?["reason"] as? String) == "capture-error")
        precondition((firstFrameFailureEvents.first?["frameCount"] as? Int) == 0)

        let failedCaptureModule = ScrollCaptureModule()
        var failedCaptureEvents: [[String: Any]] = []
        failedCaptureModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        failedCaptureModule.testSetPreviewEncoder { _ in "preview" }
        failedCaptureModule.testSetScrollEndedResponder { failedCaptureEvents.append($0) }
        precondition(failedCaptureModule.testConsumeCaptureImage(periodicFooterFrames[0]))
        precondition(!failedCaptureModule.testConsumeCaptureImage(nil))
        precondition(!failedCaptureModule.testIsAutoScrolling())
        precondition((failedCaptureEvents.first?["reason"] as? String) == "capture-error")
        precondition((failedCaptureEvents.first?["frameCount"] as? Int) == 1)
        let failedCaptureOutput = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("poratake-scroll-capture-error-\(UUID().uuidString).png")
        defer { try? FileManager.default.removeItem(at: failedCaptureOutput) }
        let failedCaptureResult = failedCaptureModule.testFinishCurrentCapture(outputPath: failedCaptureOutput.path)
        precondition((failedCaptureResult?["success"] as? Bool) == true)
        precondition((failedCaptureResult?["frameCount"] as? Int) == 1)
        let failedCaptureSource = CGImageSourceCreateWithURL(failedCaptureOutput as CFURL, nil)!
        let failedCaptureSaved = CGImageSourceCreateImageAtIndex(failedCaptureSource, 0, nil)!
        precondition(pixels(failedCaptureSaved) == pixels(periodicFooterFrames[0]))
        precondition(failedCaptureEvents.count == 1)

        let duplicateCaptureModule = ScrollCaptureModule()
        var duplicateCaptureEvents: [[String: Any]] = []
        duplicateCaptureModule.testPrepareAutoScrollPreview(width: footerWidth, height: footerHeight, step: footerStep)
        duplicateCaptureModule.testSetPreviewEncoder { _ in "preview" }
        duplicateCaptureModule.testSetScrollEndedResponder { duplicateCaptureEvents.append($0) }
        precondition(duplicateCaptureModule.testConsumeCaptureImage(periodicFooterFrames[0]))
        precondition(duplicateCaptureModule.testConsumeCaptureImage(periodicFooterFrames[0]))
        precondition(duplicateCaptureModule.testConsumeCaptureImage(periodicFooterFrames[0]))
        precondition(!duplicateCaptureModule.testConsumeCaptureImage(periodicFooterFrames[0]))
        precondition(!duplicateCaptureModule.testIsAutoScrolling())
        precondition((duplicateCaptureEvents.first?["reason"] as? String) == "duplicate")
        precondition((duplicateCaptureEvents.first?["frameCount"] as? Int) == 1)

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
