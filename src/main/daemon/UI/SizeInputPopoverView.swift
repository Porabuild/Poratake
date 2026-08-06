import Cocoa

class SizeInputPopoverView: NSView, ThemedView {
    private let widthLabel = NSTextField(labelWithString: "W")
    private let heightLabel = NSTextField(labelWithString: "H")
    private let widthField = NSTextField()
    private let heightField = NSTextField()
    private let applyButton = NSButton(title: "Apply", target: nil, action: nil)
    private let minimumSize = 20

    var onApply: ((Int, Int) -> Void)?

    init(width: Int, height: Int, onApply: @escaping (Int, Int) -> Void) {
        self.onApply = onApply
        super.init(frame: NSRect(x: 0, y: 0, width: 220, height: 88))
        setupContent()
        setSize(width: width, height: height)
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupContent()
    }

    func setSize(width: Int, height: Int) {
        widthField.stringValue = String(width)
        heightField.stringValue = String(height)
    }

    func focusWidthField() {
        window?.makeFirstResponder(widthField)
        widthField.selectText(nil)
    }

    func applyTheme() {
        let theme = Theme.current
        widthLabel.textColor = theme.foregroundMuted
        heightLabel.textColor = theme.foregroundMuted
        widthField.textColor = theme.foreground
        heightField.textColor = theme.foreground
    }

    private func setupContent() {
        wantsLayer = true

        widthLabel.alignment = .center
        widthLabel.frame = NSRect(x: 12, y: 52, width: 18, height: 20)
        addSubview(widthLabel)

        widthField.frame = NSRect(x: 34, y: 48, width: 70, height: 24)
        configureField(widthField)
        addSubview(widthField)

        heightLabel.alignment = .center
        heightLabel.frame = NSRect(x: 116, y: 52, width: 18, height: 20)
        addSubview(heightLabel)

        heightField.frame = NSRect(x: 138, y: 48, width: 70, height: 24)
        configureField(heightField)
        addSubview(heightField)

        applyButton.frame = NSRect(x: 130, y: 12, width: 78, height: 28)
        applyButton.bezelStyle = .rounded
        applyButton.target = self
        applyButton.action = #selector(applySize)
        addSubview(applyButton)

        widthField.nextKeyView = heightField
        heightField.nextKeyView = applyButton
        applyTheme()
    }

    private func configureField(_ field: NSTextField) {
        field.formatter = makeFormatter()
        field.alignment = .right
        field.font = NSFont.monospacedDigitSystemFont(ofSize: 13, weight: .regular)
        field.bezelStyle = .roundedBezel
        field.target = self
        field.action = #selector(applySize)
    }

    private func makeFormatter() -> NumberFormatter {
        let formatter = NumberFormatter()
        formatter.numberStyle = .none
        formatter.allowsFloats = false
        formatter.minimum = NSNumber(value: minimumSize)
        formatter.maximum = NSNumber(value: 100000)
        return formatter
    }

    private func value(from field: NSTextField) -> Int? {
        let trimmedValue = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let value = Int(trimmedValue), value >= minimumSize else {
            return nil
        }
        return value
    }

    @objc private func applySize() {
        guard let width = value(from: widthField),
              let height = value(from: heightField) else {
            NSSound.beep()
            return
        }

        onApply?(width, height)
    }
}
