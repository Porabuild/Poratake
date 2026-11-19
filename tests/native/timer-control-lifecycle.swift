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
#if TIMER_CONTROL_TEST
    var hiddenCase: String?
    var visibleAccessibility = false
    var restartPress = false
    var resumedAt: ContinuousClock.Instant?

    func startHiddenCase(_ name: String) {
        hiddenCase = name
        module.testKeepPanelHidden = true
        if name == "--queued" {
            show(0, duration: 30, requestId: "first")
        }
        show(1, duration: 2, requestId: "last")
    }

    func hiddenResponse(_ id: String, _ result: Any?) {
        guard let payload = result as? [String: Bool], payload["visible"] == true,
              let timer = module.testTimerButton, timer.window?.isVisible == false else {
            fail("missing hidden timer after \(id)")
            return
        }
        let expected = id == "first" ? 30 : 2
        guard timer.remainingSeconds == expected else {
            fail("\(id) displayed \(timer.remainingSeconds), expected \(expected)")
            return
        }
        guard id == "last" else { return }
        if hiddenCase == "--accessibility" {
            guard timer.isAccessibilityElement(), timer.accessibilityRole() == .button,
                  timer.accessibilityLabel() == "Cancel screenshot countdown",
                  timer.accessibilityValue() as? String == "2 seconds remaining",
                  timer.accessibilityChildren()?.isEmpty == true else {
                fail("timer did not expose one accessible button with its remaining time")
                return
            }
            timer.remainingSeconds = 1
            guard timer.accessibilityValue() as? String == "1 second remaining",
                  timer.accessibilityPerformPress(),
                  !timer.accessibilityPerformPress() else {
                fail("timer did not update its accessible value or accept a press")
                return
            }
            return
        }
        if hiddenCase == "--queued" {
            print("queued shows display their own duration")
            finish(true)
        }
        Thread.sleep(forTimeInterval: 2.2)
        resumedAt = .now
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            self.fail("countdown did not finish after the overdue main-loop callback")
        }
    }
#endif

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
#if TIMER_CONTROL_TEST
        if hiddenCase != nil {
            hiddenResponse(id, result)
            return
        }
#endif
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
#if TIMER_CONTROL_TEST
        if visibleAccessibility {
            checkVisibleAccessibility(button, in: panel)
            return
        }
        if restartPress {
            checkRestartPress(button, in: panel, restarted: index == 1)
            return
        }
#endif
        if id == "cleanup" {
            module.handle(method: "hide", params: nil, requestId: "hidden")
        } else if index == 0 {
            guard let down = NSEvent.mouseEvent(with: .leftMouseDown, location: NSPoint(x: 49, y: 26), modifierFlags: [], timestamp: 0, windowNumber: panel.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1),
                  let up = NSEvent.mouseEvent(with: .leftMouseUp, location: NSPoint(x: 49, y: 26), modifierFlags: [], timestamp: 0, windowNumber: panel.windowNumber, context: nil, eventNumber: 2, clickCount: 1, pressure: 0) else {
                fail("could not construct cancellation events")
                return
            }
            button.layoutSubtreeIfNeeded()
            for x in [10, 49, 70, 91, 130] {
                let point = NSPoint(x: x, y: 26)
                let hit = button.hitTest(point)
                guard hit === button else {
                    fail("timer click at x=\(x) hit \(String(describing: hit.map { type(of: $0) }))")
                    return
                }
            }
            guard button.hitTest(NSPoint(x: -1, y: 26)) == nil else {
                fail("timer button captured a click outside the panel")
                return
            }
            guard let entered = NSEvent.enterExitEvent(
                with: .mouseEntered, location: NSPoint(x: 49, y: 26), modifierFlags: [],
                timestamp: 0, windowNumber: panel.windowNumber, context: nil,
                eventNumber: 0, trackingNumber: 0, userData: nil
            ) else {
                fail("could not construct a countdown hover event")
                return
            }
            let previousCursor = NSCursor.current
            let sentinel = previousCursor == NSCursor.crosshair ? NSCursor.pointingHand : NSCursor.crosshair
            sentinel.push()
            button.mouseEntered(with: entered)
            panel.sendEvent(down)
            panel.sendEvent(up)
            NSCursor.pop()
            guard NSCursor.current == previousCursor else {
                previousCursor.set()
                fail("dismissing the hovered countdown left an extra cursor on the stack")
                return
            }
            print("hover dismissal: cursor stack restored without a mouse-exit event")
        }
    }

