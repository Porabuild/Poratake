import Cocoa

class ScrollCapturePreviewView: BlurredPanelView {
    private let imageView: NSImageView
    private let frameCountLabel: NSTextField
    static let headerHeight: CGFloat = 28

    override init(frame frameRect: NSRect) {
        imageView = NSImageView(frame: .zero)
        frameCountLabel = NSTextField(labelWithString: "0 frames")
        super.init(frame: frameRect)
        setupImageView()
        setupLabel()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupImageView() {
        imageView.imageScaling = .scaleProportionallyDown
        imageView.imageAlignment = .alignTop
        addSubview(imageView)
    }

    private func setupLabel() {
        frameCountLabel.font = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .medium)
        frameCountLabel.alignment = .center
        addSubview(frameCountLabel)
    }

    override func layout() {
        super.layout()

        let h = Self.headerHeight
        frameCountLabel.frame = NSRect(x: 0, y: bounds.height - h, width: bounds.width, height: h)
        imageView.frame = NSRect(x: 0, y: 0, width: bounds.width, height: bounds.height - h)
    }

    func updatePreview(image: CGImage, frameCount: Int) {
        imageView.image = NSImage(cgImage: image, size: NSSize(width: image.width, height: image.height))
        frameCountLabel.stringValue = "\(frameCount) frame\(frameCount == 1 ? "" : "s")"
    }

    override func applyTheme() {
        super.applyTheme()
        frameCountLabel.textColor = Theme.current.foregroundMuted
        layer?.borderWidth = 1
        layer?.borderColor = NSColor(white: 1.0, alpha: 0.15).cgColor
    }
}
