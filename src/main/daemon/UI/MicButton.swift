import Cocoa

class MicButton: NSView {
    var tooltipText: String = ""
    var onClick: (() -> Void)?
    var onRightClick: (() -> Void)?
    
    private var trackingArea: NSTrackingArea?
    private var currentSymbolSize: CGFloat = 16
    private var isEnabled: Bool = true
    
    private var baseImageView: NSImageView?
    private var fillImageView: NSImageView?
    private var maskLayer: CALayer?
    
    private var smoothedLevel: Float = 0
    
    var level: Float = 0 {
        didSet {
            smoothedLevel = smoothedLevel * 0.6 + level * 0.4
            updateFillMask()
        }
    }
    
    init(enabled: Bool, size: CGFloat = 16, tooltip: String = "") {
        super.init(frame: .zero)
        
        self.isEnabled = enabled
        currentSymbolSize = size
        tooltipText = tooltip
        wantsLayer = true
        
        setupViews()
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupViews()
    }
    
    func setEnabled(_ enabled: Bool) {
        isEnabled = enabled
        updateIcon()
        updateFillMask()
    }
    
    private func setupViews() {
        layer?.backgroundColor = NSColor.clear.cgColor
        
        baseImageView = NSImageView()
        baseImageView?.imageScaling = .scaleProportionallyDown
        baseImageView?.wantsLayer = true
        addSubview(baseImageView!)
        
        fillImageView = NSImageView()
        fillImageView?.imageScaling = .scaleProportionallyDown
        fillImageView?.wantsLayer = true
        addSubview(fillImageView!)
        
        updateIcon()
    }
    
    private func updateIcon() {
        let symbolName = isEnabled ? "mic" : "mic.slash"
        let theme = Theme.current
        let normalColor = theme.foreground.withAlphaComponent(0.8)
        
        guard let image = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil) else { return }
        let config = NSImage.SymbolConfiguration(pointSize: currentSymbolSize, weight: .regular)
        guard let configuredImage = image.withSymbolConfiguration(config) else { return }
        
        baseImageView?.image = configuredImage
        baseImageView?.contentTintColor = normalColor
        
        if isEnabled {
            fillImageView?.image = configuredImage
            fillImageView?.contentTintColor = theme.foreground
            fillImageView?.isHidden = false
        } else {
            fillImageView?.isHidden = true
        }
    }
    
    override func layout() {
        super.layout()
        baseImageView?.frame = bounds
        fillImageView?.frame = bounds
        updateFillMask()
    }
    
    private func updateFillMask() {
        guard isEnabled, let fillView = fillImageView else {
            fillImageView?.layer?.mask = nil
            return
        }
        
        CATransaction.begin()
        CATransaction.setAnimationDuration(0.05)
        
        if maskLayer == nil {
            maskLayer = CALayer()
            maskLayer?.backgroundColor = NSColor.white.cgColor
        }
        
        let fillHeight = bounds.height * CGFloat(smoothedLevel)
        maskLayer?.frame = CGRect(
            x: 0,
            y: 0,
            width: bounds.width,
            height: fillHeight
        )
        
        fillView.layer?.mask = maskLayer
        
        CATransaction.commit()
    }
    
    func applyTheme() {
        updateIcon()
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
    
    override func mouseDown(with event: NSEvent) {
        onClick?()
    }
    
    override func rightMouseDown(with event: NSEvent) {
        onRightClick?()
    }
    
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        applyTheme()
    }
}
