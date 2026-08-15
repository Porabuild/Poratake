import AppKit
import Foundation

@available(macOS 12.3, *)
class ScreenRecorderModule: Module {
    let name = "screen-recorder"
    
    private let screenRecorder = ScreenCaptureRecorder()
    private let iosRecorder = IOSDeviceRecorder()
    private var isIOSRecording = false

    init() {
        screenRecorder.onError = { [weak self] error in
            self?.emit(event: "error", data: [
                "code": "CAPTURE_ERROR",
                "message": error.localizedDescription
            ])
        }
        iosRecorder.onError = { [weak self] error in
            self?.emit(event: "error", data: [
                "code": "CAPTURE_ERROR",
                "message": error.localizedDescription
            ])
        }
    }
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        switch method {
        case "start":
            handleStart(params: params, requestId: requestId)
        case "pause":
            handlePause(requestId: requestId)
        case "resume":
            handleResume(requestId: requestId)
        case "stop":
            handleStop(requestId: requestId)
        case "status":
            handleStatus(requestId: requestId)
        case "setMicMuted":
            handleSetMicMuted(params: params, requestId: requestId)
        case "setMicrophone":
            handleSetMicrophone(params: params, requestId: requestId)
        case "setSystemAudio":
            handleSetSystemAudio(params: params, requestId: requestId)
        case "setCamera":
            handleSetCamera(params: params, requestId: requestId)
        default:
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
        }
    }
    
    private func handleStart(params: [String: AnyCodable]?, requestId: String) {
        guard let params = params else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "start requires params")
            return
        }
        
        guard let outputPath = params["outputPath"]?.string() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "outputPath is required")
            return
        }
        
        let iosDeviceId = params["iosDeviceId"]?.string()
        let iosDeviceName = params["iosDeviceName"]?.string()
        
        let config = RecordingConfig(
            x: params["x"]?.int(),
            y: params["y"]?.int(),
            width: params["width"]?.int(),
            height: params["height"]?.int(),
            displayID: params["displayId"]?.int().map { CGDirectDisplayID($0) },
            windowID: params["windowId"]?.int().map { CGWindowID($0) },
            includeAudio: params["includeAudio"]?.bool() ?? true,
            micEnabled: params["micEnabled"]?.bool() ?? false,
            micDeviceId: params["micDeviceId"]?.string(),
            micDeviceName: params["micDeviceName"]?.string(),
            cameraEnabled: params["cameraEnabled"]?.bool() ?? false,
            cameraDeviceId: params["cameraDeviceId"]?.string(),
            cameraDeviceName: params["cameraDeviceName"]?.string(),
            keyboardEnabled: params["keyboardEnabled"]?.bool() ?? false,
            frameRate: params["frameRate"]?.int() ?? 60,
            outputPath: outputPath,
            iosDeviceId: iosDeviceId,
            iosDeviceName: iosDeviceName
        )
        
        isIOSRecording = config.isIOSDeviceRecording
        
        if isIOSRecording {
            iosRecorder.configure(config)
        } else {
            screenRecorder.configure(config)
        }
        
        Task { @MainActor in
            do {
                if isIOSRecording {
                    try await iosRecorder.start(waitForFirstFrame: true)
                    respond(id: requestId, result: [
                        "success": true,
                        "state": iosRecorder.getStatus().state.rawValue,
                        "message": "iOS device recording started",
                        "outputPath": outputPath
                    ])
                } else {
                    try await screenRecorder.start()
                    respond(id: requestId, result: [
                        "success": true,
                        "state": screenRecorder.getStatus().state.rawValue,
                        "message": "Recording started",
                        "outputPath": outputPath
                    ])
                }
                emit(event: "started", data: ["outputPath": outputPath])
            } catch let error as RecorderError {
                respondError(id: requestId, code: error.code, message: error.localizedDescription)
            } catch {
                respondError(id: requestId, code: "START_FAILED", message: error.localizedDescription)
            }
        }
    }
    
    private func handlePause(requestId: String) {
        do {
            if isIOSRecording {
                try iosRecorder.pause()
                let status = iosRecorder.getStatus()
                respond(id: requestId, result: [
                    "success": true,
                    "state": status.state.rawValue,
                    "message": "Recording paused",
                    "duration": status.duration
                ])
                emit(event: "paused", data: ["duration": status.duration])
            } else {
                try screenRecorder.pause()
                let status = screenRecorder.getStatus()
                respond(id: requestId, result: [
                    "success": true,
                    "state": status.state.rawValue,
                    "message": "Recording paused",
                    "duration": status.duration
                ])
                emit(event: "paused", data: ["duration": status.duration])
            }
        } catch let error as RecorderError {
            respondError(id: requestId, code: error.code, message: error.localizedDescription)
        } catch {
            respondError(id: requestId, code: "PAUSE_FAILED", message: error.localizedDescription)
        }
    }
    
    private func handleResume(requestId: String) {
        do {
            if isIOSRecording {
                try iosRecorder.resume()
                let status = iosRecorder.getStatus()
                respond(id: requestId, result: [
                    "success": true,
                    "state": status.state.rawValue,
                    "message": "Recording resumed",
                    "duration": status.duration
                ])
                emit(event: "resumed", data: ["duration": status.duration])
            } else {
                try screenRecorder.resume()
                let status = screenRecorder.getStatus()
                respond(id: requestId, result: [
                    "success": true,
                    "state": status.state.rawValue,
                    "message": "Recording resumed",
                    "duration": status.duration
                ])
                emit(event: "resumed", data: ["duration": status.duration])
            }
        } catch let error as RecorderError {
            respondError(id: requestId, code: error.code, message: error.localizedDescription)
        } catch {
            respondError(id: requestId, code: "RESUME_FAILED", message: error.localizedDescription)
        }
    }
    
    private func handleStop(requestId: String) {
        Task { @MainActor in
            do {
                let result: RecordingResult
                if isIOSRecording {
                    result = try await iosRecorder.stop()
                    isIOSRecording = false
                } else {
                    result = try await screenRecorder.stop()
                }
                respond(id: requestId, result: [
                    "success": true,
                    "state": "idle",
                    "message": "Recording stopped",
                    "outputPath": result.outputPath,
                    "cursorPath": result.cursorPath as Any,
                    "cameraPath": result.cameraPath as Any,
                    "keysPath": result.keysPath as Any,
                    "systemAudioPath": result.systemAudioPath as Any,
                    "micAudioPath": result.micAudioPath as Any,
                    "duration": result.duration
                ])
                emit(event: "stopped", data: [
                    "outputPath": result.outputPath,
                    "cursorPath": result.cursorPath as Any,
                    "cameraPath": result.cameraPath as Any,
                    "keysPath": result.keysPath as Any,
                    "systemAudioPath": result.systemAudioPath as Any,
                    "micAudioPath": result.micAudioPath as Any,
                    "duration": result.duration
                ])
            } catch let error as RecorderError {
                respondError(id: requestId, code: error.code, message: error.localizedDescription)
            } catch {
                respondError(id: requestId, code: "STOP_FAILED", message: error.localizedDescription)
            }
        }
    }
    
    private func handleStatus(requestId: String) {
        let status = isIOSRecording ? iosRecorder.getStatus() : screenRecorder.getStatus()
        respond(id: requestId, result: [
            "state": status.state.rawValue,
            "duration": status.duration
        ])
    }
    
    private func handleSetMicMuted(params: [String: AnyCodable]?, requestId: String) {
        let muted = params?["muted"]?.bool() ?? false
        if isIOSRecording {
            iosRecorder.setMicMuted(muted)
        } else {
            screenRecorder.setMicMuted(muted)
        }
        respond(id: requestId, result: [
            "success": true,
            "muted": muted
        ])
    }

    private func guardLiveDeviceChange(requestId: String) -> Bool {
        if isIOSRecording {
            respondError(
                id: requestId,
                code: "UNSUPPORTED",
                message: "Device changes are not supported while recording an iOS device"
            )
            return false
        }
        return true
    }

    private func handleSetMicrophone(params: [String: AnyCodable]?, requestId: String) {
        guard guardLiveDeviceChange(requestId: requestId) else { return }
        let enabled = params?["enabled"]?.bool() ?? false
        let deviceId = params?["deviceId"]?.string()
        let deviceName = params?["deviceName"]?.string()

        Task { @MainActor in
            do {
                try screenRecorder.setMicrophone(
                    enabled: enabled,
                    deviceId: deviceId,
                    deviceName: deviceName
                )
                respond(id: requestId, result: [
                    "success": true,
                    "enabled": enabled
                ])
            } catch let error as RecorderError {
                respondError(id: requestId, code: error.code, message: error.localizedDescription)
            } catch {
                respondError(id: requestId, code: "SET_MICROPHONE_FAILED", message: error.localizedDescription)
            }
        }
    }

    private func handleSetSystemAudio(params: [String: AnyCodable]?, requestId: String) {
        guard guardLiveDeviceChange(requestId: requestId) else { return }
        let enabled = params?["enabled"]?.bool() ?? false

        Task { @MainActor in
            do {
                try screenRecorder.setSystemAudio(enabled: enabled)
                respond(id: requestId, result: [
                    "success": true,
                    "enabled": enabled
                ])
            } catch let error as RecorderError {
                respondError(id: requestId, code: error.code, message: error.localizedDescription)
            } catch {
                respondError(id: requestId, code: "SET_SYSTEM_AUDIO_FAILED", message: error.localizedDescription)
            }
        }
    }

    private func handleSetCamera(params: [String: AnyCodable]?, requestId: String) {
        guard guardLiveDeviceChange(requestId: requestId) else { return }
        let enabled = params?["enabled"]?.bool() ?? false

        Task { @MainActor in
            do {
                try screenRecorder.setCamera(enabled: enabled)
                respond(id: requestId, result: [
                    "success": true,
                    "enabled": enabled
                ])
            } catch let error as RecorderError {
                respondError(id: requestId, code: error.code, message: error.localizedDescription)
            } catch {
                respondError(id: requestId, code: "SET_CAMERA_FAILED", message: error.localizedDescription)
            }
        }
    }
}
