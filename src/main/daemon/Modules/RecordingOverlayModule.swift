import Cocoa
import Foundation
import QuartzCore

class RecordingOverlayModule: Module {
    let name = DaemonContract.RecordingOverlay.module
    private var windows: [RecordingOverlayWindow] = []
    private var overlayViews: [NSScreen: RecordingOverlayView] = [:]
    private var highlightWindow: RecordingOverlayWindow?
    private var highlightTimer: Timer?
    private var highlightTarget: RecordingWindowTarget?
    private var presentationGeneration = WindowPresentationGeneration()

    private static let highlightThickness: CGFloat = 1
    private static let highlightGap: CGFloat = 1
    private static let highlightRadius: CGFloat = 8
    private static let highlightInterval: TimeInterval = 1.0 / 30.0
    private static let highlightFallback = NSColor(
        red: 0.533, green: 0.573, blue: 0.937, alpha: 1.0
    )

    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.RecordingOverlay.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .show:
            handleShow(params: params, requestId: requestId)
        case .showWindow:
            handleShowWindow(params: params, requestId: requestId)
        case .hide:
            handleHide(requestId: requestId)
        }
    }

    private func handleShowWindow(params: [String: AnyCodable]?, requestId: String) {
        guard let rawWindowId = params?["windowId"]?.value as? Int,
              let windowId = CGWindowID(exactly: rawWindowId),
              windowId != kCGNullWindowID else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "showWindow requires windowId")
            return
        }
#if WINDOW_ID_VALIDATION_TEST
        respond(id: requestId, result: ["validated": true])
        return
