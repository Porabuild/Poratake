import Cocoa

struct FloatingPanelConfig {
    var cornerRadius: CGFloat = 8
    var level: NSWindow.Level = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.maximumWindow)) + 1)
    var hasShadow: Bool = true
    var canBecomeKey: Bool = true
    var escapeToClose: Bool = true
    var onEscape: (() -> Void)? = nil
}

class FloatingPanel: NSPanel, ThemeObserver {
    private var config: FloatingPanelConfig
    
    init(
        contentRect: NSRect,
        config: FloatingPanelConfig = FloatingPanelConfig()
    ) {
        self.config = config
        
        super.init(
            contentRect: contentRect,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        
        level = config.level
        isOpaque = false
        backgroundColor = .clear
        hasShadow = config.hasShadow
        isMovableByWindowBackground = false
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        hidesOnDeactivate = false
        acceptsMouseMovedEvents = true
        ignoresMouseEvents = false
        sharingType = .none
        
        ThemeManager.shared.addObserver(self)
    }
    
    override var canBecomeKey: Bool { config.canBecomeKey }
    override var canBecomeMain: Bool { false }
    
    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 && config.escapeToClose {
            config.onEscape?()
        } else {
            super.keyDown(with: event)
        }
    }
    
    func themeDidChange() {
        if let themedView = contentView as? ThemedView {
            themedView.applyTheme()
        }
    }
    
    func setPosition(x: Int, y: Int) {
        let mainScreenHeight = NSScreen.main?.frame.height ?? 0
        let cocoaY = mainScreenHeight - CGFloat(y) - frame.height
        setFrameOrigin(NSPoint(x: CGFloat(x), y: cocoaY))
    }
    
    func setThemedContentView(_ view: ThemedView) {
        contentView = view
        view.applyTheme()
    }
    
    deinit {
        ThemeManager.shared.removeObserver(self)
    }
}

protocol ThemedView where Self: NSView {
    func applyTheme()
}
