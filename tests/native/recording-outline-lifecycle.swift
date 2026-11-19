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

    static func verifyCameraPreviewPlacement() {
        let primary = NSRect(x: 0, y: 0, width: 1728, height: 1117)
        let primaryWorkArea = NSRect(x: 30, y: 44, width: 1698, height: 1040)
        let secondary = NSRect(x: -960, y: 1117, width: 960, height: 600)
        let secondaryWorkArea = NSRect(x: -940, y: 1137, width: 900, height: 560)
        let size = NSSize(width: 270, height: 270)
        let displays = [
            (frame: primary, visibleFrame: primaryWorkArea),
            (frame: secondary, visibleFrame: secondaryWorkArea),
        ]
        func placement(_ x: Int?, _ y: Int?) -> NSPoint {
            let origin = x.flatMap { x in
                y.map { y in NSPoint(x: CGFloat(x), y: primary.height - CGFloat(y) - size.height) }
            }
            return CameraPreviewModule.fittedPanelOrigin(
                requestedOrigin: origin, size: size, displays: displays, fallback: primaryWorkArea
            )
        }
        precondition(placement(800, 200) == NSPoint(x: 800, y: 647))
        precondition(placement(-950, -1) == NSPoint(x: -940, y: 1137))
        precondition(placement(-41, -590) == NSPoint(x: -310, y: 1427))
        let overLeftEdge = placement(-14, 200)
        let overTopEdge = placement(60, -45)
        print("camera partially off display: left=\(overLeftEdge), top=\(overTopEdge)")
        fflush(stdout)
        precondition(overLeftEdge == NSPoint(x: 30, y: 647))
        precondition(overTopEdge == NSPoint(x: 60, y: 814))
        precondition(placement(-45, 0) == NSPoint(x: 30, y: 814))
        let right = (
            frame: NSRect(x: 1728, y: 0, width: 850, height: 1117),
            visibleFrame: NSRect(x: 1750, y: 44, width: 820, height: 1040)
        )
        let sharedEdge = CameraPreviewModule.fittedPanelOrigin(
            requestedOrigin: NSPoint(x: 1690, y: primary.height + 45 - size.height), size: size,
            displays: displays + [right], fallback: primaryWorkArea
        )
        precondition(sharedEdge == NSPoint(x: 1750, y: 814))
        precondition(placement(3500, -450) == placement(nil, nil))
        precondition(placement(nil, nil) == NSPoint(x: 1426, y: 76))
        print("camera preview geometry: dock insets, negative origins and disconnected display")
    }

    static func verifyCameraSelection() {
        struct Device {
            let id: String
            let name: String
        }
        let cameras = [
            Device(id: "first", name: "USB Camera"),
            Device(id: "second", name: "USB Camera"),
            Device(id: "third", name: "Desk Camera"),
        ]
        func match(_ id: String?, _ name: String?) -> String? {
            CameraDeviceLookup.match(cameras, deviceId: id, deviceName: name, id: \.id, name: \.name)?.id
        }
        precondition(match("second", "USB Camera") == "second", "A shared camera name overrode the selected native ID")
        precondition(match("third", "USB Camera") == "third")
        precondition(match("legacy-browser-id", "Desk Camera") == "third")
        precondition(match(nil, "External Desk Camera") == "third")
        precondition(match(nil, "Unavailable camera") == nil)
        print("camera selection: native IDs win and saved names remain usable")
    }

    @MainActor
    static func verifyCameraPreviewStartup() async throws {
        let camera = CameraPreviewModule()
        func show(_ name: String, requestId: String) {
            camera.handle(method: "show", params: ["deviceName": AnyCodable(name)], requestId: requestId)
        }

        show("generated camera", requestId: "initial-camera")
        try await waitFor("initial camera preview never revealed") {
            camera.testShowResults["initial-camera"] == true
        }
        precondition(camera.testStartup.starts == ["generated camera"],
                     "initial camera preview started more than once: \(camera.testStartup.starts)")
        let protectionOrder = [true, false, true]
        for (index, enabled) in protectionOrder.enumerated() {
            camera.handle(
                method: "setContentProtection", params: ["enabled": AnyCodable(enabled)],
                requestId: "camera-protection-\(index)"
            )
        }
        try await waitFor("camera protection requests did not finish") {
            camera.testProtectionReplies.count == protectionOrder.count
        }
        precondition(camera.testProtectionReplies == protectionOrder)
        precondition(camera.testStartup.panel?.sharingType == NSWindow.SharingType.none)
        show("generated camera", requestId: "repeated-camera")
        try await waitFor("repeated camera preview request did not finish") {
            camera.testShowResults["repeated-camera"] == true
        }
        precondition(camera.testStartup.starts == ["generated camera"], "unchanged preview restarted the camera")
        camera.testReportStartupFailure()
        try await waitFor("Camera startup error did not appear") {
            camera.testCameraError == "Could not start camera preview"
        }
        show("generated camera", requestId: "retry-camera")
        try await waitFor("Same-source preview retry did not finish") {
            camera.testShowResults["retry-camera"] == true
        }
        print("camera same-source retry after startup failure: starts=\(camera.testStartup.starts.count)")
        fflush(stdout)
        precondition(camera.testStartup.starts == ["generated camera", "generated camera"])
        precondition(camera.testCameraError == nil)
        camera.testPostRuntimeError()
        try await waitFor("Camera runtime error did not appear") {
            camera.testCameraError == "Camera preview stopped"
        }
        show("generated camera", requestId: "retry-runtime-camera")
        try await waitFor("Same-source retry after a runtime error did not finish") {
            camera.testShowResults["retry-runtime-camera"] == true
        }
        precondition(camera.testStartup.starts == Array(repeating: "generated camera", count: 3))
        precondition(camera.testCameraError == nil)
        print("camera runtime error: same source retried")
        await camera.testHidePanel()

        let registry = RecordingCaptureWindowRegistry.shared
        _ = registry.beginScreenshot()
        show("superseded camera", requestId: "camera-superseded")
        try await waitFor("camera preview did not stage a hidden panel") {
            camera.testStartup.panel?.alphaValue == 0
        }
        let generation = camera.testStartup.generation
        show("latest camera", requestId: "camera-intermediate")
        try await waitFor("intermediate camera show did not reach the presentation barrier") {
            camera.testStartup.generation == generation + 1
        }
        show("latest camera", requestId: "camera-latest")
        try await waitFor("repeated camera shows did not reach the presentation barrier") {
            camera.testStartup.generation == generation + 2
        }
        precondition(camera.testStartup.starts.isEmpty, "camera started before the preview could be revealed")
        registry.finishScreenshot()
        try await waitFor("camera preview requests did not settle after the barrier") {
            camera.testShowResults["camera-latest"] == true
                && camera.testShowResults["camera-superseded"] == false
                && camera.testShowResults["camera-intermediate"] == false
        }
        precondition(camera.testStartup.starts == ["latest camera"],
                     "camera preview used a superseded configuration: \(camera.testStartup.starts)")
        await camera.testHidePanel()
        precondition(registry.currentWindowIDs().isEmpty)
        print("camera preview startup: one accepted camera request after protection")

        var blockedFilter: CheckedContinuation<Void, Never>?
        registry.setFilterUpdater { windowIDs in
            guard !windowIDs.isEmpty else { return }
            await withCheckedContinuation { blockedFilter = $0 }
            throw RecordingCaptureFilterError.unavailableWindows(windowIDs)
        }
        show("generated retry camera", requestId: "camera-failing-first")
        try await waitFor("camera preview did not enter capture filter setup") {
            blockedFilter != nil
        }
        guard let rejectFilter = blockedFilter, let failedPanel = camera.testStartup.panel else {
            fatalError("staged camera panel or pending filter missing")
        }
        let failedID = CGWindowID(failedPanel.windowNumber)
        precondition(registry.currentWindowIDs() == [failedID])
        precondition(failedPanel.alphaValue == 0 && failedPanel.ignoresMouseEvents)
        let failedGeneration = camera.testStartup.generation
        show("generated retry camera", requestId: "camera-failing-current")
        try await waitFor("replacement show did not join the pending camera presentation") {
            camera.testStartup.generation == failedGeneration + 1
        }
        rejectFilter.resume()
        try await waitFor("camera preview did not report capture filter failure") {
            camera.testShowResults["camera-failing-first"] == false
                && camera.testFailedShows.contains("camera-failing-current")
        }
        precondition(camera.testStartup.panel == nil && registry.currentWindowIDs().isEmpty)
        precondition(camera.testStartedCameras(in: failedPanel) == [])
        precondition(onScreenAlpha(failedID).map { $0 == 0 } ?? true)
        registry.setFilterUpdater(nil)
        print("camera preview filter failure: hidden panel removed without startup")

        show("generated retry camera", requestId: "camera-retry")
        try await waitFor("camera preview retry did not reveal") {
            camera.testShowResults["camera-retry"] == true
        }
        precondition(camera.testStartup.panel !== failedPanel && camera.testStartup.starts == ["generated retry camera"])
        await camera.testHidePanel()
        precondition(registry.currentWindowIDs().isEmpty)
        print("camera preview filter retry: fresh panel and one startup")
    }

    @MainActor
    static func verifyCameraPreviewError() {
        for name in [NSAppearance.Name.aqua, .darkAqua] {
            let view = CameraPreviewModule.testErrorView(messages: ["No camera found", "Camera access denied"])
            view.appearance = NSAppearance(named: name)
            view.layoutSubtreeIfNeeded()
            let container = view.subviews.last!
            let labels = container.subviews.compactMap { $0 as? NSTextField }
            guard let label = labels.last else { preconditionFailure("Camera error label missing") }
            let bitmap = label.bitmapImageRepForCachingDisplay(in: label.bounds)!
            label.cacheDisplay(in: label.bounds, to: bitmap)
            let ink = (0..<bitmap.pixelsHigh).flatMap { y in
                (0..<bitmap.pixelsWide).compactMap { x in
                    bitmap.colorAt(x: x, y: y)?.usingColorSpace(.sRGB)
                }
            }.filter { $0.alphaComponent > 0.6 }
            precondition(ink.contains { $0.redComponent > 0.8 && $0.greenComponent > 0.8 && $0.blueComponent > 0.8 },
                         "Camera error text is too dark against its black backdrop in \(name)")
            precondition(labels.count == 1 && label.stringValue == "Camera access denied")
            precondition(abs(label.frame.midY - container.bounds.midY) <= 1)
            precondition(label.frame.height <= ceil(label.fittingSize.height) + 1)
            let retry = CameraPreviewModule.testErrorView(messages: ["No camera found"], retry: true)
            precondition(retry.subviews.last?.subviews.isEmpty == true)
        }
        print("camera preview error: centered, legible in both appearances, and cleared on retry")
    }

    @MainActor
    static func verifyRecordingOverlayHideReply() async throws {
        guard let screen = NSScreen.screens.first, NSScreen.screens.count == 1 else {
            print("SKIP recording overlay hide reply: needs a single AppKit display")
            return
        }
        let module = RecordingOverlayModule()
        let shown = try await module.testShowArea(x: 0, y: 0,
            width: Int(screen.frame.width), height: Int(screen.frame.height))
        precondition(shown && module.testOverlayWindows.count == 1)
        let window = module.testOverlayWindows[0]
        let registry = RecordingCaptureWindowRegistry.shared
        var releaseRefresh: CheckedContinuation<Void, Never>?
        registry.setFilterUpdater { ids in
            precondition(ids.isEmpty)
            await withCheckedContinuation { releaseRefresh = $0 }
        }
        module.handle(method: "hide", params: nil, requestId: "overlay-hide")
        try await waitFor("hide did not reach capture filter refresh") { releaseRefresh != nil }
        precondition(module.testOverlayWindows.isEmpty && registry.currentWindowIDs().isEmpty)
        precondition(window.alphaValue == 0 && !window.isVisible)
        precondition(module.testHideReplies == ["overlay-hide"],
                     "Overlay hide reply waited for capture filter refresh")
        guard let releaseRefresh else { preconditionFailure("no filter refresh to release") }
        releaseRefresh.resume()
        registry.setFilterUpdater(nil)
        try await registry.updateFilterIfNeeded()
        print("recording overlay hide replied before the held filter refresh finished")
    }

    @MainActor
    static func verifyCameraPreviewHideReply() async throws {
        guard NSScreen.screens.first != nil else {
            print("SKIP camera preview hide reply: no AppKit display")
            return
        }
        let camera = CameraPreviewModule()
        let registry = RecordingCaptureWindowRegistry.shared
        camera.handle(method: "show", params: ["deviceName": AnyCodable("generated original")],
                      requestId: "camera-original")
        try await waitFor("original camera preview did not reveal") {
            camera.testShowResults["camera-original"] == true
        }
        let original = camera.testStartup.panel!
        var releaseRefresh: CheckedContinuation<Void, Never>?
        var refreshedIDs: [CGWindowID] = []
        registry.setFilterUpdater { ids in
            if ids.isEmpty {
                await withCheckedContinuation { releaseRefresh = $0 }
            } else {
                refreshedIDs = ids
            }
        }
        camera.handle(method: "hide", params: nil, requestId: "camera-hide")
        try await waitFor("camera hide did not reach filter refresh") { releaseRefresh != nil }
        precondition(camera.testStartup.panel == nil && registry.currentWindowIDs().isEmpty)
        precondition(original.alphaValue == 0 && !original.isVisible)
        precondition(camera.testHideReplies == ["camera-hide"],
                     "Camera preview hide reply waited for capture filter refresh")

        camera.handle(method: "show", params: ["deviceName": AnyCodable("generated replacement")],
                      requestId: "camera-replacement")
        try await waitFor("replacement camera panel was not staged") { camera.testStartup.panel != nil }
        let replacement = camera.testStartup.panel!
        precondition(replacement !== original && replacement.alphaValue == 0)
        precondition(camera.testStartup.starts.isEmpty)
        releaseRefresh!.resume()
        try await waitFor("replacement camera did not reveal after the refresh") {
            camera.testShowResults["camera-replacement"] == true
        }
        precondition(refreshedIDs == [CGWindowID(replacement.windowNumber)])
        precondition(camera.testStartup.starts == ["generated replacement"])
        registry.setFilterUpdater(nil)
        await camera.testHidePanel()
        precondition(registry.currentWindowIDs().isEmpty)
        print("camera preview hide replied during refresh; replacement waited for protection")
    }

    @MainActor
    static func verifyCameraPreviewDeferredFit() async throws {
        guard NSScreen.screens.count == 1, let work = NSScreen.screens.first?.visibleFrame,
              work.width > 300, work.height > 300 else { return }
        let registry = RecordingCaptureWindowRegistry.shared
        for superseded in [false, true] {
            let camera = CameraPreviewModule()
            _ = registry.beginScreenshot()
            camera.handle(method: "show", params: ["deviceName": AnyCodable("generated first")],
                          requestId: "deferred-camera")
            try await waitFor("The hidden camera panel was not staged") {
                camera.testStartup.panel?.alphaValue == 0
            }
            let panel = camera.testStartup.panel!
            let id = CGWindowID(panel.windowNumber)
            let request = superseded ? "replacement-camera" : "deferred-camera"
            if superseded {
                camera.handle(method: "show", params: ["deviceName": AnyCodable("generated latest")],
                              requestId: request)
                try await waitFor("The replacement camera request did not reach the screenshot wait") {
                    camera.testStartup.generation == 2
                }
            }
            let origin = NSPoint(x: work.minX - 24, y: (work.midY - 135).rounded())
            panel.setFrameOrigin(origin)
            precondition(panel.frame.origin == origin && panel.alphaValue == 0)
            NotificationCenter.default.post(name: NSApplication.didChangeScreenParametersNotification, object: NSApp)
            registry.finishScreenshot()
            try await waitFor("The camera panel did not reveal") { camera.testShowResults[request] == true }
            print("camera deferred fit: superseded=\(superseded), workAreaFit=\(work.contains(panel.frame))")
            fflush(stdout)
            precondition(panel.frame.origin == NSPoint(x: work.minX, y: origin.y))
            precondition(camera.testStartup.panel === panel && registry.currentWindowIDs() == [id])
            await camera.testHidePanel()
            precondition(registry.currentWindowIDs().isEmpty)
        }
    }

    @MainActor
    static func verifyCameraPreviewDisplayChange() async throws {
        guard NSScreen.screens.count == 1, let work = NSScreen.screens.first?.visibleFrame,
              work.width > 300, work.height > 300 else { return }
        let origin = NSPoint(x: work.minX - 24, y: work.midY - 135)
        let frame = NSRect(origin: origin, size: NSSize(width: 270, height: 270))
        let panel = NSPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel],
                            backing: .buffered, defer: false)
        panel.orderFrontRegardless()
        panel.setFrameOrigin(origin)
        let camera = CameraPreviewModule()
        precondition(camera.testInstallPanel(panel))
        let id = CGWindowID(panel.windowNumber)
        precondition(panel.frame.origin == origin)
        NotificationCenter.default.post(name: NSApplication.didChangeScreenParametersNotification, object: NSApp)
        try await waitFor("camera preview did not refit after a display update") {
            panel.frame.minX == work.minX && panel.frame.minY == origin.y
        }
        precondition(work.contains(panel.frame))
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs() == [id])
        await camera.testHidePanel()
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs().isEmpty)
        print("camera display update: same panel and exclusion ID, clamped to the work area")
    }

    @MainActor
    static func run() async throws {
        verifyCameraSelection()
        verifyCameraPreviewPlacement()
        verifyCameraPreviewError()
        let cameraCompletions = CameraPreviewModule.testCameraCompletionOrdering()
        print("camera completion ordering: \(cameraCompletions)")
        fflush(stdout)
        precondition(cameraCompletions == [false, false, true, false])
        let queuedCameraStarts = CameraPreviewModule.testCameraStartupQueue()
        print("camera queued startup work: \(queuedCameraStarts)")
        fflush(stdout)
        precondition(queuedCameraStarts == ["A"])
        try await verifyRecordingOverlayHideReply()
        try await verifyCameraPreviewHideReply()
        guard NSScreen.screens.first != nil else {
            print("SKIP recording outline lifecycle: no AppKit display")
            return
        }
        try await verifyCameraPreviewStartup()
        try await verifyCameraPreviewDisplayChange()
        try await verifyCameraPreviewDeferredFit()
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
        let preview = NSPanel(contentRect: NSRect(x: 100, y: 100, width: 270, height: 270),
                              styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        preview.orderFrontRegardless()
        let cameraPreview = CameraPreviewModule()
        precondition(cameraPreview.testInstallPanel(preview))
        let previewID = CGWindowID(preview.windowNumber)
        try await waitFor("camera preview stand-in was not visible to the window server") {
            onScreenAlpha(previewID) == 1
        }
        let visible = screen.visibleFrame
        cameraPreview.handle(
            method: "update",
            params: ["x": AnyCodable(Int(visible.minX) + 24), "y": AnyCodable(120)],
            requestId: "camera-first-position"
        )
        cameraPreview.handle(
            method: "update",
            params: [
                "x": AnyCodable(Int(visible.maxX) - 1),
                "y": AnyCodable(Int(screen.frame.height - visible.minY) - 1),
            ],
            requestId: "camera-final-position"
        )
        try await waitFor("the latest camera position did not apply") {
            preview.frame.origin == NSPoint(x: visible.maxX - preview.frame.width, y: visible.minY)
        }
        precondition(visible.contains(preview.frame), "camera preview extended beyond the display work area")
        try await waitFor("the camera panel did not report its fitted position") {
            cameraPreview.testReportedPositions.last == NSPoint(
                x: Int(preview.frame.minX), y: Int(screen.frame.height - preview.frame.maxY)
            )
        }
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs() == [previewID])
        print("camera preview placement: edge position fits the work area")
        NotificationCenter.default.post(name: NSWindow.didMoveNotification, object: preview)
        await cameraPreview.testHidePanel()
        precondition(onScreenAlpha(previewID).map { $0 == 0 } ?? true,
                     "camera preview remained opaque after cleanup")
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs().isEmpty)
        print("camera preview cleanup: panel absent or transparent in window server")

        let replacement = NSPanel(contentRect: NSRect(x: 140, y: 140, width: 270, height: 270),
                                  styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        replacement.orderFrontRegardless()
        precondition(cameraPreview.testInstallPanel(replacement))
        precondition(cameraPreview.testInstallPanel(replacement))
        let reportCount = cameraPreview.testReportedPositions.count
        NotificationCenter.default.post(name: NSWindow.didMoveNotification, object: preview)
        NotificationCenter.default.post(name: NSWindow.didMoveNotification, object: replacement)
        try await waitFor("the replacement camera panel did not report movement") {
            cameraPreview.testReportedPositions.count > reportCount
        }
        precondition(cameraPreview.testReportedPositions.count == reportCount + 1,
                     "the closed camera panel reported a move for its replacement")
        precondition(cameraPreview.testReportedPositions.last == NSPoint(
            x: Int(replacement.frame.minX), y: Int(screen.frame.height - replacement.frame.maxY)
        ))
        await cameraPreview.testHidePanel()
        precondition(RecordingCaptureWindowRegistry.shared.currentWindowIDs().isEmpty)
        print("camera preview move tracking: only the current panel reports")

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

    static func verifyFreezePrewarm() {
        var captureAttempts = 0
        let denied = FreezeScreenModule.prewarmDisplay(hasAccess: { false }) {
            captureAttempts += 1
            return true
        }
        precondition(!denied && captureAttempts == 0)

        let warmed = FreezeScreenModule.prewarmDisplay(hasAccess: { true }) {
            captureAttempts += 1
            return true
        }
        precondition(warmed && captureAttempts == 1)

        let unavailable = FreezeScreenModule.prewarmDisplay(hasAccess: { true }) {
            captureAttempts += 1
            return false
        }
        precondition(!unavailable && captureAttempts == 2)
        print("freeze prewarm requires existing permission and reports capture availability")
    }

    static func main() {
        verifyFreezePrewarm()
        if CommandLine.arguments.contains("--freeze-prewarm") { return }
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        if CommandLine.arguments.contains("--camera-selection") {
            verifyCameraSelection()
            return
        }
        if CommandLine.arguments.contains("--camera-preview-error") {
            verifyCameraPreviewError()
            return
        }
        Task { @MainActor in
            do {
                if CommandLine.arguments.contains("--recording-overlay-hide-reply") {
                    try await verifyRecordingOverlayHideReply()
                } else if CommandLine.arguments.contains("--camera-preview-hide-reply") {
                    try await verifyCameraPreviewHideReply()
                } else {
                    try await run()
                }
                exit(EXIT_SUCCESS)
            } catch {
                fputs("recording outline lifecycle failed: \(error)\n", stderr)
                exit(EXIT_FAILURE)
            }
        }
        app.run()
    }
}
