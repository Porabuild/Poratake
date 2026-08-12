import AppKit
import Foundation

class KeyboardTracker: SyncableTracker {
    private var fileHandle: FileHandle?
    private var keysFilePath: String?
    private var eventCount: Int = 0
    private var startTime: Date?
    private var isPaused: Bool = false
    private var pauseStartTime: Date?
    private var totalPausedTime: TimeInterval = 0
    
    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var isActive: Bool = false
    
    private var isSynced: Bool = false
    private var syncTime: Date?
    private var pendingEvents: [(key: String, keyCode: Int, modifiers: [String], type: String, wallTime: Date)] = []
    private let syncQueue = DispatchQueue(label: "com.porabuild.poratake.keyboard-tracker.sync")
    
    func start(videoPath: String) {
        self.startTime = Date()
        self.eventCount = 0
        self.isPaused = false
        self.pauseStartTime = nil
        self.totalPausedTime = 0
        self.isSynced = false
        self.syncTime = nil
        self.pendingEvents = []
        
        let videoDir = (videoPath as NSString).deletingLastPathComponent
        keysFilePath = (videoDir as NSString).appendingPathComponent("keys.json")
        
        guard let path = keysFilePath else { return }
        FileManager.default.createFile(atPath: path, contents: nil)
        fileHandle = FileHandle(forWritingAtPath: path)
        
        let header = """
            {
              "events": [
            
            """
        fileHandle?.write(header.data(using: .utf8)!)
        
        setupEventTap()
    }
    
    private func setupEventTap() {
        let eventMask: CGEventMask = (1 << CGEventType.keyDown.rawValue) |
                                     (1 << CGEventType.keyUp.rawValue) |
                                     (1 << CGEventType.flagsChanged.rawValue)
        
        let callback: CGEventTapCallBack = { (proxy, type, event, refcon) -> Unmanaged<CGEvent>? in
            guard let refcon = refcon else {
                return Unmanaged.passRetained(event)
            }
            
            let tracker = Unmanaged<KeyboardTracker>.fromOpaque(refcon).takeUnretainedValue()
            tracker.handleKeyEvent(event, type: type)
            
            return Unmanaged.passRetained(event)
        }
        
        eventTap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .listenOnly,
            eventsOfInterest: eventMask,
            callback: callback,
            userInfo: Unmanaged.passUnretained(self).toOpaque()
        )
        
        guard let tap = eventTap else {
            return
        }
        
