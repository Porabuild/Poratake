import AVFoundation
import CoreMedia
import Foundation

final class AudioBufferQueue {
    private enum Work {
        case buffer(CMSampleBuffer)
        case silence(CMSampleBuffer, CMTime, CMTime)
    }

    private let writer: AVAssetWriter
    private let input: AVAssetWriterInput
    private let queue = DispatchQueue(label: "com.porabuild.poratake.screen-recorder.audio-buffer-writer")
    private let prefix: (CMSampleBuffer, CMTime) -> CMSampleBuffer?
    private let adjust: (CMSampleBuffer, CMTime) -> CMSampleBuffer?
    private let onWrite: (CMSampleBuffer) -> Void
    private let onFailure: (Error) -> Void
    private var work: [Work] = []
    private var head = 0
    private var queuedBytes = 0
    private let maximumQueuedBytes: Int
    private var failure: Error?
    private var finishing: CheckedContinuation<Void, Error>?
    private var closed = false
    private var inputObservation: NSKeyValueObservation?
    private var writerObservation: NSKeyValueObservation?

    init(
        writer: AVAssetWriter,
        input: AVAssetWriterInput,
        maximumQueuedBytes: Int = 64 * 1024 * 1024,
        prefix: @escaping (CMSampleBuffer, CMTime) -> CMSampleBuffer?,
        adjust: @escaping (CMSampleBuffer, CMTime) -> CMSampleBuffer?,
        onWrite: @escaping (CMSampleBuffer) -> Void,
        onFailure: @escaping (Error) -> Void
    ) {
        self.writer = writer
        self.input = input
        self.maximumQueuedBytes = maximumQueuedBytes
        self.prefix = prefix
        self.adjust = adjust
        self.onWrite = onWrite
        self.onFailure = onFailure
        inputObservation = input.observe(\.isReadyForMoreMediaData, options: [.new]) { [weak self] _, _ in
            self?.queue.async { [weak self] in self?.drain() }
        }
        writerObservation = writer.observe(\.status, options: [.new]) { [weak self] _, _ in
            self?.queue.async { [weak self] in self?.drain() }
        }
    }

    func enqueue(_ buffer: CMSampleBuffer) -> Bool {
        queue.sync { append(.buffer(buffer)) }
    }

    func enqueueSilence(_ buffer: CMSampleBuffer, from start: CMTime, until end: CMTime) -> Bool {
        queue.sync { append(.silence(buffer, start, end)) }
    }

    func reportFailure(_ error: Error) {
        queue.sync { fail(error) }
    }

    private func append(_ item: Work) -> Bool {
        guard !closed, failure == nil, finishing == nil else { return false }
        if case .buffer(let buffer) = item {
            let size = CMSampleBufferGetDataBuffer(buffer).map(CMBlockBufferGetDataLength) ?? 0
            guard size <= maximumQueuedBytes - queuedBytes else {
                fail(RecorderError.capture("Audio writer could not keep up"))
                return false
            }
            queuedBytes += size
        }
        work.append(item)
        queue.async { [weak self] in self?.drain() }
        return true
    }

    func finish() async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            queue.async {
                if let failure = self.failure {
                    continuation.resume(throwing: failure)
                    return
                }
                guard !self.closed, self.finishing == nil else {
                    continuation.resume(throwing: RecorderError.capture("Audio writer is already closed"))
                    return
                }
                self.finishing = continuation
                self.drain()
            }
        }
    }

    func cancel() {
        queue.sync {
            closed = true
            work.removeAll()
            inputObservation?.invalidate()
            writerObservation?.invalidate()
            finishing?.resume(throwing: RecorderError.capture("Audio writer was cancelled"))
            finishing = nil
        }
    }

    private func drain() {
        guard !closed, failure == nil else { return }
        guard writer.status == .writing else {
            fail(writer.error ?? RecorderError.capture("Audio writer stopped unexpectedly"))
            return
        }
        var batch = 0
        while input.isReadyForMoreMediaData && head < work.count && batch < 64 {
            let buffer: CMSampleBuffer
            switch work[head] {
            case .buffer(let sample):
                buffer = sample
                head += 1
                queuedBytes -= CMSampleBufferGetDataBuffer(sample).map(CMBlockBufferGetDataLength) ?? 0
            case .silence(let sample, let start, let end):
                if CMTimeCompare(start, end) >= 0 {
                    head += 1
                    continue
                }
                let remaining = CMTimeSubtract(end, start)
                guard let part = prefix(sample, remaining) else {
                    var timing = CMSampleTimingInfo()
                    if CMSampleBufferGetSampleTimingInfo(sample, at: 0, timingInfoOut: &timing) == noErr,
                       timing.duration.isNumeric, CMTimeCompare(remaining, timing.duration) < 0 {
                        head += 1
                        continue
                    }
                    fail(RecorderError.capture("Could not create microphone silence"))
                    return
                }
                guard let adjusted = adjust(part, start) else {
                    fail(RecorderError.capture("Could not time microphone silence"))
                    return
                }
                buffer = adjusted
                work[head] = .silence(sample, CMTimeAdd(start, CMSampleBufferGetDuration(part)), end)
            }
            guard input.append(buffer) else {
                fail(writer.error ?? RecorderError.capture("Failed to write audio"))
                return
            }
            batch += 1
            onWrite(buffer)
        }
        if head == work.count {
            work.removeAll(keepingCapacity: true)
            head = 0
            if let continuation = finishing {
                finishing = nil
                closed = true
                inputObservation?.invalidate()
                writerObservation?.invalidate()
                input.markAsFinished()
                continuation.resume()
            }
        } else {
            if head >= 1024 {
                work.removeFirst(head)
                head = 0
            }
            if input.isReadyForMoreMediaData {
                queue.async { [weak self] in self?.drain() }
            }
        }
    }

    private func fail(_ error: Error) {
        guard !closed, failure == nil else { return }
        failure = error
        work.removeAll()
        inputObservation?.invalidate()
        writerObservation?.invalidate()
        finishing?.resume(throwing: error)
        finishing = nil
        onFailure(error)
    }
}
