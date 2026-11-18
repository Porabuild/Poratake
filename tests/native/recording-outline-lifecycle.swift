import AppKit
import CoreGraphics
import Foundation

@main
struct RecordingOutlineLifecycleTest {
    @MainActor
    static func waitFor(_ message: String, condition: () -> Bool) async throws {
        let deadline = Date(timeIntervalSinceNow: 5)
        while !condition() {
            guard Date() < deadline else { fatalError(message) }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
    }

    @MainActor
    static func bounds(_ windowID: CGWindowID) -> CGRect? {
        guard let entries = CGWindowListCopyWindowInfo(.optionIncludingWindow, windowID) as? [[String: Any]],
              entries.first?[kCGWindowIsOnscreen as String] as? Bool == true,
              let dictionary = entries.first?[kCGWindowBounds as String] as? [String: Any] else { return nil }
        return CGRect(dictionaryRepresentation: dictionary as CFDictionary)
    }

    @MainActor
    static func onScreenAlpha(_ windowID: CGWindowID) -> Double? {
        guard let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] else {
            fatalError("cannot inspect on-screen windows")
        }
        return windows.first { ($0[kCGWindowNumber as String] as? UInt32) == windowID }?[kCGWindowAlpha as String] as? Double
    }

    @MainActor
    static func expectedFrame(target: CGRect) -> CGRect {
        let primaryHeight = NSScreen.screens.first!.frame.height
        return CGRect(x: target.minX - 2, y: primaryHeight - target.maxY - 2,
                      width: target.width + 4, height: target.height + 4)
    }

