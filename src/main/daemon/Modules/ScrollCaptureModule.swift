import Cocoa
import Carbon.HIToolbox
import Foundation
import ImageIO
import UniformTypeIdentifiers

class ScrollCaptureModule: NSObject, Module {
    let name = DaemonContract.ScrollCapture.module

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
    private var boundaryWindows: [NSWindow] = []
    private var controlPanel: NSPanel?
    private var autoButton: NSButton?
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

        let mainScreenHeight = primaryScreenHeight()
        let cocoaY = cocoaYFromTopLeft(
            topLeftY: CGFloat(y),
            height: CGFloat(height),
            primaryHeight: mainScreenHeight
        )

        captureArea = NSRect(x: CGFloat(x), y: cocoaY, width: CGFloat(width), height: CGFloat(height))
        capturedFrames.removeAll()
        previewImage = nil
        isCapturing = true
        lastFrameHash = 0
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

    private func showCaptureUI() -> Bool {
        hideCaptureUI()
        guard registerCaptureHotKeys() else { return false }

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
        return true
    }

    private func controlButton(title: String, action: Selector) -> NSButton {
        let button = NSButton(title: title, target: self, action: action)
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
        if isAutoScrolling {
            stopAutoScroll()
        } else {
            startAutoScroll()
        }
    }

    @objc private func doneFromUI(_ sender: Any?) {
        guard isCapturing else { return }
        stopAutoScroll()
        hideCaptureUI()
        emit(event: "done")
    }

    @objc private func cancelFromUI(_ sender: Any?) {
        guard isCapturing else { return }
        cleanup()
        emit(event: "cancelled")
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
