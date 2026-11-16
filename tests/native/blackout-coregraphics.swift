import CoreGraphics
import Foundation

@main
struct BlackoutCoreGraphicsProbe {
    static func main() throws {
        var cases: [[String: Any]] = []
        for scale in [1.0, 1.25, 2.0] {
            for offset in [0.0, 0.25, 0.5] {
                let width = Int((100 * scale).rounded())
                let height = Int((80 * scale).rounded())
                let x = (32 + offset) * scale
                let y = (22 + offset) * scale
                var pixels = [UInt8](repeating: 0, count: width * height * 4)
                pixels.withUnsafeMutableBytes { bytes in
                    let context = CGContext(
                        data: bytes.baseAddress, width: width, height: height,
                        bitsPerComponent: 8, bytesPerRow: width * 4,
                        space: CGColorSpaceCreateDeviceRGB(),
                        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue
                    )!
                    context.setFillColor(red: 1, green: 0, blue: 1, alpha: 1)
                    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
                    context.translateBy(x: 0, y: CGFloat(height))
                    context.scaleBy(x: 1, y: -1)
                    context.setFillColor(red: 0, green: 0, blue: 0, alpha: 1)
                    let rect = CGRect(x: x, y: y, width: 20 * scale, height: 20 * scale)
                    context.addPath(CGPath(roundedRect: rect, cornerWidth: 2, cornerHeight: 2, transform: nil))
                    context.fillPath()
                }
                let rgba = (0..<4).map { row in
                    (0..<4).map { column in
                        let index = ((Int(y.rounded(.down)) + row) * width + Int(x.rounded(.down)) + column) * 4
                        return Array(pixels[index..<(index + 4)])
                    }
                }
                cases.append(["scale": scale, "offset": offset, "rgba": rgba])
            }
        }
        let data = try JSONSerialization.data(withJSONObject: cases, options: [.prettyPrinted, .sortedKeys])
        FileHandle.standardOutput.write(data)
    }
}
