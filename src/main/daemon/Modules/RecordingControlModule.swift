import AVFoundation
import AppKit
import CoreMediaIO
import Foundation

class RecordingControlModule: Module {
    let name = DaemonContract.RecordingControl.module
    @MainActor private lazy var tooltip = ProtectedTooltip()

    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.RecordingControl.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .listIosDevices:
            handleListIOSDevices(requestId: requestId)
        case .setTooltip:
            handleSetTooltip(params: params, requestId: requestId)
        }
    }

    private func handleSetTooltip(params: [String: AnyCodable]?, requestId: String) {
        guard let rawOwner = params?["ownerId"]?.int(),
              let owner = UInt32(exactly: rawOwner), owner != 0,
              let rawSequence = params?["sequence"]?.int(),
              let sequence = UInt64(exactly: rawSequence), sequence > 0 else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "Tooltip owner and sequence are required")
            return
        }
        let text: String?
        if let value = params?["text"], !(value.value is NSNull) {
            guard let value = value.string(), !value.isEmpty, value.count <= 4096 else {
                respondError(id: requestId, code: "INVALID_PARAMS", message: "Invalid tooltip text")
                return
            }
            text = value
        } else {
            text = nil
        }
        guard let dark = params?["dark"]?.bool() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "Tooltip appearance is required")
            return
        }
        let ownerFrame = tooltipOwnerFrame(params?["ownerFrame"])
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            Task { @MainActor in
                guard self.tooltip.accept(sequence: sequence, owner: owner, text: text), let text = text else {
                    self.respond(id: requestId, result: ["visible": false])
                    return
                }
                do {
                    let anchor = ownerFrame.map { TooltipOwner(windowID: owner, fallbackFrame: $0) }
                    let visible = try await self.tooltip.show(
                        sequence: sequence,
                        owner: owner,
                        text: text,
                        dark: dark,
                        at: NSEvent.mouseLocation,
                        ownerFrame: anchor?.frame
                    )
                    self.respond(id: requestId, result: ["visible": visible])
                } catch {
                    self.respondError(id: requestId, code: "CAPTURE_ERROR", message: error.localizedDescription)
                }
            }
        }
    }

    private func tooltipOwnerFrame(_ value: AnyCodable?) -> NSRect? {
        guard let frame = value?.dict(),
              let x = tooltipCoordinate(frame["x"]),
              let y = tooltipCoordinate(frame["y"]),
              let width = tooltipCoordinate(frame["width"]),
              let height = tooltipCoordinate(frame["height"]),
              width > 0,
              height > 0 else {
            return nil
        }
        return NSRect(x: x, y: y, width: width, height: height)
    }

    private func tooltipCoordinate(_ value: Any?) -> CGFloat? {
        let coordinate: Double
        if let number = value as? Double {
            coordinate = number
        } else if let number = value as? Int {
            coordinate = Double(number)
        } else {
            return nil
        }
        guard coordinate.isFinite else { return nil }
        return CGFloat(coordinate)
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
