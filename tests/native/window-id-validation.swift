import CoreGraphics
import Darwin
import Foundation

@main
struct WindowIDValidationTests {
    static func main() throws {
        precondition(AnyCodable(3.75).int() == 3)
        precondition(AnyCodable(-3.75).int() == -3)
        precondition(AnyCodable(0.0).int() == 0)
        precondition(AnyCodable(Double.nan).int() == nil)
        precondition(AnyCodable(Double.infinity).int() == nil)
        precondition(AnyCodable(-Double.infinity).int() == nil)
        precondition(AnyCodable(1e300).int() == nil)
        precondition(AnyCodable(Int.max).int() == Int.max)
        precondition(AnyCodable(Double(Int.max)).int() == nil)
        let largestRepresentable = Double(Int.max).nextDown
        precondition(AnyCodable(largestRepresentable).int() == Int(exactly: largestRepresentable))
        precondition(AnyCodable(Double(Int.min)).int() == Int.min)
        let recorder = ScreenRecorderModule()
        let screenshot = ScreenshotModule()
        let overlay = RecordingOverlayModule()
        let scroll = ScrollCaptureModule()
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
            screenshot.handle(
                method: DaemonContract.Screenshot.Method.captureArea.rawValue,
                params: ["path": AnyCodable("/tmp/invalid-area.png"), "x": AnyCodable(0), "y": AnyCodable(0),
                         "width": AnyCodable(10), "height": AnyCodable(10), "cached": AnyCodable(true), "windowId": window],
                requestId: "area-window-\(index)"
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
            screenshot.handle(
                method: DaemonContract.Screenshot.Method.captureArea.rawValue,
                params: ["path": AnyCodable("/tmp/valid-area.png"), "x": AnyCodable(0), "y": AnyCodable(0),
                         "width": AnyCodable(10), "height": AnyCodable(10), "cached": AnyCodable(true), "windowId": window],
                requestId: "valid-area-window-\(index)"
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

        let invalidScroll: [[String: Any]] = [
            ["x": 1e300], ["y": -1e300], ["x": Int(Int32.max) + 1],
            ["width": 0], ["height": -1], ["height": Int(Int32.max) + 1],
            ["x": Int(Int32.max)], ["y": Int(Int32.max)],
            ["displayOriginX": 1e300], ["displayOriginY": Int(Int32.max) + 1],
            ["displayId": -1], ["displayId": Int(UInt32.max) + 1],
            ["displayId": 1.5], ["displayId": 1e300], ["displayId": true], ["displayId": "1"]
        ]
        let validScroll: [[String: Any]] = [
            [:], ["x": -1440, "y": -900, "displayOriginX": -1440, "displayOriginY": -900],
            ["displayId": 0], ["displayId": 1],
            ["displayId": Int(UInt32.max)], ["displayId": NSNull()]
        ]
        for (index, values) in (invalidScroll + validScroll).enumerated() {
            var params: [String: AnyCodable] = [
                "x": AnyCodable(0), "y": AnyCodable(0), "width": AnyCodable(80), "height": AnyCodable(160)
            ]
            for (key, value) in values {
                params[key] = try JSONDecoder().decode(AnyCodable.self, from: JSONEncoder().encode(AnyCodable(value)))
            }
            scroll.handle(
                method: DaemonContract.ScrollCapture.Method.start.rawValue,
                params: params,
                requestId: "scroll-\(index)"
            )
        }

        fflush(stdout)
        precondition(dup2(savedStdout, STDOUT_FILENO) >= 0)
        close(savedStdout)
        output.fileHandleForWriting.closeFile()
        let bytes = output.fileHandleForReading.readDataToEndOfFile()
        let lines = String(decoding: bytes, as: UTF8.self).split(separator: "\n")
        precondition(lines.count == (invalid.count + valid.count) * 4 + (invalidExclusions.count + validExclusions.count) * 2 + invalidDisplays.count + validDisplays.count + invalidScroll.count + validScroll.count)
        let responses = try lines.map { try JSONDecoder().decode(Response.self, from: Data($0.utf8)) }
        for (index, response) in responses.prefix(invalid.count * 4).enumerated() {
            let action = ["record", "screenshot", "outline", "area-window"][index % 4]
            precondition(response.id == "\(action)-\(index / 4)")
            precondition(!response.success && response.error?.code == "INVALID_PARAMS")
        }
        for (index, response) in responses.dropFirst(invalid.count * 4).prefix(valid.count * 4).enumerated() {
            let action = ["record", "screenshot", "outline", "area-window"][index % 4]
            precondition(response.id == "valid-\(action)-\(index / 4)")
            precondition(response.success && response.error == nil)
        }
        let exclusions = responses.dropFirst((invalid.count + valid.count) * 4)
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
        let displayResponses = exclusions.dropFirst((invalidExclusions.count + validExclusions.count) * 2)
            .prefix(invalidDisplays.count + validDisplays.count)
        for (index, response) in displayResponses.enumerated() {
            precondition(response.id == "area-display-\(index)")
            if index < invalidDisplays.count {
                precondition(!response.success && response.error?.code == "INVALID_PARAMS")
            } else {
                precondition(response.success && response.error == nil)
            }
        }
        for (index, response) in responses.suffix(invalidScroll.count + validScroll.count).enumerated() {
            precondition(response.id == "scroll-\(index)")
            if index < invalidScroll.count {
                precondition(!response.success && response.error?.code == "INVALID_PARAMS")
            } else {
                precondition(response.success && response.error == nil)
            }
        }
        print("macOS window IDs validated before capture")
    }
}
