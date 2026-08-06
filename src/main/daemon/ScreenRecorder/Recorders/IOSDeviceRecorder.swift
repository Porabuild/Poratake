import AVFoundation
import CoreMedia
import CoreMediaIO
import Foundation

class IOSDeviceRecorder: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate, AVCaptureAudioDataOutputSampleBufferDelegate {
    private var captureSession: AVCaptureSession?
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
    private var micMuted = false
    private let micQueue = DispatchQueue(label: "com.capty.ios-recorder.mic")

    private let cameraRecorder = CameraRecorder()
    private var cameraEnabled = false
    
    private var sessionStarted = false
    private var assetWriterConfigured = false
    private var firstFrameTime: CMTime?
    private var lastFrameTime: CMTime = .zero
    private var pauseStartTime: CMTime?
    private var totalPauseDuration: CMTime = .zero
    
    private let videoQueue = DispatchQueue(label: "com.capty.ios-recorder.video")
    private let audioQueue = DispatchQueue(label: "com.capty.ios-recorder.audio")
    private let writerQueue = DispatchQueue(label: "com.capty.ios-recorder.writer")
    
    private(set) var state: RecorderState = .idle
    private var config: RecordingConfig?
    private var recordingDuration: Double = 0
    private var videoWidth: Int = 0
    private var videoHeight: Int = 0
    
    private var videoFrameCount: Int = 0
    private var pendingAudioBuffers: [CMSampleBuffer] = []
    
    var onFirstFrame: (() -> Void)?
    
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
        
        enableScreenCaptureDevices()
        
        let fileManager = FileManager.default
        if fileManager.fileExists(atPath: config.outputPath) {
            try fileManager.removeItem(atPath: config.outputPath)
        }
        
        guard let device = findIOSDevice(deviceId: deviceId, deviceName: config.iosDeviceName) else {
            throw RecorderError.configuration("iOS device not found: \(config.iosDeviceName ?? deviceId)")
        }
        
        captureSession = AVCaptureSession()
        
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

        if waitForFirstFrame {
            let timeoutNs: UInt64 = 8_000_000_000
            let pollIntervalNs: UInt64 = 50_000_000
            var waitedNs: UInt64 = 0

            while !sessionStarted && waitedNs < timeoutNs {
                try await Task.sleep(nanoseconds: pollIntervalNs)
                waitedNs += pollIntervalNs
            }

            if !sessionStarted {
                throw RecorderError.capture("Timed out waiting for iOS device frames")
            }
        }
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
                print("Failed to setup asset writer: \(error)")
                return
            }
        }
        
        if !sessionStarted {
            assetWriter?.startWriting()
            assetWriter?.startSession(atSourceTime: presentationTime)
            sessionStarted = true
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

        let sourceBuffer: CMSampleBuffer
        if micMuted, let mutedBuffer = createMutedSampleBuffer(from: sampleBuffer) {
            sourceBuffer = mutedBuffer
        } else {
            sourceBuffer = sampleBuffer
        }

        let micTime = CMSampleBufferGetPresentationTimeStamp(sourceBuffer)

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

        if let adjustedBuffer = createAdjustedSampleBuffer(sourceBuffer, newTime: adjustedTime) {
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
        
        captureSession?.stopRunning()
        captureSession = nil

        stopMicrophoneCapture()

        var cameraFilePath: String? = nil
        if cameraEnabled, let result = cameraRecorder.stop() {
            cameraFilePath = result.videoPath
        }
        
        if let assetWriter = assetWriter, assetWriter.status == .writing {
            videoInput?.markAsFinished()
            await assetWriter.finishWriting()
        }

        if systemAudioInputConfigured {
            systemAudioInput?.markAsFinished()
            if systemAudioAssetWriter?.status == .writing {
                await systemAudioAssetWriter?.finishWriting()
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
        
        resetState()
        
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

    func setMicMuted(_ muted: Bool) {
        micMuted = muted
    }
    
    private func resetState() {
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
        firstMicTime = nil
        micAudioSessionStarted = false
        systemAudioSessionStarted = false
        systemAudioInputConfigured = false
        micMuted = false
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
