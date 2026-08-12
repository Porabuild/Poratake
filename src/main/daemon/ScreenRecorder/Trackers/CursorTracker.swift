import AppKit
import Foundation

class CursorTracker: SyncableTracker {
    private var fileHandle: FileHandle?
    private var cursorFilePath: String?
    private var eventCount: Int = 0
    private var startTime: Date?
    private var lastPosition: NSPoint?
    private var lastNormalizedX: Double = -1
    private var lastNormalizedY: Double = -1
    private var isPaused: Bool = false
    private var pauseStartTime: Date?
    private var totalPausedTime: TimeInterval = 0

    private var bounds: CGRect = .zero
    private var trackedWindowID: CGWindowID?
    private var recordingWidth: Int = 0
    private var recordingHeight: Int = 0

    private var pollingTimer: DispatchSourceTimer?
    private let pollingQueue = DispatchQueue(label: "com.porabuild.poratake.cursor-tracker.polling")

    private var leftMouseDownMonitor: Any?
    private var leftMouseUpMonitor: Any?
    private var rightMouseDownMonitor: Any?
    private var rightMouseUpMonitor: Any?

    private var lastLeftButtonDown: Bool = false
    private var lastRightButtonDown: Bool = false

    private let movementThreshold: Double = 0.001
    private let pollingIntervalMs: Int = 16

    private var isSynced: Bool = false
    private var syncTime: Date?
    private var pendingEvents: [(position: NSPoint, type: String, button: String?, cursor: String?, wallTime: Date)] = []
    private let syncQueue = DispatchQueue(label: "com.porabuild.poratake.cursor-tracker.sync")
    
    private let cursorTypeDetector = CursorTypeDetector()
    private var lastWrittenCursorType: CursorType? = nil

    func start(bounds: CGRect, videoPath: String, windowID: CGWindowID? = nil) {
        self.bounds = bounds
        self.trackedWindowID = windowID
        self.recordingWidth = Int(bounds.width)
        self.recordingHeight = Int(bounds.height)
        self.startTime = Date()
        self.eventCount = 0
        self.lastPosition = nil
        self.lastNormalizedX = -1
        self.lastNormalizedY = -1
        self.isPaused = false
        self.pauseStartTime = nil
        self.totalPausedTime = 0
        self.lastLeftButtonDown = false
        self.lastRightButtonDown = false
        self.isSynced = false
        self.syncTime = nil
        self.pendingEvents = []
        self.lastWrittenCursorType = nil

        let videoDir = (videoPath as NSString).deletingLastPathComponent
        cursorFilePath = (videoDir as NSString).appendingPathComponent("cursor.json")

        guard let path = cursorFilePath else { return }
        FileManager.default.createFile(atPath: path, contents: nil)
        fileHandle = FileHandle(forWritingAtPath: path)

        let header = """
            {
              "recordingArea": {
                "width": \(recordingWidth),
                "height": \(recordingHeight)
              },
              "events": [

            """
        fileHandle?.write(header.data(using: .utf8)!)

        setupClickMonitors()
        startPollingTimer()

        let currentPos = NSEvent.mouseLocation
        recordMove(at: currentPos, force: true)
    }

    func syncWithVideoStart() {
        syncQueue.sync {
            guard !isSynced else { return }

            syncTime = Date()
            isSynced = true

            for event in pendingEvents {
                let timestamp = event.wallTime.timeIntervalSince(syncTime!)
                let adjustedTimestamp = max(0, timestamp)
                let (x, y) = normalizePosition(event.position)
                writeEvent(
                    timestamp: adjustedTimestamp, x: x, y: y, type: event.type, button: event.button, cursor: event.cursor)
            }
            pendingEvents.removeAll()
        }
    }

    func pause() {
        guard !isPaused else { return }
        isPaused = true
        pauseStartTime = Date()
    }

    func resume() {
        guard isPaused, let pauseStart = pauseStartTime else { return }
        totalPausedTime += Date().timeIntervalSince(pauseStart)
        pauseStartTime = nil
        isPaused = false

        let currentPos = NSEvent.mouseLocation
        recordMove(at: currentPos, force: true)
    }

