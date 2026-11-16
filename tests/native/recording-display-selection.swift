import CoreGraphics
import Foundation

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
