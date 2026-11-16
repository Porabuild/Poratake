import CoreGraphics
import Foundation

func screenshotNeedsWindowListCapture(rect: CGRect, displayBounds: [CGRect]) -> Bool {
    !displayBounds.contains { $0.contains(rect) }
}

func screenshotWindowIDs(
    from windows: [[String: Any]],
    excluding excludedWindowIDs: Set<CGWindowID>
) -> [CGWindowID]? {
    var result: [CGWindowID] = []
    for window in windows {
        guard let value = window[kCGWindowNumber as String] as? Int else { return nil }
        let id = value < 0
            ? Int32(exactly: value).map { CGWindowID(bitPattern: $0) }
            : CGWindowID(exactly: value)
        guard let id = id, id != kCGNullWindowID else {
            return nil
        }
        if !excludedWindowIDs.contains(id) { result.append(id) }
    }
    return result.isEmpty ? nil : result
}

func unresolvedVisibleScreenshotExclusions(
    requested: Set<CGWindowID>,
    available: Set<CGWindowID>,
    onScreen: Set<CGWindowID>
) -> Set<CGWindowID> {
    requested.subtracting(available).intersection(onScreen)
}

struct ScreenshotCaptureGeometry {
    let sourceRect: CGRect
    let width: Int
    let height: Int

    init?(rect: CGRect, displayBounds: CGRect, scale: CGFloat) {
        guard rect.width > 0, rect.height > 0,
              displayBounds.contains(rect), scale.isFinite, scale > 0 else { return nil }
        let width = (rect.width * scale).rounded()
        let height = (rect.height * scale).rounded()
        guard width.isFinite, height.isFinite, width >= 1, height >= 1,
              width < CGFloat(Int.max), height < CGFloat(Int.max) else { return nil }
        self.sourceRect = CGRect(x: rect.minX - displayBounds.minX,
                                 y: rect.minY - displayBounds.minY,
                                 width: rect.width,
                                 height: rect.height)
        self.width = Int(width)
        self.height = Int(height)
    }
}
