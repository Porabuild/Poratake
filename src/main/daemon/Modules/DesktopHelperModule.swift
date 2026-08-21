import Cocoa
import Foundation
import QuartzCore

class DesktopHelperModule: Module {
    let name = "desktop-helper"
    private var overlayWindows: [NSWindow] = []
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        switch method {
        case "hide":
            handleHide(requestId: requestId)
        case "show":
            handleShow(requestId: requestId)
        default:
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
        }
    }
    
    private func handleHide(requestId: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.createOverlays()
            self.respond(id: requestId, result: ["hidden": true])
        }
    }
    
    private func handleShow(requestId: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.removeOverlays()
            self.respond(id: requestId, result: ["hidden": false])
        }
    }
    
    private func captureDesktopImage(for screen: NSScreen) -> NSImage? {
        let screenFrame = screen.frame
        let mainScreenHeight = NSScreen.screens.first?.frame.height ?? screenFrame.height
        let captureRect = CGRect(
            x: screenFrame.origin.x,
            y: mainScreenHeight - screenFrame.origin.y - screenFrame.height,
            width: screenFrame.width,
            height: screenFrame.height
        )
        
        if let desktopImage = captureDesktopOnlyImage(for: screen, captureRect: captureRect) {
            return desktopImage
        }
        
        guard let cgImage = CGWindowListCreateImage(
            captureRect,
            .optionOnScreenOnly,
            kCGNullWindowID,
            [.bestResolution, .boundsIgnoreFraming]
        ) else {
            return nil
        }
        
        return NSImage(cgImage: cgImage, size: screenFrame.size)
    }
    
    private func captureDesktopOnlyImage(for screen: NSScreen, captureRect: CGRect) -> NSImage? {
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
        
        if let windowID = desktopWindowID {
            if let cgImage = CGWindowListCreateImage(
                captureRect,
                .optionIncludingWindow,
                windowID,
                [.bestResolution, .boundsIgnoreFraming]
            ) {
                return NSImage(cgImage: cgImage, size: screenFrame.size)
            }
        }
        
        for window in windowList {
            guard let layer = window[kCGWindowLayer as String] as? Int32,
                  let windowID = window[kCGWindowNumber as String] as? CGWindowID else {
                continue
            }
            
            if layer == Int32(CGWindowLevelForKey(.desktopIconWindow)) {
                if let cgImage = CGWindowListCreateImage(
                    captureRect,
                    .optionOnScreenBelowWindow,
                    windowID,
                    [.bestResolution, .boundsIgnoreFraming]
                ) {
                    return NSImage(cgImage: cgImage, size: screenFrame.size)
                }
            }
        }
        
        return nil
    }
    
    private func createOverlays() {
        removeOverlays()
        
        for screen in NSScreen.screens {
            let window = NSWindow(
                contentRect: screen.frame,
                styleMask: .borderless,
                backing: .buffered,
                defer: false
            )
            
            window.level = NSWindow.Level(
                rawValue: Int(CGWindowLevelForKey(.desktopIconWindow)) + 1
            )
            
            let contentView = NSView(frame: NSRect(origin: .zero, size: screen.frame.size))
            let imageView = NSImageView(frame: contentView.bounds)
            imageView.autoresizingMask = [.width, .height]
            
            if let desktopImage = captureDesktopImage(for: screen) {
                imageView.image = desktopImage
                imageView.imageScaling = .scaleAxesIndependently
            }
            
            contentView.addSubview(imageView)
            window.contentView = contentView
            window.isOpaque = true
            window.hasShadow = false
            window.ignoresMouseEvents = false
            window.collectionBehavior = [.canJoinAllSpaces, .stationary]
            window.animationBehavior = .none
            
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            window.orderFront(nil)
            CATransaction.commit()
            
            overlayWindows.append(window)
        }
    }
    
    private func removeOverlays() {
        for window in overlayWindows {
            window.orderOut(nil)
        }
        overlayWindows.removeAll()
    }
}
