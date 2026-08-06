import Cocoa

class DragHandle: NSView {
    var tooltipText: String = "Move"
    
    private var isDragging = false
    private var dragStartPoint: NSPoint = .zero
    private var trackingArea: NSTrackingArea?
    private var iconView: DragIconView!
    
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupView()
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupView()
    }
    
    private func setupView() {
        wantsLayer = true
        
        iconView = DragIconView(frame: NSRect(
            x: (bounds.width - 18) / 2,
            y: (bounds.height - 18) / 2,
            width: 18,
            height: 18
        ))
        iconView.autoresizingMask = [.minXMargin, .maxXMargin, .minYMargin, .maxYMargin]
        addSubview(iconView)
    }
    
    func applyTheme() {
        iconView.needsDisplay = true
    }
    
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        applyTheme()
    }
    
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        
        if let existing = trackingArea {
            removeTrackingArea(existing)
        }
        
        trackingArea = NSTrackingArea(
            rect: bounds,
            options: [.activeAlways, .mouseEnteredAndExited, .cursorUpdate],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(trackingArea!)
    }
    
    override func cursorUpdate(with event: NSEvent) {
        if isDragging {
            NSCursor.closedHand.set()
        } else {
            NSCursor.openHand.set()
        }
    }
    
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        return true
    }
    
    override func mouseEntered(with event: NSEvent) {
        if !isDragging {
            NSCursor.openHand.set()
            if let window = window, !tooltipText.isEmpty {
                TooltipManager.shared.show(text: tooltipText, for: self, in: window)
            }
        }
    }
    
    override func mouseExited(with event: NSEvent) {
        if !isDragging {
            NSCursor.arrow.set()
        }
        TooltipManager.shared.hide()
    }
    
    override func mouseDown(with event: NSEvent) {
        isDragging = true
        dragStartPoint = event.locationInWindow
        NSCursor.closedHand.set()
        TooltipManager.shared.hide()
    }
    
    override func mouseDragged(with event: NSEvent) {
        guard isDragging, let window = window else { return }
        
        let currentPoint = event.locationInWindow
        let deltaX = currentPoint.x - dragStartPoint.x
        let deltaY = currentPoint.y - dragStartPoint.y
        
        var newOrigin = window.frame.origin
        newOrigin.x += deltaX
        newOrigin.y += deltaY
        
        window.setFrameOrigin(newOrigin)
    }
    
    override func mouseUp(with event: NSEvent) {
        isDragging = false
        
        let point = convert(event.locationInWindow, from: nil)
        if bounds.contains(point) {
            NSCursor.openHand.set()
        } else {
            NSCursor.arrow.set()
        }
    }
}

private class DragIconView: NSView {
    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        
        let theme = Theme.current
        let color = theme.foregroundMuted
        color.setFill()
        
        let dotSize: CGFloat = 2
        let spacing: CGFloat = 4
        let cols = 2
        let rows = 3
        
        let totalWidth = CGFloat(cols - 1) * spacing + dotSize
        let totalHeight = CGFloat(rows - 1) * spacing + dotSize
        let startX = (bounds.width - totalWidth) / 2
        let startY = (bounds.height - totalHeight) / 2
        
        for row in 0..<rows {
            for col in 0..<cols {
                let x = startX + CGFloat(col) * spacing
                let y = startY + CGFloat(row) * spacing
                let rect = NSRect(x: x, y: y, width: dotSize, height: dotSize)
                let path = NSBezierPath(ovalIn: rect)
                path.fill()
            }
        }
    }
    
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        needsDisplay = true
    }
}
