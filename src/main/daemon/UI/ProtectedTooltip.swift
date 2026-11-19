import AppKit
import Foundation
import QuartzCore

@MainActor
struct TooltipOwner {
    private let windowID: CGWindowID
    private let fallbackFrame: NSRect?
    private let processID: Int?

    init(windowID: CGWindowID, fallbackFrame: NSRect?) {
        self.windowID = windowID
        self.fallbackFrame = fallbackFrame
        self.processID = Self.visibleWindow(windowID)?[kCGWindowOwnerPID as String] as? Int
    }

    var frame: NSRect {
        guard let processID, let window = Self.visibleWindow(windowID),
              window[kCGWindowOwnerPID as String] as? Int == processID else { return .null }
        guard let bounds = window[kCGWindowBounds as String] as? [String: Any],
              let rect = CGRect(dictionaryRepresentation: bounds as CFDictionary),
              let top = NSScreen.screens.first?.frame.maxY else { return fallbackFrame ?? .null }
        return NSRect(x: rect.minX, y: top - rect.maxY, width: rect.width, height: rect.height)
    }

    private static func visibleWindow(_ id: CGWindowID) -> [String: Any]? {
        let windows = CGWindowListCopyWindowInfo(.optionIncludingWindow, id) as? [[String: Any]]
        guard let window = windows?.first,
              window[kCGWindowNumber as String] as? UInt32 == id,
              window[kCGWindowIsOnscreen as String] as? Bool == true,
              (window[kCGWindowAlpha as String] as? Double ?? 1) > 0 else { return nil }
        return window
    }
}

@MainActor
final class ProtectedTooltip {
    struct Screen: Equatable {
        let frame: NSRect
        let visibleFrame: NSRect
    }

    private let screenAt: @MainActor (NSPoint) -> Screen?
    private let registry: RecordingCaptureWindowRegistry
    private var sequence: UInt64 = 0
    private var presentationSequence: UInt64 = 0
    private var owner: UInt32?
    private var panel: NSPanel?
    #if SCROLL_CAPTURE_TEST
    var testPanel: NSPanel? { panel }
    #endif

    init(
        registry: RecordingCaptureWindowRegistry = .shared,
        screenAt: @escaping @MainActor (NSPoint) -> Screen? = { point in
            NSScreen.screens.first { $0.frame.contains(point) }.map {
                Screen(frame: $0.frame, visibleFrame: $0.visibleFrame)
            }
        }
    ) {
        self.registry = registry
        self.screenAt = screenAt
    }

    func accept(sequence: UInt64, owner: UInt32, text: String?) -> Bool {
        guard sequence > self.sequence else { return false }
        self.sequence = sequence
        presentationSequence = sequence
        if let panel = panel {
            panel.alphaValue = 0
            self.panel = nil
            Task { @MainActor in await self.retire(panel) }
        }
        self.owner = text == nil ? nil : owner
        return text != nil
    }

    func show(
        sequence: UInt64,
        owner: UInt32,
        text: String,
        dark: Bool,
        at pointerLocation: @autoclosure () -> NSPoint,
        ownerFrame: @autoclosure () -> NSRect? = nil
    ) async throws -> Bool {
        guard isCurrent(sequence, owner: owner), !text.isEmpty else { return false }
        await registry.waitForCaptureStartIfNeeded()
        guard isCurrent(sequence, owner: owner) else { return false }
        let point = pointerLocation()
        let initialFrame = ownerFrame()
        guard initialFrame?.contains(point) != false else { return false }
        guard let screen = screenAt(point) else { return false }
        let (panel, label) = makePanel(text: text, dark: dark)
        placePanel(panel, label: label, point: point, ownerFrame: initialFrame, screen: screen)
        self.panel = panel
        panel.orderFrontRegardless()
        guard registry.register(panel) != nil else {
            panel.orderOut(nil)
            self.panel = nil
            throw RecordingCaptureFilterError.unavailableWindowNumber
        }
        do {
            try await registry.updateFilterIfNeeded()
            await registry.waitForScreenshots()
        } catch {
            guard self.panel === panel else { return false }
            self.panel = nil
            await retire(panel)
            throw error
        }
        let revealPoint = pointerLocation()
        let revealFrame = ownerFrame()
        guard isCurrent(sequence, owner: owner), self.panel === panel,
              revealFrame?.contains(revealPoint) != false,
              let revealScreen = screenAt(revealPoint) else {
            if self.panel === panel {
                self.panel = nil
                await retire(panel)
            }
            return false
        }
        if revealScreen != screen {
            placePanel(panel, label: label, point: revealPoint, ownerFrame: revealFrame, screen: revealScreen)
        } else if revealPoint != point || revealFrame != initialFrame {
            panel.setFrameOrigin(panelOrigin(
                size: panel.frame.size, point: revealPoint, ownerFrame: revealFrame, screen: revealScreen
            ))
        }
        panel.alphaValue = 1
        return true
    }

