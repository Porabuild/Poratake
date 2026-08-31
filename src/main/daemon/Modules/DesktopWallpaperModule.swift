import Cocoa
import Foundation
import ImageIO

class DesktopWallpaperModule: Module {
    let name = DaemonContract.DesktopWallpaper.module
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.DesktopWallpaper.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .get:
            handleGet(requestId: requestId)
        }
    }
    
    private func handleGet(requestId: String) {
        if let path = getDesktopWallpaperPath() {
            respond(id: requestId, result: ["type": "path", "value": path])
        } else if let imageData = getWallpaperImageData() {
            let base64 = imageData.base64EncodedString()
            respond(id: requestId, result: ["type": "data", "value": "data:image/jpeg;base64,\(base64)"])
        } else {
            respondError(id: requestId, code: "WALLPAPER_UNAVAILABLE", message: "Could not get desktop wallpaper")
        }
    }
    
    private func getDesktopWallpaperPath() -> String? {
        guard let screen = NSScreen.main else {
            return nil
        }
        
        guard let url = NSWorkspace.shared.desktopImageURL(for: screen) else {
            return nil
        }
        
        let path = url.path
        
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory) else {
            return nil
        }
        
        if isDirectory.boolValue {
            return nil
        }
        
        if path.lowercased().hasSuffix(".heic") {
            return nil
        }
        
        return path
    }
    
    private func captureDesktopWallpaper() -> Data? {
        guard let screen = NSScreen.main else {
            return nil
        }
        
        let screenFrame = screen.frame
        
        guard let windowList = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] else {
            return nil
        }
        
        var desktopWindowID: CGWindowID?
        for window in windowList {
            guard let ownerName = window[kCGWindowOwnerName as String] as? String,
                  let layer = window[kCGWindowLayer as String] as? Int32,
                  let windowID = window[kCGWindowNumber as String] as? CGWindowID else {
                continue
            }
            
            if ownerName == "Finder" && layer == Int32(CGWindowLevelForKey(.desktopWindow)) {
                desktopWindowID = windowID
                break
            }
        }
        
        let mainScreenHeight = NSScreen.screens.first?.frame.height ?? screenFrame.height
        let captureRect = CGRect(
            x: screenFrame.origin.x,
            y: mainScreenHeight - screenFrame.origin.y - screenFrame.height,
            width: screenFrame.width,
            height: screenFrame.height
        )
        
        var cgImage: CGImage?
        
        if let windowID = desktopWindowID {
            cgImage = CGWindowListCreateImage(
                captureRect,
                .optionIncludingWindow,
                windowID,
                [.bestResolution, .boundsIgnoreFraming]
            )
        }
        
        if cgImage == nil {
            for window in windowList {
                guard let layer = window[kCGWindowLayer as String] as? Int32,
                      let windowID = window[kCGWindowNumber as String] as? CGWindowID else {
                    continue
                }
                
                if layer == Int32(CGWindowLevelForKey(.desktopIconWindow)) {
                    cgImage = CGWindowListCreateImage(
                        captureRect,
                        .optionOnScreenBelowWindow,
                        windowID,
                        [.bestResolution, .boundsIgnoreFraming]
                    )
                    if cgImage != nil { break }
                }
            }
        }
        
        guard let image = cgImage else {
            return nil
        }
        
        let bitmapRep = NSBitmapImageRep(cgImage: image)
        return bitmapRep.representation(using: .jpeg, properties: [.compressionFactor: 0.85])
    }
    
    private func isVideoDynamicWallpaper(heicPath: String) -> Bool {
        let realPath = (heicPath as NSString).resolvingSymlinksInPath
        let movPath = realPath.replacingOccurrences(of: ".heic", with: ".mov")
        return FileManager.default.fileExists(atPath: movPath)
    }
    
    private func getWallpaperImageData() -> Data? {
        if let screen = NSScreen.main,
           let url = NSWorkspace.shared.desktopImageURL(for: screen) {
            let path = url.path
            
            if path.lowercased().hasSuffix(".heic") {
                if isVideoDynamicWallpaper(heicPath: path) {
                    // Fall through to screen capture
                } else {
                    if let imageData = extractImageFromHEIC(path: path) {
                        return imageData
                    }
                }
            }
        }
        
        return captureDesktopWallpaper()
    }
    
    private func extractImageFromHEIC(path: String) -> Data? {
        let url = URL(fileURLWithPath: path)
        guard let imageSource = CGImageSourceCreateWithURL(url as CFURL, nil) else {
            return nil
        }
        
        let imageCount = CGImageSourceGetCount(imageSource)
        if imageCount == 0 {
            return nil
        }
        
        var imageIndex = 0
        if imageCount > 1 {
            let calendar = Calendar.current
            let hour = calendar.component(.hour, from: Date())
            imageIndex = (hour * imageCount) / 24
            imageIndex = min(imageIndex, imageCount - 1)
        }
        
        guard let cgImage = CGImageSourceCreateImageAtIndex(imageSource, imageIndex, nil) else {
            return nil
        }
        
        let bitmapRep = NSBitmapImageRep(cgImage: cgImage)
        return bitmapRep.representation(using: .jpeg, properties: [.compressionFactor: 0.85])
    }
}
