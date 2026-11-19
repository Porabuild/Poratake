import Cocoa
import Carbon.HIToolbox
import Foundation
import ImageIO
import QuartzCore
import UniformTypeIdentifiers

private final class ScrollFinishJob {
    private let lock = NSLock()
    private var cancellationRequested = false
    private var completed = false
    var frames: [CGImage?]

    init(frames: [CGImage?] = []) {
        self.frames = frames
    }

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

private struct ScrollComposition {
    let image: CGImage
    let width: Int
    let height: Int
}

private final class ScrollPreviewSession {
    struct Snapshot {
        let width: Int
        let height: Int
        let encodedImage: String
        let frameCount: Int
        let estimatedHeight: Int
    }

    private struct PreviewRequest {
        var frames: [CGImage]
        let alignments: [ScrollFrameAlignment]
        let frameCount: Int
        let pointsPerPixel: CGFloat
        let compose: (inout [CGImage?], [ScrollFrameAlignment], () -> Bool) -> ScrollComposition?
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
        compose: @escaping (inout [CGImage?], [ScrollFrameAlignment], () -> Bool) -> ScrollComposition?,
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

    func alignmentSnapshot(releaseFrames: Bool = false) -> ScrollAlignmentSnapshot {
        alignmentQueue.sync {
            lock.lock()
            defer { lock.unlock() }
            let snapshot = ScrollAlignmentSnapshot(
                alignments: alignments,
                acceptedFrameCount: frames.count,
                stoppedForNoOverlap: alignmentStopped
            )
            if releaseFrames { frames.removeAll() }
            return snapshot
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
        compose: @escaping (inout [CGImage?], [ScrollFrameAlignment], () -> Bool) -> ScrollComposition?,
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
            guard var request = pendingPreview else {
                previewWorkerScheduled = false
                lock.unlock()
                return
            }
            pendingPreview = nil
            lock.unlock()

            let isStale = { [weak self] in self?.shouldStopPreview() ?? true }
            var frames = request.frames.map(Optional.some)
            request.frames.removeAll()
            guard let composition = request.compose(&frames, request.alignments, isStale), !isStale() else {
                continue
            }
            guard let encodedImage = request.encode(composition.image), !isStale() else {
                continue
            }

            let snapshot = Snapshot(
                width: composition.width,
                height: composition.height,
                encodedImage: encodedImage,
                frameCount: request.frameCount,
                estimatedHeight: Int(CGFloat(composition.height) * request.pointsPerPixel)
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
    private enum FrameCaptureResult {
        case captured
        case duplicate
        case failed
    }

    private struct FrameComparison {
        let hash: Int
        let isDuplicate: Bool
    }

    let name = DaemonContract.ScrollCapture.module

    private let frameComparisonQueue = DispatchQueue(label: "com.porabuild.poratake.scroll-frame-comparison", qos: .userInitiated)

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
    private var testBeforePreviewStrip: ((Int) -> Void)?
    private var testPreviewEncoder: ((CGImage) -> String?)?
    private var testPreviewResponder: ((Int, Int) -> Void)?
    private var testScrollEndedResponder: (([String: Any]) -> Void)?
    private var testCursorLocation: NSPoint?
    private var testScrollResponder: ((CGPoint) -> Bool)?
    private var testAutoScrollInputAllowed = true
    private var testCursorResponder: ((Bool) -> Void)?
    private var testBeforeFrameComparison: (() -> Void)?
    private var testCaptureProvider: ((@escaping (CGImage?) -> Void) -> Void)?
    private var testCoordinatedCaptureProvider: ((Set<CGWindowID>?, @escaping (CGImage?) -> Void) -> Void)?
    private var testScheduleAutoScroll: (() -> Void)?
    private var testCaptureStartResult: ((Bool) -> Void)?
    private var testFinishAlignmentWaitStarted: DispatchSemaphore?
    private var testRetiredPreviewSessions: [ScrollPreviewSession] = []
#endif
    private var isAutoScrolling = false
    private var captureGeneration: UInt64 = 0
    private var excludedWindowIDs: Set<CGWindowID>?
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
    private let minimumOverlapRows = 8

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
        guard let x = params?["x"]?.int(), Int32(exactly: x) != nil,
              let y = params?["y"]?.int(), Int32(exactly: y) != nil,
              let width = params?["width"]?.int(), Int32(exactly: width) != nil, width > 0,
              let height = params?["height"]?.int(), Int32(exactly: height) != nil, height > 0 else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "start requires valid x, y, width, height")
            return
        }
        guard !Int32(x).addingReportingOverflow(Int32(width)).overflow,
              !Int32(y).addingReportingOverflow(Int32(height)).overflow else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "Capture bounds exceed the supported range")
            return
        }
        var requestedOrigin = [0, 0]
        var hasRequestedOrigin = false
        for (index, field) in ["displayOriginX", "displayOriginY"].enumerated() {
            guard let value = params?[field], !(value.value is NSNull) else { continue }
            guard let coordinate = value.int(), Int32(exactly: coordinate) != nil else {
                respondError(id: requestId, code: "INVALID_PARAMS", message: "Invalid display origin")
                return
            }
            requestedOrigin[index] = coordinate
            hasRequestedOrigin = true
        }
        var requestedDisplayID: CGDirectDisplayID?
        if let value = params?["displayId"], !(value.value is NSNull) {
            guard let number = value.value as? Int, let display = CGDirectDisplayID(exactly: number) else {
                respondError(id: requestId, code: "INVALID_PARAMS", message: "Invalid display id")
                return
            }
            requestedDisplayID = display
        }
#if WINDOW_ID_VALIDATION_TEST
        respond(id: requestId, result: ["validated": true])
        return
#endif

        guard canStartCapture else {
            respondError(id: requestId, code: "ALREADY_CAPTURING", message: "A scroll capture is already active")
            return
        }

