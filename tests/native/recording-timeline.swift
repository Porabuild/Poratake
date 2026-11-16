import CoreMedia
import CoreVideo
import AudioToolbox
import AVFoundation
import Foundation
import ScreenCaptureKit

@main
struct RecordingTimelineTests {
    static func decodedAudio(_ url: URL) -> [Int16] {
        let asset = AVURLAsset(url: url)
        let track = asset.tracks(withMediaType: .audio).first!
        let reader = try! AVAssetReader(asset: asset)
        let output = AVAssetReaderTrackOutput(track: track, outputSettings: [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: 48_000,
            AVNumberOfChannelsKey: 2,
            AVLinearPCMBitDepthKey: 16,
            AVLinearPCMIsFloatKey: false,
            AVLinearPCMIsBigEndianKey: false,
            AVLinearPCMIsNonInterleaved: false,
        ])
        precondition(reader.canAdd(output))
        reader.add(output)
        precondition(reader.startReading())
        var decoded: [Int16] = []
        while let sample = output.copyNextSampleBuffer() {
            let block = CMSampleBufferGetDataBuffer(sample)!
            let count = CMBlockBufferGetDataLength(block)
            precondition(count % MemoryLayout<Int16>.size == 0)
            var values = [Int16](repeating: 0, count: count / MemoryLayout<Int16>.size)
            let status = values.withUnsafeMutableBytes { bytes in
                CMBlockBufferCopyDataBytes(block, atOffset: 0, dataLength: count, destination: bytes.baseAddress!)
            }
            precondition(status == noErr)
            decoded.append(contentsOf: values)
        }
        precondition(reader.status == .completed)
        return decoded
    }

    static func meanLevel(_ decoded: [Int16], at time: Double, samples: Int) -> Double {
        let start = Int(time * 48_000) * 2
        let end = start + samples
        precondition(end <= decoded.count)
        return decoded[start..<end].reduce(0.0) { $0 + abs(Double($1)) } / Double(end - start) / 32_768
    }

    static func verifySystemGap(_ url: URL) {
        let decoded = decodedAudio(url)
        for (time, audible) in [(1.0, false), (2.0, true), (2.01, true), (7.0, false), (8.0, true)] {
            let mean = meanLevel(decoded, at: time, samples: 480)
            precondition(audible ? mean > 0.2 : mean < 0.05, "Unexpected audio at \(time): \(mean)")
        }
    }

    static func finishMicrophone(_ recorder: ScreenCaptureRecorder) {
        let done = DispatchSemaphore(value: 0)
        var failure: Error?
        Task.detached {
            do { try await recorder.testFinishMicrophone() } catch { failure = error }
            done.signal()
        }
        precondition(done.wait(timeout: .now() + 60) == .success)
        precondition(failure == nil, "\(String(describing: failure))")
    }

    static func audioSample(multipleTimings: Bool = false, fill: UInt8 = 0x55) -> CMSampleBuffer {
        let count = 480
        var description = AudioStreamBasicDescription(
            mSampleRate: 48_000, mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsSignedInteger | kAudioFormatFlagIsPacked,
            mBytesPerPacket: 4, mFramesPerPacket: 1, mBytesPerFrame: 4,
            mChannelsPerFrame: 2, mBitsPerChannel: 16, mReserved: 0
        )
        var format: CMAudioFormatDescription?
        precondition(CMAudioFormatDescriptionCreate(allocator: nil, asbd: &description, layoutSize: 0, layout: nil, magicCookieSize: 0, magicCookie: nil, extensions: nil, formatDescriptionOut: &format) == noErr)
        var block: CMBlockBuffer?
        precondition(CMBlockBufferCreateWithMemoryBlock(allocator: nil, memoryBlock: nil, blockLength: count * 4, blockAllocator: kCFAllocatorDefault, customBlockSource: nil, offsetToData: 0, dataLength: count * 4, flags: kCMBlockBufferAssureMemoryNowFlag, blockBufferOut: &block) == noErr)
        precondition(CMBlockBufferFillDataBytes(with: Int8(bitPattern: fill), blockBuffer: block!, offsetIntoDestination: 0, dataLength: count * 4) == noErr)
        var timing = (0..<(multipleTimings ? count : 1)).map { index in
            CMSampleTimingInfo(duration: CMTime(value: 1, timescale: 48_000), presentationTimeStamp: CMTime(value: Int64(index), timescale: 48_000), decodeTimeStamp: .invalid)
        }
        var size = 4
        var buffer: CMSampleBuffer?
        precondition(CMSampleBufferCreateReady(allocator: nil, dataBuffer: block!, formatDescription: format!, sampleCount: count, sampleTimingEntryCount: timing.count, sampleTimingArray: &timing, sampleSizeEntryCount: 1, sampleSizeArray: &size, sampleBufferOut: &buffer) == noErr)
        return buffer!
    }

