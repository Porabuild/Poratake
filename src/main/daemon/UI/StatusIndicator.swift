import Cocoa

enum StatusIndicatorState {
    case idle
    case active
    case paused
}

class StatusIndicator: NSView {
    var status: StatusIndicatorState = .idle {
        didSet {
            updateAppearance()
        }
    }
    
    var pulse: Bool = false {
        didSet {
            updateAnimation()
        }
    }
    
    private let dotSize: CGFloat = 12
    private var animationTimer: Timer?
    private var isAnimating = false
    private var currentScale: CGFloat = 1.0
    private var animationPhase: CGFloat = 0
    
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupView()
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupView()
    }
    
    convenience init() {
        self.init(frame: NSRect(x: 0, y: 0, width: 12, height: 12))
    }
    
    private func setupView() {
        wantsLayer = true
        layer?.cornerRadius = dotSize / 2
    }
    
    private func updateAppearance() {
        switch status {
        case .idle:
            layer?.backgroundColor = Theme.current.foregroundMuted.cgColor
        case .active:
            layer?.backgroundColor = Theme.current.destructiveColor.cgColor
        case .paused:
            layer?.backgroundColor = NSColor.systemYellow.cgColor
        }
        updateAnimation()
    }
    
    private func updateAnimation() {
        if pulse && status == .active {
            startAnimation()
        } else {
            stopAnimation()
        }
    }
    
    private func startAnimation() {
        guard !isAnimating else { return }
        isAnimating = true
        animationPhase = 0
        
        animationTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30.0, repeats: true) { [weak self] _ in
            self?.animatePulse()
        }
    }
    
    private func stopAnimation() {
        isAnimating = false
        animationTimer?.invalidate()
        animationTimer = nil
        currentScale = 1.0
        layer?.opacity = 1.0
    }
    
    private func animatePulse() {
        animationPhase += 0.05
        if animationPhase >= .pi * 2 {
            animationPhase = 0
        }
        
        let opacity = 0.5 + 0.5 * sin(animationPhase)
        layer?.opacity = Float(opacity)
    }
    
    func applyTheme() {
        updateAppearance()
    }
    
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        applyTheme()
    }
    
    deinit {
        stopAnimation()
    }
}
