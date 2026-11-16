import AppKit
import Foundation
import QuartzCore

@MainActor
final class ProtectedTooltip {
    private let registry: RecordingCaptureWindowRegistry
    private var sequence: UInt64 = 0
    private var presentationSequence: UInt64 = 0
    private var owner: UInt32?
    private var panel: NSPanel?
    #if SCROLL_CAPTURE_TEST
    var testPanel: NSPanel? { panel }
    #endif

    init(registry: RecordingCaptureWindowRegistry = .shared) {
        self.registry = registry
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

    func show(sequence: UInt64, owner: UInt32, text: String, dark: Bool, at point: NSPoint) async throws -> Bool {
        guard isCurrent(sequence, owner: owner), !text.isEmpty else { return false }
        await registry.waitForCaptureStartIfNeeded()
        guard isCurrent(sequence, owner: owner) else { return false }
        guard let screen = NSScreen.screens.first(where: { $0.frame.contains(point) }) ?? NSScreen.main else {
            return false
        }
        let panel = makePanel(text: text, dark: dark, point: point, screen: screen)
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
        guard isCurrent(sequence, owner: owner), self.panel === panel else {
            if self.panel === panel {
                self.panel = nil
                await retire(panel)
            }
            return false
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

    private func makePanel(text: String, dark: Bool, point: NSPoint, screen: NSScreen) -> NSPanel {
        let font = NSFont.systemFont(ofSize: NSFont.smallSystemFontSize)
        let maxTextWidth = max(1, min(360, screen.visibleFrame.width - 24))
        let textRect = (text as NSString).boundingRect(
            with: NSSize(width: maxTextWidth, height: 80),
            options: [.usesLineFragmentOrigin, .usesFontLeading],
            attributes: [.font: font]
        ).integral
        let size = NSSize(width: min(maxTextWidth, textRect.width) + 16, height: min(80, textRect.height) + 8)
        let visible = screen.visibleFrame
        let x = min(max(point.x + 12, visible.minX), max(visible.minX, visible.maxX - size.width))
        let below = point.y - 18 - size.height
        let y = below >= visible.minY ? below : min(point.y + 18, visible.maxY - size.height)
        let panel = NSPanel(
            contentRect: NSRect(origin: NSPoint(x: x, y: max(visible.minY, y)), size: size),
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
        let effect = NSVisualEffectView(frame: NSRect(origin: .zero, size: size))
        effect.material = .toolTip
        effect.blendingMode = .behindWindow
        effect.state = .active
        effect.wantsLayer = true
        effect.layer?.cornerRadius = 5
        effect.layer?.masksToBounds = true
        let label = NSTextField(labelWithString: text)
        label.font = font
        label.textColor = .labelColor
        label.lineBreakMode = .byWordWrapping
        label.maximumNumberOfLines = 4
        label.frame = NSRect(x: 8, y: 2, width: size.width - 16, height: size.height - 4)
        effect.addSubview(label)
        panel.contentView = effect
        return panel
    }
}
