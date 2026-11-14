import AppKit
import Foundation

@available(macOS 12.3, *)
class ScreenRecorderModule: Module {
    let name = DaemonContract.ScreenRecorder.module
    
    private let screenRecorder = ScreenCaptureRecorder()
    private let iosRecorder = IOSDeviceRecorder()
    @MainActor private var recorderLifecycle = RecorderModuleLifecycle()
    @MainActor private var startTask: Task<Void, Never>?
    @MainActor private var startTaskSessionID: RecorderSessionID?
    @MainActor private let stopCoordinator = RecorderStopCoordinator()

    init() {
        screenRecorder.onError = { [weak self] sessionID, error in
            Task { @MainActor in
                await self?.handleRecorderError(.screen, sessionID: sessionID, error: error)
            }
        }
        iosRecorder.onError = { [weak self] sessionID, error in
            Task { @MainActor in
                await self?.handleRecorderError(.ios, sessionID: sessionID, error: error)
            }
        }
    }
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.ScreenRecorder.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .start:
            handleStart(params: params, requestId: requestId)
        case .pause:
            handlePause(requestId: requestId)
        case .resume:
            handleResume(requestId: requestId)
        case .stop:
            handleStop(requestId: requestId)
        case .status:
            handleStatus(requestId: requestId)
        case .setMicrophone:
            handleSetMicrophone(params: params, requestId: requestId)
        case .setSystemAudio:
            handleSetSystemAudio(params: params, requestId: requestId)
        case .setCamera:
            handleSetCamera(params: params, requestId: requestId)
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
        if let field = invalidStartField(in: params) {
            respondError(
                id: requestId,
                code: "INVALID_PARAMS",
                message: "\(field) has an invalid type"
            )
            return
        }

        let coordinates = [
            params["x"]?.int(),
            params["y"]?.int(),
            params["width"]?.int(),
            params["height"]?.int()
        ]
        let providedCoordinates = coordinates.compactMap { $0 }.count
        guard providedCoordinates == 0 || providedCoordinates == coordinates.count else {
            respondError(
                id: requestId,
                code: "INVALID_PARAMS",
                message: "x, y, width and height must be provided together"
            )
            return
        }
        if let width = coordinates[2], let height = coordinates[3], width <= 0 || height <= 0 {
            respondError(
                id: requestId,
                code: "INVALID_PARAMS",
                message: "width and height must be positive"
            )
            return
        }
        let frameRate = params["frameRate"]?.int() ?? 60
        guard (1...240).contains(frameRate) else {
            respondError(
                id: requestId,
                code: "INVALID_PARAMS",
                message: "frameRate must be between 1 and 240"
            )
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
            excludedWindowIDs: (params["excludedWindowIds"]?.array() ?? [])
                .compactMap { $0 as? Int }
                .map(Int64.init),
            includeAudio: params["includeAudio"]?.bool() ?? true,
            micEnabled: params["micEnabled"]?.bool() ?? false,
            micDeviceId: params["micDeviceId"]?.string(),
            micDeviceName: params["micDeviceName"]?.string(),
            cameraEnabled: params["cameraEnabled"]?.bool() ?? false,
            cameraDeviceId: params["cameraDeviceId"]?.string(),
            cameraDeviceName: params["cameraDeviceName"]?.string(),
            keyboardEnabled: params["keyboardEnabled"]?.bool() ?? false,
            frameRate: frameRate,
            outputPath: outputPath,
            iosDeviceId: iosDeviceId,
            iosDeviceName: iosDeviceName
        )
        
