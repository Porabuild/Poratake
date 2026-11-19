import Cocoa
import Foundation
import QuartzCore

class TimerControlModule: Module {
    let name = DaemonContract.TimerControl.module
    
    private var panel: FloatingPanel?
    private var timerButton: TimerButton?
    private var timer: Timer?
#if TIMER_CONTROL_TEST
    var testKeepPanelHidden = false
    var testTimerButton: TimerButton? { timerButton }
#endif
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.TimerControl.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .show:
            handleShow(params: params, requestId: requestId)
        case .hide:
            handleHide(requestId: requestId)
        }
    }
    
    private func handleShow(params: [String: AnyCodable]?, requestId: String) {
        guard let x = params?["x"]?.int(),
              let y = params?["y"]?.int(),
              let duration = params?["duration"]?.int(),
              let accentColor = themeColor(params?["color"]?.string()),
              let foregroundColor = themeColor(params?["foregroundColor"]?.string())
        else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "show requires position, duration, and theme colors")
            return
        }
        
        let seconds = max(duration, 1)

        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.showPanel(
                x: x,
                y: y,
                seconds: seconds,
                accentColor: accentColor,
                foregroundColor: foregroundColor
            )
            self.startCountdown(seconds: seconds)
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
        seconds: Int,
        accentColor: NSColor,
        foregroundColor: NSColor
    ) {
        if panel != nil {
            updatePosition(x: x, y: y)
            timerButton?.remainingSeconds = seconds
            timerButton?.setTheme(
                accentColor: accentColor,
                foregroundColor: foregroundColor
            )
            presentPanel(activate: false)
            return
        }
        
        let width: CGFloat = 140
        let height: CGFloat = 52
        
        let cocoaY = DisplayCoordinates.cocoaY(
            topLeftY: CGFloat(y), height: height,
            primaryHeight: DisplayCoordinates.primaryHeight
        )
        
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
            seconds: seconds,
            accentColor: accentColor,
            foregroundColor: foregroundColor
        )
        timerButton?.frame = NSRect(x: 0, y: 0, width: width, height: height)
        timerButton?.onCancel = { [weak self] in
            self?.cancelTimer()
        }
        
        panel?.contentView = timerButton
        presentPanel(activate: true)
    }

    private func presentPanel(activate: Bool) {
#if TIMER_CONTROL_TEST
        guard !testKeepPanelHidden else { return }
#endif
        panel?.makeKeyAndOrderFront(nil)
        timerButton?.resetInteraction()
        if activate { NSApp.activate(ignoringOtherApps: true) }
    }
    
    private func hidePanel() {
        timerButton?.onCancel = nil
        panel?.alphaValue = 0
        if panel != nil { CATransaction.flush() }
        panel?.orderOut(nil)
        panel = nil
        timerButton = nil
    }
    
    private func updatePosition(x: Int, y: Int) {
        panel?.setPosition(x: x, y: y)
    }
    
    private func startCountdown(seconds: Int) {
        stopCountdown()
        let deadline = ContinuousClock.now.advanced(by: .seconds(seconds))
        let timer = Timer(timeInterval: 1.0, repeats: true) { [weak self] _ in
            guard let self = self else { return }

            let remaining = ContinuousClock.now.duration(to: deadline)

            if remaining > .zero {
                let parts = remaining.components
                self.timerButton?.remainingSeconds = Int(parts.seconds) + (parts.attoseconds > 0 ? 1 : 0)
            } else {
                self.stopCountdown()
                self.hidePanel()
                self.emit(event: "completed")
            }
        }
        self.timer = timer
        RunLoop.main.add(timer, forMode: .common)
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
