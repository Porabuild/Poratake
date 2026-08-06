import Cocoa

struct AspectRatio {
    let name: String
    let width: Int
    let height: Int
    
    var ratio: Double? {
        guard width > 0 && height > 0 else { return nil }
        return Double(width) / Double(height)
    }
    
    var displayName: String {
        if width == 0 && height == 0 {
            return "Free"
        }
        return "\(width):\(height)"
    }
    
    static let free = AspectRatio(name: "Free", width: 0, height: 0)
    static let ratio16x9 = AspectRatio(name: "16:9", width: 16, height: 9)
    static let ratio9x16 = AspectRatio(name: "9:16", width: 9, height: 16)
    static let ratio4x3 = AspectRatio(name: "4:3", width: 4, height: 3)
    static let ratio1x1 = AspectRatio(name: "1:1", width: 1, height: 1)
    static let ratio21x9 = AspectRatio(name: "21:9", width: 21, height: 9)
    static let ratio4x5 = AspectRatio(name: "4:5", width: 4, height: 5)
    static let ratio3x2 = AspectRatio(name: "3:2", width: 3, height: 2)
    
    static let all: [AspectRatio] = [
        .free,
        .ratio16x9,
        .ratio9x16,
        .ratio4x3,
        .ratio1x1,
        .ratio21x9,
        .ratio4x5,
        .ratio3x2
    ]
}

class AspectRatioButton: NSView {
    var tooltipText: String = "Aspect Ratio"
    var onSelectRatio: ((AspectRatio) -> Void)?
    
    private var trackingArea: NSTrackingArea?
    private var currentSymbolSize: CGFloat = 16
    private var currentRatio: AspectRatio = .free
    private var imageView: NSImageView?
    
    init(size: CGFloat = 16, tooltip: String = "Aspect Ratio") {
        super.init(frame: .zero)
        
        currentSymbolSize = size
        tooltipText = tooltip
        wantsLayer = true
        
        setupViews()
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupViews()
    }
    
    func setRatio(_ ratio: AspectRatio) {
        currentRatio = ratio
        updateIcon()
    }
    
    private func setupViews() {
        layer?.backgroundColor = NSColor.clear.cgColor
        
        imageView = NSImageView()
        imageView?.imageScaling = .scaleProportionallyDown
        imageView?.wantsLayer = true
        addSubview(imageView!)
        
        updateIcon()
    }
    
    private func updateIcon() {
        let symbolName = currentRatio.width == 0 ? "rectangle.dashed" : "rectangle.ratio"
        let theme = Theme.current
        let normalColor = theme.foreground.withAlphaComponent(0.8)
        
        guard let image = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil) else { return }
        let config = NSImage.SymbolConfiguration(pointSize: currentSymbolSize, weight: .regular)
        guard let configuredImage = image.withSymbolConfiguration(config) else { return }
        
        imageView?.image = configuredImage
        imageView?.contentTintColor = normalColor
    }
    
    override func layout() {
        super.layout()
        imageView?.frame = bounds
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
        showMenu()
    }
    
    override func rightMouseDown(with event: NSEvent) {
        showMenu()
    }
    
    private func showMenu() {
        TooltipManager.shared.hide()
        
        let menu = NSMenu()
        menu.autoenablesItems = false
        
        let headerItem = NSMenuItem(title: "Aspect Ratio", action: nil, keyEquivalent: "")
        headerItem.isEnabled = false
        menu.addItem(headerItem)
        
        for ratio in AspectRatio.all {
            let item = NSMenuItem(title: ratio.displayName, action: #selector(ratioSelected(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = ratio
            item.state = (currentRatio.width == ratio.width && currentRatio.height == ratio.height) ? .on : .off
            menu.addItem(item)
        }
        
        let point = NSPoint(x: 0, y: bounds.height)
        menu.popUp(positioning: nil, at: point, in: self)
    }
    
    @objc private func ratioSelected(_ sender: NSMenuItem) {
        guard let ratio = sender.representedObject as? AspectRatio else { return }
        currentRatio = ratio
        updateIcon()
        onSelectRatio?(ratio)
    }
    
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        applyTheme()
    }
}
