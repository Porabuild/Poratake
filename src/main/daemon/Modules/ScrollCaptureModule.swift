import Cocoa
import Carbon.HIToolbox
import Foundation
import ImageIO
import UniformTypeIdentifiers

private final class ScrollFinishJob {
    private let lock = NSLock()
    private var cancellationRequested = false
    private var completed = false

    func requestCancellation() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard !completed else { return false }
        cancellationRequested = true
        return true
    }

    func isCancellationRequested() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancellationRequested
    }

    func complete() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        completed = true
        return cancellationRequested
    }
}

private struct ScrollFrameAlignment {
    let overlap: Int
    let footerHeight: Int
}

private struct ScrollFooterDetection {
    let height: Int
    let isExact: Bool
}

private struct ScrollAlignmentSnapshot {
    let alignments: [ScrollFrameAlignment]
    let acceptedFrameCount: Int
    let stoppedForNoOverlap: Bool
}

private final class ScrollPreviewSession {
    struct Snapshot {
        let image: CGImage
        let encodedImage: String
        let frameCount: Int
        let estimatedHeight: Int
    }

    private struct PreviewRequest {
        let frames: [CGImage]
        let alignments: [ScrollFrameAlignment]
        let frameCount: Int
        let pointsPerPixel: CGFloat
        let compose: ([CGImage], [ScrollFrameAlignment], () -> Bool) -> CGImage?
        let encode: (CGImage) -> String?
        let completion: ((Snapshot) -> Void)?
    }

    private let alignmentQueue = DispatchQueue(label: "com.poratake.scroll-alignment", qos: .userInitiated)
    private let previewQueue = DispatchQueue(label: "com.poratake.scroll-preview", qos: .userInitiated)
    private let lock = NSLock()
    private var cancelled = false
    private var previewStopped = false
    private var alignmentStopped = false
    private var latestPublishedFrameCount = 0
    private var frames: [CGImage] = []
    private var alignments: [ScrollFrameAlignment] = []
    private var pendingPreview: PreviewRequest?
    private var previewWorkerScheduled = false
    private var latestSnapshot: Snapshot?
    private var pendingPublication: (Snapshot, (Snapshot) -> Void)?
    private var publicationScheduled = false

    func enqueueFrame(
        previousFrame: CGImage?,
        frame: CGImage,
        expectedOverlap: Int,
        frameCount: Int,
        pointsPerPixel: CGFloat,
        align: @escaping (CGImage, CGImage, Int, () -> Bool) -> ScrollFrameAlignment?,
        alignmentFailure: @escaping (Int) -> Void,
        compose: @escaping ([CGImage], [ScrollFrameAlignment], () -> Bool) -> CGImage?,
        encode: @escaping (CGImage) -> String?,
        completion: ((Snapshot) -> Void)?
    ) {
        alignmentQueue.async { [weak self] in
            guard let self = self, !self.isCancelled(), !self.isAlignmentStopped() else { return }

            let alignment: ScrollFrameAlignment?
            if let previousFrame = previousFrame {
                alignment = align(previousFrame, frame, expectedOverlap) { [weak self] in
                    self?.isCancelled() ?? true
                }
                guard !self.isCancelled() else { return }
                guard alignment != nil else {
                    self.lock.lock()
                    let shouldNotify = !self.previewStopped
                    self.alignmentStopped = true
                    self.previewStopped = true
                    self.pendingPreview = nil
                    self.lock.unlock()
                    if shouldNotify {
                        DispatchQueue.main.async {
                            alignmentFailure(frameCount - 1)
                        }
                    }
                    return
                }
            } else {
                alignment = nil
            }

            self.lock.lock()
            self.frames.append(frame)
            if let alignment = alignment {
                self.alignments.append(alignment)
            }
            let request = self.makePreviewRequest(
                frameCount: frameCount,
                pointsPerPixel: pointsPerPixel,
                compose: compose,
                encode: encode,
                completion: completion
            )
            self.lock.unlock()
            if let request = request {
                self.schedulePreview(request)
            }
        }
    }

    func alignmentSnapshot() -> ScrollAlignmentSnapshot {
        alignmentQueue.sync {
            lock.lock()
            defer { lock.unlock() }
            return ScrollAlignmentSnapshot(
                alignments: alignments,
                acceptedFrameCount: frames.count,
                stoppedForNoOverlap: alignmentStopped
            )
        }
    }

    func waitForWork() {
        alignmentQueue.sync {}
        previewQueue.sync {}
    }

    func previewSnapshot() -> Snapshot? {
        waitForWork()
        lock.lock()
        defer { lock.unlock() }
        return latestSnapshot
    }

    func stopPreviewRendering() {
        lock.lock()
        previewStopped = true
        pendingPreview = nil
        pendingPublication = nil
        latestSnapshot = nil
        lock.unlock()
    }

    func cancel() {
        lock.lock()
        cancelled = true
        previewStopped = true
        pendingPreview = nil
        pendingPublication = nil
        latestSnapshot = nil
        lock.unlock()
    }

    private func makePreviewRequest(
        frameCount: Int,
        pointsPerPixel: CGFloat,
        compose: @escaping ([CGImage], [ScrollFrameAlignment], () -> Bool) -> CGImage?,
        encode: @escaping (CGImage) -> String?,
        completion: ((Snapshot) -> Void)?
    ) -> PreviewRequest? {
        guard !cancelled,
              !previewStopped,
              frames.count == frameCount,
              alignments.count == frameCount - 1 else {
            return nil
        }

        return PreviewRequest(
            frames: frames,
            alignments: alignments,
            frameCount: frameCount,
            pointsPerPixel: pointsPerPixel,
            compose: compose,
            encode: encode,
            completion: completion
        )
    }

    private func schedulePreview(_ request: PreviewRequest) {
        lock.lock()
        guard !cancelled, !previewStopped else {
            lock.unlock()
            return
        }
        pendingPreview = request
        let shouldStartWorker = !previewWorkerScheduled
        previewWorkerScheduled = true
        lock.unlock()

        guard shouldStartWorker else { return }
        previewQueue.async { [weak self] in
            self?.drainPreviewQueue()
        }
    }

    private func drainPreviewQueue() {
        while true {
            lock.lock()
            guard !cancelled, !previewStopped else {
                pendingPreview = nil
                previewWorkerScheduled = false
                lock.unlock()
                return
            }
            guard let request = pendingPreview else {
                previewWorkerScheduled = false
                lock.unlock()
                return
            }
            pendingPreview = nil
            lock.unlock()

            let isStale = { [weak self] in self?.shouldStopPreview() ?? true }
            guard let image = request.compose(request.frames, request.alignments, isStale), !isStale(),
                  let encodedImage = request.encode(image), !isStale() else {
                continue
            }

            let snapshot = Snapshot(
                image: image,
                encodedImage: encodedImage,
                frameCount: request.frameCount,
                estimatedHeight: Int(CGFloat(image.height) * request.pointsPerPixel)
            )
            lock.lock()
            guard !cancelled,
                  !previewStopped,
                  request.frameCount > (latestSnapshot?.frameCount ?? 0) else {
                lock.unlock()
                continue
            }
            latestSnapshot = snapshot
            lock.unlock()

            if let completion = request.completion {
                schedulePublication(snapshot, completion: completion)
            }
        }
    }

    private func schedulePublication(_ snapshot: Snapshot, completion: @escaping (Snapshot) -> Void) {
        lock.lock()
        guard !cancelled,
              !previewStopped,
              snapshot.frameCount > latestPublishedFrameCount else {
            lock.unlock()
            return
        }
        if pendingPublication.map({ snapshot.frameCount > $0.0.frameCount }) ?? true {
            pendingPublication = (snapshot, completion)
        }
        let shouldSchedule = !publicationScheduled
        publicationScheduled = true
        lock.unlock()

        guard shouldSchedule else { return }
        DispatchQueue.main.async { [weak self] in
            self?.deliverLatestPublication()
        }
    }

    private func deliverLatestPublication() {
        lock.lock()
        guard !cancelled, !previewStopped, let publication = pendingPublication else {
            pendingPublication = nil
            publicationScheduled = false
            lock.unlock()
            return
        }
        pendingPublication = nil
        publicationScheduled = false
        let isNewer = publication.0.frameCount > latestPublishedFrameCount
        if isNewer {
            latestPublishedFrameCount = publication.0.frameCount
        }
        lock.unlock()

        guard isNewer, !shouldStopPreview() else { return }
        publication.1(publication.0)
    }

    private func isCancelled() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }

    private func isAlignmentStopped() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return alignmentStopped
    }

    private func shouldStopPreview() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled || previewStopped
    }
}

class ScrollCaptureModule: NSObject, Module {
    let name = DaemonContract.ScrollCapture.module

