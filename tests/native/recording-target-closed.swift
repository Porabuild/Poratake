import Foundation
import CoreMedia
import CoreVideo
import ScreenCaptureKit

@main
struct RecordingTargetClosedTests {
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
    }
}
