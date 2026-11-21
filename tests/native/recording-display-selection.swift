import CoreGraphics
import Foundation
import AppKit

@main
struct RecordingDisplaySelectionTests {
    static func main() throws {
        let displays: [CGDirectDisplayID] = [11, 22]
        let selected = try recordingDisplayID(available: displays, windowDisplayID: nil, requestedDisplayID: 22, mainDisplayID: 11)
        let moved = try recordingDisplayID(available: displays, windowDisplayID: 22, requestedDisplayID: 11, mainDisplayID: 11)
        let movedFromRemoved = try recordingDisplayID(available: displays, windowDisplayID: 22, requestedDisplayID: 99, mainDisplayID: 11)
        let main = try recordingDisplayID(available: displays, windowDisplayID: nil, requestedDisplayID: nil, mainDisplayID: 11)
        let noMain = try recordingDisplayID(available: displays, windowDisplayID: nil, requestedDisplayID: nil, mainDisplayID: 99)
        precondition([selected, moved, movedFromRemoved, main, noMain] == [22, 22, 22, 11, 11])
        do {
            _ = try recordingDisplayID(available: displays, windowDisplayID: nil, requestedDisplayID: 99, mainDisplayID: 11)
            fatalError("An unavailable selected display must not fall back to another display")
        } catch RecorderError.configuration {
        }
        do {
            _ = try recordingDisplayID(available: [], windowDisplayID: nil, requestedDisplayID: nil, mainDisplayID: 11)
            fatalError("Recording without a display must fail")
        } catch RecorderError.configuration {
        }
        let display = CGRect(x: -1440, y: -900, width: 1440, height: 900)
        let selectedArea = CGRect(x: -1430, y: -880, width: 100, height: 200)
        let sourceRect = try recordingSourceRect(selection: selectedArea, display: display)
        precondition(sourceRect == CGRect(x: 10, y: 20, width: 100, height: 200))
        let wholeDisplay = try recordingSourceRect(selection: display, display: display)
        precondition(wholeDisplay == CGRect(x: 0, y: 0, width: 1440, height: 900))
        for invalid in [
            CGRect(x: -1441, y: -880, width: 100, height: 200),
            CGRect(x: -1430, y: -901, width: 100, height: 200),
            CGRect(x: -100, y: -880, width: 101, height: 200),
            CGRect(x: -1430, y: -100, width: 100, height: 101),
            CGRect(x: -1430, y: -880, width: 0, height: 200),
            CGRect(x: CGFloat.nan, y: -880, width: 100, height: 200),
        ] {
            do {
                _ = try recordingSourceRect(selection: invalid, display: display)
                fatalError("An empty or out-of-display area must not be recorded")
            } catch RecorderError.configuration {
            }
        }
        var count: UInt32 = 0
        guard CGGetActiveDisplayList(0, nil, &count) == .success, count > 0 else {
            print("Synthetic recording display selection passed; no active display metadata")
            return
        }
        var active = [CGDirectDisplayID](repeating: 0, count: Int(count))
        guard CGGetActiveDisplayList(count, &active, &count) == .success else {
            fatalError("Could not enumerate active display metadata")
        }
        active = Array(active.prefix(Int(count)))
        let actualMain = CGMainDisplayID()
        let chosen = try recordingDisplayID(available: active, windowDisplayID: nil, requestedDisplayID: actualMain, mainDisplayID: actualMain)
        precondition(chosen == actualMain)
        guard let mainScreen = NSScreen.screens.first(where: {
            ($0.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID) == actualMain
        }), let referenceScreen = NSScreen.screens.first else {
            fatalError("Could not resolve the main AppKit display")
        }
        let frame = mainScreen.frame
        let screenBounds = CGRect(x: frame.minX, y: referenceScreen.frame.height - frame.maxY, width: frame.width, height: frame.height)
        precondition(screenBounds == CGDisplayBounds(actualMain))
        let activeSource = try recordingSourceRect(selection: screenBounds, display: screenBounds)
        precondition(activeSource == CGRect(origin: .zero, size: screenBounds.size))
        var unavailable = CGDirectDisplayID.max
        while active.contains(unavailable) { unavailable -= 1 }
        do {
            _ = try recordingDisplayID(available: active, windowDisplayID: nil, requestedDisplayID: unavailable, mainDisplayID: actualMain)
            fatalError("An unavailable display must not select an active display")
        } catch RecorderError.configuration {
        }
        print("Active display metadata selection passed for \(active.count) display(s)")
        print("Recording display selection and moved-window preference passed")
    }
}
