import Cocoa

enum IconButtonVariant {
    case normal
    case danger
}

class IconButton: NSButton {
    var tooltipText: String = ""
    var onClick: (() -> Void)?
    var onRightClick: (() -> Void)?
    
    var variant: IconButtonVariant = .normal {
        didSet {
            applyTheme()
        }
    }
    
    private var trackingArea: NSTrackingArea?
    private var currentSymbolSize: CGFloat = 16
    
    init(symbol: String, size: CGFloat = 16, tooltip: String = "") {
        super.init(frame: .zero)
        
        currentSymbolSize = size
        bezelStyle = .regularSquare
        isBordered = false
        title = ""
        tooltipText = tooltip
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        
        if let image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil) {
            let config = NSImage.SymbolConfiguration(pointSize: size, weight: .regular)
            self.image = image.withSymbolConfiguration(config)
        }
        
        target = self
        action = #selector(handleClick)
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
    }
    
    func setSymbol(_ symbol: String, size: CGFloat? = nil) {
        let symbolSize = size ?? currentSymbolSize
        if let newSize = size {
            currentSymbolSize = newSize
        }
        if let image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil) {
            let config = NSImage.SymbolConfiguration(pointSize: symbolSize, weight: .regular)
            self.image = image.withSymbolConfiguration(config)
        }
    }
    
    func applyTheme() {
        let theme = Theme.current
        
        switch variant {
        case .normal:
            contentTintColor = theme.foreground.withAlphaComponent(0.8)
        case .danger:
            contentTintColor = theme.destructiveColor
        }
    }
    
    @objc private func handleClick() {
        onClick?()
    }
    
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        return true
    }
    
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        
        if let existing = trackingArea {
            removeTrackingArea(existing)
        }
        
        trackingArea = NSTrackingArea(
            rect: bounds,
            options: [.activeAlways, .mouseEnteredAndExited],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(trackingArea!)
    }
    
    override func mouseEntered(with event: NSEvent) {
        super.mouseEntered(with: event)
        if let window = window, !tooltipText.isEmpty {
            TooltipManager.shared.show(text: tooltipText, for: self, in: window)
        }
    }
    
    override func mouseExited(with event: NSEvent) {
        super.mouseExited(with: event)
        TooltipManager.shared.hide()
    }
    
    override func rightMouseDown(with event: NSEvent) {
        onRightClick?()
    }
}
