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
    private var systemAudioBufferQueue: AudioBufferQueue?
    private var systemAudioSessionStarted = false
    private var systemAudioOutputPath: String?
    private var systemAudioActive = false
    private var systemAudioEverActive = false
    private var lastSystemAudioEndTime: CMTime = .zero
    private var systemAudioWrittenEndTime: CMTime = .zero
    private var systemAudioGapRemainder: CMTime = .zero

    private var micAudioAssetWriter: AVAssetWriter?
    private var micAudioInput: AVAssetWriterInput?
    private var micBufferQueue: AudioBufferQueue?
    private var micAudioSessionStarted = false
    private var micAudioOutputPath: String?
    private var micPendingStartTime: CMTime?
    private var micPauseAnchor: CMTime = .zero
    private var lastMicWriteEndTime: CMTime?

    private var sessionStarted = false
    private var firstFrameTime: CMTime?
    private var lastFrameTime: CMTime = .zero
    private var pauseStartTime: CMTime?
    private var totalPauseDuration: CMTime = .zero

    private let videoQueue = DispatchQueue(label: "com.porabuild.poratake.screen-recorder.video")
    private let writerQueue = DispatchQueue(label: "com.porabuild.poratake.screen-recorder.writer")

    private(set) var state: RecorderState = .idle
    private var lifecycle = RecorderLifecycleGuard()
    private var config: RecordingConfig?
    private var activeSessionID: RecorderSessionID?
    private var recordingDuration: Double = 0
    private var videoWidth: Int = 1920
    private var videoHeight: Int = 1080

    private var micEnabled: Bool = false
    private let micCaptureSessionLock = NSLock()
    private var micCaptureSessionStorage: AVCaptureSession?
    private var micCaptureSession: AVCaptureSession? {
        get {
            micCaptureSessionLock.lock()
            defer { micCaptureSessionLock.unlock() }
            return micCaptureSessionStorage
        }
        set {
            micCaptureSessionLock.lock()
            defer { micCaptureSessionLock.unlock() }
            micCaptureSessionStorage = newValue
        }
    }
    private var micAudioOutput: AVCaptureAudioDataOutput?
    private let micQueue = DispatchQueue(label: "com.porabuild.poratake.screen-recorder.mic")
    private var micSampleCount: Int = 0
    private var firstMicTime: CMTime?
#if RECORDING_TIMELINE_TEST
    private(set) var testMicrophoneWrites: [(CMTime, CMTime)] = []
    private var testCameraClockTime: CMTime?
    private var testVideoBackpressure = false
    private var testMicrophoneClock: AnyObject?
    private var testMicrophoneScreenClock: CMClock?
