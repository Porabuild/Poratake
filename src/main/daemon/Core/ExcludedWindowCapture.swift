import Cocoa
import ScreenCaptureKit

enum ScreenshotFrameError: LocalizedError {
    case timedOut
    case missingImage

    var errorDescription: String? {
        switch self {
        case .timedOut: "ScreenCaptureKit did not return a screenshot in time"
        case .missingImage: "ScreenCaptureKit did not return a screenshot image"
        }
    }
}

@MainActor
func awaitScreenshotFrame<Frame: Sendable>(
    timeout: Duration = .seconds(15),
    start: (@escaping @Sendable (Frame?, Error?) -> Void) -> Void
) async throws -> Frame {
    try Task.checkCancellation()
    let (frames, result) = AsyncThrowingStream<Frame, Error>.makeStream(bufferingPolicy: .bufferingOldest(1))
    let deadline = Task {
        do {
            try await Task.sleep(for: timeout)
        } catch {
            return
        }
        result.finish(throwing: ScreenshotFrameError.timedOut)
    }
    defer {
        deadline.cancel()
        result.finish()
    }
    start { frame, error in
        if let error {
            result.finish(throwing: error)
            return
        }
        guard let frame else {
            result.finish(throwing: ScreenshotFrameError.missingImage)
            return
        }
        result.yield(frame)
        result.finish()
    }
    for try await frame in frames { return frame }
    throw CancellationError()
}

@MainActor
func coordinateAreaCapture<T>(
    registry: RecordingCaptureWindowRegistry,
    excludedWindowIDs: Set<CGWindowID>,
    unfiltered: @MainActor () async throws -> T,
    filtered: @MainActor (Set<CGWindowID>) async throws -> T
) async rethrows -> T {
    let ids = excludedWindowIDs.union(registry.beginScreenshot())
    defer { registry.finishScreenshot() }
    if ids.isEmpty { return try await unfiltered() }
    return try await filtered(ids)
}

@available(macOS 14.0, *)
@MainActor
func captureAreaExcludingWindows(
    _ rect: CGRect,
    displayID: CGDirectDisplayID?,
    excludedWindowIDs: Set<CGWindowID>,
    requireAllWindows: Bool = false
) async throws -> CGImage {
    let registry = RecordingCaptureWindowRegistry.shared
    let excludedWindowIDs = excludedWindowIDs.union(registry.beginScreenshot())
    defer { registry.finishScreenshot() }
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: !requireAllWindows)
    let display = content.displays.first { candidate in
        if let displayID = displayID { return candidate.displayID == displayID }
        return CGDisplayBounds(candidate.displayID).contains(CGPoint(x: rect.midX, y: rect.midY))
    }
    guard let display = display else {
        throw NSError(domain: "Screenshot", code: 1, userInfo: [NSLocalizedDescriptionKey: "The selected display is no longer available"])
    }
    let windows = content.windows.filter { excludedWindowIDs.contains($0.windowID) }
    let availableWindowIDs = Set(windows.map(\.windowID))
    if requireAllWindows && !excludedWindowIDs.isSubset(of: availableWindowIDs) {
        throw NSError(domain: "Screenshot", code: 4, userInfo: [NSLocalizedDescriptionKey: "Could not exclude capture controls"])
    }
    if !excludedWindowIDs.isSubset(of: availableWindowIDs) {
        guard let windowInfo = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] else {
            throw NSError(domain: "Screenshot", code: 3, userInfo: [NSLocalizedDescriptionKey: "Could not verify the capture controls are excluded"])
        }
        let windowIDs = windowInfo.compactMap { $0[kCGWindowNumber as String] as? CGWindowID }
        guard windowIDs.count == windowInfo.count else {
            throw NSError(domain: "Screenshot", code: 3, userInfo: [NSLocalizedDescriptionKey: "Could not verify the capture controls are excluded"])
        }
        let unresolved = unresolvedVisibleScreenshotExclusions(
            requested: excludedWindowIDs,
            available: availableWindowIDs,
            onScreen: Set(windowIDs)
        )
        guard unresolved.isEmpty else {
            throw NSError(domain: "Screenshot", code: 4, userInfo: [NSLocalizedDescriptionKey: "Could not exclude visible capture controls"])
        }
    }
    let filter = SCContentFilter(display: display, excludingWindows: windows)
    let bounds = CGDisplayBounds(display.displayID)
    guard let geometry = ScreenshotCaptureGeometry(rect: rect, displayBounds: bounds, scale: CGFloat(filter.pointPixelScale)) else {
        throw NSError(domain: "Screenshot", code: 2, userInfo: [NSLocalizedDescriptionKey: "The selected area is outside the display"])
    }
    let config = SCStreamConfiguration()
    config.sourceRect = geometry.sourceRect
    config.width = geometry.width
    config.height = geometry.height
    config.showsCursor = false
    config.capturesAudio = false
    config.captureResolution = .best
    return try await awaitScreenshotFrame { completion in
        SCScreenshotManager.captureImage(contentFilter: filter, configuration: config, completionHandler: completion)
    }
}