#endif

        let color = themeColor(params?["color"]?.string()) ?? Self.highlightFallback

        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            Task { @MainActor in
                do {
                    let visible = try await self.showWindowHighlight(
                        windowID: windowId,
                        color: color
                    )
                    self.respond(id: requestId, result: ["visible": visible])
                } catch {
                    self.respondError(id: requestId, code: "CAPTURE_ERROR", message: error.localizedDescription)
                }
            }
        }
    }

    private func handleShow(params: [String: AnyCodable]?, requestId: String) {
        guard let params = params,
              let x = params["x"]?.int(),
              let y = params["y"]?.int(),
              let width = params["width"]?.int(),
              let height = params["height"]?.int() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "show requires x, y, width, height")
            return
        }
        
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            Task { @MainActor in
                do {
                    let visible = try await self.showOverlay(x: x, y: y, width: width, height: height)
                    self.respond(id: requestId, result: ["visible": visible])
                } catch {
                    self.respondError(id: requestId, code: "CAPTURE_ERROR", message: error.localizedDescription)
                }
            }
        }
    }
    
    private func handleHide(requestId: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            Task { @MainActor in
                self.hideOverlay()
                #if RECORDING_OUTLINE_TEST
                self.testHideReplies.append(requestId)
                #endif
                self.respond(id: requestId, result: ["visible": false])
                try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
            }
        }
    }
    
    /// Frames the recorded window from the outside and keeps up with it, so it
    /// stays obvious which window the recording is following.
    @MainActor
    private func showWindowHighlight(windowID: CGWindowID, color: NSColor) async throws -> Bool {
        let generation = presentationGeneration.begin()
        removeOverlayWindows()
        await RecordingCaptureWindowRegistry.shared.waitForCaptureStartIfNeeded()
        guard presentationGeneration.isCurrent(generation) else { return false }

        guard let target = RecordingWindowTarget(id: windowID),
              case .visible = target.observe() else { return false }

        let window = RecordingOverlayWindow(
            contentRect: .zero,
            styleMask: .borderless,
            backing: .buffered,
            defer: false
        )
        window.level = .screenSaver
        window.isOpaque = false
        window.backgroundColor = .clear
        window.ignoresMouseEvents = true
        window.hasShadow = false
        window.sharingType = .none
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        window.contentView = RecordingHighlightView(
            frame: .zero,
            thickness: Self.highlightThickness,
            radius: Self.highlightRadius,
            color: color
        )
        window.alphaValue = 0

        highlightWindow = window
        highlightTarget = target
        window.orderFrontRegardless()
        guard RecordingCaptureWindowRegistry.shared.register(window) != nil else {
            highlightWindow = nil
            highlightTarget = nil
            window.orderOut(nil)
            throw RecordingCaptureFilterError.unavailableWindowNumber
        }
        do {
            try await RecordingCaptureWindowRegistry.shared.waitForWindowPresentation()
        } catch {
            guard presentationGeneration.isCurrent(generation), highlightWindow === window else { return false }
            RecordingCaptureWindowRegistry.shared.unregister(window)
            highlightWindow = nil
            highlightTarget = nil
            window.orderOut(nil)
            try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
            throw error
        }
        guard presentationGeneration.isCurrent(generation), highlightWindow === window else {
            if highlightWindow !== window {
                window.orderOut(nil)
                RecordingCaptureWindowRegistry.shared.unregister(window)
                try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
            }
            return false
        }
        followHighlight()
        guard highlightTarget != nil else {
            removeOverlayWindows()
            try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
            return false
        }

        // AppKit has no cross-application window-move notification without the
        // accessibility permission, so the frame is sampled instead.
        highlightTimer = Timer.scheduledTimer(
            withTimeInterval: Self.highlightInterval,
            repeats: true
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.followHighlight()
            }
        }

        return true
    }

    @MainActor
    private func followHighlight() {
        guard let window = highlightWindow, let target = highlightTarget else { return }
        let bounds: CGRect
        switch target.observe() {
        case let .visible(rect):
            bounds = rect
        case .absent:
            highlightTimer?.invalidate()
            highlightTimer = nil
            highlightTarget = nil
            fallthrough
        case .hidden:
            if window.alphaValue != 0 {
                window.alphaValue = 0
            }
            return
        }

        let inset = Self.highlightThickness + Self.highlightGap
        let mainScreenHeight = NSScreen.screens.first?.frame.height ?? 0
        let frame = NSRect(
            x: bounds.origin.x - inset,
            y: mainScreenHeight - bounds.origin.y - bounds.height - inset,
            width: bounds.width + inset * 2,
            height: bounds.height + inset * 2
        )

        if window.frame != frame {
            window.setFrame(frame, display: true)
            window.contentView?.frame = NSRect(origin: .zero, size: frame.size)
            window.contentView?.needsDisplay = true
        }
        if window.alphaValue != 1 || !window.isVisible {
            window.alphaValue = 1
            window.orderFrontRegardless()
        }
    }

    @MainActor
    private func showOverlay(x: Int, y: Int, width: Int, height: Int) async throws -> Bool {
        let generation = presentationGeneration.begin()
        removeOverlayWindows()
        await RecordingCaptureWindowRegistry.shared.waitForCaptureStartIfNeeded()
        guard presentationGeneration.isCurrent(generation) else { return false }
        
        let mainScreenHeight = NSScreen.screens.first?.frame.height ?? 0
        let cocoaY = mainScreenHeight - CGFloat(y) - CGFloat(height)
        
        let globalRecordingRect = NSRect(
            x: CGFloat(x),
            y: cocoaY,
            width: CGFloat(width),
            height: CGFloat(height)
        )
        
        var newWindows: [RecordingOverlayWindow] = []
        var newOverlayViews: [NSScreen: RecordingOverlayView] = [:]
        for screen in NSScreen.screens {
            let window = RecordingOverlayWindow(
                contentRect: screen.frame,
                styleMask: .borderless,
                backing: .buffered,
                defer: false
            )
            
            window.level = .screenSaver
            window.isOpaque = false
            window.backgroundColor = .clear
            window.ignoresMouseEvents = true
            window.hasShadow = false
            window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
            
            let overlayView = RecordingOverlayView(frame: screen.frame)
            
            let localRect = NSRect(
                x: globalRecordingRect.origin.x - screen.frame.origin.x,
                y: globalRecordingRect.origin.y - screen.frame.origin.y,
                width: globalRecordingRect.width,
                height: globalRecordingRect.height
            )
            
            if screen.frame.intersects(globalRecordingRect) {
                overlayView.updateRecordingRect(localRect)
            } else {
                overlayView.dimEntireScreen()
            }
            
            window.contentView = overlayView
            window.alphaValue = 0
            newWindows.append(window)
            newOverlayViews[screen] = overlayView
            window.orderFrontRegardless()
            guard RecordingCaptureWindowRegistry.shared.register(window) != nil else {
                for createdWindow in newWindows {
                    createdWindow.orderOut(nil)
                    RecordingCaptureWindowRegistry.shared.unregister(createdWindow)
                }
                throw RecordingCaptureFilterError.unavailableWindowNumber
            }
        }
        windows = newWindows
        overlayViews = newOverlayViews

        do {
            try await RecordingCaptureWindowRegistry.shared.waitForWindowPresentation()
        } catch {
            guard presentationGeneration.isCurrent(generation), windows.elementsEqual(newWindows, by: { $0 === $1 }) else { return false }
            removeOverlayWindows()
            try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
            throw error
        }
        guard presentationGeneration.isCurrent(generation), windows.elementsEqual(newWindows, by: { $0 === $1 }) else {
            if !windows.elementsEqual(newWindows, by: { $0 === $1 }) {
                for window in newWindows {
                    window.orderOut(nil)
                    RecordingCaptureWindowRegistry.shared.unregister(window)
                }
                try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
            }
            return false
        }
        for window in newWindows {
            window.alphaValue = 1
            window.orderFrontRegardless()
        }
        return true
    }
    
    @MainActor
    private func hideOverlay() {
        presentationGeneration.cancel()
        removeOverlayWindows()
    }

    @MainActor
    private func removeOverlayWindows() {
        for window in windows { window.alphaValue = 0 }
        highlightWindow?.alphaValue = 0
        if !windows.isEmpty || highlightWindow != nil { CATransaction.flush() }
        for window in windows {
            window.orderOut(nil)
            RecordingCaptureWindowRegistry.shared.unregister(window)
        }
        windows.removeAll()
        overlayViews.removeAll()

        highlightTimer?.invalidate()
        highlightTimer = nil
        highlightWindow?.orderOut(nil)
        if let highlightWindow {
            RecordingCaptureWindowRegistry.shared.unregister(highlightWindow)
        }
        highlightWindow = nil
        highlightTarget = nil
    }

    #if RECORDING_OUTLINE_TEST
    private(set) var testHideReplies: [String] = []

    @MainActor
    var testHighlightWindow: NSWindow? { highlightWindow }

    @MainActor
    var testIsTrackingHighlight: Bool { highlightTimer?.isValid == true }

    @MainActor
    var testHighlightUpdates: (frames: Int, orders: Int)? {
        highlightWindow.map { ($0.testFrameUpdates, $0.testFrontOrders) }
    }

    @MainActor
    var testOverlayWindows: [NSWindow] { windows }

    @MainActor
    func testShowWindow(_ windowID: CGWindowID) async throws -> Bool {
        try await showWindowHighlight(windowID: windowID, color: .systemPurple)
    }

    @MainActor
    func testFollowHighlight() { followHighlight() }

    @MainActor
    func testShowArea(x: Int, y: Int, width: Int, height: Int) async throws -> Bool {
        try await showOverlay(x: x, y: y, width: width, height: height)
    }

    @MainActor
    func testHideOverlay() async {
        hideOverlay()
        try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
    }
    #endif
}