    private var captureArea: NSRect = .zero
    private var displayId: CGDirectDisplayID = CGMainDisplayID()
    private var targetScreen: NSScreen?
    private var capturedFrames: [CapturedFrame] = []
    private var isCapturing = false
    private var finishingJob: ScrollFinishJob?
    private var finishCancelRequestIds: [String] = []
    private var finishRequested = false
    private var cancelBeforeFinish = false
    private var cancelEventEmitted = false
    private var previewSession: ScrollPreviewSession?
    private var publishedPreviewFrameCount = 0
    private var noOverlapReported = false
#if SCROLL_CAPTURE_TEST
    private var testCancelResponder: ((String, Bool) -> Void)?
    private var testFinishWorker: (() -> (width: Int, height: Int, frameCount: Int, stoppedForNoOverlap: Bool))?
    private var testFinishResponder: ((String, [String: Any]) -> Void)?
    private var testPreviewAlignment: ((CGImage, CGImage, Int, () -> Bool) -> Int?)?
    private var testPreviewEncoder: ((CGImage) -> String?)?
    private var testPreviewResponder: ((CGImage, Int, Int) -> Void)?
    private var testScrollEndedResponder: (([String: Any]) -> Void)?
    private var testFinishAlignmentWaitStarted: DispatchSemaphore?
    private var testRetiredPreviewSessions: [ScrollPreviewSession] = []
#endif
    private var isAutoScrolling = false
    private var autoScrollTimer: Timer?
    private var cursorMonitorTimer: Timer?
    private var isCursorOutside = false
    private var autoScrollSpeed: ScrollSpeed = .medium
    private var maxHeight: Int = 20000
    private var lastFrameHash: Int? = nil
    private var duplicateFrameCount: Int = 0
    private var currentScrollStep: Int = 0
    private var scrollDeltaPerStep: Int32 = 0
    private var scrollStepPoints: Int = 0
    private var scrollStepRemainder: Int32 = 0
    private var scrollStepsPerFrame: Int = 0

    private let frameOverlapRatio: CGFloat = 0.3
    private let maxDuplicateFrames = 3
    private let previewWidthPoints: CGFloat = 240
    private let maxMeanOverlapChannelDifference = 18.0

    private var canStartCapture: Bool {
        !isCapturing && finishingJob == nil
    }

    private var boundaryWindows: [NSWindow] = []
    private var controlPanel: NSPanel?
    private var autoButton: NSButton?
    private var doneButton: NSButton?
    private var cancelButton: NSButton?
    private var escapeHotKey: EventHotKeyRef?
    private var enterHotKey: EventHotKeyRef?
    private var hotKeyHandler: EventHandlerRef?

    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.ScrollCapture.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .start:
            handleStart(params: params, requestId: requestId)
        case .startAutoScroll:
            handleStartAutoScroll(params: params, requestId: requestId)
        case .stopAutoScroll:
            handleStopAutoScroll(requestId: requestId)
        case .finish:
            handleFinish(params: params, requestId: requestId)
        case .cancel:
            handleCancel(requestId: requestId)
        }
    }

    private func handleStart(params: [String: AnyCodable]?, requestId: String) {
        guard let x = params?["x"]?.int(),
              let y = params?["y"]?.int(),
              let width = params?["width"]?.int(),
              let height = params?["height"]?.int() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "start requires x, y, width, height")
            return
        }

        guard canStartCapture else {
            respondError(id: requestId, code: "ALREADY_CAPTURING", message: "A scroll capture is already active")
            return
        }

        finishCancelRequestIds.removeAll()
        finishRequested = false
        cancelBeforeFinish = false
        cancelEventEmitted = false
#if SCROLL_CAPTURE_TEST
        if let previewSession = previewSession {
            testRetiredPreviewSessions.append(previewSession)
        }
#endif
        previewSession?.cancel()
        previewSession = ScrollPreviewSession()
        publishedPreviewFrameCount = 0

        displayId = CGMainDisplayID()
        targetScreen = nil
        if let display = params?["displayId"]?.int() {
            displayId = CGDirectDisplayID(display)
        } else if let originX = params?["displayOriginX"]?.int(),
                  let originY = params?["displayOriginY"]?.int(),
                  let screen = screenForTopLeftOrigin(x: originX, y: originY) {
            targetScreen = screen
            if let screenNumber = screen.deviceDescription[
                NSDeviceDescriptionKey("NSScreenNumber")
            ] as? CGDirectDisplayID {
                displayId = screenNumber
            }
        }

        targetScreen = targetScreen ?? screenForDisplayId(displayId)

        if let speed = params?["autoScrollSpeed"]?.string() {
            autoScrollSpeed = ScrollSpeed(rawValue: speed) ?? .medium
        }

        if let max = params?["maxHeight"]?.int() {
            maxHeight = max
        }
        let nativeControls = params?["nativeControls"]?.bool() ?? false
        let boundaryOnly = params?["boundaryOnly"]?.bool() ?? false

        let mainScreenHeight = primaryScreenHeight()
        let cocoaY = cocoaYFromTopLeft(
            topLeftY: CGFloat(y),
            height: CGFloat(height),
            primaryHeight: mainScreenHeight
        )

        captureArea = NSRect(x: CGFloat(x), y: cocoaY, width: CGFloat(width), height: CGFloat(height))
        capturedFrames.removeAll()
        isCapturing = true
        lastFrameHash = nil
        duplicateFrameCount = 0

        let topLeftY = mainScreenHeight - cocoaY - CGFloat(height)

        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            if nativeControls && !self.showCaptureUI() {
                self.cleanup()
                self.respondError(
                    id: requestId,
                    code: "UI_ERROR",
                    message: "Failed to register scroll capture shortcuts"
                )
                return
            }
            if !nativeControls && boundaryOnly {
                self.hideCaptureUI()
                self.showBoundaryFrame()
            }
            self.respond(id: requestId, result: ["started": true])
            self.emit(event: "started", data: [
                "x": x,
                "y": Int(topLeftY),
                "width": width,
                "height": height,
                "displayId": Int(self.displayId)
            ])
        }
    }

    private func handleStartAutoScroll(params: [String: AnyCodable]?, requestId: String) {
        guard isCapturing else {
            respondError(id: requestId, code: "NOT_CAPTURING", message: "Not in capture mode")
            return
        }

        if let speed = params?["speed"]?.string() {
            autoScrollSpeed = ScrollSpeed(rawValue: speed) ?? autoScrollSpeed
        }

        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.startAutoScroll()
            self.respond(id: requestId, result: ["autoScrolling": true])
        }
    }

    private func handleStopAutoScroll(requestId: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.stopAutoScroll()
            self.respond(id: requestId, result: ["autoScrolling": false])
        }
    }

    private func handleFinish(params: [String: AnyCodable]?, requestId: String) {
        guard let outputPath = params?["outputPath"]?.string() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "finish requires outputPath")
            return
        }

        guard finishingJob == nil else {
            respondError(id: requestId, code: "FINISHING", message: "Scroll capture is already finishing")
            return
        }

        guard isCapturing else {
            respondError(id: requestId, code: "NOT_CAPTURING", message: "Not in capture mode")
            return
        }

        stopAutoScroll()
        finishRequested = true
        previewSession?.stopPreviewRendering()
        updateFinishingControls(cancelled: cancelBeforeFinish)
        let frameCount = capturedFrames.count
        let frames = capturedFrames
        let captureAreaHeight = captureArea.height
        let scrollStepPoints = self.scrollStepPoints
        let previewSession = self.previewSession
        let job = ScrollFinishJob()
        if cancelBeforeFinish {
            _ = job.requestCancellation()
        }
        finishingJob = job
        isCapturing = false
#if SCROLL_CAPTURE_TEST
        let testFinishWorker = self.testFinishWorker
#endif

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }

#if SCROLL_CAPTURE_TEST
            let alignmentSnapshot = self.finishAlignedOverlaps(session: previewSession, job: job)
            let result = testFinishWorker?() ?? self.stitchFrames(
                frames: frames,
                captureAreaHeight: captureAreaHeight,
                scrollStepPoints: scrollStepPoints,
                alignmentSnapshot: alignmentSnapshot,
                outputPath: outputPath,
                isCancelled: { job.isCancellationRequested() }
            )
#else
            let alignmentSnapshot = self.finishAlignedOverlaps(session: previewSession, job: job)
            let result = self.stitchFrames(
                frames: frames,
                captureAreaHeight: captureAreaHeight,
                scrollStepPoints: scrollStepPoints,
                alignmentSnapshot: alignmentSnapshot,
                outputPath: outputPath,
                isCancelled: { job.isCancellationRequested() }
            )
