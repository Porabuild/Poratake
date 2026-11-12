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

    static func main() {
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
        backlogModule.testWaitForPreviewWork()
        let backlogDeadline = Date(timeIntervalSinceNow: 5)
        while backlogPublications.isEmpty && Date() < backlogDeadline {
            _ = RunLoop.current.run(mode: .default, before: backlogDeadline)
        }
        backlogLock.lock()
        precondition(backlogEncodeCount == 2, "Only the active and latest preview should be encoded")
        backlogLock.unlock()
        precondition(backlogPublications == [3], "A preview backlog must publish only its latest frame")

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
