import Cocoa
import Foundation
import QuartzCore

class FreezeScreenModule: Module {
    let name = "freeze-screen"
    private var overlayWindows: [NSWindow] = []
    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var retainedSelf: Unmanaged<FreezeScreenModule>?
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        switch method {
        case "freeze":
            let watchSpaceKey = (params?["watchSpaceKey"]?.value as? Bool) ?? false
            handleFreeze(watchSpaceKey: watchSpaceKey, requestId: requestId)
        case "release":
            handleRelease(requestId: requestId)
        case "prewarm":
            handlePrewarm(requestId: requestId)
        default:
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
        }
    }
    
    private func handleFreeze(watchSpaceKey: Bool, requestId: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.createOverlays()
            if watchSpaceKey {
                self.startKeyMonitor()
            }
            self.respond(id: requestId, result: ["frozen": !self.overlayWindows.isEmpty])
        }
    }
    
    private func handleRelease(requestId: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.stopKeyMonitor()
            self.removeOverlays()
            self.respond(id: requestId, result: ["frozen": false])
        }
    }
    
    private func handlePrewarm(requestId: String) {
        let frame = NSScreen.screens.first?.frame ?? .zero
        DispatchQueue.global(qos: .utility).async { [weak self] in
            if !frame.isEmpty {
                _ = CGWindowListCreateImage(
                    frame,
                    .optionOnScreenOnly,
                    kCGNullWindowID,
                    [.bestResolution]
                )
            }
            self?.respond(id: requestId, result: ["prewarmed": true])
        }
    }
    
    private func startKeyMonitor() {
        stopKeyMonitor()
        
        let retained = Unmanaged.passRetained(self)
        
        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .listenOnly,
            eventsOfInterest: CGEventMask(1 << CGEventType.keyDown.rawValue),
            callback: { _, _, event, refcon -> Unmanaged<CGEvent>? in
                guard let refcon = refcon else { return Unmanaged.passRetained(event) }
                let module = Unmanaged<FreezeScreenModule>.fromOpaque(refcon).takeUnretainedValue()
                
                if event.getIntegerValueField(.keyboardEventKeycode) == 49 {
                    DispatchQueue.main.async {
                        module.stopKeyMonitor()
                        module.removeOverlays()
                    }
                }
                
                return Unmanaged.passRetained(event)
            },
            userInfo: retained.toOpaque()
        ) else {
            retained.release()
            return
        }
        
        retainedSelf = retained
        eventTap = tap
        
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        runLoopSource = source
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
    }
    
    private func stopKeyMonitor() {
        if let tap = eventTap {
            CGEvent.tapEnable(tap: tap, enable: false)
        }
        if let source = runLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes)
        }
        retainedSelf?.release()
        retainedSelf = nil
        eventTap = nil
        runLoopSource = nil
    }
    
    private func captureFrame(for screen: NSScreen) -> (image: CGImage, rect: CGRect)? {
        let screenFrame = screen.frame
        let mainScreenHeight = NSScreen.screens.first?.frame.height ?? screenFrame.height
        let captureRect = CGRect(
            x: screenFrame.origin.x,
            y: mainScreenHeight - screenFrame.origin.y - screenFrame.height,
            width: screenFrame.width,
            height: screenFrame.height
        )

        guard let cgImage = CGWindowListCreateImage(
            captureRect,
            .optionOnScreenOnly,
            kCGNullWindowID,
            [.bestResolution]
        ) else {
            return nil
        }

        return (cgImage, captureRect)
    }
    
    private func createOverlays() {
        removeOverlays()

        let started = CFAbsoluteTimeGetCurrent()
        var capturedAt = started

        for screen in NSScreen.screens {
            guard let frame = captureFrame(for: screen) else {
                continue
            }
            capturedAt = CFAbsoluteTimeGetCurrent()

            FrozenFrameStore.store(FrozenFrame(
                image: frame.image,
                rect: frame.rect,
                scale: CGFloat(frame.image.width) / frame.rect.width
            ))

            let window = NSWindow(
                contentRect: screen.frame,
                styleMask: .borderless,
                backing: .buffered,
                defer: false
            )
            
            window.level = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.overlayWindow)) - 1)
            
            let contentView = NSView(frame: NSRect(origin: .zero, size: screen.frame.size))
            let imageView = NSImageView(frame: contentView.bounds)
            imageView.autoresizingMask = [.width, .height]
            imageView.image = NSImage(cgImage: frame.image, size: screen.frame.size)
            imageView.imageScaling = .scaleAxesIndependently
            
            contentView.addSubview(imageView)
            window.contentView = contentView
            window.isOpaque = true
            window.hasShadow = false
            window.ignoresMouseEvents = true
            window.collectionBehavior = [.canJoinAllSpaces, .stationary]
            window.animationBehavior = .none
            
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            window.orderFront(nil)
            CATransaction.commit()
            
            overlayWindows.append(window)
        }

        if ProcessInfo.processInfo.environment["PORATAKE_CAPTURE_TIMING"] != nil {
            let total = CFAbsoluteTimeGetCurrent() - started
            let capture = capturedAt - started
            let line = "[freeze-timing] capture=\(capture)s present=\(total - capture)s total=\(total)s displays=\(overlayWindows.count)\n"
            if let data = line.data(using: .utf8) {
                FileHandle.standardError.write(data)
            }
        }
    }
    
    private func removeOverlays() {
        for window in overlayWindows {
            window.orderOut(nil)
        }
        overlayWindows.removeAll()
        FrozenFrameStore.clear()
    }
}
