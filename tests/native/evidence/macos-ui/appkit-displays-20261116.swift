import AppKit
import CoreGraphics
import Foundation
let screens = NSScreen.screens.map { screen -> [String: Any] in
    let id = (screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.uint32Value ?? 0
    let b = CGDisplayBounds(id)
    return ["id": id, "bounds": ["x": b.origin.x, "y": b.origin.y, "width": b.width, "height": b.height], "scaleFactor": screen.backingScaleFactor, "primary": id == CGMainDisplayID()]
}
let output = try JSONSerialization.data(withJSONObject: screens, options: [.sortedKeys])
print(String(decoding: output, as: UTF8.self))
