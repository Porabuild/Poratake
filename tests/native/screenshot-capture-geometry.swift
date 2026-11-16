import CoreGraphics
import Foundation

@main
struct ScreenshotCaptureGeometryTest {
    static func main() {
        let display = CGRect(x: -1440, y: 200, width: 1440, height: 900)
        let selection = CGRect(x: -1300, y: 250, width: 300, height: 200)
        let capture = ScreenshotCaptureGeometry(rect: selection, displayBounds: display, scale: 2)
        precondition(capture?.sourceRect == CGRect(x: 140, y: 50, width: 300, height: 200))
        precondition(capture?.width == 600 && capture?.height == 400)
        let oneX = ScreenshotCaptureGeometry(rect: display, displayBounds: display, scale: 1)
        precondition(oneX?.sourceRect == CGRect(x: 0, y: 0, width: 1440, height: 900))
        precondition(oneX?.width == 1440 && oneX?.height == 900)
        let fractional = ScreenshotCaptureGeometry(
            rect: CGRect(x: -1300, y: 250, width: 301, height: 201),
            displayBounds: display,
            scale: 1.5
        )
        precondition(fractional?.sourceRect == CGRect(x: 140, y: 50, width: 301, height: 201))
        precondition(fractional?.width == 452 && fractional?.height == 302)
        precondition(ScreenshotCaptureGeometry(rect: selection, displayBounds: display, scale: 0) == nil)
        precondition(ScreenshotCaptureGeometry(rect: selection, displayBounds: display, scale: .infinity) == nil)
        precondition(ScreenshotCaptureGeometry(rect: selection, displayBounds: display, scale: .nan) == nil)
        precondition(ScreenshotCaptureGeometry(rect: CGRect(x: -1300, y: 250, width: 0, height: 200), displayBounds: display, scale: 2) == nil)
        precondition(ScreenshotCaptureGeometry(rect: CGRect(x: -1500, y: 250, width: 300, height: 200), displayBounds: display, scale: 2) == nil)
        let rightDisplay = CGRect(x: 0, y: 200, width: 1920, height: 1080)
        precondition(!screenshotNeedsWindowListCapture(rect: selection, displayBounds: [display, rightDisplay]))
        precondition(screenshotNeedsWindowListCapture(rect: CGRect(x: -50, y: 250, width: 100, height: 100), displayBounds: [display, rightDisplay]))
        precondition(screenshotNeedsWindowListCapture(rect: selection, displayBounds: []))
        let idKey = kCGWindowNumber as String
        let windows: [[String: Any]] = [[idKey: 5], [idKey: 9], [idKey: 3]]
        precondition(screenshotWindowIDs(from: windows, excluding: [9]) == [5, 3])
        precondition(screenshotWindowIDs(from: [[idKey: NSNumber(value: UInt32.max)]], excluding: []) == [UInt32.max])
        precondition(screenshotWindowIDs(from: [[idKey: -1]], excluding: []) == [UInt32.max])
        precondition(screenshotWindowIDs(from: [[idKey: Int64(Int32.min) - 1]], excluding: []) == nil)
        precondition(screenshotWindowIDs(from: [[idKey: 0]], excluding: []) == nil)
        precondition(screenshotWindowIDs(from: [[idKey: NSNumber(value: 1.5)]], excluding: []) == nil)
        precondition(screenshotWindowIDs(from: [[idKey: "5"]], excluding: []) == nil)
        precondition(screenshotWindowIDs(from: [[idKey: 5], [:]], excluding: []) == nil)
        precondition(screenshotWindowIDs(from: [[idKey: 5]], excluding: [5]) == nil)
        precondition(unresolvedVisibleScreenshotExclusions(requested: [1, 2], available: [1], onScreen: [2, 3]) == [2])
        precondition(unresolvedVisibleScreenshotExclusions(requested: [1, 2], available: [1], onScreen: [1, 3]).isEmpty)
        precondition(unresolvedVisibleScreenshotExclusions(requested: [1, 2], available: [1, 2], onScreen: [1, 2]).isEmpty)
        print("Screenshot capture geometry passed")
    }
}
