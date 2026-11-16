import AVFoundation
import CoreVideo
import Foundation
import ScreenCaptureKit

@main
struct RecordingStopStreamErrorTests {
    @MainActor
    static func main() async throws {
        let inbox = StreamFailureInbox()
        let activeToken = NSObject()
        let inactiveToken = NSObject()
        let activeID = ObjectIdentifier(activeToken)
        let inactiveID = ObjectIdentifier(inactiveToken)
        inbox.activate(activeID)
        precondition(!inbox.enqueue(NSError(domain: "stale", code: 0), from: inactiveID))
        DispatchQueue.concurrentPerform(iterations: 64) { index in
            precondition(inbox.enqueue(NSError(domain: "concurrent", code: index), from: activeID))
        }
        let concurrent = inbox.drain(from: activeID)
        precondition(concurrent.count == 64)
        precondition(Set(concurrent.map { ($0.error as NSError).code }).count == 64)
        precondition(concurrent.allSatisfy { !$0.duringStop })
        inbox.markStopping()
        precondition(inbox.enqueue(NSError(domain: "stop", code: 0), from: activeID))
        precondition(inbox.close().first?.duringStop == true)
        precondition(!inbox.enqueue(NSError(domain: "late", code: 0), from: activeID))

        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let movie = directory.appendingPathComponent("recording.mov")
        let writer = try AVAssetWriter(outputURL: movie, fileType: .mov)
        let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: 64,
            AVVideoHeightKey: 64,
        ])
        let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: nil)
        precondition(writer.canAdd(input))
        writer.add(input)
        precondition(writer.startWriting())
        writer.startSession(atSourceTime: .zero)
        var frame: CVPixelBuffer?
        precondition(CVPixelBufferCreate(nil, 64, 64, kCVPixelFormatType_32BGRA, nil, &frame) == kCVReturnSuccess)
        CVPixelBufferLockBaseAddress(frame!, [])
        memset(CVPixelBufferGetBaseAddress(frame!)!, 0x55, CVPixelBufferGetDataSize(frame!))
        CVPixelBufferUnlockBaseAddress(frame!, [])
        precondition(input.isReadyForMoreMediaData && adaptor.append(frame!, withPresentationTime: .zero))
        precondition(input.isReadyForMoreMediaData && adaptor.append(frame!, withPresentationTime: CMTime(value: 1, timescale: 1)))

        let config = RecordingConfig(
            x: nil, y: nil, width: nil, height: nil, displayID: nil, windowID: 42,
            excludedWindowIDs: [], includeAudio: false, micEnabled: false,
            micDeviceId: nil, micDeviceName: nil, cameraEnabled: false,
            cameraDeviceId: nil, cameraDeviceName: nil, keyboardEnabled: false,
            frameRate: 30, outputPath: movie.path, iosDeviceId: nil, iosDeviceName: nil
        )
        let recorder = ScreenCaptureRecorder()
        recorder.testBeginStreamStartup(config: config, sessionID: 73)
        recorder.testFinishStreamStartup(recording: true)
        recorder.testConfigureStop(writer: writer, input: input)
        let stream = NSObject()
        recorder.testActivateStream(stream)
        recorder.testOnStopping = {
            let completed = DispatchSemaphore(value: 0)
            DispatchQueue.global().async {
                precondition(recorder.testQueueStreamFailure(NSError(domain: "synthetic", code: 1), stream: stream))
                completed.signal()
            }
            completed.wait()
        }
        do {
            _ = try await recorder.stop()
            fatalError("A stream failure during Stop must be reported")
        } catch RecorderError.capture {
        }
        recorder.testOnStopping = nil
        precondition(!recorder.testQueueStreamFailure(NSError(domain: "synthetic", code: 2), stream: stream))
        let asset = AVURLAsset(url: movie)
        let track = try await asset.loadTracks(withMediaType: .video).first!
        let reader = try AVAssetReader(asset: asset)
        let output = AVAssetReaderTrackOutput(track: track, outputSettings: [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
        ])
        precondition(reader.canAdd(output))
        reader.add(output)
        precondition(reader.startReading())
        precondition(output.copyNextSampleBuffer() != nil, "The partial movie must remain decodable")

        let normal = ScreenCaptureRecorder()
        normal.testBeginStreamStartup(config: config, sessionID: 74)
        normal.testFinishStreamStartup(recording: true)
        let normalStream = NSObject()
        let staleStream = NSObject()
        normal.testActivateStream(normalStream)
        normal.testOnStopping = {
            precondition(!normal.testQueueStreamFailure(NSError(domain: "synthetic", code: 1), stream: staleStream))
            precondition(normal.testQueueStreamFailure(
                NSError(domain: SCStreamErrorDomain, code: SCStreamError.Code.noCaptureSource.rawValue),
                stream: normalStream
            ))
            precondition(normal.testQueueStreamFailure(
                NSError(domain: SCStreamErrorDomain, code: SCStreamError.Code.userStopped.rawValue),
                stream: normalStream
            ))
        }
        _ = try await normal.stop()
        normal.testOnStopping = nil

        var displayConfig = config
        displayConfig.windowID = nil
        let display = ScreenCaptureRecorder()
        display.testBeginStreamStartup(config: displayConfig, sessionID: 75)
        display.testFinishStreamStartup(recording: true)
        let displayStream = NSObject()
        display.testActivateStream(displayStream)
        display.testOnStopping = {
            precondition(display.testQueueStreamFailure(
                NSError(domain: SCStreamErrorDomain, code: SCStreamError.Code.noCaptureSource.rawValue),
                stream: displayStream
            ))
        }
        do {
            _ = try await display.stop()
            fatalError("A missing display source during Stop must be reported")
        } catch RecorderError.capture {
        }
        display.testOnStopping = nil

        let interrupted = ScreenCaptureRecorder()
        interrupted.testBeginStreamStartup(config: config, sessionID: 77)
        interrupted.testFinishStreamStartup(recording: true)
        let interruptedStream = NSObject()
        interrupted.testActivateStream(interruptedStream)
        precondition(interrupted.testQueueStreamFailure(
            NSError(domain: SCStreamErrorDomain, code: SCStreamError.Code.userStopped.rawValue),
            stream: interruptedStream
        ))
        do {
            _ = try await interrupted.stop()
            fatalError("A user-stopped error received before Stop must be reported")
        } catch RecorderError.capture {
        }

        recorder.testBeginStreamStartup(config: config, sessionID: 76)
        recorder.testFinishStreamStartup(recording: true)
        let nextStream = NSObject()
        recorder.testActivateStream(nextStream)
        precondition(!recorder.testQueueStreamFailure(NSError(domain: "synthetic", code: 3), stream: stream))
        _ = try await recorder.stop()
        print("Stop reports stream failure after preserving a decodable movie")
    }
}
