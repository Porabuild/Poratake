import AppKit
import Foundation

func emit(_ value: [String: Any]) {
    let data = try! JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    print(String(decoding: data, as: UTF8.self))
    fflush(stdout)
}

final class PageView: NSView {
    var offset: CGFloat = 0
    let pageHeight: CGFloat = 2200
    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        let first = max(0, Int(offset / 40) - 1)
        let last = min(55, Int((offset + bounds.height) / 40) + 1)
        for row in first...last {
            let y = CGFloat(row * 40) - offset
            let red = CGFloat((row * 71 + 27) % 190 + 30) / 255
            let green = CGFloat((row * 113 + 43) % 190 + 30) / 255
            let blue = CGFloat((row * 151 + 61) % 190 + 30) / 255
            NSColor(srgbRed: red, green: green, blue: blue, alpha: 1).setFill()
            NSRect(x: 0, y: y, width: bounds.width, height: 40).fill()
            let label = String(format: "Generated row %02d", row) as NSString
            label.draw(at: NSPoint(x: 24, y: y + 10), withAttributes: [.font: NSFont.systemFont(ofSize: 15, weight: .bold), .foregroundColor: NSColor.white])
        }
    }
    override func scrollWheel(with event: NSEvent) {
        let previous = offset
        offset = min(max(offset - event.scrollingDeltaY, 0), pageHeight - bounds.height)
        needsDisplay = true
        emit(["event": "scroll", "delta": event.scrollingDeltaY, "offset": offset, "changed": offset != previous])
    }
}

final class PatternPanel: NSPanel {
    override var canBecomeKey: Bool { true }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
guard let screen = NSScreen.main else { fatalError("no main screen") }
let frame = NSRect(x: 500, y: screen.frame.maxY - 220 - 440, width: 520, height: 440)
let panel = PatternPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
panel.contentView = PageView(frame: NSRect(origin: .zero, size: frame.size))
panel.isOpaque = true
panel.backgroundColor = .black
panel.hasShadow = false
panel.level = NSWindow.Level(rawValue: 50)
panel.hidesOnDeactivate = false
panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
panel.orderFrontRegardless()
emit(["event": "ready", "pid": getpid(), "window": panel.windowNumber, "frame": [frame.minX, frame.minY, frame.width, frame.height], "topLeft": [frame.minX, screen.frame.maxY - frame.maxY]])
DispatchQueue.global().async {
    while let line = readLine() {
        if line == "quit" {
            DispatchQueue.main.async {
                panel.orderOut(nil)
                emit(["event": "quit"])
                NSApp.terminate(nil)
            }
            break
        }
    }
}
app.run()
