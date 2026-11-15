import Cocoa
import Foundation

class RecordingOverlayModule: Module {
    let name = DaemonContract.RecordingOverlay.module
    private var windows: [RecordingOverlayWindow] = []
    private var overlayViews: [NSScreen: RecordingOverlayView] = [:]
    private var highlightWindow: RecordingOverlayWindow?
    private var highlightTimer: Timer?
    private var highlightWindowID: CGWindowID?
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
        guard let windowId = params?["windowId"]?.int() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "showWindow requires windowId")
            return
        }

        let color = themeColor(params?["color"]?.string()) ?? Self.highlightFallback

        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            Task { @MainActor in
                do {
                    let visible = try await self.showWindowHighlight(
                        windowID: CGWindowID(windowId),
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
                await self.hideOverlay()
                self.respond(id: requestId, result: ["visible": false])
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

        guard windowBounds(of: windowID) != nil else { return false }

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
        highlightWindowID = windowID
        window.orderFrontRegardless()
        guard RecordingCaptureWindowRegistry.shared.register(window) != nil else {
            highlightWindow = nil
            highlightWindowID = nil
            window.orderOut(nil)
            throw RecordingCaptureFilterError.unavailableWindowNumber
        }
        do {
            try await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
        } catch {
            guard presentationGeneration.isCurrent(generation), highlightWindow === window else { return false }
            RecordingCaptureWindowRegistry.shared.unregister(window)
            highlightWindow = nil
            highlightWindowID = nil
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
        window.alphaValue = 1
        followHighlight()

        // AppKit has no cross-application window-move notification without the
        // accessibility permission, so the frame is sampled instead.
        highlightTimer = Timer.scheduledTimer(
            withTimeInterval: Self.highlightInterval,
            repeats: true
        ) { [weak self] _ in
            Task { @MainActor in
                self?.followHighlight()
            }
        }

        return true
    }

    @MainActor
    private func followHighlight() {
        guard let window = highlightWindow, let windowID = highlightWindowID else { return }

        guard let bounds = windowBounds(of: windowID) else {
            window.orderOut(nil)
            RecordingCaptureWindowRegistry.shared.unregister(window)
            highlightWindow = nil
            highlightWindowID = nil
            highlightTimer?.invalidate()
            highlightTimer = nil
            Task { @MainActor in
                try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
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

        window.setFrame(frame, display: true)
        window.contentView?.frame = NSRect(origin: .zero, size: frame.size)
        window.contentView?.needsDisplay = true
        window.orderFrontRegardless()
    }

    @MainActor
    private func windowBounds(of windowID: CGWindowID) -> CGRect? {
        guard let windows = CGWindowListCopyWindowInfo(.optionIncludingWindow, windowID)
                as? [[String: Any]],
              windows.first?[kCGWindowIsOnscreen as String] as? Bool == true,
              let boundsDict = windows.first?[kCGWindowBounds as String] as? [String: Any],
              let rect = CGRect(dictionaryRepresentation: boundsDict as CFDictionary),
              rect.width > 0, rect.height > 0
        else {
            return nil
        }

        return rect
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
            try await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
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
    private func hideOverlay() async {
        presentationGeneration.cancel()
        removeOverlayWindows()
        try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
    }

    @MainActor
    private func removeOverlayWindows() {
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
        highlightWindowID = nil
    }
}

private class RecordingOverlayWindow: NSWindow {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
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
