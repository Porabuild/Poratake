import AVFoundation
import CoreMediaIO
import Foundation

class RecordingControlModule: Module {
    let name = DaemonContract.RecordingControl.module

    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.RecordingControl.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .listIosDevices:
            handleListIOSDevices(requestId: requestId)
        }
    }

    private func handleListIOSDevices(requestId: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.enableScreenCaptureDevices()
            let session = AVCaptureDevice.DiscoverySession(
                deviceTypes: [.externalUnknown],
                mediaType: .muxed,
                position: .unspecified
            )
            let devices = session.devices.compactMap { device -> [String: String]? in
                let name = device.localizedName.lowercased()
                guard name.contains("iphone") || name.contains("ipad") else { return nil }
                return ["id": device.uniqueID, "label": device.localizedName]
            }
            self.respond(id: requestId, result: ["devices": devices])
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
}
