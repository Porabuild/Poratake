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
