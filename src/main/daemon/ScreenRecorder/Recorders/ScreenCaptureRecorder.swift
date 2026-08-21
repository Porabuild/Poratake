import AVFoundation
import AppKit
import CoreMedia
import Foundation
import ScreenCaptureKit

@available(macOS 12.3, *)
class ScreenCaptureRecorder: NSObject, SCStreamDelegate, AVCaptureAudioDataOutputSampleBufferDelegate {
    private var stream: SCStream?
    private var assetWriter: AVAssetWriter?
    private var videoInput: AVAssetWriterInput?
    private var pixelBufferAdaptor: AVAssetWriterInputPixelBufferAdaptor?

    private var systemAudioAssetWriter: AVAssetWriter?
    private var systemAudioInput: AVAssetWriterInput?
    private var systemAudioSessionStarted = false
    private var systemAudioOutputPath: String?
    private var systemAudioActive = false
    private var systemAudioEverActive = false

    private var micAudioAssetWriter: AVAssetWriter?
    private var micAudioInput: AVAssetWriterInput?
    private var micAudioSessionStarted = false
    private var micAudioOutputPath: String?
    private var micPendingStartTime: CMTime?
    private var micPauseAnchor: CMTime = .zero
    private var lastMicWriteTime: CMTime?

    private var sessionStarted = false
    private var firstFrameTime: CMTime?
    private var lastFrameTime: CMTime = .zero
    private var pauseStartTime: CMTime?
    private var totalPauseDuration: CMTime = .zero

    private let videoQueue = DispatchQueue(label: "com.porabuild.poratake.screen-recorder.video")
    private let audioQueue = DispatchQueue(label: "com.porabuild.poratake.screen-recorder.audio")
    private let writerQueue = DispatchQueue(label: "com.porabuild.poratake.screen-recorder.writer")
    private let audioWriterQueue = DispatchQueue(label: "com.porabuild.poratake.screen-recorder.audio-writer")

    private(set) var state: RecorderState = .idle
    private var config: RecordingConfig?
    private var recordingDuration: Double = 0
    private var videoWidth: Int = 1920
    private var videoHeight: Int = 1080

    private var micEnabled: Bool = false
    private var micCaptureSession: AVCaptureSession?
    private var micAudioOutput: AVCaptureAudioDataOutput?
    private let micQueue = DispatchQueue(label: "com.porabuild.poratake.screen-recorder.mic")
    private var micSampleCount: Int = 0
    private var firstMicTime: CMTime?
    private var micWriteCount: Int = 0

    private var videoFrameCount: Int = 0
    private var lastVideoTime: CMTime = .zero

    private let cursorTracker = CursorTracker()
    private var keyboardEnabled: Bool = false
    private let keyboardTracker = KeyboardTracker()

    private var cameraEnabled: Bool = false
    private let cameraRecorder = CameraRecorder()
    private var cameraVisibleRanges: [(start: Double, end: Double)] = []
    private var cameraRangeOpenStart: Double?
    
    var onFirstFrame: (() -> Void)?
    var onError: ((Error) -> Void)?

    func configure(_ config: RecordingConfig) {
        self.config = config
    }

    func start() async throws {
        guard state == .idle else {
            throw RecorderError.invalidState("Cannot start: recorder is \(state.rawValue)")
        }

        guard let config = config else {
            throw RecorderError.configuration("Recording config not set")
        }

        do {
            try await startCapture(config)
        } catch {
            await rollbackFailedStart()
            throw error
        }
    }

    private func displayContaining(
        _ window: SCWindow?,
        in content: SCShareableContent
    ) -> CGDirectDisplayID? {
        guard let window = window else { return nil }

        let center = CGPoint(x: window.frame.midX, y: window.frame.midY)
        return content.displays.first { $0.frame.contains(center) }?.displayID
    }

    private func startCapture(_ config: RecordingConfig) async throws {

        let fileManager = FileManager.default
        if fileManager.fileExists(atPath: config.outputPath) {
            try fileManager.removeItem(atPath: config.outputPath)
        }

        let content = try await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        )

        let capturedWindow = try config.windowID.map { windowID -> SCWindow in
            guard let window = content.windows.first(where: { $0.windowID == windowID }) else {
                throw RecorderError.configuration("The window to record is no longer open")
            }
            return window
        }

        let displayID = displayContaining(capturedWindow, in: content) ?? config.displayID
            ?? CGMainDisplayID()
        guard let display = content.displays.first(where: { $0.displayID == displayID })
                ?? content.displays.first
        else {
            throw RecorderError.configuration("No display found")
        }