    func stop() -> String? {
        stopPollingTimer()
        removeClickMonitors()

        let effectiveStart = syncTime ?? startTime

        guard let handle = fileHandle,
              let path = cursorFilePath,
              let start = effectiveStart
        else {
            return nil
        }

        var duration = Date().timeIntervalSince(start) - totalPausedTime
        if isPaused, let pauseStart = pauseStartTime {
            duration -= Date().timeIntervalSince(pauseStart)
        }

        let sampleRate = eventCount > 0 && duration > 0 ? Int(Double(eventCount) / duration) : 0

        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let startTimeStr = formatter.string(from: start)

        if eventCount > 0 {
            let currentPos = handle.offsetInFile
            if currentPos >= 2 {
                handle.seek(toFileOffset: currentPos - 2)
                handle.write("\n".data(using: .utf8)!)
            }
        }

        let footer = """
              ],
              "meta": {
                "startTime": "\(startTimeStr)",
                "duration": \(String(format: "%.3f", duration)),
                "sampleRate": \(sampleRate)
              }
            }
            """
        handle.write(footer.data(using: .utf8)!)
        handle.closeFile()

        fileHandle = nil
        cursorFilePath = nil

        return path
    }

    private func startPollingTimer() {
        let timer = DispatchSource.makeTimerSource(queue: pollingQueue)
        timer.schedule(
            deadline: .now(),
            repeating: .milliseconds(pollingIntervalMs)
        )
        timer.setEventHandler { [weak self] in
            self?.pollCursorPosition()
        }
        timer.resume()
        pollingTimer = timer
    }

    private func stopPollingTimer() {
        pollingTimer?.cancel()
        pollingTimer = nil
    }

    private func pollCursorPosition() {
        guard !isPaused else { return }

        DispatchQueue.main.async { [weak self] in
            guard let self = self, !self.isPaused else { return }

            let currentPos = NSEvent.mouseLocation
            let pressedButtons = NSEvent.pressedMouseButtons
            let leftButtonDown = (pressedButtons & (1 << 0)) != 0
            let rightButtonDown = (pressedButtons & (1 << 1)) != 0

            if leftButtonDown != self.lastLeftButtonDown {
                self.lastLeftButtonDown = leftButtonDown
                if leftButtonDown {
                    self.recordClick(at: currentPos, type: "down", button: "left")
                } else {
                    self.recordClick(at: currentPos, type: "up", button: "left")
                }
            }

            if rightButtonDown != self.lastRightButtonDown {
                self.lastRightButtonDown = rightButtonDown
                if rightButtonDown {
                    self.recordClick(at: currentPos, type: "down", button: "right")
                } else {
                    self.recordClick(at: currentPos, type: "up", button: "right")
                }
            }

            self.recordMove(at: currentPos, force: false)
        }
    }

    private func setupClickMonitors() {
        leftMouseDownMonitor = NSEvent.addGlobalMonitorForEvents(matching: .leftMouseDown) { [weak self] _ in
            self?.recordClick(at: NSEvent.mouseLocation, type: "down", button: "left")
        }
        leftMouseUpMonitor = NSEvent.addGlobalMonitorForEvents(matching: .leftMouseUp) { [weak self] _ in
            self?.recordClick(at: NSEvent.mouseLocation, type: "up", button: "left")
        }

        rightMouseDownMonitor = NSEvent.addGlobalMonitorForEvents(matching: .rightMouseDown) { [weak self] _ in
            self?.recordClick(at: NSEvent.mouseLocation, type: "down", button: "right")
        }
        rightMouseUpMonitor = NSEvent.addGlobalMonitorForEvents(matching: .rightMouseUp) { [weak self] _ in
            self?.recordClick(at: NSEvent.mouseLocation, type: "up", button: "right")
        }
    }

    private func removeClickMonitors() {
        if let monitor = leftMouseDownMonitor {
            NSEvent.removeMonitor(monitor)
            leftMouseDownMonitor = nil
        }
        if let monitor = leftMouseUpMonitor {
            NSEvent.removeMonitor(monitor)
            leftMouseUpMonitor = nil
        }
        if let monitor = rightMouseDownMonitor {
            NSEvent.removeMonitor(monitor)
            rightMouseDownMonitor = nil
        }
        if let monitor = rightMouseUpMonitor {
            NSEvent.removeMonitor(monitor)
            rightMouseUpMonitor = nil
        }
    }