        let selectedDisplayID: CGDirectDisplayID
        let selectedScreen: NSScreen?
        if let requestedDisplayID = requestedDisplayID {
            guard let screen = screenForDisplayId(requestedDisplayID) else {
                respondError(id: requestId, code: "DISPLAY_UNAVAILABLE", message: "The selected display is no longer available")
                return
            }
            selectedDisplayID = requestedDisplayID
            selectedScreen = screen
        } else if hasRequestedOrigin {
            guard let screen = screenForTopLeftOrigin(x: requestedOrigin[0], y: requestedOrigin[1]),
                  let display = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID else {
                respondError(id: requestId, code: "DISPLAY_UNAVAILABLE", message: "The selected display is no longer available")
                return
            }
            selectedDisplayID = display
            selectedScreen = screen
        } else {
            selectedDisplayID = CGMainDisplayID()
            selectedScreen = screenForDisplayId(selectedDisplayID)
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

        displayId = selectedDisplayID
        targetScreen = selectedScreen

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
        let session = previewSession

        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            guard self.previewSession === session, self.isCapturing, !self.finishRequested else {
#if SCROLL_CAPTURE_TEST
                self.testCaptureStartResult?(false)
#endif
                self.respondError(id: requestId, code: "NOT_CAPTURING", message: "Scroll capture was cancelled")
                return
            }
#if SCROLL_CAPTURE_TEST
            self.testCaptureStartResult?(true)
#endif
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

        var exclusions: Set<CGWindowID>?
        if let value = params?["excludedWindowIds"], !(value.value is NSNull) {
            guard let ids = value.array(), !ids.isEmpty,
                  ids.allSatisfy({ ($0 as? Int).flatMap(CGWindowID.init(exactly:)).map { $0 != 0 } ?? false }) else {
                respondError(id: requestId, code: "INVALID_PARAMS", message: "Invalid excluded window ids")
                return
            }
            exclusions = Set(ids.compactMap { ($0 as? Int).flatMap(CGWindowID.init(exactly:)) })
        }

        let generation = captureGeneration
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            guard generation == self.captureGeneration, self.isCapturing, !self.finishRequested else {
                self.respondError(id: requestId, code: "NOT_CAPTURING", message: "Not in capture mode")
                return
            }
            guard !self.isAutoScrolling else {
                self.respond(id: requestId, result: ["autoScrolling": true])
                return
            }
            self.excludedWindowIDs = exclusions
            if exclusions != nil {
                self.boundaryWindows.forEach { $0.sharingType = .readOnly }
                self.controlPanel?.sharingType = .readOnly
            }
            let scrolling = self.startAutoScroll()
            self.respond(id: requestId, result: ["autoScrolling": scrolling])
        }
    }

