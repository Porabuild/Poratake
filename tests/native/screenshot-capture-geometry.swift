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
        verifyFrozenWindowColorSpace()
        verifyFrozenWindowAlpha()
        print("Screenshot capture geometry passed")
    }

    private static func image(width: Int, height: Int, space: CGColorSpace, bytes: [UInt8]) -> CGImage {
        CGImage(
            width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
            bytesPerRow: width * 4, space: space,
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
            provider: CGDataProvider(data: Data(bytes) as CFData)!,
            decode: nil, shouldInterpolate: false, intent: .defaultIntent
        )!
    }

    private static func pixels(_ image: CGImage, space: CGColorSpace) -> [UInt8] {
        let width = image.width
        let height = image.height
        let context = CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8,
            bytesPerRow: width * 4, space: space,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        return Array(UnsafeBufferPointer(start: context.data!.assumingMemoryBound(to: UInt8.self), count: width * height * 4))
    }

    private static func verifyFrozenWindowColorSpace() {
        let space = CGColorSpace(name: CGColorSpace.displayP3)!
        let green: [UInt8] = [45, 223, 60, 255]
        let purple: [UInt8] = [182, 20, 220, 255]
        let left = CGRect(x: -9, y: 7, width: 2, height: 2)
        let right = CGRect(x: left.maxX, y: left.minY, width: 2, height: 2)
        var leftPixels = Array(repeating: green, count: 16).flatMap { $0 }
        leftPixels.replaceSubrange(20..<24, with: purple)
        let first = image(width: 4, height: 4, space: space, bytes: leftPixels)
        let second = image(width: 2, height: 2, space: space, bytes: Array(repeating: purple, count: 4).flatMap { $0 })
        FrozenFrameStore.clear()
        defer { FrozenFrameStore.clear() }
        FrozenFrameStore.store(FrozenFrame(image: first, rect: left, scale: 2))
        FrozenFrameStore.store(FrozenFrame(image: second, rect: right, scale: 1))

        let single = FrozenFrameStore.crop(to: CGRect(x: -8.5, y: 7.5, width: 1, height: 1))!
        precondition(single.width == 2 && single.height == 2)
        precondition(single.colorSpace?.name == CGColorSpace.displayP3, "A single-display crop must preserve its Display P3 profile")
        precondition(pixels(single, space: space) == purple + green + green + green)

        let seam = FrozenFrameStore.crop(to: CGRect(x: -8, y: 7, width: 2, height: 2))!
        precondition(seam.width == 4 && seam.height == 4)
        precondition(seam.colorSpace?.name == CGColorSpace.displayP3, "A multi-display crop must preserve the common Display P3 profile")
        precondition(pixels(seam, space: space) == Array(repeating: green + green + purple + purple, count: 4).flatMap { $0 })

        FrozenFrameStore.clear()
        let sRGB = CGColorSpace(name: CGColorSpace.sRGB)!
        let mixed = image(width: 4, height: 4, space: sRGB, bytes: Array(repeating: green, count: 16).flatMap { $0 })
        FrozenFrameStore.store(FrozenFrame(image: mixed, rect: left, scale: 2))
        FrozenFrameStore.store(FrozenFrame(image: second, rect: right, scale: 1))
        let mixedSeam = FrozenFrameStore.crop(to: CGRect(x: -8, y: 7, width: 2, height: 2))!
        precondition(mixedSeam.colorSpace?.name == CGColorSpace.displayP3)
        let convertedGreen = Array(pixels(mixed, space: space).prefix(4))
        precondition(convertedGreen != green)
        precondition(pixels(mixedSeam, space: space) == Array(repeating: convertedGreen + convertedGreen + purple + purple, count: 4).flatMap { $0 })
    }

    private static func verifyFrozenWindowAlpha() {
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        let frozen = image(width: 2, height: 2, space: space, bytes: [
            200, 0, 0, 255, 160, 0, 0, 255,
            0, 0, 200, 255, 120, 0, 0, 255,
        ])
        let mask = image(width: 2, height: 2, space: space, bytes: [
            0, 0, 0, 0, 0, 255, 0, 255,
            0, 128, 0, 128, 0, 255, 0, 255,
        ])
        let composed = applyFrozenWindowAlpha(frozen, mask: mask)
        let expected: [[UInt8]] = [
            [0, 0, 0, 0], [160, 0, 0, 255],
            [0, 0, 100, 128], [120, 0, 0, 255],
        ]
        precondition(pixels(composed, space: space) == expected.flatMap { $0 })
        let wrongSize = image(width: 1, height: 1, space: space, bytes: [0, 0, 0, 0])
        precondition(applyFrozenWindowAlpha(frozen, mask: wrongSize) === frozen)
        precondition(applyFrozenWindowAlpha(frozen, mask: nil) === frozen)
    }
}
