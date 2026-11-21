import CoreImage
import CoreMedia
import ScreenCaptureKit

enum ScreenCaptureFrame {
    static func canAnchorTimeline(_ sampleBuffer: CMSampleBuffer, type: SCStreamOutputType) -> Bool {
        type == .screen && CMSampleBufferGetImageBuffer(sampleBuffer) != nil && !isStopped(sampleBuffer)
    }

    static func isStopped(_ sampleBuffer: CMSampleBuffer) -> Bool {
        guard let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: false)
                as? [[SCStreamFrameInfo: Any]],
              let status = attachments.first?[.status] as? Int
        else { return false }
        return status == SCFrameStatus.stopped.rawValue
    }
}

final class WindowRecordingFrame {
    private lazy var context = CIContext(options: [
        .workingColorSpace: NSNull(),
        .outputColorSpace: NSNull(),
        .cacheIntermediates: false,
    ])

    func centered(
        _ image: CVPixelBuffer, sample: CMSampleBuffer, pool: CVPixelBufferPool?
    ) -> CVPixelBuffer {
        let bounds = CGRect(x: 0, y: 0, width: CVPixelBufferGetWidth(image), height: CVPixelBufferGetHeight(image))
        guard let content = Self.contentPixels(sample, bounds: bounds) else { return image }

        let source = CGRect(
            x: content.minX, y: bounds.maxY - content.maxY, width: content.width, height: content.height
        )
        let x = ((bounds.width - source.width) / 2).rounded(.down) - source.minX
        let y = ((bounds.height - source.height) / 2).rounded(.down) - source.minY
        guard x != 0 || y != 0, let pool else { return image }

        var destination: CVPixelBuffer?
        guard CVPixelBufferPoolCreatePixelBuffer(nil, pool, &destination) == kCVReturnSuccess,
              let destination else { return image }
        let placed = CIImage(cvPixelBuffer: image, options: [.colorSpace: NSNull()])
            .cropped(to: source)
            .transformed(by: CGAffineTransform(translationX: x, y: y))
        let background = CIImage(color: .black).cropped(to: bounds)
        context.render(placed.composited(over: background), to: destination, bounds: bounds, colorSpace: nil)
        CVBufferPropagateAttachments(image, destination)
        return destination
    }

    private static func contentPixels(_ sample: CMSampleBuffer, bounds: CGRect) -> CGRect? {
        guard let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: false)
                as? [[SCStreamFrameInfo: Any]],
              let info = attachments.first,
              let rectangle = info[.contentRect] as? [String: Any],
              let points = CGRect(dictionaryRepresentation: rectangle as CFDictionary),
              let scale = info[.scaleFactor] as? Double, scale.isFinite, scale > 0
        else { return nil }

        let pixels = points.applying(CGAffineTransform(scaleX: scale, y: scale))
        guard [pixels.minX, pixels.minY, pixels.maxX, pixels.maxY].allSatisfy(\.isFinite) else { return nil }
        let x = pixels.minX.rounded()
        let y = pixels.minY.rounded()
        let content = CGRect(
            x: x, y: y, width: pixels.maxX.rounded() - x, height: pixels.maxY.rounded() - y
        ).intersection(bounds)
        return content.isEmpty ? nil : content
    }
}
