import Cocoa

@main
struct TimerControlPlacementTest {
    static func main() {
        let values = CommandLine.arguments.dropFirst().compactMap(Int.init)
        precondition(values.count == 8)
        let (displayX, displayY, displayWidth, displayHeight) = (values[0], values[1], values[2], values[3])
        let positions = [(values[4], values[5]), (values[6], values[7])]
        _ = NSApplication.shared
        guard let screen = NSScreen.screens.first else { fatalError("no primary display") }
        precondition(screen.frame.minX == CGFloat(displayX))
        precondition(screen.frame.width == CGFloat(displayWidth))
        precondition(screen.frame.height == CGFloat(displayHeight))
        let primaryHeight = DisplayCoordinates.primaryHeight
        for (index, position) in positions.enumerated() {
            let cocoaY = DisplayCoordinates.cocoaY(
                topLeftY: CGFloat(position.1), height: 52, primaryHeight: primaryHeight
            )
            let panel = FloatingPanel(
                contentRect: NSRect(x: CGFloat(position.0), y: cocoaY, width: 140, height: 52)
            )
            let button = TimerButton(seconds: 5, accentColor: .blue, foregroundColor: .white)
            button.frame = NSRect(x: 0, y: 0, width: 140, height: 52)
            panel.contentView = button
            precondition(!panel.isVisible)
            precondition(
                screen.visibleFrame.contains(panel.frame),
                "Hidden timer at \(panel.frame) must fit AppKit work area \(screen.visibleFrame)"
            )
            precondition(panel.frame.minX == CGFloat(position.0))
            precondition(DisplayCoordinates.topLeftY(
                cocoaY: panel.frame.minY, height: panel.frame.height, primaryHeight: primaryHeight
            ) == CGFloat(position.1))
            print("hidden timer panel \(index): x=\(Int(panel.frame.minX)) width=\(Int(panel.frame.width)) top=\(position.1)")
            panel.close()
        }
        print("primary display: x=\(displayX) y=\(displayY) width=\(displayWidth) height=\(displayHeight)")
    }
}