    static func sample(at seconds: Int64, size: Int = 2, fill: UInt8 = 0) -> CMSampleBuffer {
        var pixelBuffer: CVPixelBuffer?
        precondition(CVPixelBufferCreate(nil, size, size, kCVPixelFormatType_32BGRA, nil, &pixelBuffer) == kCVReturnSuccess)
        CVPixelBufferLockBaseAddress(pixelBuffer!, [])
        memset(CVPixelBufferGetBaseAddress(pixelBuffer!), Int32(fill), CVPixelBufferGetDataSize(pixelBuffer!))
        CVPixelBufferUnlockBaseAddress(pixelBuffer!, [])
        var format: CMVideoFormatDescription?
        precondition(CMVideoFormatDescriptionCreateForImageBuffer(allocator: nil, imageBuffer: pixelBuffer!, formatDescriptionOut: &format) == noErr)
        var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: CMTime(value: seconds, timescale: 1), decodeTimeStamp: .invalid)
        var buffer: CMSampleBuffer?
        precondition(CMSampleBufferCreateReadyWithImageBuffer(allocator: nil, imageBuffer: pixelBuffer!, formatDescription: format!, sampleTiming: &timing, sampleBufferOut: &buffer) == noErr)
        return buffer!
    }

    static func main() async {
        if let path = ProcessInfo.processInfo.environment["PORATAKE_TEST_MIC_STATIC_CLOCK"] {
            let pausedToggle = ProcessInfo.processInfo.environment["PORATAKE_TEST_MIC_PAUSED_TOGGLE"] != nil
            let firstPending = ProcessInfo.processInfo.environment["PORATAKE_TEST_MIC_FIRST_PENDING"] != nil
            let delayedCapture = ProcessInfo.processInfo.environment["PORATAKE_TEST_MIC_DELAYED_CAPTURE"] != nil
            let shiftedClock = ProcessInfo.processInfo.environment["PORATAKE_TEST_MIC_SHIFTED_CLOCK"] != nil
            let initialDelay = ProcessInfo.processInfo.environment["PORATAKE_TEST_MIC_INITIAL_DELAY"] != nil
            let callbackLate = ProcessInfo.processInfo.environment["PORATAKE_TEST_MIC_CALLBACK_LATE"] != nil
            let installAfter = ProcessInfo.processInfo.environment["PORATAKE_TEST_MIC_INSTALL_STATIC"] != nil
            let installBefore = ProcessInfo.processInfo.environment["PORATAKE_TEST_MIC_INSTALL_BEFORE_SCREEN"] != nil
            let url = URL(fileURLWithPath: path)
            try? FileManager.default.removeItem(at: url)
            let writer = try! AVAssetWriter(outputURL: url, fileType: .m4a)
            let input = AVAssetWriterInput(mediaType: .audio, outputSettings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 48_000,
                AVNumberOfChannelsKey: 2,
                AVEncoderBitRateKey: 192_000,
            ])
            input.expectsMediaDataInRealTime = true
            precondition(writer.canAdd(input))
            writer.add(input)
            precondition(writer.startWriting())
            if !installAfter && !installBefore { writer.startSession(atSourceTime: .zero) }
            let recorder = ScreenCaptureRecorder()
            recorder.testStartTimeline()
            if installBefore { recorder.testInstallMicrophoneWriter(writer, input: input) }
            recorder.testProcessSample(sample(at: 100), type: .screen)
            if installAfter {
                recorder.testInstallMicrophoneWriter(writer, input: input)
            } else if !installBefore {
                recorder.testConfigureMicrophone(writer: writer, input: input, writtenVideo: !firstPending)
            }
            if installAfter || installBefore { precondition(recorder.testMicrophoneWriterStarted) }
            if delayedCapture {
                let clock = CMClockGetHostTimeClock()
                if shiftedClock {
                    var source: CMTimebase?
                    precondition(CMTimebaseCreateWithSourceClock(allocator: nil, sourceClock: clock, timebaseOut: &source) == noErr)
                    precondition(CMTimebaseSetRateAndAnchorTime(source!, rate: 1, anchorTime: CMTime(value: 1000, timescale: 1), immediateSourceTime: CMTime(value: 100, timescale: 1)) == noErr)
                    recorder.testSetMicrophoneClocks(source!, clock)
                } else {
                    recorder.testSetMicrophoneClocks(clock, clock)
                }
            }
            recorder.testSetCameraClockTime(CMTime(value: 120, timescale: 1))
            if delayedCapture {
                if !initialDelay { try! recorder.testArmMicrophoneDuringSetup() }
                recorder.testSetCameraClockTime(CMTime(value: callbackLate ? 123 : 121, timescale: 1))
                let sample = recorder.testAdjustedSample(audioSample(), at: CMTime(value: shiftedClock ? 1021 : 121, timescale: 1))!
                recorder.testContinueMicrophone(sample)
            } else if pausedToggle {
                try! recorder.pause()
                recorder.testSetCameraClockTime(CMTime(value: 125, timescale: 1))
                try! recorder.testArmMicrophoneDuringSetup()
                recorder.testSetCameraClockTime(CMTime(value: 130, timescale: 1))
                try! recorder.resume()
                recorder.testContinueMicrophone(audioSample())
            } else {
                try! recorder.testEnableMicrophoneDuringSetup(audioSample())
            }
            print("static microphone first end: \(CMTimeGetSeconds(recorder.testMicrophoneEnd!))")
            fflush(stdout)
            let expectedStart = delayedCapture ? 21.0 : 20.0
            precondition(abs(CMTimeGetSeconds(recorder.testMicrophoneEnd!) - (expectedStart + 0.01)) < 0.00001)
            for tick in 1...50 {
                let base: Int64 = pausedToggle ? 13_000 : delayedCapture ? (callbackLate ? 12_300 : 12_100) : 12_000
                recorder.testSetCameraClockTime(CMTime(value: base + Int64(tick), timescale: 100))
                let sampleTime = delayedCapture ? 12_100 + (shiftedClock ? 90_000 : 0) + Int64(tick) : Int64(tick)
                recorder.testContinueMicrophone(recorder.testAdjustedSample(audioSample(), at: CMTime(value: sampleTime, timescale: 100))!)
            }
            print("static microphone second end: \(CMTimeGetSeconds(recorder.testMicrophoneEnd!))")
            fflush(stdout)
            precondition(abs(CMTimeGetSeconds(recorder.testMicrophoneEnd!) - (expectedStart + 0.51)) < 0.00001)
            try! await recorder.testFinishMicrophone()
            await writer.finishWriting()
            precondition(writer.status == .completed)
            let decoded = decodedAudio(url)
            for (time, audible) in [(expectedStart - 0.5, false), (expectedStart + 0.1, true), (expectedStart + 0.45, true)] {
                let mean = meanLevel(decoded, at: time, samples: 192)
                precondition(audible ? mean > 0.4 : mean < 0.05, "Unexpected static-screen microphone audio at \(time): \(mean)")
            }
            return
        }
        if let path = ProcessInfo.processInfo.environment["PORATAKE_TEST_STATIC_VIDEO"] {
            let pausedStop = ProcessInfo.processInfo.environment["PORATAKE_TEST_STATIC_PAUSED_STOP"] != nil
            let noClock = ProcessInfo.processInfo.environment["PORATAKE_TEST_STATIC_NO_CLOCK"] != nil
            let droppedChange = ProcessInfo.processInfo.environment["PORATAKE_TEST_VIDEO_DROPPED_CHANGE"] != nil
            let droppedFirst = ProcessInfo.processInfo.environment["PORATAKE_TEST_VIDEO_DROPPED_FIRST"] != nil
            let recoveredChange = ProcessInfo.processInfo.environment["PORATAKE_TEST_VIDEO_RECOVERED_CHANGE"] != nil
            let url = URL(fileURLWithPath: path)
            try? FileManager.default.removeItem(at: url)
            let writer = try! AVAssetWriter(outputURL: url, fileType: .mov)
            let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
                AVVideoCodecKey: AVVideoCodecType.h264,
                AVVideoWidthKey: 64,
                AVVideoHeightKey: 64,
            ])
            input.expectsMediaDataInRealTime = true
            let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: [
                kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
                kCVPixelBufferWidthKey as String: 64,
                kCVPixelBufferHeightKey as String: 64,
            ])
            precondition(writer.canAdd(input))
            writer.add(input)
            precondition(writer.startWriting())
            let recorder = ScreenCaptureRecorder()
            recorder.testConfigureScreenWriter(writer: writer, input: input, adaptor: adaptor)
            let frame = sample(at: 100, size: 64, fill: 0x22)
            if droppedFirst { recorder.testSetVideoBackpressure(true) }
            recorder.testProcessSample(frame, type: .screen)
            if droppedFirst { recorder.testSetVideoBackpressure(false) }
            recorder.testProcessSample(sample(at: 120), type: .audio)
            recorder.testSetCameraClockTime(CMTime(value: 125, timescale: 1))
            if droppedChange || recoveredChange {
                recorder.testSetVideoBackpressure(true)
                recorder.testProcessSample(sample(at: 105, size: 64, fill: 0x55), type: .screen)
                recorder.testProcessSample(sample(at: 110, size: 64, fill: 0x77), type: .screen)
                recorder.testSetVideoBackpressure(false)
            }
            if recoveredChange {
                recorder.testProcessSample(sample(at: 115, size: 64, fill: 0xbb), type: .screen)
            }
            if pausedStop {
                try! recorder.pause()
                precondition(CMTimeGetSeconds(recorder.testTimeline.pause!) == 125)
                recorder.testSetCameraClockTime(CMTime(value: 130, timescale: 1))
            }
            if noClock {
                recorder.testProcessSample(sample(at: 120, size: 64, fill: 0x22), type: .screen)
                recorder.testSetCameraClockTime(nil)
            }
            if ProcessInfo.processInfo.environment["PORATAKE_TEST_STATIC_PAUSE"] != nil {
                try! recorder.pause()
                print("static pause: start=\(CMTimeGetSeconds(recorder.testTimeline.pause!))")
                precondition(CMTimeGetSeconds(recorder.testTimeline.pause!) == 125)
                recorder.testOpenCameraRange()
                recorder.testSetCameraClockTime(CMTime(value: 127, timescale: 1))
                recorder.testCloseCameraRange()
                precondition(recorder.testCameraRanges[0].start == 25 && recorder.testCameraRanges[0].end == 25)
                recorder.testSetCameraClockTime(CMTime(value: 130, timescale: 1))
                try! recorder.resume()
                precondition(recorder.testTimeline.pause == nil)
                recorder.testSetCameraClockTime(CMTime(value: 132, timescale: 1))
                recorder.testOpenCameraRange()
                recorder.testSetCameraClockTime(CMTime(value: 134, timescale: 1))
                recorder.testCloseCameraRange()
                precondition(recorder.testCameraRanges[1].start == 27 && recorder.testCameraRanges[1].end == 29)
                recorder.testSetCameraClockTime(CMTime(value: 135, timescale: 1))
                recorder.testProcessSample(sample(at: 135, size: 64, fill: 0x77), type: .screen)
                print("static pause: accumulated=\(CMTimeGetSeconds(recorder.testTimeline.pausedDuration))")
                precondition(CMTimeGetSeconds(recorder.testTimeline.pausedDuration) == 5)
                recorder.testSetCameraClockTime(CMTime(value: 136, timescale: 1))
            }
            let result = try! await recorder.stop()
            let movie = AVURLAsset(url: url)
            let duration = CMTimeGetSeconds(movie.duration)
            print("static primary: reported=\(result.duration) movie=\(duration)")
            fflush(stdout)
            let expected = ProcessInfo.processInfo.environment["PORATAKE_TEST_STATIC_PAUSE"] != nil ? 31.0 : (noClock ? 20.0 + 1.0 / 30.0 : 25.0)
            precondition(abs(result.duration - expected) < 0.001 && abs(duration - expected) < 0.001, "Static screen duration mismatch")
            let track = movie.tracks(withMediaType: .video).first!
            let reader = try! AVAssetReader(asset: movie)
            let output = AVAssetReaderTrackOutput(track: track, outputSettings: [
                kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            ])
            precondition(reader.canAdd(output))
            reader.add(output)
            precondition(reader.startReading())
            var lastTime = -1.0
            var lastValue: UInt8 = 0
            var frames = 0
            var changedFrameTime: Double?
            while let buffer = output.copyNextSampleBuffer() {
                lastTime = CMTimeGetSeconds(CMSampleBufferGetPresentationTimeStamp(buffer))
                let image = CMSampleBufferGetImageBuffer(buffer)!
                CVPixelBufferLockBaseAddress(image, .readOnly)
                lastValue = CVPixelBufferGetBaseAddress(image)!.assumingMemoryBound(to: UInt8.self).pointee
                CVPixelBufferUnlockBaseAddress(image, .readOnly)
                if abs(Int(lastValue) - 0x77) <= 5, changedFrameTime == nil { changedFrameTime = lastTime }
                frames += 1
            }
            precondition(reader.status == .completed && frames >= 2 && lastTime >= expected - 0.1 && lastTime < expected,
                         "Missing held final frame: count=\(frames), time=\(lastTime)")
            let expectedValue = recoveredChange ? 0xbb : (ProcessInfo.processInfo.environment["PORATAKE_TEST_STATIC_PAUSE"] != nil || droppedChange ? 0x77 : 0x22)
            precondition(abs(Int(lastValue) - expectedValue) <= 5, "Final frame has the wrong content: \(lastValue)")
            if droppedChange || recoveredChange {
                precondition(changedFrameTime.map { abs($0 - 10) < 0.001 } == true,
                             "Busy-writer screen change has the wrong timestamp: \(String(describing: changedFrameTime))")
            }
            return
        }
        if let path = ProcessInfo.processInfo.environment["PORATAKE_TEST_VIDEO_FINALIZER_CANCELLED"] {
            let url = URL(fileURLWithPath: path)
            try? FileManager.default.removeItem(at: url)
            let writer = try! AVAssetWriter(outputURL: url, fileType: .mov)
            let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
                AVVideoCodecKey: AVVideoCodecType.h264, AVVideoWidthKey: 64, AVVideoHeightKey: 64,
            ])
            let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: nil)
            precondition(writer.canAdd(input))
            writer.add(input)
            precondition(writer.startWriting())
            writer.startSession(atSourceTime: .zero)
            writer.cancelWriting()
            let finalizer = VideoFrameFinalizer(writer: writer, input: input, adaptor: adaptor,
                                                frame: CMSampleBufferGetImageBuffer(sample(at: 0, size: 64))!,
                                                time: CMTime(value: 1, timescale: 1))
            do {
                try await finalizer.append()
                preconditionFailure("Cancelled video writer accepted a final frame")
            } catch {
                print("Cancelled video writer reported failure")
            }
            return
        }
        if let path = ProcessInfo.processInfo.environment["PORATAKE_TEST_VIDEO_FINALIZER_BACKPRESSURE"] {
            let url = URL(fileURLWithPath: path)
            try? FileManager.default.removeItem(at: url)
            let writer = try! AVAssetWriter(outputURL: url, fileType: .mov)
            let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
                AVVideoCodecKey: AVVideoCodecType.h264, AVVideoWidthKey: 720, AVVideoHeightKey: 720,
            ])
            input.expectsMediaDataInRealTime = true
            let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: nil)
            precondition(writer.canAdd(input))
            writer.add(input)
            precondition(writer.startWriting())
            writer.startSession(atSourceTime: .zero)
            let frame = CMSampleBufferGetImageBuffer(sample(at: 0, size: 720, fill: 0x55))!
            var count: Int64 = 0
            var backpressure = false
            while count < 10_000 {
                if !input.isReadyForMoreMediaData {
                    backpressure = true
                    break
                }
                precondition(adaptor.append(frame, withPresentationTime: CMTime(value: count, timescale: 30)))
                count += 1
            }
            precondition(count > 0 && backpressure, "Could not exercise video writer backpressure")
            let finalizer = VideoFrameFinalizer(writer: writer, input: input, adaptor: adaptor, frame: frame,
                                                time: CMTime(value: count, timescale: 30))
            try! await finalizer.append()
            writer.endSession(atSourceTime: CMTime(value: count + 1, timescale: 30))
            input.markAsFinished()
            await writer.finishWriting()
            precondition(writer.status == .completed, "\(String(describing: writer.error))")
            print("Final video frame accepted after backpressure on \(count) frames")
            return
        }
        if let path = ProcessInfo.processInfo.environment["PORATAKE_TEST_SYSTEM_INITIAL_OVERLAP"] {
            let writer = try! AVAssetWriter(outputURL: URL(fileURLWithPath: path), fileType: .m4a)
            let input = AVAssetWriterInput(mediaType: .audio, outputSettings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 48_000,
                AVNumberOfChannelsKey: 2,
                AVEncoderBitRateKey: 192_000,
            ])
            input.expectsMediaDataInRealTime = true
            precondition(writer.canAdd(input))
            writer.add(input)
            precondition(writer.startWriting())
            writer.startSession(atSourceTime: .zero)
            let recorder = ScreenCaptureRecorder()
            recorder.testStartTimeline()
            recorder.testProcessSample(sample(at: 100), type: .screen)
            recorder.testConfigureSystemAudioForStream(writer: writer, input: input)
            for (fill, ticks) in [(UInt8(0x11), Int64(19_997)), (0x55, 19_999), (0x33, 20_001)] {
                let sample = recorder.testAdjustedSample(audioSample(fill: fill), at: CMTime(value: ticks, timescale: 200))!
                recorder.testProcessSample(sample, type: .audio)
            }
            try! await recorder.testFinishSystemAudio()
            await writer.finishWriting()
            precondition(writer.status == .completed)
            let decoded = decodedAudio(URL(fileURLWithPath: path))
            for (time, expected) in [(0.002, 0.667), (0.008, 0.4), (0.012, 0.4)] {
                let mean = meanLevel(decoded, at: time, samples: 192)
                precondition(abs(mean - expected) < 0.08, "Unexpected initial audio at \(time): \(mean)")
                print("Initial overlap audio at \(time): \(mean)")
            }
            return
        }
        if let path = ProcessInfo.processInfo.environment["PORATAKE_TEST_SYSTEM_FRACTIONAL_GAPS"] {
            let writer = try! AVAssetWriter(outputURL: URL(fileURLWithPath: path), fileType: .m4a)
            let input = AVAssetWriterInput(mediaType: .audio, outputSettings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 48_000,
                AVNumberOfChannelsKey: 2,
                AVEncoderBitRateKey: 192_000,
            ])
            input.expectsMediaDataInRealTime = true
            precondition(writer.canAdd(input))
            writer.add(input)
            precondition(writer.startWriting())
            writer.startSession(atSourceTime: .zero)
            let recorder = ScreenCaptureRecorder()
            recorder.testConfigureSystemAudio(writer: writer, input: input)
            let sample = audioSample()
            for index in 0..<1_000 {
                let time = CMTime(value: Int64(index) * 1_001, timescale: 100_000)
                if index == 500 {
                    recorder.testWriteSystemAudio(sample, at: CMTimeSubtract(time, CMTime(value: 1, timescale: 200)))
                }
                recorder.testWriteSystemAudio(sample, at: time)
            }
            try! await recorder.testFinishSystemAudio()
            await writer.finishWriting()
            precondition(writer.status == .completed)
            let duration = CMTimeGetSeconds(AVURLAsset(url: URL(fileURLWithPath: path)).duration)
            precondition(abs(duration - 10.00999) < 1.0 / 48_000, "Fractional gaps drifted: \(duration)")
            print("Fractional-gap audio duration: \(duration)")
            return
        }
        if let path = ProcessInfo.processInfo.environment["PORATAKE_TEST_SYSTEM_OVERLAP"] {
            let writer = try! AVAssetWriter(outputURL: URL(fileURLWithPath: path), fileType: .m4a)
            let input = AVAssetWriterInput(mediaType: .audio, outputSettings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 48_000,
                AVNumberOfChannelsKey: 2,
                AVEncoderBitRateKey: 192_000,
            ])
            input.expectsMediaDataInRealTime = true
            precondition(writer.canAdd(input))
            writer.add(input)
            precondition(writer.startWriting())
            writer.startSession(atSourceTime: .zero)
            let recorder = ScreenCaptureRecorder()
            recorder.testConfigureSystemAudio(writer: writer, input: input)
            for (sample, time) in [
                (audioSample(fill: 0x22), CMTime.zero),
                (audioSample(multipleTimings: true), CMTime(value: 1, timescale: 200)),
                (audioSample(fill: 0x33), CMTime(value: 1, timescale: 100)),
                (audioSample(fill: 0x77), CMTime.zero),
            ] {
                recorder.testWriteSystemAudio(sample, at: time)
            }
            try! await recorder.testFinishSystemAudio()
            await writer.finishWriting()
            precondition(writer.status == .completed)
            let duration = CMTimeGetSeconds(AVURLAsset(url: URL(fileURLWithPath: path)).duration)
            precondition(abs(duration - 0.02) < 0.000_01, "System audio overlap duplicated time: \(duration)")
            let decoded = decodedAudio(URL(fileURLWithPath: path))
            for (time, expected) in [(0.004, 0.267), (0.012, 0.667), (0.017, 0.4)] {
                let mean = meanLevel(decoded, at: time, samples: 192)
                precondition(abs(mean - expected) < 0.08, "Unexpected overlap audio at \(time): \(mean)")
            }
            print("System overlap duration: \(duration)")
            return
        }
        if let path = ProcessInfo.processInfo.environment["PORATAKE_TEST_SYSTEM_GAP"] {
            let writer = try! AVAssetWriter(outputURL: URL(fileURLWithPath: path), fileType: .m4a)
            let input = AVAssetWriterInput(mediaType: .audio, outputSettings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 48_000,
                AVNumberOfChannelsKey: 2,
                AVEncoderBitRateKey: 192_000,
            ])
            input.expectsMediaDataInRealTime = true
            precondition(writer.canAdd(input))
            writer.add(input)
            precondition(writer.startWriting())
            writer.startSession(atSourceTime: .zero)
            let recorder = ScreenCaptureRecorder()
            recorder.testConfigureSystemAudio(writer: writer, input: input)
            let sample = audioSample()
            for time in [CMTime(value: 2, timescale: 1), CMTime(value: 201, timescale: 100), CMTime(value: 8, timescale: 1)] {
                recorder.testWriteSystemAudio(sample, at: time)
            }
            try! await recorder.testFinishSystemAudio()
            await writer.finishWriting()
            precondition(writer.status == .completed)
            let duration = CMTimeGetSeconds(AVURLAsset(url: URL(fileURLWithPath: path)).duration)
            precondition(duration >= 8.01 && duration < 8.1, "System audio timeline collapsed: \(duration)")
            verifySystemGap(URL(fileURLWithPath: path))
            print("System audio duration after timestamps 2, 2.01 and 8 seconds: \(duration)")
            return
        }
        if let path = ProcessInfo.processInfo.environment["PORATAKE_TEST_SYSTEM_BACKPRESSURE"] {
            let writer = try! AVAssetWriter(outputURL: URL(fileURLWithPath: path), fileType: .m4a)
            let input = AVAssetWriterInput(mediaType: .audio, outputSettings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 48_000,
                AVNumberOfChannelsKey: 2,
                AVEncoderBitRateKey: 192_000,
            ])
            input.expectsMediaDataInRealTime = true
            precondition(writer.canAdd(input))
            writer.add(input)
            precondition(writer.startWriting())
            writer.startSession(atSourceTime: .zero)
            let recorder = ScreenCaptureRecorder()
            recorder.testConfigureSystemAudio(writer: writer, input: input)
            let sample = audioSample()
            for index in 0..<30_000 {
                recorder.testWriteSystemAudio(sample, at: CMTime(value: Int64(index), timescale: 100))
            }
            let done = DispatchSemaphore(value: 0)
            var failure: Error?
            Task.detached {
                do { try await recorder.testFinishSystemAudio() } catch { failure = error }
                done.signal()
            }
            precondition(done.wait(timeout: .now() + 60) == .success)
            precondition(failure == nil, "\(String(describing: failure))")
            writer.finishWriting { done.signal() }
            precondition(done.wait(timeout: .now() + 60) == .success)
            precondition(writer.status == .completed)
            let duration = CMTimeGetSeconds(AVURLAsset(url: URL(fileURLWithPath: path)).duration)
            precondition(duration >= 300 && duration < 300.1, "Unexpected system audio duration: \(duration)")
            print("Submitted 30000 system-audio buffers; AAC duration \(duration) seconds")
            return
        }
        let oldStream = NSObject()
        let activeStream = NSObject()
        let streamIdentity = ScreenCaptureRecorder()
        streamIdentity.testStartTimeline()
        streamIdentity.testActivateStream(activeStream)
        streamIdentity.testProcessStreamSample(sample(at: 10), type: .screen, stream: oldStream)
        precondition(!streamIdentity.testTimeline.started, "A previous stream must not anchor a new recording")
        streamIdentity.testProcessStreamSample(sample(at: 20), type: .screen, stream: activeStream)
        precondition(CMTimeCompare(streamIdentity.testTimeline.first!, CMTime(value: 20, timescale: 1)) == 0)
        print("Stale stream frame ignored; current stream anchors the timeline")

        let preFramePause = ScreenCaptureRecorder()
        preFramePause.testStartTimeline()
        try! preFramePause.pause()
        preFramePause.testProcessSample(sample(at: 5), type: .audio)
        precondition(preFramePause.testTimeline.pause == nil)
        preFramePause.testProcessSample(sample(at: 10), type: .screen)
        precondition(CMTimeCompare(preFramePause.testTimeline.pause!, CMTime(value: 10, timescale: 1)) == 0)
        try! preFramePause.resume()
        preFramePause.testProcessSample(sample(at: 20), type: .screen)
        precondition(preFramePause.testTimeline.started)
        precondition(CMTimeCompare(preFramePause.testTimeline.pausedDuration, CMTime(value: 10, timescale: 1)) == 0)

        let recorder = ScreenCaptureRecorder()
        recorder.testStartTimeline()
        recorder.testProcessSample(sample(at: 99), type: .audio)
        precondition(!recorder.testTimeline.started)

        let stopped = sample(at: 0)
        let attachments = CMSampleBufferGetSampleAttachmentsArray(stopped, createIfNecessary: true)! as NSArray
        (attachments[0] as! NSMutableDictionary)[SCStreamFrameInfo.status.rawValue] = SCFrameStatus.stopped.rawValue
        recorder.testProcessSample(stopped, type: .screen)
        precondition(!recorder.testTimeline.started)

        recorder.testProcessSample(sample(at: 100), type: .screen)
        precondition(recorder.testTimeline.started)
        precondition(CMTimeCompare(recorder.testTimeline.first!, CMTime(value: 100, timescale: 1)) == 0)

        let cameraRanges = ScreenCaptureRecorder()
        cameraRanges.testStartTimeline()
        cameraRanges.testProcessSample(sample(at: 100), type: .screen)
        cameraRanges.testProcessSample(sample(at: 120), type: .audio)
        cameraRanges.testSetCameraClockTime(CMTime(value: 120, timescale: 1))
        cameraRanges.testOpenCameraRange()
        cameraRanges.testSetCameraClockTime(CMTime(value: 125, timescale: 1))
        cameraRanges.testCloseCameraRange()
        precondition(cameraRanges.testCameraRanges.count == 1)
        precondition(cameraRanges.testCameraRanges[0].start == 20 && cameraRanges.testCameraRanges[0].end == 25,
                     "Camera range must follow the screen clock during a static screen: \(cameraRanges.testCameraRanges)")
        let stoppedCameraRange = ScreenCaptureRecorder()
        stoppedCameraRange.testStartTimeline()
        stoppedCameraRange.testProcessSample(sample(at: 100), type: .screen)
        stoppedCameraRange.testSetCameraClockTime(CMTime(value: 120, timescale: 1))
        stoppedCameraRange.testOpenCameraRange()
        stoppedCameraRange.testSetCameraClockTime(CMTime(value: 150, timescale: 1))
        stoppedCameraRange.testCloseCameraRange(at: 25)
        precondition(stoppedCameraRange.testCameraRanges[0].start == 20 && stoppedCameraRange.testCameraRanges[0].end == 25)
        let pausedCameraRanges = ScreenCaptureRecorder()
        pausedCameraRanges.testStartTimeline()
        pausedCameraRanges.testProcessSample(sample(at: 100), type: .screen)
        pausedCameraRanges.testSetCameraClockTime(CMTime(value: 120, timescale: 1))
        pausedCameraRanges.testOpenCameraRange()
        pausedCameraRanges.testBeginResume(from: CMTime(value: 122, timescale: 1))
        pausedCameraRanges.testSetCameraClockTime(CMTime(value: 125, timescale: 1))
        pausedCameraRanges.testCloseCameraRange()
        pausedCameraRanges.testOpenCameraRange()
        pausedCameraRanges.testProcessSample(sample(at: 130), type: .audio)
        pausedCameraRanges.testSetCameraClockTime(CMTime(value: 133, timescale: 1))
        pausedCameraRanges.testCloseCameraRange()
        precondition(pausedCameraRanges.testCameraRanges.count == 2)
        precondition(pausedCameraRanges.testCameraRanges[0].start == 20 && pausedCameraRanges.testCameraRanges[0].end == 22)
        precondition(pausedCameraRanges.testCameraRanges[1].start == 22 && pausedCameraRanges.testCameraRanges[1].end == 25)
        let invalidCameraClock = ScreenCaptureRecorder()
        invalidCameraClock.testStartTimeline()
        invalidCameraClock.testProcessSample(sample(at: 100), type: .screen)
        invalidCameraClock.testProcessSample(sample(at: 105), type: .screen)
        invalidCameraClock.testSetCameraClockTime(.invalid)
        invalidCameraClock.testOpenCameraRange()
        invalidCameraClock.testSetCameraClockTime(nil)
        invalidCameraClock.testCloseCameraRange()
        precondition(invalidCameraClock.testCameraRanges.count == 1)
        precondition(invalidCameraClock.testCameraRanges[0].start == 5 && invalidCameraClock.testCameraRanges[0].end == 5)
        print("Camera visibility follows the screen clock through static frames and pauses")

        recorder.testBeginResume(from: CMTime(value: 110, timescale: 1))
        recorder.testProcessSample(sample(at: 114), type: .audio)
        precondition(recorder.testTimeline.pause == nil)
        precondition(CMTimeCompare(recorder.testTimeline.pausedDuration, CMTime(value: 4, timescale: 1)) == 0)
        for multipleTimings in [false, true] {
            let audio = audioSample(multipleTimings: multipleTimings)
            let muted = recorder.testMutedSample(audio)!
            let adjusted = recorder.testAdjustedSample(audio, at: CMTime(value: 2, timescale: 1))!
            precondition(CMTimeCompare(CMSampleBufferGetDuration(muted), CMSampleBufferGetDuration(audio)) == 0)
            precondition(CMTimeCompare(CMSampleBufferGetDuration(adjusted), CMSampleBufferGetDuration(audio)) == 0)
            precondition(CMTimeCompare(CMSampleBufferGetPresentationTimeStamp(adjusted), CMTime(value: 2, timescale: 1)) == 0)
            var sourceTiming = CMSampleTimingInfo()
            var outputTiming = CMSampleTimingInfo()
            precondition(CMSampleBufferGetSampleTimingInfo(audio, at: 100, timingInfoOut: &sourceTiming) == noErr)
            precondition(CMSampleBufferGetSampleTimingInfo(adjusted, at: 100, timingInfoOut: &outputTiming) == noErr)
            precondition(CMTimeCompare(outputTiming.presentationTimeStamp, CMTimeAdd(sourceTiming.presentationTimeStamp, CMTime(value: 2, timescale: 1))) == 0)
            var bytes = [UInt8](repeating: 0xff, count: 480 * 4)
            precondition(CMBlockBufferCopyDataBytes(CMSampleBufferGetDataBuffer(muted)!, atOffset: 0, dataLength: bytes.count, destination: &bytes) == noErr)
            precondition(bytes.allSatisfy { $0 == 0 })
            precondition(CMBlockBufferCopyDataBytes(CMSampleBufferGetDataBuffer(audio)!, atOffset: 0, dataLength: bytes.count, destination: &bytes) == noErr)
            precondition(bytes.allSatisfy { $0 == 0x55 })
            let prefix = recorder.testSilencePrefix(muted, fitting: CMTime(value: 577, timescale: 96_000))!
            precondition(CMSampleBufferGetNumSamples(prefix) == 288)
            precondition(CMTimeCompare(CMSampleBufferGetDuration(prefix), CMTime(value: 288, timescale: 48_000)) == 0)
            precondition(recorder.testSilencePrefix(muted, fitting: CMTime(value: 1, timescale: 96_000)) == nil)
        }
        print("PCM timing and mute checks passed for single and per-sample timing")
        let path = ProcessInfo.processInfo.environment["PORATAKE_TEST_AUDIO_OUTPUT"]
        let output = URL(fileURLWithPath: path ?? NSTemporaryDirectory() + "poratake-audio-\(UUID().uuidString).m4a")
        defer { if path == nil { try? FileManager.default.removeItem(at: output) } }
        let writer = try! AVAssetWriter(outputURL: output, fileType: .m4a)
        let input = AVAssetWriterInput(mediaType: .audio, outputSettings: [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: 48_000,
            AVNumberOfChannelsKey: 2,
            AVEncoderBitRateKey: 192_000,
        ])
        input.expectsMediaDataInRealTime = true
        precondition(writer.canAdd(input))
        writer.add(input)
        precondition(writer.startWriting())
        writer.startSession(atSourceTime: .zero)
        let microphone = ScreenCaptureRecorder()
        microphone.testConfigureMicrophone(writer: writer, input: input)
        precondition(input.isReadyForMoreMediaData)
        microphone.testWriteMicrophone(audioSample(), pendingStart: .zero)
        microphone.testWriteMicrophone(audioSample(), pendingStart: CMTime(value: 13, timescale: 500))
        finishMicrophone(microphone)
        let writes = microphone.testMicrophoneWrites
        print("Microphone sample intervals: \(writes.map { (CMTimeGetSeconds($0.0), CMTimeGetSeconds($0.1)) })")
        precondition(writes.count == 4)
        let expected: [(Int64, Int64)] = [(0, 480), (480, 480), (960, 288), (1248, 480)]
        for (write, (start, length)) in zip(writes, expected) {
            precondition(CMTimeCompare(write.0, CMTime(value: start, timescale: 48_000)) == 0)
            precondition(CMTimeCompare(write.1, CMTime(value: length, timescale: 48_000)) == 0)
        }
        let done = DispatchSemaphore(value: 0)
        writer.finishWriting { done.signal() }
        precondition(done.wait(timeout: .now() + 15) == .success)
        precondition(writer.status == .completed, "\(String(describing: writer.error))")
        print("Microphone AAC writer completed without overlapping sample buffers")

        let staticPath = ProcessInfo.processInfo.environment["PORATAKE_TEST_STATIC_AUDIO_OUTPUT"]
        let staticOutput = URL(fileURLWithPath: staticPath ?? NSTemporaryDirectory() + "poratake-static-audio-\(UUID().uuidString).m4a")
        defer { if staticPath == nil { try? FileManager.default.removeItem(at: staticOutput) } }
        let staticWriter = try! AVAssetWriter(outputURL: staticOutput, fileType: .m4a)
        let staticInput = AVAssetWriterInput(mediaType: .audio, outputSettings: [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: 48_000,
            AVNumberOfChannelsKey: 2,
        ])
        staticInput.expectsMediaDataInRealTime = true
        staticWriter.add(staticInput)
        precondition(staticWriter.startWriting())
        staticWriter.startSession(atSourceTime: .zero)
        let staticRecorder = ScreenCaptureRecorder()
        do {
            try staticRecorder.testFailedMicrophoneSetup()
            preconditionFailure("Synthetic microphone setup should fail")
        } catch {
            precondition(staticRecorder.testMicrophonePending == nil)
        }
        staticRecorder.testStartTimeline()
        staticRecorder.testProcessSample(sample(at: 100), type: .screen)
        staticRecorder.testConfigureMicrophone(writer: staticWriter, input: staticInput)
        staticRecorder.testProcessSample(sample(at: 120), type: .screen)
        try! staticRecorder.testEnableMicrophoneDuringSetup(audioSample())
        print("Microphone setup callback ended at \(CMTimeGetSeconds(staticRecorder.testMicrophoneEnd!)) seconds")
        precondition(CMTimeCompare(staticRecorder.testMicrophoneEnd!, CMTime(value: 2001, timescale: 100)) == 0)
        staticRecorder.testProcessSample(sample(at: 121), type: .screen)
        for tick in 1...100 {
            staticRecorder.testContinueMicrophone(staticRecorder.testAdjustedSample(audioSample(), at: CMTime(value: Int64(tick), timescale: 100))!)
        }
        finishMicrophone(staticRecorder)
        let staticStart = staticRecorder.testMicrophoneWrites[2000].0
        print("Static-screen microphone starts at \(CMTimeGetSeconds(staticStart)) seconds")
        precondition(CMTimeCompare(staticStart, CMTime(value: 20, timescale: 1)) == 0)
        precondition(CMTimeCompare(staticRecorder.testMicrophoneWrites.last!.0, CMTime(value: 21, timescale: 1)) == 0)
        let staticDone = DispatchSemaphore(value: 0)
        staticWriter.finishWriting { staticDone.signal() }
        precondition(staticDone.wait(timeout: .now() + 15) == .success)
        precondition(staticWriter.status == .completed)

        if let gapProbe = ProcessInfo.processInfo.environment["PORATAKE_TEST_AUDIO_GAP_PROBE"] {
            let probeWriter = try! AVAssetWriter(outputURL: URL(fileURLWithPath: gapProbe), fileType: .m4a)
            let probeInput = AVAssetWriterInput(mediaType: .audio, outputSettings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 48_000,
                AVNumberOfChannelsKey: 2,
                AVEncoderBitRateKey: 192_000,
            ])
            probeInput.expectsMediaDataInRealTime = true
            probeWriter.add(probeInput)
            precondition(probeWriter.startWriting())
            probeWriter.startSession(atSourceTime: .zero)
            precondition(probeInput.append(audioSample()))
            precondition(probeInput.append(recorder.testAdjustedSample(audioSample(), at: CMTime(value: 8, timescale: 1))!))
            probeInput.markAsFinished()
            let probeDone = DispatchSemaphore(value: 0)
            probeWriter.finishWriting { probeDone.signal() }
            precondition(probeDone.wait(timeout: .now() + 15) == .success)
            precondition(probeWriter.status == .completed)
        }

        if let probePath = ProcessInfo.processInfo.environment["PORATAKE_TEST_BACKPRESSURE_PROBE"] {
            let probeWriter = try! AVAssetWriter(outputURL: URL(fileURLWithPath: probePath), fileType: .m4a)
            let probeInput = AVAssetWriterInput(mediaType: .audio, outputSettings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 48_000,
                AVNumberOfChannelsKey: 2,
                AVEncoderBitRateKey: 192_000,
            ])
            probeInput.expectsMediaDataInRealTime = true
            probeWriter.add(probeInput)
            precondition(probeWriter.startWriting())
            probeWriter.startSession(atSourceTime: .zero)
            let signal = DispatchSemaphore(value: 0)
            let queue = DispatchQueue(label: "poratake.test.backpressure")
            let active = audioSample()
            let silent = recorder.testMutedSample(active)!
            var index = 0
            var calls = 0
            var failed = false
            probeInput.requestMediaDataWhenReady(on: queue) {
                calls += 1
                while probeInput.isReadyForMoreMediaData && index <= 30_000 {
                    let source = index == 0 || index == 30_000 ? active : silent
                    guard let sample = recorder.testAdjustedSample(source, at: CMTime(value: Int64(index * 480), timescale: 48_000)),
                          probeInput.append(sample) else {
                        failed = true
                        probeInput.markAsFinished()
                        probeWriter.finishWriting { signal.signal() }
                        return
                    }
                    index += 1
                }
                if index > 30_000 {
                    probeInput.markAsFinished()
                    probeWriter.finishWriting { signal.signal() }
                }
            }
            precondition(signal.wait(timeout: .now() + 60) == .success)
            precondition(!failed && probeWriter.status == .completed && index == 30_001)
            print("Readiness-driven AAC probe wrote \(index) buffers in \(calls) callbacks")
            return
        }

        if let probePath = ProcessInfo.processInfo.environment["PORATAKE_TEST_BACKPRESSURE_KVO"] {
            let probeWriter = try! AVAssetWriter(outputURL: URL(fileURLWithPath: probePath), fileType: .m4a)
            let probeInput = AVAssetWriterInput(mediaType: .audio, outputSettings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 48_000,
                AVNumberOfChannelsKey: 2,
                AVEncoderBitRateKey: 192_000,
            ])
            probeInput.expectsMediaDataInRealTime = true
            probeWriter.add(probeInput)
            precondition(probeWriter.startWriting())
            probeWriter.startSession(atSourceTime: .zero)
            let signal = DispatchSemaphore(value: 0)
            let queue = DispatchQueue(label: "poratake.test.backpressure.kvo")
            let active = audioSample()
            let silent = recorder.testMutedSample(active)!
            var index = 0
            var available = 0
            var finished = false
            var callbacks = 0
            var failed = false
            var drain: (() -> Void)!
            drain = {
                callbacks += 1
                while probeInput.isReadyForMoreMediaData && index < available {
                    let source = index == 0 || index >= 30_000 ? active : silent
                    guard let sample = recorder.testAdjustedSample(source, at: CMTime(value: Int64(index * 480), timescale: 48_000)),
                          probeInput.append(sample) else {
                        failed = true
                        probeInput.markAsFinished()
                        probeWriter.finishWriting { signal.signal() }
                        return
                    }
                    index += 1
                }
                if finished && index == available {
                    finished = false
                    probeInput.markAsFinished()
                    probeWriter.finishWriting { signal.signal() }
                }
            }
            let observer = probeInput.observe(\.isReadyForMoreMediaData, options: [.new]) { input, _ in
                if input.isReadyForMoreMediaData { queue.async { drain() } }
            }
            queue.async {
                available = 30_000
                drain()
            }
            queue.async {
                available = 30_003
                finished = true
                drain()
            }
            precondition(signal.wait(timeout: .now() + 60) == .success)
            observer.invalidate()
            precondition(!failed && probeWriter.status == .completed && index == 30_003)
            print("KVO readiness AAC probe drained \(index) buffers, including three queued resumed buffers, in \(callbacks) drains")
            return
        }

        let longPath = ProcessInfo.processInfo.environment["PORATAKE_TEST_LONG_AUDIO_OUTPUT"]
        let longOutput = URL(fileURLWithPath: longPath ?? NSTemporaryDirectory() + "poratake-long-audio-\(UUID().uuidString).m4a")
        defer { if longPath == nil { try? FileManager.default.removeItem(at: longOutput) } }
        let longWriter = try! AVAssetWriter(outputURL: longOutput, fileType: .m4a)
        let longInput = AVAssetWriterInput(mediaType: .audio, outputSettings: [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: 48_000,
            AVNumberOfChannelsKey: 2,
            AVEncoderBitRateKey: 192_000,
        ])
        longInput.expectsMediaDataInRealTime = true
        precondition(longWriter.canAdd(longInput))
        longWriter.add(longInput)
        precondition(longWriter.startWriting())
        longWriter.startSession(atSourceTime: .zero)
        let longMicrophone = ScreenCaptureRecorder()
        longMicrophone.testConfigureMicrophone(writer: longWriter, input: longInput)
        longMicrophone.testWriteMicrophone(audioSample(), pendingStart: .zero)
        let gap = Int64(ProcessInfo.processInfo.environment["PORATAKE_TEST_LONG_GAP_SECONDS"] ?? "8")!
        precondition(gap > 0)
        longMicrophone.testWriteMicrophone(audioSample(), pendingStart: CMTime(value: gap, timescale: 1))
        longMicrophone.testWriteMicrophone(audioSample(), pendingStart: CMTime(value: gap * 100 + 1, timescale: 100))
        longMicrophone.testWriteMicrophone(audioSample(), pendingStart: CMTime(value: gap * 100 + 2, timescale: 100))
        finishMicrophone(longMicrophone)
        let longWrites = longMicrophone.testMicrophoneWrites
        print("Long-gap audio: \(longWrites.count) buffers, last end \(longWrites.last.map { CMTimeGetSeconds(CMTimeAdd($0.0, $0.1)) } ?? -1)")
        precondition(longWrites.count == Int(gap) * 100 + 3)
        for (previous, next) in zip(longWrites, longWrites.dropFirst()) {
            precondition(CMTimeCompare(CMTimeAdd(previous.0, previous.1), next.0) == 0)
        }
        let longDone = DispatchSemaphore(value: 0)
        longWriter.finishWriting { longDone.signal() }
        precondition(longDone.wait(timeout: .now() + 15) == .success)
        precondition(longWriter.status == .completed)

        let cancelledOutput = URL(fileURLWithPath: NSTemporaryDirectory() + "poratake-cancel-audio-\(UUID().uuidString).m4a")
        defer { try? FileManager.default.removeItem(at: cancelledOutput) }
        let cancelledWriter = try! AVAssetWriter(outputURL: cancelledOutput, fileType: .m4a)
        let cancelledInput = AVAssetWriterInput(mediaType: .audio, outputSettings: [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: 48_000,
            AVNumberOfChannelsKey: 2,
        ])
        cancelledInput.expectsMediaDataInRealTime = true
        cancelledWriter.add(cancelledInput)
        precondition(cancelledWriter.startWriting())
        cancelledWriter.startSession(atSourceTime: .zero)
        let cancelledMicrophone = ScreenCaptureRecorder()
        cancelledMicrophone.testConfigureMicrophone(writer: cancelledWriter, input: cancelledInput)
        cancelledWriter.cancelWriting()
        let cancelledDone = DispatchSemaphore(value: 0)
        var cancellationReported = false
        Task.detached {
            do { try await cancelledMicrophone.testFinishMicrophone() } catch { cancellationReported = true }
            cancelledDone.signal()
        }
        precondition(cancelledDone.wait(timeout: .now() + 15) == .success)
        precondition(cancellationReported)
        print("Cancelled microphone writer reported failure without hanging")

        let cancelledSystem = ScreenCaptureRecorder()
        cancelledSystem.testConfigureSystemAudio(writer: cancelledWriter, input: cancelledInput)
        let cancelledSystemDone = DispatchSemaphore(value: 0)
        var systemCancellationReported = false
        Task.detached {
            do { try await cancelledSystem.testFinishSystemAudio() } catch { systemCancellationReported = true }
            cancelledSystemDone.signal()
        }
        precondition(cancelledSystemDone.wait(timeout: .now() + 15) == .success)
        precondition(systemCancellationReported)
        print("Cancelled system audio writer reported failure without hanging")

        let recoveryPath = ProcessInfo.processInfo.environment["PORATAKE_TEST_RECOVERY_OUTPUT"]
        let recoveryOutput = URL(fileURLWithPath: recoveryPath ?? NSTemporaryDirectory() + "poratake-recovery-\(UUID().uuidString).mov")
        defer { if recoveryPath == nil { try? FileManager.default.removeItem(at: recoveryOutput) } }
        let recoveryWriter = try! AVAssetWriter(outputURL: recoveryOutput, fileType: .mov)
        let recoveryInput = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: 64,
            AVVideoHeightKey: 64,
        ])
        let recoveryAdaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: recoveryInput, sourcePixelBufferAttributes: [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: 64,
            kCVPixelBufferHeightKey as String: 64,
        ])
        recoveryWriter.add(recoveryInput)
        precondition(recoveryWriter.startWriting())
        recoveryWriter.startSession(atSourceTime: .zero)
        var recoveryFrame: CVPixelBuffer?
        precondition(CVPixelBufferCreate(nil, 64, 64, kCVPixelFormatType_32BGRA, nil, &recoveryFrame) == kCVReturnSuccess)
        precondition(recoveryAdaptor.append(recoveryFrame!, withPresentationTime: .zero))
        let recovery = ScreenCaptureRecorder()
        recovery.testConfigureStop(writer: recoveryWriter, input: recoveryInput)
        recovery.testConfigureSystemAudio(writer: cancelledWriter, input: cancelledInput)
        var stopFailed = false
        do { _ = try await recovery.stop() } catch { stopFailed = true }
        precondition(stopFailed)
        precondition(recoveryWriter.status == .completed, "Primary writer was not finalized after audio failure")
        print("Primary writer finalized after system audio failure")

        let cameraRecoveryOutput = URL(fileURLWithPath: NSTemporaryDirectory() + "poratake-camera-recovery-\(UUID().uuidString).mov")
        let failedCameraOutput = URL(fileURLWithPath: NSTemporaryDirectory() + "poratake-camera-cancelled-\(UUID().uuidString).mov")
        defer {
            try? FileManager.default.removeItem(at: cameraRecoveryOutput)
            try? FileManager.default.removeItem(at: failedCameraOutput)
        }
        let mainWriter = try! AVAssetWriter(outputURL: cameraRecoveryOutput, fileType: .mov)
        let mainInput = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: 64,
            AVVideoHeightKey: 64,
        ])
        let mainAdaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: mainInput, sourcePixelBufferAttributes: [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: 64,
            kCVPixelBufferHeightKey as String: 64,
        ])
        mainWriter.add(mainInput)
        precondition(mainWriter.startWriting())
        mainWriter.startSession(atSourceTime: .zero)
        precondition(mainAdaptor.append(recoveryFrame!, withPresentationTime: .zero))
        let failedCameraWriter = try! AVAssetWriter(outputURL: failedCameraOutput, fileType: .mov)
        let failedCameraInput = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: 64,
            AVVideoHeightKey: 64,
        ])
        failedCameraWriter.add(failedCameraInput)
        precondition(failedCameraWriter.startWriting())
        failedCameraWriter.startSession(atSourceTime: .zero)
        failedCameraWriter.cancelWriting()
        let cameraRecovery = ScreenCaptureRecorder()
        cameraRecovery.testConfigureStop(writer: mainWriter, input: mainInput)
        cameraRecovery.testConfigureCamera(writer: failedCameraWriter, input: failedCameraInput, path: failedCameraOutput.path)
        let recovered = try! await cameraRecovery.stop()
        precondition(recovered.cameraPath == nil)
        precondition(recovered.cameraTrackMissing)
        precondition(mainWriter.status == .completed)
        precondition(!FileManager.default.fileExists(atPath: failedCameraOutput.path))
        let recoveredAsset = AVURLAsset(url: cameraRecoveryOutput)
        let recoveredTrack = recoveredAsset.tracks(withMediaType: .video).first!
        let recoveredReader = try! AVAssetReader(asset: recoveredAsset)
        let recoveredOutput = AVAssetReaderTrackOutput(track: recoveredTrack, outputSettings: [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
        ])
        precondition(recoveredReader.canAdd(recoveredOutput))
        recoveredReader.add(recoveredOutput)
        precondition(recoveredReader.startReading())
        precondition(recoveredOutput.copyNextSampleBuffer() != nil)
        print("Primary frame decoded after camera cancellation; camera track omitted")

        let overflowOutput = URL(fileURLWithPath: NSTemporaryDirectory() + "poratake-overflow-audio-\(UUID().uuidString).m4a")
        defer { try? FileManager.default.removeItem(at: overflowOutput) }
        let overflowWriter = try! AVAssetWriter(outputURL: overflowOutput, fileType: .m4a)
        let overflowInput = AVAssetWriterInput(mediaType: .audio, outputSettings: [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: 48_000,
            AVNumberOfChannelsKey: 2,
        ])
        overflowWriter.add(overflowInput)
        precondition(overflowWriter.startWriting())
        overflowWriter.startSession(atSourceTime: .zero)
        let overflowDone = DispatchSemaphore(value: 0)
        let overflowQueue = AudioBufferQueue(
            writer: overflowWriter, input: overflowInput, maximumQueuedBytes: 1,
            prefix: { _, _ in nil }, adjust: { _, _ in nil }, onWrite: { _ in },
            onFailure: { _ in overflowDone.signal() }
        )
        precondition(!overflowQueue.enqueue(audioSample()))
        precondition(overflowDone.wait(timeout: .now() + 15) == .success)
        overflowQueue.cancel()
        overflowWriter.cancelWriting()
        print("Microphone queue overflow reported failure")
        print("Recording timeline startup and static-screen resume passed")
    }
}
