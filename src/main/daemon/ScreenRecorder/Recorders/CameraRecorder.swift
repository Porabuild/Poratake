import AVFoundation
import AppKit
import CoreMedia
import Foundation

class CameraRecorder: NSObject, PausableRecorder, AVCaptureVideoDataOutputSampleBufferDelegate {
    private var captureSession: AVCaptureSession?
    private var assetWriter: AVAssetWriter?
    private var videoInput: AVAssetWriterInput?
    private var pixelBufferAdaptor: AVAssetWriterInputPixelBufferAdaptor?

    private let captureQueue = DispatchQueue(label: "com.porabuild.poratake.camera-recorder.capture")
    private let writerQueue = DispatchQueue(label: "com.porabuild.poratake.camera-recorder.writer")

    private var sessionStarted = false
    private var hasWrittenFrame = false
    private var anchorWritten = false
    private var firstFrameTime: CMTime?
    private var lastFrameTime: CMTime = .zero
    private var lastOriginalTime: CMTime?
    private var accumulateGapOnNextFrame = false
    private var totalPauseDuration: CMTime = .zero
    private var screenTimeline: (origin: CMTime, clock: CMClock, cameraClock: CMClock, pause: CMTime)?

    private(set) var isPaused = false
    private(set) var isSuspended = false
    private(set) var isRecording = false
    private var outputPath: String?
    private var metadataPath: String?

    private var deviceId: String?
    private var deviceName: String?
    private var frameRate: Int = 30
    private var recordingWidth: Int = 1280
    private var recordingHeight: Int = 720
    private var visibleRanges: [(start: Double, end: Double)] = []
    
    private var isSynced: Bool = false
    private var syncTime: CMTime?
    private let syncQueue = DispatchQueue(label: "com.porabuild.poratake.camera-recorder.sync")
    private var pendingBuffers: [(buffer: CMSampleBuffer, time: CMTime)] = []
    private var syncOffsetMs: Double = 0

    func configure(
        deviceId: String?,
        deviceName: String?,
        frameRate: Int,
        outputPath: String
    ) {
        self.deviceId = deviceId
        self.deviceName = deviceName
        self.frameRate = frameRate
        self.outputPath = outputPath

        let videoDir = (outputPath as NSString).deletingLastPathComponent
        self.metadataPath = (videoDir as NSString).appendingPathComponent("camera.json")
    }

    func syncWithVideoStart() {
        syncQueue.sync {
            guard !isSynced else { return }
            
            if let firstFrame = firstFrameTime, let lastPending = pendingBuffers.last {
                let offsetSeconds = CMTimeGetSeconds(CMTimeSubtract(lastPending.time, firstFrame))
                syncOffsetMs = offsetSeconds * 1000
            }
            
            isSynced = true
            firstFrameTime = nil
            lastFrameTime = .zero
            pendingBuffers.removeAll()
        }
    }

    func syncWithScreenStart(origin: CMTime, clock: CMClock, pause: CMTime) -> Bool {
        guard let cameraClock = captureSession?.synchronizationClock else { return false }
        syncWithScreenStart(origin: origin, clock: clock, cameraClock: cameraClock, pause: pause)
        return true
    }

    private func syncWithScreenStart(origin: CMTime, clock: CMClock, cameraClock: CMClock, pause: CMTime) {
        syncQueue.sync {
            guard !isSynced else { return }
            screenTimeline = (origin, clock, cameraClock, pause)
            isSynced = true
            firstFrameTime = nil
            lastFrameTime = .zero
            accumulateGapOnNextFrame = false
            pendingBuffers.removeAll()
        }
    }

    func resumeWithScreenPause(_ pause: CMTime) {
        syncQueue.sync {
            guard isRecording, isPaused else { return }
            screenTimeline?.pause = pause
            isPaused = false
        }
    }
    
