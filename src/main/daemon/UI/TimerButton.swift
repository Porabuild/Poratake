import Cocoa

class TimerButton: NSView {
    var remainingSeconds: Int = 5 {
        didSet {
            updateDisplay()
        }
    }
    
    var onCancel: (() -> Void)?
    
    private var gradientLayer: CAGradientLayer!
    private var stackView: NSStackView!
    private var iconView: NSImageView!
    private var label: NSTextField!
    private var isHovered: Bool = false
    private var isPressed: Bool = false
    private var trackingArea: NSTrackingArea?
    private var accentColor: NSColor
    private var foregroundColor: NSColor
    
    init(seconds: Int, accentColor: NSColor, foregroundColor: NSColor) {
        self.accentColor = accentColor
        self.foregroundColor = foregroundColor
        super.init(frame: .zero)
        setupView()
        remainingSeconds = seconds
        updateDisplay()
    }
    
    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError()
    }
    
    private func setupView() {
        wantsLayer = true
        layer?.cornerRadius = 10
        layer?.masksToBounds = true
        
        gradientLayer = CAGradientLayer()
        gradientLayer.cornerRadius = 10
        gradientLayer.startPoint = CGPoint(x: 0.5, y: 0)
        gradientLayer.endPoint = CGPoint(x: 0.5, y: 1)
        layer?.insertSublayer(gradientLayer, at: 0)
        
        iconView = NSImageView()
        iconView.translatesAutoresizingMaskIntoConstraints = false
        if let image = NSImage(systemSymbolName: "timer", accessibilityDescription: nil) {
            let config = NSImage.SymbolConfiguration(pointSize: 18, weight: .semibold)
            iconView.image = image.withSymbolConfiguration(config)
            iconView.contentTintColor = foregroundColor
        }
        
        label = NSTextField(labelWithString: "\(remainingSeconds)")
        label.translatesAutoresizingMaskIntoConstraints = false
        label.isEditable = false
        label.isBordered = false
        label.drawsBackground = false
        label.textColor = foregroundColor
        label.font = NSFont.monospacedDigitSystemFont(ofSize: 24, weight: .bold)
        label.alignment = .left
        label.setContentHuggingPriority(.required, for: .horizontal)
        label.setContentCompressionResistancePriority(.required, for: .horizontal)
        
        stackView = NSStackView(views: [iconView, label])
        stackView.translatesAutoresizingMaskIntoConstraints = false
        stackView.orientation = .horizontal
        stackView.alignment = .centerY
        stackView.spacing = 8
        addSubview(stackView)
        
        NSLayoutConstraint.activate([
            iconView.widthAnchor.constraint(equalToConstant: 22),
            iconView.heightAnchor.constraint(equalToConstant: 22),
            stackView.centerXAnchor.constraint(equalTo: centerXAnchor),
            stackView.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
        
        applyTheme()
    }
    
    override func layout() {
        super.layout()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        gradientLayer.frame = bounds
        CATransaction.commit()
    }
    
    func applyTheme() {
        let darkShade = accentColor.blended(withFraction: 0.12, of: .black) ?? accentColor
        
        if isPressed {
            let pressedAccent = accentColor.blended(withFraction: 0.1, of: .black) ?? accentColor
            let pressedDark = accentColor.blended(withFraction: 0.2, of: .black) ?? accentColor
            gradientLayer.colors = [pressedAccent.cgColor, pressedDark.cgColor, pressedAccent.cgColor]
        } else {
            gradientLayer.colors = [accentColor.cgColor, darkShade.cgColor, accentColor.cgColor]
        }
        
        gradientLayer.locations = [0.0, 0.5, 1.0]
        iconView.contentTintColor = foregroundColor
        label.textColor = foregroundColor
        
        layer?.borderWidth = 0.5
        layer?.borderColor = foregroundColor.withAlphaComponent(0.25).cgColor
    }

    func setTheme(accentColor: NSColor, foregroundColor: NSColor) {
        self.accentColor = accentColor
        self.foregroundColor = foregroundColor
        applyTheme()
    }
    
    private func updateDisplay() {
        if isHovered {
            label.stringValue = "Cancel"
            label.font = NSFont.systemFont(ofSize: 16, weight: .semibold)
        } else {
            label.stringValue = "\(remainingSeconds)"
            label.font = NSFont.monospacedDigitSystemFont(ofSize: 24, weight: .bold)
        }
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
        isHovered = true
        NSCursor.arrow.push()
        updateDisplay()
    }
    
    override func mouseExited(with event: NSEvent) {
        super.mouseExited(with: event)
        isHovered = false
        isPressed = false
        NSCursor.pop()
        applyTheme()
        updateDisplay()
    }
    
    override func mouseDown(with event: NSEvent) {
        isPressed = true
        applyTheme()
    }
    
    override func mouseUp(with event: NSEvent) {
        isPressed = false
        applyTheme()
        let point = convert(event.locationInWindow, from: nil)
        if bounds.contains(point) {
            onCancel?()
        }
    }
    
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        return true
    }
    
}