        // A window filter follows the window itself, so the recording keeps
        // going when the window is moved to another place or another display.
        let filter = capturedWindow.map { SCContentFilter(desktopIndependentWindow: $0) }
            ?? SCContentFilter(display: display, excludingWindows: [])

        var scaleFactor: CGFloat = 2.0
        var targetScreen: NSScreen?
        if let screens = NSScreen.screens as [NSScreen]? {
            for screen in screens {
                let screenNumber = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")]
                    as? CGDirectDisplayID
                if screenNumber == display.displayID {
                    scaleFactor = screen.backingScaleFactor
                    targetScreen = screen
                    break
                }
            }
        }

        let streamConfig = SCStreamConfiguration()

        if let window = capturedWindow {
            videoWidth = Int(window.frame.width * scaleFactor)
            videoHeight = Int(window.frame.height * scaleFactor)
        } else if let rect = config.captureRect {
            videoWidth = Int(rect.width * scaleFactor)
            videoHeight = Int(rect.height * scaleFactor)

            let screenFrame = targetScreen?.frame
                ?? CGRect(
                    x: CGFloat(display.frame.origin.x),
                    y: CGFloat(display.frame.origin.y),
                    width: CGFloat(display.width),
                    height: CGFloat(display.height)
                )

            let mainScreenHeight = NSScreen.screens.first?.frame.height ?? screenFrame.height
            let screenTopY = mainScreenHeight - screenFrame.origin.y - screenFrame.height

            let displayLocalRect = CGRect(
                x: rect.origin.x - screenFrame.origin.x,
                y: rect.origin.y - screenTopY,
                width: rect.width,
                height: rect.height
            )

            streamConfig.sourceRect = displayLocalRect
        } else {
            videoWidth = Int(CGFloat(display.width) * scaleFactor)
            videoHeight = Int(CGFloat(display.height) * scaleFactor)
        }

        videoWidth = (videoWidth / 2) * 2
        videoHeight = (videoHeight / 2) * 2

