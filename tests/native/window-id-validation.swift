import CoreGraphics
import Darwin
import Foundation

@main
struct WindowIDValidationTests {
    static func main() throws {
        let recorder = ScreenRecorderModule()
        let screenshot = ScreenshotModule()
        let overlay = RecordingOverlayModule()
        let invalid: [Any] = [
            0,
            -1,
            Int(UInt32.max) + 1,
            Int.max,
            Int.min,
            1.5,
            1e300,
            "7",
            true
        ]
        let valid = [1, Int(UInt32.max)]
        let output = Pipe()
        let savedStdout = dup(STDOUT_FILENO)
        precondition(savedStdout >= 0)
        precondition(dup2(output.fileHandleForWriting.fileDescriptor, STDOUT_FILENO) >= 0)

        for (index, value) in invalid.enumerated() {
            let window = try JSONDecoder().decode(AnyCodable.self, from: JSONEncoder().encode(AnyCodable(value)))
            recorder.handle(
                method: DaemonContract.ScreenRecorder.Method.start.rawValue,
                params: ["outputPath": AnyCodable("/tmp/invalid-window.mov"), "windowId": window],
                requestId: "record-\(index)"
            )
            screenshot.handle(
                method: DaemonContract.Screenshot.Method.captureWindow.rawValue,
                params: ["path": AnyCodable("/tmp/invalid-window.png"), "windowId": window],
                requestId: "screenshot-\(index)"
            )
            overlay.handle(
                method: DaemonContract.RecordingOverlay.Method.showWindow.rawValue,
                params: ["windowId": window],
                requestId: "outline-\(index)"
            )
        }
        for (index, value) in valid.enumerated() {
            let window = try JSONDecoder().decode(AnyCodable.self, from: JSONEncoder().encode(AnyCodable(value)))
            recorder.handle(
                method: DaemonContract.ScreenRecorder.Method.start.rawValue,
                params: ["outputPath": AnyCodable("/tmp/valid-window.mov"), "windowId": window],
                requestId: "valid-record-\(index)"
            )
            screenshot.handle(
                method: DaemonContract.Screenshot.Method.captureWindow.rawValue,
                params: ["path": AnyCodable("/tmp/valid-window.png"), "windowId": window],
                requestId: "valid-screenshot-\(index)"
            )
            overlay.handle(
                method: DaemonContract.RecordingOverlay.Method.showWindow.rawValue,
                params: ["windowId": window],
                requestId: "valid-outline-\(index)"
            )
        }
        let invalidExclusions: [[Any]] = [[0], [-1], [Int(UInt32.max) + 1], [1, 0], [1.5], [true], ["1"]]
        let validExclusions: [[Any]] = [[], [1], [Int(UInt32.max)], [1, 2]]
        let invalidDisplays: [Any] = [-1, Int(UInt32.max) + 1, 1.5, 1e300, true, "1"]
        let validDisplays: [Any] = [0, 1, Int(UInt32.max), NSNull()]
        for (index, values) in (invalidExclusions + validExclusions).enumerated() {
            let ids = try JSONDecoder().decode(AnyCodable.self, from: JSONEncoder().encode(AnyCodable(values)))
            screenshot.handle(
                method: DaemonContract.Screenshot.Method.captureArea.rawValue,
                params: ["path": AnyCodable("/tmp/validation.png"), "x": AnyCodable(0), "y": AnyCodable(0), "width": AnyCodable(10), "height": AnyCodable(10), "excludedWindowIds": ids],
                requestId: "area-exclusion-\(index)"
            )
            recorder.handle(
                method: DaemonContract.ScreenRecorder.Method.start.rawValue,
                params: ["outputPath": AnyCodable("/tmp/validation.mov"), "excludedWindowIds": ids],
                requestId: "record-exclusion-\(index)"
            )
        }
        for (index, value) in (invalidDisplays + validDisplays).enumerated() {
            let display = try JSONDecoder().decode(AnyCodable.self, from: JSONEncoder().encode(AnyCodable(value)))
            screenshot.handle(
                method: DaemonContract.Screenshot.Method.captureArea.rawValue,
                params: ["path": AnyCodable("/tmp/validation.png"), "x": AnyCodable(0), "y": AnyCodable(0), "width": AnyCodable(10), "height": AnyCodable(10), "displayId": display],
                requestId: "area-display-\(index)"
            )
        }

        fflush(stdout)
        precondition(dup2(savedStdout, STDOUT_FILENO) >= 0)
        close(savedStdout)
        output.fileHandleForWriting.closeFile()
        let bytes = output.fileHandleForReading.readDataToEndOfFile()
        let lines = String(decoding: bytes, as: UTF8.self).split(separator: "\n")
        precondition(lines.count == (invalid.count + valid.count) * 3 + (invalidExclusions.count + validExclusions.count) * 2 + invalidDisplays.count + validDisplays.count)
        let responses = try lines.map { try JSONDecoder().decode(Response.self, from: Data($0.utf8)) }
        for (index, response) in responses.prefix(invalid.count * 3).enumerated() {
            let action = ["record", "screenshot", "outline"][index % 3]
            precondition(response.id == "\(action)-\(index / 3)")
            precondition(!response.success && response.error?.code == "INVALID_PARAMS")
        }
        for (index, response) in responses.dropFirst(invalid.count * 3).prefix(valid.count * 3).enumerated() {
            let action = ["record", "screenshot", "outline"][index % 3]
            precondition(response.id == "valid-\(action)-\(index / 3)")
            precondition(response.success && response.error == nil)
        }
        let exclusions = responses.dropFirst((invalid.count + valid.count) * 3)
        for (index, response) in exclusions.prefix((invalidExclusions.count + validExclusions.count) * 2).enumerated() {
            let action = index % 2 == 0 ? "area" : "record"
            let caseIndex = index / 2
            precondition(response.id == "\(action)-exclusion-\(caseIndex)")
            if caseIndex < invalidExclusions.count {
                precondition(!response.success && response.error?.code == "INVALID_PARAMS")
            } else {
                precondition(response.success && response.error == nil)
            }
        }
        for (index, response) in exclusions.suffix(invalidDisplays.count + validDisplays.count).enumerated() {
            precondition(response.id == "area-display-\(index)")
            if index < invalidDisplays.count {
                precondition(!response.success && response.error?.code == "INVALID_PARAMS")
            } else {
                precondition(response.success && response.error == nil)
            }
        }
        print("macOS window IDs validated before capture")
    }
}