    private func getCurrentTimestamp() -> Double {
        guard let start = syncTime ?? startTime else { return 0 }

        if isPaused, let pauseStart = pauseStartTime {
            return pauseStart.timeIntervalSince(start) - totalPausedTime
        }
        return Date().timeIntervalSince(start) - totalPausedTime
    }

    private func normalizePosition(_ screenPos: NSPoint) -> (x: Double, y: Double) {
        let mainScreenHeight = NSScreen.screens.first?.frame.height ?? bounds.height
        let topLeftY = mainScreenHeight - screenPos.y
        let area = trackedArea()

        let relX = max(0, min(1, (screenPos.x - area.origin.x) / area.width))
        let relY = max(0, min(1, (topLeftY - area.origin.y) / area.height))

        guard trackedWindowID != nil else {
            return (relX, relY)
        }

        // The video keeps the size the window started with, so a resized window
        // lands inside the same letterbox its frames are scaled into.
        let fit = fitRect(source: area.size, target: bounds.size)
        guard fit.width > 0, fit.height > 0 else {
            return (relX, relY)
        }

        return (
            (fit.origin.x + relX * fit.width) / bounds.width,
            (fit.origin.y + relY * fit.height) / bounds.height
        )
    }

    private func trackedArea() -> CGRect {
        guard let windowID = trackedWindowID,
              let windows = CGWindowListCopyWindowInfo(.optionIncludingWindow, windowID)
                as? [[String: Any]],
              let boundsDict = windows.first?[kCGWindowBounds as String] as? [String: Any],
              let rect = CGRect(dictionaryRepresentation: boundsDict as CFDictionary),
              rect.width > 0, rect.height > 0
        else {
            return bounds
        }

        return rect
    }

    private func recordMove(at screenPos: NSPoint, force: Bool = false) {
        guard !isPaused else { return }

        let (x, y) = normalizePosition(screenPos)

        if !force {
            if abs(x - lastNormalizedX) < movementThreshold
                && abs(y - lastNormalizedY) < movementThreshold
            {
                return
            }
        }

        lastPosition = screenPos
        lastNormalizedX = x
        lastNormalizedY = y
        
        let cursorChanged = cursorTypeDetector.checkForChange()
        let shouldIncludeCursor = cursorChanged || lastWrittenCursorType == nil
        let cursorType: String? = shouldIncludeCursor ? cursorTypeDetector.currentType.rawValue : nil

        syncQueue.sync {
            if isSynced {
                writeEvent(timestamp: getCurrentTimestamp(), x: x, y: y, type: "move", button: nil, cursor: cursorType)
            } else {
                pendingEvents.append((position: screenPos, type: "move", button: nil, cursor: cursorType, wallTime: Date()))
            }
        }
    }

    private func recordClick(at screenPos: NSPoint, type: String, button: String) {
        guard !isPaused else { return }

        let (x, y) = normalizePosition(screenPos)
        lastPosition = screenPos
        lastNormalizedX = x
        lastNormalizedY = y
        
        let cursorChanged = cursorTypeDetector.checkForChange()
        let cursorType: String? = cursorChanged ? cursorTypeDetector.currentType.rawValue : nil

        syncQueue.sync {
            if isSynced {
                writeEvent(timestamp: getCurrentTimestamp(), x: x, y: y, type: type, button: button, cursor: cursorType)
            } else {
                pendingEvents.append((position: screenPos, type: type, button: button, cursor: cursorType, wallTime: Date()))
            }
        }
    }

    private func writeEvent(timestamp: Double, x: Double, y: Double, type: String, button: String?, cursor: String?) {
        guard let handle = fileHandle else { return }

        var fields = "\"timestamp\":\(String(format: "%.3f", timestamp)),\"x\":\(String(format: "%.4f", x)),\"y\":\(String(format: "%.4f", y)),\"type\":\"\(type)\""
        
        if let btn = button {
            fields += ",\"button\":\"\(btn)\""
        }
        
        if let cur = cursor {
            fields += ",\"cursor\":\"\(cur)\""
            lastWrittenCursorType = CursorType(rawValue: cur)
        }
        
        let eventJson = "    {\(fields)},\n"

        handle.write(eventJson.data(using: .utf8)!)
        eventCount += 1
    }
}