        runLoopSource = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        
        guard let source = runLoopSource else {
            return
        }
        
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        isActive = true
    }
    
    private func handleKeyEvent(_ event: CGEvent, type: CGEventType) {
        guard isActive, !isPaused else { return }
        
        let keyCode = Int(event.getIntegerValueField(.keyboardEventKeycode))
        let flags = event.flags
        
        var modifiers: [String] = []
        if flags.contains(.maskCommand) { modifiers.append("command") }
        if flags.contains(.maskControl) { modifiers.append("control") }
        if flags.contains(.maskAlternate) { modifiers.append("option") }
        if flags.contains(.maskShift) { modifiers.append("shift") }
        if flags.contains(.maskSecondaryFn) { modifiers.append("fn") }
        
        var eventType: String
        var keyName: String
        
        switch type {
        case .keyDown:
            eventType = "down"
            keyName = keyCodeToKeyName(keyCode)
            
        case .keyUp:
            eventType = "up"
            keyName = keyCodeToKeyName(keyCode)
            
        case .flagsChanged:
            return
            
        default:
            return
        }
        
        recordEvent(key: keyName, keyCode: keyCode, modifiers: modifiers, type: eventType)
    }
    
    private func keyCodeToKeyName(_ keyCode: Int) -> String {
        switch keyCode {
        case 0: return "A"
        case 1: return "S"
        case 2: return "D"
        case 3: return "F"
        case 4: return "H"
        case 5: return "G"
        case 6: return "Z"
        case 7: return "X"
        case 8: return "C"
        case 9: return "V"
        case 11: return "B"
        case 12: return "Q"
        case 13: return "W"
        case 14: return "E"
        case 15: return "R"
        case 16: return "Y"
        case 17: return "T"
        case 18: return "1"
        case 19: return "2"
        case 20: return "3"
        case 21: return "4"
        case 22: return "6"
        case 23: return "5"
        case 24: return "="
        case 25: return "9"
        case 26: return "7"
        case 27: return "-"
        case 28: return "8"
        case 29: return "0"
        case 30: return "]"
        case 31: return "O"
        case 32: return "U"
        case 33: return "["
        case 34: return "I"
        case 35: return "P"
        case 36: return "Return"
        case 37: return "L"
        case 38: return "J"
        case 39: return "'"
        case 40: return "K"
        case 41: return ";"
        case 42: return "\\"
        case 43: return ","
        case 44: return "/"
        case 45: return "N"
        case 46: return "M"
        case 47: return "."
        case 48: return "Tab"
        case 49: return "Space"
        case 50: return "`"
        case 51: return "Delete"
        case 53: return "Escape"
        case 55: return "Command"
        case 56: return "Shift"
        case 57: return "CapsLock"
        case 58: return "Option"
        case 59: return "Control"
        case 60: return "RightShift"
        case 61: return "RightOption"
        case 62: return "RightControl"
        case 63: return "Fn"
        case 64: return "F17"
        case 65: return "KeypadDecimal"
        case 67: return "KeypadMultiply"
        case 69: return "KeypadPlus"
        case 71: return "KeypadClear"
        case 75: return "KeypadDivide"
        case 76: return "KeypadEnter"
        case 78: return "KeypadMinus"
        case 79: return "F18"
        case 80: return "F19"
        case 81: return "KeypadEquals"
        case 82: return "Keypad0"
        case 83: return "Keypad1"
        case 84: return "Keypad2"
        case 85: return "Keypad3"
        case 86: return "Keypad4"
        case 87: return "Keypad5"
        case 88: return "Keypad6"
        case 89: return "Keypad7"
        case 90: return "F20"
        case 91: return "Keypad8"
        case 92: return "Keypad9"
        case 96: return "F5"
        case 97: return "F6"
        case 98: return "F7"
        case 99: return "F3"
        case 100: return "F8"
        case 101: return "F9"
        case 103: return "F11"
        case 105: return "F13"
        case 106: return "F16"
        case 107: return "F14"
        case 109: return "F10"
        case 111: return "F12"
        case 113: return "F15"
        case 114: return "Help"
        case 115: return "Home"
        case 116: return "PageUp"
        case 117: return "ForwardDelete"
        case 118: return "F4"
        case 119: return "End"
        case 120: return "F2"
        case 121: return "PageDown"
        case 122: return "F1"
        case 123: return "LeftArrow"
        case 124: return "RightArrow"
        case 125: return "DownArrow"
        case 126: return "UpArrow"
        default: return "Key\(keyCode)"
        }
    }
    
    private func recordEvent(key: String, keyCode: Int, modifiers: [String], type: String) {
        syncQueue.sync {
            if isSynced {
                writeEvent(
                    timestamp: getCurrentTimestamp(),
                    key: key,
                    keyCode: keyCode,
                    modifiers: modifiers,
                    type: type
                )
            } else {
                pendingEvents.append((key: key, keyCode: keyCode, modifiers: modifiers, type: type, wallTime: Date()))
            }
        }
    }
    
    func syncWithVideoStart() {
        syncQueue.sync {
            guard !isSynced else { return }
            
            syncTime = Date()
            isSynced = true
            
            for event in pendingEvents {
                let timestamp = event.wallTime.timeIntervalSince(syncTime!)
                let adjustedTimestamp = max(0, timestamp)
                writeEvent(
                    timestamp: adjustedTimestamp,
                    key: event.key,
                    keyCode: event.keyCode,
                    modifiers: event.modifiers,
                    type: event.type
                )
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
    }
    
    func stop() -> String? {
        isActive = false
        
        if let tap = eventTap {
            CGEvent.tapEnable(tap: tap, enable: false)
        }
        
        if let source = runLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes)
        }
        
        eventTap = nil
        runLoopSource = nil
        
        let effectiveStart = syncTime ?? startTime
        
        guard let handle = fileHandle,
              let path = keysFilePath,
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
        keysFilePath = nil
        
        return path
    }
    
    private func getCurrentTimestamp() -> Double {
        guard let start = syncTime ?? startTime else { return 0 }
        
        if isPaused, let pauseStart = pauseStartTime {
            return pauseStart.timeIntervalSince(start) - totalPausedTime
        }
        return Date().timeIntervalSince(start) - totalPausedTime
    }
    
    private func writeEvent(timestamp: Double, key: String, keyCode: Int, modifiers: [String], type: String) {
        guard let handle = fileHandle else { return }
        
        let modifiersJson = modifiers.map { "\"\($0)\"" }.joined(separator: ",")
        let eventJson = "    {\"timestamp\":\(String(format: "%.3f", timestamp)),\"key\":\"\(key)\",\"keyCode\":\(keyCode),\"modifiers\":[\(modifiersJson)],\"type\":\"\(type)\"},\n"
        
        handle.write(eventJson.data(using: .utf8)!)
        eventCount += 1
    }
}
