import AppKit
import CoreGraphics

let mouse = NSEvent.mouseLocation
let top = NSScreen.screens.first?.frame.maxY ?? 0
let location = CGPoint(x: mouse.x, y: top - mouse.y)
print("pointer \(location.x),\(location.y)")
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
for item in windows {
    guard let rawBounds = item[kCGWindowBounds as String] as? [String: Any],
          let bounds = CGRect(dictionaryRepresentation: rawBounds as CFDictionary),
          bounds.insetBy(dx: -120, dy: -120).contains(location) else { continue }
    print("id=\(item[kCGWindowNumber as String] ?? "?") owner=\(item[kCGWindowOwnerName as String] ?? "?") pid=\(item[kCGWindowOwnerPID as String] ?? "?") layer=\(item[kCGWindowLayer as String] ?? "?") bounds=\(bounds) sharing=\(item[kCGWindowSharingState as String] ?? "?")")
}
