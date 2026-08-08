import Foundation

class MediaDevicesModule: Module {
    let name = "media-devices"

    private let audioLevelMonitor = AudioLevelMonitor()
    private var lastLevelEmit = Date.distantPast

    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        switch method {
        case "list":
            handleList(requestId: requestId)
        case "startMicTest":
            handleStartMicTest(params: params, requestId: requestId)
        case "stopMicTest":
            handleStopMicTest(requestId: requestId)
        default:
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
        }
    }

    private func handleList(requestId: String) {
        let microphones = MediaDeviceDiscovery.microphones().map { ["id": $0.id, "label": $0.label] }
        let cameras = MediaDeviceDiscovery.cameras().map { ["id": $0.id, "label": $0.label] }
        respond(id: requestId, result: ["microphones": microphones, "cameras": cameras])
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