    @MainActor
    static func run() async throws {
        guard NSScreen.screens.first != nil else {
            print("SKIP recording outline lifecycle: no AppKit display")
            return
        }
        let module = RecordingOverlayModule()
        let screen = NSScreen.screens.first!
        if NSScreen.screens.count == 1 {
            let areaShown = try await module.testShowArea(x: 0, y: 0,
                width: Int(screen.frame.width), height: Int(screen.frame.height))
            precondition(areaShown && module.testOverlayWindows.count == 1)
            let area = module.testOverlayWindows[0]
            let areaID = CGWindowID(area.windowNumber)
            try await waitFor("area overlay was not visible to the window server") {
                onScreenAlpha(areaID) == 1
            }
            await module.testHideOverlay()
            precondition(onScreenAlpha(areaID).map { $0 == 0 } ?? true,
                         "area overlay remained opaque after cleanup")
            precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs().isEmpty)
            print("area cleanup: overlay absent or transparent in window server")
        }
        let preview = NSPanel(contentRect: NSRect(x: 100, y: 100, width: 100, height: 100),
                              styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        preview.orderFrontRegardless()
        let cameraPreview = CameraPreviewModule()
        precondition(cameraPreview.testInstallPanel(preview))
        let previewID = CGWindowID(preview.windowNumber)
        try await waitFor("camera preview stand-in was not visible to the window server") {
            onScreenAlpha(previewID) == 1
        }
        await cameraPreview.testHidePanel()
        precondition(onScreenAlpha(previewID).map { $0 == 0 } ?? true,
                     "camera preview remained opaque after cleanup")
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs().isEmpty)
        print("camera preview cleanup: panel absent or transparent in window server")
        let freeze = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 100, height: 100),
                              styleMask: .borderless, backing: .buffered, defer: false)
        freeze.animationBehavior = .none
        freeze.orderFrontRegardless()
        let frozenScreen = FreezeScreenModule()
        frozenScreen.testInstallOverlay(freeze)
        let freezeID = CGWindowID(freeze.windowNumber)
        try await waitFor("freeze overlay stand-in was not visible to the window server") {
            onScreenAlpha(freezeID) == 1
        }
        frozenScreen.testRemoveOverlays()
        precondition(onScreenAlpha(freezeID).map { $0 == 0 } ?? true,
                     "freeze overlay remained opaque after cleanup")
        print("freeze cleanup: overlay absent or transparent in window server")
        let target = NSWindow(contentRect: NSRect(x: 120, y: 120, width: 240, height: 140),
                              styleMask: [.titled], backing: .buffered, defer: false)
        target.title = "Poratake generated outline test"
        target.isReleasedWhenClosed = false
        target.orderFrontRegardless()
        let windowID = CGWindowID(target.windowNumber)
        try await waitFor("generated target did not appear") { bounds(windowID) != nil }
        let shown = try await module.testShowWindow(windowID)
        precondition(shown)
        guard let highlight = module.testHighlightWindow, let initialBounds = bounds(windowID) else {
            fatalError("outline not created")
        }
        precondition(highlight.alphaValue == 1)
        let outlineID = CGWindowID(highlight.windowNumber)
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs() == [outlineID])
        precondition(highlight.frame.equalTo(expectedFrame(target: initialBounds)))
        print("visible target: outline matches bounds")

        guard let updates = module.testHighlightUpdates else { fatalError("outline updates missing") }
        module.testFollowHighlight()
        guard let stationaryUpdates = module.testHighlightUpdates else { fatalError("outline updates missing") }
        precondition(stationaryUpdates.frames == updates.frames && stationaryUpdates.orders == updates.orders,
                     "stationary target reset the outline frame or order")
        print("stationary target: no frame or ordering update")

        try await waitFor("outline was not visible to the window server") { onScreenAlpha(outlineID) == 1 }
        await module.testHideOverlay()
        precondition(onScreenAlpha(outlineID).map { $0 == 0 } ?? true,
                     "outline remained opaque after cleanup")
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs().isEmpty)
        print("outline cleanup: overlay absent or transparent in window server")
        let shownAgain = try await module.testShowWindow(windowID)
        precondition(shownAgain)
        guard let movedHighlight = module.testHighlightWindow else { fatalError("outline not restored") }
        let movedOutlineID = CGWindowID(movedHighlight.windowNumber)

        target.setFrameOrigin(NSPoint(x: 420, y: 220))
        try await waitFor("outline did not follow target movement") {
            guard let current = bounds(windowID), current.minX == 420 else { return false }
            return movedHighlight.frame.equalTo(expectedFrame(target: current))
        }
        print("moved target: outline follows")

        target.setFrame(NSRect(x: 420, y: 220, width: 310, height: 186), display: true)
        try await waitFor("outline did not follow target resize") {
            guard let current = bounds(windowID), current.width == 310, current.height == 186 else { return false }
            return movedHighlight.frame.equalTo(expectedFrame(target: current))
                && movedHighlight.contentView?.frame.size == movedHighlight.frame.size
        }
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs() == [movedOutlineID])
        print("resized target: outline follows with the same exclusion ID")

        let frameBeforeRestore = movedHighlight.frame
        target.orderOut(nil)
        try await waitFor("outline did not hide with target") {
            bounds(windowID) == nil && movedHighlight.alphaValue == 0
        }
        precondition(module.testHighlightWindow === movedHighlight && movedHighlight.alphaValue == 0,
                     "temporarily hidden target lost its outline tracker")
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs() == [movedOutlineID])
        print("hidden target: outline hidden, tracker retained")

        target.orderFrontRegardless()
        try await waitFor("outline did not restore at an unchanged frame") {
            movedHighlight.alphaValue == 1 && movedHighlight.frame == frameBeforeRestore
        }
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs() == [movedOutlineID])
        print("restored stationary target: same frame and exclusion ID")
        target.orderOut(nil)
        try await waitFor("outline did not hide before the second restore") {
            bounds(windowID) == nil && movedHighlight.alphaValue == 0
        }

        target.setFrameOrigin(NSPoint(x: 620, y: 320))
        target.orderFrontRegardless()
        try await waitFor("outline did not follow restored target") {
            guard let current = bounds(windowID), current.minX == 620 else { return false }
            return movedHighlight.alphaValue == 1 && movedHighlight.frame.equalTo(expectedFrame(target: current))
        }
        precondition(module.testHighlightWindow === movedHighlight && movedHighlight.alphaValue == 1)
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs() == [movedOutlineID])
        print("restored target: outline follows new bounds")

        target.close()
        try await waitFor("outline did not hide with closed target") {
            bounds(windowID) == nil && movedHighlight.alphaValue == 0
        }
        precondition(movedHighlight.alphaValue == 0)
        await module.testHideOverlay()
        precondition(module.testHighlightWindow == nil)
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs().isEmpty)
        print("closed target and explicit cleanup: outline hidden and unregistered")
    }

    static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        Task { @MainActor in
            do {
                try await run()
                exit(EXIT_SUCCESS)
            } catch {
                fputs("recording outline lifecycle failed: \(error)\n", stderr)
                exit(EXIT_FAILURE)
            }
        }
        app.run()
    }
}
