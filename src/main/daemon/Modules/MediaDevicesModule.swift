import AVFoundation
import Foundation

class MediaDevicesModule: Module {
    let name = "media-devices"

    private let audioLevelMonitor = AudioLevelMonitor()
    private var lastLevelEmit = Date.distantPast

    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        switch method {
        case "list":
            handleList(params: params, requestId: requestId)
        case "startMicTest":
            handleStartMicTest(params: params, requestId: requestId)
        case "stopMicTest":
            handleStopMicTest(requestId: requestId)
        default:
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
        }
    }

    private func parseKinds(_ params: [String: AnyCodable]?) -> (microphones: Bool, cameras: Bool) {
        guard let values = params?["kinds"]?.array() as? [String], !values.isEmpty else {
            return (true, true)
        }
        return (values.contains("microphone"), values.contains("camera"))
    }

    private func handleList(params: [String: AnyCodable]?, requestId: String) {
        let kinds = parseKinds(params)
        var mediaTypes: [AVMediaType] = []
        if kinds.microphones { mediaTypes.append(.audio) }
        if kinds.cameras { mediaTypes.append(.video) }
        guard !mediaTypes.isEmpty else {
            respondList(requestId: requestId, microphones: false, cameras: false)
            return
        }

        MediaDeviceDiscovery.requestAccess(kinds: mediaTypes) { [weak self] in
            self?.respondList(requestId: requestId, microphones: kinds.microphones, cameras: kinds.cameras)
        }
    }

    private func respondList(requestId: String, microphones: Bool, cameras: Bool) {
        var result: [String: Any] = [:]

        if microphones {
            result["microphones"] = MediaDeviceDiscovery.microphones().map { ["id": $0.id, "label": $0.label] }
            if let defaultMicrophoneId = MediaDeviceDiscovery.defaultMicrophoneId() {
                result["defaultMicrophoneId"] = defaultMicrophoneId
            }
        }
        if cameras {
            result["cameras"] = MediaDeviceDiscovery.cameras().map { ["id": $0.id, "label": $0.label] }
            if let defaultCameraId = MediaDeviceDiscovery.defaultCameraId() {
                result["defaultCameraId"] = defaultCameraId
            }
        }

        respond(id: requestId, result: result)
    }

    private func handleStartMicTest(params: [String: AnyCodable]?, requestId: String) {
        let deviceId = params?["deviceId"]?.string()

        audioLevelMonitor.onLevelUpdate = { [weak self] level in
            self?.emitLevel(level)
        }
        audioLevelMonitor.start(deviceId: deviceId)

        guard audioLevelMonitor.isRunning else {
            respondError(id: requestId, code: "MIC_TEST_ERROR", message: "Failed to start microphone monitoring")
            return
        }
        respond(id: requestId, result: ["running": true])
    }

    private func handleStopMicTest(requestId: String) {
        audioLevelMonitor.stop()
        respond(id: requestId, result: ["running": false])
    }

    private func emitLevel(_ level: Float) {
        let now = Date()
        if level > 0 && now.timeIntervalSince(lastLevelEmit) < 0.05 { return }
        lastLevelEmit = now
        emit(event: "mic-level", data: ["level": level])
    }
}