#endif
            let cancelled = job.complete()

            DispatchQueue.main.async {
                self.completeFinish(
                    job: job,
                    cancelled: cancelled,
                    result: result,
                    requestId: requestId,
                    outputPath: outputPath,
                    frameCount: frameCount
                )
            }
        }
    }

    private func finishAlignedOverlaps(
        session: ScrollPreviewSession?,
        job: ScrollFinishJob
    ) -> ScrollAlignmentSnapshot? {
        guard !job.isCancellationRequested() else { return nil }
#if SCROLL_CAPTURE_TEST
        testFinishAlignmentWaitStarted?.signal()
#endif
        return session?.alignmentSnapshot()
    }

    private func handleCancel(requestId: String) {
        if let job = finishingJob {
            guard job.requestCancellation() else {
                respondCancel(id: requestId, cancelled: job.isCancellationRequested())
                return
            }
            previewSession?.cancel()
            finishCancelRequestIds.append(requestId)
            emitCancelledOnce()
            updateFinishingControls(cancelled: true)
            return
        }
        if finishRequested {
            cancelBeforeFinish = true
            previewSession?.cancel()
            finishCancelRequestIds.append(requestId)
            emitCancelledOnce()
            updateFinishingControls(cancelled: true)
            return
        }
        guard isCapturing else {
            respondCancel(id: requestId, cancelled: false)
            return
        }
        stopAutoScroll()
        emitCancelledOnce()
        cleanup()
        respondCancel(id: requestId, cancelled: true)
    }

    private func completeFinish(
        job: ScrollFinishJob,
        cancelled: Bool,
        result: (width: Int, height: Int, frameCount: Int, stoppedForNoOverlap: Bool)?,
        requestId: String,
        outputPath: String,
        frameCount: Int
    ) {
        guard finishingJob === job else { return }
        let cancelRequestIds = finishCancelRequestIds
        let didReportNoOverlap = noOverlapReported
        if cancelled {
            emitCancelledOnce()
        }
        finishingJob = nil
        finishCancelRequestIds.removeAll()
        cleanup()

        if cancelled {
            respondFinish(id: requestId, result: cancelledFinishResult(outputPath: outputPath, frameCount: frameCount))
            cancelRequestIds.forEach { respondCancel(id: $0, cancelled: true) }
            return
        }

        if let result = result {
            if result.stoppedForNoOverlap && !didReportNoOverlap {
                emitNoOverlap(frameCount: result.frameCount)
            }
            respondFinish(id: requestId, result: [
                "success": true,
                "outputPath": outputPath,
                "width": result.width,
                "height": result.height,
                "frameCount": result.frameCount
            ])
        } else {
            respondError(id: requestId, code: "STITCH_ERROR", message: "Failed to stitch frames")
        }
        cancelRequestIds.forEach { respondCancel(id: $0, cancelled: false) }
    }

    private func respondCancel(id: String, cancelled: Bool) {
        #if SCROLL_CAPTURE_TEST
        if let testCancelResponder {
            testCancelResponder(id, cancelled)
            return
        }
        #endif
        respond(id: id, result: ["cancelled": cancelled])
    }

    private func respondFinish(id: String, result: [String: Any]) {
        #if SCROLL_CAPTURE_TEST
        if let testFinishResponder {
            testFinishResponder(id, result)
            return
        }
        #endif
        respond(id: id, result: result)
    }

    private func cancelledFinishResult(outputPath: String, frameCount: Int) -> [String: Any] {
        [
            "success": false,
            "outputPath": outputPath,
            "width": 0,
            "height": 0,
            "frameCount": frameCount
        ]
    }

    private func startAutoScroll() {
        guard !isAutoScrolling else { return }

        isAutoScrolling = true
        autoButton?.title = "Stop"
        isCursorOutside = false
        currentScrollStep = 0

        let frameHeight = Int(captureArea.height)
        scrollStepPoints = max(1, Int(Double(frameHeight) * (1.0 - frameOverlapRatio)))
        scrollStepsPerFrame = max(1, min(autoScrollSpeed.scrollStepsPerFrame, scrollStepPoints))

        let stepAmount = max(1, scrollStepPoints / scrollStepsPerFrame)
        scrollDeltaPerStep = -Int32(stepAmount)
        scrollStepRemainder = -Int32(scrollStepPoints - (stepAmount * scrollStepsPerFrame))

        warpCursorToCaptureCenter()
        captureCurrentFrame()
        startCursorMonitor()
        emitAutoScrollChanged(scrolling: true)

        let interval = autoScrollSpeed.stepInterval
        autoScrollTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: true) { [weak self] _ in
            self?.performAutoScrollStep()
        }
    }

    private func stopAutoScroll() {
        let wasScrolling = isAutoScrolling
        isAutoScrolling = false
        isCursorOutside = false
        autoScrollTimer?.invalidate()
        autoScrollTimer = nil
        autoButton?.title = "Auto"
        stopCursorMonitor()
        if wasScrolling {
            emitAutoScrollChanged(scrolling: false)
        }
    }

    private func performAutoScrollStep() {
        guard !isCursorOutside else { return }

        let stepsPerFrame = scrollStepsPerFrame
        let isFinalStep = currentScrollStep == stepsPerFrame - 1
        let delta = isFinalStep ? scrollDeltaPerStep + scrollStepRemainder : scrollDeltaPerStep
        simulateScroll(delta: delta)
        currentScrollStep += 1

        guard currentScrollStep >= stepsPerFrame else { return }

        currentScrollStep = 0
        autoScrollTimer?.invalidate()
        autoScrollTimer = nil

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { [weak self] in
            guard let self = self, self.isAutoScrolling else { return }

            let frameIndex = self.captureCurrentFrame()

            if frameIndex == nil {
                self.stopAutoScroll()
                self.emit(event: "scroll-ended", data: [
                    "reason": "duplicate",
                    "frameCount": self.capturedFrames.count
                ])
                return
            }

            let estimatedHeight = self.calculateEstimatedHeight()
            if estimatedHeight >= self.maxHeight {
                self.stopAutoScroll()
                self.emit(event: "scroll-ended", data: [
                    "reason": "max-height",
                    "frameCount": self.capturedFrames.count,
                    "estimatedHeight": estimatedHeight
                ])
                return
            }

            self.autoScrollTimer = Timer.scheduledTimer(withTimeInterval: self.autoScrollSpeed.stepInterval, repeats: true) { [weak self] _ in
                self?.performAutoScrollStep()
            }
        }
    }

    private func warpCursorToCaptureCenter() {
        let centerX = captureArea.midX
        let centerY = primaryScreenHeight() - captureArea.midY
        CGWarpMouseCursorPosition(CGPoint(x: centerX, y: centerY))
    }

    private func isCursorInsideCaptureArea() -> Bool {
        let mouseLocation = NSEvent.mouseLocation
        return captureArea.contains(mouseLocation)
    }

    private func startCursorMonitor() {
        stopCursorMonitor()
        cursorMonitorTimer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            self?.checkCursorPosition()
        }
    }

    private func stopCursorMonitor() {
        cursorMonitorTimer?.invalidate()
        cursorMonitorTimer = nil
    }

    private func checkCursorPosition() {
        guard isAutoScrolling else { return }

        let isInside = isCursorInsideCaptureArea()
        guard isInside != !isCursorOutside else { return }

        isCursorOutside = !isInside
        let frameColor = isCursorOutside
            ? NSColor(red: 249 / 255, green: 115 / 255, blue: 22 / 255, alpha: 1)
            : NSColor(red: 59 / 255, green: 130 / 255, blue: 246 / 255, alpha: 1)
        boundaryWindows.forEach { $0.backgroundColor = frameColor }
        emit(event: "cursor", data: ["outside": isCursorOutside])
    }

    private func simulateScroll(delta: Int32) {
        let mouseLocation = NSEvent.mouseLocation
        let cgMouseY = primaryScreenHeight() - mouseLocation.y
        let cgPoint = CGPoint(x: mouseLocation.x, y: cgMouseY)

        if let scrollEvent = CGEvent(scrollWheelEvent2Source: nil,
                                      units: .pixel,
                                      wheelCount: 1,
                                      wheel1: delta,
                                      wheel2: 0,
                                      wheel3: 0) {
            scrollEvent.location = cgPoint
            scrollEvent.post(tap: .cghidEventTap)
        }
    }

    @discardableResult
    private func captureCurrentFrame() -> Int? {
        let captureRect = CGRect(
            x: captureArea.origin.x,
            y: primaryScreenHeight() - captureArea.origin.y - captureArea.height,
            width: captureArea.width,
            height: captureArea.height
        )

        CGDisplayHideCursor(displayId)

        let cgImage = CGWindowListCreateImage(
            captureRect,
            .optionOnScreenOnly,
            kCGNullWindowID,
            [.bestResolution, .boundsIgnoreFraming]
        )

        CGDisplayShowCursor(displayId)

        guard let cgImage = cgImage else {
            return nil
        }

        return appendCapturedFrame(cgImage)
    }

    private func appendCapturedFrame(_ cgImage: CGImage) -> Int? {
        let frameHash = computeFrameHash(cgImage)

        if let lastFrameHash = lastFrameHash, frameHash == lastFrameHash {
            duplicateFrameCount += 1
            if duplicateFrameCount >= maxDuplicateFrames {
                return nil
            }
            return capturedFrames.count - 1
        }

        duplicateFrameCount = 0
        lastFrameHash = frameHash

        let frame = CapturedFrame(
            image: cgImage,
            index: capturedFrames.count
        )
        capturedFrames.append(frame)

        updatePreviewImage()

        return frame.index
    }

    private func computeFrameHash(_ image: CGImage) -> Int {
        let sampleWidth = 100
        let sampleHeight = 50

        guard let context = CGContext(
            data: nil,
            width: sampleWidth,
            height: sampleHeight,
            bitsPerComponent: 8,
            bytesPerRow: sampleWidth * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return 0
        }

        let bottomRect = CGRect(
            x: 0,
            y: 0,
            width: image.width,
            height: min(image.height / 4, 100)
        )

        guard let croppedImage = image.cropping(to: bottomRect) else {
            return 0
        }

        context.draw(croppedImage, in: CGRect(x: 0, y: 0, width: sampleWidth, height: sampleHeight))

        guard let data = context.data else { return 0 }
        let buffer = data.bindMemory(to: UInt8.self, capacity: sampleWidth * sampleHeight * 4)

        var hash = 0
        let strideAmount = 16
        for i in Swift.stride(from: 0, to: sampleWidth * sampleHeight * 4, by: strideAmount) {
            hash = hash &* 31 &+ Int(buffer[i])
        }

        return hash
    }

    private func calculateEstimatedHeight() -> Int {
        guard capturedFrames.count > 0 else { return 0 }

        let frameHeight = Int(captureArea.height)
        return frameHeight + (capturedFrames.count - 1) * scrollStepPoints
    }

    private func stitchFrames(
        frames: [CapturedFrame],
        captureAreaHeight: CGFloat,
        scrollStepPoints: Int,
        alignmentSnapshot: ScrollAlignmentSnapshot?,
        outputPath: String,
        isCancelled: () -> Bool
    ) -> (width: Int, height: Int, frameCount: Int, stoppedForNoOverlap: Bool)? {
        guard !frames.isEmpty, !isCancelled() else { return nil }

        if frames.count == 1 {
            guard let result = saveSingleFrame(
                frame: frames[0].image,
                outputPath: outputPath,
                isCancelled: isCancelled
            ) else { return nil }
            return (result.width, result.height, 1, false)
        }

        let frameHeight = frames[0].image.height

        let scale = CGFloat(frameHeight) / captureAreaHeight
        let scrollStepPixels = Int(CGFloat(scrollStepPoints) * scale)
        let clampedScrollStep = min(max(scrollStepPixels, 1), frameHeight)
        let expectedOverlapPixels = frameHeight - clampedScrollStep
        let frameLimit = alignmentSnapshot?.stoppedForNoOverlap == true
            ? max(1, min(alignmentSnapshot?.acceptedFrameCount ?? 1, frames.count))
            : frames.count

        var acceptedFrames = [frames[0]]
        var alignments = [ScrollFrameAlignment]()
        for index in 1..<frameLimit {
            guard !isCancelled() else { return nil }
            let alignment: ScrollFrameAlignment?
            if let alignmentSnapshot,
               index - 1 < alignmentSnapshot.alignments.count {
                let stored = alignmentSnapshot.alignments[index - 1]
                alignment = stored
            } else {
                alignment = findOverlap(
                    topFrame: frames[index - 1].image,
                    bottomFrame: frames[index].image,
                    expectedOverlap: expectedOverlapPixels,
                    isCancelled: isCancelled
                )
            }
            guard let alignment else {
                break
            }
            acceptedFrames.append(frames[index])
            alignments.append(alignment)
        }
        guard !isCancelled() else { return nil }
        let stoppedForNoOverlap = acceptedFrames.count < frames.count
            || alignmentSnapshot?.stoppedForNoOverlap == true

        if acceptedFrames.count == 1 {
            guard let result = saveSingleFrame(
                frame: acceptedFrames[0].image,
                outputPath: outputPath,
                isCancelled: isCancelled
            ) else { return nil }
            return (result.width, result.height, acceptedFrames.count, stoppedForNoOverlap)
        }

        guard let outputImage = composeFrames(
            frames: acceptedFrames.map(\.image),
            alignments: alignments,
            isCancelled: isCancelled
        ) else { return nil }

        let url = URL(fileURLWithPath: outputPath)
        guard let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else {
            return nil
        }

        CGImageDestinationAddImage(destination, outputImage, nil)

        guard !isCancelled() else { return nil }
        if CGImageDestinationFinalize(destination) {
            guard !isCancelled() else { return nil }
            return (
                width: outputImage.width,
                height: outputImage.height,
                frameCount: acceptedFrames.count,
                stoppedForNoOverlap: stoppedForNoOverlap
            )
        }

        return nil
    }

    private func composeFrames(
        frames: [CGImage],
        alignments: [ScrollFrameAlignment],
        isCancelled: () -> Bool
    ) -> CGImage? {
        guard let firstFrame = frames.first, !isCancelled() else { return nil }
        guard frames.count > 1 else { return firstFrame }

        guard alignments.count == frames.count - 1 else { return nil }
        let trimBottoms = alignments.map(\.footerHeight) + [0]

        let firstContentHeight = firstFrame.height - trimBottoms[0]
        var totalHeight = firstContentHeight
        for index in 1..<frames.count {
            guard !isCancelled() else { return nil }
            let newContent = frames[index].height - alignments[index - 1].overlap - trimBottoms[index]
            guard newContent > 0 else { return nil }
            totalHeight += newContent
        }
        guard !isCancelled() else { return nil }

        guard let context = CGContext(
            data: nil,
            width: firstFrame.width,
            height: totalHeight,
            bitsPerComponent: 8,
            bytesPerRow: firstFrame.width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return nil }

        var currentY = totalHeight - firstContentHeight
        guard let firstContent = firstFrame.cropping(
            to: CGRect(x: 0, y: 0, width: firstFrame.width, height: firstContentHeight)
        ) else { return nil }
        context.draw(firstContent, in: CGRect(x: 0, y: currentY, width: firstFrame.width, height: firstContentHeight))

        for index in 1..<frames.count {
            guard !isCancelled() else { return nil }
            let frame = frames[index]
            let overlap = alignments[index - 1].overlap
            let newContentHeight = frame.height - overlap - trimBottoms[index]

            guard newContentHeight > 0 else { return nil }

            let cropRect = CGRect(x: 0, y: overlap, width: frame.width, height: newContentHeight)
            guard let croppedImage = frame.cropping(to: cropRect) else { continue }

            currentY -= newContentHeight
            context.draw(croppedImage, in: CGRect(x: 0, y: currentY, width: firstFrame.width, height: newContentHeight))
        }

        guard !isCancelled() else { return nil }
        return context.makeImage()
    }

    private func saveSingleFrame(
        frame: CGImage,
        outputPath: String,
        isCancelled: () -> Bool
    ) -> (width: Int, height: Int)? {
        guard !isCancelled() else { return nil }

        let url = URL(fileURLWithPath: outputPath)
        guard let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else {
            return nil
        }

        CGImageDestinationAddImage(destination, frame, nil)

        guard !isCancelled() else { return nil }
        if CGImageDestinationFinalize(destination) {
            guard !isCancelled() else { return nil }
            return (width: frame.width, height: frame.height)
        }

        return nil
    }

    private func findOverlap(
        topFrame: CGImage,
        bottomFrame: CGImage,
        expectedOverlap: Int,
        isCancelled: () -> Bool = { false }
    ) -> ScrollFrameAlignment? {
        guard !isCancelled() else { return nil }
        guard topFrame.width == bottomFrame.width,
              topFrame.height == bottomFrame.height else {
            return nil
        }
        let stripWidth = min(topFrame.width, 800)
        let stripHeight = 40
        guard let footer = fixedFooterHeight(
            topFrame: topFrame,
            bottomFrame: bottomFrame,
            isCancelled: isCancelled
        ) else {
            return nil
        }
        guard !isCancelled() else { return nil }
        let footerHeight = footer.height
        let contentHeight = topFrame.height - footerHeight

        guard contentHeight >= stripHeight else { return nil }
        let topStrip: [UInt8]?
        if footerHeight == 0 {
            guard !isCancelled() else { return nil }
            topStrip = extractBottomStrip(from: topFrame, height: stripHeight, width: stripWidth)
        } else {
            guard !isCancelled() else { return nil }
            topStrip = extractStrip(
                from: topFrame,
                y: contentHeight - stripHeight,
                height: stripHeight,
                width: stripWidth
            )
        }
        guard !isCancelled() else { return nil }
        guard let topStrip else { return nil }

        let minOverlap = stripHeight
        let maxOverlap = max(minOverlap, contentHeight)
        let adjustedExpected = footerHeight > 0
            ? max(minOverlap, expectedOverlap - footerHeight)
            : expectedOverlap

        var bestMatch = adjustedExpected
        var bestScore = Double.infinity
        var scores = [(overlap: Int, score: Double)]()

        let step = 4

        let checkMatch = { (overlap: Int, score: Double) in
            scores.append((overlap, score))
            if score < bestScore - 1.0 {
                bestScore = score
                bestMatch = overlap
            } else if abs(score - bestScore) <= 1.0 {
                if abs(overlap - adjustedExpected) < abs(bestMatch - adjustedExpected) {
                    bestMatch = overlap
                }
            }
        }

        for overlap in stride(from: minOverlap, through: maxOverlap, by: step) {
            guard !isCancelled() else { return nil }
            let bottomStrip = extractTopStrip(from: bottomFrame, overlap: overlap, height: stripHeight, width: stripWidth)
            guard !isCancelled() else { return nil }
            guard let bottomStrip else {
                continue
            }

            let score = compareStrips(topStrip, bottomStrip)
            checkMatch(overlap, score)
        }

        let fineMin = max(minOverlap, bestMatch - step)
        let fineMax = min(maxOverlap, bestMatch + step)

        for overlap in fineMin...fineMax {
            guard !isCancelled() else { return nil }
            let bottomStrip = extractTopStrip(from: bottomFrame, overlap: overlap, height: stripHeight, width: stripWidth)
            guard !isCancelled() else { return nil }
            guard let bottomStrip else {
                continue
            }

            let score = compareStrips(topStrip, bottomStrip)
            checkMatch(overlap, score)
        }

        let nearMatches = scores.filter { $0.score <= bestScore + 1.0 }
        let nearOverlapMinimum = nearMatches.map(\.overlap).min()
        let nearOverlapMaximum = nearMatches.map(\.overlap).max()
        let hasAmbiguousMatches: Bool
        if let minimum = nearOverlapMinimum, let maximum = nearOverlapMaximum {
            hasAmbiguousMatches = maximum - minimum >= 8
        } else {
            hasAmbiguousMatches = false
        }
        let requiresExactOverlap = hasAmbiguousMatches
            || bestScore / 3 > maxMeanOverlapChannelDifference
            || bestMatch == minOverlap
        if !footer.isExact || requiresExactOverlap {
            let exactMinimumOverlap = min(8, minOverlap)
            guard let (overlap, alternative) = exactFullImageOverlap(
                topFrame: topFrame,
                bottomFrame: bottomFrame,
                contentHeight: contentHeight,
                minimumOverlap: exactMinimumOverlap,
                isCancelled: isCancelled
            ), alternative < exactMinimumOverlap else {
                return nil
            }
            return ScrollFrameAlignment(overlap: overlap, footerHeight: footerHeight)
        }

        guard bestMatch < contentHeight else { return nil }
        return ScrollFrameAlignment(overlap: bestMatch, footerHeight: footerHeight)
    }

    private func exactFullImageOverlap(
        topFrame: CGImage,
        bottomFrame: CGImage,
        contentHeight: Int,
        minimumOverlap: Int,
        isCancelled: () -> Bool
    ) -> (overlap: Int, alternative: Int)? {
        guard topFrame.width == bottomFrame.width,
              topFrame.height == bottomFrame.height else {
            return nil
        }
        guard !isCancelled() else { return nil }
        let topPixels = extractPixelData(from: topFrame)
        guard !isCancelled() else { return nil }
        guard let topPixels else { return nil }
        let bottomPixels = extractPixelData(from: bottomFrame)
        guard !isCancelled() else { return nil }
        guard let bottomPixels else {
            return nil
        }

        let rowBytes = topFrame.width * 4
        let rowEquals = { (left: [UInt8], leftRow: Int, right: [UInt8], rightRow: Int) in
            let leftStart = leftRow * rowBytes
            let rightStart = rightRow * rowBytes
            return left[leftStart..<(leftStart + rowBytes)] == right[rightStart..<(rightStart + rowBytes)]
        }
        var prefix = [Int](repeating: 0, count: contentHeight)
        for index in 1..<contentHeight {
            guard !isCancelled() else { return nil }
            var matched = prefix[index - 1]
            while matched > 0 {
                guard !isCancelled() else { return nil }
                if rowEquals(bottomPixels, index, bottomPixels, matched) { break }
                matched = prefix[matched - 1]
            }
            if rowEquals(bottomPixels, index, bottomPixels, matched) {
                matched += 1
            }
            prefix[index] = matched
        }

        var matched = 0
        for row in 0..<contentHeight {
            guard !isCancelled() else { return nil }
            while matched > 0 {
                guard !isCancelled() else { return nil }
                if matched < contentHeight && rowEquals(topPixels, row, bottomPixels, matched) { break }
                matched = prefix[matched - 1]
            }
            if rowEquals(topPixels, row, bottomPixels, matched) {
                matched += 1
            }
        }

        guard matched >= minimumOverlap, matched < contentHeight else { return nil }
        return (matched, prefix[matched - 1])
    }

    private func fixedFooterHeight(
        topFrame: CGImage,
        bottomFrame: CGImage,
        isCancelled: () -> Bool,
        extractPixels: ((CGImage) -> [UInt8]?)? = nil
    ) -> ScrollFooterDetection? {
        guard !isCancelled() else { return nil }
        guard topFrame.width == bottomFrame.width,
              topFrame.height == bottomFrame.height else {
            return ScrollFooterDetection(height: 0, isExact: true)
        }
        let extract = extractPixels ?? { image in self.extractPixelData(from: image) }
        let topPixels = extract(topFrame)
        guard !isCancelled() else { return nil }
        guard let topPixels else { return nil }
        let bottomPixels = extract(bottomFrame)
        guard !isCancelled() else { return nil }
        guard let bottomPixels else { return nil }

        let rowBytes = topFrame.width * 4
        var exactFooterHeight = 0
        var similarFooterHeight = 0
        for row in (0..<topFrame.height).reversed() {
            guard !isCancelled() else { return nil }
            let range = row * rowBytes..<(row + 1) * rowBytes
            let previousRow = topPixels[range]
            let currentRow = bottomPixels[range]
            if exactFooterHeight == similarFooterHeight && previousRow == currentRow {
                exactFooterHeight += 1
                similarFooterHeight += 1
                continue
            }

            var difference = 0
            for offset in stride(from: 0, to: rowBytes, by: 4) {
                let index = range.lowerBound + offset
                difference += abs(Int(topPixels[index]) - Int(bottomPixels[index]))
                difference += abs(Int(topPixels[index + 1]) - Int(bottomPixels[index + 1]))
                difference += abs(Int(topPixels[index + 2]) - Int(bottomPixels[index + 2]))
            }
            guard Double(difference) / Double(rowBytes / 4 * 3) <= maxMeanOverlapChannelDifference else {
                break
            }
            similarFooterHeight += 1
        }

        guard !isCancelled() else { return nil }
        if similarFooterHeight >= 8, similarFooterHeight < topFrame.height / 2 {
            return ScrollFooterDetection(
                height: similarFooterHeight,
                isExact: similarFooterHeight == exactFooterHeight
            )
        }
        return ScrollFooterDetection(height: 0, isExact: true)
    }

    private func extractBottomStrip(from image: CGImage, height: Int, width: Int) -> [UInt8]? {
        extractStrip(from: image, y: image.height - height, height: height, width: width)
    }

    private func extractStrip(from image: CGImage, y: Int, height: Int, width: Int) -> [UInt8]? {
        let startX = (image.width - width) / 2

        let rect = CGRect(x: startX, y: y, width: width, height: height)
        guard let cropped = image.cropping(to: rect) else { return nil }

        return extractPixelData(from: cropped)
    }

    private func extractTopStrip(from image: CGImage, overlap: Int, height: Int, width: Int) -> [UInt8]? {
        let startX = (image.width - width) / 2
        let startY = overlap - height

        guard startY >= 0 else { return nil }

        let rect = CGRect(x: startX, y: startY, width: width, height: height)
        guard let cropped = image.cropping(to: rect) else { return nil }

        return extractPixelData(from: cropped)
    }

    private func extractPixelData(from image: CGImage) -> [UInt8]? {
        let width = image.width
        let height = image.height
        let bytesPerRow = width * 4

        var pixelData = [UInt8](repeating: 0, count: width * height * 4)

        guard let context = CGContext(
            data: &pixelData,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return nil
        }

        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))

        return pixelData
    }

    private func compareStrips(_ strip1: [UInt8], _ strip2: [UInt8]) -> Double {
        guard strip1.count == strip2.count else { return Double.infinity }

        var totalDiff: Int = 0
        let strideAmount = 4

        for i in Swift.stride(from: 0, to: strip1.count, by: strideAmount) {
            let r1 = Int(strip1[i])
            let g1 = Int(strip1[i + 1])
            let b1 = Int(strip1[i + 2])

            let r2 = Int(strip2[i])
            let g2 = Int(strip2[i + 1])
            let b2 = Int(strip2[i + 2])

            totalDiff += abs(r1 - r2) + abs(g1 - g2) + abs(b1 - b2)
        }

        return Double(totalDiff) / Double(strip1.count / strideAmount)
    }

    private func screenForDisplayId(_ displayId: CGDirectDisplayID) -> NSScreen? {
        return NSScreen.screens.first { screen in
            guard let screenNumber = screen.deviceDescription[
                NSDeviceDescriptionKey("NSScreenNumber")
            ] as? CGDirectDisplayID else {
                return false
            }
            return screenNumber == displayId
        }
    }

    private func showCaptureUI() -> Bool {
        hideCaptureUI()
        guard registerCaptureHotKeys() else { return false }

        showBoundaryFrame()

        let panelSize = NSSize(width: 280, height: 52)
        let screen = targetScreen ?? NSScreen.main ?? NSScreen.screens.first
        let screenFrame = screen?.frame ?? .zero
        let below = captureArea.minY - 12 - panelSize.height
        let panelY = below >= screenFrame.minY ? below : captureArea.maxY + 12
        let panelRect = NSRect(
            x: captureArea.midX - panelSize.width / 2,
            y: panelY,
            width: panelSize.width,
            height: panelSize.height
        )
        let panel = NSPanel(
            contentRect: panelRect,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.level = .screenSaver
        panel.isOpaque = false
        panel.backgroundColor = NSColor.windowBackgroundColor.withAlphaComponent(0.96)
        panel.hasShadow = true
        panel.sharingType = .none
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.becomesKeyOnlyIfNeeded = true

        let auto = controlButton(title: "Auto", action: #selector(toggleAutoScrollFromUI(_:)))
        let done = controlButton(title: "Done", action: #selector(doneFromUI(_:)))
        let cancel = controlButton(title: "Cancel", action: #selector(cancelFromUI(_:)))
        let stack = NSStackView(views: [auto, done, cancel])
        stack.orientation = .horizontal
        stack.alignment = .centerY
        stack.distribution = .fillEqually
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        panel.contentView = stack
        panel.orderFrontRegardless()
        controlPanel = panel
        autoButton = auto
        doneButton = done
        cancelButton = cancel
        return true
    }

    private func showBoundaryFrame() {
        let thickness: CGFloat = 2
        let edgeRects = [
            NSRect(
                x: captureArea.minX - thickness,
                y: captureArea.maxY,
                width: captureArea.width + thickness * 2,
                height: thickness
            ),
            NSRect(
                x: captureArea.minX - thickness,
                y: captureArea.minY - thickness,
                width: captureArea.width + thickness * 2,
                height: thickness
            ),
            NSRect(
                x: captureArea.minX - thickness,
                y: captureArea.minY,
                width: thickness,
                height: captureArea.height
            ),
            NSRect(
                x: captureArea.maxX,
                y: captureArea.minY,
                width: thickness,
                height: captureArea.height
            )
        ]
        boundaryWindows = edgeRects.map { rect in
            let window = NSWindow(
                contentRect: rect,
                styleMask: .borderless,
                backing: .buffered,
                defer: false
            )
            window.level = .screenSaver
            window.isOpaque = true
            window.backgroundColor = .systemBlue
            window.ignoresMouseEvents = true
            window.hasShadow = false
            window.sharingType = .none
            window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
            window.orderFrontRegardless()
            return window
        }
    }

    private func controlButton(title: String, action: Selector) -> NSButton {        let button = NSButton(title: title, target: self, action: action)
        button.bezelStyle = .rounded
        button.setButtonType(.momentaryPushIn)
        return button
    }

    private func hideCaptureUI() {
        unregisterCaptureHotKeys()
        boundaryWindows.forEach { $0.orderOut(nil) }
        boundaryWindows.removeAll()
        controlPanel?.orderOut(nil)
        controlPanel = nil
        autoButton = nil
        doneButton = nil
        cancelButton = nil
    }

    private func updateFinishingControls(cancelled: Bool) {
        autoButton?.isEnabled = false
        doneButton?.title = cancelled ? "Cancelling…" : "Saving…"
        doneButton?.isEnabled = false
        cancelButton?.isEnabled = !cancelled
    }

    private func emitCancelledOnce() {
        guard !cancelEventEmitted else { return }
        cancelEventEmitted = true
        emit(event: "cancelled")
    }

    private func registerCaptureHotKeys() -> Bool {
        unregisterCaptureHotKeys()
        var eventType = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: UInt32(kEventHotKeyPressed)
        )
        let installed = InstallEventHandler(
            GetApplicationEventTarget(),
            { _, event, userData in
                guard let event = event, let userData = userData else {
                    return OSStatus(eventNotHandledErr)
                }
                var hotKeyId = EventHotKeyID()
                let status = GetEventParameter(
                    event,
                    EventParamName(kEventParamDirectObject),
                    EventParamType(typeEventHotKeyID),
                    nil,
                    MemoryLayout<EventHotKeyID>.size,
                    nil,
                    &hotKeyId
                )
                guard status == noErr else { return status }
                let module = Unmanaged<ScrollCaptureModule>
                    .fromOpaque(userData)
                    .takeUnretainedValue()
                DispatchQueue.main.async {
                    if hotKeyId.id == 1 {
                        module.cancelFromUI(nil)
                    } else if hotKeyId.id == 2 {
                        module.doneFromUI(nil)
                    }
                }
                return noErr
            },
            1,
            &eventType,
            Unmanaged.passUnretained(self).toOpaque(),
            &hotKeyHandler
        )
        guard installed == noErr else { return false }

        let target = GetApplicationEventTarget()
        let signature = OSType(0x5053_544b)
        var escapeId = EventHotKeyID(signature: signature, id: 1)
        var enterId = EventHotKeyID(signature: signature, id: 2)
        let escapeStatus = RegisterEventHotKey(
            UInt32(kVK_Escape),
            0,
            escapeId,
            target,
            0,
            &escapeHotKey
        )
        let enterStatus = RegisterEventHotKey(
            UInt32(kVK_Return),
            0,
            enterId,
            target,
            0,
            &enterHotKey
        )
        guard escapeStatus == noErr && enterStatus == noErr else {
            unregisterCaptureHotKeys()
            return false
        }
        return true
    }

    private func unregisterCaptureHotKeys() {
        if let hotKey = escapeHotKey {
            UnregisterEventHotKey(hotKey)
            escapeHotKey = nil
        }
        if let hotKey = enterHotKey {
            UnregisterEventHotKey(hotKey)
            enterHotKey = nil
        }
        if let handler = hotKeyHandler {
            RemoveEventHandler(handler)
            hotKeyHandler = nil
        }
    }

    @objc private func toggleAutoScrollFromUI(_ sender: NSButton) {
        guard isCapturing, !finishRequested else { return }
        if isAutoScrolling {
            stopAutoScroll()
        } else {
            startAutoScroll()
        }
    }

    @objc private func doneFromUI(_ sender: Any?) {
        guard isCapturing, !finishRequested else { return }
        stopAutoScroll()
        finishRequested = true
        updateFinishingControls(cancelled: false)
        emit(event: "done")
    }

    @objc private func cancelFromUI(_ sender: Any?) {
        if let job = finishingJob {
            if job.requestCancellation() {
                previewSession?.cancel()
                emitCancelledOnce()
                updateFinishingControls(cancelled: true)
            }
            return
        }
        guard isCapturing else { return }
        if finishRequested {
            cancelBeforeFinish = true
            previewSession?.cancel()
            emitCancelledOnce()
            updateFinishingControls(cancelled: true)
            return
        }
        cleanup()
        emitCancelledOnce()
    }

    private func screenForTopLeftOrigin(x: Int, y: Int) -> NSScreen? {
        let mainScreenHeight = primaryScreenHeight()
        return NSScreen.screens.first { screen in
            Int(screen.frame.minX.rounded()) == x
                && Int(topLeftYFromCocoaFrame(
                    screen.frame,
                    primaryHeight: mainScreenHeight
                ).rounded()) == y
        }
    }

    private func primaryScreenHeight() -> CGFloat {
        NSScreen.screens.first {
            $0.frame.minX.rounded() == 0 && $0.frame.minY.rounded() == 0
        }?.frame.height ?? NSScreen.screens.first?.frame.height ?? targetScreen?.frame.maxY ?? 0
    }

    private func cleanup() {
        stopAutoScroll()
        stopCursorMonitor()
        hideCaptureUI()

#if SCROLL_CAPTURE_TEST
        if let previewSession = previewSession {
            testRetiredPreviewSessions.append(previewSession)
        }
#endif
        previewSession?.cancel()
        previewSession = nil
        publishedPreviewFrameCount = 0
        capturedFrames.removeAll()
        isCapturing = false
        isCursorOutside = false
        lastFrameHash = nil
        duplicateFrameCount = 0
        currentScrollStep = 0
        scrollDeltaPerStep = 0
        scrollStepPoints = 0
        scrollStepRemainder = 0
        scrollStepsPerFrame = 0
        targetScreen = nil
        finishRequested = false
        cancelBeforeFinish = false
        cancelEventEmitted = false
        noOverlapReported = false
    }

    private func updatePreviewImage() {
        guard let latestFrame = capturedFrames.last else { return }

        guard let session = previewSession else { return }
        let frameIndex = capturedFrames.count - 1
        let previousFrame = frameIndex > 0 ? capturedFrames[frameIndex - 1].image : nil
        enqueuePreviewImage(
            latestFrame.image,
            previousFrame: previousFrame,
            frameCount: capturedFrames.count,
            session: session
        ) { [weak self, weak session] snapshot in
            guard let self = self,
                  let session = session,
                  self.previewSession === session,
                  self.isCapturing,
                  !self.finishRequested,
                  self.finishingJob == nil,
                  snapshot.frameCount > self.publishedPreviewFrameCount else {
                return
            }
            self.publishedPreviewFrameCount = snapshot.frameCount
#if SCROLL_CAPTURE_TEST
            if let testPreviewResponder = self.testPreviewResponder {
                testPreviewResponder(snapshot.image, snapshot.frameCount, snapshot.estimatedHeight)
                return
            }
#endif
            self.emitFrameCaptured(snapshot)
        }
    }

    private func enqueuePreviewImage(
        _ image: CGImage,
        previousFrame: CGImage?,
        frameCount: Int,
        session: ScrollPreviewSession,
        completion: ((ScrollPreviewSession.Snapshot) -> Void)?
    ) {
        let pixelScale = CGFloat(image.height) / captureArea.height
        let scrollStepPixels = Int(CGFloat(scrollStepPoints) * pixelScale)
        let expectedOverlap = image.height - min(max(scrollStepPixels, 1), image.height)
        let pointsPerPixel = captureArea.height / CGFloat(image.height)
        let backingScaleFactor = targetScreen?.backingScaleFactor ?? 1
#if SCROLL_CAPTURE_TEST
        let testPreviewAlignment = self.testPreviewAlignment
        let testPreviewEncoder = self.testPreviewEncoder
#endif

        session.enqueueFrame(
            previousFrame: previousFrame,
            frame: image,
            expectedOverlap: expectedOverlap,
            frameCount: frameCount,
            pointsPerPixel: pointsPerPixel,
            align: { [weak self] topFrame, bottomFrame, expected, isCancelled in
#if SCROLL_CAPTURE_TEST
                if let testPreviewAlignment = testPreviewAlignment {
                    return testPreviewAlignment(topFrame, bottomFrame, expected, isCancelled).map {
                        ScrollFrameAlignment(overlap: $0, footerHeight: 0)
                    }
                }
#endif
                guard let self = self else { return nil }
                return self.findOverlap(
                    topFrame: topFrame,
                    bottomFrame: bottomFrame,
                    expectedOverlap: expected,
                    isCancelled: isCancelled
                )
            },
            alignmentFailure: { [weak self, weak session] acceptedFrameCount in
                guard let self = self,
                      let session = session,
                      self.previewSession === session,
                      self.isCapturing,
                      !self.finishRequested else {
                    return
                }
                self.stopAutoScroll()
                self.noOverlapReported = true
                self.emitNoOverlap(frameCount: acceptedFrameCount)
            },
            compose: { [weak self] frames, alignments, isCancelled in
                self?.composeFrames(frames: frames, alignments: alignments, isCancelled: isCancelled)
            },
            encode: { [weak self] image in
#if SCROLL_CAPTURE_TEST
                if let testPreviewEncoder = testPreviewEncoder {
                    return testPreviewEncoder(image)
                }
#endif
                return self?.encodePreviewBase64(image, backingScaleFactor: backingScaleFactor)
            },
            completion: completion
        )
    }

    private func emitAutoScrollChanged(scrolling: Bool) {
        emit(event: "auto-scroll", data: ["scrolling": scrolling])
    }

    private func emitNoOverlap(frameCount: Int) {
        let data: [String: Any] = ["reason": "no-overlap", "frameCount": frameCount]
#if SCROLL_CAPTURE_TEST
        if let testScrollEndedResponder {
            testScrollEndedResponder(data)
            return
        }
#endif
        emit(event: "scroll-ended", data: data)
    }

    private func emitFrameCaptured(_ snapshot: ScrollPreviewSession.Snapshot) {
        let payload: [String: Any] = [
            "frameCount": snapshot.frameCount,
            "estimatedHeight": snapshot.estimatedHeight,
            "preview": snapshot.encodedImage,
            "previewWidth": snapshot.image.width,
            "previewHeight": snapshot.image.height
        ]
        emit(event: "frame", data: payload)
    }

    private func encodePreviewBase64(_ image: CGImage, backingScaleFactor: CGFloat) -> String? {
        let targetWidth = Int(previewWidthPoints * backingScaleFactor)
        guard image.width > targetWidth else {
            return base64PNG(for: image)
        }

        let scale = CGFloat(targetWidth) / CGFloat(image.width)
        let targetHeight = max(1, Int(CGFloat(image.height) * scale))

        guard let context = CGContext(
            data: nil,
            width: targetWidth,
            height: targetHeight,
            bitsPerComponent: 8,
            bytesPerRow: targetWidth * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return nil
        }

        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: targetWidth, height: targetHeight))

        guard let scaled = context.makeImage() else { return nil }
        return base64PNG(for: scaled)
    }

    private func base64PNG(for image: CGImage) -> String? {
        let mutableData = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(
            mutableData,
            UTType.png.identifier as CFString,
            1,
            nil
        ) else {
            return nil
        }

        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else { return nil }

        return mutableData.base64EncodedString()
    }

    #if SCROLL_CAPTURE_TEST
    func testRepeatedFrame(_ image: CGImage) -> (Int, [Int?], Int, Int?, Bool) {
        let params = [
            "x": AnyCodable(0), "y": AnyCodable(0),
            "width": AnyCodable(image.width), "height": AnyCodable(image.height)
        ]
        handleStart(params: params, requestId: "zero-hash-test")
        let hash = computeFrameHash(image)
        let results = (0..<4).map { _ in appendCapturedFrame(image) }
        let count = capturedFrames.count
        cleanup()
        handleStart(params: params, requestId: "zero-hash-restart-test")
        let restarted = appendCapturedFrame(image)
        cleanup()
        return (hash, results, count, restarted, lastFrameHash == nil)
    }

    func testPreviewAndSave(
        frames: [CGImage],
        step: Int,
        outputPath: String
    ) -> (CGImage, CGImage, String, [Int], Int, Int) {
        let first = frames[0]
        previewSession?.cancel()
        let session = ScrollPreviewSession()
        previewSession = session
        captureArea = NSRect(x: 0, y: 0, width: first.width, height: first.height)
        scrollStepPoints = step
        capturedFrames.removeAll()
        for (index, image) in frames.enumerated() {
            let previousFrame = index > 0 ? frames[index - 1] : nil
            capturedFrames.append(CapturedFrame(image: image, index: capturedFrames.count))
            enqueuePreviewImage(
                image,
                previousFrame: previousFrame,
                frameCount: index + 1,
                session: session,
                completion: nil
            )
        }
        guard let snapshot = session.previewSnapshot() else {
            fatalError("Failed to create scroll preview")
        }
        let alignmentSnapshot = session.alignmentSnapshot()
        let overlaps = alignmentSnapshot.alignments.map(\.overlap)
        guard stitchFrames(
            frames: capturedFrames,
            captureAreaHeight: captureArea.height,
            scrollStepPoints: step,
            alignmentSnapshot: alignmentSnapshot,
            outputPath: outputPath,
            isCancelled: { false }
        ) != nil,
        let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: outputPath) as CFURL, nil),
        let saved = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            fatalError("Failed to create preview or saved scroll capture")
        }
        return (
            snapshot.image,
            saved,
            snapshot.encodedImage,
            overlaps,
            snapshot.frameCount,
            snapshot.estimatedHeight
        )
    }

    func testFinishRace(cancelBeforeCompletion: Bool) -> (Bool, Bool, Bool, Bool) {
        let job = ScrollFinishJob()
        finishingJob = job
        let unavailableWhileFinishing = !canStartCapture
        let cancelAccepted: Bool
        let cancelled: Bool
        if cancelBeforeCompletion {
            cancelAccepted = job.requestCancellation()
            cancelled = job.complete()
        } else {
            cancelled = job.complete()
            cancelAccepted = job.requestCancellation()
        }
        finishingJob = nil
        return (cancelled, cancelAccepted, unavailableWhileFinishing, canStartCapture)
    }

    func testPrepareCaptureForFinish(frames: [CGImage], step: Int) {
        cleanup()
        capturedFrames = frames.enumerated().map { CapturedFrame(image: $0.element, index: $0.offset) }
        guard let frame = frames.first else { return }
        captureArea = NSRect(x: 0, y: 0, width: frame.width, height: frame.height)
        scrollStepPoints = step
        isCapturing = true
    }

    func testPreparePreview(step: Int, width: Int = 80, height: Int = 160) {
        cleanup()
        captureArea = NSRect(x: 0, y: 0, width: width, height: height)
        scrollStepPoints = step
        previewSession = ScrollPreviewSession()
        isCapturing = true
    }

    func testPrepareAutoScrollPreview(width: Int, height: Int, step: Int) {
        cleanup()
        captureArea = NSRect(x: 0, y: 0, width: width, height: height)
        scrollStepPoints = step
        previewSession = ScrollPreviewSession()
        isCapturing = true
        isAutoScrolling = true
    }

    func testIsAutoScrolling() -> Bool {
        isAutoScrolling
    }

    func testAppendPreviewFrame(_ image: CGImage) {
        let previousFrame = capturedFrames.last?.image
        capturedFrames.append(CapturedFrame(image: image, index: capturedFrames.count))
        guard let session = previewSession else { return }
        enqueuePreviewImage(
            image,
            previousFrame: previousFrame,
            frameCount: capturedFrames.count,
            session: session
        ) { [weak self, weak session] snapshot in
            guard let self = self,
                  let session = session,
                  self.previewSession === session,
                  self.isCapturing,
                  !self.finishRequested,
                  self.finishingJob == nil,
                  snapshot.frameCount > self.publishedPreviewFrameCount else {
                return
            }
            self.publishedPreviewFrameCount = snapshot.frameCount
            self.testPreviewResponder?(snapshot.image, snapshot.frameCount, snapshot.estimatedHeight)
        }
    }

    func testWaitForPreviewAlignment() {
        _ = previewSession?.alignmentSnapshot()
    }

    func testWaitForPreviewWork() {
        previewSession?.waitForWork()
    }

    func testWaitForRetiredPreviewWork() {
        testRetiredPreviewSessions.forEach { $0.waitForWork() }
    }

    func testSetPreviewAlignment(_ alignment: @escaping (CGImage, CGImage, Int, () -> Bool) -> Int?) {
        testPreviewAlignment = alignment
    }

    func testFixedFooterHeight(
        topFrame: CGImage,
        bottomFrame: CGImage,
        isCancelled: () -> Bool
    ) -> Int? {
        fixedFooterHeight(topFrame: topFrame, bottomFrame: bottomFrame, isCancelled: isCancelled)?.height
    }

    func testFooterDetection(
        topFrame: CGImage,
        bottomFrame: CGImage
    ) -> (height: Int, isExact: Bool)? {
        fixedFooterHeight(topFrame: topFrame, bottomFrame: bottomFrame, isCancelled: { false })
            .map { ($0.height, $0.isExact) }
    }

    func testFooterExtractionFailure(topFrame: CGImage, bottomFrame: CGImage) -> Bool {
        fixedFooterHeight(
            topFrame: topFrame,
            bottomFrame: bottomFrame,
            isCancelled: { false },
            extractPixels: { _ in nil }
        ) == nil
    }

    func testFindOverlap(topFrame: CGImage, bottomFrame: CGImage, expectedOverlap: Int) -> Int? {
        findOverlap(topFrame: topFrame, bottomFrame: bottomFrame, expectedOverlap: expectedOverlap)?.overlap
    }

    func testExactFullImageOverlap(
        topFrame: CGImage,
        bottomFrame: CGImage,
        contentHeight: Int,
        minimumOverlap: Int,
        isCancelled: () -> Bool
    ) -> Int? {
        exactFullImageOverlap(
            topFrame: topFrame,
            bottomFrame: bottomFrame,
            contentHeight: contentHeight,
            minimumOverlap: minimumOverlap,
            isCancelled: isCancelled
        )?.overlap
    }

    func testSetPreviewEncoder(_ encoder: @escaping (CGImage) -> String?) {
        testPreviewEncoder = encoder
    }

    func testSetPreviewResponder(_ responder: @escaping (CGImage, Int, Int) -> Void) {
        testPreviewResponder = responder
    }

    func testSetScrollEndedResponder(_ responder: @escaping ([String: Any]) -> Void) {
        testScrollEndedResponder = responder
    }

    func testHoldFinishWorker(started: DispatchSemaphore, release: DispatchSemaphore) {
        testFinishWorker = {
            started.signal()
            release.wait()
            return (width: 80, height: 320, frameCount: 1, stoppedForNoOverlap: false)
        }
    }

    func testSetFinishResponder(_ responder: @escaping (String, [String: Any]) -> Void) {
        testFinishResponder = responder
    }

    func testSetCancelResponder(_ responder: @escaping (String, Bool) -> Void) {
        testCancelResponder = responder
    }

    func testSetFinishAlignmentWaitStarted(_ semaphore: DispatchSemaphore) {
        testFinishAlignmentWaitStarted = semaphore
    }

    func testStartCaptureAfterFinish() -> Bool {
        handle(
            method: DaemonContract.ScrollCapture.Method.start.rawValue,
            params: [
                "x": AnyCodable(0),
                "y": AnyCodable(0),
                "width": AnyCodable(80),
                "height": AnyCodable(160),
                "nativeControls": AnyCodable(false)
            ],
            requestId: "scroll-retry-start"
        )
        return isCapturing && finishingJob == nil
    }

    func testCancelledFinishResult(outputPath: String, frameCount: Int) -> [String: Any] {
        cancelledFinishResult(outputPath: outputPath, frameCount: frameCount)
    }

    func testDoneCancelHandoff() -> (Bool, Bool, Bool, Bool) {
        isCapturing = true
        doneFromUI(nil)
        let retainedAfterDone = finishRequested && isCapturing && finishingJob == nil
        cancelFromUI(nil)
        cancelFromUI(nil)
        let retainedAfterCancel = finishRequested
            && cancelBeforeFinish
            && isCapturing
            && finishingJob == nil
            && cancelEventEmitted
        let cancelIntentEmitted = cancelEventEmitted
        cleanup()
        let lateCancelRejected = testLateCancelAfterCleanupIsFalse()
        return (retainedAfterDone, retainedAfterCancel, cancelIntentEmitted, lateCancelRejected)
    }

    private func testLateCancelAfterCleanupIsFalse() -> Bool {
        var response: Bool?
        testCancelResponder = { _, cancelled in response = cancelled }
        handleCancel(requestId: "late-cancel-test")
        testCancelResponder = nil
        return response == false
    }

    func testCancelledStitchPreservesOutputPath(_ outputPath: String) -> Bool {
        let marker = Data("existing output".utf8)
        do {
            try marker.write(to: URL(fileURLWithPath: outputPath))
        } catch {
            return false
        }
        let frame = testFrame(width: 80, height: 160, seed: 0)
        let result = stitchFrames(
            frames: [CapturedFrame(image: frame, index: 0)],
            captureAreaHeight: 160,
            scrollStepPoints: 112,
            alignmentSnapshot: nil,
            outputPath: outputPath,
            isCancelled: { true }
        )
        guard case nil = result else { return false }
        return (try? Data(contentsOf: URL(fileURLWithPath: outputPath))) == marker
    }

    func testFinishReportsAcceptedStitchPrefix(
        frames: [CGImage],
        step: Int,
        outputPath: String
    ) -> [String: Any]? {
        testPrepareCaptureForFinish(frames: frames, step: step)
        return testFinishCurrentCapture(outputPath: outputPath)
    }

    func testFinishCurrentCapture(outputPath: String) -> [String: Any]? {
        var response: [String: Any]?
        testSetFinishResponder { _, result in response = result }
        handle(
            method: DaemonContract.ScrollCapture.Method.finish.rawValue,
            params: ["outputPath": AnyCodable(outputPath)],
            requestId: "scroll-prefix-finish"
        )
        let deadline = Date(timeIntervalSinceNow: 5)
        while response == nil && Date() < deadline {
            _ = RunLoop.current.run(mode: .default, before: deadline)
        }
        return response
    }

    private func testFrame(width: Int, height: Int, seed: Int) -> CGImage {
        let color = CGColor(
            colorSpace: CGColorSpaceCreateDeviceRGB(),
            components: [CGFloat(seed), 0, 0, 1]
        )!
        let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.setFillColor(color)
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        return context.makeImage()!
    }
    #endif
}

private func cocoaYFromTopLeft(
    topLeftY: CGFloat,
    height: CGFloat,
    primaryHeight: CGFloat
) -> CGFloat {
    primaryHeight - topLeftY - height
}

private func topLeftYFromCocoaFrame(_ frame: NSRect, primaryHeight: CGFloat) -> CGFloat {
    primaryHeight - frame.maxY
}

private struct CapturedFrame {
    let image: CGImage
    let index: Int
}

enum ScrollSpeed: String {
    case slow
    case medium
    case fast

    var scrollStepsPerFrame: Int {
        switch self {
        case .slow: return 10
        case .medium: return 6
        case .fast: return 3
        }
    }

    var stepInterval: TimeInterval {
        switch self {
        case .slow: return 0.04
        case .medium: return 0.03
        case .fast: return 0.02
        }
    }
}