        streamConfig.width = videoWidth
        streamConfig.height = videoHeight
        streamConfig.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(config.frameRate))
        streamConfig.pixelFormat = kCVPixelFormatType_32BGRA
        streamConfig.showsCursor = false
        streamConfig.queueDepth = 8

        if #available(macOS 14.0, *) {
            streamConfig.captureResolution = .best
        }

        streamConfig.colorSpaceName = CGColorSpace.sRGB

        if #available(macOS 13.0, *) {
            streamConfig.capturesAudio = true
            streamConfig.sampleRate = 48000
            streamConfig.channelCount = 2
            // The video size is fixed at the size the window started with, so a
            // resized window is letterboxed into it rather than cropped.
            streamConfig.scalesToFit = capturedWindow != nil
        }

        let outputURL = URL(fileURLWithPath: config.outputPath)
        assetWriter = try AVAssetWriter(outputURL: outputURL, fileType: .mov)

        let pixelCount = videoWidth * videoHeight
        let bitsPerPixel = 12.0
        let rawBitrate = Int(Double(pixelCount) * bitsPerPixel * Double(config.frameRate))
        let bitrate = min(max(rawBitrate, 50_000_000), 200_000_000)

        let videoSettings: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: videoWidth,
            AVVideoHeightKey: videoHeight,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: bitrate,
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
                AVVideoMaxKeyFrameIntervalKey: config.frameRate,
                AVVideoAllowFrameReorderingKey: false,
            ],
        ]

        videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: videoSettings)
        videoInput?.expectsMediaDataInRealTime = true

        let sourcePixelBufferAttributes: [String: Any] = [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: videoWidth,
            kCVPixelBufferHeightKey as String: videoHeight,
        ]

        pixelBufferAdaptor = AVAssetWriterInputPixelBufferAdaptor(
            assetWriterInput: videoInput!,
            sourcePixelBufferAttributes: sourcePixelBufferAttributes
        )

        if let videoInput = videoInput, assetWriter?.canAdd(videoInput) == true {
            assetWriter?.add(videoInput)
        }

        let audioSettings: [String: Any] = [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: 48000,
            AVNumberOfChannelsKey: 2,
            AVEncoderBitRateKey: 320000,
        ]

        let hasSystemAudio = config.includeAudio
        let hasMicAudio = config.micEnabled
        let videoDir = (config.outputPath as NSString).deletingLastPathComponent

        systemAudioActive = hasSystemAudio
        systemAudioEverActive = hasSystemAudio

        let canCaptureSystemAudio: Bool
        if #available(macOS 13.0, *) {
            canCaptureSystemAudio = true
        } else {
            canCaptureSystemAudio = hasSystemAudio
        }

        if canCaptureSystemAudio {
            systemAudioOutputPath = (videoDir as NSString).appendingPathComponent("system.m4a")
            let systemAudioURL = URL(fileURLWithPath: systemAudioOutputPath!)

            if fileManager.fileExists(atPath: systemAudioOutputPath!) {
                try fileManager.removeItem(atPath: systemAudioOutputPath!)
            }

            systemAudioAssetWriter = try AVAssetWriter(outputURL: systemAudioURL, fileType: .m4a)

            systemAudioInput = AVAssetWriterInput(mediaType: .audio, outputSettings: audioSettings)
            systemAudioInput?.expectsMediaDataInRealTime = true

            if let systemAudioInput = systemAudioInput, systemAudioAssetWriter?.canAdd(systemAudioInput) == true {
                systemAudioAssetWriter?.add(systemAudioInput)
            }

            guard systemAudioAssetWriter?.startWriting() == true else {
                throw RecorderError.capture(
                    "Failed to start system audio writer: \(systemAudioAssetWriter?.error?.localizedDescription ?? "unknown error")"
                )
            }
        }

        micEnabled = hasMicAudio
        if hasMicAudio {
            micAudioOutputPath = (videoDir as NSString).appendingPathComponent("mic.m4a")
            let micAudioURL = URL(fileURLWithPath: micAudioOutputPath!)

            if fileManager.fileExists(atPath: micAudioOutputPath!) {
                try fileManager.removeItem(atPath: micAudioOutputPath!)
            }

            micAudioAssetWriter = try AVAssetWriter(outputURL: micAudioURL, fileType: .m4a)

            micAudioInput = AVAssetWriterInput(mediaType: .audio, outputSettings: audioSettings)
            micAudioInput?.expectsMediaDataInRealTime = true

            if let micAudioInput = micAudioInput, micAudioAssetWriter?.canAdd(micAudioInput) == true {
                micAudioAssetWriter?.add(micAudioInput)
            }

            guard micAudioAssetWriter?.startWriting() == true else {
                throw RecorderError.capture(
                    "Failed to start mic audio writer: \(micAudioAssetWriter?.error?.localizedDescription ?? "unknown error")"
                )
            }

            try setupMicrophoneCapture(config: config)
            micPendingStartTime = .zero
        }

        guard assetWriter?.startWriting() == true else {
            throw RecorderError.capture(
                "Failed to start asset writer: \(assetWriter?.error?.localizedDescription ?? "unknown error")"
            )
        }

        cameraEnabled = config.cameraEnabled
        if config.cameraEnabled {
            let videoDir = (config.outputPath as NSString).deletingLastPathComponent
            let cameraOutputPath = (videoDir as NSString).appendingPathComponent("camera.mov")
            cameraRecorder.configure(
                deviceId: config.cameraDeviceId,
                deviceName: config.cameraDeviceName,
                frameRate: 30,
                outputPath: cameraOutputPath
            )
            try cameraRecorder.start()
            cameraRangeOpenStart = 0
        }

        if let window = capturedWindow {
            cursorTracker.start(
                bounds: window.frame,
                videoPath: config.outputPath,
                windowID: window.windowID
            )
        } else if let rect = config.captureRect {
            cursorTracker.start(bounds: rect, videoPath: config.outputPath)
        } else {
            let screen = NSScreen.screens.first { screen in
                guard let screenNumber = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID else {
                    return false
                }
                return screenNumber == displayID
            } ?? NSScreen.main

            if let screen = screen {
                let mainScreenHeight = NSScreen.screens.first?.frame.height ?? screen.frame.height
                let displayBounds = CGRect(
                    x: screen.frame.origin.x,
                    y: mainScreenHeight - screen.frame.origin.y - screen.frame.height,
                    width: screen.frame.width,
                    height: screen.frame.height
                )
                cursorTracker.start(bounds: displayBounds, videoPath: config.outputPath)
            }
        }

        keyboardEnabled = config.keyboardEnabled
        if config.keyboardEnabled {
            keyboardTracker.start(videoPath: config.outputPath)
        }

        stream = SCStream(filter: filter, configuration: streamConfig, delegate: self)

        try stream?.addStreamOutput(self, type: .screen, sampleHandlerQueue: videoQueue)

        if #available(macOS 13.0, *) {
            try stream?.addStreamOutput(self, type: .audio, sampleHandlerQueue: audioQueue)
        }

        sessionStarted = false
        firstFrameTime = nil
        lastFrameTime = .zero
        pauseStartTime = nil
        totalPauseDuration = .zero
        recordingDuration = 0
        micSampleCount = 0
        micWriteCount = 0
        videoFrameCount = 0
        lastVideoTime = .zero
        firstMicTime = nil

        try await stream?.startCapture()
        state = .recording
    }

    private func rollbackFailedStart() async {
        state = .idle

        try? await stream?.stopCapture()
        stream = nil
        stopMicrophoneCapture()
        cameraRecorder.abort()
        _ = cursorTracker.stop()
        _ = keyboardTracker.stop()
        waitForPendingSamples()

        if assetWriter?.status == .writing {
            assetWriter?.cancelWriting()
        }
        if systemAudioAssetWriter?.status == .writing {
            systemAudioAssetWriter?.cancelWriting()
        }
        if micAudioAssetWriter?.status == .writing {
            micAudioAssetWriter?.cancelWriting()
        }

        resetRecordingState()
    }

    private func setupMicrophoneCapture(config: RecordingConfig) throws {
        try setupMicrophoneCapture(
            deviceId: config.micDeviceId,
            deviceName: config.micDeviceName
        )
    }

    private func resolveMicrophoneDevice(
        deviceId: String?,
        deviceName: String?
    ) throws -> AVCaptureDevice {
        let discoverySession: AVCaptureDevice.DiscoverySession
        if #available(macOS 14.0, *) {
            discoverySession = AVCaptureDevice.DiscoverySession(
                deviceTypes: [.microphone, .external, .continuityCamera],
                mediaType: .audio,
                position: .unspecified
            )
        } else {
            discoverySession = AVCaptureDevice.DiscoverySession(
                deviceTypes: [.builtInMicrophone, .externalUnknown],
                mediaType: .audio,
                position: .unspecified
            )
        }

        if let deviceName = deviceName {
            if let exact = discoverySession.devices.first(where: { $0.localizedName == deviceName }) {
                return exact
            }
            if let fuzzy = discoverySession.devices.first(where: { device in
                deviceName.contains(device.localizedName)
                    || device.localizedName.contains(deviceName)
            }) {
                return fuzzy
            }
        }

        if let deviceId = deviceId,
           let byId = discoverySession.devices.first(where: { $0.uniqueID == deviceId }) {
            return byId
        }

        guard let fallback = AVCaptureDevice.default(for: .audio) else {
            throw RecorderError.configuration("No microphone found")
        }
        return fallback
    }

    private func setupMicrophoneCapture(deviceId: String?, deviceName: String?) throws {
        micCaptureSession = AVCaptureSession()

        let mic = try resolveMicrophoneDevice(deviceId: deviceId, deviceName: deviceName)

        let micDeviceInput = try AVCaptureDeviceInput(device: mic)
        if micCaptureSession?.canAddInput(micDeviceInput) == true {
            micCaptureSession?.addInput(micDeviceInput)
        } else {
            throw RecorderError.configuration("Cannot add microphone input to capture session")
        }

        micAudioOutput = AVCaptureAudioDataOutput()
        micAudioOutput?.setSampleBufferDelegate(self, queue: micQueue)

        if let output = micAudioOutput, micCaptureSession?.canAddOutput(output) == true {
            micCaptureSession?.addOutput(output)
        } else {
            throw RecorderError.configuration("Cannot add audio output to capture session")
        }

        micCaptureSession?.startRunning()
    }

    private func swapMicrophoneDevice(deviceId: String?, deviceName: String?) throws {
        guard let session = micCaptureSession else { return }

        let device = try resolveMicrophoneDevice(deviceId: deviceId, deviceName: deviceName)

        let previousInput = session.inputs.first as? AVCaptureDeviceInput
        if previousInput?.device.uniqueID == device.uniqueID {
            return
        }

        let newInput = try AVCaptureDeviceInput(device: device)
        guard session.canAddInput(newInput) else {
            throw RecorderError.configuration("Cannot add microphone input to capture session")
        }

        session.beginConfiguration()
        for input in session.inputs {
            session.removeInput(input)
        }
        session.addInput(newInput)
        session.commitConfiguration()
    }

    private func startMicrophoneDuringRecording(
        deviceId: String?,
        deviceName: String?
    ) throws {
        guard let config = config else {
            throw RecorderError.configuration("Recording config not set")
        }

        if micAudioAssetWriter == nil {
            let videoDir = (config.outputPath as NSString).deletingLastPathComponent
            micAudioOutputPath = (videoDir as NSString).appendingPathComponent("mic.m4a")
            let micAudioURL = URL(fileURLWithPath: micAudioOutputPath!)

            if FileManager.default.fileExists(atPath: micAudioOutputPath!) {
                try FileManager.default.removeItem(atPath: micAudioOutputPath!)
            }

            let audioSettings: [String: Any] = [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 48000,
                AVNumberOfChannelsKey: 2,
                AVEncoderBitRateKey: 320000,
            ]

            micAudioAssetWriter = try AVAssetWriter(outputURL: micAudioURL, fileType: .m4a)
            micAudioInput = AVAssetWriterInput(mediaType: .audio, outputSettings: audioSettings)
            micAudioInput?.expectsMediaDataInRealTime = true

            if let micAudioInput = micAudioInput,
               micAudioAssetWriter?.canAdd(micAudioInput) == true {
                micAudioAssetWriter?.add(micAudioInput)
            }

            guard micAudioAssetWriter?.startWriting() == true else {
                throw RecorderError.capture(
                    "Failed to start mic audio writer: \(micAudioAssetWriter?.error?.localizedDescription ?? "unknown error")"
                )
            }
        }

        try setupMicrophoneCapture(deviceId: deviceId, deviceName: deviceName)
        micPendingStartTime = videoFrameCount > 0 ? lastVideoTime : .zero
        micEnabled = true
    }

    func setMicrophone(enabled: Bool, deviceId: String?, deviceName: String?) throws {
        guard state == .recording || state == .paused else {
            throw RecorderError.invalidState(
                "Cannot change the microphone: recorder is \(state.rawValue)"
            )
        }

        if !enabled {
            micEnabled = false
            stopMicrophoneCapture()
            return
        }

        if micCaptureSession != nil {
            try swapMicrophoneDevice(deviceId: deviceId, deviceName: deviceName)
            micEnabled = true
            return
        }

        try startMicrophoneDuringRecording(deviceId: deviceId, deviceName: deviceName)
    }

    func setSystemAudio(enabled: Bool) throws {
        guard state == .recording || state == .paused else {
            throw RecorderError.invalidState(
                "Cannot change system audio: recorder is \(state.rawValue)"
            )
        }

        guard systemAudioAssetWriter != nil else {
            throw RecorderError.invalidState(
                "System audio cannot be changed for this recording"
            )
        }

        systemAudioActive = enabled
        if enabled {
            systemAudioEverActive = true
        }
    }

    func setCamera(enabled: Bool) throws {
        guard state == .recording || state == .paused else {
            throw RecorderError.invalidState(
                "Cannot change the camera: recorder is \(state.rawValue)"
            )
        }

        if enabled {
            if cameraRecorder.isRecording {
                cameraRecorder.unsuspend()
            } else {
                try startCameraDuringRecording()
            }
            openCameraRange()
            return
        }

        guard cameraRecorder.isRecording else { return }
        cameraRecorder.suspend()
        closeCameraRange()
    }

    private func startCameraDuringRecording() throws {
        guard let config = config else {
            throw RecorderError.configuration("Recording config not set")
        }

        let videoDir = (config.outputPath as NSString).deletingLastPathComponent
        let cameraOutputPath = (videoDir as NSString).appendingPathComponent("camera.mov")
        cameraRecorder.configure(
            deviceId: config.cameraDeviceId,
            deviceName: config.cameraDeviceName,
            frameRate: 30,
            outputPath: cameraOutputPath
        )
        try cameraRecorder.start()
        cameraRecorder.syncWithVideoStart()
        cameraEnabled = true
    }

    private func openCameraRange() {
        guard cameraRangeOpenStart == nil else { return }
        cameraRangeOpenStart = recordingDuration
    }

    private func closeCameraRange() {
        guard let start = cameraRangeOpenStart else { return }
        cameraRangeOpenStart = nil
        cameraVisibleRanges.append((start, max(recordingDuration, start)))
    }

    func captureOutput(
        _ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        guard state == .recording else { return }
        guard sessionStarted else { return }
        guard micAudioSessionStarted else { return }

        micSampleCount += 1
        writeMicAudioSample(sampleBuffer)
    }

    private func writeMicAudioSample(_ sampleBuffer: CMSampleBuffer) {
        guard let micAudioInput = micAudioInput,
            micAudioInput.isReadyForMoreMediaData
        else { return }

        guard videoFrameCount > 0 else { return }

        let micTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)

        if let pendingStart = micPendingStartTime {
            micPendingStartTime = nil
            firstMicTime = CMTimeSubtract(micTime, pendingStart)
            micPauseAnchor = totalPauseDuration
            let silenceStart = micWriteCount > 0 ? (lastMicWriteTime ?? .zero) : .zero
            appendMicSilence(
                from: silenceStart,
                until: pendingStart,
                like: sampleBuffer
            )
        } else if firstMicTime == nil {
            firstMicTime = micTime
            micPauseAnchor = totalPauseDuration
        }

        guard let firstMic = firstMicTime else { return }

        let micRelativeTime = CMTimeSubtract(micTime, firstMic)
        var adjustedTime = micRelativeTime
        adjustedTime = CMTimeSubtract(
            adjustedTime,
            CMTimeSubtract(totalPauseDuration, micPauseAnchor)
        )

        if CMTimeCompare(adjustedTime, .zero) < 0 {
            adjustedTime = .zero
        }

        let maxTime = CMTimeAdd(lastVideoTime, CMTimeMake(value: 1, timescale: 10))
        if CMTimeCompare(adjustedTime, maxTime) > 0 {
            adjustedTime = maxTime
        }

        if let adjustedBuffer = createAdjustedSampleBuffer(sampleBuffer, newTime: adjustedTime) {
            if appendMicBufferToWriter(adjustedBuffer) {
                lastMicWriteTime = adjustedTime
            }
        }
    }

    private func appendMicSilence(
        from startTime: CMTime,
        until endTime: CMTime,
        like sampleBuffer: CMSampleBuffer
    ) {
        guard CMTimeCompare(startTime, endTime) < 0 else { return }

        let step = CMSampleBufferGetDuration(sampleBuffer)
        guard CMTimeCompare(step, .zero) > 0 else { return }

        var time = startTime
        while CMTimeCompare(time, endTime) < 0 {
            guard let muted = createMutedSampleBuffer(from: sampleBuffer),
                  let adjusted = createAdjustedSampleBuffer(muted, newTime: time)
            else { return }

            if appendMicBufferToWriter(adjusted) {
                lastMicWriteTime = time
            }
            time = CMTimeAdd(time, step)
        }
    }

    private func appendMicBufferToWriter(_ sampleBuffer: CMSampleBuffer) -> Bool {
        guard let micAudioInput = micAudioInput,
              micAudioInput.isReadyForMoreMediaData
        else { return false }

        var appended = false
        audioWriterQueue.sync {
            guard micAudioAssetWriter?.status == .writing else { return }
            if micAudioInput.append(sampleBuffer) {
                micWriteCount += 1
                appended = true
            }
        }
        return appended
    }

    private func stopMicrophoneCapture() {
        if let session = micCaptureSession {
            if session.isRunning {
                session.stopRunning()
            }
            for input in session.inputs {
                session.removeInput(input)
            }
            for output in session.outputs {
                session.removeOutput(output)
            }
        }
        micCaptureSession = nil
        micAudioOutput = nil
    }

    private func createMutedSampleBuffer(from originalBuffer: CMSampleBuffer) -> CMSampleBuffer? {
        guard let formatDescription = CMSampleBufferGetFormatDescription(originalBuffer),
              let originalBlockBuffer = CMSampleBufferGetDataBuffer(originalBuffer) else {
            return nil
        }
        
        var totalLength: Int = 0
        var dataPointer: UnsafeMutablePointer<Int8>?
        
        let getStatus = CMBlockBufferGetDataPointer(
            originalBlockBuffer,
            atOffset: 0,
            lengthAtOffsetOut: nil,
            totalLengthOut: &totalLength,
            dataPointerOut: &dataPointer
        )
        
        guard getStatus == kCMBlockBufferNoErr, totalLength > 0 else {
            return nil
        }
        
        var newBlockBuffer: CMBlockBuffer?
        var status = CMBlockBufferCreateWithMemoryBlock(
            allocator: kCFAllocatorDefault,
            memoryBlock: nil,
            blockLength: totalLength,
            blockAllocator: kCFAllocatorDefault,
            customBlockSource: nil,
            offsetToData: 0,
            dataLength: totalLength,
            flags: kCMBlockBufferAssureMemoryNowFlag,
            blockBufferOut: &newBlockBuffer
        )
        
        guard status == kCMBlockBufferNoErr, let silentBlock = newBlockBuffer else {
            return nil
        }
        
        status = CMBlockBufferFillDataBytes(with: 0, blockBuffer: silentBlock, offsetIntoDestination: 0, dataLength: totalLength)
        guard status == kCMBlockBufferNoErr else {
            return nil
        }
        
        let numSamples = CMSampleBufferGetNumSamples(originalBuffer)
        let duration = CMSampleBufferGetDuration(originalBuffer)
        let presentationTime = CMSampleBufferGetPresentationTimeStamp(originalBuffer)
        
        var timing = CMSampleTimingInfo(
            duration: duration,
            presentationTimeStamp: presentationTime,
            decodeTimeStamp: .invalid
        )
        
        var sampleSizeArray: [Int] = []
        for i in 0..<numSamples {
            sampleSizeArray.append(CMSampleBufferGetSampleSize(originalBuffer, at: i))
        }
        
        var newSampleBuffer: CMSampleBuffer?
        status = CMSampleBufferCreate(
            allocator: kCFAllocatorDefault,
            dataBuffer: silentBlock,
            dataReady: true,
            makeDataReadyCallback: nil,
            refcon: nil,
            formatDescription: formatDescription,
            sampleCount: numSamples,
            sampleTimingEntryCount: 1,
            sampleTimingArray: &timing,
            sampleSizeEntryCount: sampleSizeArray.count,
            sampleSizeArray: sampleSizeArray,
            sampleBufferOut: &newSampleBuffer
        )
        
        guard status == noErr else {
            return nil
        }
        
        return newSampleBuffer
    }

    func pause() throws {
        guard state == .recording else {
            throw RecorderError.invalidState("Cannot pause: recorder is \(state.rawValue)")
        }

        pauseStartTime = videoFrameCount > 0 ? lastFrameTime : nil
        state = .paused

        cursorTracker.pause()

        if keyboardEnabled {
            keyboardTracker.pause()
        }

        if cameraEnabled {
            cameraRecorder.pause()
        }
    }

    func resume() throws {
        guard state == .paused else {
            throw RecorderError.invalidState("Cannot resume: recorder is \(state.rawValue)")
        }

        state = .recording

        cursorTracker.resume()

        if keyboardEnabled {
            keyboardTracker.resume()
        }

        if cameraEnabled {
            cameraRecorder.resume()
        }
    }

    func stop() async throws -> RecordingResult {
        guard state == .recording || state == .paused else {
            throw RecorderError.invalidState("Cannot stop: recorder is \(state.rawValue)")
        }

        state = .idle
        defer { resetRecordingState() }

        let cursorFilePath = cursorTracker.stop()

        var keysFilePath: String? = nil
        if keyboardEnabled {
            keysFilePath = keyboardTracker.stop()
        }

        var cameraFilePath: String? = nil
        if cameraEnabled {
            closeCameraRange()
            cameraRecorder.setVisibleRanges(cameraVisibleRanges)
            if let result = cameraRecorder.stop() {
                cameraFilePath = result.videoPath
            }
        }

        stopMicrophoneCapture()

        do {
            try await stream?.stopCapture()
        } catch {
            // Ignore stop errors
        }
        stream = nil

        waitForPendingSamples()

        videoInput?.markAsFinished()
        systemAudioInput?.markAsFinished()
        micAudioInput?.markAsFinished()

        let finalPath = config?.outputPath ?? ""
        let finalDuration = recordingDuration
        var finalSystemAudioPath = systemAudioOutputPath
        let finalMicAudioPath = micAudioOutputPath

        if assetWriter?.status == .writing {
            await assetWriter?.finishWriting()
        }

        if assetWriter?.status == .failed, let error = assetWriter?.error {
            throw RecorderError.capture("Asset writer error: \(error.localizedDescription)")
        }

        if systemAudioAssetWriter?.status == .writing {
            await systemAudioAssetWriter?.finishWriting()
        }

        if systemAudioAssetWriter?.status == .failed, let error = systemAudioAssetWriter?.error {
            throw RecorderError.capture("System audio writer error: \(error.localizedDescription)")
        }

        if micAudioAssetWriter?.status == .writing {
            await micAudioAssetWriter?.finishWriting()
        }

        if micAudioAssetWriter?.status == .failed, let error = micAudioAssetWriter?.error {
            throw RecorderError.capture("Mic audio writer error: \(error.localizedDescription)")
        }

        if let path = finalSystemAudioPath, !systemAudioEverActive {
            try? FileManager.default.removeItem(atPath: path)
            finalSystemAudioPath = nil
        }

        return RecordingResult(
            outputPath: finalPath,
            cursorPath: cursorFilePath,
            cameraPath: cameraFilePath,
            keysPath: keysFilePath,
            systemAudioPath: finalSystemAudioPath,
            micAudioPath: finalMicAudioPath,
            duration: finalDuration
        )
    }

    func getStatus() -> (state: RecorderState, duration: Double) {
        return (state, recordingDuration)
    }

    private func waitForPendingSamples() {
        videoQueue.sync {}
        audioQueue.sync {}
        micQueue.sync {}
        writerQueue.sync {}
        audioWriterQueue.sync {}
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        Task { @MainActor [weak self] in
            guard let self, self.state == .recording || self.state == .paused else { return }
            _ = try? await self.stop()
            self.onError?(error)
        }
    }

    private func resetRecordingState() {
        assetWriter = nil
        videoInput = nil
        pixelBufferAdaptor = nil
        systemAudioAssetWriter = nil
        systemAudioInput = nil
        systemAudioOutputPath = nil
        systemAudioSessionStarted = false
        systemAudioActive = false
        systemAudioEverActive = false
        micAudioAssetWriter = nil
        micAudioInput = nil
        micAudioOutputPath = nil
        micAudioSessionStarted = false
        micPendingStartTime = nil
        micPauseAnchor = .zero
        lastMicWriteTime = nil
        firstMicTime = nil
        cameraVisibleRanges = []
        cameraRangeOpenStart = nil
        sessionStarted = false
        firstFrameTime = nil
        pauseStartTime = nil
        totalPauseDuration = .zero
        recordingDuration = 0
    }
}