    func start(paused: Bool = false) throws {
        guard !isRecording else { return }
        guard let outputPath = outputPath else {
            throw RecorderError.configuration("Camera output path not configured")
        }

        let fileManager = FileManager.default
        if fileManager.fileExists(atPath: outputPath) {
            try fileManager.removeItem(atPath: outputPath)
        }

        var cameraDevice: AVCaptureDevice?

        let discoverySession: AVCaptureDevice.DiscoverySession
        if #available(macOS 14.0, *) {
            discoverySession = AVCaptureDevice.DiscoverySession(
                deviceTypes: [.builtInWideAngleCamera, .external, .continuityCamera],
                mediaType: .video,
                position: .unspecified
            )
        } else {
            discoverySession = AVCaptureDevice.DiscoverySession(
                deviceTypes: [.builtInWideAngleCamera, .externalUnknown],
                mediaType: .video,
                position: .unspecified
            )
        }

        if let deviceName = deviceName {
            cameraDevice = discoverySession.devices.first { $0.localizedName == deviceName }
            if cameraDevice == nil {
                cameraDevice = discoverySession.devices.first { device in
                    deviceName.contains(device.localizedName) || device.localizedName.contains(deviceName)
                }
            }
        }

        if cameraDevice == nil, let deviceId = deviceId {
            cameraDevice = discoverySession.devices.first { $0.uniqueID == deviceId }
        }

        if cameraDevice == nil {
            cameraDevice = AVCaptureDevice.default(for: .video)
        }

        guard let camera = cameraDevice else {
            throw RecorderError.configuration("No camera device found")
        }

        self.deviceName = camera.localizedName
        self.deviceId = camera.uniqueID

        captureSession = AVCaptureSession()
        captureSession?.sessionPreset = .high

        let cameraInput = try AVCaptureDeviceInput(device: camera)
        if captureSession?.canAddInput(cameraInput) == true {
            captureSession?.addInput(cameraInput)
        } else {
            throw RecorderError.configuration("Cannot add camera input to capture session")
        }

        if let format = camera.activeFormat.formatDescription as CMFormatDescription? {
            let dimensions = CMVideoFormatDescriptionGetDimensions(format)
            recordingWidth = Int(dimensions.width)
            recordingHeight = Int(dimensions.height)
        }

        recordingWidth = (recordingWidth / 2) * 2
        recordingHeight = (recordingHeight / 2) * 2

        let videoOutput = AVCaptureVideoDataOutput()
        videoOutput.videoSettings = [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA
        ]
        videoOutput.setSampleBufferDelegate(self, queue: captureQueue)
        videoOutput.alwaysDiscardsLateVideoFrames = true

        if captureSession?.canAddOutput(videoOutput) == true {
            captureSession?.addOutput(videoOutput)
        } else {
            throw RecorderError.configuration("Cannot add video output to capture session")
        }

        let outputURL = URL(fileURLWithPath: outputPath)
        assetWriter = try AVAssetWriter(outputURL: outputURL, fileType: .mov)

