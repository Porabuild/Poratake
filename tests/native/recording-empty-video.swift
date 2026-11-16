import AVFoundation
import Foundation

@main
struct RecordingEmptyVideoTests {
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
        precondition(writer.canAdd(input))
        writer.add(input)
        precondition(writer.startWriting())

        let config = RecordingConfig(
            x: nil, y: nil, width: nil, height: nil, displayID: nil, windowID: nil,
            excludedWindowIDs: [], includeAudio: false, micEnabled: false,
            micDeviceId: nil, micDeviceName: nil, cameraEnabled: false,
            cameraDeviceId: nil, cameraDeviceName: nil, keyboardEnabled: false,
            frameRate: 30, outputPath: movie.path, iosDeviceId: nil, iosDeviceName: nil
        )
        let recorder = ScreenCaptureRecorder()
        recorder.testBeginStreamStartup(config: config, sessionID: 91)
        recorder.testFinishStreamStartup(recording: true)
        recorder.testConfigureStop(writer: writer, input: input)
        do {
            _ = try await recorder.stop()
            fatalError("A recording without a video frame must not report success")
        } catch RecorderError.capture(let message) {
            precondition(!message.isEmpty)
            print("Empty recording error: \(message)")
        }
        precondition(recorder.state == .idle)
        print("A recording without screen frames fails without claiming a saved movie")
    }
}
