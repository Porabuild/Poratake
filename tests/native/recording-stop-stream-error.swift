import AVFoundation
import CoreVideo
import Foundation
import ScreenCaptureKit

@main
struct RecordingStopStreamErrorTests {
    @MainActor
    static func main() async throws {
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
        recorder.testOnStopping = {
            recorder.testStreamFailure(NSError(domain: "synthetic", code: 1), isCurrentStream: true)
        }
        do {
            _ = try await recorder.stop()
            fatalError("A stream failure during Stop must be reported")
        } catch RecorderError.capture {
        }
        recorder.testOnStopping = nil
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
        normal.testOnStopping = {
            normal.testStreamFailure(NSError(domain: "synthetic", code: 1), isCurrentStream: false)
            normal.testStreamFailure(
                NSError(domain: SCStreamErrorDomain, code: SCStreamError.Code.noCaptureSource.rawValue),
                isCurrentStream: true
            )
            normal.testStreamFailure(
                NSError(domain: SCStreamErrorDomain, code: SCStreamError.Code.userStopped.rawValue),
                isCurrentStream: true
            )
        }
        _ = try await normal.stop()
        normal.testOnStopping = nil

        var displayConfig = config
        displayConfig.windowID = nil
        let display = ScreenCaptureRecorder()
        display.testBeginStreamStartup(config: displayConfig, sessionID: 75)
        display.testFinishStreamStartup(recording: true)
        display.testOnStopping = {
            display.testStreamFailure(
                NSError(domain: SCStreamErrorDomain, code: SCStreamError.Code.noCaptureSource.rawValue),
                isCurrentStream: true
            )
        }
        do {
            _ = try await display.stop()
            fatalError("A missing display source during Stop must be reported")
        } catch RecorderError.capture {
        }
        display.testOnStopping = nil

        recorder.testBeginStreamStartup(config: config, sessionID: 76)
        recorder.testFinishStreamStartup(recording: true)
        _ = try await recorder.stop()
        print("Stop reports stream failure after preserving a decodable movie")
    }
}
