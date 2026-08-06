import Cocoa

class BlurredPanelView: NSView, ThemedView {
    private var visualEffectView: NSVisualEffectView!
    
    override init(frame: NSRect) {
        super.init(frame: frame)
        setupView()
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupView()
    }
    
    private func setupView() {
        wantsLayer = true
        layer?.cornerRadius = 8
        layer?.masksToBounds = true
        
        visualEffectView = NSVisualEffectView(frame: bounds)
        visualEffectView.autoresizingMask = [.width, .height]
        visualEffectView.blendingMode = .behindWindow
        visualEffectView.state = .active
        visualEffectView.wantsLayer = true
        visualEffectView.layer?.cornerRadius = 8
        addSubview(visualEffectView)
    }
    
    func applyTheme() {
        let isDark = Theme.isDarkMode
        
        visualEffectView.material = isDark ? .hudWindow : .menu
        
        if isDark {
            layer?.borderWidth = 1
            layer?.borderColor = NSColor(white: 1.0, alpha: 0.15).cgColor
        } else {
            layer?.borderWidth = 0
            layer?.borderColor = nil
        }
    }
}
