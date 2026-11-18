import AVFoundation

enum CameraDeviceLookup {
    static func find(deviceId: String?, deviceName: String?) -> AVCaptureDevice? {
        let types: [AVCaptureDevice.DeviceType]
        if #available(macOS 14.0, *) {
            types = [.builtInWideAngleCamera, .external, .continuityCamera]
        } else {
            types = [.builtInWideAngleCamera, .externalUnknown]
        }
        let devices = AVCaptureDevice.DiscoverySession(
            deviceTypes: types, mediaType: .video, position: .unspecified
        ).devices
        return match(devices, deviceId: deviceId, deviceName: deviceName,
                     id: \.uniqueID, name: \.localizedName) ?? AVCaptureDevice.default(for: .video)
    }

    static func match<Device>(
        _ devices: [Device], deviceId: String?, deviceName: String?,
        id: KeyPath<Device, String>, name: KeyPath<Device, String>
    ) -> Device? {
        if let deviceId, !deviceId.isEmpty,
           let exact = devices.first(where: { $0[keyPath: id] == deviceId }) {
            return exact
        }
        guard let deviceName, !deviceName.isEmpty else { return nil }
        return devices.first(where: { $0[keyPath: name] == deviceName }) ?? devices.first {
            let candidate = $0[keyPath: name]
            return !candidate.isEmpty && (deviceName.contains(candidate) || candidate.contains(deviceName))
        }
    }
}
