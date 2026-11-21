import CoreMedia
import CoreVideo
import AVFoundation
import Foundation

protocol PausableRecorder {
    var isRecording: Bool { get }
    var isPaused: Bool { get }
    func pause()
    func resume()
}

extension CameraRecorder {
    func probeStart() {
        isRecording = true
        isSynced = true
    }

    func probeScreenStart() {
        let clock = CMClockGetHostTimeClock()
        isSynced = false
        syncWithScreenStart(origin: CMTime(value: 90, timescale: 1), clock: clock, cameraClock: clock, pause: .zero)
        syncWithScreenStart(origin: CMTime(value: 80, timescale: 1), clock: clock, cameraClock: clock, pause: .zero)
        _ = probeFrame(89, red: 10)
        precondition(!sessionStarted && !hasWrittenFrame)
    }

    func probeWriter(_ path: String) throws {
        let url = URL(fileURLWithPath: path)
        try? FileManager.default.removeItem(at: url)
        let writer = try AVAssetWriter(outputURL: url, fileType: .mov)
        let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: 64,
            AVVideoHeightKey: 64,
        ])
        let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: 64,
            kCVPixelBufferHeightKey as String: 64,
        ])
        precondition(writer.canAdd(input))
        writer.add(input)
        precondition(writer.startWriting())
        assetWriter = writer
        videoInput = input
        pixelBufferAdaptor = adaptor
        outputPath = path
    }

    func probeImage(_ red: UInt8) -> CVPixelBuffer {
        var image: CVPixelBuffer?
        precondition(CVPixelBufferCreate(nil, 64, 64, kCVPixelFormatType_32BGRA, nil, &image) == kCVReturnSuccess)
        precondition(CVPixelBufferLockBaseAddress(image!, []) == kCVReturnSuccess)
        let base = CVPixelBufferGetBaseAddress(image!)!.assumingMemoryBound(to: UInt8.self)
        let stride = CVPixelBufferGetBytesPerRow(image!)
        for y in 0..<64 {
            for x in 0..<64 {
                let pixel = base + y * stride + x * 4
                pixel[0] = 0
                pixel[1] = 0
                pixel[2] = red
                pixel[3] = 255
            }
        }
        precondition(CVPixelBufferUnlockBaseAddress(image!, []) == kCVReturnSuccess)
        return image!
    }

    func probeFrame(_ seconds: Int64, red: UInt8) -> Double {
        let image = probeImage(red)
        var format: CMVideoFormatDescription?
        precondition(CMVideoFormatDescriptionCreateForImageBuffer(allocator: nil, imageBuffer: image, formatDescriptionOut: &format) == noErr)
        let timestamp = CMTime(value: seconds, timescale: 1)
        var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: timestamp, decodeTimeStamp: .invalid)
        var buffer: CMSampleBuffer?
        precondition(CMSampleBufferCreateReadyWithImageBuffer(allocator: nil, imageBuffer: image, formatDescription: format!, sampleTiming: &timing, sampleBufferOut: &buffer) == noErr)
        processBuffer(buffer!, originalTime: timestamp)
        return CMTimeGetSeconds(lastFrameTime)
    }

    func probeAlignedFrames() {
        assetWriter!.startSession(atSourceTime: .zero)
        let frames = [(0, 40), (10, 40), (11, 80), (16, 160), (17, 240)]
        let complete = DispatchSemaphore(value: 0)
        let queue = DispatchQueue(label: "camera-timeline-probe.writer")
        var index = 0
        videoInput!.requestMediaDataWhenReady(on: queue) {
            while self.videoInput!.isReadyForMoreMediaData && index < frames.count {
                let (time, red) = frames[index]
                let image = self.probeImage(UInt8(red))
                precondition(self.pixelBufferAdaptor!.append(image, withPresentationTime: CMTime(value: Int64(time), timescale: 1)))
                index += 1
            }
            if index == frames.count {
                self.videoInput!.markAsFinished()
                complete.signal()
            }
        }
        precondition(complete.wait(timeout: .now() + 30) == .success)
    }

    func probeFixedFrames() {
        let frames: [(Int64, UInt8)] = [(100, 40), (101, 80), (106, 160), (107, 240), (112, 120)]
        let expected: [Double] = [10, 11, 16, 17, 19]
        let complete = DispatchSemaphore(value: 0)
        let queue = DispatchQueue(label: "camera-timeline-probe.fixed")
        var index = 0
        videoInput!.requestMediaDataWhenReady(on: queue) {
            while self.videoInput!.isReadyForMoreMediaData && index < frames.count {
                if index == 2 {
                    self.suspend()
                    self.unsuspend()
                }
                if index == 4 {
                    self.pause()
                    self.resumeWithScreenPause(CMTime(value: 3, timescale: 1))
                }
                let (time, red) = frames[index]
                precondition(self.probeFrame(time, red: red) == expected[index])
                index += 1
            }
            if index == frames.count {
                self.videoInput!.markAsFinished()
                complete.signal()
            }
        }
        precondition(complete.wait(timeout: .now() + 30) == .success)
    }

    func probeCancelAfterFrame() {
        let complete = DispatchSemaphore(value: 0)
        let queue = DispatchQueue(label: "camera-timeline-probe.cancel")
        videoInput!.requestMediaDataWhenReady(on: queue) {
            guard self.videoInput!.isReadyForMoreMediaData else { return }
            _ = self.probeFrame(100, red: 40)
            precondition(self.hasWrittenFrame)
            self.assetWriter!.cancelWriting()
            complete.signal()
        }
        precondition(complete.wait(timeout: .now() + 30) == .success)
    }
}

@main
struct CameraTimelineProbe {
    static func main() {
        guard CommandLine.arguments.count == 3 else {
            fatalError("expected output path and fixed, aligned, empty or cancelled mode")
        }
        let camera = CameraRecorder()
        try! camera.probeWriter(CommandLine.arguments[1])
        camera.probeStart()
        if CommandLine.arguments[2] == "cancelled" {
            camera.probeScreenStart()
            camera.probeCancelAfterFrame()
            let result = camera.stop()
            precondition(result != nil && result?.videoPath == nil && result?.metadataPath == nil)
            precondition(!FileManager.default.fileExists(atPath: CommandLine.arguments[1]))
            print("cancelled camera writer discarded")
            return
        }
        if CommandLine.arguments[2] == "empty" {
            camera.pause()
            let result = camera.stop()
            precondition(result != nil && result?.videoPath == nil && result?.metadataPath == nil)
            precondition(!FileManager.default.fileExists(atPath: CommandLine.arguments[1]))
            print("empty camera writer stopped and removed")
            return
        }
        if CommandLine.arguments[2] == "aligned" {
            camera.probeAlignedFrames()
            precondition(camera.stop() != nil)
            print("synthetic aligned movie finalized")
            return
        }
        if CommandLine.arguments[2] == "fixed" {
            camera.probeScreenStart()
            camera.probeFixedFrames()
            precondition(camera.stop() != nil)
            print("screen-aligned camera frames: 10, 11, 16, 17, 19")
            return
        }
        fatalError("expected fixed, aligned, empty or cancelled mode")
    }
}
