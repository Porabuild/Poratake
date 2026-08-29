import Cocoa

func themeColor(_ value: String?) -> NSColor? {
    guard let hex = value?.trimmingCharacters(in: CharacterSet(charactersIn: "#")),
          hex.count == 6,
          let packed = UInt32(hex, radix: 16)
    else {
        return nil
    }

    return NSColor(
        red: CGFloat((packed >> 16) & 0xff) / 255,
        green: CGFloat((packed >> 8) & 0xff) / 255,
        blue: CGFloat(packed & 0xff) / 255,
        alpha: 1.0
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
