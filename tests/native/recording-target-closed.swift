import Foundation
import CoreMedia
import CoreVideo
import ScreenCaptureKit

@main
struct RecordingTargetClosedTests {
    static func verifyWindowFrameCentering() {
        let width = 16
        let height = 12
        let attributes: [String: Any] = [
            kCVPixelBufferWidthKey as String: width,
            kCVPixelBufferHeightKey as String: height,
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferIOSurfacePropertiesKey as String: [:],
            kCVPixelBufferMetalCompatibilityKey as String: true,
        ]
        var pool: CVPixelBufferPool?
        precondition(CVPixelBufferPoolCreate(nil, nil, attributes as CFDictionary, &pool) == kCVReturnSuccess)
        var buffer: CVPixelBuffer?
        precondition(CVPixelBufferPoolCreatePixelBuffer(nil, pool!, &buffer) == kCVReturnSuccess)
        let image = buffer!
        var format: CMVideoFormatDescription?
        precondition(CMVideoFormatDescriptionCreateForImageBuffer(allocator: nil, imageBuffer: image, formatDescriptionOut: &format) == noErr)
        var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: .zero, decodeTimeStamp: .invalid)
        var sample: CMSampleBuffer?
        precondition(CMSampleBufferCreateReadyWithImageBuffer(allocator: nil, imageBuffer: image, formatDescription: format!, sampleTiming: &timing, sampleBufferOut: &sample) == noErr)
        let frame = sample!
        let centering = WindowRecordingFrame()
        precondition(centering.centered(image, sample: frame, pool: pool) === image)
        let attachments = CMSampleBufferGetSampleAttachmentsArray(frame, createIfNecessary: true)! as NSArray
        let metadata = attachments[0] as! NSMutableDictionary
        metadata[SCStreamFrameInfo.scaleFactor.rawValue] = 2.0
        metadata[SCStreamFrameInfo.contentRect.rawValue] = CGRect(x: 0, y: 0, width: 8, height: 6).dictionaryRepresentation
        precondition(centering.centered(image, sample: frame, pool: pool) === image)

        for (contentWidth, contentHeight) in [(6, 12), (16, 4)] {
            metadata[SCStreamFrameInfo.contentRect.rawValue] = CGRect(x: 0, y: 0, width: contentWidth / 2, height: contentHeight / 2).dictionaryRepresentation
            CVPixelBufferLockBaseAddress(image, [])
            let bytes = CVPixelBufferGetBaseAddress(image)!.assumingMemoryBound(to: UInt8.self)
            let stride = CVPixelBufferGetBytesPerRow(image)
            for y in 0..<height {
                for x in 0..<width {
                    let offset = y * stride + x * 4
                    bytes[offset] = x < contentWidth && y < contentHeight ? UInt8(40 + x) : 250
                    bytes[offset+1] = UInt8(80 + y)
                    bytes[offset+2] = 120
                    bytes[offset+3] = 255
                }
            }
            CVPixelBufferUnlockBaseAddress(image, [])

            let centered = centering.centered(image, sample: frame, pool: pool)
            precondition(centered !== image, "An off-center frame must be moved into a new buffer")
            let left = (width - contentWidth) / 2
            let top = (height - contentHeight) / 2
            CVPixelBufferLockBaseAddress(centered, .readOnly)
            let result = CVPixelBufferGetBaseAddress(centered)!.assumingMemoryBound(to: UInt8.self)
            let resultStride = CVPixelBufferGetBytesPerRow(centered)
            for y in 0..<height {
                for x in 0..<width {
                    let offset = y * resultStride + x * 4
                    let inside = x >= left && x < left + contentWidth && y >= top && y < top + contentHeight
                    let expected: [UInt8] = inside ? [UInt8(40 + x - left), UInt8(80 + y - top), 120, 255] : [0, 0, 0, 255]
                    precondition((0..<4).map { result[offset+$0] } == expected, "Incorrect centered frame pixel at \(x),\(y)")
                }
            }
            CVPixelBufferUnlockBaseAddress(centered, .readOnly)
        }
        print("Resized window frames use centered, opaque black padding on both axes")
    }

    static func main() {
        let missingSource = NSError(
            domain: SCStreamErrorDomain,
            code: SCStreamError.Code.noCaptureSource.rawValue
        )
        let windowFailure = RecorderError.streamFailure(missingSource, capturingWindow: true)
        guard case RecorderError.targetClosed = windowFailure else {
            fatalError("A closed recorded window must use the normal stop path")
        }
        precondition(RecorderError.targetClosed.code == "TARGET_CLOSED")

        let displayFailure = RecorderError.streamFailure(missingSource, capturingWindow: false)
        precondition((displayFailure as NSError) === missingSource)

        let otherStreamError = NSError(domain: SCStreamErrorDomain, code: -1)
        let otherFailure = RecorderError.streamFailure(otherStreamError, capturingWindow: true)
        precondition((otherFailure as NSError) === otherStreamError)

        let unrelatedError = NSError(domain: "other", code: SCStreamError.Code.noCaptureSource.rawValue)
        let unrelatedFailure = RecorderError.streamFailure(unrelatedError, capturingWindow: true)
        precondition((unrelatedFailure as NSError) === unrelatedError)

        var pixelBuffer: CVPixelBuffer?
        precondition(CVPixelBufferCreate(nil, 2, 2, kCVPixelFormatType_32BGRA, nil, &pixelBuffer) == kCVReturnSuccess)
        var format: CMVideoFormatDescription?
        precondition(CMVideoFormatDescriptionCreateForImageBuffer(allocator: nil, imageBuffer: pixelBuffer!, formatDescriptionOut: &format) == noErr)
        var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: .zero, decodeTimeStamp: .invalid)
        var sample: CMSampleBuffer?
        precondition(CMSampleBufferCreateReadyWithImageBuffer(allocator: nil, imageBuffer: pixelBuffer!, formatDescription: format!, sampleTiming: &timing, sampleBufferOut: &sample) == noErr)
        let frame = sample!
        precondition(!ScreenCaptureFrame.isStopped(frame))
        precondition(ScreenCaptureFrame.canAnchorTimeline(frame, type: .screen))
        precondition(!ScreenCaptureFrame.canAnchorTimeline(frame, type: .audio))
        var emptySample: CMSampleBuffer?
        precondition(CMSampleBufferCreateReady(
            allocator: nil, dataBuffer: nil, formatDescription: nil, sampleCount: 0,
            sampleTimingEntryCount: 0, sampleTimingArray: nil, sampleSizeEntryCount: 0,
            sampleSizeArray: nil, sampleBufferOut: &emptySample
        ) == noErr)
        precondition(!ScreenCaptureFrame.canAnchorTimeline(emptySample!, type: .screen))
        let attachments = CMSampleBufferGetSampleAttachmentsArray(frame, createIfNecessary: true)! as NSArray
        let metadata = attachments[0] as! NSMutableDictionary
        metadata[SCStreamFrameInfo.status.rawValue] = SCFrameStatus.complete.rawValue
        precondition(!ScreenCaptureFrame.isStopped(frame))
        metadata[SCStreamFrameInfo.status.rawValue] = SCFrameStatus.stopped.rawValue
        precondition(ScreenCaptureFrame.isStopped(frame))
        precondition(!ScreenCaptureFrame.canAnchorTimeline(frame, type: .screen))
        print("Recording target-closure classification passed")
        verifyWindowFrameCentering()
    }
}