@available(macOS 12.3, *)
extension ScreenCaptureRecorder: SCStreamOutput {
    func stream(
        _ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of type: SCStreamOutputType
    ) {
        guard CMSampleBufferDataIsReady(sampleBuffer) else { return }

        let currentTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)

        if state == .paused {
            if type == .screen {
                if pauseStartTime == nil {
                    pauseStartTime = currentTime
                }
            }
            return
        }

        guard state == .recording else { return }

        if let pauseStart = pauseStartTime {
            let pauseDuration = CMTimeSubtract(currentTime, pauseStart)
            totalPauseDuration = CMTimeAdd(totalPauseDuration, pauseDuration)
            pauseStartTime = nil
        }

        if !sessionStarted {
            firstFrameTime = currentTime
            assetWriter?.startSession(atSourceTime: .zero)
            sessionStarted = true
            cursorTracker.syncWithVideoStart()
            if keyboardEnabled {
                keyboardTracker.syncWithVideoStart()
            }
            if cameraEnabled {
                cameraRecorder.syncWithVideoStart()
            }
            onFirstFrame?()
        }

        if !systemAudioSessionStarted && systemAudioAssetWriter != nil {
            systemAudioAssetWriter?.startSession(atSourceTime: .zero)
            systemAudioSessionStarted = true
        }