#if TIMER_CONTROL_TEST
    func checkRestartPress(_ button: TimerButton, in panel: NSWindow, restarted: Bool) {
        let location = NSPoint(x: 49, y: 26)
        guard let event = NSEvent.mouseEvent(with: restarted ? .leftMouseUp : .leftMouseDown,
            location: location, modifierFlags: [], timestamp: 0, windowNumber: panel.windowNumber,
            context: nil, eventNumber: restarted ? 2 : 1, clickCount: 1, pressure: restarted ? 0 : 1
        ) else {
            fail("could not construct the countdown press")
            return
        }
        if !restarted {
            guard let entered = NSEvent.enterExitEvent(
                with: .mouseEntered, location: location, modifierFlags: [], timestamp: 0,
                windowNumber: panel.windowNumber, context: nil, eventNumber: 0,
                trackingNumber: 0, userData: nil
            ) else {
                fail("could not construct the countdown hover")
                return
            }
            button.mouseEntered(with: entered)
            panel.sendEvent(event)
            show(1, duration: 30)
            return
        }
        let label = button.subviews.compactMap { $0 as? NSStackView }.first?
            .arrangedSubviews.compactMap { $0 as? NSTextField }.first
        let caption = label?.stringValue
        print("restart caption: \(caption ?? "missing")")
        fflush(stdout)
        panel.sendEvent(event)
        guard panel.isVisible, button.remainingSeconds == 30, caption == "30" else {
            fail("the previous countdown's pointer state survived the restart")
            return
        }
        print("restart: new countdown stayed visible after the old mouse-up")
        module.handle(method: "hide", params: nil, requestId: "hidden")
    }

    func checkVisibleAccessibility(_ button: TimerButton, in panel: NSWindow) {
        let frame = panel.frame
        let center = NSPoint(x: frame.midX, y: frame.midY)
        guard button.accessibilityFrame() == frame,
              panel.accessibilityHitTest(center) as? TimerButton === button,
              button.accessibilityLabel() == "Cancel screenshot countdown",
              button.accessibilityValue() as? String == "2 seconds remaining" else {
            fail("the visible countdown was not located as its accessible button")
            return
        }
        awaitAccessibleTick(button)
    }

    func awaitAccessibleTick(_ button: TimerButton) {
        if button.accessibilityValue() as? String == "1 second remaining" {
            guard button.accessibilityPerformPress(), !button.accessibilityPerformPress() else {
                fail("the visible accessible button did not accept exactly one press")
                return
            }
            return
        }
        guard button.window?.isVisible == true else {
            fail("the visible countdown disappeared before reporting its running value")
            return
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.02) {
            self.awaitAccessibleTick(button)
        }
    }
#endif

    func event(_ name: String) {
#if TIMER_CONTROL_TEST
        if restartPress {
            fail("the previous press sent \(name) to the restarted countdown")
            return
        }
        if visibleAccessibility {
            guard name == "cancel", module.testTimerButton == nil else {
                fail("visible accessibility press did not cancel the countdown")
                return
            }
            checkDismissed(name)
            DispatchQueue.main.async {
                print("visible accessible countdown reported its tick and canceled through AppKit")
                self.finish(true)
            }
            return
        }
        if hiddenCase == "--accessibility" {
            guard name == "cancel", module.testTimerButton == nil else {
                fail("accessible press did not cancel and dismiss the countdown")
                return
            }
            DispatchQueue.main.async {
                print("accessible countdown reported its value and canceled through AppKit")
                self.finish(true)
            }
            return
        }
        if hiddenCase != nil {
            guard name == "completed", module.testTimerButton == nil, let resumedAt else {
                fail("unexpected event during hidden countdown: \(name)")
                return
            }
            guard resumedAt.duration(to: .now) < .milliseconds(500) else {
                fail("overdue countdown completed after another tick")
                return
            }
            print("overdue countdown completed after the main loop resumed")
            finish(true)
        }
#endif
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
        DispatchQueue.main.async {
#if TIMER_CONTROL_TEST
            if CommandLine.arguments.last == "--restart-press" {
                let work = NSScreen.screens.first!.visibleFrame
                let pointer = NSEvent.mouseLocation
                let left = Int(work.minX)
                let right = Int(work.maxX - 140)
                fixture.positions = pointer.x < work.midX
                    ? [(left, numbers[1]), (right, numbers[3])]
                    : [(right, numbers[1]), (left, numbers[3])]
                fixture.restartPress = true
                fixture.show(0, duration: 30)
                return
            }
            if CommandLine.arguments.last == "--visible-accessibility" {
                fixture.visibleAccessibility = true
                fixture.show(0, duration: 2)
                return
            }
            if let name = CommandLine.arguments.last, ["--accessibility", "--late", "--queued"].contains(name) {
                fixture.startHiddenCase(name)
                return
            }
#endif
            fixture.show(0, duration: 30)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 10) {
            if !fixture.passed { fixture.fail("countdown timed out") }
        }
        NSApp.run()
        fixture.finish(false)
    }
}
