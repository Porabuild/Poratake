import AppKit
import CoreMedia
import Foundation

@MainActor
final class RecordingCaptureWindowRegistry {
    static let shared = RecordingCaptureWindowRegistry()

    typealias FilterUpdater = @MainActor ([CGWindowID]) async throws -> Void
    private var windowIDs: Set<CGWindowID> = []
    private var registeredWindowIDs: [ObjectIdentifier: CGWindowID] = [:]
    private var filterUpdater: FilterUpdater?
    private var revision: UInt64 = 0
    private var appliedRevision: UInt64?
    private var refreshID: UInt64?
    private var nextRefreshID: UInt64 = 0
    private var refreshWaiters: [CheckedContinuation<Void, Error>] = []
    private var captureStartInProgress = false
    private var captureStartWaiters: [CheckedContinuation<Void, Never>] = []
    #if SCROLL_CAPTURE_TEST
    var testOnRefreshCleared: (@MainActor () async -> Void)?
    #endif

    func register(_ window: NSWindow) -> CGWindowID? {
        let key = ObjectIdentifier(window)
        if let windowID = registeredWindowIDs[key] { return windowID }
        let number = window.windowNumber
        guard number >= 0, let windowID = CGWindowID(exactly: number) else { return nil }
        registeredWindowIDs[key] = windowID
        guard windowIDs.insert(windowID).inserted else { return windowID }
        revision &+= 1
        return windowID
    }

    func unregister(_ window: NSWindow) {
        guard let windowID = registeredWindowIDs.removeValue(forKey: ObjectIdentifier(window)) else { return }
        guard !registeredWindowIDs.values.contains(windowID) else { return }
        unregister(windowID: windowID)
    }

    func register(windowID: CGWindowID) {
        guard windowIDs.insert(windowID).inserted else { return }
        revision &+= 1
    }

    func unregister(windowID: CGWindowID) {
        guard windowIDs.remove(windowID) != nil else { return }
        revision &+= 1
    }

    func currentWindowIDs() -> [CGWindowID] {
        return windowIDs.sorted()
    }

    func setFilterUpdater(_ updater: FilterUpdater?) {
        invalidateRefresh()
        filterUpdater = updater
        revision &+= 1
        appliedRevision = nil
    }

    func beginCaptureStart() {
        invalidateRefresh()
        filterUpdater = nil
        revision &+= 1
        captureStartInProgress = true
        appliedRevision = nil
    }

    func finishCaptureStart() {
        captureStartInProgress = false
        let waiters = captureStartWaiters
        captureStartWaiters.removeAll()
        for waiter in waiters {
            waiter.resume()
        }
    }

    func waitForCaptureStartIfNeeded() async {
        if captureStartInProgress {
            await withCheckedContinuation { continuation in
                if captureStartInProgress {
                    captureStartWaiters.append(continuation)
                    return
                }
                continuation.resume()
            }
        }
    }

    func updateFilterIfNeeded() async throws {
        await waitForCaptureStartIfNeeded()

        while appliedRevision != revision {
            guard let updater = filterUpdater else { return }
            if refreshID == nil {
                let refreshID = nextRefreshID
                nextRefreshID &+= 1
                self.refreshID = refreshID
                let refreshRevision = revision
                let windowIDs = currentWindowIDs()
                Task { @MainActor in
                    let result: Result<Void, Error>
                    do {
                        try await updater(windowIDs)
                        result = .success(())
                    } catch {
                        result = .failure(error)
                    }
                    await self.finishRefresh(
                        id: refreshID,
                        revision: refreshRevision,
                        result: result
                    )
                }
            }

            try await withCheckedThrowingContinuation { continuation in
                refreshWaiters.append(continuation)
            }
        }
    }

    private func finishRefresh(
        id: UInt64,
        revision: UInt64,
        result: Result<Void, Error>
    ) async {
        guard refreshID == id else { return }
        refreshID = nil
        if case .success = result {
            appliedRevision = revision
        }
        #if SCROLL_CAPTURE_TEST
        await testOnRefreshCleared?()
        #endif
        let waiters = refreshWaiters
        refreshWaiters.removeAll()
        for waiter in waiters.reversed() {
            waiter.resume(with: result)
        }
    }