#endif

    private var videoFrameCount: Int = 0
    private var lastVideoTime: CMTime = .zero
    private var lastVideoFrame: CVPixelBuffer?
    private var pendingVideoFrame: (frame: CVPixelBuffer, time: CMTime)?
    private var videoFrameDuration = CMTime(value: 1, timescale: 30)

    private let cursorTracker = CursorTracker()
    private var keyboardEnabled: Bool = false
    private let keyboardTracker = KeyboardTracker()

    private var cameraEnabled: Bool = false
    private let cameraRecorder = CameraRecorder()
    private var cameraVisibleRanges: [(start: Double, end: Double)] = []
    private var cameraRangeOpenStart: Double?
    
    var onFirstFrame: (() -> Void)?
    var onError: (@MainActor (RecorderSessionID, Error) -> Void)?

    @MainActor
    func start(_ config: RecordingConfig, sessionID: RecorderSessionID) async throws {
        guard lifecycle.beginStart(state: state) else {
            throw RecorderError.invalidState(
                "Cannot start: recorder is \(state.rawValue) or a lifecycle transition is in progress"
            )
        }
        self.config = config
        activeSessionID = sessionID
        defer { lifecycle.finishStart() }

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

    @MainActor
    private func startCapture(_ config: RecordingConfig) async throws {
        RecordingCaptureWindowRegistry.shared.beginCaptureStart()

        let fileManager = FileManager.default
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

        let displayID = try recordingDisplayID(
            available: content.displays.map(\.displayID),
            windowDisplayID: displayContaining(capturedWindow, in: content),
            requestedDisplayID: config.displayID,
            mainDisplayID: CGMainDisplayID()
        )
        guard let display = content.displays.first(where: { $0.displayID == displayID }) else {
            throw RecorderError.configuration("No display found")
        }

        // A window filter follows the window itself, so the recording keeps
        // going when the window is moved to another place or another display.
        let filter: SCContentFilter
        let registeredWindowIDs = RecordingCaptureWindowRegistry.shared.currentWindowIDs()
        let excludedWindowIDs = try resolvedRecordingExclusionWindowIDs(
            requested: config.excludedWindowIDs,
            registered: registeredWindowIDs,
            selectedWindowID: capturedWindow?.windowID,
            availableWindowIDs: Set(content.windows.map(\.windowID))
        )
        if let capturedWindow {
            filter = SCContentFilter(desktopIndependentWindow: capturedWindow)
        } else {
            let excludedWindows = content.windows.filter { excludedWindowIDs.contains($0.windowID) }
            filter = SCContentFilter(display: display, excludingWindows: excludedWindows)
        }

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
        let frameRate = effectiveFrameRate(
            configured: config.frameRate,
            maximum: targetScreen?.maximumFramesPerSecond
        )
        videoFrameDuration = CMTime(value: 1, timescale: CMTimeScale(frameRate))

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

            let displayBounds = CGRect(
                x: screenFrame.origin.x,
                y: screenTopY,
                width: screenFrame.width,
                height: screenFrame.height
            )
            streamConfig.sourceRect = try recordingSourceRect(selection: rect, display: displayBounds)
        } else {
            videoWidth = Int(CGFloat(display.width) * scaleFactor)
            videoHeight = Int(CGFloat(display.height) * scaleFactor)
        }

        videoWidth = (videoWidth / 2) * 2
        videoHeight = (videoHeight / 2) * 2

        streamConfig.width = videoWidth
        streamConfig.height = videoHeight
        streamConfig.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(frameRate))
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
        if fileManager.fileExists(atPath: config.outputPath) {
            try fileManager.removeItem(at: outputURL)
        }
        assetWriter = try AVAssetWriter(outputURL: outputURL, fileType: .mov)

        let pixelCount = videoWidth * videoHeight
        let bitsPerPixel = 12.0
        let rawBitrate = Int(Double(pixelCount) * bitsPerPixel * Double(frameRate))
        let bitrate = min(max(rawBitrate, 50_000_000), 200_000_000)

        let videoSettings: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: videoWidth,
            AVVideoHeightKey: videoHeight,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: bitrate,
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
                AVVideoMaxKeyFrameIntervalKey: frameRate,
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

        if capturedWindow == nil {
            guard let captureStream = stream else {
                throw RecorderError.capture("Failed to initialize ScreenCaptureKit stream")
            }
            let requestedWindowIDs = config.excludedWindowIDs
            let captureDisplayID = display.displayID
            RecordingCaptureWindowRegistry.shared.setFilterUpdater { windowIDs in
                try await Self.updateCaptureFilter(
                    on: captureStream,
                    displayID: captureDisplayID,
                    requestedWindowIDs: requestedWindowIDs,
                    excluding: windowIDs
                )
            }
        }

        try stream?.addStreamOutput(self, type: .screen, sampleHandlerQueue: videoQueue)

        if #available(macOS 13.0, *) {
            try stream?.addStreamOutput(self, type: .audio, sampleHandlerQueue: videoQueue)
        }

        sessionStarted = false
        firstFrameTime = nil
        lastFrameTime = .zero
        pauseStartTime = nil
        totalPauseDuration = .zero
        recordingDuration = 0
        micSampleCount = 0
        videoFrameCount = 0
        lastVideoTime = .zero
        lastVideoFrame = nil
        pendingVideoFrame = nil
        firstMicTime = nil

        try await stream?.startCapture()
        RecordingCaptureWindowRegistry.shared.finishCaptureStart()
        try await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
        state = .recording
    }

    @MainActor
    private static func updateCaptureFilter(
        on stream: SCStream,
        displayID captureDisplayID: CGDirectDisplayID,
        requestedWindowIDs: [Int64],
        excluding windowIDs: [CGWindowID]
    ) async throws {
        let content = try await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: false
        )
        guard let display = content.displays.first(where: { $0.displayID == captureDisplayID }) else {
            throw RecordingCaptureFilterError.unavailableDisplay(captureDisplayID)
        }
        let excludedIDs = try resolvedRecordingExclusionWindowIDs(
            requested: requestedWindowIDs,
            registered: windowIDs,
            selectedWindowID: nil,
            availableWindowIDs: Set(content.windows.map(\.windowID))
        )
        let excludedWindows = content.windows.filter { excludedIDs.contains($0.windowID) }
        try await stream.updateContentFilter(
            SCContentFilter(display: display, excludingWindows: excludedWindows)
        )
    }

    @MainActor
    private func rollbackFailedStart() async {
        state = .idle

        RecordingCaptureWindowRegistry.shared.finishCaptureStart()
        try? await stream?.stopCapture()
        stream = nil
        RecordingCaptureWindowRegistry.shared.setFilterUpdater(nil)
        stopMicrophoneCapture()
        cameraRecorder.abort()
        _ = cursorTracker.stop()
        _ = keyboardTracker.stop()
        waitForPendingSamples()
        systemAudioBufferQueue?.cancel()
        micBufferQueue?.cancel()

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

            let writer = try AVAssetWriter(outputURL: micAudioURL, fileType: .m4a)
            let input = AVAssetWriterInput(mediaType: .audio, outputSettings: audioSettings)
            input.expectsMediaDataInRealTime = true
            guard writer.canAdd(input) else {
                throw RecorderError.configuration("Cannot add microphone input to writer")
            }
            writer.add(input)
            guard writer.startWriting() else {
                throw RecorderError.capture(
                    "Failed to start mic audio writer: \(writer.error?.localizedDescription ?? "unknown error")"
                )
            }
            installMicrophoneWriter(writer, input: input)
        }

        try setupMicrophoneCaptureForRecording(at: microphoneStartTime) {
            try setupMicrophoneCapture(deviceId: deviceId, deviceName: deviceName)
        }
        micEnabled = true
    }

    private func installMicrophoneWriter(_ writer: AVAssetWriter, input: AVAssetWriterInput) {
        videoQueue.sync {
            micAudioAssetWriter = writer
            micAudioInput = input
            startMicrophoneWriterIfReady()
        }
    }

    private func startMicrophoneWriterIfReady() {
        guard sessionStarted, !micAudioSessionStarted, let writer = micAudioAssetWriter else { return }
        writer.startSession(atSourceTime: .zero)
        configureMicrophoneBufferQueue()
        micAudioSessionStarted = true
    }

    private func setupMicrophoneCaptureForRecording(at start: CMTime, setup: () throws -> Void) throws {
        micQueue.sync { micPendingStartTime = start }
        do {
            try setup()
        } catch {
            micQueue.sync { micPendingStartTime = nil }
            stopMicrophoneCapture()
            throw error
        }
    }

    private var microphoneStartTime: CMTime {
        microphoneScreenTimeline.position
    }

    private var microphoneScreenTimeline: (position: CMTime, paused: CMTime, hasVideo: Bool, origin: CMTime?, clock: CMClock?) {
        videoQueue.sync {
            let hasVideo = sessionStarted || videoFrameCount > 0
            let fallback = hasVideo ? latestScreenPresentationTime : .zero
            var clock: CMClock?
            if #available(macOS 13.0, *) { clock = stream?.synchronizationClock }
            #if RECORDING_TIMELINE_TEST
            if let testMicrophoneScreenClock { clock = testMicrophoneScreenClock }
            #endif
            guard sessionStarted, let origin = firstFrameTime, origin.isNumeric,
                  let time = screenClockTime, time.isNumeric else {
                return (fallback, totalPauseDuration, hasVideo, firstFrameTime, clock)
            }
            let position = CMTimeSubtract(CMTimeSubtract(pauseStartTime ?? time, origin), totalPauseDuration)
            guard position.isNumeric else { return (fallback, totalPauseDuration, hasVideo, origin, clock) }
            return (CMTimeMaximum(fallback, CMTimeMaximum(position, .zero)), totalPauseDuration, hasVideo, origin, clock)
        }
    }

    private var latestScreenPresentationTime: CMTime {
        guard let firstFrameTime, firstFrameTime.isNumeric, lastFrameTime.isNumeric else {
            return lastVideoTime
        }
        let time = CMTimeSubtract(CMTimeSubtract(lastFrameTime, firstFrameTime), totalPauseDuration)
        guard time.isNumeric else { return lastVideoTime }
        return CMTimeMaximum(time, lastVideoTime)
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
        try cameraRecorder.start(paused: state == .paused || pauseStartTime != nil)
        cameraEnabled = true
        if sessionStarted && !syncCameraWithScreenTimeline() {
            cameraRecorder.abort()
            cameraEnabled = false
            throw RecorderError.capture("Could not synchronize camera and screen clocks")
        }
    }

    private func syncCameraWithScreenTimeline() -> Bool {
        guard #available(macOS 13.0, *), let origin = firstFrameTime,
              let clock = stream?.synchronizationClock else { return false }
        return cameraRecorder.syncWithScreenStart(origin: origin, clock: clock, pause: totalPauseDuration)
    }

    private func openCameraRange() {
        guard cameraRangeOpenStart == nil else { return }
        cameraRangeOpenStart = cameraTimelinePosition
    }

    private func closeCameraRange(at position: Double? = nil) {
        guard let start = cameraRangeOpenStart else { return }
        cameraRangeOpenStart = nil
        cameraVisibleRanges.append((start, max(position ?? cameraTimelinePosition, start)))
    }

    private var cameraTimelinePosition: Double {
        videoQueue.sync {
            guard let time = screenClockTime else { return recordingDuration }
            return cameraTimelinePosition(at: time)
        }
    }

    private var screenClockTime: CMTime? {
        #if RECORDING_TIMELINE_TEST
        if let time = testCameraClockTime { return time }
        #endif
        guard #available(macOS 13.0, *), let clock = stream?.synchronizationClock else { return nil }
        return CMClockGetTime(clock)
    }

    private func cameraTimelinePosition(at time: CMTime) -> Double {
        guard sessionStarted, let origin = firstFrameTime, origin.isNumeric else {
            return recordingDuration
        }
        let position = CMTimeSubtract(CMTimeSubtract(pauseStartTime ?? time, origin), totalPauseDuration)
        guard position.isNumeric else { return recordingDuration }
        let seconds = CMTimeGetSeconds(position)
        guard seconds.isFinite else { return recordingDuration }
        return max(0, seconds)
    }

    func captureOutput(
        _ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        guard state == .recording else { return }
        guard sessionStarted else { return }
        guard micAudioSessionStarted else { return }

        micSampleCount += 1
        writeMicAudioSample(sampleBuffer, clock: micCaptureSession?.synchronizationClock)
    }

    private func writeMicAudioSample(_ sampleBuffer: CMSampleBuffer, clock: CMClock? = nil) {
        guard micBufferQueue != nil else { return }
        let screenTimeline = microphoneScreenTimeline
        guard screenTimeline.hasVideo else { return }

        let micTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        var sourceClock: AnyObject? = clock
        #if RECORDING_TIMELINE_TEST
        if let testMicrophoneClock { sourceClock = testMicrophoneClock }
        #endif
        var capturedPosition: CMTime?
        if (micPendingStartTime != nil || firstMicTime == nil),
           let sourceClock, let screenClock = screenTimeline.clock, let origin = screenTimeline.origin, origin.isNumeric {
            let screenTime = CMSyncConvertTime(micTime, from: sourceClock, to: screenClock)
            let position = CMTimeSubtract(CMTimeSubtract(screenTime, origin), screenTimeline.paused)
            if position.isNumeric { capturedPosition = CMTimeMaximum(position, .zero) }
        }

        if let pendingStart = micPendingStartTime {
            micPendingStartTime = nil
            let start = CMTimeMaximum(CMTimeMaximum(pendingStart, capturedPosition ?? .zero), lastMicWriteEndTime ?? .zero)
            firstMicTime = CMTimeSubtract(micTime, start)
            micPauseAnchor = screenTimeline.paused
            guard appendMicSilence(
                from: lastMicWriteEndTime ?? .zero,
                until: start,
                like: sampleBuffer
            ) else { return }
        } else if firstMicTime == nil {
            let start = CMTimeMaximum(capturedPosition ?? .zero, lastMicWriteEndTime ?? .zero)
            firstMicTime = CMTimeSubtract(micTime, start)
            micPauseAnchor = screenTimeline.paused
            guard appendMicSilence(from: lastMicWriteEndTime ?? .zero, until: start, like: sampleBuffer) else { return }
        }

        guard let firstMic = firstMicTime else { return }

        let micRelativeTime = CMTimeSubtract(micTime, firstMic)
        var adjustedTime = micRelativeTime
        adjustedTime = CMTimeSubtract(
            adjustedTime,
            CMTimeSubtract(screenTimeline.paused, micPauseAnchor)
        )

        if CMTimeCompare(adjustedTime, .zero) < 0 {
            adjustedTime = .zero
        }

        let maxTime = CMTimeAdd(screenTimeline.position, CMTimeMake(value: 1, timescale: 10))
        if CMTimeCompare(adjustedTime, maxTime) > 0 {
            adjustedTime = maxTime
        }
        if let endTime = lastMicWriteEndTime, CMTimeCompare(adjustedTime, endTime) < 0 {
            adjustedTime = endTime
        }

        guard let adjustedBuffer = createAdjustedSampleBuffer(sampleBuffer, newTime: adjustedTime) else {
            micBufferQueue?.reportFailure(RecorderError.capture("Could not time microphone audio"))
            return
        }
        _ = appendMicBufferToWriter(adjustedBuffer)
    }

    private func appendMicSilence(
        from startTime: CMTime,
        until endTime: CMTime,
        like sampleBuffer: CMSampleBuffer
    ) -> Bool {
        guard CMTimeCompare(startTime, endTime) < 0 else { return true }

        guard let muted = createMutedSampleBuffer(from: sampleBuffer) else {
            micBufferQueue?.reportFailure(RecorderError.capture("Could not create microphone silence"))
            return false
        }
        let step = CMSampleBufferGetDuration(muted)
        guard step.isNumeric, CMTimeCompare(step, .zero) > 0 else {
            micBufferQueue?.reportFailure(RecorderError.capture("Invalid microphone silence duration"))
            return false
        }
        if micBufferQueue?.enqueueSilence(muted, from: startTime, until: endTime) == true {
            lastMicWriteEndTime = endTime
            return true
        }
        return false
    }

    private func silencePrefix(_ buffer: CMSampleBuffer, fitting duration: CMTime) -> CMSampleBuffer? {
        if CMTimeCompare(CMSampleBufferGetDuration(buffer), duration) <= 0 { return buffer }
        let start = CMSampleBufferGetPresentationTimeStamp(buffer)
        var low = 0
        var high = CMSampleBufferGetNumSamples(buffer)
        while low < high {
            let mid = low + (high - low + 1) / 2
            var timing = CMSampleTimingInfo()
            guard CMSampleBufferGetSampleTimingInfo(buffer, at: mid - 1, timingInfoOut: &timing) == noErr,
                  timing.presentationTimeStamp.isNumeric, timing.duration.isNumeric else { return nil }
            let end = CMTimeSubtract(CMTimeAdd(timing.presentationTimeStamp, timing.duration), start)
            if CMTimeCompare(end, duration) <= 0 {
                low = mid
            } else {
                high = mid - 1
            }
        }
        guard low > 0 else { return nil }
        var prefix: CMSampleBuffer?
        guard CMSampleBufferCopySampleBufferForRange(
            allocator: kCFAllocatorDefault, sampleBuffer: buffer,
            sampleRange: CFRange(location: 0, length: low), sampleBufferOut: &prefix
        ) == noErr else { return nil }
        return prefix
    }

    private func configureMicrophoneBufferQueue() {
        guard let writer = micAudioAssetWriter, let input = micAudioInput else { return }
        micBufferQueue = makeAudioBufferQueue(writer: writer, input: input) { [weak self] buffer in
#if RECORDING_TIMELINE_TEST
            self?.testMicrophoneWrites.append((
                CMSampleBufferGetPresentationTimeStamp(buffer), CMSampleBufferGetDuration(buffer)
            ))
#endif
        }
    }

    private func configureSystemAudioBufferQueue() {
        guard let writer = systemAudioAssetWriter, let input = systemAudioInput else { return }
        systemAudioBufferQueue = makeAudioBufferQueue(writer: writer, input: input) { _ in }
    }

    private func makeAudioBufferQueue(
        writer: AVAssetWriter,
        input: AVAssetWriterInput,
        onWrite: @escaping (CMSampleBuffer) -> Void
    ) -> AudioBufferQueue {
        let sessionID = activeSessionID
        return AudioBufferQueue(
            writer: writer,
            input: input,
            prefix: { [weak self] buffer, duration in self?.silencePrefix(buffer, fitting: duration) },
            adjust: { [weak self] buffer, time in self?.createAdjustedSampleBuffer(buffer, newTime: time) },
            onWrite: onWrite,
            onFailure: { [weak self] error in
                Task { @MainActor [weak self] in
                    guard let self, let sessionID, self.activeSessionID == sessionID else { return }
                    self.onError?(sessionID, error)
                }
            }
        )
    }

    private func appendMicBufferToWriter(_ sampleBuffer: CMSampleBuffer) -> Bool {
        guard micBufferQueue?.enqueue(sampleBuffer) == true else { return false }
        lastMicWriteEndTime = CMTimeAdd(
            CMSampleBufferGetPresentationTimeStamp(sampleBuffer),
            CMSampleBufferGetDuration(sampleBuffer)
        )
        return true
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
        guard var timing = sampleTimingEntries(originalBuffer) else { return nil }
        
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
            sampleTimingEntryCount: timing.count,
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

        videoQueue.sync {
            if sessionStarted {
                if let time = screenClockTime, time.isNumeric, CMTimeCompare(time, lastFrameTime) >= 0 {
                    pauseStartTime = time
                } else {
                    pauseStartTime = lastFrameTime
                }
            } else {
                pauseStartTime = nil
            }
            state = .paused
        }

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

        videoQueue.sync {
            if let start = pauseStartTime, let time = screenClockTime,
               time.isNumeric, CMTimeCompare(time, start) >= 0 {
                totalPauseDuration = CMTimeAdd(totalPauseDuration, CMTimeSubtract(time, start))
                pauseStartTime = nil
                if cameraEnabled {
                    cameraRecorder.resumeWithScreenPause(totalPauseDuration)
                }
            }
            state = .recording
        }

        cursorTracker.resume()

        if keyboardEnabled {
            keyboardTracker.resume()
        }

    }

    @MainActor
    func stop() async throws -> RecordingResult {
        guard lifecycle.beginStop(state: state) else {
            throw RecorderError.invalidState(
                "Cannot stop: recorder is \(state.rawValue) or a lifecycle transition is in progress"
            )
        }

        let stoppedPosition = videoQueue.sync {
            let position = screenClockTime.map { cameraTimelinePosition(at: $0) } ?? recordingDuration
            state = .idle
            return max(position, recordingDuration)
        }
        defer {
            resetRecordingState()
            lifecycle.finishStop()
        }

        let cursorFilePath = cursorTracker.stop()

        var keysFilePath: String? = nil
        if keyboardEnabled {
            keysFilePath = keyboardTracker.stop()
        }

        var cameraFilePath: String? = nil
        if cameraEnabled {
            closeCameraRange(at: stoppedPosition)
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
        RecordingCaptureWindowRegistry.shared.setFilterUpdater(nil)

        waitForPendingSamples()

        var finishingError: Error?
        var finalDuration = stoppedPosition
        if let writer = assetWriter, let input = videoInput, let adaptor = pixelBufferAdaptor,
           writer.status == .writing {
            do {
                if let pending = pendingVideoFrame {
                    let finalizer = VideoFrameFinalizer(writer: writer, input: input, adaptor: adaptor, frame: pending.frame, time: pending.time)
                    try await finalizer.append()
                    lastVideoFrame = pending.frame
                    lastVideoTime = pending.time
                    videoFrameCount += 1
                    pendingVideoFrame = nil
                }
            } catch {
                finishingError = error
            }
        }
        if let writer = assetWriter, let input = videoInput, let adaptor = pixelBufferAdaptor,
           let frame = lastVideoFrame, writer.status == .writing, finishingError == nil {
            let stopTime = CMTime(seconds: stoppedPosition, preferredTimescale: 600_000_000)
            let endTime = CMTimeMaximum(stopTime, CMTimeAdd(lastVideoTime, videoFrameDuration))
            let frameTime = CMTimeMaximum(
                CMTimeSubtract(endTime, videoFrameDuration),
                CMTimeAdd(lastVideoTime, CMTimeMultiplyByRatio(CMTimeSubtract(endTime, lastVideoTime), multiplier: 1, divisor: 2))
            )
            let finalizer = VideoFrameFinalizer(writer: writer, input: input, adaptor: adaptor, frame: frame, time: frameTime)
            do {
                try await finalizer.append()
                writer.endSession(atSourceTime: endTime)
                finalDuration = CMTimeGetSeconds(endTime)
            } catch {
                finishingError = error
            }
        }
        videoInput?.markAsFinished()
        if let systemAudioBufferQueue {
            do {
                try await systemAudioBufferQueue.finish()
            } catch {
                finishingError = error
                if systemAudioAssetWriter?.status == .writing {
                    systemAudioInput?.markAsFinished()
                }
            }
        } else {
            systemAudioInput?.markAsFinished()
        }
        if let micBufferQueue {
            do {
                try await micBufferQueue.finish()
            } catch {
                if finishingError == nil { finishingError = error }
                if micAudioAssetWriter?.status == .writing {
                    micAudioInput?.markAsFinished()
                }
            }
        } else {
            micAudioInput?.markAsFinished()
        }

        let finalPath = config?.outputPath ?? ""
        var finalSystemAudioPath = systemAudioOutputPath
        let finalMicAudioPath = micAudioOutputPath

        if assetWriter?.status == .writing {
            await assetWriter?.finishWriting()
        }

        if assetWriter?.status == .failed, let error = assetWriter?.error {
            if finishingError == nil {
                finishingError = RecorderError.capture("Asset writer error: \(error.localizedDescription)")
            }
        }

        if systemAudioAssetWriter?.status == .writing {
            await systemAudioAssetWriter?.finishWriting()
        }

        if systemAudioAssetWriter?.status == .failed, let error = systemAudioAssetWriter?.error {
            if finishingError == nil {
                finishingError = RecorderError.capture("System audio writer error: \(error.localizedDescription)")
            }
        }

        if micAudioAssetWriter?.status == .writing {
            await micAudioAssetWriter?.finishWriting()
        }

        if micAudioAssetWriter?.status == .failed, let error = micAudioAssetWriter?.error {
            if finishingError == nil {
                finishingError = RecorderError.capture("Mic audio writer error: \(error.localizedDescription)")
            }
        }

        if let finishingError { throw finishingError }

        if let path = finalSystemAudioPath, !systemAudioEverActive {
            try? FileManager.default.removeItem(atPath: path)
            finalSystemAudioPath = nil
        }

        return RecordingResult(
            outputPath: finalPath,
            cursorPath: cursorFilePath,
            cameraPath: cameraFilePath,
            cameraTrackMissing: cameraEnabled && cameraFilePath == nil,
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
        micQueue.sync {}
        writerQueue.sync {}
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        Task { @MainActor [weak self] in
            guard let self else { return }
            self.handleStreamFailure(error, isCurrentStream: self.stream === stream)
        }
    }

    @MainActor
    private func handleStreamFailure(_ error: Error, isCurrentStream: Bool) {
        guard isCurrentStream,
              state == .recording || state == .paused || lifecycle.isStarting,
              let sessionID = activeSessionID else { return }
        onError?(
            sessionID,
            RecorderError.streamFailure(error, capturingWindow: config?.windowID != nil)
        )
    }

    @MainActor
    private func resetRecordingState() {
        RecordingCaptureWindowRegistry.shared.finishCaptureStart()
        RecordingCaptureWindowRegistry.shared.setFilterUpdater(nil)
        assetWriter = nil
        videoInput = nil
        pixelBufferAdaptor = nil
        lastVideoFrame = nil
        pendingVideoFrame = nil
        systemAudioAssetWriter = nil
        systemAudioInput = nil
        systemAudioBufferQueue = nil
        systemAudioOutputPath = nil
        systemAudioSessionStarted = false
        systemAudioActive = false
        systemAudioEverActive = false
        lastSystemAudioEndTime = .zero
        systemAudioWrittenEndTime = .zero
        systemAudioGapRemainder = .zero
        micAudioAssetWriter = nil
        micAudioInput = nil
        micBufferQueue = nil
        micAudioOutputPath = nil
        micAudioSessionStarted = false
        micPendingStartTime = nil
        micPauseAnchor = .zero
        lastMicWriteEndTime = nil
        firstMicTime = nil
        cameraVisibleRanges = []
        cameraRangeOpenStart = nil
        sessionStarted = false
        firstFrameTime = nil
        pauseStartTime = nil
        totalPauseDuration = .zero
        recordingDuration = 0
        activeSessionID = nil
    }
}

@available(macOS 12.3, *)
extension ScreenCaptureRecorder: SCStreamOutput {
    func stream(
        _ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of type: SCStreamOutputType
    ) {
        handleStreamSample(sampleBuffer, type: type)
    }

    private func handleStreamSample(_ sampleBuffer: CMSampleBuffer, type: SCStreamOutputType) {
        guard CMSampleBufferDataIsReady(sampleBuffer) else { return }
        if type == .screen && ScreenCaptureFrame.isStopped(sampleBuffer) { return }

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

        if !sessionStarted {
            guard ScreenCaptureFrame.canAnchorTimeline(sampleBuffer, type: type) else { return }
        }

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
                if !syncCameraWithScreenTimeline(), let sessionID = activeSessionID {
                    Task { @MainActor in
                        self.onError?(sessionID, RecorderError.capture("Could not synchronize camera and screen clocks"))
                    }
                }
            }
            onFirstFrame?()
        }

        if cameraEnabled {
            cameraRecorder.resumeWithScreenPause(totalPauseDuration)
        }

        if !systemAudioSessionStarted && systemAudioAssetWriter != nil {
            systemAudioAssetWriter?.startSession(atSourceTime: .zero)
            configureSystemAudioBufferQueue()
            systemAudioSessionStarted = true
        }

        startMicrophoneWriterIfReady()

        guard let firstTime = firstFrameTime else { return }

        var presentationTime = CMTimeSubtract(currentTime, firstTime)
        presentationTime = CMTimeSubtract(presentationTime, totalPauseDuration)
        var timedSampleBuffer = sampleBuffer

        if CMTimeCompare(presentationTime, .zero) < 0 {
            if #available(macOS 13.0, *), type == .audio {
                let timelineStart = CMTimeAdd(firstTime, totalPauseDuration)
                do {
                    guard let suffix = try audioSuffix(sampleBuffer, atOrAfter: timelineStart) else { return }
                    timedSampleBuffer = suffix
                    presentationTime = CMTimeSubtract(CMSampleBufferGetPresentationTimeStamp(suffix), timelineStart)
                } catch {
                    systemAudioBufferQueue?.reportFailure(error)
                    return
                }
            } else {
                presentationTime = .zero
            }
        }

        if type == .screen {
            lastFrameTime = currentTime
            recordingDuration = CMTimeGetSeconds(presentationTime)

            guard let videoInput = videoInput,
                let pixelBufferAdaptor = pixelBufferAdaptor
            else { return }

            if videoFrameCount > 0 && CMTimeCompare(presentationTime, lastVideoTime) <= 0 {
                return
            }

            if let pendingVideoFrame, CMTimeCompare(presentationTime, pendingVideoFrame.time) <= 0 {
                return
            }

            guard let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }

            let time = presentationTime
            writerQueue.sync {
                #if RECORDING_TIMELINE_TEST
                if testVideoBackpressure {
                    pendingVideoFrame = (imageBuffer, time)
                    return
                }
                #endif
                if let pending = pendingVideoFrame {
                    guard appendVideoFrame(pending.frame, at: pending.time, input: videoInput, adaptor: pixelBufferAdaptor) else {
                        pendingVideoFrame = (imageBuffer, time)
                        return
                    }
                    pendingVideoFrame = nil
                }
                if !appendVideoFrame(imageBuffer, at: time, input: videoInput, adaptor: pixelBufferAdaptor) {
                    pendingVideoFrame = (imageBuffer, time)
                }
            }
        } else if #available(macOS 13.0, *), type == .audio {
            if systemAudioActive {
                appendSystemAudioBuffer(timedSampleBuffer, at: presentationTime)
            } else if let muted = createMutedSampleBuffer(from: timedSampleBuffer) {
                appendSystemAudioBuffer(muted, at: presentationTime)
            } else {
                systemAudioBufferQueue?.reportFailure(RecorderError.capture("Could not create system audio silence"))
            }
        }
    }

    private func appendVideoFrame(_ frame: CVPixelBuffer, at time: CMTime, input: AVAssetWriterInput,
                                  adaptor: AVAssetWriterInputPixelBufferAdaptor) -> Bool {
        guard assetWriter?.status == .writing, input.isReadyForMoreMediaData,
              adaptor.append(frame, withPresentationTime: time) else { return false }
        videoFrameCount += 1
        lastVideoTime = time
        lastVideoFrame = frame
        return true
    }

    #if RECORDING_TIMELINE_TEST
    @MainActor
    func testBeginStreamStartup(config: RecordingConfig, sessionID: RecorderSessionID) {
        precondition(lifecycle.beginStart(state: state))
        self.config = config
        activeSessionID = sessionID
    }

    @MainActor
    func testStreamFailure(_ error: Error, isCurrentStream: Bool) {
        handleStreamFailure(error, isCurrentStream: isCurrentStream)
    }

    @MainActor
    func testFinishStreamStartup(recording: Bool) {
        lifecycle.finishStart()
        state = recording ? .recording : .idle
        if !recording { activeSessionID = nil }
    }

    func testStartTimeline() {
        state = .recording
    }

    func testSetCameraClockTime(_ time: CMTime?) {
        testCameraClockTime = time
    }

    func testSetMicrophoneClocks(_ microphone: AnyObject?, _ screen: CMClock?) {
        testMicrophoneClock = microphone
        testMicrophoneScreenClock = screen
    }

    func testSetVideoBackpressure(_ enabled: Bool) {
        testVideoBackpressure = enabled
    }

    func testOpenCameraRange() { openCameraRange() }
    func testCloseCameraRange(at position: Double? = nil) { closeCameraRange(at: position) }
    var testCameraRanges: [(start: Double, end: Double)] { cameraVisibleRanges }

    func testProcessSample(_ sampleBuffer: CMSampleBuffer, type: SCStreamOutputType) {
        handleStreamSample(sampleBuffer, type: type)
    }

    func testBeginResume(from time: CMTime) {
        pauseStartTime = time
    }

    var testTimeline: (started: Bool, first: CMTime?, pause: CMTime?, pausedDuration: CMTime) {
        (sessionStarted, firstFrameTime, pauseStartTime, totalPauseDuration)
    }

    func testMutedSample(_ sampleBuffer: CMSampleBuffer) -> CMSampleBuffer? {
        createMutedSampleBuffer(from: sampleBuffer)
    }

    func testAdjustedSample(_ sampleBuffer: CMSampleBuffer, at time: CMTime) -> CMSampleBuffer? {
        createAdjustedSampleBuffer(sampleBuffer, newTime: time)
    }

    func testSilencePrefix(_ sampleBuffer: CMSampleBuffer, fitting duration: CMTime) -> CMSampleBuffer? {
        silencePrefix(sampleBuffer, fitting: duration)
    }

    func testConfigureMicrophone(writer: AVAssetWriter, input: AVAssetWriterInput, writtenVideo: Bool = true) {
        micAudioAssetWriter = writer
        micAudioInput = input
        if writtenVideo {
            videoFrameCount = 1
            lastVideoTime = CMTime(value: 1, timescale: 1)
        }
        configureMicrophoneBufferQueue()
    }

    func testInstallMicrophoneWriter(_ writer: AVAssetWriter, input: AVAssetWriterInput) {
        installMicrophoneWriter(writer, input: input)
    }

    var testMicrophoneWriterStarted: Bool { videoQueue.sync { micAudioSessionStarted } }

    func testWriteMicrophone(_ sampleBuffer: CMSampleBuffer, pendingStart: CMTime) {
        micPendingStartTime = pendingStart
        writeMicAudioSample(sampleBuffer)
    }

    func testEnableMicrophone(_ sampleBuffer: CMSampleBuffer) {
        micPendingStartTime = microphoneStartTime
        writeMicAudioSample(sampleBuffer)
    }

    func testEnableMicrophoneDuringSetup(_ sampleBuffer: CMSampleBuffer) throws {
        try setupMicrophoneCaptureForRecording(at: microphoneStartTime) {
            self.micQueue.sync { self.writeMicAudioSample(sampleBuffer) }
        }
    }

    func testArmMicrophoneDuringSetup() throws {
        try setupMicrophoneCaptureForRecording(at: microphoneStartTime) {}
    }

    func testFailedMicrophoneSetup() throws {
        try setupMicrophoneCaptureForRecording(at: CMTime(value: 20, timescale: 1)) {
            throw RecorderError.configuration("Synthetic setup failure")
        }
    }

    var testMicrophonePending: CMTime? { micQueue.sync { micPendingStartTime } }

    var testMicrophoneEnd: CMTime? { lastMicWriteEndTime }

    func testContinueMicrophone(_ sampleBuffer: CMSampleBuffer) {
        writeMicAudioSample(sampleBuffer)
    }

    func testFinishMicrophone() async throws {
        try await micBufferQueue?.finish()
    }

    func testConfigureSystemAudio(writer: AVAssetWriter, input: AVAssetWriterInput) {
        systemAudioAssetWriter = writer
        systemAudioInput = input
        configureSystemAudioBufferQueue()
    }

    func testConfigureSystemAudioForStream(writer: AVAssetWriter, input: AVAssetWriterInput) {
        testConfigureSystemAudio(writer: writer, input: input)
        systemAudioSessionStarted = true
        systemAudioActive = true
    }

    func testWriteSystemAudio(_ sampleBuffer: CMSampleBuffer, at time: CMTime) {
        appendSystemAudioBuffer(sampleBuffer, at: time)
    }

    func testFinishSystemAudio() async throws {
        try await systemAudioBufferQueue?.finish()
    }

    func testConfigureStop(writer: AVAssetWriter, input: AVAssetWriterInput) {
        assetWriter = writer
        videoInput = input
        state = .recording
    }

    func testConfigureScreenWriter(writer: AVAssetWriter, input: AVAssetWriterInput, adaptor: AVAssetWriterInputPixelBufferAdaptor) {
        assetWriter = writer
        videoInput = input
        pixelBufferAdaptor = adaptor
        state = .recording
    }

    func testConfigureCamera(writer: AVAssetWriter, input: AVAssetWriterInput, path: String) {
        cameraRecorder.testConfigureWriter(writer, input: input, path: path)
        cameraEnabled = true
    }
    #endif

    private func appendSystemAudioBuffer(_ sampleBuffer: CMSampleBuffer, at time: CMTime) {
        guard time.isNumeric, CMTimeCompare(time, .zero) >= 0 else {
            systemAudioBufferQueue?.reportFailure(RecorderError.capture("Invalid system audio timestamp"))
            return
        }
        let duration = CMSampleBufferGetDuration(sampleBuffer)
        guard duration.isNumeric, CMTimeCompare(duration, .zero) > 0 else {
            systemAudioBufferQueue?.reportFailure(RecorderError.capture("Invalid system audio duration"))
            return
        }
        guard let adjustedBuffer = createAdjustedSampleBuffer(sampleBuffer, newTime: time) else {
            systemAudioBufferQueue?.reportFailure(RecorderError.capture("Could not time system audio"))
            return
        }
        guard let systemAudioBufferQueue else { return }
        let end = CMTimeAdd(time, duration)
        guard end.isNumeric else {
            systemAudioBufferQueue.reportFailure(RecorderError.capture("Invalid system audio end time"))
            return
        }
        guard CMTimeCompare(end, lastSystemAudioEndTime) > 0 else { return }
        let remaining: CMSampleBuffer
        if CMTimeCompare(time, lastSystemAudioEndTime) < 0 {
            do {
                guard let suffix = try audioSuffix(adjustedBuffer, atOrAfter: lastSystemAudioEndTime) else { return }
                remaining = suffix
            } catch {
                systemAudioBufferQueue.reportFailure(error)
                return
            }
        } else {
            remaining = adjustedBuffer
        }
        let start = CMSampleBufferGetPresentationTimeStamp(remaining)
        let remainingDuration = CMSampleBufferGetDuration(remaining)
        guard start.isNumeric, remainingDuration.isNumeric,
              CMTimeCompare(remainingDuration, .zero) > 0 else {
            systemAudioBufferQueue.reportFailure(RecorderError.capture("Invalid trimmed system audio timing"))
            return
        }
        let gap = CMTimeAdd(systemAudioGapRemainder, CMTimeSubtract(start, lastSystemAudioEndTime))
        guard gap.isNumeric, CMTimeCompare(gap, .zero) >= 0 else {
            systemAudioBufferQueue.reportFailure(RecorderError.capture("Invalid system audio gap"))
            return
        }
        let padding: CMTime
        do {
            padding = try audioPaddingDuration(sampleBuffer, fitting: gap)
        } catch {
            systemAudioBufferQueue.reportFailure(error)
            return
        }
        let outputStart = CMTimeAdd(systemAudioWrittenEndTime, padding)
        guard outputStart.isNumeric else {
            systemAudioBufferQueue.reportFailure(RecorderError.capture("Invalid system audio output time"))
            return
        }
        if CMTimeCompare(padding, .zero) > 0 {
            guard let silence = createMutedSampleBuffer(from: sampleBuffer) else {
                systemAudioBufferQueue.reportFailure(RecorderError.capture("Could not create system audio silence"))
                return
            }
            guard systemAudioBufferQueue.enqueueSilence(silence, from: systemAudioWrittenEndTime, until: outputStart) else { return }
        }
        let output: CMSampleBuffer
        if CMTimeCompare(start, outputStart) == 0 {
            output = remaining
        } else if let aligned = createAdjustedSampleBuffer(remaining, newTime: outputStart) {
            output = aligned
        } else {
            systemAudioBufferQueue.reportFailure(RecorderError.capture("Could not align system audio"))
            return
        }
        guard systemAudioBufferQueue.enqueue(output) else { return }
        lastSystemAudioEndTime = CMTimeAdd(start, remainingDuration)
        systemAudioWrittenEndTime = CMTimeAdd(outputStart, remainingDuration)
        systemAudioGapRemainder = CMTimeSubtract(gap, padding)
    }

    private func audioPaddingDuration(_ buffer: CMSampleBuffer, fitting gap: CMTime) throws -> CMTime {
        guard CMTimeCompare(gap, .zero) > 0 else { return .zero }
        let duration = CMSampleBufferGetDuration(buffer)
        guard duration.isNumeric, duration.value > 0, duration.timescale > 0 else {
            throw RecorderError.capture("Invalid system audio padding duration")
        }
        let ticks = CMTimeConvertScale(gap, timescale: duration.timescale, method: .roundTowardZero)
        guard ticks.isNumeric, ticks.value >= 0 else {
            throw RecorderError.capture("Invalid system audio padding gap")
        }
        let whole = CMTime(value: ticks.value - ticks.value % duration.value, timescale: duration.timescale)
        let remaining = CMTimeSubtract(gap, whole)
        if CMTimeCompare(remaining, .zero) == 0 { return whole }
        if let prefix = silencePrefix(buffer, fitting: remaining) {
            return CMTimeAdd(whole, CMSampleBufferGetDuration(prefix))
        }
        var first = CMSampleTimingInfo()
        guard CMSampleBufferGetSampleTimingInfo(buffer, at: 0, timingInfoOut: &first) == noErr,
              first.duration.isNumeric, CMTimeCompare(first.duration, .zero) > 0,
              CMTimeCompare(remaining, first.duration) < 0 else {
            throw RecorderError.capture("Could not calculate system audio padding")
        }
        return whole
    }

    private func audioSuffix(_ buffer: CMSampleBuffer, atOrAfter time: CMTime) throws -> CMSampleBuffer? {
        let count = CMSampleBufferGetNumSamples(buffer)
        var low = 0
        var high = count
        while low < high {
            let mid = low + (high - low) / 2
            var timing = CMSampleTimingInfo()
            guard CMSampleBufferGetSampleTimingInfo(buffer, at: mid, timingInfoOut: &timing) == noErr,
                  timing.presentationTimeStamp.isNumeric else {
                throw RecorderError.capture("Could not inspect system audio timing")
            }
            if CMTimeCompare(timing.presentationTimeStamp, time) < 0 {
                low = mid + 1
            } else {
                high = mid
            }
        }
        guard low < count else { return nil }
        var suffix: CMSampleBuffer?
        guard CMSampleBufferCopySampleBufferForRange(
            allocator: kCFAllocatorDefault, sampleBuffer: buffer,
            sampleRange: CFRange(location: low, length: count - low), sampleBufferOut: &suffix
        ) == noErr, let suffix else {
            throw RecorderError.capture("Could not trim overlapping system audio")
        }
        return suffix
    }

    private func createAdjustedSampleBuffer(_ buffer: CMSampleBuffer, newTime: CMTime)
        -> CMSampleBuffer?
    {
        guard var timing = sampleTimingEntries(buffer) else { return nil }
        let offset = CMTimeSubtract(newTime, CMSampleBufferGetPresentationTimeStamp(buffer))
        guard offset.isNumeric else { return nil }
        for index in timing.indices {
            if timing[index].presentationTimeStamp.isNumeric {
                timing[index].presentationTimeStamp = CMTimeAdd(timing[index].presentationTimeStamp, offset)
            }
            if timing[index].decodeTimeStamp.isNumeric {
                timing[index].decodeTimeStamp = CMTimeAdd(timing[index].decodeTimeStamp, offset)
            }
        }

        var newBuffer: CMSampleBuffer?
        let status = CMSampleBufferCreateCopyWithNewTiming(
            allocator: kCFAllocatorDefault,
            sampleBuffer: buffer,
            sampleTimingEntryCount: timing.count,
            sampleTimingArray: &timing,
            sampleBufferOut: &newBuffer
        )

        return status == noErr ? newBuffer : nil
    }

    private func sampleTimingEntries(_ buffer: CMSampleBuffer) -> [CMSampleTimingInfo]? {
        var count = 0
        guard CMSampleBufferGetSampleTimingInfoArray(
            buffer, entryCount: 0, arrayToFill: nil, entriesNeededOut: &count
        ) == noErr, count > 0 else { return nil }
        var timing = [CMSampleTimingInfo](
            repeating: CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: .invalid, decodeTimeStamp: .invalid),
            count: count
        )
        guard CMSampleBufferGetSampleTimingInfoArray(
            buffer, entryCount: count, arrayToFill: &timing, entriesNeededOut: &count
        ) == noErr else { return nil }
        return timing
    }
}