private class RecordingOverlayWindow: NSWindow {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }

    #if RECORDING_OUTLINE_TEST
    private(set) var testFrameUpdates = 0
    private(set) var testFrontOrders = 0

    override func setFrame(_ frameRect: NSRect, display flag: Bool) {
        testFrameUpdates += 1
        super.setFrame(frameRect, display: flag)
    }

    override func orderFrontRegardless() {
        testFrontOrders += 1
        super.orderFrontRegardless()
    }
    #endif
}

private class RecordingHighlightView: NSView {
    private let thickness: CGFloat
    private let radius: CGFloat
    private let color: NSColor

    init(frame frameRect: NSRect, thickness: CGFloat, radius: CGFloat, color: NSColor) {
        self.thickness = thickness
        self.radius = radius
        self.color = color
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError()
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        let ring = bounds.insetBy(dx: thickness / 2, dy: thickness / 2)
        guard ring.width > 0, ring.height > 0 else { return }

        let border = NSBezierPath(roundedRect: ring, xRadius: radius, yRadius: radius)
        border.lineWidth = thickness
        color.setStroke()
        border.stroke()
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        return nil
    }
}

private class RecordingOverlayView: NSView {
    var recordingRect: NSRect = .zero
    private var dimsEntireScreen = false
    
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
    }
    
    func updateRecordingRect(_ rect: NSRect) {
        dimsEntireScreen = false
        recordingRect = rect
        needsDisplay = true
    }

    func dimEntireScreen() {
        dimsEntireScreen = true
        recordingRect = .zero
        needsDisplay = true
    }
    
    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        if dimsEntireScreen {
            NSColor.black.withAlphaComponent(0.5).setFill()
            bounds.fill()
            return
        }
        
        guard recordingRect.width > 0 && recordingRect.height > 0 else { return }
        
        NSColor.black.withAlphaComponent(0.5).setFill()
        
        let topRect = NSRect(
            x: 0,
            y: recordingRect.maxY,
            width: bounds.width,
            height: bounds.height - recordingRect.maxY
        )
        topRect.fill()
        
        let bottomRect = NSRect(
            x: 0,
            y: 0,
            width: bounds.width,
            height: recordingRect.minY
        )
        bottomRect.fill()
        
        let leftRect = NSRect(
            x: 0,
            y: recordingRect.minY,
            width: recordingRect.minX,
            height: recordingRect.height
        )
        leftRect.fill()
        
        let rightRect = NSRect(
            x: recordingRect.maxX,
            y: recordingRect.minY,
            width: bounds.width - recordingRect.maxX,
            height: recordingRect.height
        )
        rightRect.fill()
    }
    
    override func hitTest(_ point: NSPoint) -> NSView? {
        return nil
    }
}
