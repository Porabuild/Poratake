import Cocoa
import Foundation

class TimerControlModule: Module {
    let name = "timer-control"
    
    private var panel: FloatingPanel?
    private var timerButton: TimerButton?
    private var timer: Timer?
    private var remainingSeconds: Int = 5
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        switch method {
        case "show":
            handleShow(params: params, requestId: requestId)
        case "hide":
            handleHide(requestId: requestId)
        default:
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
        }
    }
    
    private func handleShow(params: [String: AnyCodable]?, requestId: String) {
        let x = params?["x"]?.int() ?? 100
        let y = params?["y"]?.int() ?? 100
        let duration = params?["duration"]?.int() ?? 5
        guard let accentColor = themeColor(params?["color"]?.string()),
              let foregroundColor = themeColor(params?["foregroundColor"]?.string())
        else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "show requires theme colors")
            return
        }
        
        remainingSeconds = duration
        
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.showPanel(
                x: x,
                y: y,
                accentColor: accentColor,
                foregroundColor: foregroundColor
            )
            self.startCountdown()
            self.respond(id: requestId, result: ["visible": true])
        }
    }
    
    private func handleHide(requestId: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.stopCountdown()
            self.hidePanel()
            self.respond(id: requestId, result: ["visible": false])
        }
    }
    
    private func showPanel(
        x: Int,
        y: Int,
        accentColor: NSColor,
        foregroundColor: NSColor
    ) {
        if panel != nil {
            updatePosition(x: x, y: y)
            timerButton?.remainingSeconds = remainingSeconds
            timerButton?.setTheme(
                accentColor: accentColor,
                foregroundColor: foregroundColor
            )
            panel?.makeKeyAndOrderFront(nil)
            return
        }
        
        let width: CGFloat = 120
        let height: CGFloat = 44
        
        let mainScreenHeight = NSScreen.main?.frame.height ?? 0
        let cocoaY = mainScreenHeight - CGFloat(y) - height
        
        var config = FloatingPanelConfig()
        config.cornerRadius = 10
        config.escapeToClose = true
        config.hasShadow = true
        config.onEscape = { [weak self] in
            self?.cancelTimer()
        }
        
        panel = FloatingPanel(
            contentRect: NSRect(x: CGFloat(x), y: cocoaY, width: width, height: height),
            config: config
        )
        
        timerButton = TimerButton(
            seconds: remainingSeconds,
            accentColor: accentColor,
            foregroundColor: foregroundColor
        )
        timerButton?.frame = NSRect(x: 0, y: 0, width: width, height: height)
        timerButton?.onCancel = { [weak self] in
            self?.cancelTimer()
        }
        
        panel?.contentView = timerButton
        panel?.makeKeyAndOrderFront(nil)
        
        NSApp.activate(ignoringOtherApps: true)
    }
    
    private func hidePanel() {
        panel?.orderOut(nil)
        panel = nil
        timerButton = nil
    }
    
    private func updatePosition(x: Int, y: Int) {
        panel?.setPosition(x: x, y: y)
    }
    
    private func startCountdown() {
        timer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            guard let self = self else { return }

            self.remainingSeconds -= 1

            if self.remainingSeconds > 0 {
                self.timerButton?.remainingSeconds = self.remainingSeconds
            } else {
                self.stopCountdown()
                self.hidePanel()
                self.emit(event: "completed")
            }
        }
    }
    
    private func stopCountdown() {
        timer?.invalidate()
        timer = nil
    }
    
    private func cancelTimer() {
        stopCountdown()
        hidePanel()
        emit(event: "cancel")
    }
}