        let backend: RecorderBackend = config.isIOSDeviceRecording ? .ios : .screen
        Task { @MainActor in
            guard let sessionID = self.recorderLifecycle.beginStart(backend) else {
                self.respondError(
                    id: requestId,
                    code: "INVALID_STATE",
                    message: "A recording is already starting, active, or stopping"
                )
                return
            }
            let startTask = Task { @MainActor in
                await self.startRecorder(
                    backend,
                    sessionID: sessionID,
                    config: config,
                    outputPath: outputPath,
                    requestId: requestId
                )
            }
            self.startTaskSessionID = sessionID
            self.startTask = startTask
            await startTask.value
            if self.startTaskSessionID == sessionID {
                self.startTask = nil
                self.startTaskSessionID = nil
            }
        }
    }

    @MainActor
    private func startRecorder(
        _ backend: RecorderBackend,
        sessionID: RecorderSessionID,
        config: RecordingConfig,
        outputPath: String,
        requestId: String
    ) async {
        do {
            switch backend {
            case .ios:
                iosRecorder.configure(config)
                try await iosRecorder.start(waitForFirstFrame: true, sessionID: sessionID)
                guard recorderLifecycle.finishStart(sessionID, success: true) else {
                    respondError(id: requestId, code: "INVALID_STATE", message: "Recording stopped during startup")
                    return
                }
                respond(id: requestId, result: [
                    "success": true,
                    "state": iosRecorder.getStatus().state.rawValue,
                    "message": "iOS device recording started",
                    "outputPath": outputPath
                ])
            case .screen:
                try await screenRecorder.start(config, sessionID: sessionID)
                guard recorderLifecycle.finishStart(sessionID, success: true) else {
                    respondError(id: requestId, code: "INVALID_STATE", message: "Recording stopped during startup")
                    return
                }
                respond(id: requestId, result: [
                    "success": true,
                    "state": screenRecorder.getStatus().state.rawValue,
                    "message": "Recording started",
                    "outputPath": outputPath
                ])
            }
            emit(event: "started", data: ["outputPath": outputPath])
        } catch let error as RecorderError {
            recorderLifecycle.finishStart(sessionID, success: false)
            respondError(id: requestId, code: error.code, message: error.localizedDescription)
        } catch {
            recorderLifecycle.finishStart(sessionID, success: false)
            respondError(id: requestId, code: "START_FAILED", message: error.localizedDescription)
        }
    }

    private func invalidStartField(in params: [String: AnyCodable]) -> String? {
        for field in ["x", "y", "width", "height"] {
            guard let value = params[field], !(value.value is NSNull) else { continue }
            guard let integer = value.value as? Int, Int32(exactly: integer) != nil else {
                return field
            }
        }
        if let value = params["displayId"], !(value.value is NSNull) {
            guard let integer = value.value as? Int, UInt32(exactly: integer) != nil else {
                return "displayId"
            }
        }
        if let value = params["windowId"], !(value.value is NSNull), !(value.value is Int) {
            return "windowId"
        }
        if let value = params["excludedWindowIds"], !(value.value is NSNull) {
            guard let windowIDs = value.array(),
                  windowIDs.allSatisfy({
                      guard let windowID = $0 as? Int else { return false }
                      return CGWindowID(exactly: windowID) != nil
                  }) else {
                return "excludedWindowIds"
            }
        }
        if let value = params["frameRate"], !(value.value is Int) {
            return "frameRate"
        }
        for field in ["includeAudio", "micEnabled", "cameraEnabled", "keyboardEnabled"] {
            if let value = params[field], !(value.value is Bool) {
                return field
            }
        }
        for field in [
            "micDeviceId", "micDeviceName", "cameraDeviceId", "cameraDeviceName",
            "iosDeviceId", "iosDeviceName"
        ] {
            if let value = params[field], !(value.value is NSNull), !(value.value is String) {
                return field
            }
        }
        return nil
    }

    private func handlePause(requestId: String) {
        Task { @MainActor in
            guard case let .recording(backend, _) = recorderLifecycle.phase else {
                respondError(id: requestId, code: "INVALID_STATE", message: "No recording is active")
                return
            }
            do {
                let status: (state: RecorderState, duration: Double)
                switch backend {
                case .ios:
                    try iosRecorder.pause()
                    status = iosRecorder.getStatus()
                case .screen:
                    try screenRecorder.pause()
                    status = screenRecorder.getStatus()
                }
                respond(id: requestId, result: [
                    "success": true,
                    "state": status.state.rawValue,
                    "message": "Recording paused",
                    "duration": status.duration
                ])
                emit(event: "paused", data: ["duration": status.duration])
            } catch let error as RecorderError {
                respondError(id: requestId, code: error.code, message: error.localizedDescription)
            } catch {
                respondError(id: requestId, code: "PAUSE_FAILED", message: error.localizedDescription)
            }
        }
    }
    
    private func handleResume(requestId: String) {
        Task { @MainActor in
            guard case let .recording(backend, _) = recorderLifecycle.phase else {
                respondError(id: requestId, code: "INVALID_STATE", message: "No recording is active")
                return
            }
            do {
                let status: (state: RecorderState, duration: Double)
                switch backend {
                case .ios:
                    try iosRecorder.resume()
                    status = iosRecorder.getStatus()
                case .screen:
                    try screenRecorder.resume()
                    status = screenRecorder.getStatus()
                }
                respond(id: requestId, result: [
                    "success": true,
                    "state": status.state.rawValue,
                    "message": "Recording resumed",
                    "duration": status.duration
                ])
                emit(event: "resumed", data: ["duration": status.duration])
            } catch let error as RecorderError {
                respondError(id: requestId, code: error.code, message: error.localizedDescription)
            } catch {
                respondError(id: requestId, code: "RESUME_FAILED", message: error.localizedDescription)
            }
        }
    }
    
    private func handleStop(requestId: String) {
        Task { @MainActor in
            guard let sessionID = recorderLifecycle.currentSessionID else {
                respondError(id: requestId, code: "INVALID_STATE", message: "No recording is active")
                return
            }
            do {
                let result = try await stopRecording(sessionID)
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
            } catch let error as RecorderError {
                respondError(id: requestId, code: error.code, message: error.localizedDescription)
            } catch {
                respondError(id: requestId, code: "STOP_FAILED", message: error.localizedDescription)
            }
        }
    }

    @MainActor
    private func stopRecording(_ sessionID: RecorderSessionID) async throws -> RecordingResult {
        guard recorderLifecycle.isCurrent(sessionID) else {
            throw RecorderError.invalidState("Recording session is no longer active")
        }
        return try await stopCoordinator.stop(sessionID: sessionID) {
            guard let backend = self.recorderLifecycle.beginStop(sessionID) else {
                throw RecorderError.invalidState("Recording session is no longer active")
            }
            defer { self.recorderLifecycle.finishStop(sessionID) }
            let result: RecordingResult
            switch backend {
            case .ios:
                result = try await self.iosRecorder.stop()
            case .screen:
                result = try await self.screenRecorder.stop()
            }
            self.emit(event: "stopped", data: [
                "outputPath": result.outputPath,
                "cursorPath": result.cursorPath as Any,
                "cameraPath": result.cameraPath as Any,
                "keysPath": result.keysPath as Any,
                "systemAudioPath": result.systemAudioPath as Any,
                "micAudioPath": result.micAudioPath as Any,
                "duration": result.duration
            ])
            return result
        }
    }

    @MainActor
    private func handleRecorderError(
        _ backend: RecorderBackend,
        sessionID: RecorderSessionID,
        error: Error
    ) async {
        guard recorderLifecycle.isCurrent(sessionID), recorderLifecycle.currentBackend == backend else {
            return
        }
        if startTaskSessionID == sessionID {
            await startTask?.value
        }
        guard recorderLifecycle.isCurrent(sessionID) else { return }
        emit(event: "error", data: [
            "code": "CAPTURE_ERROR",
            "message": error.localizedDescription
        ])
        _ = try? await stopRecording(sessionID)
    }
    
    private func handleStatus(requestId: String) {
        Task { @MainActor in
            let status: (state: RecorderState, duration: Double)
            switch recorderLifecycle.currentBackend {
            case .ios:
                status = iosRecorder.getStatus()
            case .screen, nil:
                status = screenRecorder.getStatus()
            }
            respond(id: requestId, result: [
                "state": status.state.rawValue,
                "duration": status.duration
            ])
        }
    }
    
    @MainActor
    private func guardLiveDeviceChange(requestId: String) -> Bool {
        if recorderLifecycle.currentBackend == .ios {
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
        guard let enabled = params?["enabled"]?.bool() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "enabled is required")
            return
        }
        for field in ["deviceId", "deviceName"] {
            if let value = params?[field], !(value.value is NSNull), !(value.value is String) {
                respondError(id: requestId, code: "INVALID_PARAMS", message: "\(field) has an invalid type")
                return
            }
        }
        let deviceId = params?["deviceId"]?.string()
        let deviceName = params?["deviceName"]?.string()

        Task { @MainActor in
            guard guardLiveDeviceChange(requestId: requestId) else { return }
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
        guard let enabled = params?["enabled"]?.bool() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "enabled is required")
            return
        }

        Task { @MainActor in
            guard guardLiveDeviceChange(requestId: requestId) else { return }
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
        guard let enabled = params?["enabled"]?.bool() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "enabled is required")
            return
        }

        Task { @MainActor in
            guard guardLiveDeviceChange(requestId: requestId) else { return }
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
