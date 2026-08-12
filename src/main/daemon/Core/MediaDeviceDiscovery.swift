import AVFoundation

struct MediaDevice {
    let id: String
    let label: String
}

enum MediaDeviceDiscovery {
    static func microphones() -> [MediaDevice] {
        let session = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.builtInMicrophone, .externalUnknown],
            mediaType: .audio,
            position: .unspecified
        )
        return session.devices.map { MediaDevice(id: $0.uniqueID, label: $0.localizedName) }
    }

    static func cameras() -> [MediaDevice] {
        let session: AVCaptureDevice.DiscoverySession
        if #available(macOS 14.0, *) {
            session = AVCaptureDevice.DiscoverySession(
                deviceTypes: [.builtInWideAngleCamera, .external, .continuityCamera],
                mediaType: .video,
                position: .unspecified
            )
        } else {
            session = AVCaptureDevice.DiscoverySession(
                deviceTypes: [.builtInWideAngleCamera, .externalUnknown],
                mediaType: .video,
                position: .unspecified
            )
        }
        return session.devices.map { MediaDevice(id: $0.uniqueID, label: $0.localizedName) }
    }

    static func defaultMicrophoneId() -> String? {
        AVCaptureDevice.default(for: .audio)?.uniqueID
    }

    static func defaultCameraId() -> String? {
        AVCaptureDevice.default(for: .video)?.uniqueID
    }
}
