import Cocoa
import Foundation
import ImageIO
import UniformTypeIdentifiers

class ScrollCaptureModule: Module {
    let name = "scroll-capture"

    private var captureArea: NSRect = .zero
    private var displayId: CGDirectDisplayID = CGMainDisplayID()
    private var targetScreen: NSScreen?
    private var capturedFrames: [CapturedFrame] = []
    private var isCapturing = false
    private var isAutoScrolling = false
    private var autoScrollTimer: Timer?
    private var cursorMonitorTimer: Timer?
    private var isCursorOutside = false
    private var autoScrollSpeed: ScrollSpeed = .medium
    private var maxHeight: Int = 20000
    private var lastFrameHash: Int = 0
    private var duplicateFrameCount: Int = 0
    private var currentScrollStep: Int = 0
    private var scrollDeltaPerStep: Int32 = 0
    private var scrollStepPoints: Int = 0
    private var scrollStepRemainder: Int32 = 0
    private var scrollStepsPerFrame: Int = 0

    private let frameOverlapRatio: CGFloat = 0.3
    private let maxDuplicateFrames = 3
    private let previewWidthPoints: CGFloat = 240

    private var previewImage: CGImage?

    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        switch method {
        case "start":
            handleStart(params: params, requestId: requestId)
        case "startAutoScroll":
            handleStartAutoScroll(params: params, requestId: requestId)
        case "stopAutoScroll":
            handleStopAutoScroll(requestId: requestId)
        case "captureFrame":
            handleCaptureFrame(requestId: requestId)
        case "finish":
            handleFinish(params: params, requestId: requestId)
        case "cancel":
            handleCancel(requestId: requestId)
        case "status":
            handleStatus(requestId: requestId)
        case "hide":
            respond(id: requestId, result: ["hidden": true])
        case "show":
            respond(id: requestId, result: ["visible": true])
        default:
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
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

        if let display = params?["displayId"]?.int() {
            displayId = CGDirectDisplayID(display)
        }

        targetScreen = screenForDisplayId(displayId)

        if let speed = params?["autoScrollSpeed"]?.string() {
            autoScrollSpeed = ScrollSpeed(rawValue: speed) ?? .medium
        }

        if let max = params?["maxHeight"]?.int() {
            maxHeight = max
        }

        let screen = targetScreen ?? NSScreen.main ?? NSScreen.screens.first!
        let screenFrame = screen.frame
        let cocoaY = screenFrame.maxY - CGFloat(y) - CGFloat(height)

        captureArea = NSRect(x: CGFloat(x), y: cocoaY, width: CGFloat(width), height: CGFloat(height))
        capturedFrames.removeAll()
        previewImage = nil
        isCapturing = true
        lastFrameHash = 0
        duplicateFrameCount = 0

        let mainScreenHeight = NSScreen.screens.first?.frame.height ?? screenFrame.height
        let topLeftY = mainScreenHeight - cocoaY - CGFloat(height)

        respond(id: requestId, result: ["started": true])
        emit(event: "started", data: [
            "x": x,
            "y": Int(topLeftY),
            "width": width,
            "height": height,
            "displayId": Int(displayId)
        ])
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

    private func handleCaptureFrame(requestId: String) {
        guard isCapturing else {
            respondError(id: requestId, code: "NOT_CAPTURING", message: "Not in capture mode")
            return
        }

        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            let frameIndex = self.captureCurrentFrame()
            self.respond(id: requestId, result: [
                "captured": true,
                "frameCount": self.capturedFrames.count,
                "frameIndex": frameIndex ?? -1
            ])
        }
    }

