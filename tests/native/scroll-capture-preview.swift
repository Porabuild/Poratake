import Cocoa
import Foundation
import ImageIO

@main
struct ScrollCapturePreviewTest {
    static func frame(start: Int, width: Int, height: Int) -> CGImage {
        var pixels = [UInt8]()
        pixels.reserveCapacity(width * height * 4)
        for y in start..<(start + height) {
            for x in 0..<width {
                pixels.append(UInt8(truncatingIfNeeded: y))
                pixels.append(UInt8(truncatingIfNeeded: y >> 8))
                pixels.append(UInt8(truncatingIfNeeded: x * 17 + y * 31))
                pixels.append(255)
            }
        }
        let provider = CGDataProvider(data: Data(pixels) as CFData)!
        return CGImage(
            width: width,
            height: height,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
            provider: provider,
            decode: nil,
            shouldInterpolate: false,
            intent: .defaultIntent
        )!
    }

    static func pixels(_ image: CGImage) -> Data {
        let context = CGContext(
            data: nil,
            width: image.width,
            height: image.height,
            bitsPerComponent: 8,
            bytesPerRow: image.width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return Data(bytes: context.data!, count: image.width * image.height * 4)
    }

    static func main() {
        let width = 80
        let height = 160
        let step = 112
        let allFrames = [0, step, step * 2].map { frame(start: $0, width: width, height: height) }
        for count in [1, 2, 3] {
            let output = URL(fileURLWithPath: NSTemporaryDirectory())
                .appendingPathComponent("poratake-scroll-preview-\(UUID().uuidString).png")
            defer { try? FileManager.default.removeItem(at: output) }
            let (preview, saved, encoded, overlaps) = ScrollCaptureModule().testPreviewAndSave(
                frames: Array(allFrames.prefix(count)),
                step: step,
                outputPath: output.path
            )
            precondition(overlaps == Array(repeating: height - step, count: count - 1))
            precondition(preview.width == width && preview.height == height + step * (count - 1))
            precondition(saved.width == preview.width && saved.height == preview.height)
            precondition(pixels(preview) == pixels(saved), "Preview differs from the saved scroll capture")
            let data = Data(base64Encoded: encoded)!
            let source = CGImageSourceCreateWithData(data as CFData, nil)!
            let emitted = CGImageSourceCreateImageAtIndex(source, 0, nil)!
            precondition(pixels(emitted) == pixels(saved), "Emitted preview differs from the saved scroll capture")
        }
        print("macOS scroll previews match saved pixels for one, two and three frames")
    }
}
