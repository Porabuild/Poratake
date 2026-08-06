import Cocoa

class AudioLevelIndicator: NSView {
    private let barWidth: CGFloat = 3
    private let barHeight: CGFloat = 20
    
    private var backgroundLayer: CALayer?
    private var fillLayer: CALayer?
    private var smoothedLevel: Float = 0
    
    var level: Float = 0 {
        didSet {
            smoothedLevel = smoothedLevel * 0.6 + level * 0.4
            updateFill()
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
    
    convenience init() {
        self.init(frame: NSRect(x: 0, y: 0, width: 3, height: 20))
    }
    
    private func setupView() {
        wantsLayer = true
        layer?.masksToBounds = true
        
        let cornerRadius = barWidth / 2
        
        backgroundLayer = CALayer()
        backgroundLayer?.backgroundColor = Theme.current.foregroundMuted.cgColor
        backgroundLayer?.cornerRadius = cornerRadius
        layer?.addSublayer(backgroundLayer!)
        
        fillLayer = CALayer()
        fillLayer?.backgroundColor = Theme.current.foreground.cgColor
        fillLayer?.cornerRadius = cornerRadius
        layer?.addSublayer(fillLayer!)
        
        layoutLayers()
    }
    
    override func layout() {
        super.layout()
        layoutLayers()
    }
    
    private func layoutLayers() {
        let cornerRadius = barWidth / 2
        
        backgroundLayer?.frame = bounds
        backgroundLayer?.cornerRadius = cornerRadius
        
        fillLayer?.cornerRadius = cornerRadius
        updateFill()
    }
    
    private func updateFill() {
        CATransaction.begin()
        CATransaction.setAnimationDuration(0.05)
        
        let fillHeight = bounds.height * CGFloat(smoothedLevel)
        fillLayer?.frame = CGRect(
            x: 0,
            y: 0,
            width: bounds.width,
            height: fillHeight
        )
        
        CATransaction.commit()
    }
    
    func applyTheme() {
        backgroundLayer?.backgroundColor = Theme.current.foregroundMuted.cgColor
        fillLayer?.backgroundColor = Theme.current.foreground.cgColor
    }
    
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        applyTheme()
    }
}