    private func handleFinish(params: [String: AnyCodable]?, requestId: String) {
        guard let outputPath = params?["outputPath"]?.string() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "finish requires outputPath")
            return
        }

        guard isCapturing else {
            respondError(id: requestId, code: "NOT_CAPTURING", message: "Not in capture mode")
            return
        }

        stopAutoScroll()

        let frameCount = capturedFrames.count

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }

            let result = self.stitchFrames(outputPath: outputPath)

            DispatchQueue.main.async {
                self.cleanup()

                if let (width, height) = result {
                    self.respond(id: requestId, result: [
                        "success": true,
                        "outputPath": outputPath,
                        "width": width,
                        "height": height,
                        "frameCount": frameCount
                    ])
                } else {
                    self.respondError(id: requestId, code: "STITCH_ERROR", message: "Failed to stitch frames")
                }
            }
        }
    }

    private func handleCancel(requestId: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.stopAutoScroll()
            self.cleanup()
            self.emit(event: "cancelled")
            self.respond(id: requestId, result: ["cancelled": true])
        }
    }

    private func handleStatus(requestId: String) {
        let estimatedHeight = calculateEstimatedHeight()
        respond(id: requestId, result: [
            "isCapturing": isCapturing,
            "isAutoScrolling": isAutoScrolling,
            "frameCount": capturedFrames.count,
            "estimatedHeight": estimatedHeight
        ])
    }

    private func startAutoScroll() {
        guard !isAutoScrolling else { return }

        isAutoScrolling = true
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
        let screen = targetScreen ?? NSScreen.main ?? NSScreen.screens.first!
        let centerX = captureArea.midX
        let centerY = screen.frame.maxY - captureArea.midY
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
        emit(event: "cursor", data: ["outside": isCursorOutside])
    }

    private func simulateScroll(delta: Int32) {
        let mouseLocation = NSEvent.mouseLocation
        let screen = targetScreen ?? NSScreen.main ?? NSScreen.screens.first!
        let cgMouseY = screen.frame.maxY - mouseLocation.y
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
        let screen = targetScreen ?? NSScreen.main ?? NSScreen.screens.first!
        let captureRect = CGRect(
            x: captureArea.origin.x,
            y: screen.frame.maxY - captureArea.origin.y - captureArea.height,
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

        let frameHash = computeFrameHash(cgImage)

        if frameHash == lastFrameHash {
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
        emitFrameCaptured()

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

    private func stitchFrames(outputPath: String) -> (width: Int, height: Int)? {
        guard capturedFrames.count > 0 else { return nil }

        if capturedFrames.count == 1 {
            return saveSingleFrame(outputPath: outputPath)
        }

        let frameWidth = capturedFrames[0].image.width
        let frameHeight = capturedFrames[0].image.height

        let scale = CGFloat(frameHeight) / captureArea.height
        let scrollStepPixels = Int(CGFloat(scrollStepPoints) * scale)
        let clampedScrollStep = min(max(scrollStepPixels, 1), frameHeight)
        let expectedOverlapPixels = frameHeight - clampedScrollStep

        var overlaps = [Int]()
        for i in 1..<capturedFrames.count {
            let overlap = findOverlap(
                topFrame: capturedFrames[i - 1].image,
                bottomFrame: capturedFrames[i].image,
                expectedOverlap: expectedOverlapPixels
            )
            overlaps.append(overlap)
        }

        var totalHeight = frameHeight
        for i in 0..<overlaps.count {
            let newContent = capturedFrames[i + 1].image.height - overlaps[i]
            if newContent > 0 {
                totalHeight += newContent
            }
        }

        guard let context = CGContext(
            data: nil,
            width: frameWidth,
            height: totalHeight,
            bitsPerComponent: 8,
            bytesPerRow: frameWidth * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return nil
        }

        var currentY = totalHeight - frameHeight
        context.draw(capturedFrames[0].image, in: CGRect(x: 0, y: currentY, width: frameWidth, height: frameHeight))

        for i in 1..<capturedFrames.count {
            let frame = capturedFrames[i]
            let overlap = overlaps[i - 1]
            let newContentHeight = frame.image.height - overlap

            guard newContentHeight > 0 else { continue }

            let cropRect = CGRect(x: 0, y: overlap, width: frame.image.width, height: newContentHeight)
            guard let croppedImage = frame.image.cropping(to: cropRect) else { continue }

            currentY -= newContentHeight
            context.draw(croppedImage, in: CGRect(x: 0, y: currentY, width: frameWidth, height: newContentHeight))
        }

        guard let outputImage = context.makeImage() else { return nil }

        let url = URL(fileURLWithPath: outputPath)
        guard let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else {
            return nil
        }

        CGImageDestinationAddImage(destination, outputImage, nil)

        if CGImageDestinationFinalize(destination) {
            return (width: frameWidth, height: totalHeight)
        }

        return nil
    }

    private func saveSingleFrame(outputPath: String) -> (width: Int, height: Int)? {
        guard let frame = capturedFrames.first else { return nil }

        let url = URL(fileURLWithPath: outputPath)
        guard let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else {
            return nil
        }

        CGImageDestinationAddImage(destination, frame.image, nil)

        if CGImageDestinationFinalize(destination) {
            return (width: frame.image.width, height: frame.image.height)
        }

        return nil
    }

    private func findOverlap(topFrame: CGImage, bottomFrame: CGImage, expectedOverlap: Int) -> Int {
        let stripWidth = min(topFrame.width, 800)
        let stripHeight = 40

        guard let topStrip = extractBottomStrip(from: topFrame, height: stripHeight, width: stripWidth) else {
            return expectedOverlap
        }

        let minOverlap = stripHeight
        let maxOverlap = max(minOverlap, bottomFrame.height - stripHeight)

        var bestMatch = expectedOverlap
        var bestScore = Double.infinity

        let step = 4

        let checkMatch = { (overlap: Int, score: Double) in
            if score < bestScore - 1.0 {
                bestScore = score
                bestMatch = overlap
            } else if abs(score - bestScore) <= 1.0 {
                if abs(overlap - expectedOverlap) < abs(bestMatch - expectedOverlap) {
                    bestMatch = overlap
                }
            }
        }

        for overlap in stride(from: minOverlap, through: maxOverlap, by: step) {
            guard let bottomStrip = extractTopStrip(from: bottomFrame, overlap: overlap, height: stripHeight, width: stripWidth) else {
                continue
            }

            let score = compareStrips(topStrip, bottomStrip)
            checkMatch(overlap, score)
        }

        let fineMin = max(minOverlap, bestMatch - step)
        let fineMax = min(maxOverlap, bestMatch + step)

        for overlap in fineMin...fineMax {
            guard let bottomStrip = extractTopStrip(from: bottomFrame, overlap: overlap, height: stripHeight, width: stripWidth) else {
                continue
            }

            let score = compareStrips(topStrip, bottomStrip)
            checkMatch(overlap, score)
        }

        return bestMatch
    }

    private func extractBottomStrip(from image: CGImage, height: Int, width: Int) -> [UInt8]? {
        let startX = (image.width - width) / 2
        let startY = image.height - height

        let rect = CGRect(x: startX, y: startY, width: width, height: height)
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

    private func cleanup() {
        stopAutoScroll()
        stopCursorMonitor()

        previewImage = nil
        capturedFrames.removeAll()
        isCapturing = false
        isCursorOutside = false
        lastFrameHash = 0
        duplicateFrameCount = 0
        currentScrollStep = 0
        scrollDeltaPerStep = 0
        scrollStepPoints = 0
        scrollStepRemainder = 0
        scrollStepsPerFrame = 0
        targetScreen = nil
    }

    private func updatePreviewImage() {
        guard let latestFrame = capturedFrames.last else {
            previewImage = nil
            return
        }

        if capturedFrames.count == 1 {
            previewImage = latestFrame.image
            return
        }

        guard let previousImage = previewImage else {
            previewImage = latestFrame.image
            return
        }

        let pixelScale = CGFloat(latestFrame.image.height) / captureArea.height
        let scrollStepPixels = Int(CGFloat(scrollStepPoints) * pixelScale)
        let newContentHeight = min(latestFrame.image.height, scrollStepPixels)

        guard newContentHeight > 0 else { return }

        let cropRect = CGRect(
            x: 0,
            y: 0,
            width: latestFrame.image.width,
            height: newContentHeight
        )
        guard let croppedNew = latestFrame.image.cropping(to: cropRect) else { return }

        let totalHeight = previousImage.height + newContentHeight

        guard let context = CGContext(
            data: nil,
            width: previousImage.width,
            height: totalHeight,
            bitsPerComponent: 8,
            bytesPerRow: previousImage.width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return }

        context.draw(previousImage, in: CGRect(
            x: 0,
            y: newContentHeight,
            width: previousImage.width,
            height: previousImage.height
        ))

        context.draw(croppedNew, in: CGRect(
            x: 0,
            y: 0,
            width: croppedNew.width,
            height: newContentHeight
        ))

        guard let stitched = context.makeImage() else { return }
        previewImage = stitched
    }

    private func emitAutoScrollChanged(scrolling: Bool) {
        emit(event: "auto-scroll", data: ["scrolling": scrolling])
    }

    private func emitFrameCaptured() {
        var payload: [String: Any] = [
            "frameCount": capturedFrames.count,
            "estimatedHeight": calculateEstimatedHeight()
        ]

        if let preview = previewImage, let encoded = encodePreviewBase64(preview) {
            payload["preview"] = encoded
            payload["previewWidth"] = preview.width
            payload["previewHeight"] = preview.height
        }

        emit(event: "frame", data: payload)
    }

    private func encodePreviewBase64(_ image: CGImage) -> String? {
        let backingScaleFactor = targetScreen?.backingScaleFactor ?? 1
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
