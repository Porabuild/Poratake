import AVFoundation
import CoreMedia
import CoreMediaIO
import Foundation

class IOSDeviceRecorder: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate, AVCaptureAudioDataOutputSampleBufferDelegate {
    private var captureSession: AVCaptureSession?
    private var runtimeErrorObserver: NSObjectProtocol?
    private var iosAudioOutput: AVCaptureAudioDataOutput?
    private var micCaptureSession: AVCaptureSession?
    private var micAudioOutput: AVCaptureAudioDataOutput?
    private var assetWriter: AVAssetWriter?
    private var videoInput: AVAssetWriterInput?
    private var pixelBufferAdaptor: AVAssetWriterInputPixelBufferAdaptor?
    private var micAudioAssetWriter: AVAssetWriter?
    private var micAudioInput: AVAssetWriterInput?
    private var micAudioOutputPath: String?
    private var micAudioSessionStarted = false
    private var systemAudioAssetWriter: AVAssetWriter?
    private var systemAudioInput: AVAssetWriterInput?
    private var systemAudioOutputPath: String?
    private var systemAudioSessionStarted = false
    private var systemAudioInputConfigured = false
    private var firstMicTime: CMTime?
    private let micQueue = DispatchQueue(label: "com.porabuild.poratake.ios-recorder.mic")

    private let cameraRecorder = CameraRecorder()
    private var cameraEnabled = false
    
    private var sessionStarted = false
    private var assetWriterConfigured = false
    private var firstFrameTime: CMTime?
    private var lastFrameTime: CMTime = .zero
    private var pauseStartTime: CMTime?
    private var totalPauseDuration: CMTime = .zero
    
    private let videoQueue = DispatchQueue(label: "com.porabuild.poratake.ios-recorder.video")
    private let audioQueue = DispatchQueue(label: "com.porabuild.poratake.ios-recorder.audio")
    private let writerQueue = DispatchQueue(label: "com.porabuild.poratake.ios-recorder.writer")
    
    private(set) var state: RecorderState = .idle
    private var config: RecordingConfig?
    private var recordingDuration: Double = 0
    private var videoWidth: Int = 0
    private var videoHeight: Int = 0
    
    private var videoFrameCount: Int = 0
    private var pendingAudioBuffers: [CMSampleBuffer] = []
    private let firstFrameLock = NSLock()
    private var firstFrameContinuation: CheckedContinuation<Void, Error>?
    private var firstFrameTimeoutTask: Task<Void, Never>?
    private var firstFrameError: Error?
    
    var onFirstFrame: (() -> Void)?
    var onError: ((Error) -> Void)?
    
    func configure(_ config: RecordingConfig) {
        self.config = config
    }
    
    func start(waitForFirstFrame: Bool = false) async throws {
        guard state == .idle else {
            throw RecorderError.invalidState("Cannot start: recorder is \(state.rawValue)")
        }
        
        guard let config = config else {
            throw RecorderError.configuration("Recording config not set")
        }
        
        guard let deviceId = config.iosDeviceId else {
            throw RecorderError.configuration("iOS device ID not set")
        }
        
        do {
            try await startCapture(
                config,
                deviceId: deviceId,
                waitForFirstFrame: waitForFirstFrame
            )
        } catch {
            rollbackFailedStart()
            throw error
        }
    }

    private func startCapture(
        _ config: RecordingConfig,
        deviceId: String,
        waitForFirstFrame shouldWaitForFirstFrame: Bool
    ) async throws {
        enableScreenCaptureDevices()
        
        let fileManager = FileManager.default
        if fileManager.fileExists(atPath: config.outputPath) {
            try fileManager.removeItem(atPath: config.outputPath)
        }
        
        guard let device = findIOSDevice(deviceId: deviceId, deviceName: config.iosDeviceName) else {
            throw RecorderError.configuration("iOS device not found: \(config.iosDeviceName ?? deviceId)")
        }
        
        let session = AVCaptureSession()
        captureSession = session
        observeRuntimeErrors(for: session)
        
        let deviceInput = try AVCaptureDeviceInput(device: device)
        
        if captureSession?.canAddInput(deviceInput) == true {
            captureSession?.addInput(deviceInput)
        } else {
            throw RecorderError.configuration("Cannot add iOS device input")
        }
        
        let videoOutput = AVCaptureVideoDataOutput()
        videoOutput.videoSettings = [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA
        ]
        videoOutput.setSampleBufferDelegate(self, queue: videoQueue)
        videoOutput.alwaysDiscardsLateVideoFrames = false
        
        if captureSession?.canAddOutput(videoOutput) == true {
            captureSession?.addOutput(videoOutput)
        } else {
            throw RecorderError.configuration("Cannot add video output")
        }
        
        if config.includeAudio {
            let audioOutput = AVCaptureAudioDataOutput()
            audioOutput.setSampleBufferDelegate(self, queue: audioQueue)

            if captureSession?.canAddOutput(audioOutput) == true {
                captureSession?.addOutput(audioOutput)
                iosAudioOutput = audioOutput
            }
        }

        try setupMicAndCameraIfNeeded(config: config)
        
        captureSession?.startRunning()
        state = .recording

        if shouldWaitForFirstFrame {
            try await waitForFirstFrame()
        }
    }

