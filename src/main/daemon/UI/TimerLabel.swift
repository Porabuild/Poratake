import Cocoa

class TimerLabel: NSTextField {
    var elapsedSeconds: Int = 0 {
        didSet {
            updateDisplay()
        }
    }
    
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupView()
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupView()
    }
    
    private func setupView() {
        isEditable = false
        isBordered = false
        isSelectable = false
        drawsBackground = false
        alignment = .left
        
        if let font = NSFont.monospacedDigitSystemFont(ofSize: 13, weight: .semibold) as NSFont? {
            self.font = font
        }
        
        updateDisplay()
    }
    
    private func updateDisplay() {
        stringValue = formatTime(elapsedSeconds)
    }
    
    private func formatTime(_ totalSeconds: Int) -> String {
        let hours = totalSeconds / 3600
        let minutes = (totalSeconds % 3600) / 60
        let seconds = totalSeconds % 60
        
        if hours > 0 {
            return String(format: "%d:%02d:%02d", hours, minutes, seconds)
        }
        return String(format: "%02d:%02d", minutes, seconds)
    }
    
    func applyTheme() {
        textColor = Theme.current.foreground
    }
    
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        applyTheme()
    }
}
