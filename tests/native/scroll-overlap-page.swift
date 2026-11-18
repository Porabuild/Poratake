import AppKit
import QuartzCore

final class PageView: NSView {
    var offset = 0
    override var isFlipped: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        let bandHeight = 1
        let first = (Int(dirtyRect.minY) + offset) / bandHeight
        let last = (Int(dirtyRect.maxY) + offset) / bandHeight
        for band in first...last {
            for column in 0..<12 {
                var value = UInt32(band * 12 + column + 1) &* 0x9e3779b9
                value ^= value >> 16
                value = value &* 0x85ebca6b
                value ^= value >> 13
                let red = CGFloat(value & 127) / 255 + 0.22
                let green = CGFloat((value >> 8) & 127) / 255 + 0.22
                let blue = CGFloat((value >> 16) & 127) / 255 + 0.22
                NSColor(srgbRed: red, green: green, blue: blue, alpha: 1).setFill()
                let x1 = (bounds.width * CGFloat(column) / 12).rounded(.down)
                let x2 = (bounds.width * CGFloat(column + 1) / 12).rounded(.down)
                NSRect(x: x1, y: CGFloat(band * bandHeight - offset),
                       width: x2 - x1, height: CGFloat(bandHeight)).fill()
            }
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
guard NSScreen.screens.count == 1, let screen = NSScreen.main else { exit(2) }
let panel = NSPanel(contentRect: screen.frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
let page = PageView(frame: NSRect(origin: .zero, size: screen.frame.size))
panel.level = NSWindow.Level(rawValue: 100)
panel.contentView = page
panel.isOpaque = true
panel.hasShadow = false
panel.ignoresMouseEvents = true
panel.orderFrontRegardless()
func emit(_ message: String) { print(message); fflush(stdout) }
emit("READY \(panel.windowNumber) \(Int(screen.frame.width)) \(Int(screen.frame.height))")
DispatchQueue.global().async {
    while let line = readLine(), let offset = Int(line), offset >= 0 {
        DispatchQueue.main.async {
            page.offset = offset
            page.needsDisplay = true
            panel.displayIfNeeded()
            CATransaction.flush()
            emit("OFFSET \(offset)")
        }
    }
    DispatchQueue.main.async { app.terminate(nil) }
}
app.run()