    private func handleStopAutoScroll(requestId: String) {
        stopAutoScroll()
        respond(id: requestId, result: ["autoScrolling": false])
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
        let captureAreaHeight = captureArea.height
        let scrollStepPoints = self.scrollStepPoints
        let previewSession = self.previewSession
        let job = ScrollFinishJob(frames: capturedFrames.map { $0.image })
        capturedFrames.removeAll()
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
                frames: &job.frames,
                captureAreaHeight: captureAreaHeight,
                scrollStepPoints: scrollStepPoints,
                alignmentSnapshot: alignmentSnapshot,
                outputPath: outputPath,
                isCancelled: { job.isCancellationRequested() }
            )
#else
            let alignmentSnapshot = self.finishAlignedOverlaps(session: previewSession, job: job)
            let result = self.stitchFrames(
                frames: &job.frames,
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
        return session?.alignmentSnapshot(releaseFrames: true)
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
                emitScrollEnded(reason: "no-overlap", frameCount: result.frameCount)
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

    @discardableResult
    private func startAutoScroll() -> Bool {
        guard !isAutoScrolling else { return true }
        guard canPostAutoScrollInput() else {
            emitScrollEnded(reason: "input-error", frameCount: capturedFrames.count)
            return false
        }

        isAutoScrolling = true
        autoButton?.title = "Stop"
        currentScrollStep = 0
        duplicateFrameCount = 0

        let frameHeight = Int(captureArea.height)
        scrollStepPoints = max(1, Int(Double(frameHeight) * (1.0 - frameOverlapRatio)))
        scrollStepsPerFrame = max(1, min(autoScrollSpeed.scrollStepsPerFrame, scrollStepPoints))

        let stepAmount = max(1, scrollStepPoints / scrollStepsPerFrame)
        scrollDeltaPerStep = -Int32(stepAmount)
        scrollStepRemainder = -Int32(scrollStepPoints - (stepAmount * scrollStepsPerFrame))

        warpCursorToCaptureCenter()
        startCursorMonitor()
        emitAutoScrollChanged(scrolling: true)
        captureCurrentFrame(countsDuplicate: false) { [weak self] result in
            guard let self = self, self.continueAfterCapture(result) else { return }
            self.scheduleAutoScrollStep()
        }
        return isAutoScrolling
    }

    private func stopAutoScroll() {
        let wasScrolling = isAutoScrolling
        captureGeneration &+= 1
        isAutoScrolling = false
        setCursorOutside(false)
        autoScrollTimer?.invalidate()
        autoScrollTimer = nil
        autoButton?.title = "Auto"
        stopCursorMonitor()
        if wasScrolling {
            emitAutoScrollChanged(scrolling: false)
        }
    }

    private func performAutoScrollStep() {
        guard isAutoScrolling else { return }
        let location = scrollPointerLocation()
        let outside = !captureArea.contains(location)
        setCursorOutside(outside)
        guard !outside else { return }

        let stepsPerFrame = scrollStepsPerFrame
        let isFinalStep = currentScrollStep == stepsPerFrame - 1
        let delta = isFinalStep ? scrollDeltaPerStep + scrollStepRemainder : scrollDeltaPerStep
        guard postScroll(delta: delta, at: location) else {
            stopAutoScroll()
            emitScrollEnded(reason: "input-error", frameCount: capturedFrames.count)
            return
        }
        currentScrollStep += 1

        guard currentScrollStep >= stepsPerFrame else { return }

        currentScrollStep = 0
        autoScrollTimer?.invalidate()
        autoScrollTimer = nil

        let generation = captureGeneration
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { [weak self] in
            self?.captureAfterScroll(generation: generation)
        }
    }

    private func captureAfterScroll(generation: UInt64) {
        guard generation == captureGeneration, isAutoScrolling else { return }

        captureCurrentFrame { [weak self] result in
            guard let self = self, self.continueAfterCapture(result) else { return }

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
            self.scheduleAutoScrollStep()
        }
    }

    private func scheduleAutoScrollStep() {
#if SCROLL_CAPTURE_TEST
        if let testScheduleAutoScroll {
            testScheduleAutoScroll()
            return
        }
#endif
        autoScrollTimer = Timer.scheduledTimer(withTimeInterval: autoScrollSpeed.stepInterval, repeats: true) { [weak self] _ in
            self?.performAutoScrollStep()
        }
    }

    private func canPostAutoScrollInput() -> Bool {
#if SCROLL_CAPTURE_TEST
        if testCaptureProvider != nil || testScheduleAutoScroll != nil || testCoordinatedCaptureProvider != nil {
            return testAutoScrollInputAllowed
        }
#endif
        return CGPreflightPostEventAccess()
    }

    private func warpCursorToCaptureCenter() {
#if SCROLL_CAPTURE_TEST
        if testCaptureProvider != nil || testScheduleAutoScroll != nil { return }
#endif
        let centerX = captureArea.midX
        let centerY = primaryScreenHeight() - captureArea.midY
        CGWarpMouseCursorPosition(CGPoint(x: centerX, y: centerY))
    }

    private func scrollPointerLocation() -> NSPoint {
#if SCROLL_CAPTURE_TEST
        if let testCursorLocation { return testCursorLocation }
#endif
        return NSEvent.mouseLocation
    }

    private func startCursorMonitor() {
        stopCursorMonitor()
#if SCROLL_CAPTURE_TEST
        if testScheduleAutoScroll != nil { return }
#endif
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
        setCursorOutside(!captureArea.contains(scrollPointerLocation()))
    }

    private func setCursorOutside(_ outside: Bool) {
        guard outside != isCursorOutside else { return }
        isCursorOutside = outside
        let frameColor = outside
            ? NSColor(red: 249 / 255, green: 115 / 255, blue: 22 / 255, alpha: 1)
            : .systemBlue
        boundaryWindows.forEach { $0.backgroundColor = frameColor }
#if SCROLL_CAPTURE_TEST
        if let testCursorResponder {
            testCursorResponder(outside)
            return
        }
#endif
        emit(event: "cursor", data: ["outside": outside])
    }

    private func postScroll(delta: Int32, at location: NSPoint) -> Bool {
        let cgPoint = CGPoint(x: location.x, y: primaryScreenHeight() - location.y)
#if SCROLL_CAPTURE_TEST
        if let testScrollResponder {
            return testScrollResponder(cgPoint)
        }
#endif

        guard let scrollEvent = CGEvent(scrollWheelEvent2Source: nil,
                                         units: .pixel,
                                         wheelCount: 1,
                                         wheel1: delta,
                                         wheel2: 0,
                                         wheel3: 0) else { return false }
        scrollEvent.location = cgPoint
        scrollEvent.post(tap: .cghidEventTap)
        return true
    }

    private func continueAfterCapture(_ result: FrameCaptureResult) -> Bool {
        let reason: String
        switch result {
        case .captured:
            return true
        case .duplicate:
            reason = "duplicate"
        case .failed:
            reason = "capture-error"
        }
        stopAutoScroll()
        emitScrollEnded(reason: reason, frameCount: capturedFrames.count)
        return false
    }

    private func captureCurrentFrame(
        countsDuplicate: Bool = true,
        completion: @escaping (FrameCaptureResult) -> Void
    ) {
        let generation = captureGeneration
        let receive: (CGImage?) -> Void = { [weak self] image in
            self?.completeExcludedCapture(image, generation: generation, countsDuplicate: countsDuplicate) { result in
                if let result { completion(result) }
            }
        }
#if SCROLL_CAPTURE_TEST
        if let testCaptureProvider {
            testCaptureProvider(receive)
            return
        }
#endif
        let captureRect = CGRect(
            x: captureArea.origin.x,
            y: primaryScreenHeight() - captureArea.origin.y - captureArea.height,
            width: captureArea.width,
            height: captureArea.height
        )

        if let excludedWindowIDs = excludedWindowIDs {
            guard #available(macOS 14.0, *) else {
                completion(.failed)
                return
            }
            var ids = excludedWindowIDs
            for window in boundaryWindows + (controlPanel.map { [$0] } ?? []) {
                guard let id = CGWindowID(exactly: window.windowNumber), id != 0 else {
                    completion(.failed)
                    return
                }
                ids.insert(id)
            }
            let display = displayId
            let captureIDs = ids
            Task { @MainActor in
                let image = try? await captureAreaExcludingWindows(captureRect, displayID: display, excludedWindowIDs: captureIDs, requireAllWindows: true)
                receive(image)
            }
            return
        }

        let display = displayId
        Task { @MainActor [weak self] in
            guard let self, self.canAcceptFrame(generation: generation) else { return }
            let image: CGImage?
            do {
                image = try await coordinateAreaCapture(
                    registry: RecordingCaptureWindowRegistry.shared,
                    excludedWindowIDs: [],
                    unfiltered: {
#if SCROLL_CAPTURE_TEST
                        if let provider = self.testCoordinatedCaptureProvider {
                            return await withCheckedContinuation { continuation in
                                provider(nil) { continuation.resume(returning: $0) }
                            }
                        }
#endif
                        CGDisplayHideCursor(display)
                        defer { CGDisplayShowCursor(display) }
                        return CGWindowListCreateImage(
                            captureRect,
                            .optionOnScreenOnly,
                            kCGNullWindowID,
                            [.bestResolution, .boundsIgnoreFraming]
                        )
                    },
                    filtered: { ids in
                        var protectedIDs = ids
                        for window in self.boundaryWindows + (self.controlPanel.map { [$0] } ?? []) {
                            guard let id = CGWindowID(exactly: window.windowNumber), id != 0 else {
                                throw RecordingCaptureFilterError.unavailableWindows([])
                            }
                            protectedIDs.insert(id)
                        }
#if SCROLL_CAPTURE_TEST
                        if let provider = self.testCoordinatedCaptureProvider {
                            return await withCheckedContinuation { continuation in
                                provider(protectedIDs) { continuation.resume(returning: $0) }
                            }
                        }
#endif
                        guard #available(macOS 14.0, *) else {
                            throw RecordingCaptureFilterError.unavailableWindows(Array(protectedIDs))
                        }
                        return try await captureAreaExcludingWindows(
                            captureRect, displayID: display, excludedWindowIDs: protectedIDs
                        )
                    }
                )
            } catch {
                image = nil
            }
            receive(image)
        }
    }

    private func canAcceptFrame(generation: UInt64) -> Bool {
        generation == captureGeneration && isCapturing && isAutoScrolling && !finishRequested
    }

    private func completeExcludedCapture(
        _ image: CGImage?,
        generation: UInt64,
        countsDuplicate: Bool = true,
        completion: @escaping (FrameCaptureResult?) -> Void
    ) {
        guard canAcceptFrame(generation: generation) else {
            completion(nil)
            return
        }
        guard let image else {
            completion(.failed)
            return
        }
        let previous = capturedFrames.last?.image
        let previousHash = lastFrameHash
#if SCROLL_CAPTURE_TEST
        let beforeComparison = testBeforeFrameComparison
#endif
        frameComparisonQueue.async { [self] in
#if SCROLL_CAPTURE_TEST
            beforeComparison?()
#endif
            let comparison = compareFrame(image, previous: previous, previousHash: previousHash)
            DispatchQueue.main.async { [self] in
                guard canAcceptFrame(generation: generation) else {
                    completion(nil)
                    return
                }
                let index = appendCapturedFrame(image, comparison: comparison, countsDuplicate: countsDuplicate)
                completion(index == nil ? .duplicate : .captured)
            }
        }
    }

    private func compareFrame(_ image: CGImage, previous: CGImage?, previousHash: Int?) -> FrameComparison {
        let hash = computeFrameHash(image)
        let isDuplicate = hash == previousHash && previous.map { framesHaveSamePixels($0, image) } == true
        return FrameComparison(hash: hash, isDuplicate: isDuplicate)
    }

    private func appendCapturedFrame(
        _ cgImage: CGImage,
        comparison: FrameComparison,
        countsDuplicate: Bool = true
    ) -> Int? {
        if comparison.isDuplicate {
            guard countsDuplicate else { return capturedFrames.count - 1 }
            duplicateFrameCount += 1
            if duplicateFrameCount >= maxDuplicateFrames {
                return nil
            }
            return capturedFrames.count - 1
        }

        duplicateFrameCount = 0
        lastFrameHash = comparison.hash

        let frame = CapturedFrame(
            image: cgImage,
            index: capturedFrames.count
        )
        capturedFrames.append(frame)

        updatePreviewImage()

        return frame.index
    }

    private func framesHaveSamePixels(_ previous: CGImage, _ current: CGImage) -> Bool {
        if previous === current { return true }
        guard previous.width == current.width, previous.height == current.height,
              let previousPixels = extractPixelData(from: previous),
              let currentPixels = extractPixelData(from: current) else { return false }
        return previousPixels == currentPixels
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

        context.draw(image, in: CGRect(x: 0, y: 0, width: sampleWidth, height: sampleHeight))

        guard let data = context.data else { return 0 }
        let buffer = data.bindMemory(to: UInt8.self, capacity: sampleWidth * sampleHeight * 4)

        var hash = 0
        let strideAmount = 16
        for i in Swift.stride(from: 0, to: sampleWidth * sampleHeight * 4, by: strideAmount) {
            let rgb = (Int(buffer[i]) << 16) | (Int(buffer[i + 1]) << 8) | Int(buffer[i + 2])
            hash = hash &* 31 &+ rgb
        }

        return hash
    }

    private func calculateEstimatedHeight() -> Int {
        guard capturedFrames.count > 0 else { return 0 }

        let frameHeight = Int(captureArea.height)
        return frameHeight + (capturedFrames.count - 1) * scrollStepPoints
    }

    private func stitchFrames(
        frames: inout [CGImage?],
        captureAreaHeight: CGFloat,
        scrollStepPoints: Int,
        alignmentSnapshot: ScrollAlignmentSnapshot?,
        outputPath: String,
        isCancelled: () -> Bool
    ) -> (width: Int, height: Int, frameCount: Int, stoppedForNoOverlap: Bool)? {
        defer { frames.removeAll() }
        guard !isCancelled(), let frameHeight = frames.first.flatMap({ $0?.height }) else { return nil }

        let scale = CGFloat(frameHeight) / captureAreaHeight
        let scrollStepPixels = Int(CGFloat(scrollStepPoints) * scale)
        let clampedScrollStep = min(max(scrollStepPixels, 1), frameHeight)
        let expectedOverlapPixels = frameHeight - clampedScrollStep
        let frameLimit = alignmentSnapshot?.stoppedForNoOverlap == true
            ? max(1, min(alignmentSnapshot?.acceptedFrameCount ?? 1, frames.count))
            : frames.count

        var alignments = [ScrollFrameAlignment]()
        for index in 1..<frameLimit {
            guard !isCancelled(), let top = frames[index - 1], let bottom = frames[index] else { return nil }
            let alignment: ScrollFrameAlignment?
            if let alignmentSnapshot, index - 1 < alignmentSnapshot.alignments.count {
                alignment = alignmentSnapshot.alignments[index - 1]
            } else {
                alignment = findOverlap(
                    topFrame: top, bottomFrame: bottom,
                    expectedOverlap: expectedOverlapPixels, isCancelled: isCancelled
                )
            }
            guard let alignment else { break }
            alignments.append(alignment)
        }

        let acceptedCount = alignments.count + 1
        let stoppedForNoOverlap = acceptedCount < frames.count || alignmentSnapshot?.stoppedForNoOverlap == true
        frames.removeLast(frames.count - acceptedCount)
        guard let outputImage = composeFrames(
            frames: &frames, alignments: alignments, isCancelled: isCancelled
        )?.image, let result = savePNG(
            frame: outputImage, outputPath: outputPath, isCancelled: isCancelled
        ) else { return nil }
        return (result.width, result.height, acceptedCount, stoppedForNoOverlap)
    }

    private func composeFrames(
        frames: inout [CGImage?],
        alignments: [ScrollFrameAlignment],
        maximumWidth: Int? = nil,
        isCancelled: () -> Bool
    ) -> ScrollComposition? {
        guard let firstFrame = frames.first ?? nil, !isCancelled(),
              alignments.count == frames.count - 1 else { return nil }
        let width = firstFrame.width
        let targetWidth = min(width, maximumWidth ?? width)
        if frames.count == 1 && targetWidth == width {
            frames[0] = nil
            return ScrollComposition(image: firstFrame, width: width, height: firstFrame.height)
        }

        let trimTop = [0] + alignments.map(\.overlap)
        let trimBottom = alignments.map(\.footerHeight) + [0]
        var contentHeights: [Int] = []
        var height = 0
        for index in frames.indices {
            guard !isCancelled(), let frame = frames[index] else { return nil }
            let contentHeight = frame.height - trimTop[index] - trimBottom[index]
            guard contentHeight > 0 else { return nil }
            contentHeights.append(contentHeight)
            height += contentHeight
        }

        let targetHeight = max(1, Int(CGFloat(height) * (CGFloat(targetWidth) / CGFloat(width))))
        guard !isCancelled(), let context = scrollBitmapContext(
            width: targetWidth, height: targetHeight, matching: firstFrame
        ) else { return nil }

        if targetWidth != width { context.interpolationQuality = .high }
        let scaleY = CGFloat(targetHeight) / CGFloat(height)
        var remainingRows = height
        var top = targetHeight
        for index in frames.indices {
#if SCROLL_CAPTURE_TEST
            if maximumWidth != nil { testBeforePreviewStrip?(index) }
#endif
            guard !isCancelled(), let frame = frames[index] else { return nil }
            frames[index] = nil
            let contentHeight = contentHeights[index]
            remainingRows -= contentHeight
            let bottom = Int((CGFloat(remainingRows) * scaleY).rounded())
            if bottom < top {
                let crop = CGRect(x: 0, y: trimTop[index], width: frame.width, height: contentHeight)
                guard let image = frame.cropping(to: crop) else { return nil }
                context.draw(image, in: CGRect(x: 0, y: bottom, width: targetWidth, height: top - bottom))
            }
            top = bottom
        }

        guard !isCancelled(), let image = context.makeImage() else { return nil }
        return ScrollComposition(image: image, width: width, height: height)
    }

    private func savePNG(
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
        guard let (headerHeight, footer) = fixedViewportEdges(
            topFrame: topFrame,
            bottomFrame: bottomFrame,
            isCancelled: isCancelled
        ) else {
            return nil
        }
        guard !isCancelled() else { return nil }
        let footerHeight = footer.height
        let contentHeight = topFrame.height - footerHeight

        guard contentHeight >= stripHeight,
              let topStrip = extractStrip(
                  from: topFrame, y: contentHeight - stripHeight, height: stripHeight, width: stripWidth
              ),
              !isCancelled(),
              let bottomStrip = extractStrip(from: bottomFrame, y: 0, height: contentHeight, width: stripWidth),
              !isCancelled() else { return nil }
        let bytesPerRow = stripWidth * 4

        let minOverlap = headerHeight == 0 ? stripHeight : headerHeight + minimumOverlapRows
        let maxOverlap = contentHeight
        guard minOverlap <= maxOverlap else { return nil }
        let firstFullOverlap = min(headerHeight + stripHeight, maxOverlap + 1)
        let adjustedExpected = footerHeight > 0
            ? max(minOverlap, expectedOverlap - footerHeight)
            : expectedOverlap

        var bestMatch = adjustedExpected
        var bestScore = Double.infinity
        var scores = [(overlap: Int, score: Double)]()

        let step = 4
        let scoreMargin = 1.0

        func overlapRange(_ overlap: Int, rowBytes: Int) -> Range<Int> {
            let height = min(stripHeight, overlap - headerHeight)
            return (overlap - height) * rowBytes..<overlap * rowBytes
        }

        func checkMatch(_ overlap: Int) -> Bool {
            guard !isCancelled() else { return false }
            let score = compareStrips(topStrip, bottomStrip[overlapRange(overlap, rowBytes: bytesPerRow)])
            scores.append((overlap, score))
            if score < bestScore - scoreMargin {
                bestScore = score
                bestMatch = overlap
            } else if abs(score - bestScore) <= scoreMargin {
                if abs(overlap - adjustedExpected) < abs(bestMatch - adjustedExpected) {
                    bestMatch = overlap
                }
            }
            return true
        }

        for overlap in minOverlap..<firstFullOverlap {
            guard checkMatch(overlap) else { return nil }
        }
        for overlap in stride(from: firstFullOverlap, through: maxOverlap, by: step) {
            guard checkMatch(overlap) else { return nil }
        }
        let shortMatchIsCompetitive = bestScore / 3 <= maxMeanOverlapChannelDifference
            && scores.contains { $0.overlap < firstFullOverlap && $0.score <= bestScore + scoreMargin }
        if shortMatchIsCompetitive {
            for overlap in firstFullOverlap..<(maxOverlap + 1) where (overlap - firstFullOverlap) % step != 0 {
                guard checkMatch(overlap) else { return nil }
            }
        } else {
            let fineMin = max(firstFullOverlap, bestMatch - step)
            let fineMax = min(maxOverlap, bestMatch + step)
            for overlap in stride(from: fineMin, through: fineMax, by: 1) {
                guard checkMatch(overlap) else { return nil }
            }
        }

        let nearMatches = scores.filter { $0.score <= bestScore + scoreMargin }
        let nearOverlapMinimum = nearMatches.map(\.overlap).min()
        let nearOverlapMaximum = nearMatches.map(\.overlap).max()
        let hasAmbiguousMatches: Bool
        if let minimum = nearOverlapMinimum, let maximum = nearOverlapMaximum {
            hasAmbiguousMatches = minimum < firstFullOverlap
                ? minimum != maximum
                : maximum - minimum >= 8
        } else {
            hasAmbiguousMatches = false
        }
        let isConfidentCenterScore = bestScore / 3 <= maxMeanOverlapChannelDifference
        let isMinimumOverlapWithoutHeader = headerHeight == 0 && bestMatch == minOverlap
        let requiresExactOverlap = !footer.isExact || hasAmbiguousMatches
            || !isConfidentCenterScore || isMinimumOverlapWithoutHeader
        let canResolveCenterTie = hasAmbiguousMatches && isConfidentCenterScore
            && !isMinimumOverlapWithoutHeader && footer.isExact && stripWidth < topFrame.width
        var wideScore: Double?
        if !requiresExactOverlap, stripWidth < topFrame.width,
           let centerScore = scores.first(where: { $0.overlap == bestMatch })?.score {
            let height = min(stripHeight, bestMatch - headerHeight)
            guard !isCancelled(),
                  let top = extractStrip(
                      from: topFrame, y: contentHeight - height, height: height, width: topFrame.width
                  ),
                  !isCancelled(),
                  let bottom = extractStrip(
                      from: bottomFrame, y: bestMatch - height, height: height, width: bottomFrame.width
                  ) else { return nil }
            if top != bottom {
                let fullWidthScore = compareStrips(top, bottom[...])
                wideScore = fullWidthScore > centerScore ? fullWidthScore / 3 : nil
            }
        }
        func recoverFullWidthOverlap() -> Int? {
            let width = topFrame.width
            guard !isCancelled(),
                  let top = extractStrip(from: topFrame, y: contentHeight - stripHeight, height: stripHeight, width: width),
                  !isCancelled(),
                  let bottom = extractStrip(from: bottomFrame, y: 0, height: contentHeight, width: width) else {
                return nil
            }
            let rowBytes = width * 4
            let maximumScore = maxMeanOverlapChannelDifference * 3
            var overlap = contentHeight
            var score = Double.infinity
            var alternativeScore = Double.infinity
            for candidate in minOverlap...maxOverlap {
                guard !isCancelled() else { return nil }
                let range = overlapRange(candidate, rowBytes: rowBytes)
                let ceiling = min(score, maximumScore) + scoreMargin
                guard let candidateScore = boundedStripScore(
                    top, bottom[range], rowBytes: rowBytes, ceiling: ceiling
                ) else { continue }
                if candidateScore < score {
                    alternativeScore = score
                    score = candidateScore
                    overlap = candidate
                } else {
                    alternativeScore = min(alternativeScore, candidateScore)
                }
            }
            guard !isCancelled(), overlap < contentHeight,
                  score <= maximumScore,
                  alternativeScore > score + scoreMargin else { return nil }
            return overlap
        }

        if requiresExactOverlap || wideScore != nil {
            if let (overlap, alternative) = exactFullImageOverlap(
                topFrame: topFrame,
                bottomFrame: bottomFrame,
                contentHeight: contentHeight,
                minimumOverlap: minimumOverlapRows,
                isCancelled: isCancelled
            ) {
                guard alternative < minimumOverlapRows else { return nil }
                return ScrollFrameAlignment(overlap: overlap, footerHeight: footerHeight)
            }
            if canResolveCenterTie || wideScore.map({ $0 > maxMeanOverlapChannelDifference }) == true {
                guard let overlap = recoverFullWidthOverlap() else { return nil }
                return ScrollFrameAlignment(overlap: overlap, footerHeight: footerHeight)
            }
            guard !requiresExactOverlap else { return nil }
        }

        guard !isCancelled(), bestMatch < contentHeight else { return nil }
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
        func matchBody(startRow: Int) -> (overlap: Int, alternative: Int)? {
            let rowCount = contentHeight - startRow
            var prefix = [Int](repeating: 0, count: rowCount)
            for index in 1..<rowCount {
                guard !isCancelled() else { return nil }
                var matched = prefix[index - 1]
                while matched > 0 {
                    guard !isCancelled() else { return nil }
                    if rowEquals(bottomPixels, startRow + index, bottomPixels, startRow + matched) { break }
                    matched = prefix[matched - 1]
                }
                if rowEquals(bottomPixels, startRow + index, bottomPixels, startRow + matched) {
                    matched += 1
                }
                prefix[index] = matched
            }

            var matched = 0
            for row in startRow..<contentHeight {
                guard !isCancelled() else { return nil }
                while matched > 0 {
                    guard !isCancelled() else { return nil }
                    if rowEquals(topPixels, row, bottomPixels, startRow + matched) { break }
                    matched = prefix[matched - 1]
                }
                if rowEquals(topPixels, row, bottomPixels, startRow + matched) {
                    matched += 1
                }
            }
            return (matched, matched == 0 ? 0 : prefix[matched - 1])
        }

        guard contentHeight > minimumOverlap, let match = matchBody(startRow: 0) else { return nil }
        if match.overlap >= minimumOverlap {
            return match.overlap < contentHeight ? match : nil
        }

        var headerHeight = 0
        for row in 0..<contentHeight {
            guard !isCancelled() else { return nil }
            guard rowEquals(topPixels, row, bottomPixels, row) else { break }
            headerHeight += 1
        }
        guard headerHeight > 0, contentHeight - headerHeight > minimumOverlap,
              let bodyMatch = matchBody(startRow: headerHeight),
              bodyMatch.overlap >= minimumOverlap, bodyMatch.alternative == 0 else { return nil }
        return (headerHeight + bodyMatch.overlap, 0)
    }

    private func fixedViewportEdges(
        topFrame: CGImage,
        bottomFrame: CGImage,
        isCancelled: () -> Bool,
        extractPixels: ((CGImage) -> [UInt8]?)? = nil
    ) -> (headerHeight: Int, footer: ScrollFooterDetection)? {
        guard !isCancelled() else { return nil }
        let noFooter = ScrollFooterDetection(height: 0, isExact: true)
        guard topFrame.width == bottomFrame.width,
              topFrame.height == bottomFrame.height else {
            return (0, noFooter)
        }
        let extract = extractPixels ?? { image in self.extractPixelData(from: image) }
        let topPixels = extract(topFrame)
        guard !isCancelled() else { return nil }
        guard let topPixels else { return nil }
        let bottomPixels = extract(bottomFrame)
        guard !isCancelled() else { return nil }
        guard let bottomPixels else { return nil }

        let rowBytes = topFrame.width * 4
        func stationaryRows(_ rows: some Sequence<Int>) -> (height: Int, isExact: Bool)? {
            var height = 0
            var isExact = true
            for row in rows {
                guard !isCancelled() else { return nil }
                let range = row * rowBytes..<(row + 1) * rowBytes
                if topPixels[range] == bottomPixels[range] {
                    height += 1
                    continue
                }

                var difference = 0
                for index in stride(from: range.lowerBound, to: range.upperBound, by: 4) {
                    difference += abs(Int(topPixels[index]) - Int(bottomPixels[index]))
                    difference += abs(Int(topPixels[index + 1]) - Int(bottomPixels[index + 1]))
                    difference += abs(Int(topPixels[index + 2]) - Int(bottomPixels[index + 2]))
                }
                guard Double(difference) / Double(rowBytes / 4 * 3) <= maxMeanOverlapChannelDifference else {
                    break
                }
                isExact = false
                height += 1
            }
            return (height, isExact)
        }

        guard let trailing = stationaryRows((0..<topFrame.height).reversed()) else { return nil }
        let footer = trailing.height >= minimumOverlapRows && trailing.height < topFrame.height / 2
            ? ScrollFooterDetection(height: trailing.height, isExact: trailing.isExact)
            : noFooter
        let contentHeight = topFrame.height - footer.height
        guard let leading = stationaryRows(0..<contentHeight),
              !isCancelled() else { return nil }
        let headerHeight = leading.height >= minimumOverlapRows && contentHeight - leading.height >= minimumOverlapRows
            ? leading.height
            : 0
        return (headerHeight, footer)
    }

    private func extractStrip(from image: CGImage, y: Int, height: Int, width: Int) -> [UInt8]? {
        let startX = (image.width - width) / 2

        let rect = CGRect(x: startX, y: y, width: width, height: height)
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

    private func stripDifference(_ first: ArraySlice<UInt8>, _ second: ArraySlice<UInt8>) -> Int {
        var difference = 0
        for offset in stride(from: 0, to: second.count, by: 4) {
            let a = first.startIndex + offset
            let b = second.startIndex + offset
            difference += abs(Int(first[a]) - Int(second[b]))
            difference += abs(Int(first[a + 1]) - Int(second[b + 1]))
            difference += abs(Int(first[a + 2]) - Int(second[b + 2]))
        }
        return difference
    }

    private func compareStrips(_ strip1: [UInt8], _ strip2: ArraySlice<UInt8>) -> Double {
        guard !strip2.isEmpty, strip1.count >= strip2.count else { return Double.infinity }
        return Double(stripDifference(strip1.suffix(strip2.count), strip2)) / Double(strip2.count / 4)
    }

    private func boundedStripScore(
        _ first: [UInt8], _ second: ArraySlice<UInt8>, rowBytes: Int, ceiling: Double
    ) -> Double? {
        let pixels = Double(second.count / 4)
        let start = first.count - second.count
        var difference = 0
        for offset in stride(from: 0, to: second.count, by: rowBytes) {
            let a = start + offset
            let b = second.startIndex + offset
            difference += stripDifference(first[a..<(a + rowBytes)], second[b..<(b + rowBytes)])
            guard Double(difference) / pixels <= ceiling else { return nil }
        }
        return Double(difference) / pixels
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
        boundaryWindows.forEach { $0.alphaValue = 0 }
        controlPanel?.alphaValue = 0
        if !boundaryWindows.isEmpty || controlPanel != nil { CATransaction.flush() }
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
        excludedWindowIDs = nil
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
                testPreviewResponder(snapshot.frameCount, snapshot.estimatedHeight)
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
        let previewWidth = max(1, Int(previewWidthPoints * (targetScreen?.backingScaleFactor ?? 1)))
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
                self.emitScrollEnded(reason: "no-overlap", frameCount: acceptedFrameCount)
            },
            compose: { [weak self] frames, alignments, isCancelled in
                self?.composeFrames(frames: &frames, alignments: alignments, maximumWidth: previewWidth, isCancelled: isCancelled)
            },
            encode: { [weak self] image in
#if SCROLL_CAPTURE_TEST
                if let testPreviewEncoder = testPreviewEncoder {
                    return testPreviewEncoder(image)
                }
#endif
                return self?.base64PNG(for: image)
            },
            completion: completion
        )
    }

    private func emitAutoScrollChanged(scrolling: Bool) {
        emit(event: "auto-scroll", data: ["scrolling": scrolling])
    }

    private func emitScrollEnded(reason: String, frameCount: Int) {
        let data: [String: Any] = ["reason": reason, "frameCount": frameCount]
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
            "previewWidth": snapshot.width,
            "previewHeight": snapshot.height
        ]
        emit(event: "frame", data: payload)
    }

    private func scrollBitmapContext(width: Int, height: Int, matching image: CGImage) -> CGContext? {
        func makeContext(colorSpace: CGColorSpace) -> CGContext? {
            CGContext(
                data: nil,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width * 4,
                space: colorSpace,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            )
        }

        if let space = image.colorSpace, space.model == .rgb, let context = makeContext(colorSpace: space) {
            return context
        }
        return makeContext(colorSpace: CGColorSpaceCreateDeviceRGB())
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
    func testShowCaptureUI(rect: NSRect) -> [NSWindow] {
        captureArea = rect
        showBoundaryFrame()
        let panel = NSPanel(contentRect: NSRect(x: rect.minX, y: rect.maxY + 8, width: 80, height: 30),
                            styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.orderFrontRegardless()
        controlPanel = panel
        return boundaryWindows + [panel]
    }

    func testHideCaptureUI() { hideCaptureUI() }

    func testEnqueueCaptureStart(width: Int, height: Int) {
        handleStart(params: [
            "x": AnyCodable(0), "y": AnyCodable(0),
            "width": AnyCodable(width), "height": AnyCodable(height)
        ], requestId: "stale-scroll-start")
    }

    func testCancelCapture() {
        handleCancel(requestId: "cancel-stale-scroll-start")
    }

    func testSetCaptureStartResult(_ responder: @escaping (Bool) -> Void) {
        testCaptureStartResult = responder
    }

    func testEnqueueAutoScrollStart() {
        handleStartAutoScroll(params: nil, requestId: "stale-auto-scroll-start")
    }

    func testEnqueueAutoScrollStop() {
        handleStopAutoScroll(requestId: "stale-auto-scroll-stop")
    }

    func testSetCaptureProvider(_ provider: @escaping (@escaping (CGImage?) -> Void) -> Void) {
        testCaptureProvider = provider
    }

    func testSetCoordinatedCaptureProvider(_ provider: @escaping (Set<CGWindowID>?, @escaping (CGImage?) -> Void) -> Void) {
        testCoordinatedCaptureProvider = provider
    }

    func testSetScheduleAutoScroll(_ schedule: @escaping () -> Void) {
        testScheduleAutoScroll = schedule
    }

    func testCaptureAfterScroll(generation: UInt64) {
        captureAfterScroll(generation: generation)
    }

    func testCaptureGeneration() -> UInt64 {
        captureGeneration
    }

    func testStopAutoScroll() {
        stopAutoScroll()
    }

    func testResumeAutoScroll() {
        isAutoScrolling = true
    }

    func testSetBeforeFrameComparison(_ hook: @escaping () -> Void) {
        testBeforeFrameComparison = hook
    }

    func testCompleteExcludedCapture(_ image: CGImage?, generation: UInt64) async -> Bool? {
        await withCheckedContinuation { continuation in
            completeExcludedCapture(image, generation: generation) {
                continuation.resume(returning: $0.map { self.continueAfterCapture($0) })
            }
        }
    }

    func testConsumeCaptureImage(_ image: CGImage?) -> Bool {
        guard let image else { return continueAfterCapture(.failed) }
        return continueAfterCapture(appendTestFrame(image) == nil ? .duplicate : .captured)
    }

    private func appendTestFrame(_ image: CGImage) -> Int? {
        let comparison = compareFrame(image, previous: capturedFrames.last?.image, previousHash: lastFrameHash)
        return appendCapturedFrame(image, comparison: comparison)
    }

    func testFrameSequence(_ frames: [CGImage]) -> (Int, [Int?], Int, Int?, Bool) {
        let image = frames[0]
        let params = [
            "x": AnyCodable(0), "y": AnyCodable(0),
            "width": AnyCodable(image.width), "height": AnyCodable(image.height)
        ]
        handleStart(params: params, requestId: "zero-hash-test")
        let hash = computeFrameHash(image)
        let results = frames.map { appendTestFrame($0) }
        let count = capturedFrames.count
        cleanup()
        handleStart(params: params, requestId: "zero-hash-restart-test")
        let restarted = appendTestFrame(image)
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
        var previewFrames = frames.prefix(snapshot.frameCount).map(Optional.some)
        var finalFrames: [CGImage?] = capturedFrames.map { $0.image }
        guard let unscaled = composeFrames(
            frames: &previewFrames,
            alignments: Array(alignmentSnapshot.alignments.prefix(snapshot.frameCount - 1)),
            isCancelled: { false }
        ), stitchFrames(
            frames: &finalFrames,
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
            unscaled.image,
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

    func testIsCapturing() -> Bool {
        isCapturing
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
            self.testPreviewResponder?(snapshot.frameCount, snapshot.estimatedHeight)
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
        fixedViewportEdges(topFrame: topFrame, bottomFrame: bottomFrame, isCancelled: isCancelled)?.footer.height
    }

    func testFooterDetection(
        topFrame: CGImage,
        bottomFrame: CGImage
    ) -> (height: Int, isExact: Bool)? {
        fixedViewportEdges(topFrame: topFrame, bottomFrame: bottomFrame, isCancelled: { false })
            .map { ($0.footer.height, $0.footer.isExact) }
    }

    func testFooterExtractionFailure(topFrame: CGImage, bottomFrame: CGImage) -> Bool {
        fixedViewportEdges(
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

    func testSetBeforePreviewStrip(_ hook: @escaping (Int) -> Void) {
        testBeforePreviewStrip = hook
    }

    func testSetPreviewEncoder(_ encoder: @escaping (CGImage) -> String?) {
        testPreviewEncoder = encoder
    }

    func testSetPreviewResponder(_ responder: @escaping (Int, Int) -> Void) {
        testPreviewResponder = responder
    }

    func testSetScrollEndedResponder(_ responder: @escaping ([String: Any]) -> Void) {
        testScrollEndedResponder = responder
    }

    func testSetCursorResponder(_ responder: @escaping (Bool) -> Void) {
        testCursorResponder = responder
    }

    func testCheckCursor(inside: Bool) {
        testCursorLocation = NSPoint(x: inside ? captureArea.midX : captureArea.minX - 1, y: captureArea.midY)
        checkCursorPosition()
        testCursorLocation = nil
    }

    func testSetAutoScrollInputAllowed(_ allowed: Bool) {
        testAutoScrollInputAllowed = allowed
    }

    func testPerformAutoScrollStep(at point: NSPoint, post: @escaping (CGPoint) -> Bool) {
        testCursorLocation = point
        testScrollResponder = post
        performAutoScrollStep()
        testCursorLocation = nil
        testScrollResponder = nil
    }

    func testInstallBoundary(_ window: NSWindow) {
        boundaryWindows.append(window)
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
        var frames: [CGImage?] = [testFrame(width: 80, height: 160, seed: 0)]
        let result = stitchFrames(
            frames: &frames,
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
