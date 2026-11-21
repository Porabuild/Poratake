import AVFoundation
import CoreMedia
import CoreVideo
import Foundation

final class VideoFrameFinalizer {
    private let writer: AVAssetWriter
    private let input: AVAssetWriterInput
    private let adaptor: AVAssetWriterInputPixelBufferAdaptor
    private let frame: CVPixelBuffer
    private let time: CMTime
    private let queue = DispatchQueue(label: "com.porabuild.poratake.screen-recorder.video-finalizer")
    private var inputObservation: NSKeyValueObservation?
    private var writerObservation: NSKeyValueObservation?
    private var continuation: CheckedContinuation<Void, Error>?
    private var closed = false

    init(writer: AVAssetWriter, input: AVAssetWriterInput, adaptor: AVAssetWriterInputPixelBufferAdaptor, frame: CVPixelBuffer, time: CMTime) {
        self.writer = writer
        self.input = input
        self.adaptor = adaptor
        self.frame = frame
        self.time = time
        inputObservation = input.observe(\.isReadyForMoreMediaData, options: [.new]) { [weak self] _, _ in
            self?.queue.async { [weak self] in self?.drain() }
        }
        writerObservation = writer.observe(\.status, options: [.new]) { [weak self] _, _ in
            self?.queue.async { [weak self] in self?.drain() }
        }
    }

    func append() async throws {
        try await withCheckedThrowingContinuation { continuation in
            queue.async {
                self.continuation = continuation
                self.drain()
            }
        }
    }

    private func drain() {
        guard !closed, let continuation else { return }
        guard writer.status == .writing else {
            finish(continuation, error: writer.error ?? RecorderError.capture("Video writer stopped unexpectedly"))
            return
        }
        guard input.isReadyForMoreMediaData else { return }
        guard adaptor.append(frame, withPresentationTime: time) else {
            finish(continuation, error: writer.error ?? RecorderError.capture("Failed to write final video frame"))
            return
        }
        finish(continuation, error: nil)
    }

    private func finish(_ continuation: CheckedContinuation<Void, Error>, error: Error?) {
        closed = true
        self.continuation = nil
        inputObservation?.invalidate()
        writerObservation?.invalidate()
        if let error {
            continuation.resume(throwing: error)
        } else {
            continuation.resume()
        }
    }
}
