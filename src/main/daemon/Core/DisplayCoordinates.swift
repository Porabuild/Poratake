import Cocoa

enum DisplayCoordinates {
    static var primaryHeight: CGFloat {
        NSScreen.screens.first?.frame.height ?? 0
    }

    static func cocoaY(topLeftY: CGFloat, height: CGFloat, primaryHeight: CGFloat) -> CGFloat {
        primaryHeight - topLeftY - height
    }

    static func topLeftY(cocoaY: CGFloat, height: CGFloat, primaryHeight: CGFloat) -> CGFloat {
        primaryHeight - cocoaY - height
    }
}