    private func isCurrent(_ sequence: UInt64, owner: UInt32) -> Bool {
        presentationSequence == sequence && self.owner == owner
    }

    private func retire(_ panel: NSPanel) async {
        panel.alphaValue = 0
        CATransaction.flush()
        await registry.waitForScreenshots()
        panel.orderOut(nil)
        registry.unregister(panel)
        do {
            try await registry.updateFilterIfNeeded()
        } catch {
            NSLog("[tooltip] failed to update capture exclusions: %@", error.localizedDescription)
        }
    }

    private func makePanel(text: String, dark: Bool) -> (NSPanel, NSTextField) {
        let panel = NSPanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.level = NSWindow.Level(rawValue: NSWindow.Level.screenSaver.rawValue + 1)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.ignoresMouseEvents = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.alphaValue = 0
        panel.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        let effect = NSVisualEffectView(frame: .zero)
        effect.material = .toolTip
        effect.blendingMode = .behindWindow
        effect.state = .active
        effect.wantsLayer = true
        effect.layer?.cornerRadius = 5
        effect.layer?.masksToBounds = true
        let label = NSTextField(labelWithString: text)
        label.font = .systemFont(ofSize: NSFont.smallSystemFontSize)
        label.textColor = .labelColor
        label.lineBreakMode = .byWordWrapping
        label.maximumNumberOfLines = 4
        label.cell?.truncatesLastVisibleLine = true
        effect.addSubview(label)
        panel.contentView = effect
        return (panel, label)
    }

    private func placePanel(
        _ panel: NSPanel,
        label: NSTextField,
        point: NSPoint,
        ownerFrame: NSRect?,
        screen: Screen
    ) {
        let maxTextWidth = max(1, min(360, screen.visibleFrame.width - 24))
        let fitted = label.sizeThatFits(NSSize(width: maxTextWidth, height: 80))
        let labelSize = NSSize(width: min(maxTextWidth, ceil(fitted.width)), height: ceil(fitted.height))
        let padding = NSSize(width: 8, height: 4)
        let size = NSSize(width: labelSize.width + padding.width * 2, height: labelSize.height + padding.height * 2)
        let origin = panelOrigin(size: size, point: point, ownerFrame: ownerFrame, screen: screen)
        label.frame = NSRect(origin: NSPoint(x: padding.width, y: padding.height), size: labelSize)
        panel.setFrame(NSRect(origin: origin, size: size), display: true)
    }

    private func panelOrigin(size: NSSize, point: NSPoint, ownerFrame: NSRect?, screen: Screen) -> NSPoint {
        let visible = screen.visibleFrame
        var x = min(max(point.x + 12, visible.minX), max(visible.minX, visible.maxX - size.width))
        let anchorFrame = ownerFrame.flatMap { screen.frame.intersects($0) ? $0 : nil }
        let below = anchorFrame.map { $0.minY - size.height - 4 } ?? point.y - 18 - size.height
        let above = anchorFrame.map { $0.maxY + 4 } ?? point.y + 18
        let belowFits = below >= visible.minY
        let aboveFits = above + size.height <= visible.maxY
        var preferredY = point.y - 18 - size.height
        if belowFits {
            preferredY = below
        } else if aboveFits {
            preferredY = above
        } else if let anchorFrame {
            let right = max(visible.minX, anchorFrame.maxX + 4)
            let left = min(visible.maxX - size.width, anchorFrame.minX - size.width - 4)
            if right + size.width <= visible.maxX {
                x = right
                preferredY = point.y - size.height / 2
            } else if left >= visible.minX {
                x = left
                preferredY = point.y - size.height / 2
            }
        }
        let y = min(max(preferredY, visible.minY), max(visible.minY, visible.maxY - size.height))
        return NSPoint(x: x, y: y)
    }
}