    private func invalidateRefresh() {
        refreshID = nil
        let waiters = refreshWaiters
        refreshWaiters.removeAll()
        for waiter in waiters.reversed() {
            waiter.resume(returning: ())
        }
    }

    #if SCROLL_CAPTURE_TEST
    var testPendingRefreshWaiterCount: Int {
        refreshWaiters.count
    }

    var testPendingCaptureStartWaiterCount: Int {
        captureStartWaiters.count
    }
    #endif
}

struct WindowPresentationGeneration {
    private(set) var value: UInt64 = 0

    mutating func begin() -> UInt64 {
        value &+= 1
        return value
    }

    mutating func cancel() {
        value &+= 1
    }

    func isCurrent(_ generation: UInt64) -> Bool {
        value == generation
    }
}

enum RecordingCaptureFilterError: LocalizedError {
    case unavailableWindows([CGWindowID])
    case unavailableWindowNumber
    case unavailableDisplay(CGDirectDisplayID)

    var errorDescription: String? {
        switch self {
        case let .unavailableWindows(windowIDs):
            return "ScreenCaptureKit could not resolve recording window IDs: \(windowIDs.map { String($0) }.joined(separator: ", "))"
        case .unavailableWindowNumber:
            return "AppKit did not assign a window ID to a recording overlay"
        case let .unavailableDisplay(displayID):
            return "ScreenCaptureKit could not resolve recording display ID: \(displayID)"
        }
    }
}

func recordingExclusionWindowIDs(
    requested: [Int64],
    registered: [CGWindowID],
    selectedWindowID: CGWindowID?
) -> [CGWindowID] {
    guard selectedWindowID == nil else { return [] }
    let requestedIDs = requested.compactMap { CGWindowID(exactly: $0) }
    return Array(Set(requestedIDs + registered)).sorted()
}

func resolvedRecordingExclusionWindowIDs(
    requested: [Int64],
    registered: [CGWindowID],
    selectedWindowID: CGWindowID?,
    availableWindowIDs: Set<CGWindowID>
) throws -> [CGWindowID] {
    let excluded = recordingExclusionWindowIDs(
        requested: requested,
        registered: registered,
        selectedWindowID: selectedWindowID
    )
    let unavailable = excluded.filter { !availableWindowIDs.contains($0) }
    guard unavailable.isEmpty else {
        throw RecordingCaptureFilterError.unavailableWindows(unavailable)
    }
    return excluded
}

enum RecorderState: String {
    case idle
    case recording
    case paused
}

struct RecorderLifecycleGuard {
    private(set) var isStarting = false
    private(set) var isStopping = false

    mutating func beginStart(state: RecorderState) -> Bool {
        guard state == .idle, !isStarting, !isStopping else { return false }
        isStarting = true
        return true
    }

    mutating func finishStart() {
        isStarting = false
    }

    mutating func beginStop(state: RecorderState) -> Bool {
        guard (state == .recording || state == .paused), !isStarting, !isStopping else {
            return false
        }
        isStopping = true
        return true
    }

    mutating func finishStop() {
        isStopping = false
    }
}

enum RecorderBackend: Equatable {
    case screen
    case ios
}

typealias RecorderSessionID = UInt64

enum RecorderStopCoordinatorError: Error {
    case differentSessionIsStopping
}

final class RecorderStopCoordinator {
    typealias Operation = @MainActor () async throws -> RecordingResult

    @MainActor private var sessionID: RecorderSessionID?
    @MainActor private var task: Task<RecordingResult, Error>?
    #if SCROLL_CAPTURE_TEST
    @MainActor private(set) var testWaiterCount = 0
    #endif

    @MainActor
    func stop(sessionID: RecorderSessionID, operation: @escaping Operation) async throws -> RecordingResult {
        if let task {
            guard self.sessionID == sessionID else {
                throw RecorderStopCoordinatorError.differentSessionIsStopping
            }
            #if SCROLL_CAPTURE_TEST
            testWaiterCount += 1
            defer { testWaiterCount -= 1 }
            #endif
            return try await task.value
        }

        let task = Task { @MainActor in
            do {
                let result = try await operation()
                self.clear(sessionID: sessionID)
                return result
            } catch {
                self.clear(sessionID: sessionID)
                throw error
            }
        }
        self.sessionID = sessionID
        self.task = task
        return try await task.value
    }

