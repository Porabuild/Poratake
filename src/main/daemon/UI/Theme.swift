import Cocoa

struct Theme {
    let cardBackground: NSColor
    let foreground: NSColor
    let foregroundMuted: NSColor
    let separatorColor: NSColor
    let tooltipBackground: NSColor
    let tooltipForeground: NSColor
    let accentColor: NSColor
    let destructiveColor: NSColor
    
    static var current: Theme {
        isDarkMode ? dark : light
    }
    
    static var isDarkMode: Bool {
        if let appearance = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) {
            return appearance == .darkAqua
        }
        return false
    }
    
    static let dark = Theme(
        cardBackground: NSColor(red: 0.24, green: 0.24, blue: 0.24, alpha: 1.0),
        foreground: NSColor(white: 0.98, alpha: 1.0),
        foregroundMuted: NSColor(white: 0.98, alpha: 0.5),
        separatorColor: NSColor(white: 1.0, alpha: 0.15),
        tooltipBackground: NSColor(white: 0.15, alpha: 0.95),
        tooltipForeground: NSColor(white: 0.95, alpha: 1.0),
        accentColor: NSColor.systemBlue,
        destructiveColor: NSColor.systemRed
    )
    
    static let light = Theme(
        cardBackground: NSColor(white: 1.0, alpha: 1.0),
        foreground: NSColor(white: 0.0, alpha: 1.0),
        foregroundMuted: NSColor(white: 0.0, alpha: 0.5),
        separatorColor: NSColor(white: 0.0, alpha: 0.12),
        tooltipBackground: NSColor(white: 0.97, alpha: 0.95),
        tooltipForeground: NSColor(white: 0.2, alpha: 1.0),
        accentColor: NSColor.systemBlue,
        destructiveColor: NSColor.systemRed
    )
}

protocol ThemeObserver: AnyObject {
    func themeDidChange()
}

class ThemeManager {
    static let shared = ThemeManager()
    
    private var observers: [WeakThemeObserver] = []
    
    private init() {}
    
    func addObserver(_ observer: ThemeObserver) {
        observers.removeAll { $0.observer == nil }
        observers.append(WeakThemeObserver(observer))
    }
    
    func removeObserver(_ observer: ThemeObserver) {
        observers.removeAll { $0.observer === observer || $0.observer == nil }
    }
    
    func notifyThemeChange() {
        observers.removeAll { $0.observer == nil }
        for weakObserver in observers {
            weakObserver.observer?.themeDidChange()
        }
    }
}

private class WeakThemeObserver {
    weak var observer: ThemeObserver?
    
    init(_ observer: ThemeObserver) {
        self.observer = observer
    }
}