        let bitrate = recordingWidth * recordingHeight * 8
        let videoSettings: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: recordingWidth,
            AVVideoHeightKey: recordingHeight,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: bitrate,
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
                AVVideoMaxKeyFrameIntervalKey: frameRate,
            ]
        ]

        videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: videoSettings)
        videoInput?.expectsMediaDataInRealTime = true
        videoInput?.transform = CGAffineTransform(scaleX: -1, y: 1)

        let sourcePixelBufferAttributes: [String: Any] = [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: recordingWidth,
            kCVPixelBufferHeightKey as String: recordingHeight,
        ]

        pixelBufferAdaptor = AVAssetWriterInputPixelBufferAdaptor(
            assetWriterInput: videoInput!,
            sourcePixelBufferAttributes: sourcePixelBufferAttributes
        )

        if let videoInput = videoInput, assetWriter?.canAdd(videoInput) == true {
            assetWriter?.add(videoInput)
        }

        guard assetWriter?.startWriting() == true else {
            throw RecorderError.capture(
                "Failed to start camera asset writer: \(assetWriter?.error?.localizedDescription ?? "unknown")"
            )
        }

        isRecording = true
        isPaused = paused
        isSuspended = false
        sessionStarted = false
        hasWrittenFrame = false
        anchorWritten = false
        firstFrameTime = nil
        lastFrameTime = .zero
        lastOriginalTime = nil
        accumulateGapOnNextFrame = false
        totalPauseDuration = .zero
        screenTimeline = nil
        visibleRanges = []

        isSynced = false
        syncTime = nil
        pendingBuffers = []
        syncOffsetMs = 0

        captureSession?.startRunning()
    }

    func pause() {
        syncQueue.sync {
            guard isRecording, !isPaused else { return }
            isPaused = true
            if screenTimeline == nil {
                accumulateGapOnNextFrame = true
            }
        }
    }

    func resume() {
        syncQueue.sync {
            guard isRecording, isPaused else { return }
            isPaused = false
        }
    }

    func suspend() {
        syncQueue.sync {
            guard isRecording, !isSuspended else { return }
            isSuspended = true
            if screenTimeline == nil {
                accumulateGapOnNextFrame = true
            }
        }
    }

    func unsuspend() {
        syncQueue.sync {
            guard isRecording, isSuspended else { return }
            isSuspended = false
        }
    }

    func setVisibleRanges(_ ranges: [(start: Double, end: Double)]) {
        visibleRanges = ranges
    }

    func stop() -> (videoPath: String?, metadataPath: String?)? {
        guard isRecording else { return nil }

        syncQueue.sync {
            isRecording = false
            isPaused = false
            isSuspended = false
        }

        if let session = captureSession {
            session.stopRunning()
            for input in session.inputs {
                session.removeInput(input)
            }
            for output in session.outputs {
                session.removeOutput(output)
            }
        }
        captureSession = nil

        captureQueue.sync {}
        writerQueue.sync {}

        if !hasWrittenFrame {
            abort()
            return (nil, nil)
        }

        guard assetWriter?.status == .writing else {
            abort()
            return (nil, nil)
        }

        videoInput?.markAsFinished()

        let duration = CMTimeGetSeconds(lastFrameTime)

        let semaphore = DispatchSemaphore(value: 0)
        assetWriter?.finishWriting {
            semaphore.signal()
        }
        semaphore.wait()

        guard assetWriter?.status == .completed else {
            abort()
            return (nil, nil)
        }

        let finalOutputPath = outputPath
        let finalMetadataPath = metadataPath
        outputPath = nil
        metadataPath = nil

        if let metaPath = finalMetadataPath,
           let videoPath = finalOutputPath,
           let deviceName = deviceName {
            let formatter = ISO8601DateFormatter()
            formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
            let startTimeStr = formatter.string(from: Date())

            let videoFilename = (videoPath as NSString).lastPathComponent

            var rangesJson = ""
            if !visibleRanges.isEmpty {
                let spans = visibleRanges
                    .map { range in
                        String(
                            format: "{\"start\": %.3f, \"end\": %.3f}",
                            range.start,
                            range.end
                        )
                    }
                    .joined(separator: ", ")
                rangesJson = ",\n  \"visibleRanges\": [\(spans)]"
            }

            let metadata = """
            {
              "videoFile": "\(videoFilename)",
              "meta": {
                "deviceId": "\(deviceId ?? "")",
                "deviceName": "\(deviceName)",
                "width": \(recordingWidth),
                "height": \(recordingHeight),
                "duration": \(String(format: "%.3f", duration)),
                "startTime": "\(startTimeStr)",
                "frameRate": \(frameRate),
                "syncOffsetMs": \(String(format: "%.1f", syncOffsetMs)),
                "synced": \(isSynced)\(rangesJson)
              }
            }
            """

            try? metadata.write(toFile: metaPath, atomically: true, encoding: .utf8)
        }

        assetWriter = nil
        videoInput = nil
        pixelBufferAdaptor = nil

        return (finalOutputPath, finalMetadataPath)
    }

    func abort() {
        syncQueue.sync {
            isRecording = false
            isPaused = false
            isSuspended = false
        }

        if let session = captureSession {
            session.stopRunning()
            for input in session.inputs {
                session.removeInput(input)
            }
            for output in session.outputs {
                session.removeOutput(output)
            }
        }
        captureSession = nil

        captureQueue.sync {}
        writerQueue.sync {}

        if assetWriter?.status == .writing {
            assetWriter?.cancelWriting()
        }
        assetWriter = nil
        videoInput = nil
        pixelBufferAdaptor = nil

        syncQueue.sync {
            isSynced = false
            screenTimeline = nil
            syncTime = nil
            pendingBuffers.removeAll()
            syncOffsetMs = 0
        }

        if let outputPath, FileManager.default.fileExists(atPath: outputPath) {
            try? FileManager.default.removeItem(atPath: outputPath)
        }
        if let metadataPath, FileManager.default.fileExists(atPath: metadataPath) {
            try? FileManager.default.removeItem(atPath: metadataPath)
        }
        outputPath = nil
        metadataPath = nil
    }

    func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        let currentTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)

        syncQueue.sync {
            guard isRecording, !isPaused, !isSuspended else { return }
            if firstFrameTime == nil && !isSynced {
                firstFrameTime = currentTime
            }
            
            if isSynced {
                processBuffer(sampleBuffer, originalTime: currentTime)
            } else {
                if pendingBuffers.count > 5 {
                    pendingBuffers.removeFirst()
                }
                pendingBuffers.append((buffer: sampleBuffer, time: currentTime))
            }
        }
    }

    private func processBuffer(_ sampleBuffer: CMSampleBuffer, originalTime: CMTime) {
        let isFirstFrame = !sessionStarted
        let firstTime = firstFrameTime ?? originalTime

        if accumulateGapOnNextFrame, let lastOriginal = lastOriginalTime {
            let pauseDuration = CMTimeSubtract(originalTime, lastOriginal)
            if CMTimeCompare(pauseDuration, .zero) > 0 {
                totalPauseDuration = CMTimeAdd(totalPauseDuration, pauseDuration)
            }
            accumulateGapOnNextFrame = false
        }
        lastOriginalTime = originalTime

        var presentationTime: CMTime
        if let timeline = screenTimeline {
            let screenTime = CMSyncConvertTime(originalTime, from: timeline.cameraClock, to: timeline.clock)
            presentationTime = CMTimeSubtract(CMTimeSubtract(screenTime, timeline.origin), timeline.pause)
        } else {
            presentationTime = CMTimeSubtract(originalTime, firstTime)
            presentationTime = CMTimeSubtract(presentationTime, totalPauseDuration)
        }

        guard presentationTime.isNumeric else { return }
        if CMTimeCompare(presentationTime, .zero) < 0 && screenTimeline != nil {
            return
        }
        if CMTimeCompare(presentationTime, .zero) < 0 {
            presentationTime = .zero
        }

        if hasWrittenFrame && CMTimeCompare(presentationTime, lastFrameTime) <= 0 {
            return
        }

        guard let videoInput = videoInput,
              let pixelBufferAdaptor = pixelBufferAdaptor,
              videoInput.isReadyForMoreMediaData,
              let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer)
        else { return }

        writerQueue.sync {
            guard assetWriter?.status == .writing else { return }
            if isFirstFrame {
                firstFrameTime = originalTime
                assetWriter?.startSession(atSourceTime: .zero)
                sessionStarted = true
            }
            if !hasWrittenFrame && !anchorWritten && CMTimeCompare(presentationTime, .zero) > 0 {
                guard pixelBufferAdaptor.append(imageBuffer, withPresentationTime: .zero) else { return }
                anchorWritten = true
            }
            if pixelBufferAdaptor.append(imageBuffer, withPresentationTime: presentationTime) {
                hasWrittenFrame = true
                lastFrameTime = presentationTime
            }
        }
    }

    #if RECORDING_TIMELINE_TEST
    func testConfigureWriter(_ writer: AVAssetWriter, input: AVAssetWriterInput, path: String) {
        assetWriter = writer
        videoInput = input
        outputPath = path
        isRecording = true
        hasWrittenFrame = true
    }
    #endif
}