    @MainActor
    private func clear(sessionID: RecorderSessionID) {
        guard self.sessionID == sessionID else { return }
        self.sessionID = nil
        task = nil
    }
}

struct RecorderModuleLifecycle {
    enum Phase: Equatable {
        case idle
        case starting(RecorderBackend, RecorderSessionID)
        case recording(RecorderBackend, RecorderSessionID)
        case stopping(RecorderBackend, RecorderSessionID)
    }

    private(set) var phase: Phase = .idle
    private var nextSessionID: RecorderSessionID = 0

    mutating func beginStart(_ backend: RecorderBackend) -> RecorderSessionID? {
        guard phase == .idle else { return nil }
        let sessionID = nextSessionID
        nextSessionID &+= 1
        phase = .starting(backend, sessionID)
        return sessionID
    }

    @discardableResult
    mutating func finishStart(_ sessionID: RecorderSessionID, success: Bool) -> Bool {
        guard case let .starting(backend, activeID) = phase, activeID == sessionID else {
            return false
        }
        phase = success ? .recording(backend, sessionID) : .idle
        return true
    }

    mutating func beginStop(_ sessionID: RecorderSessionID) -> RecorderBackend? {
        guard case let .recording(backend, activeID) = phase, activeID == sessionID else { return nil }
        phase = .stopping(backend, sessionID)
        return backend
    }

    mutating func finishStop(_ sessionID: RecorderSessionID) {
        guard case let .stopping(_, activeID) = phase, activeID == sessionID else { return }
        phase = .idle
    }

    var currentBackend: RecorderBackend? {
        switch phase {
        case .starting(let backend, _), .recording(let backend, _), .stopping(let backend, _):
            return backend
        case .idle:
            return nil
        }
    }

    var currentSessionID: RecorderSessionID? {
        switch phase {
        case .starting(_, let sessionID), .recording(_, let sessionID), .stopping(_, let sessionID):
            return sessionID
        case .idle:
            return nil
        }
    }

    func isCurrent(_ sessionID: RecorderSessionID) -> Bool {
        currentSessionID == sessionID
    }

}

/// Places `source` inside `target` at the largest scale that keeps its aspect
/// ratio, centring the letterbox. A recorded window keeps the video size it
/// started with, so every later window size is fitted into that frame.
func fitRect(source: CGSize, target: CGSize) -> CGRect {
    guard source.width > 0, source.height > 0, target.width > 0, target.height > 0 else {
        return .zero
    }

    let scale = min(target.width / source.width, target.height / source.height)
    let size = CGSize(width: source.width * scale, height: source.height * scale)

    return CGRect(
        x: (target.width - size.width) / 2,
        y: (target.height - size.height) / 2,
        width: size.width,
        height: size.height
    )
}

func effectiveFrameRate(configured: Int, maximum: Int?) -> Int {
    let selected = min(max(1, configured), 240)
    return min(selected, max(1, maximum ?? selected))
}

struct RecordingConfig {
    var x: Int?
    var y: Int?
    var width: Int?
    var height: Int?
    var displayID: CGDirectDisplayID?
    var windowID: CGWindowID?
    var excludedWindowIDs: [Int64]
    var includeAudio: Bool
    var micEnabled: Bool
    var micDeviceId: String?
    var micDeviceName: String?
    var cameraEnabled: Bool
    var cameraDeviceId: String?
    var cameraDeviceName: String?
    var keyboardEnabled: Bool
    var frameRate: Int
    var outputPath: String
    var iosDeviceId: String?
    var iosDeviceName: String?
    
    var captureRect: CGRect? {
        guard let x = x, let y = y, let width = width, let height = height else {
            return nil
        }
        return CGRect(x: x, y: y, width: width, height: height)
    }
    
    var isAreaRecording: Bool {
        captureRect != nil
    }
    
    var isIOSDeviceRecording: Bool {
        iosDeviceId != nil
    }
}

struct RecordingResult: Sendable {
    let outputPath: String
    let cursorPath: String?
    let cameraPath: String?
    let keysPath: String?
    let systemAudioPath: String?
    let micAudioPath: String?
    let duration: Double
}

protocol SyncableTracker {
    func syncWithVideoStart()
    func pause()
    func resume()
    func stop() -> String?
}

protocol PausableRecorder {
    var isRecording: Bool { get }
    var isPaused: Bool { get }
    func pause()
    func resume()
}
