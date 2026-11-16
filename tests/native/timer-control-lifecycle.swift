import Cocoa
import Foundation

struct AnyCodable {
    let value: Any
    init(_ value: Any) { self.value = value }
    func int() -> Int? { value as? Int }
    func string() -> String? { value as? String }
}

protocol Module {
    var name: String { get }
    func handle(method: String, params: [String: AnyCodable]?, requestId: String)
}

extension Module {
    func respond(id: String, result: Any? = nil) { Fixture.shared.response(id, result) }
    func respondError(id: String, code: String, message: String) {
        Fixture.shared.fail("\(id): \(code): \(message)")
    }
    func emit(event: String, data: Any? = nil) { Fixture.shared.event(event) }
}

enum DaemonContract {
    enum TimerControl {
        static let module = "timer-control"
        enum Method: String { case show, hide }
    }
}

final class Fixture {
    static let shared = Fixture()
    let module = TimerControlModule()
    let previous = NSWorkspace.shared.frontmostApplication
    var passed = false
    var positions: [(Int, Int)] = []
    var panelID: CGWindowID = 0

    func checkDismissed(_ label: String) {
        guard let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] else {
            fail("cannot inspect timer window after \(label)")
            return
        }
        guard let window = windows.first(where: { ($0[kCGWindowNumber as String] as? UInt32) == panelID }) else {
            print("\(label): timer absent from on-screen window list")
            return
        }
        guard let alpha = window[kCGWindowAlpha as String] as? Double, alpha == 0 else {
            fail("timer remained opaque in the window server after \(label)")
            return
        }
        print("\(label): timer window server alpha is zero")
    }

    func finish(_ success: Bool) -> Never {
        if NSWorkspace.shared.frontmostApplication?.processIdentifier == ProcessInfo.processInfo.processIdentifier {
            _ = previous?.activate(options: [])
        }
        fflush(stdout)
        fflush(stderr)
        exit(success ? 0 : 1)
    }

    func fail(_ message: String) {
        fputs("fixture failure: \(message)\n", stderr)
        finish(false)
    }

    func show(_ index: Int, duration: Int, requestId: String? = nil) {
        let position = positions[index]
        module.handle(method: "show", params: [
            "x": AnyCodable(position.0), "y": AnyCodable(position.1),
            "duration": AnyCodable(duration), "color": AnyCodable("#8892ef"),
            "foregroundColor": AnyCodable("#0a0a12"),
        ], requestId: requestId ?? (index == 0 ? "left" : "right"))
    }

    func response(_ id: String, _ result: Any?) {
        if id == "hidden" {
            guard let payload = result as? [String: Bool], payload["visible"] == false,
                  !NSApp.windows.contains(where: { $0.isVisible && $0.contentView is TimerButton }) else {
                fail("timer remained visible after hide")
                return
            }
            checkDismissed("hide")
            print("hide: timer hidden")
            passed = true
            finish(true)
        }
        guard let payload = result as? [String: Bool], payload["visible"] == true,
              let panel = NSApp.windows.first(where: { $0.isVisible && $0.contentView is TimerButton }),
              let button = panel.contentView as? TimerButton else {
            fail("missing visible timer after \(id)")
            return
        }
        panelID = CGWindowID(panel.windowNumber)
        let index = id == "right" ? 1 : 0
        let position = positions[index]
        guard Int(panel.frame.minX) == position.0,
              Int(DisplayCoordinates.topLeftY(cocoaY: panel.frame.minY, height: panel.frame.height, primaryHeight: DisplayCoordinates.primaryHeight)) == position.1,
              Int(panel.frame.width) == 140, Int(panel.frame.height) == 52 else {
            fail("unexpected timer frame after \(id)")
            return
        }
        print("\(id) visible at x=\(position.0) y=\(position.1) size=140x52")
        if id == "cleanup" {
            module.handle(method: "hide", params: nil, requestId: "hidden")
        } else if index == 0 {
            guard let down = NSEvent.mouseEvent(with: .leftMouseDown, location: NSPoint(x: 70, y: 26), modifierFlags: [], timestamp: 0, windowNumber: panel.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1),
                  let up = NSEvent.mouseEvent(with: .leftMouseUp, location: NSPoint(x: 70, y: 26), modifierFlags: [], timestamp: 0, windowNumber: panel.windowNumber, context: nil, eventNumber: 2, clickCount: 1, pressure: 0) else {
                fail("could not construct cancellation events")
                return
            }
            button.mouseDown(with: down)
            button.mouseUp(with: up)
        }
    }

    func event(_ name: String) {
        guard !NSApp.windows.contains(where: { $0.isVisible && $0.contentView is TimerButton }) else {
            fail("timer remained visible after \(name)")
            return
        }
        checkDismissed(name)
        print("\(name): timer hidden")
        switch name {
        case "cancel": show(1, duration: 1)
        case "completed": show(0, duration: 30, requestId: "cleanup")
        default: fail("unexpected event \(name)")
        }
    }
}

@main
struct TimerControlLifecycleTest {
    static func main() {
        let numbers = CommandLine.arguments.dropFirst().compactMap(Int.init)
        guard numbers.count == 4 else { fatalError("expected left x/y and right x/y") }
        _ = NSApplication.shared
        NSApp.setActivationPolicy(.accessory)
        guard NSScreen.screens.first != nil else {
            print("SKIP timer control lifecycle: no AppKit display")
            return
        }
        let fixture = Fixture.shared
        fixture.positions = [(numbers[0], numbers[1]), (numbers[2], numbers[3])]
        DispatchQueue.main.async { fixture.show(0, duration: 30) }
        DispatchQueue.main.asyncAfter(deadline: .now() + 10) {
            if !fixture.passed { fixture.fail("countdown timed out") }
        }
        NSApp.run()
        fixture.finish(false)
    }
}