        if !micAudioSessionStarted && micAudioAssetWriter != nil {
            micAudioAssetWriter?.startSession(atSourceTime: .zero)
            micAudioSessionStarted = true
        }

        guard let firstTime = firstFrameTime else { return }

        var presentationTime = CMTimeSubtract(currentTime, firstTime)
        presentationTime = CMTimeSubtract(presentationTime, totalPauseDuration)

        if CMTimeCompare(presentationTime, .zero) < 0 {
            presentationTime = .zero
        }

        if type == .screen {
            lastFrameTime = currentTime
            recordingDuration = CMTimeGetSeconds(presentationTime)

            guard let videoInput = videoInput,
                let pixelBufferAdaptor = pixelBufferAdaptor
            else { return }

            guard videoInput.isReadyForMoreMediaData else { return }

            if videoFrameCount > 0 && CMTimeCompare(presentationTime, lastVideoTime) <= 0 {
                return
            }

            guard let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }

            let time = presentationTime
            writerQueue.sync {
                guard assetWriter?.status == .writing else { return }
                if pixelBufferAdaptor.append(imageBuffer, withPresentationTime: time) {
                    videoFrameCount += 1
                    lastVideoTime = time
                }
            }
        } else if #available(macOS 13.0, *), type == .audio {
            if systemAudioActive {
                appendSystemAudioBuffer(sampleBuffer, at: presentationTime)
            } else if let muted = createMutedSampleBuffer(from: sampleBuffer) {
                appendSystemAudioBuffer(muted, at: presentationTime)
            }
        }
    }

    private func appendSystemAudioBuffer(_ sampleBuffer: CMSampleBuffer, at time: CMTime) {
        guard let systemAudioInput = systemAudioInput,
            systemAudioInput.isReadyForMoreMediaData
        else { return }

        guard let adjustedBuffer = createAdjustedSampleBuffer(sampleBuffer, newTime: time) else {
            return
        }

        audioWriterQueue.sync {
            guard systemAudioAssetWriter?.status == .writing else { return }
            systemAudioInput.append(adjustedBuffer)
        }
    }

    private func createAdjustedSampleBuffer(_ buffer: CMSampleBuffer, newTime: CMTime)
        -> CMSampleBuffer?
    {
        var timing = CMSampleTimingInfo(
            duration: CMSampleBufferGetDuration(buffer),
            presentationTimeStamp: newTime,
            decodeTimeStamp: .invalid
        )

        var newBuffer: CMSampleBuffer?
        let status = CMSampleBufferCreateCopyWithNewTiming(
            allocator: kCFAllocatorDefault,
            sampleBuffer: buffer,
            sampleTimingEntryCount: 1,
            sampleTimingArray: &timing,
            sampleBufferOut: &newBuffer
        )

        return status == noErr ? newBuffer : nil
    }
}
