import CoreGraphics
import Foundation

struct FrozenFrame {
    let image: CGImage
    let rect: CGRect
    let scale: CGFloat
}

enum FrozenFrameStore {
    private static let lock = NSLock()
    private static var frames: [FrozenFrame] = []

    private static func snapshot() -> [FrozenFrame] {
        lock.lock()
        defer { lock.unlock() }
        return frames
    }

    static func store(_ frame: FrozenFrame) {
        lock.lock()
        defer { lock.unlock() }
        frames.append(frame)
    }

    static func clear() {
        lock.lock()
        defer { lock.unlock() }
        frames.removeAll()
    }

    static func crop(to rect: CGRect) -> CGImage? {
        let intersections = snapshot().compactMap { frame -> (FrozenFrame, CGRect)? in
            let intersection = frame.rect.intersection(rect)
            if intersection.isNull || intersection.isEmpty {
                return nil
            }
            return (frame, intersection)
        }

        guard !intersections.isEmpty else {
            return nil
        }

        let scale = intersections.map { $0.0.scale }.max() ?? 1
        let width = Int(ceil(rect.width * scale))
        let height = Int(ceil(rect.height * scale))

        guard width > 0, height > 0 else {
            return nil
        }

        guard let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return nil
        }

        for (frame, intersection) in intersections {
            let sourceRect = CGRect(
                x: (intersection.minX - frame.rect.minX) * frame.scale,
                y: (intersection.minY - frame.rect.minY) * frame.scale,
                width: intersection.width * frame.scale,
                height: intersection.height * frame.scale
            ).integral.intersection(CGRect(
                origin: .zero,
                size: CGSize(width: frame.image.width, height: frame.image.height)
            ))

            guard let image = frame.image.cropping(to: sourceRect) else {
                continue
            }

            context.draw(image, in: CGRect(
                x: (intersection.minX - rect.minX) * scale,
                y: (rect.maxY - intersection.maxY) * scale,
                width: intersection.width * scale,
                height: intersection.height * scale
            ))
        }

        return context.makeImage()
    }
}
