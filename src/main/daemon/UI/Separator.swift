import Cocoa

class VerticalSeparator: NSView {
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
        wantsLayer = true
    }
    
    convenience init(height: CGFloat, at x: CGFloat, yOffset: CGFloat = 8) {
        self.init(frame: NSRect(x: x - 0.5, y: yOffset, width: 1, height: height))
    }
    
    func applyTheme() {
        let theme = Theme.current
        layer?.backgroundColor = theme.separatorColor.cgColor
    }
}
