import AppKit
import Foundation

let width: CGFloat = 460
let headerHeight: CGFloat = 72
let bodyHeight: CGFloat = 1080
let viewportHeight: CGFloat = 308

func emit(_ value: [String: Any]) {
    let data = try! JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    print(String(decoding: data, as: UTF8.self))
    fflush(stdout)
}

final class PageView: NSView {
    let inset: CGFloat
    let visibleBodyHeight: CGFloat
    var offset: CGFloat = 0

    init(frame: NSRect, inset: CGFloat, visibleBodyHeight: CGFloat) {
        self.inset = inset
        self.visibleBodyHeight = visibleBodyHeight
        super.init(frame: frame)
    }

    required init?(coder: NSCoder) { fatalError("unsupported initializer") }
    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        NSColor(srgbRed: 0.13, green: 0.17, blue: 0.24, alpha: 1).setFill()
        bounds.fill()
        NSColor(srgbRed: 0.34, green: 0.12, blue: 0.54, alpha: 1).setFill()
        NSRect(x: inset, y: inset, width: width, height: headerHeight).fill()
        let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 15, weight: .bold), .foregroundColor: NSColor.white]
        ("Fixed header" as NSString).draw(at: NSPoint(x: inset + 18, y: inset + 23), withAttributes: attributes)
        let bodyY = inset + headerHeight
        NSGraphicsContext.saveGraphicsState()
        NSBezierPath(rect: NSRect(x: inset, y: bodyY, width: width, height: visibleBodyHeight)).addClip()
        let first = max(0, Int(offset))
        let last = min(Int(bodyHeight), Int(ceil(offset + visibleBodyHeight)))
        for row in first..<last {
            var value = UInt32(row) &+ 0x9e3779b9
            value ^= value << 13
            value ^= value >> 17
            value ^= value << 5
            let red = CGFloat(25 + (value & 127)) / 255
            let green = CGFloat(35 + ((value >> 8) & 127)) / 255
            let blue = CGFloat(50 + ((value >> 16) & 127)) / 255
            NSColor(srgbRed: red, green: green, blue: blue, alpha: 1).setFill()
            NSRect(x: inset, y: bodyY + CGFloat(row) - offset, width: width, height: 1).fill()
        }
        for row in stride(from: (first / 40) * 40, to: last, by: 40) {
            (String(format: "Generated row %02d", row / 40) as NSString)
                .draw(at: NSPoint(x: inset + 18, y: bodyY + CGFloat(row) + 7 - offset), withAttributes: attributes)
        }
        NSGraphicsContext.restoreGraphicsState()
    }

    override func scrollWheel(with event: NSEvent) {
        let previous = offset
        offset = min(max(offset - event.scrollingDeltaY, 0), bodyHeight - visibleBodyHeight)
        needsDisplay = true
        emit(["event": "scroll", "offset": offset, "changed": offset != previous])
    }
}

final class PatternPanel: NSPanel {
    override var canBecomeKey: Bool { true }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let screen = NSScreen.main!
let top: CGFloat = 220
let x: CGFloat = 500
let anchorFrame = NSRect(x: x, y: screen.frame.maxY - top - 620, width: 520, height: 620)
let anchor = NSWindow(contentRect: anchorFrame, styleMask: .borderless, backing: .buffered, defer: false)
anchor.title = "Poratake generated scroll target"
anchor.hasShadow = false
anchor.backgroundColor = NSColor(srgbRed: 0.13, green: 0.17, blue: 0.24, alpha: 1)
anchor.orderFrontRegardless()
let frame = NSRect(x: x, y: screen.frame.maxY - top - 440, width: 520, height: 440)
let panel = PatternPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
panel.contentView = PageView(frame: NSRect(origin: .zero, size: frame.size), inset: 30, visibleBodyHeight: viewportHeight)
panel.hasShadow = false
panel.isOpaque = true
panel.level = NSWindow.Level(rawValue: 50)
panel.hidesOnDeactivate = false
panel.orderFrontRegardless()
if CommandLine.arguments.count == 2 {
    let height = headerHeight + bodyHeight
    let reference = PageView(frame: NSRect(x: 0, y: 0, width: width, height: height), inset: 0, visibleBodyHeight: bodyHeight)
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(width * 2), pixelsHigh: Int(height * 2), bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    bitmap.size = reference.bounds.size
    reference.cacheDisplay(in: reference.bounds, to: bitmap)
    try! bitmap.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
}
emit(["event": "ready", "anchor": anchor.windowNumber, "page": panel.windowNumber, "expectedOffset": bodyHeight - viewportHeight])
DispatchQueue.global().async {
    while let line = readLine() {
        if line == "quit" { break }
    }
    DispatchQueue.main.async { app.terminate(nil) }
}
app.run()