    private func waitForFirstFrame() async throws {
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
                firstFrameLock.lock()
                if Task.isCancelled {
                    firstFrameLock.unlock()
                    continuation.resume(throwing: CancellationError())
                    return
                }
                if let error = firstFrameError {
                    firstFrameLock.unlock()
                    continuation.resume(throwing: error)
                    return
                }
                if sessionStarted {
                    firstFrameLock.unlock()
                    continuation.resume()
                    return
                }
                firstFrameContinuation = continuation
                firstFrameLock.unlock()

                let timeoutTask = Task { [weak self] in
                    try? await Task.sleep(nanoseconds: 8_000_000_000)
                    guard !Task.isCancelled else { return }
                    self?.resolveFirstFrameWaiter(
                        .failure(RecorderError.capture("Timed out waiting for iOS device frames"))
                    )
                }

                firstFrameLock.lock()
                if firstFrameContinuation == nil {
                    firstFrameLock.unlock()
                    timeoutTask.cancel()
                    return
                }
                firstFrameTimeoutTask = timeoutTask
                firstFrameLock.unlock()
            }
        } onCancel: { [weak self] in
            self?.resolveFirstFrameWaiter(.failure(CancellationError()))
        }
    }

    private func markFirstFrameReady() {
        firstFrameLock.lock()
        sessionStarted = true
        firstFrameLock.unlock()
        resolveFirstFrameWaiter(.success(()))
    }

    private func resolveFirstFrameWaiter(_ result: Result<Void, Error>) {
        firstFrameLock.lock()
        guard let continuation = firstFrameContinuation else {
            if case .failure(let error) = result {
                firstFrameError = error
            }
            firstFrameLock.unlock()
            return
        }
        firstFrameContinuation = nil
        firstFrameError = nil
        let timeoutTask = firstFrameTimeoutTask
        firstFrameTimeoutTask = nil
        firstFrameLock.unlock()

        timeoutTask?.cancel()
        continuation.resume(with: result)
    }

    private func observeRuntimeErrors(for session: AVCaptureSession) {
        removeRuntimeErrorObserver()
        runtimeErrorObserver = NotificationCenter.default.addObserver(
            forName: AVCaptureSession.runtimeErrorNotification,
            object: session,
            queue: .main
        ) { [weak self] notification in
            let error = notification.userInfo?[AVCaptureSessionErrorKey] as? Error
                ?? RecorderError.capture("iOS device capture failed")
            Task { @MainActor [weak self] in
                guard let self, self.state == .recording || self.state == .paused else { return }
                _ = try? await self.stop()
                self.onError?(error)
            }
        }
    }

    private func removeRuntimeErrorObserver() {
        guard let runtimeErrorObserver else { return }
        NotificationCenter.default.removeObserver(runtimeErrorObserver)
        self.runtimeErrorObserver = nil
    }

    private func rollbackFailedStart() {
        state = .idle
        removeRuntimeErrorObserver()
        if captureSession?.isRunning == true {
            captureSession?.stopRunning()
        }
        captureSession = nil
        stopMicrophoneCapture()
        cameraRecorder.abort()
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
        resetState()
    }

    private func setupMicAndCameraIfNeeded(config: RecordingConfig) throws {
        let audioSettings: [String: Any] = [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: 48000,
            AVNumberOfChannelsKey: 2,
            AVEncoderBitRateKey: 320000,
        ]

        if config.includeAudio {
            let videoDir = (config.outputPath as NSString).deletingLastPathComponent
            let systemPath = (videoDir as NSString).appendingPathComponent("system.m4a")
            systemAudioOutputPath = systemPath

            let fileManager = FileManager.default
            if fileManager.fileExists(atPath: systemPath) {
                try fileManager.removeItem(atPath: systemPath)
            }

            systemAudioAssetWriter = try AVAssetWriter(outputURL: URL(fileURLWithPath: systemPath), fileType: .m4a)
        }

        if config.micEnabled {
            let videoDir = (config.outputPath as NSString).deletingLastPathComponent
            let micPath = (videoDir as NSString).appendingPathComponent("mic.m4a")
            micAudioOutputPath = micPath

            let fileManager = FileManager.default
            if fileManager.fileExists(atPath: micPath) {
                try fileManager.removeItem(atPath: micPath)
            }

            micAudioAssetWriter = try AVAssetWriter(outputURL: URL(fileURLWithPath: micPath), fileType: .m4a)
            micAudioInput = AVAssetWriterInput(mediaType: .audio, outputSettings: audioSettings)
            micAudioInput?.expectsMediaDataInRealTime = true

            if let micAudioInput = micAudioInput, micAudioAssetWriter?.canAdd(micAudioInput) == true {
                micAudioAssetWriter?.add(micAudioInput)
            }

            guard micAudioAssetWriter?.startWriting() == true else {
                let writerError = micAudioAssetWriter?.error?.localizedDescription ?? "unknown error"
                throw RecorderError.capture("Failed to start mic audio writer: \(writerError)")
            }

            try setupMicrophoneCapture(config: config)
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
        }
    }
    
    private func enableScreenCaptureDevices() {
        var property = CMIOObjectPropertyAddress(
            mSelector: CMIOObjectPropertySelector(kCMIOHardwarePropertyAllowScreenCaptureDevices),
            mScope: CMIOObjectPropertyScope(kCMIOObjectPropertyScopeGlobal),
            mElement: CMIOObjectPropertyElement(kCMIOObjectPropertyElementMain)
        )
        
        var allow: UInt32 = 1
        let dataSize = UInt32(MemoryLayout<UInt32>.size)
        
        CMIOObjectSetPropertyData(
            CMIOObjectID(kCMIOObjectSystemObject),
            &property,
            0,
            nil,
            dataSize,
            &allow
        )
    }
    
    private func findIOSDevice(deviceId: String, deviceName: String?) -> AVCaptureDevice? {
        let discoverySession = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.externalUnknown],
            mediaType: .muxed,
            position: .unspecified
        )
        
        if let deviceName = deviceName {
            if let device = discoverySession.devices.first(where: { $0.localizedName == deviceName }) {
                return device
            }
            if let device = discoverySession.devices.first(where: {
                $0.localizedName.lowercased().contains(deviceName.lowercased()) ||
                deviceName.lowercased().contains($0.localizedName.lowercased())
            }) {
                return device
            }
        }
        
        if let device = discoverySession.devices.first(where: { $0.uniqueID == deviceId }) {
            return device
        }
        
        return discoverySession.devices.first { device in
            let name = device.localizedName.lowercased()
            return name.contains("iphone") || name.contains("ipad")
        }
    }

    private func setupMicrophoneCapture(config: RecordingConfig) throws {
        micCaptureSession = AVCaptureSession()

        var micDevice: AVCaptureDevice?

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

        if let deviceName = config.micDeviceName {
            micDevice = discoverySession.devices.first { $0.localizedName == deviceName }

            if micDevice == nil {
                micDevice = discoverySession.devices.first { device in
                    deviceName.contains(device.localizedName)
                        || device.localizedName.contains(deviceName)
                }
            }
        }

        if micDevice == nil, let deviceId = config.micDeviceId {
            micDevice = discoverySession.devices.first { $0.uniqueID == deviceId }
        }

        if micDevice == nil {
            micDevice = AVCaptureDevice.default(for: .audio)
        }

        guard let mic = micDevice else {
            throw RecorderError.configuration("No microphone found")
        }

        let micDeviceInput = try AVCaptureDeviceInput(device: mic)
        if micCaptureSession?.canAddInput(micDeviceInput) == true {
            micCaptureSession?.addInput(micDeviceInput)
        } else {
            throw RecorderError.configuration("Cannot add microphone input to capture session")
        }

        let output = AVCaptureAudioDataOutput()
        output.setSampleBufferDelegate(self, queue: micQueue)

        if micCaptureSession?.canAddOutput(output) == true {
            micCaptureSession?.addOutput(output)
            micAudioOutput = output
        } else {
            throw RecorderError.configuration("Cannot add microphone output to capture session")
        }

        micCaptureSession?.startRunning()
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
    
    private func setupAssetWriter(width: Int, height: Int) throws {
        guard let outputPath = config?.outputPath else {
            throw RecorderError.configuration("Output path not set")
        }
        
        videoWidth = (width / 2) * 2
        videoHeight = (height / 2) * 2
        
        let outputURL = URL(fileURLWithPath: outputPath)
        assetWriter = try AVAssetWriter(outputURL: outputURL, fileType: .mov)
        
        let videoSettings: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: videoWidth,
            AVVideoHeightKey: videoHeight,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: videoWidth * videoHeight * 8,
                AVVideoExpectedSourceFrameRateKey: 60,
                AVVideoMaxKeyFrameIntervalKey: 60,
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel
            ]
        ]
        
        videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: videoSettings)
        videoInput?.expectsMediaDataInRealTime = true
        
        let sourcePixelBufferAttributes: [String: Any] = [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: videoWidth,
            kCVPixelBufferHeightKey as String: videoHeight
        ]
        
        pixelBufferAdaptor = AVAssetWriterInputPixelBufferAdaptor(
            assetWriterInput: videoInput!,
            sourcePixelBufferAttributes: sourcePixelBufferAttributes
        )
        
        if assetWriter?.canAdd(videoInput!) == true {
            assetWriter?.add(videoInput!)
        }
        
        assetWriterConfigured = true
    }
    
    func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        guard state == .recording else { return }

        if output is AVCaptureVideoDataOutput {
            writerQueue.async { [weak self] in
                self?.processVideoSampleBuffer(sampleBuffer)
            }
        } else if let micOutput = micAudioOutput, output === micOutput {
            writerQueue.async { [weak self] in
                self?.processMicSampleBuffer(sampleBuffer)
            }
        } else if let iosOutput = iosAudioOutput, output === iosOutput {
            writerQueue.async { [weak self] in
                self?.processDeviceAudioSampleBuffer(sampleBuffer)
            }
        }
    }
    
    private func processVideoSampleBuffer(_ sampleBuffer: CMSampleBuffer) {
        guard CMSampleBufferDataIsReady(sampleBuffer) else { return }
        guard let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        
        let presentationTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        
        if !assetWriterConfigured {
            let width = CVPixelBufferGetWidth(imageBuffer)
            let height = CVPixelBufferGetHeight(imageBuffer)
            
            do {
                try setupAssetWriter(width: width, height: height)
            } catch {
                resolveFirstFrameWaiter(.failure(error))
                return
            }
        }
        
        if !sessionStarted {
            guard assetWriter?.startWriting() == true else {
                let message = assetWriter?.error?.localizedDescription ?? "unknown error"
                resolveFirstFrameWaiter(
                    .failure(RecorderError.capture("Failed to start video writer: \(message)"))
                )
                return
            }
            assetWriter?.startSession(atSourceTime: presentationTime)
            markFirstFrameReady()
            firstFrameTime = presentationTime
            
            for audioBuffer in pendingAudioBuffers {
                processDeviceAudioSampleBuffer(audioBuffer)
            }
            pendingAudioBuffers.removeAll()
            
            DispatchQueue.main.async { [weak self] in
                self?.onFirstFrame?()
            }

            if micAudioAssetWriter != nil {
                micAudioAssetWriter?.startSession(atSourceTime: .zero)
                micAudioSessionStarted = true
            }

            if cameraEnabled {
                cameraRecorder.syncWithVideoStart()
            }
        }
        
        guard let videoInput = videoInput,
              videoInput.isReadyForMoreMediaData else {
            return
        }
        
        var adjustedTime = presentationTime
        if let firstTime = firstFrameTime {
            adjustedTime = CMTimeSubtract(presentationTime, firstTime)
            adjustedTime = CMTimeSubtract(adjustedTime, totalPauseDuration)
        }
        
        if adjustedTime < lastFrameTime {
            return
        }
        
        pixelBufferAdaptor?.append(imageBuffer, withPresentationTime: adjustedTime)
        lastFrameTime = adjustedTime
        videoFrameCount += 1
        
        let seconds = CMTimeGetSeconds(adjustedTime)
        if seconds > 0 {
            recordingDuration = seconds
        }
    }

    private func processDeviceAudioSampleBuffer(_ sampleBuffer: CMSampleBuffer) {
        guard CMSampleBufferDataIsReady(sampleBuffer) else { return }
        
        if !sessionStarted {
            pendingAudioBuffers.append(sampleBuffer)
            return
        }

        guard let firstTime = firstFrameTime else { return }
        
        if !systemAudioInputConfigured, let writer = systemAudioAssetWriter {
            guard let formatDescription = CMSampleBufferGetFormatDescription(sampleBuffer) else { return }
            
            var sampleRate: Double = 48000
            var channelCount: UInt32 = 2
            
            if let asbd = CMAudioFormatDescriptionGetStreamBasicDescription(formatDescription)?.pointee {
                sampleRate = asbd.mSampleRate
                channelCount = asbd.mChannelsPerFrame
            }
            
            let audioSettings: [String: Any] = [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: sampleRate,
                AVNumberOfChannelsKey: channelCount,
                AVEncoderBitRateKey: 128000
            ]
            
            let audioInput = AVAssetWriterInput(mediaType: .audio, outputSettings: audioSettings, sourceFormatHint: formatDescription)
            audioInput.expectsMediaDataInRealTime = true
            
            if writer.canAdd(audioInput) {
                writer.add(audioInput)
                systemAudioInput = audioInput
                
                if writer.startWriting() {
                    writer.startSession(atSourceTime: .zero)
                    systemAudioSessionStarted = true
                    systemAudioInputConfigured = true
                }
            }
        }
        
        let sampleTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        var adjustedTime = CMTimeSubtract(sampleTime, firstTime)
        adjustedTime = CMTimeSubtract(adjustedTime, totalPauseDuration)

        if CMTimeCompare(adjustedTime, .zero) < 0 {
            adjustedTime = .zero
        }
        
        guard systemAudioSessionStarted,
              let systemAudioInput = systemAudioInput,
              systemAudioInput.isReadyForMoreMediaData,
              let adjustedBuffer = createAdjustedSampleBuffer(sampleBuffer, newTime: adjustedTime)
        else {
            return
        }

        if systemAudioAssetWriter?.status == .writing {
            systemAudioInput.append(adjustedBuffer)
        }
    }

    private func processMicSampleBuffer(_ sampleBuffer: CMSampleBuffer) {
        guard CMSampleBufferDataIsReady(sampleBuffer) else { return }
        guard micAudioSessionStarted else { return }
        guard let micAudioInput = micAudioInput, micAudioInput.isReadyForMoreMediaData else { return }
        guard videoFrameCount > 0 else { return }

        let micTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)

        if firstMicTime == nil {
            firstMicTime = micTime
        }

        guard let firstMic = firstMicTime else { return }

        var adjustedTime = CMTimeSubtract(micTime, firstMic)
        adjustedTime = CMTimeSubtract(adjustedTime, totalPauseDuration)

        if CMTimeCompare(adjustedTime, .zero) < 0 {
            adjustedTime = .zero
        }

        let maxTime = CMTimeAdd(lastFrameTime, CMTimeMake(value: 1, timescale: 10))
        if CMTimeCompare(adjustedTime, maxTime) > 0 {
            adjustedTime = maxTime
        }

        if let adjustedBuffer = createAdjustedSampleBuffer(sampleBuffer, newTime: adjustedTime) {
            if micAudioAssetWriter?.status == .writing {
                micAudioInput.append(adjustedBuffer)
            }
        }
    }
    
    func pause() throws {
        guard state == .recording else {
            throw RecorderError.invalidState("Cannot pause: recorder is \(state.rawValue)")
        }

        pauseStartTime = CMClockGetTime(CMClockGetHostTimeClock())
        state = .paused

        if cameraEnabled {
            cameraRecorder.pause()
        }
    }
    
    func resume() throws {
        guard state == .paused else {
            throw RecorderError.invalidState("Cannot resume: recorder is \(state.rawValue)")
        }
        
        if let pauseStart = pauseStartTime {
            let pauseEnd = CMClockGetTime(CMClockGetHostTimeClock())
            let pauseDuration = CMTimeSubtract(pauseEnd, pauseStart)
            totalPauseDuration = CMTimeAdd(totalPauseDuration, pauseDuration)
        }

        pauseStartTime = nil
        state = .recording

        if cameraEnabled {
            cameraRecorder.resume()
        }
    }
    
    func stop() async throws -> RecordingResult {
        guard state != .idle else {
            throw RecorderError.invalidState("Cannot stop: recorder is idle")
        }

        state = .idle
        resolveFirstFrameWaiter(
            .failure(RecorderError.capture("iOS device capture stopped"))
        )
        defer { resetState() }
        removeRuntimeErrorObserver()
        
        captureSession?.stopRunning()
        captureSession = nil

        stopMicrophoneCapture()

        var cameraFilePath: String? = nil
        if cameraEnabled, let result = cameraRecorder.stop() {
            cameraFilePath = result.videoPath
        }

        waitForPendingSamples()
        
        if let assetWriter = assetWriter, assetWriter.status == .writing {
            videoInput?.markAsFinished()
            await assetWriter.finishWriting()
        }

        if assetWriter?.status == .failed, let error = assetWriter?.error {
            throw RecorderError.capture("Asset writer error: \(error.localizedDescription)")
        }

        if systemAudioInputConfigured {
            systemAudioInput?.markAsFinished()
            if systemAudioAssetWriter?.status == .writing {
                await systemAudioAssetWriter?.finishWriting()
            }

            if systemAudioAssetWriter?.status == .failed, let error = systemAudioAssetWriter?.error {
                throw RecorderError.capture("System audio writer error: \(error.localizedDescription)")
            }
        } else {
            systemAudioAssetWriter?.cancelWriting()
            if let path = systemAudioOutputPath {
                try? FileManager.default.removeItem(atPath: path)
            }
            systemAudioOutputPath = nil
        }

        micAudioInput?.markAsFinished()
        if micAudioAssetWriter?.status == .writing {
            await micAudioAssetWriter?.finishWriting()
        }

        if micAudioAssetWriter?.status == .failed, let error = micAudioAssetWriter?.error {
            throw RecorderError.capture("Mic audio writer error: \(error.localizedDescription)")
        }
        
        let duration = recordingDuration
        let outputPath = config?.outputPath ?? ""
        let finalMicAudioPath = micAudioOutputPath
        let finalSystemAudioPath = systemAudioOutputPath
        
        return RecordingResult(
            outputPath: outputPath,
            cursorPath: nil,
            cameraPath: cameraFilePath,
            keysPath: nil,
            systemAudioPath: finalSystemAudioPath,
            micAudioPath: finalMicAudioPath,
            duration: duration
        )
    }

    private func createAdjustedSampleBuffer(_ originalBuffer: CMSampleBuffer, newTime: CMTime) -> CMSampleBuffer? {
        var timing = CMSampleTimingInfo(
            duration: CMSampleBufferGetDuration(originalBuffer),
            presentationTimeStamp: newTime,
            decodeTimeStamp: .invalid
        )

        var newBuffer: CMSampleBuffer?
        let status = CMSampleBufferCreateCopyWithNewTiming(
            allocator: kCFAllocatorDefault,
            sampleBuffer: originalBuffer,
            sampleTimingEntryCount: 1,
            sampleTimingArray: &timing,
            sampleBufferOut: &newBuffer
        )

        return status == noErr ? newBuffer : nil
    }

    private func waitForPendingSamples() {
        videoQueue.sync {}
        audioQueue.sync {}
        micQueue.sync {}
        writerQueue.sync {}
    }
    
    private func resetState() {
        removeRuntimeErrorObserver()
        state = .idle
        sessionStarted = false
        assetWriterConfigured = false
        firstFrameTime = nil
        lastFrameTime = .zero
        pauseStartTime = nil
        totalPauseDuration = .zero
        recordingDuration = 0
        videoFrameCount = 0
        videoWidth = 0
        videoHeight = 0
        pendingAudioBuffers.removeAll()
        firstFrameError = nil
        firstMicTime = nil
        micAudioSessionStarted = false
        systemAudioSessionStarted = false
        systemAudioInputConfigured = false
        cameraEnabled = false

        assetWriter = nil
        videoInput = nil
        pixelBufferAdaptor = nil
        iosAudioOutput = nil
        micAudioAssetWriter = nil
        micAudioInput = nil
        micAudioOutputPath = nil
        systemAudioAssetWriter = nil
        systemAudioInput = nil
        systemAudioOutputPath = nil
        config = nil
    }
    
    func getStatus() -> (state: RecorderState, duration: Double) {
        return (state: state, duration: recordingDuration)
    }
    
    var isRecording: Bool {
        state == .recording
    }
    
    var isPaused: Bool {
        state == .paused
    }
}
