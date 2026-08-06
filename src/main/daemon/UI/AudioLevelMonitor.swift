import AVFoundation

class AudioLevelMonitor {
    private var audioEngine: AVAudioEngine?
    private var inputNode: AVAudioInputNode?
    private var deviceId: String?
    
    var onLevelUpdate: ((Float) -> Void)?
    
    private(set) var isRunning = false
    
    func start(deviceId: String?) {
        stop()
        
        self.deviceId = deviceId
        
        audioEngine = AVAudioEngine()
        guard let engine = audioEngine else { return }
        
        inputNode = engine.inputNode
        guard let input = inputNode else { return }
        
        if let deviceId = deviceId {
            setInputDevice(deviceId: deviceId)
        }
        
        let hwFormat = input.inputFormat(forBus: 0)
        guard hwFormat.sampleRate > 0 && hwFormat.channelCount > 0 else { return }
        
        input.installTap(onBus: 0, bufferSize: 1024, format: hwFormat) { [weak self] buffer, _ in
            self?.processAudioBuffer(buffer)
        }
        
        do {
            try engine.start()
            isRunning = true
        } catch {
            print("AudioLevelMonitor: Failed to start engine: \(error)")
            stop()
        }
    }
    
    func stop() {
        isRunning = false
        
        if let input = inputNode {
            input.removeTap(onBus: 0)
        }
        
        audioEngine?.stop()
        audioEngine = nil
        inputNode = nil
        deviceId = nil
        
        DispatchQueue.main.async { [weak self] in
            self?.onLevelUpdate?(0)
        }
    }
    
    func updateDevice(deviceId: String?) {
        guard isRunning else {
            start(deviceId: deviceId)
            return
        }
        
        if self.deviceId == deviceId { return }
        
        start(deviceId: deviceId)
    }
    
    private func setInputDevice(deviceId: String) {
        guard let engine = audioEngine else { return }
        
        let discoverySession = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.builtInMicrophone, .externalUnknown],
            mediaType: .audio,
            position: .unspecified
        )
        
        guard let device = discoverySession.devices.first(where: { $0.uniqueID == deviceId }) else {
            return
        }
        
        var allDevicesAddress = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        
        var devicesSize: UInt32 = 0
        AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &allDevicesAddress, 0, nil, &devicesSize)
        
        let deviceCount = Int(devicesSize) / MemoryLayout<AudioDeviceID>.size
        var deviceIDs = [AudioDeviceID](repeating: 0, count: deviceCount)
        AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &allDevicesAddress, 0, nil, &devicesSize, &deviceIDs)
        
        var targetDeviceID: AudioDeviceID = 0
        
        for audioDeviceID in deviceIDs {
            var uidSize: UInt32 = UInt32(MemoryLayout<CFString>.size)
            var uidAddress = AudioObjectPropertyAddress(
                mSelector: kAudioDevicePropertyDeviceUID,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain
            )
            
            var uid: CFString?
            let uidPointer = withUnsafeMutablePointer(to: &uid) { ptr in
                UnsafeMutableRawPointer(ptr)
            }
            
            if AudioObjectGetPropertyData(audioDeviceID, &uidAddress, 0, nil, &uidSize, uidPointer) == noErr {
                if let uidString = uid as String?, uidString == device.uniqueID {
                    targetDeviceID = audioDeviceID
                    break
                }
            }
        }
        
        if targetDeviceID != 0 {
            let result = AudioUnitSetProperty(
                engine.inputNode.audioUnit!,
                kAudioOutputUnitProperty_CurrentDevice,
                kAudioUnitScope_Global,
                0,
                &targetDeviceID,
                UInt32(MemoryLayout<AudioDeviceID>.size)
            )
            if result != noErr {
                print("AudioLevelMonitor: Failed to set input device: \(result)")
            }
        }
    }
    
    private func processAudioBuffer(_ buffer: AVAudioPCMBuffer) {
        guard let channelData = buffer.floatChannelData else { return }
        
        let channelCount = Int(buffer.format.channelCount)
        let frameLength = Int(buffer.frameLength)
        guard frameLength > 0 && channelCount > 0 else { return }
        
        var sum: Float = 0
        for channel in 0..<channelCount {
            let data = channelData[channel]
            for frame in 0..<frameLength {
                let sample = data[frame]
                sum += sample * sample
            }
        }
        
        let rms = sqrt(sum / Float(frameLength * channelCount))
        let db = 20 * log10(max(rms, 0.0001))
        let normalizedLevel = max(0, min(1, (db + 60) / 60))
        
        DispatchQueue.main.async { [weak self] in
            self?.onLevelUpdate?(normalizedLevel)
        }
    }
    
    deinit {
        stop()
    }
}
