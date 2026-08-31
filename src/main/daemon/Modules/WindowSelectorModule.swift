import Cocoa
import Foundation

class WindowSelectorModule: Module {
    let name = DaemonContract.WindowSelector.module

    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.WindowSelector.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .list:
            let windows = collectVisibleWindows().map { info -> [String: Any] in
                [
                    "windowId": info.windowId,
                    "title": info.title,
                    "ownerName": info.ownerName,
                    "ownerPid": info.ownerPid,
                    "bounds": [
                        "x": Int(info.bounds.origin.x),
                        "y": Int(info.bounds.origin.y),
                        "width": Int(info.bounds.width),
                        "height": Int(info.bounds.height)
                    ]
                ]
            }
            respond(id: requestId, result: ["windows": windows])
        }
    }
}

struct WindowSelectorInfo {
    let windowId: Int
    let title: String
    let ownerName: String
    let ownerPid: Int
    let bounds: CGRect
}

func collectVisibleWindows() -> [WindowSelectorInfo] {
    var result: [WindowSelectorInfo] = []
    let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
    guard let windowList = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
        return result
    }
    let myPid = ProcessInfo.processInfo.processIdentifier

    for windowDict in windowList {
        guard let windowId = windowDict[kCGWindowNumber as String] as? Int,
              let ownerPid = windowDict[kCGWindowOwnerPID as String] as? Int,
              let boundsDict = windowDict[kCGWindowBounds as String] as? [String: Any],
              let x = boundsDict["X"] as? CGFloat,
              let y = boundsDict["Y"] as? CGFloat,
              let width = boundsDict["Width"] as? CGFloat,
              let height = boundsDict["Height"] as? CGFloat,
              let layer = windowDict[kCGWindowLayer as String] as? Int
        else { continue }

        if ownerPid == Int(myPid) { continue }
        if layer != 0 { continue }
        if width < 50 || height < 50 { continue }

        let ownerName = windowDict[kCGWindowOwnerName as String] as? String ?? "Unknown"
        let title = windowDict[kCGWindowName as String] as? String ?? ""
        result.append(WindowSelectorInfo(
            windowId: windowId,
            title: title.isEmpty ? ownerName : title,
            ownerName: ownerName,
            ownerPid: ownerPid,
            bounds: CGRect(x: x, y: y, width: width, height: height)
        ))
    }

    return result
}
