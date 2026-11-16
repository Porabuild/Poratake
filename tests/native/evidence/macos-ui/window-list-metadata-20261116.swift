import Cocoa
import CoreGraphics

@main
struct WindowListMetadataProbe {
    static func main() {
        guard let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]],
              let ids = screenshotWindowIDs(from: windows, excluding: []) else {
            print("window metadata unavailable")
            exit(1)
        }
        guard let first = ids.first,
              let filtered = screenshotWindowIDs(from: windows, excluding: [first]),
              filtered == Array(ids.dropFirst()) else {
            print("window metadata filtering failed")
            exit(1)
        }
        print("on-screen window IDs validated: \(ids.count); excluding one preserved order")
    }
}
