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
    private var firstFrameTime: CMTime?
    private var lastFrameTime: CMTime = .zero
    private var pauseStartTime: CMTime?
    private var totalPauseDuration: CMTime = .zero

    private(set) var isPaused = false
    private(set) var isRecording = false
    private var outputPath: String?
    private var metadataPath: String?

    private var deviceId: String?
    private var deviceName: String?
    private var frameRate: Int = 30
    private var recordingWidth: Int = 1280
    private var recordingHeight: Int = 720
    
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
    
    func start() throws {
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

        captureSession?.startRunning()

        isRecording = true
        isPaused = false
        sessionStarted = false
        firstFrameTime = nil
        pauseStartTime = nil
        totalPauseDuration = .zero
        
        isSynced = false
        syncTime = nil
        pendingBuffers = []
        syncOffsetMs = 0
    }

    func pause() {
        guard isRecording, !isPaused else { return }
        isPaused = true
        pauseStartTime = lastFrameTime
    }

    func resume() {
        guard isRecording, isPaused else { return }
        isPaused = false
    }

    func stop() -> (videoPath: String?, metadataPath: String?)? {
        guard isRecording else { return nil }

        isRecording = false
        isPaused = false

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

        videoInput?.markAsFinished()

        let finalOutputPath = outputPath
        let finalMetadataPath = metadataPath
        outputPath = nil
        metadataPath = nil
        let duration = CMTimeGetSeconds(lastFrameTime)

        if assetWriter?.status == .writing {
            let semaphore = DispatchSemaphore(value: 0)
            assetWriter?.finishWriting {
                semaphore.signal()
            }
            semaphore.wait()
        }

        if let metaPath = finalMetadataPath,
           let videoPath = finalOutputPath,
           let deviceName = deviceName {
            let formatter = ISO8601DateFormatter()
            formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
            let startTimeStr = formatter.string(from: Date())

            let videoFilename = (videoPath as NSString).lastPathComponent

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
                "synced": \(isSynced)
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
        isRecording = false
        isPaused = false

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
        guard isRecording, !isPaused else { return }

        let currentTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)

        syncQueue.sync {
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
        if isFirstFrame {
            firstFrameTime = originalTime
            assetWriter?.startSession(atSourceTime: .zero)
            sessionStarted = true
        }

        guard let firstTime = firstFrameTime else { return }

        if let pauseStart = pauseStartTime {
            let pauseDuration = CMTimeSubtract(originalTime, pauseStart)
            totalPauseDuration = CMTimeAdd(totalPauseDuration, pauseDuration)
            pauseStartTime = nil
        }

        var presentationTime = CMTimeSubtract(originalTime, firstTime)
        presentationTime = CMTimeSubtract(presentationTime, totalPauseDuration)

        if CMTimeCompare(presentationTime, .zero) < 0 {
            presentationTime = .zero
        }

        if !isFirstFrame && CMTimeCompare(presentationTime, lastFrameTime) <= 0 {
            return
        }

        lastFrameTime = presentationTime

        guard let videoInput = videoInput,
              let pixelBufferAdaptor = pixelBufferAdaptor,
              videoInput.isReadyForMoreMediaData,
              let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer)
        else { return }

        writerQueue.sync {
            guard assetWriter?.status == .writing else { return }
            pixelBufferAdaptor.append(imageBuffer, withPresentationTime: presentationTime)
        }
    }
}
